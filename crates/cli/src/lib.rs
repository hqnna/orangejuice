use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{ArgAction, Args, Parser, Subcommand};
use oj_diag::SourceMap;
use oj_lexer::Interner;

pub const VERSION_LINE: &str = concat!(
  "orangejuice ",
  env!("CARGO_PKG_VERSION"),
  " (jai beta 0.2.009 compatible, LLVM 19)"
);

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_FAILURE: u8 = 1;
pub const EXIT_USAGE: u8 = 2;

/// The `oj` command line, mirroring the jai driver so that `jai a.jai - -x`
/// translates to `oj build a.jai - -x`. The single-dash options of `build` and
/// `run` are the reference compiler's: `docs/spec.md` §5.1 tables them, and
/// `vendor/jai/modules/Default_Metaprogram.jai` defines them.
#[derive(Debug, Parser)]
#[command(
  name = "oj",
  about = "orangejuice: a cleanroom implementation of the Jai programming language",
  disable_version_flag = true,
  arg_required_else_help = true
)]
pub struct Cli {
  /// Print the version and exit
  #[arg(short = 'V', long = "version", action = ArgAction::SetTrue)]
  pub version: bool,

  #[command(subcommand)]
  pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
  /// Compile a Jai program
  Build(BuildArgs),
  /// Compile a Jai program, then execute the produced program
  #[command(after_help = "Arguments after `--` are passed to the compiled program.")]
  Run(RunArgs),
  /// Print intermediate compiler output for a single file
  Dump {
    #[command(subcommand)]
    stage: DumpStage,
  },
  /// Print the version
  Version,
}

#[derive(Debug, Args)]
pub struct BuildArgs {
  /// The Jai file to compile
  pub file: PathBuf,

  /// Options forwarded to the metaprogram, in jai syntax
  #[arg(allow_hyphen_values = true, num_args = 0.., value_name = "JAI_OPTION")]
  pub options: Vec<String>,
}

#[derive(Debug, Args)]
pub struct RunArgs {
  /// The Jai file to compile and run
  pub file: PathBuf,

  /// Options forwarded to the metaprogram, in jai syntax
  #[arg(allow_hyphen_values = true, num_args = 0.., value_name = "JAI_OPTION")]
  pub options: Vec<String>,

  /// Arguments after `--`, passed to the compiled program
  #[arg(skip)]
  pub program_args: Vec<OsString>,
}

#[derive(Debug, Subcommand)]
pub enum DumpStage {
  /// The token stream produced by the lexer
  Tokens { file: PathBuf },
  /// The parsed syntax tree
  Ast {
    file: PathBuf,
    /// Print the tree form instead of the source form
    #[arg(long)]
    tree: bool,
  },
  /// The scope tree the file resolves into
  Scopes {
    file: PathBuf,
    /// Resolve this file alone, without following its `#load`s and `#import`s
    #[arg(long)]
    file_only: bool,
  },
  /// The types the file declares, with their layout
  Types {
    file: PathBuf,
    /// Check this file alone, without following its `#load`s and `#import`s
    #[arg(long)]
    file_only: bool,
  },
  /// The typed IR of the live procedures
  Ir {
    file: PathBuf,
    /// Restrict the listing to one procedure
    #[arg(long, value_name = "NAME")]
    proc: Option<String>,
  },
  /// The generated assembly, or LLVM IR with `--llvm`
  Asm {
    file: PathBuf,
    /// Restrict the listing to one procedure
    #[arg(long, value_name = "NAME")]
    proc: Option<String>,
    /// Print LLVM IR instead of target assembly
    #[arg(long)]
    llvm: bool,
  },
}

pub fn run<I, T>(args: I) -> ExitCode
where
  I: IntoIterator<Item = T>,
  T: Into<OsString>,
{
  ExitCode::from(exit_code(args))
}

pub fn exit_code<I, T>(args: I) -> u8
where
  I: IntoIterator<Item = T>,
  T: Into<OsString>,
{
  let (compiler_args, program_args) =
    split_program_args(args.into_iter().map(Into::into).collect());

  match Cli::try_parse_from(compiler_args) {
    Ok(mut cli) => match attach_program_args(&mut cli, program_args) {
      Ok(()) => execute(&cli),
      Err(message) => {
        eprintln!("error: {message}");
        EXIT_USAGE
      }
    },
    Err(error) => {
      let _ = error.print();
      match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => EXIT_SUCCESS,
        _ => EXIT_USAGE,
      }
    }
  }
}

/// Splits off the arguments of the compiled program, which clap cannot do
/// itself: the jai-style option lists accept hyphenated values, so a trailing
/// `--` would be swallowed as one of them.
fn split_program_args(mut argv: Vec<OsString>) -> (Vec<OsString>, Option<Vec<OsString>>) {
  match argv.iter().position(|argument| argument == "--") {
    Some(index) => {
      let program_args = argv.split_off(index + 1);
      argv.pop();
      (argv, Some(program_args))
    }
    None => (argv, None),
  }
}

fn attach_program_args(cli: &mut Cli, program_args: Option<Vec<OsString>>) -> Result<(), String> {
  let Some(program_args) = program_args else {
    return Ok(());
  };

  match &mut cli.command {
    Some(Command::Run(args)) => {
      args.program_args = program_args;
      Ok(())
    }
    _ => Err("arguments after `--` are only accepted by `oj run`".to_string()),
  }
}

fn execute(cli: &Cli) -> u8 {
  if cli.version {
    println!("{VERSION_LINE}");
    return EXIT_SUCCESS;
  }

  match &cli.command {
    Some(Command::Version) => {
      println!("{VERSION_LINE}");
      EXIT_SUCCESS
    }
    Some(Command::Build(args)) => build(&args.file, &args.options, None),
    Some(Command::Run(args)) => build(&args.file, &args.options, Some(&args.program_args)),
    Some(Command::Dump { stage }) => match stage {
      DumpStage::Tokens { file } => dump_tokens(file),
      DumpStage::Ast { file, tree } => dump_ast(file, *tree),
      DumpStage::Scopes { file, file_only } => dump_scopes(file, *file_only),
      DumpStage::Types { file, file_only } => dump_types(file, *file_only),
      DumpStage::Ir { file, proc } => dump(file, oj_driver::Stage::Ir, proc.as_deref()),
      DumpStage::Asm { file, llvm, proc } => {
        let stage = if *llvm {
          oj_driver::Stage::LlvmIr
        } else {
          oj_driver::Stage::Assembly
        };
        dump(file, stage, proc.as_deref())
      }
    },
    None => EXIT_USAGE,
  }
}

/// `oj dump tokens`: lexes one file and prints its token stream. Diagnostics go
/// to stderr in the reference layout; only an error makes the command fail.
fn dump_tokens(path: &Path) -> u8 {
  let sources = SourceMap::new();
  let id = match oj_source::load_file(&sources, path) {
    Ok(id) => id,
    Err(error) => {
      eprintln!("error: could not read {}: {error}", path.display());
      return EXIT_FAILURE;
    }
  };

  let file = sources.file(id);
  let interner = Interner::new();
  let lexed = oj_lexer::tokenize(file.bytes(), id, &interner);

  print!("{}", oj_lexer::dump_tokens(&lexed.tokens, &file, &interner));
  for diagnostic in &lexed.diagnostics {
    eprint!("{}", oj_diag::render(diagnostic, &file));
  }

  if lexed.has_errors() {
    EXIT_FAILURE
  } else {
    EXIT_SUCCESS
  }
}

/// `oj dump ast`: parses one file and prints it back as source, or as the tree
/// form with `--tree`.
fn dump_ast(path: &Path, tree: bool) -> u8 {
  let sources = SourceMap::new();
  let id = match oj_source::load_file(&sources, path) {
    Ok(id) => id,
    Err(error) => {
      eprintln!("error: could not read {}: {error}", path.display());
      return EXIT_FAILURE;
    }
  };

  let file = sources.file(id);
  let interner = Interner::new();
  let parsed = oj_syntax::parse(file.bytes(), id, &interner);

  if tree {
    print!(
      "{}",
      oj_syntax::print_tree(&parsed.ast, parsed.root, &interner)
    );
  } else {
    print!(
      "{}",
      oj_syntax::print_source(&parsed.ast, parsed.root, &interner)
    );
  }
  for diagnostic in &parsed.diagnostics {
    eprint!("{}", oj_diag::render(diagnostic, &file));
  }

  if parsed.has_errors() {
    EXIT_FAILURE
  } else {
    EXIT_SUCCESS
  }
}

/// `oj dump scopes`: resolves one file — and, unless `--file-only`, everything
/// it `#load`s and `#import`s — and prints the scope tree it produced.
/// Undeclared identifiers are reported in one batch (**L§11.7**).
fn dump_scopes(path: &Path, file_only: bool) -> u8 {
  let sources = SourceMap::new();
  let interner = Interner::new();
  let options = if file_only {
    oj_scope::Options::single_file()
  } else {
    oj_scope::Options {
      jai_dir: jai_dir(),
      ..oj_scope::Options::default()
    }
  };

  let program = oj_scope::Program::build(&sources, &interner, path, options);
  print!("{}", oj_scope::print_scopes(&program));
  println!(
    "{}",
    oj_scope::summary(program.tree(), program.units().len())
  );

  // A file resolved on its own has no imports to look names up in, so only
  // what it gets wrong by itself is worth reporting.
  let undeclared = if file_only {
    Vec::new()
  } else {
    oj_scope::undeclared_identifiers(&program)
  };
  let mut failed = program.has_errors() || !undeclared.is_empty();
  for diagnostic in program
    .diagnostics()
    .iter()
    .cloned()
    .chain(oj_scope::undeclared_diagnostics(&undeclared))
  {
    let file = sources.file(diagnostic.source);
    eprint!("{}", oj_diag::render(&diagnostic, &file));
  }

  if program.units().is_empty() {
    failed = true;
  }
  if failed { EXIT_FAILURE } else { EXIT_SUCCESS }
}

/// `oj dump types`: resolves one file the way `oj dump scopes` does, then
/// types every declaration and prints the scope tree with each name's type and
/// the layout of every struct and enum it declares.
fn dump_types(path: &Path, file_only: bool) -> u8 {
  let sources = SourceMap::new();
  let interner = Interner::new();
  let options = if file_only {
    oj_scope::Options::single_file()
  } else {
    oj_scope::Options {
      jai_dir: jai_dir(),
      ..oj_scope::Options::default()
    }
  };

  let program = oj_scope::Program::build(&sources, &interner, path, options);
  let mut checker = oj_sema::Checker::new(&program);
  checker.check();

  print!("{}", oj_sema::print_types(&checker));
  println!("{}", oj_sema::summary(&checker));

  for diagnostic in program.diagnostics().iter().chain(checker.diagnostics()) {
    let file = sources.file(diagnostic.source);
    eprint!("{}", oj_diag::render(diagnostic, &file));
  }

  if program.has_errors() || checker.has_errors() || program.units().is_empty() {
    EXIT_FAILURE
  } else {
    EXIT_SUCCESS
  }
}

fn jai_dir() -> Option<PathBuf> {
  oj_driver::jai_dir()
}

/// `oj dump ir` and `oj dump asm`: the whole pipeline, stopped one stage early
/// and printed.
fn dump(path: &Path, stage: oj_driver::Stage, only: Option<&str>) -> u8 {
  let options = oj_driver::BuildOptions::new();
  let report = oj_driver::run(path, &options, stage, only);
  print!("{}", report.output);
  for diagnostic in &report.diagnostics {
    eprint!("{diagnostic}");
  }
  if report.failed {
    EXIT_FAILURE
  } else {
    EXIT_SUCCESS
  }
}

/// `oj build`, and `oj run` when `program_args` is given: compile, link, and
/// then execute what came out.
fn build(path: &Path, arguments: &[String], program_args: Option<&[OsString]>) -> u8 {
  let parsed = match oj_driver::parse(arguments) {
    Ok(parsed) => parsed,
    Err(error) => {
      eprintln!("{}", error.message);
      return EXIT_FAILURE;
    }
  };
  if parsed.options.print_version {
    println!("{VERSION_LINE}");
    return EXIT_SUCCESS;
  }
  for deferred in &parsed.deferred {
    eprintln!(
      "error: '{}' is not implemented yet (planned for milestone {})",
      deferred.option, deferred.milestone
    );
  }
  if !parsed.deferred.is_empty() {
    return EXIT_FAILURE;
  }

  let report = oj_driver::run(path, &parsed.options, oj_driver::Stage::Executable, None);
  for diagnostic in &report.diagnostics {
    eprint!("{diagnostic}");
  }
  if report.failed {
    return EXIT_FAILURE;
  }
  let Some(executable) = report.executable else {
    return EXIT_SUCCESS;
  };
  if parsed.options.verbose
    && let Some(line) = &report.link_line
  {
    eprintln!("{line}");
  }
  let Some(program_args) = program_args else {
    if !parsed.options.quiet {
      println!("{}", executable.display());
    }
    return EXIT_SUCCESS;
  };
  match std::process::Command::new(&executable)
    .args(program_args)
    .status()
  {
    Ok(status) => status.code().unwrap_or(EXIT_FAILURE as i32) as u8,
    Err(error) => {
      eprintln!("error: could not run {}: {error}", executable.display());
      EXIT_FAILURE
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use clap::CommandFactory;

  fn parse(argv: &[&str]) -> Cli {
    let (compiler_args, program_args) =
      split_program_args(argv.iter().map(OsString::from).collect());
    let mut cli = Cli::try_parse_from(compiler_args).expect("the command line should parse");
    attach_program_args(&mut cli, program_args).expect("the program arguments should attach");
    cli
  }

  #[test]
  fn command_definition_is_valid() {
    Cli::command().debug_assert();
  }

  #[test]
  fn version_line_names_the_reference_compiler_and_backend() {
    assert_eq!(
      VERSION_LINE,
      format!(
        "orangejuice {} (jai beta 0.2.009 compatible, LLVM 19)",
        env!("CARGO_PKG_VERSION")
      )
    );
  }

  #[test]
  fn version_is_available_as_subcommand_and_flag() {
    assert_eq!(exit_code(["oj", "version"]), EXIT_SUCCESS);
    assert_eq!(exit_code(["oj", "--version"]), EXIT_SUCCESS);
    assert_eq!(exit_code(["oj", "-V"]), EXIT_SUCCESS);
  }

  #[test]
  fn help_succeeds_but_a_missing_file_is_a_usage_error() {
    assert_eq!(exit_code(["oj", "help"]), EXIT_SUCCESS);
    assert_eq!(exit_code(["oj", "--help"]), EXIT_SUCCESS);
    assert_eq!(exit_code(["oj"]), EXIT_USAGE);
    assert_eq!(exit_code(["oj", "build"]), EXIT_USAGE);
    assert_eq!(exit_code(["oj", "frobnicate"]), EXIT_USAGE);
  }

  #[test]
  fn a_missing_file_fails_rather_than_pretending_to_work() {
    assert_eq!(exit_code(["oj", "build", "first.jai"]), EXIT_FAILURE);
    assert_eq!(exit_code(["oj", "dump", "ast", "first.jai"]), EXIT_FAILURE);
  }

  #[test]
  fn an_unknown_build_option_is_reported_in_the_reference_wording() {
    // The option list is rejected before the file is even looked at.
    assert_eq!(
      exit_code(["oj", "build", "first.jai", "-frobnicate"]),
      EXIT_FAILURE
    );
  }

  fn compile_stage(stage: &str, extra: &[&str], source: &str) -> (u8, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("input.jai");
    std::fs::write(&path, source).expect("the input should be writable");
    let mut argv = vec!["oj", "dump", stage, path.to_str().unwrap()];
    argv.extend_from_slice(extra);
    (exit_code(argv), dir)
  }

  #[test]
  fn the_back_end_stages_run_when_the_distribution_is_present() {
    if oj_driver::jai_dir().is_none() {
      return;
    }
    let program = "main :: () { n := 1 + 2; }\n";
    assert_eq!(compile_stage("ir", &[], program).0, EXIT_SUCCESS);
    assert_eq!(compile_stage("asm", &["--llvm"], program).0, EXIT_SUCCESS);
    assert_eq!(compile_stage("asm", &[], program).0, EXIT_SUCCESS);
  }

  #[test]
  fn a_program_needing_a_later_milestone_fails_the_back_end_stages() {
    if oj_driver::jai_dir().is_none() {
      return;
    }
    assert_eq!(
      compile_stage(
        "ir",
        &[],
        "#import \"Basic\";\nmain :: () { print(\"x\"); }\n"
      )
      .0,
      EXIT_FAILURE
    );
  }

  fn dump_tokens_of(source: &str) -> (u8, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("input.jai");
    std::fs::write(&path, source).expect("the input should be writable");
    (
      exit_code(["oj", "dump", "tokens", path.to_str().unwrap()]),
      dir,
    )
  }

  #[test]
  fn dump_tokens_succeeds_on_a_well_formed_file() {
    assert_eq!(dump_tokens_of("main :: () {}\n").0, EXIT_SUCCESS);
  }

  #[test]
  fn dump_tokens_fails_on_a_lexer_error_or_a_missing_file() {
    assert_eq!(dump_tokens_of("x :: 1.2.3;\n").0, EXIT_FAILURE);
    assert_eq!(
      exit_code(["oj", "dump", "tokens", "no/such/file.jai"]),
      EXIT_FAILURE
    );
  }

  #[test]
  fn dump_tokens_tolerates_a_file_that_only_warns() {
    assert_eq!(dump_tokens_of("x :: \"a\\q\";\n").0, EXIT_SUCCESS);
  }

  fn dump_ast_of(source: &str, tree: bool) -> (u8, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("input.jai");
    std::fs::write(&path, source).expect("the input should be writable");
    let mut argv = vec!["oj", "dump", "ast", path.to_str().unwrap()];
    if tree {
      argv.push("--tree");
    }
    (exit_code(argv), dir)
  }

  #[test]
  fn dump_ast_succeeds_on_a_well_formed_file() {
    assert_eq!(dump_ast_of("main :: () {\n}\n", false).0, EXIT_SUCCESS);
    assert_eq!(dump_ast_of("main :: () {\n}\n", true).0, EXIT_SUCCESS);
  }

  #[test]
  fn dump_ast_fails_on_a_parse_error_or_a_missing_file() {
    assert_eq!(dump_ast_of("main :: ( {\n", false).0, EXIT_FAILURE);
    assert_eq!(
      exit_code(["oj", "dump", "ast", "no/such/file.jai"]),
      EXIT_FAILURE
    );
  }

  fn dump_scopes_of(source: &str) -> (u8, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("input.jai");
    std::fs::write(&path, source).expect("the input should be writable");
    (
      exit_code([
        "oj",
        "dump",
        "scopes",
        "--file-only",
        path.to_str().unwrap(),
      ]),
      dir,
    )
  }

  #[test]
  fn dump_scopes_succeeds_on_a_well_formed_file() {
    assert_eq!(
      dump_scopes_of("Point :: struct { x: float; }\nmain :: () {}\n").0,
      EXIT_SUCCESS
    );
  }

  #[test]
  fn dump_scopes_fails_on_a_redeclaration_or_a_missing_file() {
    assert_eq!(dump_scopes_of("x := 1;\nx := 2;\n").0, EXIT_FAILURE);
    assert_eq!(
      exit_code(["oj", "dump", "scopes", "no/such/file.jai"]),
      EXIT_FAILURE
    );
  }

  fn dump_types_of(source: &str) -> (u8, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("input.jai");
    std::fs::write(&path, source).expect("the input should be writable");
    (
      exit_code(["oj", "dump", "types", "--file-only", path.to_str().unwrap()]),
      dir,
    )
  }

  #[test]
  fn dump_types_succeeds_on_a_well_formed_file() {
    assert_eq!(
      dump_types_of("Point :: struct { x, y: float; }\nmain :: () {}\n").0,
      EXIT_SUCCESS
    );
  }

  #[test]
  fn dump_types_fails_on_a_struct_that_contains_itself_or_a_missing_file() {
    assert_eq!(
      dump_types_of("Bad :: struct { self: Bad; }\n").0,
      EXIT_FAILURE
    );
    assert_eq!(
      exit_code(["oj", "dump", "types", "no/such/file.jai"]),
      EXIT_FAILURE
    );
  }

  #[test]
  fn dump_scopes_on_one_file_does_not_report_the_names_its_imports_would_supply() {
    // Resolved on its own the file has nothing to look `print` up in, so it is
    // not an undeclared identifier here.
    assert_eq!(
      dump_scopes_of("main :: () { print(\"hi\"); }\n").0,
      EXIT_SUCCESS
    );
  }

  #[test]
  fn dump_scopes_carries_its_flag() {
    let Some(Command::Dump {
      stage: DumpStage::Scopes { file, file_only },
    }) = parse(&["oj", "dump", "scopes", "first.jai", "--file-only"]).command
    else {
      panic!("expected a scope dump");
    };
    assert_eq!(file, PathBuf::from("first.jai"));
    assert!(file_only);
  }

  #[test]
  fn build_forwards_jai_style_options_verbatim() {
    let Some(Command::Build(args)) = parse(&["oj", "build", "first.jai", "-", "-android"]).command
    else {
      panic!("expected a build command");
    };

    assert_eq!(args.file, PathBuf::from("first.jai"));
    assert_eq!(args.options, ["-", "-android"]);
  }

  #[test]
  fn run_separates_metaprogram_options_from_program_arguments() {
    let Some(Command::Run(args)) =
      parse(&["oj", "run", "first.jai", "-quiet", "--", "-x", "42"]).command
    else {
      panic!("expected a run command");
    };

    assert_eq!(args.file, PathBuf::from("first.jai"));
    assert_eq!(args.options, ["-quiet"]);
    assert_eq!(args.program_args, ["-x", "42"]);
  }

  #[test]
  fn a_program_argument_list_may_itself_contain_a_double_dash() {
    let Some(Command::Run(args)) = parse(&["oj", "run", "first.jai", "--", "--", "-x"]).command
    else {
      panic!("expected a run command");
    };

    assert_eq!(args.program_args, ["--", "-x"]);
  }

  #[test]
  fn only_run_accepts_program_arguments() {
    assert_eq!(
      exit_code(["oj", "build", "first.jai", "--", "-x"]),
      EXIT_USAGE
    );
  }

  #[test]
  fn dump_stages_carry_their_flags() {
    let Some(Command::Dump {
      stage: DumpStage::Ast { file, tree },
    }) = parse(&["oj", "dump", "ast", "first.jai", "--tree"]).command
    else {
      panic!("expected an ast dump");
    };
    assert_eq!(file, PathBuf::from("first.jai"));
    assert!(tree);

    let Some(Command::Dump {
      stage: DumpStage::Asm { llvm, proc, .. },
    }) = parse(&["oj", "dump", "asm", "first.jai", "--llvm"]).command
    else {
      panic!("expected an asm dump");
    };
    assert!(llvm);
    assert_eq!(proc, None);
  }
}

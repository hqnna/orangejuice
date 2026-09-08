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
    Some(Command::Build(_)) => not_implemented("oj build", "M5"),
    Some(Command::Run(_)) => not_implemented("oj run", "M5"),
    Some(Command::Dump { stage }) => match stage {
      DumpStage::Tokens { file } => dump_tokens(file),
      DumpStage::Ast { file, tree } => dump_ast(file, *tree),
      DumpStage::Ir { .. } => not_implemented("oj dump ir", "M5"),
      DumpStage::Asm { .. } => not_implemented("oj dump asm", "M5"),
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

fn not_implemented(command: &str, milestone: &str) -> u8 {
  eprintln!("error: `{command}` is not implemented yet (planned for milestone {milestone})");
  EXIT_FAILURE
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
  fn unimplemented_stages_fail_rather_than_pretending_to_work() {
    assert_eq!(exit_code(["oj", "build", "first.jai"]), EXIT_FAILURE);
    assert_eq!(exit_code(["oj", "dump", "ast", "first.jai"]), EXIT_FAILURE);
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

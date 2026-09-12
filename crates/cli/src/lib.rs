//! The `oj` command line, which is `jai`'s.
//!
//! ```text
//! oj <files...> [metaprogram options] [- user args] [-- compiler options]
//! ```
//!
//! A non-dash argument is a source file and the first one names the output
//! (**C§2.1**). The single-dash options are the reference's, parsed by
//! `oj-driver` and handed to the metaprogram verbatim, so `jai first.jai -
//! -android` is `oj first.jai - -android`. A lone `-` ends option processing
//! and gives the rest to the program.
//!
//! Everything after the last `--` or `---` belongs to the *compiler* rather
//! than to the metaprogram — that is the reference's own rule, and its own
//! diagnostic says so: "Any command-line option after the last -- is reserved
//! for the compiler." Those options are bare words, and orangejuice's own go
//! there: `run`, `dump`, `no_metaprogram`. They cannot be single-dash names of
//! ours, both because `docs/spec.md` §5.1 reserves that namespace for the
//! reference and because `-run` is already taken — it means
//! `add_build_string("#run expr;")`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oj_diag::SourceMap;
use oj_lexer::Interner;

/// What `oj version` prints. The Jai release named here is
/// `oj_driver::JAI_VERSION` rather than a second copy of it, so the compiler
/// cannot claim one version to a person and another to a metaprogram.
pub fn version_line() -> String {
  format!(
    "orangejuice {} (jai {} compatible, LLVM 19)",
    env!("CARGO_PKG_VERSION"),
    oj_driver::JAI_VERSION
  )
}

/// Writes a listing to stdout. A reader that has gone away — `oj … -- dump ir
/// | head` — is not a failure of the compiler's, so the write is dropped
/// rather than reported: Rust's own `print!` panics on `EPIPE`, which is not
/// what a command-line tool should do.
fn out(text: &str) {
  use std::io::Write;
  let mut stdout = std::io::stdout().lock();
  if let Err(error) = stdout.write_all(text.as_bytes())
    && error.kind() != std::io::ErrorKind::BrokenPipe
  {
    eprintln!("error: could not write to stdout: {error}");
  }
}

/// The same, with a newline after it.
fn out_line(text: &str) {
  out(text);
  out("\n");
}

/// Changes to the first file's directory, the way the reference does before it
/// compiles (**C§2.1**), and gives back the file list and the metaprogram's
/// command line with every path made absolute — a relative one would name a
/// different file, or none, from the directory just moved to. `-no_cwd` leaves
/// the directory alone, and so does a command line with no files at all:
/// `-add` and `-run` name no directory to move to.
fn enter_first_files_directory(
  invocation: &Invocation,
  options: &oj_driver::BuildOptions,
) -> Result<(Vec<PathBuf>, Vec<String>), u8> {
  let absolute: Vec<PathBuf> = invocation
    .files
    .iter()
    .map(|file| std::path::absolute(file).unwrap_or_else(|_| file.clone()))
    .collect();
  // The metaprogram is handed the command line rather than the file list, so
  // the words in it that *are* files have to move with them.
  let arguments: Vec<String> = invocation
    .metaprogram
    .iter()
    .map(|argument| {
      match invocation
        .files
        .iter()
        .position(|file| file.as_os_str() == argument.as_str())
      {
        Some(index) => absolute[index].display().to_string(),
        None => argument.clone(),
      }
    })
    .collect();

  if !options.set_working_directory {
    return Ok((absolute, arguments));
  }
  let Some(directory) = absolute.first().and_then(|file| file.parent()) else {
    return Ok((absolute, arguments));
  };
  // A directory that is already the working one needs no move, which is what
  // keeps the common `oj main.jai` from touching the process at all.
  if std::env::current_dir().is_ok_and(|current| current == directory) {
    return Ok((absolute, arguments));
  }
  if let Err(error) = std::env::set_current_dir(directory) {
    eprintln!(
      "error: could not change to {}: {error}",
      directory.display()
    );
    return Err(EXIT_FAILURE);
  }
  Ok((absolute, arguments))
}

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_FAILURE: u8 = 1;
pub const EXIT_USAGE: u8 = 2;

/// What the reference says when it was given nothing to do (**C§2.1**).
const NOTHING_TO_COMPILE: &str = "You need to provide an argument telling the compiler what to compile! Sorry. Pass -help for help.";

/// The compiler's own options: the bare words after the last `--`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Compiler {
  /// Run the program once it is built, passing it whatever followed `run`.
  pub run: bool,
  pub program_args: Vec<OsString>,
  /// Stop at a stage and print it instead of building.
  pub dump: Option<Dump>,
  /// Drive the pipeline directly rather than through the distribution's
  /// `Default_Metaprogram`, which is what a checkout with no distribution
  /// behind it has to do.
  pub no_metaprogram: bool,
  /// `import_dir name`, the reference's own.
  pub import_dirs: Vec<PathBuf>,
  /// `meta metaprogram_name`, the reference's own.
  pub meta: Option<String>,
  pub help: bool,
  pub version: bool,
}

/// `dump <stage>` and the words that shape it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Dump {
  pub stage: DumpStage,
  /// `tree`: print the tree form of the AST rather than the source form.
  pub tree: bool,
  /// `file_only`: resolve the file alone, without its `#load`s and `#import`s.
  pub file_only: bool,
  /// `proc NAME`: restrict an IR or assembly listing to one procedure.
  pub proc: Option<String>,
  /// `llvm`: print LLVM IR rather than target assembly.
  pub llvm: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DumpStage {
  Tokens,
  Ast,
  Scopes,
  Types,
  #[default]
  Ir,
  Asm,
}

/// One whole command line, taken apart.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Invocation {
  /// The source files, first one first.
  pub files: Vec<PathBuf>,
  /// What the metaprogram parses, verbatim — every single-dash option, the
  /// lone `-`, and whatever follows it.
  pub metaprogram: Vec<String>,
  pub compiler: Compiler,
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
  let argv: Vec<OsString> = args.into_iter().map(Into::into).skip(1).collect();
  match parse(&argv) {
    Ok(invocation) => execute(&invocation),
    Err(error) => {
      eprintln!("{}", error.message);
      error.code
    }
  }
}

/// A command line that did not parse, and what the compiler exits with because
/// of it. A metaprogram option the reference would have rejected exits 1, the
/// way its own `log_error`-then-`exit(1)` does (**C§2.1**); a usage error of
/// ours — a developer option nobody knows, a stage that is not a stage — exits
/// 2, which is the CLI's own.
#[derive(Debug, PartialEq, Eq)]
pub struct UsageError {
  pub message: String,
  pub code: u8,
}

impl UsageError {
  fn usage(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
      code: EXIT_USAGE,
    }
  }

  fn metaprogram(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
      code: EXIT_FAILURE,
    }
  }
}

/// Splits a command line into the metaprogram's half and the compiler's.
///
/// The compiler's half is whatever follows the *last* `--` or `---`, which is
/// what the reference does — an earlier one is left where it is, since the
/// metaprogram ignores `--` and a user argument may be one.
pub fn parse(argv: &[OsString]) -> Result<Invocation, UsageError> {
  let split = argv
    .iter()
    .rposition(|argument| argument == "--" || argument == "---");

  let (left, right) = match split {
    Some(index) => (&argv[..index], &argv[index + 1..]),
    None => (argv, &argv[..0]),
  };

  let compiler = parse_compiler_options(right)?;

  let metaprogram: Vec<String> = left
    .iter()
    .map(|argument| argument.to_string_lossy().into_owned())
    .collect();

  // The files are picked out by the same parser that knows every option's
  // arity, so that `-exe out.jai` does not look like a file (**C§2.1**).
  let files = match oj_driver::parse(&metaprogram) {
    Ok(parsed) => parsed.files,
    Err(error) => return Err(UsageError::metaprogram(error.message)),
  };

  Ok(Invocation {
    files,
    metaprogram,
    compiler,
  })
}

fn parse_compiler_options(words: &[OsString]) -> Result<Compiler, UsageError> {
  let mut compiler = Compiler::default();
  let mut index = 0;
  while index < words.len() {
    let word = words[index].to_string_lossy().into_owned();
    index += 1;

    let mut take = |option: &str| -> Result<String, UsageError> {
      let value = words
        .get(index)
        .map(|value| value.to_string_lossy().into_owned());
      index += 1;
      value.ok_or_else(|| UsageError::usage(format!("Command line: Missing argument to {option}.")))
    };

    match word.as_str() {
      // `run` is terminal: what follows it is the program's own arguments,
      // which may be anything at all.
      "run" => {
        compiler.run = true;
        compiler.program_args = words[index..].to_vec();
        index = words.len();
      }
      "dump" => {
        let stage = take("dump")?;
        let stage = match stage.as_str() {
          "tokens" => DumpStage::Tokens,
          "ast" => DumpStage::Ast,
          "scopes" => DumpStage::Scopes,
          "types" => DumpStage::Types,
          "ir" => DumpStage::Ir,
          "asm" => DumpStage::Asm,
          other => {
            return Err(UsageError::usage(format!(
              "Command line: '{other}' is not a stage. The stages are tokens, ast, scopes, types, ir, asm."
            )));
          }
        };
        compiler.dump.get_or_insert_with(Dump::default).stage = stage;
      }
      "tree" => compiler.dump.get_or_insert_with(Dump::default).tree = true,
      "file_only" => compiler.dump.get_or_insert_with(Dump::default).file_only = true,
      "llvm" => compiler.dump.get_or_insert_with(Dump::default).llvm = true,
      "proc" => {
        let name = take("proc")?;
        compiler.dump.get_or_insert_with(Dump::default).proc = Some(name);
      }
      "no_metaprogram" => compiler.no_metaprogram = true,
      "import_dir" => {
        let directory = take("import_dir")?;
        compiler.import_dirs.push(PathBuf::from(directory));
      }
      "meta" => compiler.meta = Some(take("meta")?),
      "help" => compiler.help = true,
      "version" => compiler.version = true,
      other => {
        return Err(UsageError::usage(format!(
          "Unknown developer option '{other}'. Use '-- help' for help. Any command-line option after the last -- is reserved for the compiler."
        )));
      }
    }
  }
  Ok(compiler)
}

/// What `-- help` prints. The reference lists its own developer options here;
/// ours are listed beside them, since this is the namespace they live in.
const COMPILER_HELP: &str = "\
Developer options: import_dir name, meta metaprogram_name, help, version.
orangejuice options: run [program arguments...], dump stage, no_metaprogram.
  dump stage        one of tokens, ast, scopes, types, ir, asm
  tree              with `dump ast`, print the tree form
  file_only         with `dump scopes` or `dump types`, do not follow #load and #import
  proc NAME         with `dump ir` or `dump asm`, one procedure only
  llvm              with `dump asm`, print LLVM IR instead of assembly";

fn execute(invocation: &Invocation) -> u8 {
  let compiler = &invocation.compiler;
  if compiler.help {
    out_line(COMPILER_HELP);
    return EXIT_SUCCESS;
  }
  if compiler.version {
    out_line(&version_line());
    return EXIT_SUCCESS;
  }

  if let Some(wanted) = &compiler.dump {
    let Some(file) = invocation.files.first() else {
      eprintln!("{NOTHING_TO_COMPILE}");
      return EXIT_USAGE;
    };
    return match wanted.stage {
      DumpStage::Tokens => dump_tokens(file),
      DumpStage::Ast => dump_ast(file, wanted.tree),
      DumpStage::Scopes => dump_scopes(file, wanted.file_only),
      DumpStage::Types => dump_types(file, wanted.file_only),
      DumpStage::Ir => dump(file, oj_driver::Stage::Ir, wanted.proc.as_deref()),
      DumpStage::Asm => {
        let stage = match wanted.llvm {
          true => oj_driver::Stage::LlvmIr,
          false => oj_driver::Stage::Assembly,
        };
        dump(file, stage, wanted.proc.as_deref())
      }
    };
  }

  build(invocation)
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

  out(&oj_lexer::dump_tokens(&lexed.tokens, &file, &interner));
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
    out(&oj_syntax::print_tree(&parsed.ast, parsed.root, &interner));
  } else {
    out(&oj_syntax::print_source(
      &parsed.ast,
      parsed.root,
      &interner,
    ));
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
      distribution: oj_driver::distribution(),
      ..oj_scope::Options::default()
    }
  };

  let program = oj_scope::Program::build(&sources, &interner, path, options);
  out(&oj_scope::print_scopes(&program));
  out_line(&oj_scope::summary(program.tree(), program.unit_count()));

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

  if program.unit_count() == 0 {
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
      distribution: oj_driver::distribution(),
      ..oj_scope::Options::default()
    }
  };

  let program = oj_scope::Program::build(&sources, &interner, path, options);
  let mut checker = oj_sema::Checker::new(&program);
  checker.check();

  out(&oj_sema::print_types(&checker));
  out_line(&oj_sema::summary(&checker));

  for diagnostic in program.diagnostics().iter().chain(checker.diagnostics()) {
    let file = sources.file(diagnostic.source);
    eprint!("{}", oj_diag::render(diagnostic, &file));
  }

  if program.has_errors() || checker.has_errors() || program.unit_count() == 0 {
    EXIT_FAILURE
  } else {
    EXIT_SUCCESS
  }
}

/// `oj dump ir` and `oj dump asm`: the whole pipeline, stopped one stage early
/// and printed.
fn dump(path: &Path, stage: oj_driver::Stage, only: Option<&str>) -> u8 {
  let options = oj_driver::BuildOptions::new();
  let report = oj_driver::run(path, &options, stage, only);
  out(&report.output);
  for diagnostic in &report.diagnostics {
    eprint!("{diagnostic}");
  }
  if report.failed {
    EXIT_FAILURE
  } else {
    EXIT_SUCCESS
  }
}

/// Compiles, and runs what came out when `-- run` asked for it.
fn build(invocation: &Invocation) -> u8 {
  let compiler = &invocation.compiler;
  let parsed = match oj_driver::parse(&invocation.metaprogram) {
    Ok(parsed) => parsed,
    Err(error) => {
      eprintln!("{}", error.message);
      return EXIT_FAILURE;
    }
  };
  // `-version` prints and stops, whether or not there was anything to build.
  if parsed.options.print_version {
    out_line(&version_line());
    return EXIT_SUCCESS;
  }
  for warning in &parsed.warnings {
    eprintln!("{warning}");
  }
  // `-help` is answered by the metaprogram, which owns the text (**C§2.1**),
  // so it has to reach one — and asking for help is not asking to compile
  // nothing. Without this the compiler tells you to pass the option you just
  // passed. `-add` and `-run` are something to compile in their own right
  // (**C§2.1**).
  if invocation.files.is_empty()
    && parsed.options.build_strings.is_empty()
    && !parsed.options.print_help
  {
    eprintln!("{NOTHING_TO_COMPILE}");
    return EXIT_USAGE;
  }

  let mut options = parsed.options;
  // `-- import_dir` is the compiler's own, and goes in front of whatever the
  // metaprogram's `-import_dir` asked for (**C§2.1**).
  let mut import_dirs = compiler.import_dirs.clone();
  import_dirs.extend(options.import_dirs.iter().cloned());
  options.import_dirs = import_dirs;

  // The compiler compiles from the first file's directory unless `-no_cwd`
  // (**C§2.1**), so a `#run` doing relative file I/O reads what is next to the
  // program rather than what is next to whoever invoked the compiler. The
  // reference has `Default_Metaprogram` call `set_working_directory`;
  // orangejuice does it here, because `oj-driver` is a library and a working
  // directory belongs to a whole process rather than to one compilation.
  let (files, arguments) = match enter_first_files_directory(invocation, &options) {
    Ok(moved) => moved,
    Err(code) => return code,
  };

  // The reference compiles `Default_Metaprogram` and lets *it* create the
  // workspace the program is compiled in (**C§2.1**); `-- meta` names a
  // different one, and a checkout with no distribution behind it has none, so
  // the pipeline drives itself instead.
  let metaprogram = match compiler.no_metaprogram {
    true => None,
    false => match &compiler.meta {
      Some(name) => match oj_driver::named_metaprogram(name) {
        Some(path) => Some(path),
        None => {
          eprintln!("Command line: could not find a metaprogram module named '{name}'.");
          return EXIT_FAILURE;
        }
      },
      None => oj_driver::default_metaprogram(),
    },
  };

  let input = oj_driver::Input {
    files: files.clone(),
    strings: oj_driver::command_line_strings(&files, &options),
  };
  let report = match metaprogram {
    Some(metaprogram) => oj_driver::run_through_metaprogram(
      &metaprogram,
      &files,
      &arguments,
      &options,
      oj_driver::Stage::Executable,
    ),
    None => oj_driver::run_input(&input, &options, oj_driver::Stage::Executable, None),
  };
  for diagnostic in &report.diagnostics {
    eprint!("{diagnostic}");
  }
  if report.failed {
    return EXIT_FAILURE;
  }
  let Some(executable) = report.executable else {
    return EXIT_SUCCESS;
  };
  if options.verbose
    && let Some(line) = &report.link_line
  {
    eprintln!("{line}");
  }
  if !compiler.run {
    if !options.quiet {
      out_line(&executable.display().to_string());
    }
    return EXIT_SUCCESS;
  }
  // A bare name is looked up on `PATH` rather than in the working directory,
  // and building `x.jai` next to it produces exactly that.
  let program = std::path::absolute(&executable).unwrap_or_else(|_| executable.clone());
  match std::process::Command::new(&program)
    .args(&compiler.program_args)
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

  fn parse_argv(argv: &[&str]) -> Invocation {
    let words: Vec<OsString> = argv.iter().map(OsString::from).collect();
    parse(&words).expect("the command line should parse")
  }

  fn code(argv: &[&str]) -> u8 {
    let mut full = vec!["oj"];
    full.extend_from_slice(argv);
    exit_code(full)
  }

  #[test]
  fn version_line_names_the_jai_release_and_the_backend() {
    let line = version_line();
    assert!(line.contains(oj_driver::JAI_VERSION), "{line}");
    assert!(line.contains("LLVM 19"), "{line}");
  }

  #[test]
  fn a_file_and_the_metaprogram_options_after_it_are_the_metaprograms() {
    // `jai first.jai - -android` is `oj first.jai - -android`, verbatim.
    let invocation = parse_argv(&["first.jai", "-", "-android"]);
    assert_eq!(invocation.files, [PathBuf::from("first.jai")]);
    assert_eq!(invocation.metaprogram, ["first.jai", "-", "-android"]);
    assert_eq!(invocation.compiler, Compiler::default());
  }

  #[test]
  fn every_non_dash_argument_before_a_lone_dash_is_a_file() {
    let invocation = parse_argv(&["a.jai", "-quiet", "b.jai", "-", "c.jai"]);
    // `c.jai` is the program's argument, not a third file.
    assert_eq!(
      invocation.files,
      [PathBuf::from("a.jai"), PathBuf::from("b.jai")]
    );
  }

  #[test]
  fn an_options_value_is_not_mistaken_for_a_file() {
    let invocation = parse_argv(&["-exe", "out.jai", "real.jai"]);
    assert_eq!(invocation.files, [PathBuf::from("real.jai")]);
  }

  #[test]
  fn the_compilers_own_options_follow_the_last_double_dash() {
    let invocation = parse_argv(&["a.jai", "-quiet", "--", "no_metaprogram", "dump", "ir"]);
    assert_eq!(invocation.files, [PathBuf::from("a.jai")]);
    assert_eq!(invocation.metaprogram, ["a.jai", "-quiet"]);
    assert!(invocation.compiler.no_metaprogram);
    assert_eq!(
      invocation.compiler.dump.as_ref().map(|dump| dump.stage),
      Some(DumpStage::Ir)
    );
  }

  #[test]
  fn three_dashes_are_the_same_delimiter() {
    let invocation = parse_argv(&["a.jai", "---", "import_dir", "modules"]);
    assert_eq!(invocation.compiler.import_dirs, [PathBuf::from("modules")]);
  }

  #[test]
  fn the_last_delimiter_is_the_one_that_splits() {
    // An earlier `--` is a user argument; the metaprogram ignores it.
    let invocation = parse_argv(&["a.jai", "-", "--", "-x", "--", "run"]);
    assert_eq!(invocation.metaprogram, ["a.jai", "-", "--", "-x"]);
    assert!(invocation.compiler.run);
  }

  #[test]
  fn run_takes_everything_after_it_as_the_programs_arguments() {
    let invocation = parse_argv(&["a.jai", "--", "run", "--port", "8080", "-x"]);
    assert!(invocation.compiler.run);
    assert_eq!(invocation.compiler.program_args, ["--port", "8080", "-x"]);
  }

  #[test]
  fn a_dump_carries_the_words_that_shape_it() {
    let invocation = parse_argv(&["a.jai", "--", "dump", "asm", "llvm", "proc", "main"]);
    let dump = invocation.compiler.dump.expect("a dump");
    assert_eq!(dump.stage, DumpStage::Asm);
    assert!(dump.llvm);
    assert_eq!(dump.proc.as_deref(), Some("main"));
  }

  #[test]
  fn an_unknown_compiler_option_says_what_the_reference_says() {
    let words: Vec<OsString> = ["a.jai", "--", "frobnicate"]
      .iter()
      .map(OsString::from)
      .collect();
    let error = parse(&words).expect_err("an unknown developer option");
    assert!(
      error
        .message
        .contains("Unknown developer option 'frobnicate'"),
      "{error:?}"
    );
    assert!(
      error.message.contains("reserved for the compiler"),
      "{error:?}"
    );
    assert_eq!(error.code, EXIT_USAGE);
  }

  #[test]
  fn a_compiler_option_missing_its_value_says_so() {
    let words: Vec<OsString> = ["a.jai", "--", "import_dir"]
      .iter()
      .map(OsString::from)
      .collect();
    let error = parse(&words).expect_err("a missing argument");
    assert_eq!(
      error.message,
      "Command line: Missing argument to import_dir."
    );
    assert_eq!(error.code, EXIT_USAGE);
  }

  #[test]
  fn nothing_to_compile_is_the_references_own_complaint() {
    assert_eq!(code(&[]), EXIT_USAGE);
  }

  #[test]
  fn a_metaprogram_option_the_reference_rejects_exits_one() {
    // The reference logs the error and exits 1 (**C§2.1**); a usage error of
    // ours exits 2.
    let words: Vec<OsString> = ["a.jai", "-frobnicate"]
      .iter()
      .map(OsString::from)
      .collect();
    let error = parse(&words).expect_err("an unknown argument");
    assert_eq!(error.message, "Unknown argument '-frobnicate'.\nExiting.");
    assert_eq!(error.code, EXIT_FAILURE);
  }

  #[test]
  fn the_compilers_help_and_version_are_its_own_options() {
    assert_eq!(code(&["--", "version"]), EXIT_SUCCESS);
    assert_eq!(code(&["--", "help"]), EXIT_SUCCESS);
  }

  /// `-help` is the metaprogram's, and it is asked for *instead of* a file to
  /// compile (**C§2.1**) — so the usual complaint about having nothing to
  /// build must not swallow it. It used to, which meant the compiler answered
  /// `-help` by telling you to pass `-help`.
  #[test]
  fn asking_for_help_is_not_asking_to_compile_nothing() {
    for spelling in ["-help", "-?"] {
      let parsed = oj_driver::parse(&[String::from(spelling)]).expect("a known option");
      assert!(parsed.options.print_help, "{spelling} asks for help");
      assert!(parsed.files.is_empty(), "{spelling} names no file");
    }

    // Everything else with no file to compile still says so.
    let parsed = oj_driver::parse(&[String::from("-release")]).expect("a known option");
    assert!(!parsed.options.print_help);
    assert!(parsed.files.is_empty());
  }

  fn dump_scopes_of(source: &str) -> (u8, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("input.jai");
    std::fs::write(&path, source).expect("the input should be writable");
    (
      code(&[path.to_str().unwrap(), "--", "dump", "scopes", "file_only"]),
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
      code(&["no/such/file.jai", "--", "dump", "scopes"]),
      EXIT_FAILURE
    );
  }

  fn dump_types_of(source: &str) -> (u8, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("input.jai");
    std::fs::write(&path, source).expect("the input should be writable");
    (
      code(&[path.to_str().unwrap(), "--", "dump", "types", "file_only"]),
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
      code(&["no/such/file.jai", "--", "dump", "types"]),
      EXIT_FAILURE
    );
  }

  #[test]
  fn the_first_file_s_directory_is_where_the_compiler_works_from() {
    // **C§2.1**: the compiler moves to the first file's directory before it
    // compiles, unless `-no_cwd`, and the file list comes back absolute so it
    // still names the same files from there. The move itself is not exercised
    // here — a working directory belongs to the whole test process — only the
    // paths it hands on.
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("main.jai");
    std::fs::write(&path, "main :: () {}\n").expect("the fixture should be writable");

    let invocation = Invocation {
      files: vec![path.clone()],
      metaprogram: vec![path.display().to_string(), String::from("-quiet")],
      compiler: Compiler::default(),
    };
    let mut options = oj_driver::BuildOptions::new();
    options.set_working_directory = false;
    let (files, arguments) =
      enter_first_files_directory(&invocation, &options).expect("nothing to move to");
    assert_eq!(files, [path.clone()].as_slice());
    assert_eq!(arguments[0], path.display().to_string());
    assert_eq!(arguments[1], "-quiet");
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
}

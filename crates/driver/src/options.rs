//! The jai-style command line of `oj build` and `oj run`.
//!
//! The single-dash options are the reference compiler's, not ours: their
//! names, arity, order and error wording come from **C§2.1** and from
//! `modules/Default_Metaprogram.jai`, which `docs/spec.md` §5.1
//! tables. Nothing here may be invented.

use std::path::PathBuf;

/// `Build_Options.context_size_max`'s own default (**C§4**), which is what
/// `size_of(#Context)` reports whatever a program imports (**L§10.1**).
pub const DEFAULT_CONTEXT_SIZE: u64 = 4096;

/// `CONTEXT_SIZE_MAX`, the largest `-context_size` the reference takes
/// (**C§2.1**).
const CONTEXT_SIZE_MAX: i64 = 0x4_0000;

/// `size_of(Context_Base)`, the smallest one. The struct is Runtime_Support's
/// and its size is pinned by `crates/sema/tests/corpus.rs`, which is what lets
/// the command line be checked before anything is typed.
const CONTEXT_BASE_SIZE: i64 = 96;

/// How much the back end optimizes (`Build_Options.optimization_level`,
/// **C§4**).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Optimization {
  #[default]
  Debug,
  VeryDebug,
  Optimized,
}

impl Optimization {
  /// `Llvm_Options.machine_code_optimization_setting` (**C§4**): `NONE` for a
  /// debug build, `DEFAULT` for an optimized one.
  pub fn level(self) -> u8 {
    match self {
      Self::VeryDebug | Self::Debug => 0,
      Self::Optimized => 2,
    }
  }

  /// `Llvm_Options.bitcode_optimization_setting`, which `set_optimization`
  /// pairs with the machine level (**C§4**).
  pub fn bitcode(self) -> oj_codegen::Bitcode {
    match self {
      Self::VeryDebug | Self::Debug => oj_codegen::Bitcode::O0,
      Self::Optimized => oj_codegen::Bitcode::O2,
    }
  }

  /// What `set_optimization` turns on beside the pipeline (**C§4**): a debug
  /// build unrolls and vectorizes nothing, and only `VERY_OPTIMIZED`
  /// vectorizes at all.
  pub fn passes(self) -> oj_codegen::PassOptions {
    let debug = matches!(self, Self::Debug | Self::VeryDebug);
    oj_codegen::PassOptions {
      loop_unrolling: !debug,
      loop_vectorization: false,
      slp_vectorization: false,
      merge_functions: false,
    }
  }
}

/// `Build_Options.runtime_support_definitions` (**C§4**), which decides what a
/// compilation defines out of Runtime_Support. orangejuice generates its own
/// `main` rather than using `__system_entry_point` (`docs/spec.md` §10), so
/// what is left to decide is whether `__jai_runtime_init` and
/// `__jai_runtime_fini` are exported — which is what a Jai library linked into
/// a non-Jai program needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RuntimeSupport {
  /// `ENTRY_POINT_AND_INIT` for an executable, `ONLY_INIT` for everything else.
  #[default]
  Auto,
  EntryPointAndInit,
  OnlyInit,
  Omit,
}

impl RuntimeSupport {
  /// The values the distribution's enum gives its members (**C§4**).
  pub fn from_value(value: u8) -> Self {
    match value {
      1 => Self::EntryPointAndInit,
      2 => Self::OnlyInit,
      3 => Self::Omit,
      _ => Self::Auto,
    }
  }

  /// Whether a compilation that produces no executable still defines the
  /// runtime's init and fini, and exports them.
  pub fn defines_init(self) -> bool {
    self != Self::Omit
  }
}

/// The subset of `Build_Options` the command line sets, plus the switches the
/// driver itself reads.
#[derive(Clone, Debug, Default)]
pub struct BuildOptions {
  /// `Build_Options.os_target`/`cpu_target` (**C§4**): what this compilation
  /// emits and links for. The host unless a metaprogram named another
  /// (`docs/spec.md` §2.1).
  pub target: oj_types::Target,
  pub optimization: Optimization,
  pub stack_trace: bool,
  /// `Build_Options.emit_debug_info` (**C§4**): DWARF is what `.DEFAULT` means
  /// on Linux, and `set_optimization` turns it off only when the metaprogram
  /// asks it not to preserve debug info.
  pub debug_info: bool,
  pub quiet: bool,
  pub verbose: bool,
  pub use_color: bool,
  pub visual_studio_format: bool,
  pub output_llvm_ir: bool,
  pub enable_split_modules: bool,
  pub dead_code_elimination: bool,
  pub output_executable_name: Option<String>,
  pub output_path: Option<PathBuf>,
  /// `Build_Options.output_type` (**C§4**): what the program is linked into.
  pub output_type: oj_link::OutputType,
  /// `Build_Options.append_executable_filename_extension`.
  pub append_extension: bool,
  /// `Build_Options.runtime_support_definitions` (**C§4**): what a compilation
  /// takes from Runtime_Support.
  pub runtime_support: RuntimeSupport,
  /// `Build_Options.use_custom_link_command` (**C§4**): the metaprogram links,
  /// so the compiler stops at the object it made.
  pub custom_link_command: bool,
  pub import_dirs: Vec<PathBuf>,
  pub set_working_directory: bool,
  pub debug_for_expansions: bool,
  pub backtrace_on_crash: bool,
  /// Everything after a lone `-`, which the program and its `#run`s see
  /// (**L§14.4**).
  pub compile_time_command_line: Vec<String>,
  /// `remap_import` calls a metaprogram made for this workspace (**C§3.3**).
  pub import_remaps: Vec<oj_scope::ImportRemap>,
  /// `provide_import` answers a metaprogram gave for imports that failed
  /// (**C§3.3**).
  pub provided_imports: Vec<oj_scope::ProvidedImport>,
  /// Strings a metaprogram added to a scope it named with a message
  /// (**C§3.3**).
  pub added_strings: Vec<oj_scope::AddedString>,
  /// Procedures `compiler_make_procedure_live` asked to be lowered whether or
  /// not anything calls them (**C§3.3**), by file and name.
  pub live_procedures: Vec<(String, String)>,
  /// Procedure bodies `compiler_modify_procedure` handed back, as the source
  /// they now read as (**C§3.3**).
  pub modified_bodies: Vec<oj_scope::ModifiedBody>,
  /// `-version`, which prints the version and stops when no file follows.
  pub print_version: bool,
  /// `-help` / `-?`, which prints the metaprogram's own help text.
  pub print_help: bool,
  /// `Build_Options.context_size_max` (**C§4**), which `-context_size` sets
  /// and `#Context` is padded out to (**L§10.1**).
  pub context_size_max: u64,
  /// What `-add` and `-run` asked to be compiled beside the files, each
  /// already wrapped in the text the reference wraps it in (**C§2.1**).
  pub build_strings: Vec<String>,
  /// `-debugger`: the reference's interactive bytecode debugger, which
  /// orangejuice answers with a stack trace (`docs/spec.md` §2).
  pub debugger: bool,
  /// The number this compilation's own workspace takes (**C§3.1**). The
  /// reference numbers them once for the whole compiler, so a workspace a
  /// metaprogram created carries its number into the compilation it drives.
  pub workspace_id: i64,
}

impl BuildOptions {
  pub fn new() -> Self {
    Self {
      target: oj_types::Target::HOST,
      optimization: Optimization::Debug,
      stack_trace: true,
      debug_info: true,
      workspace_id: 1,
      use_color: true,
      enable_split_modules: true,
      dead_code_elimination: true,
      set_working_directory: true,
      backtrace_on_crash: true,
      append_extension: true,
      context_size_max: DEFAULT_CONTEXT_SIZE,
      ..Self::default()
    }
  }
}

/// A command line the metaprogram would reject. The wording is the
/// reference's (**C§2.1**).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionError {
  pub message: String,
}

/// What one command line came to.
#[derive(Clone, Debug)]
pub struct ParsedOptions {
  pub options: BuildOptions,
  /// The source files, in the order they were written. An argument that does
  /// not start with `-` is a file (**C§2.1**), and the first one names the
  /// output and the directory it lands in.
  pub files: Vec<std::path::PathBuf>,
  /// What the command line asked for that orangejuice answers differently,
  /// in the words the CLI prints before it builds.
  pub warnings: Vec<String>,
}

/// Parses the jai-style option list that follows the source file.
pub fn parse(arguments: &[String]) -> Result<ParsedOptions, OptionError> {
  let mut options = BuildOptions::new();
  let mut files = Vec::new();
  let mut warnings = Vec::new();
  let mut index = 0;

  while index < arguments.len() {
    let argument = arguments[index].clone();
    index += 1;

    // A lone `-` ends option processing; the rest is the program's
    // (`docs/spec.md` §5.1).
    if argument == "-" {
      options.compile_time_command_line = arguments[index..].to_vec();
      break;
    }
    // Anything that is not an option is a file to compile (**C§2.1**).
    if !argument.starts_with('-') {
      files.push(std::path::PathBuf::from(argument));
      continue;
    }

    let mut take = |option: &str| -> Result<String, OptionError> {
      let value = arguments.get(index).cloned();
      index += 1;
      value.ok_or_else(|| OptionError {
        message: format!("Command line: Missing argument to {option}."),
      })
    };

    match argument.as_str() {
      "-release" => {
        options.optimization = Optimization::Optimized;
        options.stack_trace = false;
      }
      "-very_debug" => options.optimization = Optimization::VeryDebug,
      "-quiet" => options.quiet = true,
      "-verbose" => options.verbose = true,
      "-no_color" => options.use_color = false,
      "-msvc_format" => options.visual_studio_format = true,
      "-output_ir" => options.output_llvm_ir = true,
      "-no_split" => options.enable_split_modules = false,
      "-no_dce" => options.dead_code_elimination = false,
      "-no_cwd" => options.set_working_directory = false,
      "-debug_for" => options.debug_for_expansions = true,
      "-no_backtrace_on_crash" => options.backtrace_on_crash = false,
      "-version" => options.print_version = true,
      "-help" | "-?" => options.print_help = true,
      "-exe" => options.output_executable_name = Some(take("-exe")?),
      "-output_path" => options.output_path = Some(PathBuf::from(take("-output_path")?)),
      "-import_dir" => options
        .import_dirs
        .push(PathBuf::from(take("-import_dir")?)),
      // Accepted and recorded, but the backend they select is not a choice
      // orangejuice offers (`docs/spec.md` §2).
      "-llvm" => {}
      "-x64" => warnings.push(String::from(X64_IS_LLVM)),
      "-natvis" | "-no_inline" => {}
      // The plugin list belongs to `Default_Metaprogram`, which reads it out
      // of the command line it is handed (**C§2.1**); the driver only has to
      // take the name along with the switch.
      "-plug" | "-plugin" => {
        let _ = take(&argument)?;
      }
      // `-add CODE` and `-run EXPR` are `add_build_string` of the text the
      // reference wraps them in (**C§2.1**). `Default_Metaprogram` adds them
      // to the workspace it creates, and the driver adds them itself when no
      // metaprogram is in the way.
      "-add" => {
        let value = take("-add")?;
        options.build_strings.push(format!("{value};"));
      }
      "-run" => {
        let value = take("-run")?;
        options.build_strings.push(format!("#run {value};"));
      }
      "-context_size" => {
        let value = take("-context_size")?;
        options.context_size_max = context_size(&value)?;
      }
      // The `Check` plugin never runs — `Metaprogram_Plugins` fills a
      // `#placeholder` from inside a `#run` that orangejuice has already
      // compiled (`docs/spec.md` §10) — so a switch that turns it off or
      // configures it describes what happens anyway.
      "-no_check" | "-no_check_bindings" | "-check_bindings" => {}
      // `-ps5` is `-plug ps5_build` spelled shorter (**C§2.1**): the
      // metaprogram reads it out of the command line and loads the plugin,
      // and says so when the distribution behind it has none.
      "-ps5" => {}
      // The reference stops a failing `#run` in its interactive bytecode
      // debugger; orangejuice has no interpreter to stop, so what a
      // compile-time failure prints is a stack trace (`docs/spec.md` §2).
      "-debugger" => options.debugger = true,
      _ => return Err(unknown(&argument)),
    }
  }

  Ok(ParsedOptions {
    options,
    files,
    warnings,
  })
}

/// What `-x64` gets instead of the reference's native back end, which
/// orangejuice does not have (`docs/spec.md` §2).
const X64_IS_LLVM: &str =
  "Command line: -x64 names a back end orangejuice does not have; building with LLVM instead.";

/// `Build_Options.context_size_max` (**C§4**), with the three ways the
/// reference rejects an argument to it (**C§2.1**).
fn context_size(written: &str) -> Result<u64, OptionError> {
  let Ok(value) = written.parse::<i64>() else {
    return Err(OptionError {
      message: format!(
        "Command line: Unable to parse an integer argument to context size; got '{written}'."
      ),
    });
  };
  if value > CONTEXT_SIZE_MAX {
    return Err(OptionError {
      message: format!(
        "Command line: Invalid argument to -context_size. The context must be less than or equal to CONTEXT_SIZE_MAX, which is {CONTEXT_SIZE_MAX} (but the value provided was {value})."
      ),
    });
  }
  if value < CONTEXT_BASE_SIZE {
    return Err(OptionError {
      message: format!(
        "Command line: Invalid argument to -context_size. The context must be at least as large as size_of(Context_Base), which is {CONTEXT_BASE_SIZE}."
      ),
    });
  }
  Ok(value as u64)
}

fn unknown(argument: &str) -> OptionError {
  // Two lines, the way the reference writes it (**C§2.1**).
  OptionError {
    message: format!("Unknown argument '{argument}'.\nExiting."),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse_of(arguments: &[&str]) -> Result<ParsedOptions, OptionError> {
    parse(
      &arguments
        .iter()
        .map(|text| text.to_string())
        .collect::<Vec<_>>(),
    )
  }

  #[test]
  fn release_optimizes_and_drops_the_stack_trace() {
    let parsed = parse_of(&["-release"]).expect("the option list should parse");
    assert_eq!(parsed.options.optimization, Optimization::Optimized);
    assert!(!parsed.options.stack_trace);
    assert_eq!(parsed.options.optimization.level(), 2);
  }

  #[test]
  fn a_lone_dash_hands_the_rest_to_the_program() {
    let parsed = parse_of(&["-quiet", "-", "-android", "x"]).expect("it should parse");
    assert!(parsed.options.quiet);
    assert_eq!(parsed.options.compile_time_command_line, ["-android", "x"]);
  }

  #[test]
  fn options_with_arguments_take_the_next_word() {
    let parsed = parse_of(&["-exe", "game", "-output_path", "/tmp/out"]).expect("it should parse");
    assert_eq!(
      parsed.options.output_executable_name.as_deref(),
      Some("game")
    );
    assert_eq!(parsed.options.output_path, Some(PathBuf::from("/tmp/out")));
  }

  #[test]
  fn a_missing_argument_is_the_reference_wording() {
    let error = parse_of(&["-exe"]).expect_err("the option list should be rejected");
    assert_eq!(error.message, "Command line: Missing argument to -exe.");
  }

  #[test]
  fn an_unknown_option_is_the_reference_wording() {
    let error = parse_of(&["-frobnicate"]).expect_err("the option list should be rejected");
    assert_eq!(error.message, "Unknown argument '-frobnicate'.\nExiting.");
  }

  #[test]
  fn add_and_run_are_the_strings_the_reference_builds_from_them() {
    let parsed = parse_of(&["-add", "x := 1", "-run", "main()"]).expect("it should parse");
    assert_eq!(parsed.options.build_strings, ["x := 1;", "#run main();"]);
  }

  #[test]
  fn a_plugin_switch_takes_its_name_and_is_left_to_the_metaprogram() {
    let parsed = parse_of(&["-plug", "Check"]).expect("it should parse");
    assert!(parsed.warnings.is_empty());
    let parsed = parse_of(&["-plug"]).expect_err("the name is not optional");
    assert_eq!(parsed.message, "Command line: Missing argument to -plug.");
  }

  #[test]
  fn x64_names_a_back_end_there_is_not_and_says_so() {
    let parsed = parse_of(&["-x64"]).expect("it should parse");
    assert_eq!(parsed.warnings.len(), 1);
    assert!(parsed.warnings[0].contains("-x64"), "{:?}", parsed.warnings);
  }

  #[test]
  fn context_size_takes_the_number_and_the_reference_wording_for_a_bad_one() {
    let parsed = parse_of(&["-context_size", "8192"]).expect("it should parse");
    assert_eq!(parsed.options.context_size_max, 8192);
    assert_eq!(
      parse_of(&["-context_size", "wide"])
        .expect_err("a word is not a size")
        .message,
      "Command line: Unable to parse an integer argument to context size; got 'wide'."
    );
    assert!(
      parse_of(&["-context_size", "1048576"])
        .expect_err("that is above CONTEXT_SIZE_MAX")
        .message
        .contains("less than or equal to CONTEXT_SIZE_MAX, which is 262144")
    );
    assert!(
      parse_of(&["-context_size", "8"])
        .expect_err("that is below size_of(Context_Base)")
        .message
        .contains("at least as large as size_of(Context_Base), which is 96")
    );
  }

  #[test]
  fn the_debugger_switch_is_recorded_rather_than_refused() {
    let parsed = parse_of(&["-debugger"]).expect("it should parse");
    assert!(parsed.options.debugger);
    assert!(parsed.warnings.is_empty());
  }
}

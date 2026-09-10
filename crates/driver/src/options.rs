//! The jai-style command line of `oj build` and `oj run`.
//!
//! The single-dash options are the reference compiler's, not ours: their
//! names, arity, order and error wording come from **C§2.1** and from
//! `vendor/jai/modules/Default_Metaprogram.jai`, which `docs/spec.md` §5.1
//! tables. Nothing here may be invented.

use std::path::PathBuf;

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
  /// `-help` / `-?`, which prints the vendored metaprogram's help text.
  pub print_help: bool,
}

impl BuildOptions {
  pub fn new() -> Self {
    Self {
      optimization: Optimization::Debug,
      stack_trace: true,
      debug_info: true,
      use_color: true,
      enable_split_modules: true,
      dead_code_elimination: true,
      set_working_directory: true,
      backtrace_on_crash: true,
      append_extension: true,
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

/// An option whose effect belongs to a milestone the compiler has not reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deferred {
  pub option: String,
  pub milestone: &'static str,
}

/// What one command line came to.
#[derive(Clone, Debug)]
pub struct ParsedOptions {
  pub options: BuildOptions,
  /// Options that parsed but whose behaviour needs a later milestone.
  pub deferred: Vec<Deferred>,
}

/// Parses the jai-style option list that follows the source file.
pub fn parse(arguments: &[String]) -> Result<ParsedOptions, OptionError> {
  let mut options = BuildOptions::new();
  let mut deferred = Vec::new();
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
    if !argument.starts_with('-') {
      return Err(unknown(&argument));
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
      "-x64" => deferred.push(Deferred {
        option: argument,
        milestone: "M5",
      }),
      "-natvis" | "-no_inline" => {}
      // These need the metaprogram, the interpreter, or both.
      // The plugin list belongs to `Default_Metaprogram`, which reads it out
      // of the command line it is handed (**C§2.1**); the driver only has to
      // take the name along with the switch.
      "-plug" | "-plugin" => {
        let _ = take(&argument)?;
      }
      "-add" | "-run" => {
        let value = take(&argument)?;
        deferred.push(Deferred {
          option: format!("{argument} {value}"),
          milestone: "M6",
        });
      }
      "-context_size" => {
        let value = take("-context_size")?;
        deferred.push(Deferred {
          option: format!("-context_size {value}"),
          milestone: "M6",
        });
      }
      // The `Check` plugin never runs — `Metaprogram_Plugins` fills a
      // `#placeholder` from inside a `#run` that orangejuice has already
      // compiled (`docs/spec.md` §10) — so a switch that turns it off or
      // configures it describes what happens anyway.
      "-no_check" | "-no_check_bindings" | "-check_bindings" => {}
      "-ps5" => {
        deferred.push(Deferred {
          option: argument,
          milestone: "M8",
        });
      }
      "-debugger" => deferred.push(Deferred {
        option: argument,
        milestone: "M6",
      }),
      _ => return Err(unknown(&argument)),
    }
  }

  Ok(ParsedOptions { options, deferred })
}

fn unknown(argument: &str) -> OptionError {
  OptionError {
    message: format!("Unknown argument '{argument}'."),
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
    assert_eq!(error.message, "Unknown argument '-frobnicate'.");
  }

  #[test]
  fn options_that_need_a_later_milestone_are_recorded_rather_than_ignored() {
    let parsed = parse_of(&["-run", "main()"]).expect("it should parse");
    assert_eq!(parsed.deferred.len(), 1);
    assert_eq!(parsed.deferred[0].milestone, "M6");
  }

  #[test]
  fn a_plugin_switch_takes_its_name_and_is_left_to_the_metaprogram() {
    let parsed = parse_of(&["-plug", "Check"]).expect("it should parse");
    assert!(parsed.deferred.is_empty());
    let parsed = parse_of(&["-plug"]).expect_err("the name is not optional");
    assert_eq!(parsed.message, "Command line: Missing argument to -plug.");
  }
}

//! The pipeline: source to scope tree to types to IR to an object to an
//! executable.
//!
//! `oj-driver` owns the order the stages run in, the `Build_Options` a command
//! line sets, and where the artefacts land. Everything the reference does
//! through a metaprogram — workspaces, messages, plugins — is milestone M8;
//! what is here is the straight-line build the default metaprogram would ask
//! for.

mod options;

use std::path::{Path, PathBuf};

use oj_diag::SourceMap;
use oj_lexer::Interner;

pub use options::{BuildOptions, Deferred, Optimization, OptionError, ParsedOptions, parse};

/// How far the pipeline runs, and what it prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
  /// Stop after lowering and print the IR listing.
  Ir,
  /// Stop after code generation and print the LLVM IR.
  LlvmIr,
  /// Stop after code generation and print the target assembly.
  Assembly,
  /// Emit an object and link it.
  Executable,
}

/// What one run of the pipeline produced.
#[derive(Debug, Default)]
pub struct Report {
  /// The listing a dump stage asked for.
  pub output: String,
  /// Diagnostics, already rendered in the reference format (**C§12**).
  pub diagnostics: Vec<String>,
  pub failed: bool,
  /// The executable a successful `Stage::Executable` produced.
  pub executable: Option<PathBuf>,
  /// The link command, for `OUTPUT_LINK_LINE` and for `-verbose`.
  pub link_line: Option<String>,
}

impl Report {
  fn failure(message: impl Into<String>) -> Self {
    Self {
      diagnostics: vec![format!("error: {}\n", message.into())],
      failed: true,
      ..Self::default()
    }
  }
}

/// Runs the pipeline over one root file.
pub fn run(root: &Path, options: &BuildOptions, stage: Stage, only: Option<&str>) -> Report {
  let sources = SourceMap::new();
  let interner = Interner::new();
  let mut scope_options = oj_scope::Options {
    jai_dir: jai_dir(),
    ..oj_scope::Options::default()
  };
  scope_options.import_dirs = options.import_dirs.clone();

  let program = oj_scope::Program::build(&sources, &interner, root, scope_options);
  let mut report = Report::default();
  let render = |diagnostics: &[oj_diag::Diagnostic], report: &mut Report| {
    for diagnostic in diagnostics {
      let file = sources.file(diagnostic.source);
      report.diagnostics.push(oj_diag::render(diagnostic, &file));
    }
  };
  render(&program.diagnostics(), &mut report);
  if program.has_errors() || program.unit_count() == 0 {
    report.failed = true;
    return report;
  }

  let undeclared = oj_scope::undeclared_identifiers(&program);
  let undeclared = oj_scope::undeclared_diagnostics(&undeclared);
  render(&undeclared, &mut report);
  if undeclared.iter().any(oj_diag::Diagnostic::is_error) {
    report.failed = true;
    return report;
  }

  let mut checker = oj_sema::Checker::new(&program);
  // Compile-time execution is part of typechecking: a `#run` produces the
  // constant a declaration was waiting for (**L§12.1**).
  let engine = match oj_jit::Engine::new() {
    Ok(engine) => std::rc::Rc::new(engine),
    Err(error) => return Report::failure(error),
  };
  checker.set_compile_time(engine.clone());
  checker.check();
  render(checker.diagnostics(), &mut report);
  if checker.has_errors() {
    report.failed = true;
    return report;
  }

  // Whatever compile time wrote into an ordinary global is thrown away before
  // the executable is written; `#no_reset` is what survives (**L§12.3**).
  engine.reset_globals();
  let mut lowered = oj_ir::lower(&mut checker);
  keep_compile_time_data(&mut lowered.program, &engine);
  let lowered = lowered;
  render(&lowered.diagnostics, &mut report);
  if stage == Stage::Ir {
    report.output = format!(
      "{}{}\n",
      oj_ir::print_ir(&lowered.program, &interner, only),
      oj_ir::summary(&lowered.program)
    );
    report.failed = lowered.has_errors();
    return report;
  }
  if lowered.has_errors() {
    report.failed = true;
    return report;
  }

  let codegen = oj_codegen::Options {
    optimization: options.optimization.level(),
    module_name: oj_link::default_output_name(root),
    ..oj_codegen::Options::default()
  };

  match stage {
    Stage::Ir => unreachable!("handled above"),
    Stage::LlvmIr | Stage::Assembly => {
      let output = if stage == Stage::LlvmIr {
        oj_codegen::Output::LlvmIr
      } else {
        oj_codegen::Output::Assembly
      };
      match oj_codegen::compile(&lowered.program, &codegen, output) {
        Ok(text) => report.output = text,
        Err(error) => return Report::failure(error),
      }
      report
    }
    Stage::Executable => {
      let name = options
        .output_executable_name
        .clone()
        .unwrap_or_else(|| oj_link::default_output_name(root));
      let directory = options.output_path.clone().unwrap_or_else(|| {
        root
          .parent()
          .map(Path::to_path_buf)
          .unwrap_or_else(|| PathBuf::from("."))
      });
      let build = directory.join(".build");
      if let Err(error) = std::fs::create_dir_all(&build) {
        return Report::failure(format!("could not create {}: {error}", build.display()));
      }
      let object = build.join(format!("{name}.o"));
      if let Err(error) = oj_codegen::compile(
        &lowered.program,
        &codegen,
        oj_codegen::Output::Object(&object),
      ) {
        return Report::failure(error);
      }

      let request = oj_link::Request {
        objects: vec![object],
        output: directory.join(&name),
        output_type: oj_link::OutputType::Executable,
        libraries: lowered.program.libraries.clone(),
        additional_arguments: Vec::new(),
      };
      match oj_link::link(&request) {
        Ok(line) => {
          report.link_line = line.map(|line| line.display());
          report.executable = Some(request.output);
          report
        }
        Err(error) => Report::failure(error),
      }
    }
  }
}

/// Gives every `#no_reset` global the bytes compile-time execution left in it
/// (**L§12.3**). Everything else keeps the initializer the front end folded,
/// which is the reset.
fn keep_compile_time_data(program: &mut oj_ir::Program, engine: &oj_jit::Engine) {
  for global in &mut program.globals {
    if !global.no_reset {
      continue;
    }
    let Some(decl) = global.decl else { continue };
    let Some(mut bytes) = engine.kept_bytes(decl) else {
      continue;
    };
    bytes.resize(global.size as usize, 0);
    global.init = oj_ir::GlobalInit::Bytes(bytes.into_boxed_slice());
  }
}

/// The jai distribution the standard modules come from: `OJ_JAI_DIR`, else a
/// `vendor/jai` in the current directory or one of its ancestors
/// (`docs/spec.md` §5).
pub fn jai_dir() -> Option<PathBuf> {
  if let Some(value) = std::env::var_os("OJ_JAI_DIR").filter(|value| !value.is_empty()) {
    let candidate = PathBuf::from(value);
    return candidate.join("modules").is_dir().then_some(candidate);
  }
  let current = std::env::current_dir().ok()?;
  current.ancestors().find_map(|directory| {
    let candidate = directory.join("vendor").join("jai");
    candidate.join("modules").is_dir().then_some(candidate)
  })
}

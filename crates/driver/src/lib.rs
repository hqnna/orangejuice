//! The pipeline: source to scope tree to types to IR to an object to an
//! executable.
//!
//! `oj-driver` owns the order the stages run in, the `Build_Options` a command
//! line sets, and where the artefacts land. A metaprogram joins in through
//! `oj-meta`: its `#run`s are part of typechecking, so the workspaces it
//! created are compiled here once the checker is done with the program that
//! created them (**C§3.1**). The message loop that would let it watch a
//! workspace compile is still M8 work.

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
  /// What a metaprogram watching this compilation is told about it
  /// (**C§3.2**).
  pub compiled: oj_meta::Compiled,
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

/// The source of one compilation: the file a command line named, or the files
/// and strings a metaprogram added to a workspace (**C§3.1**).
#[derive(Clone, Debug, Default)]
pub struct Input {
  pub files: Vec<PathBuf>,
  /// Strings added to the program, each under a path of its own so that a
  /// diagnostic about one can be rendered.
  pub strings: Vec<(PathBuf, String)>,
}

impl Input {
  pub fn file(path: &Path) -> Self {
    Self {
      files: vec![path.to_path_buf()],
      strings: Vec::new(),
    }
  }

  /// The file the output is named after and the artefacts land beside.
  fn anchor(&self) -> PathBuf {
    self
      .files
      .first()
      .cloned()
      .or_else(|| self.strings.first().map(|(path, _)| path.clone()))
      .unwrap_or_default()
  }
}

/// Runs the pipeline over one root file.
pub fn run(root: &Path, options: &BuildOptions, stage: Stage, only: Option<&str>) -> Report {
  run_input(&Input::file(root), options, stage, only)
}

/// Runs the pipeline over a whole workspace.
pub fn run_input(
  input: &Input,
  options: &BuildOptions,
  stage: Stage,
  only: Option<&str>,
) -> Report {
  let root = &input.anchor();
  let sources = SourceMap::new();
  let interner = Interner::new();
  let mut scope_options = oj_scope::Options {
    jai_dir: jai_dir(),
    ..oj_scope::Options::default()
  };
  scope_options.import_dirs = options.import_dirs.clone();
  scope_options.import_remaps = options.import_remaps.clone();

  let program = oj_scope::Program::build_input(
    &sources,
    &interner,
    &input.files,
    &input.strings,
    scope_options,
  );
  let mut report = Report {
    compiled: oj_meta::Compiled {
      failed: true,
      ..describe_program(&program)
    },
    ..Report::default()
  };
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

  // A metaprogram runs as part of typechecking, since its work is done by the
  // `#run`s the checker executes (**C§3.1**). What it asks the compiler for
  // lands in this, and the workspaces it created are built once it is done.
  let mut state = metaprogram_state(&mut checker, options);
  // A workspace compiled while its metaprogram watches reports through this,
  // since the compilation that produced the diagnostics is not this one.
  let watched: std::rc::Rc<std::cell::RefCell<Vec<String>>> = std::rc::Rc::default();
  state.compiler = Some(workspace_compiler(
    root,
    options,
    stage,
    state.build_options_layout,
    watched.clone(),
  ));
  let outer = oj_meta::install(state);
  checker.check();
  let meta = oj_meta::uninstall().unwrap_or_default();
  if let Some(outer) = outer {
    oj_meta::install(outer);
  }
  render(checker.diagnostics(), &mut report);
  report.diagnostics.extend(watched.borrow_mut().drain(..));
  report_metaprogram_diagnostics(&meta, &mut report);
  if checker.has_errors() || meta.has_errors() {
    report.failed = true;
    return report;
  }
  build_workspaces(&meta, root, options, stage, &mut report);
  if report.failed {
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
      let object_name = object.display().to_string();
      if let Err(error) = oj_codegen::compile(
        &lowered.program,
        &codegen,
        oj_codegen::Output::Object(&object),
      ) {
        return Report::failure(error);
      }

      let file = match options.append_extension {
        true => format!("{name}{}", output_extension(options.output_type)),
        false => name.clone(),
      };
      let request = oj_link::Request {
        objects: vec![object],
        output: directory.join(&file),
        output_type: options.output_type,
        libraries: lowered.program.libraries.clone(),
        additional_arguments: Vec::new(),
      };
      report.compiled.object_files = vec![object_name];
      report.compiled.system_libraries = lowered
        .program
        .libraries
        .iter()
        .filter(|library| library.system)
        .map(|library| library.name.clone())
        .collect();
      report.compiled.user_libraries = lowered
        .program
        .libraries
        .iter()
        .filter(|library| !library.system)
        .map(|library| library.name.clone())
        .collect();
      match oj_link::link(&request) {
        Ok(line) => {
          report.link_line = line.map(|line| line.display());
          report.compiled.failed = false;
          // `output_type = .NO_OUTPUT` produces nothing to point at
          // (**C§4**).
          if options.output_type != oj_link::OutputType::NoOutput {
            report.compiled.executable = Some(request.output.clone());
            report.executable = Some(request.output);
          }
          report
        }
        Err(error) => Report::failure(error),
      }
    }
  }
}

/// What `compiler_get_version_info` reports: the reference distribution this
/// compiler is compatible with, spelled the way that compiler spells it
/// (**C§3.3**).
pub const JAI_VERSION: &str = "beta 0.2.009, built on 6 February 2025";
const JAI_VERSION_NUMBERS: (i32, i32, i32) = (0, 2, 9);

/// The compile-time state a metaprogram works on: what the compiler will tell
/// it about itself, and the `Build_Options` a fresh workspace starts with.
///
/// The defaults come out of the distribution's own `Build_Options`, member by
/// member, rather than being written down here — the struct is the
/// distribution's, so the only honest source for what a field starts at is the
/// declaration itself (**C§4**).
fn metaprogram_state(checker: &mut oj_sema::Checker<'_>, options: &BuildOptions) -> oj_meta::Meta {
  let mut meta = oj_meta::Meta::new();
  meta.base_path = oj_meta::base_path_of(jai_dir().as_ref());
  meta.command_line = options.compile_time_command_line.clone();
  meta.version = String::from(JAI_VERSION);
  meta.version_numbers = JAI_VERSION_NUMBERS;
  if let Some(build_options) = checker.type_named("Build_Options") {
    meta.default_build_options = checker.default_bytes(build_options).unwrap_or_default();
    meta.build_options_layout = build_options_layout(checker, build_options);
  }
  meta
}

/// Where the fields the driver acts on sit inside `Build_Options`.
fn build_options_layout(
  checker: &mut oj_sema::Checker<'_>,
  build_options: oj_types::TypeId,
) -> oj_meta::BuildOptionsLayout {
  let mut layout = oj_meta::BuildOptionsLayout::default();
  let Some(definition) = checker.types().struct_of(build_options) else {
    return layout;
  };
  let members: Vec<(String, u64)> = checker
    .types()
    .struct_info(definition)
    .members
    .iter()
    .map(|member| {
      (
        checker.interner().resolve_lossy(member.name).into_owned(),
        member.offset,
      )
    })
    .collect();
  for (name, offset) in members {
    match name.as_str() {
      "output_executable_name" => layout.output_executable_name = Some(offset),
      "output_path" => layout.output_path = Some(offset),
      "output_type" => layout.output_type = Some(offset),
      "append_executable_filename_extension" => {
        layout.append_executable_filename_extension = Some(offset);
      }
      _ => {}
    }
  }
  layout
}

/// What a metaprogram is told about a compilation's source (**C§3.2**): one
/// module instantiation per `#import`, and the files each one loaded.
fn describe_program(program: &oj_scope::Program<'_>) -> oj_meta::Compiled {
  let modules = program.modules();
  let compiled_modules: Vec<oj_meta::CompiledModule> = modules
    .iter()
    .map(|module| oj_meta::CompiledModule {
      name: module.name.clone(),
      entry: module.entry.clone(),
      module_type: match module.kind {
        oj_scope::ModuleKind::Preload => oj_meta::ModuleType::Preload,
        oj_scope::ModuleKind::RuntimeSupport => oj_meta::ModuleType::RuntimeSupport,
        oj_scope::ModuleKind::MainProgram => oj_meta::ModuleType::MainProgram,
        oj_scope::ModuleKind::File => oj_meta::ModuleType::File,
      },
    })
    .collect();

  let files = program
    .units()
    .map(|unit| {
      let module = program.tree().parent(unit.scope);
      oj_meta::CompiledFile {
        module: module.and_then(|scope| modules.iter().position(|entry| entry.scope == scope)),
        from_a_string: !unit.path.is_file(),
        path: unit.path.clone(),
      }
    })
    .collect();

  oj_meta::Compiled {
    modules: compiled_modules,
    files,
    ..oj_meta::Compiled::default()
  }
}

/// How a metaprogram's `compiler_wait_for_message` gets a workspace compiled:
/// the whole pipeline again, from inside the `#run` that is watching it
/// (`docs/spec.md` §10).
fn workspace_compiler(
  outer: &Path,
  options: &BuildOptions,
  stage: Stage,
  layout: oj_meta::BuildOptionsLayout,
  sink: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
) -> oj_meta::Compiler {
  let outer = outer.to_path_buf();
  let options = options.clone();
  std::rc::Rc::new(move |workspace: &oj_meta::Workspace| {
    let (input, nested) = workspace_input(workspace, &layout, &outer, &options);
    let report = run_input(&input, &nested, stage, None);
    let mut compiled = report.compiled;
    compiled.errors = report.diagnostics.len();
    compiled.failed |= report.failed;
    sink.borrow_mut().extend(report.diagnostics);
    compiled
  })
}

/// The input and the options one workspace compiles with, as its metaprogram
/// set them (**C§3.1**).
fn workspace_input(
  workspace: &oj_meta::Workspace,
  layout: &oj_meta::BuildOptionsLayout,
  outer: &Path,
  options: &BuildOptions,
) -> (Input, BuildOptions) {
  let directory = outer
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_else(|| PathBuf::from("."));
  let mut nested = options.clone();
  nested.compile_time_command_line = Vec::new();
  if let Some(name) = workspace.option_string(layout, |layout| layout.output_executable_name)
    && !name.is_empty()
  {
    nested.output_executable_name = Some(name);
  }
  if let Some(path) = workspace.option_string(layout, |layout| layout.output_path)
    && !path.is_empty()
  {
    nested.output_path = Some(PathBuf::from(path));
  }
  if let Some(output_type) = workspace.option_u8(layout, |layout| layout.output_type) {
    nested.output_type = output_type_of(output_type);
  }
  if let Some(append) =
    workspace.option_u8(layout, |layout| layout.append_executable_filename_extension)
  {
    nested.append_extension = append != 0;
  }
  nested.import_remaps = workspace
    .remaps
    .iter()
    .map(|(host, import, replacement)| oj_scope::ImportRemap {
      host: host.clone(),
      import: import.clone(),
      replacement: replacement.clone(),
    })
    .collect();
  let input = Input {
    files: workspace.files.clone(),
    strings: workspace
      .strings
      .iter()
      .enumerate()
      .map(|(index, text)| {
        (
          added_string_path(workspace, &directory, index),
          text.clone(),
        )
      })
      .collect(),
  };
  (input, nested)
}
/// Turns what a metaprogram reported into diagnostics of the compilation
/// (**C§3.3**).
fn report_metaprogram_diagnostics(meta: &oj_meta::Meta, report: &mut Report) {
  for entry in &meta.reports {
    let label = match entry.mode {
      oj_meta::ReportMode::Error | oj_meta::ReportMode::ErrorContinuable => "Error",
      oj_meta::ReportMode::Warning => "Warning",
      oj_meta::ReportMode::Info => "Info",
    };
    let location = if entry.filename.is_empty() {
      String::new()
    } else {
      format!("{}:{},{}: ", entry.filename, entry.line, entry.character)
    };
    report
      .diagnostics
      .push(format!("{location}{label}: {}\n", entry.message));
  }
}

/// Compiles the workspaces a metaprogram created (**C§3.1**). Each one is a
/// program of its own: its own scope tree, its own typechecking, its own
/// executable, named by the `Build_Options` the metaprogram set on it.
fn build_workspaces(
  meta: &oj_meta::Meta,
  outer: &Path,
  options: &BuildOptions,
  stage: Stage,
  report: &mut Report,
) {
  for workspace in meta.buildable() {
    let (input, nested) = workspace_input(workspace, &meta.build_options_layout, outer, options);
    let inner = run_input(&input, &nested, stage, None);
    report.diagnostics.extend(inner.diagnostics);
    if inner.failed {
      report.failed = true;
      return;
    }
    if report.executable.is_none() {
      report.executable = inner.executable;
      report.link_line = inner.link_line;
    }
  }
}

/// `Build_Options.output_type` as the distribution numbers it (**C§4**). A
/// static library is the one orangejuice cannot produce yet, so it is linked
/// as an object file instead.
fn output_type_of(value: u8) -> oj_link::OutputType {
  match value {
    0 => oj_link::OutputType::NoOutput,
    2 => oj_link::OutputType::DynamicLibrary,
    3 | 4 => oj_link::OutputType::ObjectFile,
    _ => oj_link::OutputType::Executable,
  }
}

/// The extension the output takes, which is the platform's rather than the
/// name's (**C§4**).
fn output_extension(output_type: oj_link::OutputType) -> &'static str {
  match output_type {
    oj_link::OutputType::DynamicLibrary => ".so",
    oj_link::OutputType::ObjectFile => ".o",
    _ => "",
  }
}

/// The name an added string is given, which is where a diagnostic about it
/// points and what a `#load` inside it resolves against (**C§3.1**).
fn added_string_path(workspace: &oj_meta::Workspace, outer: &Path, index: usize) -> PathBuf {
  let directory = workspace
    .files
    .first()
    .and_then(|file| file.parent().map(Path::to_path_buf))
    .unwrap_or_else(|| outer.to_path_buf());
  directory.join(format!(
    ".added_strings_w{}_{}.jai",
    workspace.id,
    index + 1
  ))
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

//! The pipeline: source to scope tree to types to IR to an object to an
//! executable.
//!
//! `oj-driver` owns the order the stages run in, the `Build_Options` a command
//! line sets, and where the artefacts land. A metaprogram joins in through
//! `oj-meta`: its `#run`s are part of typechecking, so the workspaces it
//! created are compiled here once the checker is done with the program that
//! created them (**C§3.1**), and one it *watches* is compiled from inside the
//! `compiler_wait_for_message` that asked for its messages. The distribution's
//! own `Default_Metaprogram` is the driver: `oj build` compiles it and hands it
//! the command line, and it creates the workspace the program is compiled in
//! (**C§2.1**).

mod options;

use std::path::{Path, PathBuf};

use oj_diag::SourceMap;
use oj_lexer::Interner;

pub use options::{BuildOptions, Optimization, OptionError, ParsedOptions, RuntimeSupport, parse};

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
  /// How many of them are *errors*. A metaprogram is sent one `Message.ERROR`
  /// each (**C§3.2**), and `Default_Metaprogram` and every example's own build
  /// file `exit(1)` the moment they see one — so a warning must not be counted
  /// among them, and neither must a diagnostic belonging to some other
  /// workspace this one only carried out.
  pub errors: usize,
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
      errors: 1,
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

/// What `-add` and `-run` put into a compilation nothing else drives: one
/// unit per string, named the way a metaprogram's `add_build_string` names
/// one so that a diagnostic about it can be rendered (**C§2.1**).
pub fn command_line_strings(files: &[PathBuf], options: &BuildOptions) -> Vec<(PathBuf, String)> {
  let directory = files
    .first()
    .and_then(|file| file.parent().map(Path::to_path_buf))
    .unwrap_or_else(|| PathBuf::from("."));
  options
    .build_strings
    .iter()
    .enumerate()
    .map(|(index, text)| {
      (
        directory.join(format!(
          ".added_strings_w{}_{}.jai",
          options.workspace_id,
          index + 1
        )),
        text.clone(),
      )
    })
    .collect()
}

/// Runs the pipeline over one root file.
pub fn run(root: &Path, options: &BuildOptions, stage: Stage, only: Option<&str>) -> Report {
  run_input(&Input::file(root), options, stage, only)
}

/// Where the distribution's own startup metaprogram lives, when there is one
/// (**C§2.1**).
pub fn default_metaprogram() -> Option<PathBuf> {
  named_metaprogram("Default_Metaprogram")
}

/// The metaprogram module `-- meta` named, when the distribution has one
/// (**C§2.1**). A module is either `modules/<Name>.jai` or
/// `modules/<Name>/module.jai`, the same two shapes an `#import` looks for.
pub fn named_metaprogram(name: &str) -> Option<PathBuf> {
  let modules = distribution()?.join("modules");
  let flat = modules.join(format!("{name}.jai"));
  if flat.is_file() {
    return Some(flat);
  }
  let directory = modules.join(name).join("module.jai");
  directory.is_file().then_some(directory)
}

/// Compiles the way the reference does: `Default_Metaprogram` is the program,
/// and the file and options the command line named reach it as
/// `Build_Options.compile_time_command_line`, from which it creates the
/// workspace the target program is compiled in (**C§2.1**, **C§3.1**).
///
/// The metaprogram itself produces no executable — it turns its own output off
/// with `set_build_options_dc` — so what comes back is what the workspace it
/// created produced.
pub fn run_through_metaprogram(
  metaprogram: &Path,
  files: &[PathBuf],
  arguments: &[String],
  options: &BuildOptions,
  stage: Stage,
) -> Report {
  let mut driver = options.clone();
  // The command line reaches the metaprogram with its file names made
  // absolute: it hands them to `add_build_file`, and a workspace resolves a
  // relative path against the file that created it — which is the
  // metaprogram, sitting in the distribution's own modules directory, not
  // wherever the user is standing.
  let absolute: Vec<PathBuf> = files
    .iter()
    .map(|file| std::fs::canonicalize(file).unwrap_or_else(|_| file.clone()))
    .collect();
  let mut line: Vec<String> = arguments
    .iter()
    .map(|argument| {
      match absolute
        .iter()
        .zip(files)
        .find(|(_, original)| original.as_os_str() == argument.as_str())
      {
        Some((made, _)) => made.display().to_string(),
        None => argument.clone(),
      }
    })
    .collect();
  // A caller that passed the files separately rather than as part of the
  // command line still has to tell the metaprogram what to compile, since the
  // command line is all it is given.
  for file in &absolute {
    let named = file.display().to_string();
    if !line.contains(&named) {
      line.insert(0, named);
    }
  }
  driver.compile_time_command_line = line;
  // The target workspace's output belongs where its own file is, not where the
  // distribution keeps its modules; the metaprogram sets that itself, so the
  // compilation it drives starts from nothing of ours.
  driver.output_executable_name = None;
  driver.output_path = None;
  run(metaprogram, &driver, stage, None)
}

/// The executable a workspace produced, and the line that linked it.
type Produced = (PathBuf, Option<String>);

/// What a compilation shares with the workspaces its metaprogram watches
/// compile (**C§3.1**): those compilations are not this one, so their
/// diagnostics, the storage their trees are exported into, and whatever the
/// first of them produced all come back through here.
///
/// The diagnostics are kept per workspace and replaced rather than appended,
/// because a `provide_import` makes a workspace compile again and only the
/// last attempt is what happened (**C§3.2**).
#[derive(Clone, Default)]
struct Watching {
  diagnostics: std::rc::Rc<std::cell::RefCell<std::collections::BTreeMap<i64, Vec<String>>>>,
  nodes: std::rc::Rc<std::cell::RefCell<oj_meta::Nodes>>,
  produced: std::rc::Rc<std::cell::RefCell<Option<Produced>>>,
}

/// Runs the pipeline over a whole workspace.
pub fn run_input(
  input: &Input,
  options: &BuildOptions,
  stage: Stage,
  only: Option<&str>,
) -> Report {
  run_workspace(input, options, stage, only, None)
}

/// Runs the pipeline over a workspace a metaprogram is watching, exporting its
/// trees into the storage that metaprogram reads (**C§3.2**, **C§5.3**). The
/// nodes belong to the compilation *above* this one, since that is the one
/// whose metaprogram keeps the pointers.
fn run_workspace(
  input: &Input,
  options: &BuildOptions,
  stage: Stage,
  only: Option<&str>,
  watching: Option<std::rc::Rc<std::cell::RefCell<oj_meta::Nodes>>>,
) -> Report {
  // A metaprogram may add source to the compilation it is itself part of —
  // that is how `Metaprogram_Plugins` fills the `#placeholder` its own body
  // waits on. The reference stalls the `#run` until the declaration arrives;
  // orangejuice cannot (`docs/spec.md` §6.5), so the compilation is run again
  // with the source in place, which is the same replay every other
  // after-the-fact answer gets.
  let mut options = options.clone();
  for _ in 0..SELF_MODIFICATION_ROUNDS {
    let mut added = Vec::new();
    let report = run_workspace_once(input, &options, stage, only, watching.clone(), &mut added);
    if added.is_empty() {
      return report;
    }
    options.added_strings.extend(added);
  }
  Report::failure("a metaprogram kept adding source to its own compilation")
}

/// How many times a compilation is run again for source its own metaprogram
/// added. One round is what `Metaprogram_Plugins` needs; the rest is only so
/// that a metaprogram that never settles stops rather than runs forever.
const SELF_MODIFICATION_ROUNDS: usize = 8;

fn run_workspace_once(
  input: &Input,
  options: &BuildOptions,
  stage: Stage,
  only: Option<&str>,
  watching: Option<std::rc::Rc<std::cell::RefCell<oj_meta::Nodes>>>,
  self_added: &mut Vec<oj_scope::AddedString>,
) -> Report {
  let mut timing = Timing::new();
  let root = &input.anchor();
  let sources = SourceMap::new();
  let interner = Interner::new();
  let mut scope_options = oj_scope::Options {
    distribution: distribution(),
    ..oj_scope::Options::default()
  };
  // `OS`, `CPU` and `IS_CROSS_COMPILING` are what the workspace is built for
  // (**L§17**). The checker reads them back off the program, so this is the
  // one place a compilation is told what it is for.
  scope_options.target = options.target;
  scope_options.import_dirs = options.import_dirs.clone();
  scope_options.import_remaps = options.import_remaps.clone();
  scope_options.provided_imports = options.provided_imports.clone();
  scope_options.added_strings = options.added_strings.clone();
  scope_options.modified_bodies = options.modified_bodies.clone();

  let program = oj_scope::Program::build_input(
    &sources,
    &interner,
    &input.files,
    &input.strings,
    scope_options,
  );
  timing.mark("lex+parse+scope");
  let mut report = Report {
    compiled: oj_meta::Compiled {
      failed: true,
      ..describe_program(&program)
    },
    ..Report::default()
  };
  // `-msvc_format` is what an editor that parses MSVC output reads (**C§4**).
  let style = match options.visual_studio_format {
    true => oj_diag::Style::VisualStudio,
    false => oj_diag::Style::Plain,
  };
  let render = |diagnostics: &[oj_diag::Diagnostic], report: &mut Report| {
    for diagnostic in diagnostics {
      report.errors += usize::from(diagnostic.is_error());
      if diagnostic.source == oj_diag::SourceId::NONE {
        report
          .diagnostics
          .push(oj_diag::render_unplaced(diagnostic));
        continue;
      }
      let file = sources.file(diagnostic.source);
      report
        .diagnostics
        .push(oj_diag::render_with(diagnostic, &file, style));
    }
  };
  render(&program.diagnostics(), &mut report);
  if program.has_errors() || program.unit_count() == 0 {
    report.failed = true;
    // An import that could not be found is what stopped this compilation, and
    // it is the one a metaprogram most wants to be pointed at (**C§3.2**), so
    // the `#import`s are exported even though nothing else will be.
    if let Some(nodes) = &watching {
      let generation = nodes.borrow_mut().begin_compilation();
      let mut checker = oj_sema::Checker::new(&program);
      let mut borrowed = nodes.borrow_mut();
      let mut exporter = oj_sema::Exporter::new(&mut checker, &mut borrowed, generation);
      export_failed_imports(&mut exporter, &program, &mut report.compiled);
    }
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
  checker.set_context_size_max(options.context_size_max);
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
  let mut state = metaprogram_state(&mut checker, options, root);
  if let Some(nodes) = &watching {
    state.nodes = nodes.clone();
  }
  // A `Code` value is the address of a node in this storage (**L§13.1**), so
  // the checker writes into the same arena the metaprogram reads. The arena
  // outlives this compilation — a watched workspace shares the one above it,
  // and compiles again after a `provide_import` — so what this one exports is
  // keyed under a number of its own.
  let nodes = state.nodes.clone();
  let generation = nodes.borrow_mut().begin_compilation();
  checker.set_nodes(nodes.clone(), generation);
  let watched = Watching {
    nodes: nodes.clone(),
    ..Watching::default()
  };
  state.compiler = Some(workspace_compiler(
    root,
    options,
    stage,
    state.build_options_layout,
    state.during_compile_layout,
    watched.clone(),
  ));
  // A node carries the `Message_File` of the file it was written in, and the
  // reference makes those whether or not a metaprogram is listening — so this
  // compilation makes its own before the checker can export anything
  // (**C§3.2**).
  state.declare_own_files(&report.compiled);
  let outer = oj_meta::install(state);
  checker.check();
  let meta = oj_meta::uninstall().unwrap_or_default();
  timing.mark("typecheck+#run");
  if meta.self_modified {
    *self_added = self_added_strings(&meta, root, &options.added_strings);
  }
  // What a watching metaprogram is told about this compilation's program: the
  // trees, exported once the checker is done with them (**C§3.2**).
  if watching.is_some() {
    let addresses = place_type_image(&mut checker, &nodes);
    let mut borrowed = nodes.borrow_mut();
    let mut exporter = oj_sema::Exporter::new(&mut checker, &mut borrowed, generation);
    exporter.set_type_addresses(addresses);
    report.compiled.typechecked = vec![exporter.program()];
    export_failed_imports(&mut exporter, &program, &mut report.compiled);
  }
  if let Some(outer) = outer {
    oj_meta::install(outer);
  }
  // A polymorphic body nobody instantiated was never typechecked, so the batch
  // before typechecking left its names alone; the ones a call site did
  // specialize are reported now (**L§7.8**).
  let instantiated = oj_scope::undeclared_in_instantiations(&program);
  let instantiated = oj_scope::undeclared_diagnostics(&instantiated);
  let instantiated_failed = instantiated.iter().any(oj_diag::Diagnostic::is_error);
  render(&instantiated, &mut report);
  render(checker.diagnostics(), &mut report);
  for (_, diagnostics) in std::mem::take(&mut *watched.diagnostics.borrow_mut()) {
    report.diagnostics.extend(diagnostics);
  }
  report_metaprogram_diagnostics(&meta, &mut report);
  if instantiated_failed || checker.has_errors() || meta.has_errors() {
    report.failed = true;
    return report;
  }
  if let Some((executable, line)) = watched.produced.borrow_mut().take() {
    report.executable = Some(executable);
    report.link_line = line;
  }
  build_workspaces(&meta, root, options, stage, &mut report);
  if report.failed {
    return report;
  }

  // `set_build_options_dc` changes a compilation that is already running, so
  // what the metaprogram said about this one is read here rather than before
  // it started (**C§3.1**).
  let mut effective = options.clone();
  if let Some(current) = meta
    .workspaces
    .iter()
    .find(|workspace| workspace.id == meta.current)
  {
    apply_during_compile(current, &meta.during_compile_layout, &mut effective);
  }
  let options = &effective;

  // What a metaprogram asked to be left out of a type's record is decided
  // before the executable's own type table is laid out (**C§3.3**).
  for (type_id, flags) in &meta.type_info_flags {
    checker.add_type_info_flags(oj_types::TypeId(*type_id), *flags);
  }

  // Whatever compile time wrote into an ordinary global is thrown away before
  // the executable is written; `#no_reset` is what survives (**L§12.3**).
  engine.reset_globals();
  // `NO_OUTPUT` produces nothing, so there is nothing to lower and no entry
  // point to ask for (**C§4**) — a metaprogram that sets `do_output = false`
  // is the whole program. A dump stage was asked for explicitly, so it still
  // gets its answer.
  if options.output_type == oj_link::OutputType::NoOutput && stage == Stage::Executable {
    // Producing nothing is what the metaprogram asked for, not a failure: a
    // watching one is told the workspace compiled (**C§3.2**).
    report.compiled.failed = false;
    return report;
  }
  // A library has no `main`; what it holds is whatever its exports reach
  // (**L§11.6**).
  let lower_options = oj_ir::LowerOptions {
    stack_trace: options.stack_trace,
    backtrace_on_crash: options.backtrace_on_crash,
  };
  // Only an executable has to have a `main`: a library and an object file hold
  // whatever their exports reach (**L§11.6**, **C§4**).
  let mut lowered = match options.output_type {
    oj_link::OutputType::Executable => {
      oj_ir::lower_with_roots(&mut checker, &options.live_procedures, lower_options)
    }
    _ => oj_ir::lower_library(
      &mut checker,
      options.runtime_support.defines_init(),
      lower_options,
    ),
  };
  keep_compile_time_data(&mut lowered.program, &engine);
  let lowered = lowered;
  timing.mark("lower to ir");
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
    target: options.target,
    optimization: options.optimization.level(),
    bitcode: options.optimization.bitcode(),
    debug_info: options.debug_info,
    passes: options.optimization.passes(),
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
      // An object-file build *is* the object the caller asked for, so there is
      // nothing to split it across; everything else is linked, and may be.
      // `llvm_options.enable_split_modules` is what `-no_split` turns off
      // (**C§4**), which is one module for the whole program.
      let units = match options.output_type {
        oj_link::OutputType::ObjectFile => 1,
        _ if !options.enable_split_modules => 1,
        _ => oj_codegen::default_units(&lowered.program),
      };
      // `llvm_options.output_llvm_ir` writes the module beside the object, so
      // that what the back end was given can be read after a build (**C§4**).
      // One module, whatever the split would have been: a listing is for
      // reading.
      if options.output_llvm_ir {
        match oj_codegen::compile(&lowered.program, &codegen, oj_codegen::Output::LlvmIr) {
          Ok(listing) => {
            let path = object.with_extension("ll");
            if let Err(error) = std::fs::write(&path, listing) {
              return Report::failure(format!("could not write {}: {error}", path.display()));
            }
          }
          Err(error) => return Report::failure(error),
        }
      }
      let objects = match oj_codegen::compile_objects(&lowered.program, &codegen, &object, units) {
        Ok(objects) => objects,
        Err(error) => return Report::failure(error),
      };
      timing.mark("llvm + object");

      let file = match options.append_extension {
        true => format!("{name}{}", output_extension(options.output_type)),
        false => name.clone(),
      };
      let request = oj_link::Request {
        target: options.target,
        objects: objects.clone(),
        output: directory.join(&file),
        output_type: options.output_type,
        libraries: lowered.program.libraries.clone(),
        additional_arguments: Vec::new(),
      };
      report.compiled.object_files = objects
        .iter()
        .map(|path| path.display().to_string())
        .collect();
      let _ = object_name;
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
      // `use_custom_link_command` hands the link over to the metaprogram: the
      // compiler stops at the object and reports what it made, and the
      // metaprogram runs its own linker when it sees the phase (**C§3.2**).
      if options.custom_link_command {
        report.compiled.custom_link_command = true;
        report.compiled.failed = false;
        report.compiled.executable = Some(request.output);
        return report;
      }
      let linked = oj_link::link(&request);
      timing.mark("link");
      match linked {
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

/// The release of the Jai language orangejuice implements. Every other place
/// the version is spelled — `oj version`, the README, a metaprogram's
/// `compiler_get_version_info` — reads it from here, so there is one answer to
/// what this compiler is compatible with.
pub const JAI_VERSION: &str = "beta 0.2.009";

/// The date that release was built on, which `compiler_get_version_info`
/// reports beside the version (**C§3.3**).
pub const JAI_BUILD_DATE: &str = "6 February 2025";

/// The same release as the three numbers a metaprogram compares against.
pub const JAI_VERSION_NUMBERS: (i32, i32, i32) = (0, 2, 9);

/// The compile-time state a metaprogram works on: what the compiler will tell
/// it about itself, and the `Build_Options` a fresh workspace starts with.
///
/// The defaults come out of the distribution's own `Build_Options`, member by
/// member, rather than being written down here — the struct is the
/// distribution's, so the only honest source for what a field starts at is the
/// declaration itself (**C§4**).
fn metaprogram_state(
  checker: &mut oj_sema::Checker<'_>,
  options: &BuildOptions,
  root: &Path,
) -> oj_meta::Meta {
  let mut meta = oj_meta::Meta::numbered_from(options.workspace_id.max(1));
  meta.base_path = oj_meta::base_path_of(distribution().as_ref());
  meta.command_line = options.compile_time_command_line.clone();
  meta.version = format!("{JAI_VERSION}, built on {JAI_BUILD_DATE}");
  meta.version_numbers = JAI_VERSION_NUMBERS;
  if let Some(build_options) = checker.type_named("Build_Options") {
    meta.default_build_options = checker.default_bytes(build_options).unwrap_or_default();
    meta.build_options_layout = build_options_layout(checker, build_options);
    if let Some(at) = meta.build_options_layout.compile_time_command_line {
      meta.seed_command_line(at);
    }
    // A fresh `Build_Options` already names the paths the compiler chose
    // (**C§4**): where the output goes, where the intermediates go, and what a
    // `#import` searches.
    seed_default_paths(&mut meta, options, root);
  }
  if let Some(during_compile) = checker.type_named("Build_Options_During_Compile") {
    meta.during_compile_layout = during_compile_layout(checker, during_compile);
  }
  // What an earlier round of this compilation already added to itself: this
  // run is the replay it asked for, so asking again changes nothing.
  meta.applied_self_strings = options
    .added_strings
    .iter()
    .filter_map(|added| match &added.target {
      oj_scope::StringTarget::File(path) => {
        Some((oj_meta::StringScope::File(path.clone()), added.text.clone()))
      }
      _ => None,
    })
    .collect();
  meta
}

/// The paths a fresh `Build_Options` already names (**C§4**): the ones the
/// command line and the file being compiled decide, rather than defaults
/// written down here.
fn seed_default_paths(meta: &mut oj_meta::Meta, options: &BuildOptions, root: &Path) {
  let directory = root
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_else(|| PathBuf::from("."));
  let layout = meta.build_options_layout;
  // A *fresh* `Build_Options` names the directory the file being compiled is
  // in, not whatever `-output_path` said: `output_path` is not one of the
  // `Commonly_Propagated` fields, so a workspace a metaprogram creates writes
  // beside its own first file unless it is told otherwise (**C§4**). The
  // command line reaches the target workspace because `Default_Metaprogram`
  // parses it and sets it there.
  if let Some(at) = layout.output_path {
    meta.seed_string(at, &directory.display().to_string());
  }
  if let Some(at) = layout.intermediate_path {
    let intermediate = directory.join(".build");
    meta.seed_string(at, &intermediate.display().to_string());
  }
  if let Some(at) = layout.import_path {
    let mut path: Vec<String> = options
      .import_dirs
      .iter()
      .map(|directory| directory.display().to_string())
      .collect();
    path.push(directory.join("modules").display().to_string());
    if let Some(jai) = distribution() {
      path.push(format!("{}/", jai.join("modules").display()));
    }
    meta.seed_strings(at, &path);
  }
}

/// Where the fields the driver acts on sit inside `Build_Options`.
fn build_options_layout(
  checker: &mut oj_sema::Checker<'_>,
  build_options: oj_types::TypeId,
) -> oj_meta::BuildOptionsLayout {
  let mut layout = oj_meta::BuildOptionsLayout::default();
  for (name, offset) in struct_members(checker, build_options) {
    match name.as_str() {
      "output_executable_name" => layout.output_executable_name = Some(offset),
      "output_path" => layout.output_path = Some(offset),
      "output_type" => layout.output_type = Some(offset),
      "compile_time_command_line" => layout.compile_time_command_line = Some(offset),
      "runtime_support_definitions" => layout.runtime_support_definitions = Some(offset),
      "use_custom_link_command" => layout.use_custom_link_command = Some(offset),
      "intermediate_path" => layout.intermediate_path = Some(offset),
      "import_path" => layout.import_path = Some(offset),
      "os_target" => layout.os_target = Some(offset),
      "cpu_target" => layout.cpu_target = Some(offset),
      "context_size_max" => layout.context_size_max = Some(offset),
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

  // An import that did not happen is one a metaprogram may still answer
  // (**C§3.2**). The `#import` it names is exported later, once the checker
  // exists to export it with.
  let failed_imports = program
    .failed_imports()
    .into_iter()
    .map(|failed| oj_meta::CompiledFailedImport {
      status: match failed.status {
        oj_scope::FailedImportStatus::Blocked => oj_meta::ImportStatus::Blocked,
        oj_scope::FailedImportStatus::NotFound => oj_meta::ImportStatus::NotFound,
      },
      host: failed.host,
      target: failed.target,
      import_code: 0,
    })
    .collect();

  oj_meta::Compiled {
    modules: compiled_modules,
    files,
    failed_imports,
    ..oj_meta::Compiled::default()
  }
}

/// Points each failed import at the `#import` that asked for it, which is what
/// `Message_Failed_Import.import_code` names (**C§3.2**).
fn export_failed_imports(
  exporter: &mut oj_sema::Exporter<'_, '_>,
  program: &oj_scope::Program<'_>,
  compiled: &mut oj_meta::Compiled,
) {
  let failed = program.failed_imports();
  for (entry, import) in compiled.failed_imports.iter_mut().zip(&failed) {
    entry.import_code = exporter.node(import.source, import.node) as usize;
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
  dc_layout: oj_meta::DuringCompileLayout,
  watching: Watching,
) -> oj_meta::Compiler {
  let outer = outer.to_path_buf();
  let options = options.clone();
  std::rc::Rc::new(move |workspace: &oj_meta::Workspace| {
    let report = match unsupported_target(workspace, &layout) {
      Some(message) => Report::failure(message),
      None => {
        let (input, nested) = workspace_input(workspace, &layout, &dc_layout, &outer, &options);
        run_workspace(&input, &nested, stage, None, Some(watching.nodes.clone()))
      }
    };
    let mut compiled = report.compiled;
    compiled.errors = report.errors;
    compiled.failed |= report.failed;
    // The compiler says which workspace a diagnostic belongs to before it
    // reports one, since the compilation it came from is not the one the
    // command line named (**C§12**).
    let mut diagnostics = report.diagnostics;
    if !diagnostics.is_empty() {
      diagnostics.insert(
        0,
        format!(
          "\nIn Workspace {} (\"{}\"):\n",
          workspace.id, workspace.name
        ),
      );
    }
    watching
      .diagnostics
      .borrow_mut()
      .insert(workspace.id, diagnostics);
    if let Some(executable) = report.executable
      && watching.produced.borrow().is_none()
    {
      *watching.produced.borrow_mut() = Some((executable, report.link_line));
    }
    compiled
  })
}

/// What a workspace asked to be compiled for: the target its `os_target` and
/// `cpu_target` name (**C§4**), or the message saying why that is not one
/// orangejuice produces output for (`docs/spec.md` §2.1). A metaprogram that
/// names a target the back end has nothing to emit for is told so rather than
/// handed a native executable it did not ask for.
fn workspace_target(
  workspace: &oj_meta::Workspace,
  layout: &oj_meta::BuildOptionsLayout,
  default: oj_types::Target,
) -> Result<oj_types::Target, String> {
  let os = match workspace.option_u32(layout, |layout| layout.os_target) {
    Some(tag) => oj_types::Os::from_tag(tag).ok_or_else(|| {
      format!("Compiling for this operating system is not supported (os_target is {tag}).")
    })?,
    None => default.os,
  };
  let cpu = match workspace.option_u32(layout, |layout| layout.cpu_target) {
    Some(tag) => oj_types::Cpu::from_tag(tag).ok_or_else(|| {
      format!("Compiling for this processor is not supported (cpu_target is {tag}).")
    })?,
    None => default.cpu,
  };
  let target = oj_types::Target::new(os, cpu);
  if !SUPPORTED_TARGETS.contains(&target) {
    return Err(format!(
      "Compiling for {target} is not supported (os_target is {}, cpu_target is {}).",
      os.tag(),
      cpu.tag()
    ));
  }
  Ok(target)
}

/// The targets orangejuice produces output for (`docs/spec.md` §2.1).
const SUPPORTED_TARGETS: [oj_types::Target; 3] = [
  oj_types::Target::LINUX_X64,
  oj_types::Target::LINUX_ARM64,
  oj_types::Target::MACOS_ARM64,
];

/// The message a workspace naming a target the compiler has no output for is
/// answered with.
fn unsupported_target(
  workspace: &oj_meta::Workspace,
  layout: &oj_meta::BuildOptionsLayout,
) -> Option<String> {
  workspace_target(workspace, layout, oj_types::Target::HOST).err()
}

/// The input and the options one workspace compiles with, as its metaprogram
/// set them (**C§3.1**).
fn workspace_input(
  workspace: &oj_meta::Workspace,
  layout: &oj_meta::BuildOptionsLayout,
  dc_layout: &oj_meta::DuringCompileLayout,
  outer: &Path,
  options: &BuildOptions,
) -> (Input, BuildOptions) {
  let directory = outer
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_else(|| PathBuf::from("."));
  let mut nested = options.clone();
  // `os_target`/`cpu_target` are `Commonly_Propagated` (**C§4**), so a
  // workspace starts with the outer compilation's and keeps whatever its own
  // metaprogram then set.
  nested.target = workspace_target(workspace, layout, options.target).unwrap_or(options.target);
  // What the workspace's own `#run`s read as their command line: the
  // metaprogram driving it decides that, which is how `Default_Metaprogram`
  // hands a program the arguments written after the lone `-` (**C§2.1**).
  nested.compile_time_command_line = workspace
    .option_strings(layout, |layout| layout.compile_time_command_line)
    .unwrap_or_default();
  nested.workspace_id = workspace.id;
  // Where a workspace writes is *not* one of the `Commonly_Propagated` fields
  // (**C§4**): a fresh one puts its output beside the first file it was given
  // unless its own `Build_Options` say otherwise, which is what lets
  // `examples/dll` build its library beside `helper.jai` and then link it.
  nested.output_path = None;
  nested.output_executable_name = None;
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
  if let Some(support) = workspace.option_u8(layout, |layout| layout.runtime_support_definitions) {
    nested.runtime_support = RuntimeSupport::from_value(support);
  }
  if let Some(custom) = workspace.option_u8(layout, |layout| layout.use_custom_link_command) {
    nested.custom_link_command = custom != 0;
  }
  // A workspace a metaprogram created starts from a *fresh* `Build_Options`
  // (**C§4**), so what `#Context` is padded out to there is what that struct's
  // own default says unless the metaprogram changed it.
  if let Some(size) = workspace.option_s64(layout, |layout| layout.context_size_max)
    && size > 0
  {
    nested.context_size_max = size as u64;
  }
  // A metaprogram that added a directory to `import_path` means a `#import` in
  // the workspace it drives to search there (**C§4**).
  if let Some(path) = workspace.option_strings(layout, |layout| layout.import_path)
    && !path.is_empty()
  {
    nested.import_dirs = path.into_iter().map(PathBuf::from).collect();
  }
  apply_during_compile(workspace, dc_layout, &mut nested);
  nested.import_remaps = workspace
    .remaps
    .iter()
    .map(|(host, import, replacement)| oj_scope::ImportRemap {
      host: host.clone(),
      import: import.clone(),
      replacement: replacement.clone(),
    })
    .collect();
  nested.provided_imports = workspace
    .provided_imports
    .iter()
    .map(|provided| oj_scope::ProvidedImport {
      host: provided.host.clone(),
      import: provided.import.clone(),
      kind: oj_scope::ProvidedImportKind::from_value(provided.kind),
      value: provided.value.clone(),
    })
    .collect();
  nested.added_strings = workspace
    .scoped_strings
    .iter()
    .enumerate()
    .map(|(index, added)| oj_scope::AddedString {
      target: match &added.target {
        oj_meta::StringScope::MainProgram => oj_scope::StringTarget::MainProgram,
        oj_meta::StringScope::File(path) => oj_scope::StringTarget::File(path.clone()),
        oj_meta::StringScope::Module(name) => oj_scope::StringTarget::Module(name.clone()),
      },
      path: directory.join(format!(".scoped_string_w{}_{index}.jai", workspace.id)),
      text: added.text.clone(),
    })
    .collect();
  nested.live_procedures = workspace.live_procedures.clone();
  nested.modified_bodies = workspace
    .modified_bodies
    .iter()
    .map(|modified| oj_scope::ModifiedBody {
      path: PathBuf::from(&modified.file),
      name: modified.name.clone(),
      text: modified.text.clone(),
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
/// Where the `Build_Options_During_Compile` members the driver acts on sit
/// (**C§3.1**).
fn during_compile_layout(
  checker: &mut oj_sema::Checker<'_>,
  during_compile: oj_types::TypeId,
) -> oj_meta::DuringCompileLayout {
  let mut layout = oj_meta::DuringCompileLayout {
    size: checker
      .layout(during_compile)
      .map_or(0, |l| l.size as usize),
    ..oj_meta::DuringCompileLayout::default()
  };
  for (name, offset) in struct_members(checker, during_compile) {
    match name.as_str() {
      "do_output" => layout.do_output = Some(offset),
      "append_executable_filename_extension" => {
        layout.append_executable_filename_extension = Some(offset);
      }
      "output_executable_name" => layout.output_executable_name = Some(offset),
      "output_path" => layout.output_path = Some(offset),
      _ => {}
    }
  }
  layout
}

/// The members of a struct the checker laid out, by name and offset.
fn struct_members(
  checker: &mut oj_sema::Checker<'_>,
  type_id: oj_types::TypeId,
) -> Vec<(String, u64)> {
  let Some(definition) = checker.types().struct_of(type_id) else {
    return Vec::new();
  };
  checker
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
    .collect()
}

/// What a `set_build_options_dc` changes about a compilation that is already
/// running (**C§3.1**).
fn apply_during_compile(
  workspace: &oj_meta::Workspace,
  layout: &oj_meta::DuringCompileLayout,
  options: &mut BuildOptions,
) {
  if workspace.during_compile.is_none() {
    return;
  }
  if workspace.during_compile_bool(layout, |layout| layout.do_output) == Some(false) {
    options.output_type = oj_link::OutputType::NoOutput;
  }
  if let Some(append) =
    workspace.during_compile_bool(layout, |layout| layout.append_executable_filename_extension)
  {
    options.append_extension = append;
  }
  if let Some(name) =
    workspace.during_compile_string(layout, |layout| layout.output_executable_name)
    && !name.is_empty()
  {
    options.output_executable_name = Some(name);
  }
  if let Some(path) = workspace.during_compile_string(layout, |layout| layout.output_path)
    && !path.is_empty()
  {
    options.output_path = Some(PathBuf::from(path));
  }
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
    if label == "Error" {
      report.errors += 1;
    }
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
    let inner = match unsupported_target(workspace, &meta.build_options_layout) {
      Some(message) => Report::failure(message),
      None => {
        let (input, nested) = workspace_input(
          workspace,
          &meta.build_options_layout,
          &meta.during_compile_layout,
          outer,
          options,
        );
        run_input(&input, &nested, stage, None)
      }
    };
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
    3 => oj_link::OutputType::ObjectFile,
    4 => oj_link::OutputType::ObjectFile,
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
/// Builds the `Type_Info` image for a compilation a metaprogram is watching
/// and puts it where that metaprogram can read it (**C§5.3**).
///
/// A `Code_Node.type` has to be an address, and the table it points into
/// belongs to the workspace being compiled rather than to the metaprogram — so
/// the image is built here, in the compiler's own memory, and lives as long as
/// the storage the trees are exported into.
fn place_type_image(
  checker: &mut oj_sema::Checker<'_>,
  nodes: &std::rc::Rc<std::cell::RefCell<oj_meta::Nodes>>,
) -> Vec<(oj_types::TypeId, usize)> {
  let all: Vec<oj_types::TypeId> = (0..checker.types().len() as u32)
    .map(oj_types::TypeId)
    .collect();
  let image = oj_ir::TypeImage::build(checker, &all);
  let base = nodes.borrow_mut().place_image(&image.bytes);
  if base.is_null() {
    return Vec::new();
  }
  // SAFETY: the bytes were just copied into the arena, which outlives every
  // address handed out of them.
  unsafe { image.relocate(base) };
  image
    .offsets()
    .iter()
    .map(|(type_id, offset)| (*type_id, base as usize + *offset as usize))
    .collect()
}

/// The strings a metaprogram added to a scope of the compilation it is itself
/// part of, minus the ones a previous round already put there (**C§3.3**).
fn self_added_strings(
  meta: &oj_meta::Meta,
  root: &Path,
  already: &[oj_scope::AddedString],
) -> Vec<oj_scope::AddedString> {
  let directory = root
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_else(|| root.to_path_buf());
  let mut added: Vec<oj_scope::AddedString> = Vec::new();
  for workspace in &meta.workspaces {
    if !workspace.implicit {
      continue;
    }
    for scoped in &workspace.scoped_strings {
      let oj_meta::StringScope::File(path) = &scoped.target else {
        continue;
      };
      let target = oj_scope::StringTarget::File(path.clone());
      let seen = already
        .iter()
        .chain(added.iter())
        .any(|had| had.target == target && had.text == scoped.text);
      if seen {
        continue;
      }
      added.push(oj_scope::AddedString {
        target,
        path: directory.join(format!(
          ".self_string_{}.jai",
          already.len() + added.len() + 1
        )),
        text: scoped.text.clone(),
      });
    }
  }
  added
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

/// The distribution the standard modules come from: the `modules/` directory
/// `OJ_MODULES` names, else the one that sits beside the compiler's own binary
/// (`docs/spec.md` §5).
///
/// orangejuice ships its own modules, so this is the only distribution there
/// is; `Preload.jai` is what identifies a directory as one, since a program
/// cannot be compiled at all without it (**L§11.4**).
pub fn distribution() -> Option<PathBuf> {
  if let Some(value) = std::env::var_os("OJ_MODULES").filter(|value| !value.is_empty()) {
    let candidate = PathBuf::from(value);
    return candidate.join("Preload.jai").is_file().then(|| {
      candidate
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(candidate.clone())
    });
  }
  // Beside the binary for an installed compiler, further up for one being run
  // out of `target/release` in a checkout.
  let executable = std::env::current_exe().ok()?;
  executable.ancestors().find_map(|directory| {
    let candidate = directory.join("modules");
    candidate
      .join("Preload.jai")
      .is_file()
      .then(|| directory.to_path_buf())
  })
}

/// Wall-clock time per stage, printed to stderr when `OJ_TIMING` is set.
/// Compilation is one long chain of stages that hand work to one another, and
/// which of them a program spends its time in is not the same for a small
/// program as for a large one — so the answer has to be measured rather than
/// assumed.
pub(crate) struct Timing {
  enabled: bool,
  start: std::time::Instant,
  last: std::time::Instant,
}

impl Timing {
  pub(crate) fn new() -> Self {
    let now = std::time::Instant::now();
    Self {
      enabled: std::env::var_os("OJ_TIMING").is_some(),
      start: now,
      last: now,
    }
  }

  /// Records how long the stage that just finished took.
  pub(crate) fn mark(&mut self, stage: &str) {
    if !self.enabled {
      return;
    }
    let now = std::time::Instant::now();
    eprintln!(
      "oj timing  {stage:<22} {:>8.1}ms   (total {:>8.1}ms)",
      now.duration_since(self.last).as_secs_f64() * 1000.0,
      now.duration_since(self.start).as_secs_f64() * 1000.0,
    );
    self.last = now;
  }
}

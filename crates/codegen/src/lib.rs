//! LLVM code generation for orangejuice.
//!
//! [`compile`] turns an [`oj_ir::Program`] into an object file, LLVM IR or
//! target assembly through inkwell. Aggregates are byte arrays with an
//! explicit alignment, because every member access the front end resolved is
//! already a byte offset (**L§3.14**) — LLVM never has to agree with
//! `oj-types` about a layout.

mod debug;
mod emit;
mod machine;

use std::path::Path;

use inkwell::context::{Context, ContextRef};
use inkwell::module::Module;
use inkwell::targets::FileType;

pub use machine::{Bitcode, Options, PassOptions, default_triple, target_machine};

/// What a module is being built for.
///
/// The two differ in who owns the program's data: an executable defines its
/// globals and gets the generated `main` the C runtime calls, while a
/// compile-time module declares its globals and lets the JIT bind them to the
/// compiler's own segments (`docs/spec.md` §6.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Purpose {
  #[default]
  Executable,
  CompileTime,
}

/// What the caller wants out of one compilation.
#[derive(Clone, Debug)]
pub enum Output<'a> {
  /// An object file written to this path, in whatever format the target uses.
  Object(&'a Path),
  /// The textual LLVM IR of the module.
  LlvmIr,
  /// The target assembly of the module.
  Assembly,
}

/// Compiles a lowered program. Returns the listing for the textual outputs and
/// an empty string for an object file.
pub fn compile(
  program: &oj_ir::Program,
  options: &Options,
  output: Output<'_>,
) -> Result<String, String> {
  let timing = std::env::var_os("OJ_TIMING").is_some();
  let t0 = std::time::Instant::now();
  let machine = machine::target_machine(options)?;
  let context = Context::create();
  let module = build_module(
    context.create_module(&options.module_name),
    program,
    options,
  )?;
  let t1 = std::time::Instant::now();
  optimize(&module, &machine, options)?;
  let t2 = std::time::Instant::now();
  let module = &module;

  match output {
    Output::LlvmIr => Ok(module.print_to_string().to_string()),
    Output::Assembly => machine
      .write_to_memory_buffer(module, FileType::Assembly)
      .map(|buffer| String::from_utf8_lossy(buffer.as_slice()).into_owned())
      .map_err(|error| error.to_string()),
    Output::Object(path) => {
      let result = machine
        .write_to_file(module, FileType::Object, path)
        .map(|()| String::new())
        .map_err(|error| error.to_string());
      if timing {
        let t3 = std::time::Instant::now();
        eprintln!(
          "oj timing    build_module {:.1}ms  optimize {:.1}ms  emit {:.1}ms  (procedures {})",
          (t1 - t0).as_secs_f64() * 1000.0,
          (t2 - t1).as_secs_f64() * 1000.0,
          (t3 - t2).as_secs_f64() * 1000.0,
          program.procedures.len(),
        );
      }
      result
    }
  }
}

/// Runs the module through the pass pipeline
/// `Llvm_Options.bitcode_optimization_setting` asks for (**C§4**). At `O0`
/// there is nothing to run, which is what a debug build gets.
pub fn optimize(
  module: &Module<'_>,
  machine: &inkwell::targets::TargetMachine,
  options: &Options,
) -> Result<(), String> {
  let Some(pipeline) = options.bitcode.pipeline() else {
    return Ok(());
  };
  let passes = inkwell::passes::PassBuilderOptions::create();
  passes.set_loop_unrolling(options.passes.loop_unrolling);
  passes.set_loop_vectorization(options.passes.loop_vectorization);
  passes.set_loop_slp_vectorization(options.passes.slp_vectorization);
  passes.set_merge_functions(options.passes.merge_functions);
  module
    .run_passes(pipeline, machine, passes)
    .map_err(|error| error.to_string())
}

/// Builds a program into a module the caller supplied, which is how the JIT
/// gets one inside its own thread-safe context (`docs/spec.md` §6.5).
pub fn build_module<'ctx>(
  module: Module<'ctx>,
  program: &oj_ir::Program,
  options: &Options,
) -> Result<Module<'ctx>, String> {
  let machine = machine::target_machine(options)?;
  // The data layout has to be in place before anything is built: it is what
  // LLVM infers the alignment of every load and store from.
  module.set_triple(&machine.get_triple());
  module.set_data_layout(&machine.get_target_data().get_data_layout());
  let mut emitter = emit::Emitter::new(module, program, options.purpose, options);
  emitter.emit()?;
  Ok(emitter.into_module())
}

/// A borrowed context, for a caller that owns one LLVM context but not the
/// [`Context`] wrapper — the JIT's thread-safe context, for instance.
pub type Borrowed<'ctx> = ContextRef<'ctx>;

/// Compiles a program into one or more object files, built in parallel.
///
/// LLVM's own work — the pass pipeline, instruction selection, register
/// allocation, object emission — is most of a build's time, and all of it is
/// per-module: on a 12,000-line program it is nearly half of a debug build and
/// nine tenths of a release one. So the program is split across `units`
/// modules, each built on a thread of its own with a context of its own, and
/// the objects are linked together.
///
/// A split program cannot give anything internal linkage — a call may cross
/// units — and defines its globals and its entry point in the first unit only.
/// One unit is the whole program in one module, which is what a small program
/// and every textual output get.
pub fn compile_objects(
  program: &oj_ir::Program,
  options: &Options,
  object: &Path,
  units: usize,
) -> Result<Vec<std::path::PathBuf>, String> {
  let units = units.max(1);
  if units == 1 {
    compile(program, options, Output::Object(object))?;
    return Ok(vec![object.to_path_buf()]);
  }

  let paths: Vec<std::path::PathBuf> = (0..units)
    .map(|index| match index {
      // The first unit keeps the name the caller asked for, so a build that
      // stopped splitting still writes where it used to.
      0 => object.to_path_buf(),
      _ => object.with_extension(format!("{index}.o")),
    })
    .collect();

  let results: Vec<Result<(), String>> = std::thread::scope(|scope| {
    let handles: Vec<_> = paths
      .iter()
      .enumerate()
      .map(|(index, path)| {
        let mut unit = options.clone();
        unit.unit_index = index;
        unit.unit_count = units;
        scope.spawn(move || {
          // Each unit needs a context of its own: LLVM's are not shared
          // between threads.
          compile(program, &unit, Output::Object(path)).map(|_| ())
        })
      })
      .collect();
    handles
      .into_iter()
      .map(|handle| {
        handle
          .join()
          .unwrap_or_else(|_| Err(String::from("a codegen unit panicked")))
      })
      .collect()
  });

  for result in results {
    result?;
  }
  Ok(paths)
}

/// How many objects a program of this size is worth splitting across. Below a
/// few hundred procedures the threads cost more than they save, and past the
/// machine's parallelism they only make more objects to link.
pub fn default_units(program: &oj_ir::Program) -> usize {
  const PROCEDURES_PER_UNIT: usize = 128;
  if let Some(value) = std::env::var_os("OJ_CODEGEN_UNITS")
    && let Some(count) = value.to_str().and_then(|text| text.parse::<usize>().ok())
  {
    return count.max(1);
  }
  let bodies = program
    .procedures
    .iter()
    .filter(|procedure| procedure.has_body())
    .count();
  if bodies < 2 * PROCEDURES_PER_UNIT {
    return 1;
  }
  let cores = std::thread::available_parallelism()
    .map(std::num::NonZeroUsize::get)
    .unwrap_or(1);
  (bodies / PROCEDURES_PER_UNIT).min(cores).max(1)
}

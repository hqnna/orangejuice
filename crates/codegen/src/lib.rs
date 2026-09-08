//! LLVM code generation for orangejuice.
//!
//! [`compile`] turns an [`oj_ir::Program`] into an object file, LLVM IR or
//! target assembly through inkwell. Aggregates are byte arrays with an
//! explicit alignment, because every member access the front end resolved is
//! already a byte offset (**L§3.14**) — LLVM never has to agree with
//! `oj-types` about a layout.

mod emit;
mod machine;

use std::path::Path;

use inkwell::context::{Context, ContextRef};
use inkwell::module::Module;
use inkwell::targets::FileType;

pub use machine::{DEFAULT_TRIPLE, Options, target_machine};

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
  /// An ELF object written to this path.
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
  let machine = machine::target_machine(options)?;
  let context = Context::create();
  let module = build_module(
    context.create_module(&options.module_name),
    program,
    options,
  )?;
  let module = &module;

  match output {
    Output::LlvmIr => Ok(module.print_to_string().to_string()),
    Output::Assembly => machine
      .write_to_memory_buffer(module, FileType::Assembly)
      .map(|buffer| String::from_utf8_lossy(buffer.as_slice()).into_owned())
      .map_err(|error| error.to_string()),
    Output::Object(path) => machine
      .write_to_file(module, FileType::Object, path)
      .map(|()| String::new())
      .map_err(|error| error.to_string()),
  }
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
  let mut emitter = emit::Emitter::new(module, program, options.purpose);
  emitter.emit()?;
  Ok(emitter.into_module())
}

/// A borrowed context, for a caller that owns one LLVM context but not the
/// [`Context`] wrapper — the JIT's thread-safe context, for instance.
pub type Borrowed<'ctx> = ContextRef<'ctx>;

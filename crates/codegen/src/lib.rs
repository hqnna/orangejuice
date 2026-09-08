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

use inkwell::context::Context;
use inkwell::targets::FileType;

pub use machine::{DEFAULT_TRIPLE, Options};

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
  let mut emitter = emit::Emitter::new(&context, program, &options.module_name);
  // The data layout has to be in place before anything is built: it is what
  // LLVM infers the alignment of every load and store from.
  emitter.module().set_triple(&machine.get_triple());
  emitter
    .module()
    .set_data_layout(&machine.get_target_data().get_data_layout());
  emitter.emit()?;
  let module = emitter.module();

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

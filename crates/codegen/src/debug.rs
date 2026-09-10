//! DWARF debug information (**C§4**).
//!
//! `Build_Options.emit_debug_info` asks the back end to describe the program to
//! a debugger: the files it was written in, a subprogram per procedure, a line
//! for every instruction, and a variable per local. The types come out of
//! `oj-types` — the same table the front end laid the program out with — so
//! what a debugger prints is what the program says, even though the LLVM types
//! themselves are byte arrays (`docs/spec.md` §10).

use std::collections::HashMap;

use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::ContextRef;
use inkwell::debug_info::{
  AsDIScope, DIFile, DIFlags, DIFlagsConstants, DIScope, DISubprogram, DIType, DWARFEmissionKind,
  DWARFSourceLanguage, DebugInfoBuilder,
};
use inkwell::module::Module;
use inkwell::values::{FunctionValue, PointerValue};

use oj_ir::{DebugNames, Loc, Local, ProcId, Procedure, Program};
use oj_types::{ArrayKind, FloatKind, MemberFlags, TypeId, TypeKind, Types};

/// DWARF's `DW_ATE_*` encodings, which `create_basic_type` takes as a number.
const DW_ATE_ADDRESS: u32 = 0x01;
const DW_ATE_BOOLEAN: u32 = 0x02;
const DW_ATE_FLOAT: u32 = 0x04;
const DW_ATE_SIGNED: u32 = 0x05;
const DW_ATE_UNSIGNED: u32 = 0x07;

/// How deep a type is followed before it is described as opaque storage. A
/// struct that reaches itself through a pointer stops at the pointer, so this
/// is only about pathological nesting.
const MAX_TYPE_DEPTH: u32 = 12;

/// What describing a type needs: the table the front end laid the program out
/// with, and the names it used, which the back end has no interner for.
#[derive(Clone, Copy)]
pub struct Described<'p> {
  pub types: &'p Types,
  pub names: &'p DebugNames,
}

pub struct Debug<'ctx> {
  builder: DebugInfoBuilder<'ctx>,
  context: ContextRef<'ctx>,
  unit: DIScope<'ctx>,
  files: Vec<DIFile<'ctx>>,
  types: HashMap<TypeId, DIType<'ctx>>,
  /// The types being described right now, so that one naming itself stops.
  building: Vec<TypeId>,
  /// Each procedure's subprogram and the line it starts at.
  subprograms: HashMap<u32, (DISubprogram<'ctx>, u32)>,
  optimized: bool,
}

impl<'ctx> Debug<'ctx> {
  /// Starts a compile unit for the program, or `None` when there is nothing to
  /// describe it against — a program the lowering recorded no file for.
  pub fn new(module: &Module<'ctx>, program: &Program, optimized: bool) -> Option<Self> {
    let first = program.debug_files.first()?;
    let (directory, name) = split(first);
    let (builder, unit) = module.create_debug_info_builder(
      true,
      // Jai is not one of DWARF's languages; C is what a debugger treats a
      // flat, pointer-and-struct program as (`docs/spec.md` §10).
      DWARFSourceLanguage::C,
      &name,
      &directory,
      concat!("orangejuice ", env!("CARGO_PKG_VERSION")),
      optimized,
      "",
      0,
      "",
      DWARFEmissionKind::Full,
      0,
      false,
      false,
      "",
      "",
    );
    let context = module.get_context();
    let files = program
      .debug_files
      .iter()
      .map(|path| {
        let (directory, name) = split(path);
        builder.create_file(&name, &directory)
      })
      .collect();
    module.add_basic_value_flag(
      "Debug Info Version",
      inkwell::module::FlagBehavior::Warning,
      context.i32_type().const_int(3, false),
    );
    module.add_basic_value_flag(
      "Dwarf Version",
      inkwell::module::FlagBehavior::Warning,
      context.i32_type().const_int(4, false),
    );
    Some(Self {
      builder,
      context,
      unit: unit.as_debug_info_scope(),
      files,
      types: HashMap::new(),
      building: Vec::new(),
      subprograms: HashMap::new(),
      optimized,
    })
  }

  fn file(&self, loc: Option<Loc>) -> DIFile<'ctx> {
    loc
      .and_then(|loc| self.files.get(loc.file as usize).copied())
      .or_else(|| self.files.first().copied())
      .expect("a compile unit was made from at least one file")
  }

  /// Describes one procedure, and gives its LLVM function the subprogram.
  pub fn declare_function(
    &mut self,
    described: Described<'_>,
    id: ProcId,
    procedure: &Procedure,
    function: FunctionValue<'ctx>,
  ) {
    let file = self.file(procedure.location);
    let line = procedure.location.map_or(1, |loc| loc.line);
    let signature = described.types.procedure_of(procedure.type_id).cloned();
    let returns = signature
      .as_ref()
      .and_then(|signature| signature.returns.first().copied())
      .map(|type_id| self.type_of(described, type_id, 0));
    let parameters: Vec<DIType<'ctx>> = signature
      .as_ref()
      .map(|signature| {
        signature
          .arguments
          .iter()
          .map(|type_id| self.type_of(described, *type_id, 0))
          .collect()
      })
      .unwrap_or_default();
    let subroutine =
      self
        .builder
        .create_subroutine_type(file, returns, &parameters, DIFlags::PUBLIC);
    let subprogram = self.builder.create_function(
      self.unit,
      &procedure.name,
      Some(&procedure.symbol),
      file,
      line,
      subroutine,
      false,
      true,
      line,
      DIFlags::PUBLIC,
      self.optimized,
    );
    function.set_subprogram(subprogram);
    self.subprograms.insert(id.0, (subprogram, line));
  }

  /// Attaches a line to whatever the builder emits next.
  pub fn locate(&self, builder: &Builder<'ctx>, id: ProcId, loc: Option<Loc>) {
    let Some((subprogram, start)) = self.subprograms.get(&id.0).copied() else {
      return;
    };
    // An instruction the lowering recorded no place for still needs a line:
    // LLVM rejects a call in a function that has debug info without one.
    let (line, column) = match loc {
      Some(loc) => (loc.line, loc.column),
      None => (start, 0),
    };
    let location = self.builder.create_debug_location(
      self.context,
      line,
      column,
      subprogram.as_debug_info_scope(),
      None,
    );
    builder.set_current_debug_location(location);
  }

  /// Describes one local, so a debugger can name the storage it was given.
  pub fn declare_local(
    &mut self,
    described: Described<'_>,
    id: ProcId,
    local: &Local,
    storage: PointerValue<'ctx>,
    block: BasicBlock<'ctx>,
  ) {
    let Some((subprogram, start)) = self.subprograms.get(&id.0).copied() else {
      return;
    };
    if local.name.is_empty() {
      return;
    }
    let file = self.file(local.location);
    let line = local.location.map_or(start, |loc| loc.line);
    let type_id = self.type_of(described, local.type_id, 0);
    let scope = subprogram.as_debug_info_scope();
    let variable = match local.parameter {
      Some(number) => self.builder.create_parameter_variable(
        scope,
        &local.name,
        number,
        file,
        line,
        type_id,
        true,
        DIFlags::ZERO,
      ),
      None => self.builder.create_auto_variable(
        scope,
        &local.name,
        file,
        line,
        type_id,
        true,
        DIFlags::ZERO,
        local.alignment as u32 * 8,
      ),
    };
    let location = self
      .builder
      .create_debug_location(self.context, line, 0, scope, None);
    // LLVM 19 records a declaration rather than emitting an intrinsic call, so
    // what comes back is a `DbgRecord` and not a value; inkwell's wrapper
    // insists on the latter, so the record is made through the C API directly
    // and its handle dropped.
    let expression = self.builder.create_expression(Vec::new());
    unsafe {
      inkwell::llvm_sys::debuginfo::LLVMDIBuilderInsertDeclareRecordAtEnd(
        self.builder.as_mut_ptr(),
        inkwell::values::AsValueRef::as_value_ref(&storage),
        variable.as_mut_ptr(),
        expression.as_mut_ptr(),
        location.as_mut_ptr(),
        block.as_mut_ptr(),
      );
    }
  }

  pub fn finalize(&self) {
    self.builder.finalize();
  }

  // -------------------------------------------------------------- types ------

  /// The DWARF description of one of the front end's types (**L§3**).
  fn type_of(&mut self, described: Described<'_>, type_id: TypeId, depth: u32) -> DIType<'ctx> {
    if let Some(already) = self.types.get(&type_id) {
      return *already;
    }
    if depth > MAX_TYPE_DEPTH || self.building.contains(&type_id) {
      return self.opaque(described, type_id);
    }
    self.building.push(type_id);
    let description = self.describe(described, type_id, depth);
    self.building.pop();
    self.types.insert(type_id, description);
    description
  }

  fn describe(&mut self, described: Described<'_>, type_id: TypeId, depth: u32) -> DIType<'ctx> {
    let types = described.types;
    let underlying = types.underlying(type_id);
    let name = String::from(described.names.type_name(type_id));
    let bits = types.size_of(underlying).unwrap_or(0) * 8;
    match types.kind(underlying).clone() {
      TypeKind::Bool => self.basic(&name, 8, DW_ATE_BOOLEAN),
      TypeKind::Integer(kind) => {
        let encoding = match kind.is_signed() {
          true => DW_ATE_SIGNED,
          false => DW_ATE_UNSIGNED,
        };
        self.basic(&name, bits.max(8), encoding)
      }
      TypeKind::UntypedInt => self.basic("int", 64, DW_ATE_SIGNED),
      TypeKind::Float(FloatKind::F32) | TypeKind::UntypedFloat(FloatKind::F32) => {
        self.basic(&name, 32, DW_ATE_FLOAT)
      }
      TypeKind::Float(FloatKind::F64) | TypeKind::UntypedFloat(FloatKind::F64) => {
        self.basic(&name, 64, DW_ATE_FLOAT)
      }
      // A `Type` is the address of its record in the type table and a `Code` a
      // handle the compiler hands out (**L§13.1**, **L§17**): both are one
      // pointer-sized number as far as a debugger is concerned.
      TypeKind::Type | TypeKind::Code => self.basic(&name, 64, DW_ATE_ADDRESS),
      TypeKind::Void => self.basic("void", 8, DW_ATE_UNSIGNED),
      TypeKind::Enum(id) => {
        let base = types.enum_info(id).base;
        let encoding = match types.kind(types.underlying(base)) {
          TypeKind::Integer(kind) if !kind.is_signed() => DW_ATE_UNSIGNED,
          _ => DW_ATE_SIGNED,
        };
        self.basic(&name, bits.max(8), encoding)
      }
      TypeKind::Pointer(pointee) => {
        let inner = self.type_of(described, pointee, depth + 1);
        self
          .builder
          .create_pointer_type(&name, inner, 64, 64, AddressSpace::default())
          .as_type()
      }
      TypeKind::Procedure(_) => self.basic(&name, 64, DW_ATE_ADDRESS),
      TypeKind::Array {
        element,
        kind: ArrayKind::Fixed(count),
      } => {
        let inner = self.type_of(described, element, depth + 1);
        let stride = types.size_of(element).unwrap_or(1).max(1);
        let alignment = types.align_of(underlying).unwrap_or(1).max(1) as u32 * 8;
        self
          .builder
          .create_array_type(
            inner,
            stride * count * 8,
            alignment,
            std::slice::from_ref(&(0..count as i64)),
          )
          .as_type()
      }
      // A view, a dynamic array, a string and an `Any` are all structs the
      // front end laid out (**L§3.3**, **L§3.8**), so they are described as
      // whatever the table says they hold.
      _ => self.aggregate(described, type_id, depth),
    }
  }

  /// A struct, described member by member so that a debugger can walk it.
  fn aggregate(&mut self, described: Described<'_>, type_id: TypeId, depth: u32) -> DIType<'ctx> {
    let types = described.types;
    let underlying = types.underlying(type_id);
    let Some(definition) = types.struct_of(underlying) else {
      return self.opaque(described, type_id);
    };
    let name = String::from(described.names.type_name(type_id));
    let size = types.size_of(underlying).unwrap_or(0) * 8;
    let alignment = types.align_of(underlying).unwrap_or(1).max(1) as u32 * 8;
    let members: Vec<(String, TypeId, u64)> = types
      .struct_info(definition)
      .members
      .iter()
      .enumerate()
      .filter(|(_, member)| {
        member.imported_through.is_none()
          && !member
            .flags
            .intersects(MemberFlags::CONSTANT | MemberFlags::IMPORTED)
      })
      .map(|(index, member)| {
        (
          String::from(described.names.member_name(definition, index)),
          member.type_id,
          member.offset,
        )
      })
      .collect();
    let file = self.files.first().copied().expect("at least one file");
    let mut described_members = Vec::with_capacity(members.len());
    for (member_name, member_type, offset) in members {
      let inner = self.type_of(described, member_type, depth + 1);
      let member_size = types.size_of(member_type).unwrap_or(0) * 8;
      let member_align = types.align_of(member_type).unwrap_or(1).max(1) as u32 * 8;
      described_members.push(
        self
          .builder
          .create_member_type(
            self.unit,
            &member_name,
            file,
            0,
            member_size,
            member_align,
            offset * 8,
            DIFlags::ZERO,
            inner,
          )
          .as_type(),
      );
    }
    self
      .builder
      .create_struct_type(
        self.unit,
        &name,
        file,
        0,
        size,
        alignment,
        DIFlags::ZERO,
        None,
        &described_members,
        0,
        None,
        &name,
      )
      .as_type()
  }

  fn basic(&mut self, name: &str, bits: u64, encoding: u32) -> DIType<'ctx> {
    let name = match name.is_empty() {
      true => "unnamed",
      false => name,
    };
    match self
      .builder
      .create_basic_type(name, bits, encoding, DIFlags::ZERO)
    {
      Ok(basic) => basic.as_type(),
      Err(_) => self.byte(),
    }
  }

  fn byte(&self) -> DIType<'ctx> {
    self
      .builder
      .create_basic_type("byte", 8, DW_ATE_UNSIGNED, DIFlags::ZERO)
      .expect("a one-byte type is describable")
      .as_type()
  }

  /// Storage of the right size with nothing said about what is in it, for a
  /// type DWARF has no shape for.
  fn opaque(&mut self, described: Described<'_>, type_id: TypeId) -> DIType<'ctx> {
    let size = described
      .types
      .size_of(described.types.underlying(type_id))
      .unwrap_or(1)
      .max(1);
    let byte = self.byte();
    self
      .builder
      .create_array_type(byte, size * 8, 8, std::slice::from_ref(&(0..size as i64)))
      .as_type()
  }
}

/// A path split into the directory a compile unit records and the file name.
fn split(path: &str) -> (String, String) {
  let path = std::path::Path::new(path);
  let directory = path
    .parent()
    .map(|parent| parent.to_string_lossy().into_owned())
    .unwrap_or_default();
  let name = path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| path.to_string_lossy().into_owned());
  (directory, name)
}

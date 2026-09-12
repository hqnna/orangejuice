//! Lowering `oj-ir` into an LLVM module.
//!
//! Aggregates are byte arrays here: every member access the front end resolved
//! is already a byte offset (**L§3.14**), so the back end never has to make
//! LLVM agree with `oj-types` about a struct layout — it only has to keep the
//! size and the alignment.

use std::collections::HashMap;

use inkwell::GlobalVisibility;
use inkwell::attributes::AttributeLoc;
use inkwell::builder::Builder;
use inkwell::context::ContextRef;
use inkwell::module::{Linkage, Module};
use inkwell::types::{AnyType, BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{
  BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, GlobalValue, IntValue,
  PointerValue,
};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate};

use oj_ir::{
  BinaryOp, Callee, ConstLink, Constant, ConvertKind, GlobalInit, Inst, ParameterKind, ProcId,
  Procedure, ProcedureFlags, Program, Terminator, UnaryOp,
};
use oj_types::{Classification, Eightbyte, FloatKind, TypeId, TypeKind, Types};

/// The name the C runtime calls. orangejuice emits its own, which sets up a
/// zeroed `#Context` and calls the program's `main`; the `Runtime_Support`
/// entry point that would normally own this symbol sets temporary storage up
/// through `initializer_of` and a `push_context` (`docs/spec.md` §10).
const ENTRY_SYMBOL: &str = "main";

/// A pointer on the one target orangejuice has (`docs/spec.md` §2).
const POINTER_SIZE: usize = 8;

pub struct Emitter<'ctx, 'p> {
  context: ContextRef<'ctx>,
  module: Module<'ctx>,
  builder: Builder<'ctx>,
  program: &'p Program,
  /// What the module is for: an executable defines its globals and gets a
  /// generated `main`; a compile-time module leaves both to the JIT
  /// (`docs/spec.md` §6.5).
  purpose: crate::Purpose,
  functions: Vec<FunctionValue<'ctx>>,
  globals: Vec<GlobalValue<'ctx>>,
  strings: HashMap<Box<[u8]>, GlobalValue<'ctx>>,
  /// How many aggregate constants have been laid down, so each gets a name.
  constants: usize,
  /// The DWARF description of the program, when one was asked for (**C§4**).
  debug: Option<crate::debug::Debug<'ctx>>,
  /// Which of the program's codegen units this module is. A split program
  /// defines each procedure's body in exactly one of them and declares it in
  /// the rest, so nothing may keep internal linkage.
  unit: crate::Options,
}

impl<'ctx, 'p> Emitter<'ctx, 'p> {
  pub fn new(
    module: Module<'ctx>,
    program: &'p Program,
    purpose: crate::Purpose,
    options: &crate::Options,
  ) -> Self {
    let context = module.get_context();
    // Compile-time code is thrown away when the compilation ends, so only the
    // executable is described (`docs/spec.md` §6.5).
    let debug = (options.debug_info && purpose == crate::Purpose::Executable)
      .then(|| crate::debug::Debug::new(&module, program, options.bitcode != crate::Bitcode::O0))
      .flatten();
    Self {
      builder: context.create_builder(),
      context,
      module,
      program,
      purpose,
      functions: Vec::new(),
      globals: Vec::new(),
      strings: HashMap::new(),
      constants: 0,
      debug,
      unit: options.clone(),
    }
  }

  pub fn into_module(self) -> Module<'ctx> {
    self.module
  }

  fn types(&self) -> &'p Types {
    &self.program.types
  }

  pub fn emit(&mut self) -> Result<(), String> {
    // Functions first: a global's own bytes may hold a procedure's address —
    // a struct record's `initializer` in the type table image (**L§17**), or a
    // constant array of procedures — so the declarations have to exist before
    // the initializer that names them is built.
    self.declare_functions();
    self.declare_globals();
    if self.debug.is_some() {
      for (index, procedure) in self.program.procedures.iter().enumerate() {
        if !procedure.has_body() || !self.unit.owns_procedure(index) {
          continue;
        }
        let function = self.functions[index];
        let described = crate::debug::Described {
          types: &self.program.types,
          names: &self.program.names,
        };
        if let Some(debug) = self.debug.as_mut() {
          debug.declare_function(described, ProcId(index as u32), procedure, function);
        }
      }
    }
    for (index, procedure) in self.program.procedures.iter().enumerate() {
      if !procedure.has_body() || !self.unit.owns_procedure(index) {
        continue;
      }
      self.emit_body(ProcId(index as u32), procedure)?;
    }
    if self.purpose == crate::Purpose::Executable && self.unit.is_primary_unit() {
      self.emit_entry_point()?;
    }
    if let Some(debug) = self.debug.as_ref() {
      debug.finalize();
    }
    self
      .module
      .verify()
      .map_err(|error| format!("the generated module is not valid:\n{}", error.to_string()))
  }

  // ------------------------------------------------------------- types ------

  fn llvm_type(&self, type_id: TypeId) -> BasicTypeEnum<'ctx> {
    let types = self.types();
    let underlying = types.underlying(type_id);
    match types.kind(underlying) {
      TypeKind::Bool => self.context.i8_type().into(),
      TypeKind::Integer(kind) => self.integer_type(kind.size() * 8),
      TypeKind::Float(FloatKind::F32) | TypeKind::UntypedFloat(FloatKind::F32) => {
        self.context.f32_type().into()
      }
      TypeKind::Float(FloatKind::F64) | TypeKind::UntypedFloat(FloatKind::F64) => {
        self.context.f64_type().into()
      }
      TypeKind::UntypedInt => self.context.i64_type().into(),
      TypeKind::Pointer(_) | TypeKind::Procedure(_) | TypeKind::Type | TypeKind::Code => {
        self.context.ptr_type(AddressSpace::default()).into()
      }
      TypeKind::Enum(id) => self.llvm_type(types.enum_info(*id).base),
      // The vector register `#asm` works in, which is a value LLVM can bind to
      // an `xmm` constraint rather than storage it can only address
      // (**L§3.1**, **L§15**).
      TypeKind::V128 => self.context.i64_type().vec_type(2).into(),
      // Everything else is storage of a known size, addressed by byte offset.
      _ => {
        let size = types.size_of(underlying).unwrap_or(0);
        self.context.i8_type().array_type(size as u32).into()
      }
    }
  }

  fn integer_type(&self, bits: u64) -> BasicTypeEnum<'ctx> {
    let bits = std::num::NonZeroU32::new(bits as u32).unwrap_or(std::num::NonZeroU32::MIN);
    match self.context.custom_width_int_type(bits) {
      Ok(int) => int.into(),
      Err(_) => self.context.i64_type().into(),
    }
  }

  /// The LLVM types one eightbyte of a classified value travels in
  /// (**L§7.11**): a general-purpose register takes an integer as wide as what
  /// is left, a vector register takes a float or two.
  fn eightbyte_types(&self, type_id: TypeId, classes: &[Eightbyte]) -> Vec<BasicTypeEnum<'ctx>> {
    let size = self.types().size_of(type_id).unwrap_or(0);
    classes
      .iter()
      .enumerate()
      .map(|(index, class)| {
        let width = oj_types::eightbyte_size(size, index);
        match class {
          Eightbyte::Sse if width > 4 => self.context.f64_type().into(),
          Eightbyte::Sse => self.context.f32_type().into(),
          Eightbyte::Integer => self.integer_type(width * 8),
        }
      })
      .collect()
  }

  /// The LLVM values a classified one travels as, and where in its storage
  /// each of them sits (**L§7.11**). `None` is a value that goes through
  /// memory and is addressed rather than copied into registers.
  ///
  /// A value in general-purpose registers is one LLVM value per word,
  /// whichever target it is: as wide as what is left of the aggregate, so that
  /// loading and storing one touches exactly the bytes that are there. A
  /// homogeneous float aggregate is one coerced array instead, which is what
  /// tells the AArch64 back end to take that many consecutive vector registers
  /// rather than spreading the elements over both register files.
  fn register_parts(
    &self,
    type_id: TypeId,
    class: &Classification,
  ) -> Option<Vec<(BasicTypeEnum<'ctx>, u64)>> {
    match class {
      Classification::Memory => None,
      Classification::Registers(classes) => Some(
        self
          .eightbyte_types(type_id, classes)
          .into_iter()
          .enumerate()
          .map(|(index, part)| (part, index as u64 * 8))
          .collect(),
      ),
      // `classify` only answers this for an aggregate whose size is exactly
      // its elements, so the one part covers the storage and no more.
      Classification::FloatRegisters { kind, count } => {
        let element = match kind {
          oj_types::FloatKind::F32 => self.context.f32_type(),
          oj_types::FloatKind::F64 => self.context.f64_type(),
        };
        let part: BasicTypeEnum<'ctx> = match count {
          1 => element.into(),
          _ => element.array_type(*count as u32).into(),
        };
        Some(vec![(part, 0)])
      }
    }
  }

  /// What a classified value is when it travels as one LLVM value: the one
  /// part it has, or a struct of the several System V split it into.
  fn coerced_type(&self, type_id: TypeId, class: &Classification) -> Option<BasicTypeEnum<'ctx>> {
    let parts = self.register_parts(type_id, class)?;
    match parts.len() {
      1 => Some(parts[0].0),
      _ => {
        let parts: Vec<BasicTypeEnum<'ctx>> = parts.into_iter().map(|(part, _)| part).collect();
        Some(self.context.struct_type(&parts, false).into())
      }
    }
  }

  fn target(&self) -> oj_types::Target {
    self.types().target()
  }

  /// `signext`/`zeroext` for a value narrower than a word, where the platform
  /// makes widening the caller's job. Darwin's AArch64 ABI does — a callee
  /// there may read the whole register — where System V leaves the high bits
  /// unspecified and every callee re-narrows. LLVM only widens when the
  /// attribute says to, so this is what a `#c_call` of a `u8` needs to arrive
  /// as the C side expects.
  fn extension_attribute(&self, type_id: TypeId) -> Option<inkwell::attributes::Attribute> {
    if !self.target().extends_narrow_arguments() {
      return None;
    }
    let underlying = self.types().underlying(type_id);
    let name = match self.types().kind(underlying) {
      TypeKind::Bool => "zeroext",
      TypeKind::Integer(kind) if kind.size() < 4 => match kind.is_signed() {
        true => "signext",
        false => "zeroext",
      },
      TypeKind::Enum(id) => {
        let base = self.types().enum_info(*id).base;
        return self.extension_attribute(base);
      }
      _ => return None,
    };
    let kind = inkwell::attributes::Attribute::get_named_enum_kind_id(name);
    Some(self.context.create_enum_attribute(kind, 0))
  }

  fn function_type(&self, abi: &oj_ir::Abi) -> FunctionType<'ctx> {
    let pointer = self.context.ptr_type(AddressSpace::default());
    let mut parameters: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = Vec::new();
    for parameter in &abi.parameters {
      // A `#c_call`'s aggregate return comes back in registers when it is
      // small enough, and then the storage the IR set aside is the caller's
      // own business rather than a parameter (**L§7.11**).
      if parameter.kind == ParameterKind::ReturnPointer
        && abi
          .return_class
          .as_ref()
          .is_some_and(Classification::in_registers)
        && parameters.is_empty()
      {
        continue;
      }
      let parts = parameter
        .class
        .as_ref()
        .and_then(|class| self.register_parts(parameter.type_id, class));
      match (parameter.kind, parts) {
        (ParameterKind::Value, _) => parameters.push(self.llvm_type(parameter.type_id).into()),
        (ParameterKind::Pointer, Some(parts)) => {
          for (part, _) in parts {
            parameters.push(part.into());
          }
        }
        _ => parameters.push(pointer.into()),
      }
    }
    // A `#foreign` header whose last parameter is `..Any` is a C-variadic
    // procedure, and LLVM has to be told so for a call to be built the way the
    // platform passes one (**L§7.11**, **L§12.2**).
    let variadic = abi.variadic;
    match (&abi.return_class, abi.direct_return) {
      (Some(class), _) if class.in_registers() => {
        let type_id = abi
          .parameters
          .first()
          .map(|parameter| parameter.type_id)
          .unwrap_or(TypeId::VOID);
        match self.coerced_type(type_id, class) {
          Some(coerced) => coerced.fn_type(&parameters, variadic),
          None => self.context.void_type().fn_type(&parameters, variadic),
        }
      }
      (_, Some(type_id)) => self.llvm_type(type_id).fn_type(&parameters, variadic),
      _ => self.context.void_type().fn_type(&parameters, variadic),
    }
  }

  // ------------------------------------------------------ declarations ------

  fn declare_globals(&mut self) {
    // At compile time a global is storage the compiler owns and the JIT binds
    // by symbol, so the module only declares it (`docs/spec.md` §6.5).
    if self.purpose == crate::Purpose::CompileTime {
      for global in &self.program.globals {
        let storage = self.context.i8_type().array_type(global.size.max(1) as u32);
        let value = self
          .module
          .add_global(storage, Some(AddressSpace::default()), &global.symbol);
        value.set_alignment(global.alignment as u32);
        value.set_linkage(Linkage::External);
        self.globals.push(value);
      }
      return;
    }
    // A split program defines its globals once, in the primary unit; the rest
    // declare them, the way a compile-time module does.
    if !self.unit.is_primary_unit() {
      for global in &self.program.globals {
        let storage = self.context.i8_type().array_type(global.size.max(1) as u32);
        let value = self
          .module
          .add_global(storage, Some(AddressSpace::default()), &global.symbol);
        value.set_alignment(global.alignment as u32);
        value.set_linkage(Linkage::External);
        if !global.external && !global.imported {
          value.set_visibility(GlobalVisibility::Hidden);
        }
        self.globals.push(value);
      }
      return;
    }
    for global in &self.program.globals {
      // The type table points at itself, so it has to exist before its own
      // initializer can be written (**L§17**).
      if let GlobalInit::Image { bytes, relocations } = &global.init {
        let value = self.image_global(&global.symbol, global.alignment as u32, bytes, relocations);
        self.globals.push(value);
        continue;
      }
      let (llvm_type, initializer) = match &global.init {
        GlobalInit::Image { .. } => unreachable!("handled above"),
        GlobalInit::Zero => {
          let storage = self.context.i8_type().array_type(global.size as u32);
          (
            BasicTypeEnum::from(storage),
            Some(BasicValueEnum::from(storage.const_zero())),
          )
        }
        GlobalInit::Constant(value) => {
          let llvm_type = self.llvm_type(global.type_id);
          match self.constant_data(value, global.type_id) {
            Some(data) => (data.get_type(), Some(data)),
            None => (llvm_type, Some(llvm_type.const_zero())),
          }
        }
        // `#no_reset`: the bytes compile time left in the global are what the
        // program starts with (**L§12.3**).
        GlobalInit::Bytes(bytes) => {
          let data = self.context.const_string(bytes, false);
          (
            BasicTypeEnum::from(data.get_type()),
            Some(BasicValueEnum::from(data)),
          )
        }
      };
      let value = self
        .module
        .add_global(llvm_type, Some(AddressSpace::default()), &global.symbol);
      value.set_alignment(global.alignment as u32);
      if global.imported {
        value.set_linkage(Linkage::External);
      } else {
        if let Some(initializer) = initializer {
          value.set_initializer(&initializer);
        }
        if global.external {
          value.set_linkage(Linkage::External);
        } else if self.unit.is_split() {
          value.set_linkage(Linkage::External);
          value.set_visibility(GlobalVisibility::Hidden);
        } else {
          value.set_linkage(Linkage::Internal);
        }
      }
      self.globals.push(value);
    }
  }

  fn declare_functions(&mut self) {
    for (index, procedure) in self.program.procedures.iter().enumerate() {
      let function_type = self.function_type(&procedure.abi);
      // Two modules may declare the same `#foreign` procedure — Runtime_Support
      // and the program both reach `write(2)` — and they name one symbol, so
      // the module declares it once (**L§12.2**).
      let existing = (!procedure.has_body())
        .then(|| self.module.get_function(&procedure.symbol))
        .flatten()
        .filter(|function| function.count_basic_blocks() == 0);
      if let Some(function) = existing {
        self.functions.push(function);
        continue;
      }
      let function = self
        .module
        .add_function(&procedure.symbol, function_type, None);
      // A compile-time module shares its JIT dylib with every other one, so
      // only the procedure the run was built for and the ones this module is
      // the first to reach may claim a public symbol; the rest are private
      // copies (`docs/spec.md` §6.5).
      let private = match self.purpose {
        crate::Purpose::CompileTime => {
          self.program.entry != Some(ProcId(index as u32))
            && !procedure.flags.contains(ProcedureFlags::SHARED)
        }
        crate::Purpose::Executable => !procedure.flags.contains(ProcedureFlags::EXPORT),
      };
      if procedure.has_body() && private {
        // A split program cannot give a procedure internal linkage — a call
        // may cross units — so it gets external linkage and *hidden*
        // visibility instead: a sibling object can call it, and it never
        // reaches `.dynsym`. Without the visibility, `-export-dynamic` would
        // publish every internal name the program has, and one of them
        // interposing on a libc symbol is enough to break the program before
        // it starts — our `errno` wrapper over glibc's thread-local `errno`
        // did exactly that.
        match self.unit.is_split() {
          true => function
            .as_global_value()
            .set_visibility(GlobalVisibility::Hidden),
          false => function.set_linkage(Linkage::Internal),
        }
      }
      for (position, attribute) in self.memory_attributes(&procedure.abi) {
        function.add_attribute(AttributeLoc::Param(position), attribute);
      }
      if let Some(attribute) = self.frame_pointer_attribute() {
        function.add_attribute(AttributeLoc::Function, attribute);
      }
      self.functions.push(function);
    }
  }
  /// Apple's arm64 ABI requires every procedure to keep a valid frame record,
  /// so that a walker — a crash handler's, a profiler's, a debugger's — can
  /// follow the chain from any instruction. LLVM omits the frame pointer
  /// where it can unless the function says otherwise, which is what this says.
  fn frame_pointer_attribute(&self) -> Option<inkwell::attributes::Attribute> {
    if !self.target().is_darwin() {
      return None;
    }
    Some(self.context.create_string_attribute("frame-pointer", "all"))
  }

  /// The attributes a `#c_call` needs on the parameters the C convention
  /// passes through memory: `byval` copies an argument into the caller's
  /// argument area, `sret` says the first parameter is where the return goes
  /// (**L§7.11**).
  fn memory_attributes(&self, abi: &oj_ir::Abi) -> Vec<(u32, inkwell::attributes::Attribute)> {
    let mut attributes = Vec::new();
    let mut position = 0u32;
    for parameter in &abi.parameters {
      if abi.c_call
        && parameter.kind == ParameterKind::Value
        && let Some(attribute) = self.extension_attribute(parameter.type_id)
      {
        attributes.push((position, attribute));
      }
      let parts = parameter
        .class
        .as_ref()
        .and_then(|class| self.register_parts(parameter.type_id, class));
      match (parameter.kind, parts) {
        (ParameterKind::ReturnPointer, _)
          if position == 0 && abi.return_class == Some(Classification::Memory) =>
        {
          attributes.push((position, self.type_attribute("sret", parameter.type_id)));
        }
        (ParameterKind::ReturnPointer, _)
          if position == 0
            && abi
              .return_class
              .as_ref()
              .is_some_and(Classification::in_registers) =>
        {
          // The return travels in registers, so this is not a parameter at all.
          continue;
        }
        (ParameterKind::Pointer, Some(parts)) => {
          position += parts.len() as u32;
          continue;
        }
        // System V copies a memory-class argument into the caller's argument
        // area, which is what `byval` asks LLVM for. AAPCS64 does not: it
        // passes the address of a copy, which is the pointer the IR already
        // hands over, so asking for `byval` there would put the bytes on the
        // stack where the callee expects a pointer.
        (ParameterKind::Pointer, None)
          if parameter.class.is_some() && !self.target().is_aarch64() =>
        {
          attributes.push((position, self.type_attribute("byval", parameter.type_id)));
        }
        _ => {}
      }
      position += 1;
    }
    attributes
  }

  fn type_attribute(&self, name: &str, type_id: TypeId) -> inkwell::attributes::Attribute {
    let kind = inkwell::attributes::Attribute::get_named_enum_kind_id(name);
    self
      .context
      .create_type_attribute(kind, self.llvm_type(type_id).as_any_type_enum())
  }

  /// The data a constant global starts out as. A `string` is the two words a
  /// view is, pointing at a private byte array (**L§3.4**).
  fn constant_data(&mut self, value: &Constant, type_id: TypeId) -> Option<BasicValueEnum<'ctx>> {
    match value {
      Constant::String(text) => {
        let data = self.string_bytes(text);
        let count = self.context.i64_type().const_int(text.len() as u64, false);
        Some(
          self
            .context
            .const_struct(&[count.into(), data.as_pointer_value().into()], false)
            .into(),
        )
      }
      // As a *value* the bytes stand on their own; a pointer among them needs
      // an address, which only the data form has.
      Constant::Bytes { bytes, .. } => Some(self.context.const_string(bytes, false).into()),
      Constant::Zero => Some(self.llvm_type(type_id).const_zero()),
      _ => {
        let llvm_type = self.llvm_type(type_id);
        self.scalar_constant(value, llvm_type)
      }
    }
  }

  fn scalar_constant(
    &self,
    value: &Constant,
    llvm_type: BasicTypeEnum<'ctx>,
  ) -> Option<BasicValueEnum<'ctx>> {
    Some(match (value, llvm_type) {
      (Constant::Int(number), BasicTypeEnum::IntType(int)) => {
        int.const_int(*number as u64, false).into()
      }
      (Constant::Bool(flag), BasicTypeEnum::IntType(int)) => {
        int.const_int(u64::from(*flag), false).into()
      }
      (Constant::Float(number), BasicTypeEnum::FloatType(float)) => {
        float.const_float(*number).into()
      }
      (Constant::Int(number), BasicTypeEnum::FloatType(float)) => {
        float.const_float(*number as f64).into()
      }
      (Constant::Null, BasicTypeEnum::PointerType(pointer)) => pointer.const_null().into(),
      (Constant::Int(0), BasicTypeEnum::PointerType(pointer)) => pointer.const_null().into(),
      // A compile-time address: the `Type_Info` a `Type` names, or the
      // `Code_Node` a `Code` does (**L§3.10**, **L§13.1**).
      (Constant::Int(number), BasicTypeEnum::PointerType(pointer)) => self
        .context
        .i64_type()
        .const_int(*number as u64, false)
        .const_to_pointer(pointer)
        .into(),
      (Constant::Zero, _) => llvm_type.const_zero(),
      _ => return None,
    })
  }

  fn string_bytes(&mut self, text: &[u8]) -> GlobalValue<'ctx> {
    if let Some(existing) = self.strings.get(text) {
      return *existing;
    }
    // A literal's bytes carry a trailing zero the count does not include, so
    // that one handed to a C procedure is the string it expects (**L§3.4**).
    let data = self.context.const_string(text, true);
    let global = self.module.add_global(
      data.get_type(),
      Some(AddressSpace::default()),
      &format!(".str.{}", self.strings.len()),
    );
    global.set_initializer(&data);
    global.set_constant(true);
    global.set_linkage(Linkage::Private);
    global.set_alignment(1);
    self.strings.insert(Box::from(text), global);
    global
  }

  /// A block of bytes with pointers into itself: the type table (**L§17**).
  ///
  /// LLVM has no "byte array with relocations", so the block becomes a packed
  /// struct alternating byte runs and pointers, and each pointer is a constant
  /// offset from the global's own address — which is legal because the global
  /// is declared before its initializer is written.
  fn image_global(
    &mut self,
    symbol: &str,
    alignment: u32,
    bytes: &[u8],
    relocations: &[(u64, ConstLink)],
  ) -> GlobalValue<'ctx> {
    let mut sorted: Vec<(u64, ConstLink)> = relocations.to_vec();
    sorted.sort_unstable_by_key(|(at, _)| *at);

    let i8_type = self.context.i8_type();
    let pointer = self.context.ptr_type(AddressSpace::default());
    let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
    let mut cursor = 0usize;
    for (at, _) in &sorted {
      let at = *at as usize;
      if at > cursor {
        fields.push(i8_type.array_type((at - cursor) as u32).into());
      }
      fields.push(pointer.into());
      cursor = at + POINTER_SIZE;
    }
    if cursor < bytes.len() {
      fields.push(i8_type.array_type((bytes.len() - cursor) as u32).into());
    }
    let storage = self.context.struct_type(&fields, true);

    let value = self
      .module
      .add_global(storage, Some(AddressSpace::default()), symbol);
    value.set_alignment(alignment);
    value.set_constant(true);
    // A split program keeps the type-table image in the primary unit, so every
    // other unit reaches it by symbol.
    match self.unit.is_split() {
      true => {
        value.set_linkage(Linkage::External);
        value.set_visibility(GlobalVisibility::Hidden);
      }
      false => value.set_linkage(Linkage::Internal),
    }

    let base = value.as_pointer_value();
    let i64_type = self.context.i64_type();
    let mut values: Vec<BasicValueEnum<'ctx>> = Vec::new();
    let mut cursor = 0usize;
    for (at, target) in &sorted {
      let at = *at as usize;
      if at > cursor {
        values.push(self.context.const_string(&bytes[cursor..at], false).into());
      }
      values.push(match target {
        ConstLink::Offset(offset) => {
          let offset = i64_type.const_int(*offset, false);
          unsafe { base.const_gep(i8_type, &[offset]) }.into()
        }
        ConstLink::Procedure(id) => self.functions[id.0 as usize]
          .as_global_value()
          .as_pointer_value()
          .into(),
      });
      cursor = at + POINTER_SIZE;
    }
    if cursor < bytes.len() {
      values.push(self.context.const_string(&bytes[cursor..], false).into());
    }
    value.set_initializer(&self.context.const_struct(&values, true));
    value
  }

  /// Read-only storage holding an aggregate constant's bytes, aligned the way
  /// its type asks so that loads out of it are legal.
  fn bytes_data(
    &mut self,
    bytes: &[u8],
    links: &[(u64, ConstLink)],
    alignment: u32,
  ) -> PointerValue<'ctx> {
    // A pointer among the bytes points somewhere else inside them, which is
    // the same shape the type table image has (**L§12.1**, **L§17**).
    if !links.is_empty() {
      let symbol = format!(".data.{}", self.constants);
      self.constants += 1;
      let global = self.image_global(&symbol, alignment.max(8), bytes, links);
      global.set_linkage(Linkage::Private);
      return global.as_pointer_value();
    }
    let data = self.context.const_string(bytes, false);
    let global = self.module.add_global(
      data.get_type(),
      Some(AddressSpace::default()),
      &format!(".data.{}", self.constants),
    );
    self.constants += 1;
    global.set_initializer(&data);
    global.set_constant(true);
    global.set_linkage(Linkage::Private);
    global.set_alignment(alignment);
    global.as_pointer_value()
  }

  // -------------------------------------------------------------- bodies ----

  fn emit_body(&mut self, id: ProcId, procedure: &Procedure) -> Result<(), String> {
    // Whatever the last body left in the builder belongs to that function, and
    // a location from another function is not valid metadata here.
    self.builder.unset_current_debug_location();
    let function = self.functions[id.0 as usize];
    let entry = self.context.append_basic_block(function, "entry");
    let blocks: Vec<_> = (0..procedure.blocks.len())
      .map(|index| {
        self
          .context
          .append_basic_block(function, &format!("block{index}"))
      })
      .collect();

    self.builder.position_at_end(entry);
    let mut locals = Vec::with_capacity(procedure.locals.len());
    for local in &procedure.locals {
      let storage = self.context.i8_type().array_type(local.size as u32);
      let slot = self
        .builder
        .build_alloca(storage, &local.name)
        .map_err(|error| error.to_string())?;
      slot
        .as_instruction_value()
        .and_then(|instruction| instruction.set_alignment(local.alignment as u32).ok());
      locals.push(slot);
    }
    if self.debug.is_some() {
      let described = crate::debug::Described {
        types: &self.program.types,
        names: &self.program.names,
      };
      for (index, local) in procedure.locals.iter().enumerate() {
        let storage = locals[index];
        if let Some(debug) = self.debug.as_mut() {
          debug.declare_local(described, id, local, storage, entry);
        }
      }
    }

    let mut values: Vec<Option<BasicValueEnum<'ctx>>> = vec![None; procedure.value_types.len()];
    // The IR's first values are the ABI parameters, one each. A `#c_call`
    // carrying an aggregate in registers arrives as several LLVM parameters,
    // which are written back into storage so the body has the pointer it
    // expects; a return that comes back in registers has no parameter at all,
    // and its storage is the body's own (**L§7.11**).
    let mut position = 0u32;
    for (index, parameter) in procedure.abi.parameters.iter().enumerate() {
      if parameter.kind == ParameterKind::ReturnPointer
        && index == 0
        && procedure
          .abi
          .return_class
          .as_ref()
          .is_some_and(Classification::in_registers)
      {
        values[index] = Some(self.aggregate_slot(parameter.type_id)?.into());
        continue;
      }
      let parts = parameter
        .class
        .as_ref()
        .and_then(|class| self.register_parts(parameter.type_id, class));
      match (parameter.kind, parts) {
        (ParameterKind::Pointer, Some(parts)) => {
          let slot = self.aggregate_slot(parameter.type_id)?;
          for (_, offset) in parts {
            let Some(incoming) = function.get_nth_param(position) else {
              return Err(String::from("a coerced parameter is missing its registers"));
            };
            let target = self.byte_offset(slot, offset)?;
            self
              .builder
              .build_store(target, incoming)
              .map_err(|error| error.to_string())?;
            position += 1;
          }
          values[index] = Some(slot.into());
        }
        _ => {
          values[index] = function.get_nth_param(position);
          position += 1;
        }
      }
    }

    self
      .builder
      .build_unconditional_branch(blocks[procedure.entry.0 as usize])
      .map_err(|error| error.to_string())?;

    for (index, block) in procedure.blocks.iter().enumerate() {
      self.builder.position_at_end(blocks[index]);
      for (position, instruction) in block.instructions.iter().enumerate() {
        if let Some(debug) = self.debug.as_ref() {
          debug.locate(&self.builder, id, block.location(position));
        }
        self.emit_instruction(procedure, instruction, &mut values, &locals)?;
      }
      if let Some(debug) = self.debug.as_ref() {
        debug.locate(&self.builder, id, block.terminator_location);
      }
      self.emit_terminator(procedure, &block.terminator, &values, &blocks)?;
    }
    Ok(())
  }

  /// Stack storage for an aggregate a `#c_call` hands over in registers, so
  /// that the body still has one pointer to it (**L§7.11**).
  fn aggregate_slot(&self, type_id: TypeId) -> Result<PointerValue<'ctx>, String> {
    let size = self.types().size_of(type_id).unwrap_or(0).max(1);
    let alignment = self.types().align_of(type_id).unwrap_or(1).max(1);
    let storage = self.context.i8_type().array_type(size as u32);
    let slot = self
      .builder
      .build_alloca(storage, "coerced")
      .map_err(|error| error.to_string())?;
    slot
      .as_instruction_value()
      .and_then(|instruction| instruction.set_alignment(alignment as u32).ok());
    Ok(slot)
  }

  fn value(
    &self,
    values: &[Option<BasicValueEnum<'ctx>>],
    id: oj_ir::ValueId,
  ) -> Result<BasicValueEnum<'ctx>, String> {
    values[id.0 as usize].ok_or_else(|| format!("value %{} was used before it was set", id.0))
  }

  fn pointer(
    &self,
    values: &[Option<BasicValueEnum<'ctx>>],
    id: oj_ir::ValueId,
  ) -> Result<PointerValue<'ctx>, String> {
    match self.value(values, id)? {
      BasicValueEnum::PointerValue(pointer) => Ok(pointer),
      other => Err(format!("value %{} is {other:?}, not an address", id.0)),
    }
  }

  fn emit_instruction(
    &mut self,
    procedure: &Procedure,
    instruction: &Inst,
    values: &mut Vec<Option<BasicValueEnum<'ctx>>>,
    locals: &[PointerValue<'ctx>],
  ) -> Result<(), String> {
    let i8_type = self.context.i8_type();
    match instruction {
      Inst::Const { dest, value } => {
        let type_id = procedure.value_type(*dest);
        let emitted = match value {
          Constant::String(text) => {
            let global = self.string_view(text);
            global.as_pointer_value().into()
          }
          // An aggregate constant is data, and its value is where that data
          // sits (**L§3.14**).
          Constant::Bytes { bytes, links } => {
            let alignment = self
              .types()
              .align_of(self.types().underlying(type_id))
              .unwrap_or(1)
              .max(1);
            self.bytes_data(bytes, links, alignment as u32).into()
          }
          _ => {
            let llvm_type = self.llvm_type(type_id);
            self
              .scalar_constant(value, llvm_type)
              .ok_or_else(|| format!("cannot emit the constant {value:?}"))?
          }
        };
        values[dest.0 as usize] = Some(emitted);
      }
      Inst::LocalAddress { dest, local } => {
        values[dest.0 as usize] = Some(locals[local.0 as usize].into());
      }
      Inst::GlobalAddress { dest, global } => {
        values[dest.0 as usize] = Some(self.globals[global.0 as usize].as_pointer_value().into());
      }
      Inst::ProcedureAddress { dest, procedure } => {
        values[dest.0 as usize] = Some(
          self.functions[procedure.0 as usize]
            .as_global_value()
            .as_pointer_value()
            .into(),
        );
      }
      Inst::Load { dest, address } => {
        let llvm_type = self.llvm_type(procedure.value_type(*dest));
        let address = self.pointer(values, *address)?;
        let loaded = self
          .builder
          .build_load(llvm_type, address, "")
          .map_err(|error| error.to_string())?;
        values[dest.0 as usize] = Some(loaded);
      }
      // Preload's `compare_and_swap` (**L§17**). LLVM's `cmpxchg` gives back
      // the pair the procedure returns: what was there, and whether the swap
      // happened.
      Inst::AtomicCompareExchange {
        success,
        previous,
        address,
        expected,
        desired,
      } => {
        let address = self.pointer(values, *address)?;
        let expected = self.value(values, *expected)?;
        let desired = self.value(values, *desired)?;
        let exchanged = self
          .builder
          .build_cmpxchg(
            address,
            expected,
            desired,
            inkwell::AtomicOrdering::SequentiallyConsistent,
            inkwell::AtomicOrdering::SequentiallyConsistent,
          )
          .map_err(|error| error.to_string())?;
        let before = self
          .builder
          .build_extract_value(exchanged, 0, "cmpxchg.previous")
          .map_err(|error| error.to_string())?;
        let happened = self
          .builder
          .build_extract_value(exchanged, 1, "cmpxchg.success")
          .map_err(|error| error.to_string())?;
        let happened = self
          .builder
          .build_int_z_extend(
            happened.into_int_value(),
            self
              .llvm_type(procedure.value_type(*success))
              .into_int_type(),
            "cmpxchg.bool",
          )
          .map_err(|error| error.to_string())?;
        values[previous.0 as usize] = Some(before);
        values[success.0 as usize] = Some(happened.into());
      }
      Inst::Store { address, value } => {
        let address = self.pointer(values, *address)?;
        let value = self.value(values, *value)?;
        self
          .builder
          .build_store(address, value)
          .map_err(|error| error.to_string())?;
      }
      Inst::Copy {
        destination,
        source,
        size,
        alignment,
      } => {
        let destination = self.pointer(values, *destination)?;
        let source = self.pointer(values, *source)?;
        let size = self.context.i64_type().const_int(*size, false);
        self
          .builder
          .build_memcpy(
            destination,
            *alignment as u32,
            source,
            *alignment as u32,
            size,
          )
          .map_err(|error| error.to_string())?;
      }
      Inst::Clear {
        destination,
        size,
        alignment,
      } => {
        let destination = self.pointer(values, *destination)?;
        let size = self.context.i64_type().const_int(*size, false);
        let zero = i8_type.const_zero();
        self
          .builder
          .build_memset(destination, *alignment as u32, zero, size)
          .map_err(|error| error.to_string())?;
      }
      Inst::Offset { dest, base, offset } => {
        let base = self.pointer(values, *base)?;
        let index = self.context.i64_type().const_int(*offset as u64, true);
        let address = unsafe {
          self
            .builder
            .build_in_bounds_gep(i8_type, base, &[index], "")
            .map_err(|error| error.to_string())?
        };
        values[dest.0 as usize] = Some(address.into());
      }
      Inst::Index {
        dest,
        base,
        index,
        stride,
      } => {
        let base = self.pointer(values, *base)?;
        let index = self.value(values, *index)?.into_int_value();
        let index = self.widen_index(index)?;
        let stride = self.context.i64_type().const_int(*stride, false);
        let offset = self
          .builder
          .build_int_mul(index, stride, "")
          .map_err(|error| error.to_string())?;
        let address = unsafe {
          self
            .builder
            .build_in_bounds_gep(i8_type, base, &[offset], "")
            .map_err(|error| error.to_string())?
        };
        values[dest.0 as usize] = Some(address.into());
      }
      Inst::Unary {
        dest,
        operator,
        operand,
      } => {
        let value = self.value(values, *operand)?;
        let result = self.unary(*operator, value)?;
        values[dest.0 as usize] = Some(result);
      }
      Inst::Binary {
        dest,
        operator,
        left,
        right,
      } => {
        let signed = self
          .types()
          .integer_kind(procedure.value_type(*left))
          .is_none_or(|kind| kind.is_signed());
        let left = self.value(values, *left)?;
        let right = self.value(values, *right)?;
        let result = self.binary(*operator, left, right, signed)?;
        values[dest.0 as usize] = Some(result);
      }
      Inst::Convert {
        dest,
        kind,
        operand,
      } => {
        let target = self.llvm_type(procedure.value_type(*dest));
        let value = self.value(values, *operand)?;
        let result = self.convert(*kind, value, target)?;
        values[dest.0 as usize] = Some(result);
      }
      // An `#asm` block arrives with its registers already chosen (**L§15**),
      // so all that is left is to name each one in a constraint and let the
      // integrated assembler read the text.
      Inst::Asm {
        text,
        inputs,
        outputs,
        clobbers,
      } => {
        let output_types: Vec<BasicTypeEnum<'ctx>> = outputs
          .iter()
          .map(|binding| self.llvm_type(procedure.value_type(binding.value)))
          .collect();
        let parameter_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = inputs
          .iter()
          .map(|binding| self.llvm_type(procedure.value_type(binding.value)).into())
          .collect();
        let signature = match output_types.len() {
          0 => self.context.void_type().fn_type(&parameter_types, false),
          1 => output_types[0].fn_type(&parameter_types, false),
          _ => self
            .context
            .struct_type(&output_types, false)
            .fn_type(&parameter_types, false),
        };
        let mut constraints: Vec<String> = Vec::new();
        for binding in outputs.iter().chain(inputs) {
          constraints.push(binding.constraint.clone());
        }
        for register in clobbers {
          constraints.push(format!("~{register}"));
        }
        // Every `#asm` block may set the flags and touch memory, and the
        // reference's own blocks do both.
        constraints.push(String::from("~{dirflag}"));
        constraints.push(String::from("~{fpsr}"));
        constraints.push(String::from("~{flags}"));
        constraints.push(String::from("~{memory}"));
        let assembly = self.context.create_inline_asm(
          signature,
          text.clone(),
          constraints.join(","),
          true,
          false,
          Some(inkwell::InlineAsmDialect::Intel),
          false,
        );
        let arguments: Vec<BasicMetadataValueEnum<'ctx>> = inputs
          .iter()
          .map(|binding| self.value(values, binding.value).map(Into::into))
          .collect::<Result<_, _>>()?;
        let site = self
          .builder
          .build_indirect_call(signature, assembly, &arguments, "")
          .map_err(|error| error.to_string())?;
        match outputs.len() {
          0 => {}
          1 => {
            values[outputs[0].value.0 as usize] = site.try_as_basic_value().basic();
          }
          _ => {
            let Some(result) = site.try_as_basic_value().basic() else {
              return Err(String::from("an '#asm' block produced no result to read"));
            };
            for (index, binding) in outputs.iter().enumerate() {
              let extracted = self
                .builder
                .build_extract_value(result.into_struct_value(), index as u32, "")
                .map_err(|error| error.to_string())?;
              values[binding.value.0 as usize] = Some(extracted);
            }
          }
        }
      }
      Inst::Call {
        dest,
        callee,
        signature,
        arguments,
      } => {
        let abi = match callee {
          Callee::Direct(id) => self.program.procedure(*id).abi.clone(),
          Callee::Indirect(_) => {
            let flags = if self
              .types()
              .procedure_of(*signature)
              .is_some_and(|signature| {
                signature.flags.intersects(
                  oj_types::ProcedureFlags::IS_C_CALL | oj_types::ProcedureFlags::HAS_NO_CONTEXT,
                )
              }) {
              ProcedureFlags::NO_CONTEXT
            } else {
              ProcedureFlags::empty()
            };
            oj_ir::abi_of(self.types(), *signature, flags, self.program.context_type)
          }
        };
        let (call_arguments, returned_into) = self.call_arguments(&abi, arguments, values)?;
        let site = match callee {
          Callee::Direct(id) => self
            .builder
            .build_direct_call(self.functions[id.0 as usize], &call_arguments, "")
            .map_err(|error| error.to_string())?,
          Callee::Indirect(value) => {
            let pointer = self.pointer(values, *value)?;
            let function_type = self.function_type(&abi);
            self
              .builder
              .build_indirect_call(function_type, pointer, &call_arguments, "")
              .map_err(|error| error.to_string())?
          }
        };
        for (position, attribute) in self.memory_attributes(&abi) {
          site.add_attribute(AttributeLoc::Param(position), attribute);
        }
        // A `#c_call` whose aggregate return came back in registers leaves it
        // where the IR expected to find it (**L§7.11**).
        if let Some(storage) = returned_into
          && let Some(result) = site.try_as_basic_value().basic()
        {
          self
            .builder
            .build_store(storage, result)
            .map_err(|error| error.to_string())?;
        }
        if let Some(dest) = dest {
          values[dest.0 as usize] = site.try_as_basic_value().basic();
        }
      }
    }
    Ok(())
  }

  /// The arguments a call actually passes. The IR hands one value per
  /// `AbiParameter`; a `#c_call` carrying an aggregate in registers loads its
  /// eightbytes out of the pointer here, and one whose return comes back in
  /// registers drops the storage from the list and hands it back instead
  /// (**L§7.11**).
  #[allow(clippy::type_complexity)]
  fn call_arguments(
    &mut self,
    abi: &oj_ir::Abi,
    arguments: &[oj_ir::ValueId],
    values: &[Option<BasicValueEnum<'ctx>>],
  ) -> Result<
    (
      Vec<BasicMetadataValueEnum<'ctx>>,
      Option<PointerValue<'ctx>>,
    ),
    String,
  > {
    let mut passed = Vec::with_capacity(arguments.len());
    let mut returned_into = None;
    for (index, argument) in arguments.iter().enumerate() {
      let Some(parameter) = abi.parameters.get(index) else {
        passed.push(self.value(values, *argument)?.into());
        continue;
      };
      if parameter.kind == ParameterKind::ReturnPointer
        && index == 0
        && abi
          .return_class
          .as_ref()
          .is_some_and(Classification::in_registers)
      {
        returned_into = Some(self.pointer(values, *argument)?);
        continue;
      }
      let parts = parameter
        .class
        .as_ref()
        .and_then(|class| self.register_parts(parameter.type_id, class));
      match (parameter.kind, parts) {
        (ParameterKind::Pointer, Some(parts)) => {
          let address = self.pointer(values, *argument)?;
          for (part_type, offset) in parts {
            let slot = self.byte_offset(address, offset)?;
            let loaded = self
              .builder
              .build_load(part_type, slot, "")
              .map_err(|error| error.to_string())?;
            passed.push(loaded.into());
          }
        }
        _ => passed.push(self.value(values, *argument)?.into()),
      }
    }
    Ok((passed, returned_into))
  }

  fn byte_offset(
    &self,
    address: PointerValue<'ctx>,
    offset: u64,
  ) -> Result<PointerValue<'ctx>, String> {
    if offset == 0 {
      return Ok(address);
    }
    let index = self.context.i64_type().const_int(offset, false);
    unsafe {
      self
        .builder
        .build_in_bounds_gep(self.context.i8_type(), address, &[index], "")
        .map_err(|error| error.to_string())
    }
  }

  fn string_view(&mut self, text: &[u8]) -> GlobalValue<'ctx> {
    let data = self.string_bytes(text);
    let count = self.context.i64_type().const_int(text.len() as u64, false);
    let view = self
      .context
      .const_struct(&[count.into(), data.as_pointer_value().into()], false);
    let global = self.module.add_global(
      view.get_type(),
      Some(AddressSpace::default()),
      &format!(".view.{}", self.strings.len()),
    );
    global.set_initializer(&view);
    global.set_constant(true);
    global.set_linkage(Linkage::Private);
    global.set_alignment(8);
    global
  }

  fn widen_index(&self, index: IntValue<'ctx>) -> Result<IntValue<'ctx>, String> {
    let target = self.context.i64_type();
    match index.get_type().get_bit_width().cmp(&64) {
      std::cmp::Ordering::Less => self
        .builder
        .build_int_s_extend(index, target, "")
        .map_err(|error| error.to_string()),
      std::cmp::Ordering::Greater => self
        .builder
        .build_int_truncate(index, target, "")
        .map_err(|error| error.to_string()),
      std::cmp::Ordering::Equal => Ok(index),
    }
  }

  fn unary(
    &self,
    operator: UnaryOp,
    value: BasicValueEnum<'ctx>,
  ) -> Result<BasicValueEnum<'ctx>, String> {
    let result = match (operator, value) {
      (UnaryOp::Negate, BasicValueEnum::IntValue(value)) => self
        .builder
        .build_int_neg(value, "")
        .map_err(|error| error.to_string())?
        .into(),
      (UnaryOp::Negate, BasicValueEnum::FloatValue(value)) => self
        .builder
        .build_float_neg(value, "")
        .map_err(|error| error.to_string())?
        .into(),
      (UnaryOp::BitwiseNot, BasicValueEnum::IntValue(value)) => self
        .builder
        .build_not(value, "")
        .map_err(|error| error.to_string())?
        .into(),
      (UnaryOp::LogicalNot, BasicValueEnum::IntValue(value)) => {
        let zero = value.get_type().const_zero();
        let compared = self
          .builder
          .build_int_compare(IntPredicate::EQ, value, zero, "")
          .map_err(|error| error.to_string())?;
        self.widen_bit(compared)?
      }
      _ => return Err(format!("cannot apply {operator:?} to {value:?}")),
    };
    Ok(result)
  }

  /// A `bool` is a byte in memory and a bit in a comparison (**L§3.1**).
  fn widen_bit(&self, value: IntValue<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    self
      .builder
      .build_int_z_extend(value, self.context.i8_type(), "")
      .map(Into::into)
      .map_err(|error| error.to_string())
  }

  fn narrow_to_bit(&self, value: BasicValueEnum<'ctx>) -> Result<IntValue<'ctx>, String> {
    let value = value.into_int_value();
    if value.get_type().get_bit_width() == 1 {
      return Ok(value);
    }
    self
      .builder
      .build_int_compare(IntPredicate::NE, value, value.get_type().const_zero(), "")
      .map_err(|error| error.to_string())
  }

  fn binary(
    &self,
    operator: BinaryOp,
    left: BasicValueEnum<'ctx>,
    right: BasicValueEnum<'ctx>,
    signed: bool,
  ) -> Result<BasicValueEnum<'ctx>, String> {
    if let (BasicValueEnum::FloatValue(left), BasicValueEnum::FloatValue(right)) = (left, right) {
      let builder = &self.builder;
      let result: BasicValueEnum<'ctx> = match operator {
        BinaryOp::Add => builder.build_float_add(left, right, ""),
        BinaryOp::Subtract => builder.build_float_sub(left, right, ""),
        BinaryOp::Multiply => builder.build_float_mul(left, right, ""),
        BinaryOp::Divide => builder.build_float_div(left, right, ""),
        BinaryOp::Modulus => builder.build_float_rem(left, right, ""),
        _ => {
          let predicate = match operator {
            BinaryOp::Equal => FloatPredicate::OEQ,
            BinaryOp::NotEqual => FloatPredicate::ONE,
            BinaryOp::Less => FloatPredicate::OLT,
            BinaryOp::LessOrEqual => FloatPredicate::OLE,
            BinaryOp::Greater => FloatPredicate::OGT,
            BinaryOp::GreaterOrEqual => FloatPredicate::OGE,
            _ => return Err(format!("cannot apply {operator:?} to floats")),
          };
          let compared = builder
            .build_float_compare(predicate, left, right, "")
            .map_err(|error| error.to_string())?;
          return self.widen_bit(compared);
        }
      }
      .map_err(|error| error.to_string())?
      .into();
      return Ok(result);
    }

    // Comparing two pointers is comparing two addresses.
    if let (BasicValueEnum::PointerValue(left), BasicValueEnum::PointerValue(right)) = (left, right)
    {
      let target = self.context.i64_type();
      let left = self
        .builder
        .build_ptr_to_int(left, target, "")
        .map_err(|error| error.to_string())?;
      let right = self
        .builder
        .build_ptr_to_int(right, target, "")
        .map_err(|error| error.to_string())?;
      return self.binary(operator, left.into(), right.into(), false);
    }

    let (BasicValueEnum::IntValue(left), BasicValueEnum::IntValue(right)) = (left, right) else {
      return Err(format!(
        "cannot apply {operator:?} to {left:?} and {right:?}"
      ));
    };
    // A shift keeps the width of what is being shifted; the amount is a count,
    // and LLVM wants it in the same type (**L§5.2**).
    let right = match operator {
      BinaryOp::ShiftLeft | BinaryOp::ShiftRight | BinaryOp::RotateLeft | BinaryOp::RotateRight => {
        let (from, to) = (
          right.get_type().get_bit_width(),
          left.get_type().get_bit_width(),
        );
        match from.cmp(&to) {
          std::cmp::Ordering::Equal => right,
          std::cmp::Ordering::Less => self
            .builder
            .build_int_z_extend(right, left.get_type(), "")
            .map_err(|error| error.to_string())?,
          std::cmp::Ordering::Greater => self
            .builder
            .build_int_truncate(right, left.get_type(), "")
            .map_err(|error| error.to_string())?,
        }
      }
      _ => right,
    };
    let builder = &self.builder;
    let result: BasicValueEnum<'ctx> = match operator {
      BinaryOp::Add => builder.build_int_add(left, right, ""),
      BinaryOp::Subtract => builder.build_int_sub(left, right, ""),
      BinaryOp::Multiply => builder.build_int_mul(left, right, ""),
      BinaryOp::Divide if signed => builder.build_int_signed_div(left, right, ""),
      BinaryOp::Divide => builder.build_int_unsigned_div(left, right, ""),
      BinaryOp::Modulus if signed => builder.build_int_signed_rem(left, right, ""),
      BinaryOp::Modulus => builder.build_int_unsigned_rem(left, right, ""),
      BinaryOp::BitwiseAnd => builder.build_and(left, right, ""),
      BinaryOp::BitwiseOr => builder.build_or(left, right, ""),
      BinaryOp::BitwiseXor => builder.build_xor(left, right, ""),
      BinaryOp::ShiftLeft => builder.build_left_shift(left, right, ""),
      BinaryOp::ShiftRight => builder.build_right_shift(left, right, signed, ""),
      BinaryOp::RotateLeft | BinaryOp::RotateRight => {
        return self.rotate(operator, left, right);
      }
      _ => {
        let predicate = match (operator, signed) {
          (BinaryOp::Equal, _) => IntPredicate::EQ,
          (BinaryOp::NotEqual, _) => IntPredicate::NE,
          (BinaryOp::Less, true) => IntPredicate::SLT,
          (BinaryOp::Less, false) => IntPredicate::ULT,
          (BinaryOp::LessOrEqual, true) => IntPredicate::SLE,
          (BinaryOp::LessOrEqual, false) => IntPredicate::ULE,
          (BinaryOp::Greater, true) => IntPredicate::SGT,
          (BinaryOp::Greater, false) => IntPredicate::UGT,
          (BinaryOp::GreaterOrEqual, true) => IntPredicate::SGE,
          (BinaryOp::GreaterOrEqual, false) => IntPredicate::UGE,
          _ => return Err(format!("cannot apply {operator:?} to integers")),
        };
        let compared = builder
          .build_int_compare(predicate, left, right, "")
          .map_err(|error| error.to_string())?;
        return self.widen_bit(compared);
      }
    }
    .map_err(|error| error.to_string())?
    .into();
    Ok(result)
  }

  /// `<<<` and `>>>` are the funnel shifts (**L§5.2**).
  fn rotate(
    &self,
    operator: BinaryOp,
    left: IntValue<'ctx>,
    right: IntValue<'ctx>,
  ) -> Result<BasicValueEnum<'ctx>, String> {
    let width = left.get_type().get_bit_width();
    let name = match operator {
      BinaryOp::RotateLeft => format!("llvm.fshl.i{width}"),
      _ => format!("llvm.fshr.i{width}"),
    };
    let function = match self.module.get_function(&name) {
      Some(function) => function,
      None => {
        let int = left.get_type();
        let signature = int.fn_type(&[int.into(), int.into(), int.into()], false);
        self.module.add_function(&name, signature, None)
      }
    };
    let amount = if right.get_type().get_bit_width() == width {
      right
    } else {
      self
        .builder
        .build_int_truncate_or_bit_cast(right, left.get_type(), "")
        .map_err(|error| error.to_string())?
    };
    let site = self
      .builder
      .build_direct_call(function, &[left.into(), left.into(), amount.into()], "")
      .map_err(|error| error.to_string())?;
    site
      .try_as_basic_value()
      .basic()
      .ok_or_else(|| String::from("a rotate produced no value"))
  }

  fn convert(
    &self,
    kind: ConvertKind,
    value: BasicValueEnum<'ctx>,
    target: BasicTypeEnum<'ctx>,
  ) -> Result<BasicValueEnum<'ctx>, String> {
    let builder = &self.builder;
    let result: BasicValueEnum<'ctx> = match kind {
      ConvertKind::IntegerZeroExtend => builder
        .build_int_z_extend(value.into_int_value(), target.into_int_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::IntegerSignExtend => builder
        .build_int_s_extend(value.into_int_value(), target.into_int_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::IntegerTruncate => builder
        .build_int_truncate(value.into_int_value(), target.into_int_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::SignedToFloat => builder
        .build_signed_int_to_float(value.into_int_value(), target.into_float_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::UnsignedToFloat => builder
        .build_unsigned_int_to_float(value.into_int_value(), target.into_float_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::FloatToSigned => builder
        .build_float_to_signed_int(value.into_float_value(), target.into_int_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::FloatToUnsigned => builder
        .build_float_to_unsigned_int(value.into_float_value(), target.into_int_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::FloatExtend => builder
        .build_float_ext(value.into_float_value(), target.into_float_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::FloatTruncate => builder
        .build_float_trunc(value.into_float_value(), target.into_float_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::IntegerToPointer => builder
        .build_int_to_ptr(value.into_int_value(), target.into_pointer_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      ConvertKind::PointerToInteger => builder
        .build_ptr_to_int(value.into_pointer_value(), target.into_int_type(), "")
        .map_err(|error| error.to_string())?
        .into(),
      // Pointers are opaque, so a pointer cast is nothing at all; two integers
      // of the same width are the same bits.
      ConvertKind::Bitcast => value,
    };
    Ok(result)
  }

  fn emit_terminator(
    &self,
    procedure: &Procedure,
    terminator: &Terminator,
    values: &[Option<BasicValueEnum<'ctx>>],
    blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
  ) -> Result<(), String> {
    let _ = procedure;
    match terminator {
      Terminator::Return(returned) => {
        // A `#c_call` whose aggregate return travels in registers reads it back
        // out of the storage the body wrote it into (**L§7.11**).
        if let Some(class) = &procedure.abi.return_class
          && let Some(parameter) = procedure.abi.parameters.first()
          && let Some(coerced) = self.coerced_type(parameter.type_id, class)
        {
          let storage = self.pointer(values, oj_ir::ValueId(0))?;
          let value = self
            .builder
            .build_load(coerced, storage, "")
            .map_err(|error| error.to_string())?;
          self
            .builder
            .build_return(Some(&value))
            .map_err(|error| error.to_string())?;
          return Ok(());
        }
        match returned.first() {
          Some(value) => {
            let value = self.value(values, *value)?;
            self.builder.build_return(Some(&value))
          }
          None => self.builder.build_return(None),
        }
        .map_err(|error| error.to_string())?;
      }
      Terminator::Jump(block) => {
        self
          .builder
          .build_unconditional_branch(blocks[block.0 as usize])
          .map_err(|error| error.to_string())?;
      }
      Terminator::Branch {
        condition,
        then_block,
        else_block,
      } => {
        let condition = self.narrow_to_bit(self.value(values, *condition)?)?;
        self
          .builder
          .build_conditional_branch(
            condition,
            blocks[then_block.0 as usize],
            blocks[else_block.0 as usize],
          )
          .map_err(|error| error.to_string())?;
      }
      Terminator::Unreachable => {
        self
          .builder
          .build_unreachable()
          .map_err(|error| error.to_string())?;
      }
    }
    Ok(())
  }

  // -------------------------------------------------------- entry point -----

  /// The `main` the C runtime calls: it hands the program a zeroed `#Context`
  /// and runs the global initializers first (**L§10.1**, **L§11.1**).
  fn emit_entry_point(&mut self) -> Result<(), String> {
    self.builder.unset_current_debug_location();
    let Some(entry) = self.program.entry else {
      return Ok(());
    };
    let context_size = self
      .types()
      .size_of(self.program.context_type)
      .unwrap_or(0)
      .max(1);
    let context_align = self
      .types()
      .align_of(self.program.context_type)
      .unwrap_or(1)
      .max(1);
    let storage = self.context.i8_type().array_type(context_size as u32);
    let context = self
      .module
      .add_global(storage, Some(AddressSpace::default()), "__oj_context");
    context.set_initializer(&storage.const_zero());
    context.set_linkage(Linkage::Internal);
    context.set_alignment(context_align as u32);

    let i32_type = self.context.i32_type();
    let pointer = self.context.ptr_type(AddressSpace::default());
    let signature = i32_type.fn_type(&[i32_type.into(), pointer.into()], false);
    let function = self.module.add_function(ENTRY_SYMBOL, signature, None);
    let block = self.context.append_basic_block(function, "entry");
    self.builder.position_at_end(block);

    // `Runtime_Support.__jai_runtime_init` records the command line and hands
    // back the `#Context` the program starts with: its allocator, its logger
    // and its temporary storage (**C§13**). A program that does not reach
    // Runtime_Support gets zeroed storage instead.
    let context_pointer = match self.program.runtime_init {
      Some(init) => {
        let argc = function
          .get_nth_param(0)
          .ok_or("the entry point takes argc")?;
        let argv = function
          .get_nth_param(1)
          .ok_or("the entry point takes argv")?;
        let site = self
          .builder
          .build_direct_call(
            self.functions[init.0 as usize],
            &[argc.into(), argv.into()],
            "",
          )
          .map_err(|error| error.to_string())?;
        match site.try_as_basic_value().basic() {
          Some(BasicValueEnum::PointerValue(pointer)) => pointer,
          _ => context.as_pointer_value(),
        }
      }
      None => context.as_pointer_value(),
    };
    // The handler goes in before anything the program does, so that a crash on
    // the very first statement still says what happened (**C§13**).
    if let Some(handler) = self.program.crash_handler_init {
      self
        .builder
        .build_direct_call(self.functions[handler.0 as usize], &[], "")
        .map_err(|error| error.to_string())?;
    }
    // The trace info records name procedures, whose addresses only this module
    // knows, so they are filled before anything can trace (**C§13**).
    if let Some(initializer) = self.program.stack_trace_init {
      self
        .builder
        .build_direct_call(self.functions[initializer.0 as usize], &[], "")
        .map_err(|error| error.to_string())?;
    }
    if let Some(initializer) = self.program.global_init {
      self
        .builder
        .build_direct_call(
          self.functions[initializer.0 as usize],
          &[context_pointer.into()],
          "",
        )
        .map_err(|error| error.to_string())?;
    }

    let main = self.functions[entry.0 as usize];
    let arguments: Vec<BasicMetadataValueEnum<'ctx>> =
      if self.program.procedure(entry).abi.parameters.is_empty() {
        Vec::new()
      } else {
        vec![context_pointer.into()]
      };
    self
      .builder
      .build_direct_call(main, &arguments, "")
      .map_err(|error| error.to_string())?;
    self
      .builder
      .build_return(Some(&i32_type.const_zero().as_basic_value_enum()))
      .map_err(|error| error.to_string())?;
    Ok(())
  }
}

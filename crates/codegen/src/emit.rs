//! Lowering `oj-ir` into an LLVM module.
//!
//! Aggregates are byte arrays here: every member access the front end resolved
//! is already a byte offset (**L§3.14**), so the back end never has to make
//! LLVM agree with `oj-types` about a struct layout — it only has to keep the
//! size and the alignment.

use std::collections::HashMap;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::{BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{
  BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, GlobalValue, IntValue,
  PointerValue,
};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate};

use oj_ir::{
  BinaryOp, Callee, Constant, ConvertKind, GlobalInit, Inst, ParameterKind, ProcId, Procedure,
  ProcedureFlags, Program, Terminator, UnaryOp,
};
use oj_types::{FloatKind, TypeId, TypeKind, Types};

/// The name the C runtime calls. orangejuice emits its own, which sets up a
/// zeroed `#Context` and calls the program's `main`; the `Runtime_Support`
/// entry point that would normally own this symbol needs compile-time
/// execution to initialize temporary storage, which is milestone M6.
const ENTRY_SYMBOL: &str = "main";

pub struct Emitter<'ctx, 'p> {
  context: &'ctx Context,
  module: Module<'ctx>,
  builder: Builder<'ctx>,
  program: &'p Program,
  functions: Vec<FunctionValue<'ctx>>,
  globals: Vec<GlobalValue<'ctx>>,
  strings: HashMap<Box<[u8]>, GlobalValue<'ctx>>,
}

impl<'ctx, 'p> Emitter<'ctx, 'p> {
  pub fn new(context: &'ctx Context, program: &'p Program, name: &str) -> Self {
    Self {
      context,
      module: context.create_module(name),
      builder: context.create_builder(),
      program,
      functions: Vec::new(),
      globals: Vec::new(),
      strings: HashMap::new(),
    }
  }

  pub fn module(&self) -> &Module<'ctx> {
    &self.module
  }

  fn types(&self) -> &'p Types {
    &self.program.types
  }

  pub fn emit(&mut self) -> Result<(), String> {
    self.declare_globals();
    self.declare_functions();
    for (index, procedure) in self.program.procedures.iter().enumerate() {
      if !procedure.has_body() {
        continue;
      }
      self.emit_body(ProcId(index as u32), procedure)?;
    }
    self.emit_entry_point()?;
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

  fn function_type(&self, abi: &oj_ir::Abi) -> FunctionType<'ctx> {
    let pointer = self.context.ptr_type(AddressSpace::default());
    let parameters: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = abi
      .parameters
      .iter()
      .map(|parameter| match parameter.kind {
        ParameterKind::Value => self.llvm_type(parameter.type_id).into(),
        _ => pointer.into(),
      })
      .collect();
    match abi.direct_return {
      Some(type_id) => self.llvm_type(type_id).fn_type(&parameters, false),
      None => self.context.void_type().fn_type(&parameters, false),
    }
  }

  // ------------------------------------------------------ declarations ------

  fn declare_globals(&mut self) {
    for global in &self.program.globals {
      let (llvm_type, initializer) = match &global.init {
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
        value.set_linkage(if global.external {
          Linkage::External
        } else {
          Linkage::Internal
        });
      }
      self.globals.push(value);
    }
  }

  fn declare_functions(&mut self) {
    for procedure in &self.program.procedures {
      let function_type = self.function_type(&procedure.abi);
      let function = self
        .module
        .add_function(&procedure.symbol, function_type, None);
      if procedure.has_body() && !procedure.flags.contains(ProcedureFlags::EXPORT) {
        function.set_linkage(Linkage::Internal);
      }
      self.functions.push(function);
    }
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
      (Constant::Zero, _) => llvm_type.const_zero(),
      _ => return None,
    })
  }

  fn string_bytes(&mut self, text: &[u8]) -> GlobalValue<'ctx> {
    if let Some(existing) = self.strings.get(text) {
      return *existing;
    }
    let data = self.context.const_string(text, false);
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

  // -------------------------------------------------------------- bodies ----

  fn emit_body(&mut self, id: ProcId, procedure: &Procedure) -> Result<(), String> {
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
    self
      .builder
      .build_unconditional_branch(blocks[procedure.entry.0 as usize])
      .map_err(|error| error.to_string())?;

    let mut values: Vec<Option<BasicValueEnum<'ctx>>> = vec![None; procedure.value_types.len()];
    for (index, _) in procedure.abi.parameters.iter().enumerate() {
      values[index] = function.get_nth_param(index as u32);
    }

    for (index, block) in procedure.blocks.iter().enumerate() {
      self.builder.position_at_end(blocks[index]);
      for instruction in &block.instructions {
        self.emit_instruction(procedure, instruction, &mut values, &locals)?;
      }
      self.emit_terminator(procedure, &block.terminator, &values, &blocks)?;
    }
    Ok(())
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
      Inst::Call {
        dest,
        callee,
        signature,
        arguments,
      } => {
        let arguments: Vec<BasicMetadataValueEnum<'ctx>> = arguments
          .iter()
          .map(|value| self.value(values, *value).map(Into::into))
          .collect::<Result<_, _>>()?;
        let site = match callee {
          Callee::Direct(id) => self
            .builder
            .build_direct_call(self.functions[id.0 as usize], &arguments, "")
            .map_err(|error| error.to_string())?,
          Callee::Indirect(value) => {
            let pointer = self.pointer(values, *value)?;
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
            let abi = oj_ir::abi_of(self.types(), *signature, flags, self.program.context_type);
            let function_type = self.function_type(&abi);
            self
              .builder
              .build_indirect_call(function_type, pointer, &arguments, "")
              .map_err(|error| error.to_string())?
          }
        };
        if let Some(dest) = dest {
          values[dest.0 as usize] = site.try_as_basic_value().basic();
        }
      }
    }
    Ok(())
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

    let context_pointer = context.as_pointer_value();
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

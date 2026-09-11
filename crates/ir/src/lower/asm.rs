//! `#asm` blocks (**L§15**).
//!
//! An operand names a register the block declared, a variable of the program
//! around it, or a constant. The registers a block declares are placed here —
//! lifetime-based, no spilling, pinned where the block or the instruction
//! requires it — and written into the text by name. A variable of the program
//! stays the back end's to place: it becomes an operand of the assembly, so
//! the register allocator around the block is the one that decides whether it
//! sits in a register or is addressed on the stack.

use std::collections::HashMap;
use std::fmt::Write as _;

use oj_syntax::ast::{
  AsmClass, AsmDeclaration, AsmNode, AsmOperand, AsmOperandKind, AsmRegister, AsmSize,
};

use super::*;
use crate::ir::AsmBinding;

/// A register the block declared, which the allocator here has to place.
struct Reg {
  class: AsmClass,
  pin: Option<u32>,
  /// Where the register's value lives between one block and the next
  /// (**L§15**): a block declares into the scope around it, so a later block
  /// in that scope reads what this one left.
  local: Option<LocalId>,
  first: usize,
  last: usize,
  assigned: Option<u32>,
}

/// A value of the program around the block, which travels as an operand of the
/// assembly rather than in a register the block chose.
struct Slot {
  /// The value the operand starts out holding.
  input: Option<ValueId>,
  /// The storage the operand came from, when it is an lvalue.
  address: Option<(ValueId, TypeId)>,
  /// Whether an instruction of the block writes it, so that the value has to
  /// go back where it came from when the block ends.
  written: bool,
  /// The register the block pinned it to (**L§15**).
  pin: Option<u32>,
  class: AsmClass,
  /// Whether the operand has to be in a register: the base of a memory
  /// operand cannot be addressed on the stack.
  register_only: bool,
  /// The width the instructions use it at, which is what the value is widened
  /// to before the block and narrowed from after it.
  bits: u32,
  /// Where the operand ended up in the assembly's operand list.
  position: usize,
}

impl Slot {
  /// Whether the block leaves a value here that has to go back where it came
  /// from.
  fn writes_back(&self) -> bool {
    self.written && self.address.is_some()
  }
}

/// What one operand of one instruction turned out to be.
enum Operand {
  Register(usize),
  Slot(usize),
  Immediate(i128),
  Memory {
    base: Place,
    index: Option<Place>,
    scale: u64,
    displacement: i64,
    /// `[p]!` reads one lane and repeats it across the register (**L§15**).
    broadcast: bool,
  },
}

/// The EVEX decorations an operand carries: the mask applied to what is
/// written, and the rounding an instruction was told to use (**L§15**).
#[derive(Default)]
struct Decorations {
  mask: Option<(usize, bool)>,
  rounding: Option<Option<oj_syntax::ast::RoundingMode>>,
}

/// Where a memory operand's base or index sits.
#[derive(Clone, Copy)]
enum Place {
  Register(usize),
  Slot(usize),
}

/// One instruction, with its operands resolved and its widths decided.
struct Resolved {
  form: &'static Form,
  written: String,
  scalar_bits: u32,
  vector_bits: u32,
  vex: bool,
  /// The `lock_` the reference writes into the opcode name (**L§15**).
  locked: bool,
  operands: Vec<Operand>,
  decorations: Vec<Decorations>,
}

/// Where the lowering stands while it reads one operand.
#[derive(Clone, Copy)]
struct At {
  scope: ScopeId,
  source: SourceId,
  node: NodeId,
  index: usize,
  /// The class a declaration written here takes when it does not say.
  fresh: AsmClass,
  /// The register this position forces, for an instruction whose encoding
  /// leaves the operand implicit (**L§15**).
  pin: Option<u32>,
  /// Whether the instruction writes this operand.
  writes: bool,
  /// The width the instruction uses a general-purpose operand at.
  scalar_bits: u32,
  /// The width it uses a vector operand at.
  vector_bits: u32,
}

impl At {
  /// The width an operand of this class is used at here. A memory operand's
  /// base and index are addresses, so they are always the full width.
  fn bits_for(&self, class: AsmClass, by_reference: bool) -> u32 {
    match (class, by_reference) {
      (_, true) => 64,
      (AsmClass::Vec, _) => self.vector_bits,
      _ => self.scalar_bits,
    }
  }
}

/// The operands of one block, and what each name stands for.
#[derive(Default)]
struct Operands {
  registers: Vec<Reg>,
  slots: Vec<Slot>,
  by_key: HashMap<Key, Place>,
}

/// What the back end binds an `#asm` block to, and what the block leaves for
/// the program to keep.
struct Bindings {
  inputs: Vec<AsmBinding>,
  outputs: Vec<AsmBinding>,
  /// The registers the block writes and nothing reads back.
  clobbers: Vec<String>,
  /// Where each output goes: the storage, the value the block produced, and
  /// the type that storage was declared as.
  stores: Vec<(ValueId, ValueId, TypeId)>,
}

/// What an operand is remembered by: the declaration a name stands for, so
/// that two mentions of one thing are one operand, and the node itself for an
/// operand that is not a plain name.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
  Decl(DeclId),
  Node(SourceId, NodeId),
}

/// A register an earlier `#asm` block in the same scope declared.
#[derive(Clone, Copy)]
pub(super) struct AsmRegisterState {
  class: AsmClass,
  /// The storage its value survives in. A vector register has none, so one
  /// declared in an earlier block is reported rather than read as garbage.
  local: Option<LocalId>,
  /// The block that declared it.
  block: NodeId,
}

impl Lowering<'_, '_> {
  /// Lowers one `#asm` block into the text the back end assembles, with the
  /// values its operands carry in and out (**L§15**).
  pub(super) fn asm(&mut self, source: SourceId, node: NodeId) {
    let scope = self.checker.scope_for(source, node, self.body_scope);
    let Some(ast) = self.checker.tree_of(source) else {
      return;
    };
    let NodeData::Asm(block) = ast.data(node) else {
      return;
    };
    let block: AsmNode = (**block).clone();

    let features: Vec<String> = block
      .features
      .iter()
      .map(|feature| self.text(*feature))
      .collect();
    let vex = features.iter().any(|name| name.starts_with("AVX"));
    let block_vector_bits = if features.iter().any(|name| name.starts_with("AVX512")) {
      512
    } else if vex {
      256
    } else {
      128
    };

    let mut operands = Operands::default();
    let mut resolved: Vec<Resolved> = Vec::new();
    for (index, instruction) in block.instructions.iter().enumerate() {
      let written = instruction
        .mnemonic
        .map(|mnemonic| self.text(mnemonic))
        .unwrap_or_default();
      let (form, locked) = match instruction.mnemonic {
        None => (&DECLARATION_FORM, false),
        Some(_) => match form_of(&written) {
          Some(found) => found,
          None => {
            self.error(
              source,
              node,
              format!("orangejuice has no encoding for the '#asm' instruction '{written}'."),
            );
            return;
          }
        },
      };
      let tagged = self.asm_size(scope, source, instruction.size);
      let scalar_bits = form.scalar_bits.or(tagged).unwrap_or(64);
      let vector_bits = match form.class {
        AsmClass::Vec => tagged.unwrap_or(block_vector_bits),
        _ => block_vector_bits,
      };

      let mut written_operands = Vec::new();
      let mut written_decorations: Vec<Decorations> = Vec::new();
      for (position, operand) in instruction.operands.iter().enumerate() {
        let at = At {
          scope,
          source,
          node,
          index,
          fresh: match position {
            0 => form.destination_class.unwrap_or(form.class),
            _ => form.class,
          },
          pin: form
            .pins
            .iter()
            .find(|(slot, _)| *slot == position)
            .map(|(_, register)| *register),
          writes: form.writes.contains(&position),
          scalar_bits,
          vector_bits,
        };
        let decoration = match self.asm_decorations(&at, operand, &mut operands) {
          Some(decoration) => decoration,
          None => return,
        };
        let Some(resolved) = self.asm_operand(&at, operand, &mut operands) else {
          return;
        };
        written_operands.push(resolved);
        written_decorations.push(decoration);
      }
      if instruction.mnemonic.is_some() {
        resolved.push(Resolved {
          form,
          written,
          scalar_bits,
          vector_bits,
          vex,
          locked,
          operands: written_operands,
          decorations: written_decorations,
        });
      }
    }

    if !self.allocate(source, node, &mut operands.registers) {
      return;
    }
    pin_to_registers(&resolved, &mut operands);
    self.number_operands(&mut operands);

    let mut text = String::new();
    for instruction in &resolved {
      let Some(line) = self.asm_text(source, node, instruction, &operands) else {
        return;
      };
      if !text.is_empty() {
        text.push('\n');
      }
      text.push_str(&line);
    }

    let bindings = self.bindings(&operands);
    self.emit(Inst::Asm {
      text,
      inputs: bindings.inputs,
      outputs: bindings.outputs,
      clobbers: bindings.clobbers,
    });
    // The instructions used each operand at their own width, so what comes
    // back is narrowed to what the variable was declared as (**L§15**).
    for (address, value, type_id) in bindings.stores {
      let wide = self.value_types[value.0 as usize];
      let value = self.asm_convert(value, wide, type_id);
      self.emit(Inst::Store { address, value });
    }
  }

  /// Gives every operand its place in the assembly's operand list. The
  /// outputs come first, because that is how LLVM numbers them: a `$n` in the
  /// text counts through the outputs and then the inputs.
  fn number_operands(&mut self, operands: &mut Operands) {
    let mut position = 0;
    let mut tied = 0;
    for slot in operands.slots.iter_mut() {
      if slot.writes_back() {
        slot.position = position;
        position += 1;
        tied += usize::from(slot.input.is_some());
      }
    }
    // A register the block declared is named in the text rather than written
    // as a `$n`, but it still takes a place in the list, and so does the input
    // tied to it.
    let registers = operands
      .registers
      .iter()
      .filter(|register| register.local.is_some())
      .count();
    position += registers + tied + registers;
    for slot in operands.slots.iter_mut() {
      if !slot.writes_back() && slot.input.is_some() {
        slot.position = position;
        position += 1;
      }
    }
  }

  /// The constraints the back end binds, in the order `number_operands` gave
  /// them, and the stores that put the block's results back where they came
  /// from.
  fn bindings(&mut self, operands: &Operands) -> Bindings {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut stores: Vec<(ValueId, ValueId, TypeId)> = Vec::new();
    let mut tied: Vec<(usize, ValueId)> = Vec::new();
    let mut clobbers: Vec<String> = Vec::new();

    for slot in &operands.slots {
      let Some((address, type_id)) = slot.written.then_some(slot.address).flatten() else {
        continue;
      };
      let wide = self.asm_width(type_id, slot.bits);
      let dest = self.value(wide);
      outputs.push(AsmBinding {
        constraint: format!("={}", self.constraint_of(slot)),
        value: dest,
      });
      stores.push((address, dest, type_id));
      if let Some(value) = slot.input {
        tied.push((outputs.len() - 1, self.asm_convert(value, type_id, wide)));
      }
    }
    for register in &operands.registers {
      let Some(local) = register.local else {
        // A register the block uses but keeps nothing in is still one the
        // procedure around it may not have left something live in.
        if let Some(assigned) = register.assigned {
          clobbers.push(constraint_name(register.class, Some(assigned)));
        }
        continue;
      };
      let address = self.local_address(local);
      let type_id = self.locals[local.0 as usize].type_id;
      let value = self.value(type_id);
      self.emit(Inst::Load {
        dest: value,
        address,
      });
      let dest = self.value(type_id);
      outputs.push(AsmBinding {
        constraint: format!("={}", constraint_name(register.class, register.assigned)),
        value: dest,
      });
      stores.push((address, dest, type_id));
      tied.push((outputs.len() - 1, value));
    }
    for (position, value) in tied {
      inputs.push(AsmBinding {
        constraint: position.to_string(),
        value,
      });
    }
    for slot in &operands.slots {
      if slot.writes_back() {
        continue;
      }
      if let Some(value) = slot.input {
        let from = self.value_types[value.0 as usize];
        let wide = self.asm_width(from, slot.bits);
        inputs.push(AsmBinding {
          constraint: self.constraint_of(slot),
          value: self.asm_convert(value, from, wide),
        });
      }
    }
    clobbers.sort();
    clobbers.dedup();
    Bindings {
      inputs,
      outputs,
      stores,
      clobbers,
    }
  }

  /// The type an operand travels in: an integer as wide as the instructions
  /// used it, and anything else — a pointer, a float — as it stands.
  fn asm_width(&mut self, type_id: TypeId, bits: u32) -> TypeId {
    let types = self.checker.types();
    let underlying = types.underlying(type_id);
    let signed = match types.kind(underlying) {
      oj_types::TypeKind::Integer(kind) => kind.is_signed(),
      oj_types::TypeKind::Bool => false,
      oj_types::TypeKind::Enum(id) => {
        let base = types.enum_info(*id).base;
        match types.kind(types.underlying(base)) {
          oj_types::TypeKind::Integer(kind) => kind.is_signed(),
          _ => false,
        }
      }
      _ => return type_id,
    };
    let kind = match (bits, signed) {
      (8, true) => oj_types::IntKind::S8,
      (8, false) => oj_types::IntKind::U8,
      (16, true) => oj_types::IntKind::S16,
      (16, false) => oj_types::IntKind::U16,
      (32, true) => oj_types::IntKind::S32,
      (32, false) => oj_types::IntKind::U32,
      (_, true) => oj_types::IntKind::S64,
      (_, false) => oj_types::IntKind::U64,
    };
    self.checker.types().integer(kind)
  }

  fn asm_convert(&mut self, value: ValueId, from: TypeId, to: TypeId) -> ValueId {
    if from == to {
      return value;
    }
    let types = self.checker.types();
    let (before, after) = (types.size_of(from), types.size_of(to));
    let (Some(before), Some(after)) = (before, after) else {
      return value;
    };
    let signed = matches!(
      types.kind(types.underlying(from)),
      oj_types::TypeKind::Integer(kind) if kind.is_signed()
    );
    let kind = match before.cmp(&after) {
      std::cmp::Ordering::Equal => return value,
      std::cmp::Ordering::Greater => ConvertKind::IntegerTruncate,
      std::cmp::Ordering::Less if signed => ConvertKind::IntegerSignExtend,
      std::cmp::Ordering::Less => ConvertKind::IntegerZeroExtend,
    };
    let dest = self.value(to);
    self.emit(Inst::Convert {
      dest,
      kind,
      operand: value,
    });
    dest
  }

  fn constraint_of(&self, slot: &Slot) -> String {
    match slot.pin {
      Some(pin) => constraint_name(slot.class, Some(pin)),
      // Anything the block did not pin is the back end's to place; only a
      // memory operand's base has to be a register (**L§15**).
      None if slot.register_only => String::from("r"),
      None => String::from("rm"),
    }
  }

  /// The EVEX mask and rounding an operand was written with (**L§15**). A
  /// mask names a register the block declared, so it is placed like any other.
  fn asm_decorations(
    &mut self,
    at: &At,
    operand: &AsmOperand,
    operands: &mut Operands,
  ) -> Option<Decorations> {
    let mut decorations = Decorations::default();
    if let Some(mask) = &operand.mask {
      let at = At {
        fresh: AsmClass::Omr,
        pin: None,
        writes: false,
        ..*at
      };
      let Place::Register(index) = self.asm_place(&at, mask.register, false, operands)? else {
        self.error(
          at.source,
          at.node,
          "An '#asm' mask has to name a register the block declared (**L§15**).",
        );
        return None;
      };
      decorations.mask = Some((index, mask.zeroing));
    }
    // A `!` on a memory operand is the broadcast, which the operand itself
    // carries; on a register one it is the rounding mode.
    if let Some(flag) = operand.flag
      && !matches!(operand.kind, AsmOperandKind::Memory(_))
    {
      decorations.rounding = Some(match flag {
        oj_syntax::ast::AsmFlag::Plain => None,
        oj_syntax::ast::AsmFlag::Rounding(mode) => Some(mode),
      });
    }
    Some(decorations)
  }

  fn asm_operand(
    &mut self,
    at: &At,
    operand: &AsmOperand,
    operands: &mut Operands,
  ) -> Option<Operand> {
    match &operand.kind {
      AsmOperandKind::Declaration(declaration) => {
        let index = self.asm_declaration(at, declaration, operands)?;
        Some(Operand::Register(index))
      }
      AsmOperandKind::Pin { name, register } => {
        let pin = self.pinned(at, *register)?;
        let at = At {
          pin: Some(pin),
          ..*at
        };
        Some(self.asm_place(&at, *name, false, operands)?.into())
      }
      AsmOperandKind::Expression(expression) => {
        let info = self.checker.expression(at.scope, at.source, *expression);
        if let Some(value) = info.constant.as_ref().and_then(immediate_of) {
          return Some(Operand::Immediate(value));
        }
        Some(self.asm_place(at, *expression, false, operands)?.into())
      }
      AsmOperandKind::Memory(memory) => {
        // What a memory operand names is an address, not the storage the
        // instruction writes.
        let at = &At {
          writes: false,
          ..*at
        };
        let base = self.asm_place(at, memory.base, memory.by_reference, operands)?;
        let index = match memory.index {
          Some(index) => Some(self.asm_place(at, index, false, operands)?),
          None => None,
        };
        // A base and an index are addresses: they sit in a register, whole.
        for slot in [Some(base), index].into_iter().flatten() {
          if let Place::Slot(slot) = slot {
            operands.slots[slot].register_only = true;
            operands.slots[slot].bits = 64;
          }
        }
        let scale = match memory.scale {
          Some(scale) => self.asm_constant(at, scale)? as u64,
          None => 1,
        };
        let displacement = match memory.displacement {
          Some(displacement) => {
            let value = self.asm_constant(at, displacement)?;
            match memory.displacement_is_negative {
              true => -value,
              false => value,
            }
          }
          None => 0,
        };
        Some(Operand::Memory {
          base,
          index,
          scale,
          displacement,
          broadcast: operand.flag.is_some(),
        })
      }
    }
  }

  /// `mov apple:, 10` names a register the block is introducing, or one an
  /// earlier block in the same scope already did (**L§15**).
  fn asm_declaration(
    &mut self,
    at: &At,
    declaration: &AsmDeclaration,
    operands: &mut Operands,
  ) -> Option<usize> {
    let pin = match declaration.register {
      Some(register) => Some(self.pinned(at, register)?),
      None => at.pin,
    };
    let class = declaration.class.unwrap_or(at.fresh);
    let decl = self.checker.decl_at(at.source, declaration.name);
    let key = match decl {
      Some(decl) => Key::Decl(decl),
      None => Key::Node(at.source, declaration.name),
    };
    if let Some(Place::Register(index)) = operands.by_key.get(&key).copied() {
      return self.pin_register(at, index, pin, operands);
    }
    let index = self.new_register(at, decl, class, pin, operands)?;
    operands.by_key.insert(key, Place::Register(index));
    Some(index)
  }

  fn new_register(
    &mut self,
    at: &At,
    decl: Option<DeclId>,
    class: AsmClass,
    pin: Option<u32>,
    operands: &mut Operands,
  ) -> Option<usize> {
    let existing = decl.and_then(|decl| self.asm_registers.get(&self.local_key(decl)).copied());
    let (class, local) = match existing {
      Some(state) if state.block != at.node => match state.local {
        Some(local) => (state.class, Some(local)),
        None => {
          self.error(
            at.source,
            at.node,
            "orangejuice keeps only a general-purpose register alive between two '#asm' \
             blocks; this one was declared in an earlier block.",
          );
          return None;
        }
      },
      _ => {
        // A general-purpose register keeps its value between blocks in storage
        // of its own, which is the spilling the reference describes
        // (**L§15**).
        let local = match class {
          AsmClass::Gpr => Some(self.new_local(String::from("asm"), TypeId::S64)),
          // A vector register survives between blocks as the 128 bits it is;
          // the wider ones have no type of their own to keep them in yet.
          AsmClass::Vec if at.vector_bits == 128 => {
            Some(self.new_local(String::from("asm"), TypeId::V128))
          }
          _ => None,
        };
        if let Some(decl) = decl {
          let key = self.local_key(decl);
          self.asm_registers.insert(
            key,
            AsmRegisterState {
              class,
              local,
              block: at.node,
            },
          );
        }
        (class, local)
      }
    };
    let index = operands.registers.len();
    operands.registers.push(Reg {
      class,
      pin,
      local,
      first: at.index,
      last: at.index,
      assigned: None,
    });
    Some(index)
  }

  /// A name an operand wrote: a register an earlier statement declared, or a
  /// value of the program around the block, which becomes an operand of the
  /// assembly and is written back when the block ends (**L§15**).
  fn asm_place(
    &mut self,
    at: &At,
    expression: NodeId,
    by_reference: bool,
    operands: &mut Operands,
  ) -> Option<Place> {
    let key = self.asm_key_of(at, expression);
    let key = self.asm_alias(key);
    if let Some(place) = operands.by_key.get(&key).copied() {
      return match place {
        Place::Register(index) => {
          self.pin_register(at, index, at.pin, operands)?;
          Some(place)
        }
        Place::Slot(index) => {
          let slot = &mut operands.slots[index];
          if let Some(pin) = at.pin {
            slot.pin = Some(pin);
          }
          slot.written |= at.writes;
          slot.bits = slot.bits.max(at.bits_for(slot.class, by_reference));
          Some(place)
        }
      };
    }
    // A name a preceding `#asm` block declared is a register, not storage.
    if let Key::Decl(decl) = key
      && self.checker.program().tree().decl(decl).kind == DeclKind::AsmRegister
    {
      let index = self.new_register(at, Some(decl), at.fresh, at.pin, operands)?;
      operands.by_key.insert(key, Place::Register(index));
      return Some(Place::Register(index));
    }

    let value = self.expression(at.scope, at.source, expression, None)?;
    let (input, address) = match by_reference {
      // `[*p + 8]` addresses the value rather than reading it (**L§15**).
      true => (self.address_of(value), None),
      false => {
        if !self.is_scalar(value.type_id) {
          self.error(
            at.source,
            at.node,
            "An '#asm' operand has to fit in a register; write '*' before it to address \
             it instead (**L§15**).",
          );
          return None;
        }
        let address = value.indirect.then_some((value.id, value.type_id));
        (self.scalar(value), address)
      }
    };
    let class = match self.checker.types().kind(value.type_id) {
      oj_types::TypeKind::Float(_) => AsmClass::Vec,
      _ => AsmClass::Gpr,
    };
    let index = operands.slots.len();
    operands.slots.push(Slot {
      input: Some(input),
      address,
      written: at.writes,
      pin: at.pin,
      class,
      register_only: false,
      bits: at.bits_for(class, by_reference),
      position: 0,
    });
    operands.by_key.insert(key, Place::Slot(index));
    Some(Place::Slot(index))
  }

  /// A macro parameter of type `__reg` is the caller's register (**L§15**).
  fn asm_alias(&self, key: Key) -> Key {
    let mut key = key;
    for _ in 0..MAX_ALIAS_STEPS {
      let Key::Decl(decl) = key else {
        return key;
      };
      let Some(target) = self
        .asm_register_aliases
        .get(&self.local_key(decl))
        .copied()
      else {
        return key;
      };
      key = Key::Decl(target);
    }
    key
  }

  fn asm_key_of(&mut self, at: &At, expression: NodeId) -> Key {
    let info = self.checker.expression(at.scope, at.source, expression);
    match info.overloads[..] {
      [only] => Key::Decl(only),
      _ => Key::Node(at.source, expression),
    }
  }

  fn pin_register(
    &mut self,
    at: &At,
    index: usize,
    pin: Option<u32>,
    operands: &mut Operands,
  ) -> Option<usize> {
    let register = &mut operands.registers[index];
    register.last = register.last.max(at.index);
    if let Some(pin) = pin {
      match register.pin {
        Some(existing) if existing != pin => {
          self.error(
            at.source,
            at.node,
            "This '#asm' operand is pinned to two different registers.",
          );
          return None;
        }
        _ => register.pin = Some(pin),
      }
    }
    Some(index)
  }

  fn pinned(&mut self, at: &At, register: AsmRegister) -> Option<u32> {
    match register {
      AsmRegister::Numbered(index) => Some(index),
      AsmRegister::Named(symbol) => {
        let name = self.text(symbol);
        match pinned_register(&name) {
          Some(index) => Some(index),
          None => {
            self.error(
              at.source,
              at.node,
              format!("'{name}' does not name a register an '#asm' block can pin to."),
            );
            None
          }
        }
      }
    }
  }

  fn asm_constant(&mut self, at: &At, expression: NodeId) -> Option<i64> {
    let info = self.checker.expression(at.scope, at.source, expression);
    match info.constant.as_ref().and_then(immediate_of) {
      Some(value) => Some(value as i64),
      None => {
        self.error(
          at.source,
          at.node,
          "The scale and the displacement of an '#asm' memory operand have to be \
           constants (**L§15**).",
        );
        None
      }
    }
  }

  /// The size a mnemonic was tagged with, in bits. `?T` is a type's width or a
  /// constant integer's value (**L§15**).
  fn asm_size(&mut self, scope: ScopeId, source: SourceId, size: AsmSize) -> Option<u32> {
    match size {
      AsmSize::Inferred => None,
      AsmSize::Bits(bits) => Some(bits),
      AsmSize::Of(expression) => {
        let info = self.checker.expression(scope, source, expression);
        if let Some(denoted) = info.denoted {
          return self
            .checker
            .layout(denoted)
            .map(|layout| layout.size as u32 * 8);
        }
        match info.constant.map(|constant| constant.value) {
          Some(oj_sema::Value::Int(bits)) => Some(bits as u32),
          _ => None,
        }
      }
    }
  }

  /// Places every register the block declared: pinned ones where they were
  /// pinned, the rest by lifetime, sharing a register where two lifetimes do
  /// not overlap. Nothing is spilled — running out is an error (**L§15**).
  fn allocate(&mut self, source: SourceId, node: NodeId, registers: &mut [Reg]) -> bool {
    let mut held: Vec<(AsmClass, u32, usize, usize)> = Vec::new();
    let mut order: Vec<usize> = (0..registers.len()).collect();
    order.sort_by_key(|index| {
      (
        registers[*index].pin.is_none(),
        registers[*index].first,
        *index,
      )
    });

    for index in order {
      let register = &registers[index];
      if register.class == AsmClass::Str {
        self.error(
          source,
          node,
          "orangejuice does not allocate the x87 stack registers of an '#asm' block.",
        );
        return false;
      }
      // A register that survives to the next block carries a value across the
      // whole of this one.
      let (first, last) = match register.local.is_some() {
        true => (0, usize::MAX),
        false => (register.first, register.last),
      };
      let (class, pin) = (register.class, register.pin);
      let assigned = match pin {
        Some(pin) => pin,
        None => {
          let free = pool(class).iter().copied().find(|candidate| {
            !held.iter().any(|(other, taken, from, to)| {
              *other == class && taken == candidate && *from <= last && first <= *to
            })
          });
          match free {
            Some(register) => register,
            None => {
              self.error(
                source,
                node,
                "There are too many registers alive in this '#asm' block; nothing is \
                 spilled to the stack (**L§15**).",
              );
              return false;
            }
          }
        }
      };
      held.push((class, assigned, first, last));
      registers[index].assigned = Some(assigned);
    }
    true
  }

  /// One instruction, as the assembler reads it.
  fn asm_text(
    &mut self,
    source: SourceId,
    node: NodeId,
    instruction: &Resolved,
    operands: &Operands,
  ) -> Option<String> {
    let form = instruction.form;
    let rotate_bytes = form.text == "bswap" && instruction.scalar_bits == 16;
    let mut text = String::new();
    if instruction.locked {
      text.push_str("lock ");
    }
    if form.text.is_empty() {
      let mnemonic = self.widened_mnemonic(source, node, instruction)?;
      text.push_str(&mnemonic);
    } else {
      if form.vex && instruction.vex {
        text.push('v');
      }
      // `bswap` has no 16-bit form the integrated assembler will take, and a
      // 16-bit byte swap *is* a rotate by eight, which is what the reference
      // means by emulating it (**L§15**).
      match rotate_bytes {
        true => text.push_str("rol"),
        false => text.push_str(form.text),
      }
      // AVX512 has no untyped `vmovdqu`: a 512-bit move names the lane width it
      // moves in (**L§15**).
      if instruction.vector_bits == 512 && matches!(form.text, "movdqu" | "movdqa") {
        text.push_str("64");
      }
    }

    let positions: Vec<usize> = match form.operands {
      Ops::None => Vec::new(),
      Ops::All => (0..instruction.operands.len()).collect(),
      Ops::Pick(picked) => picked
        .iter()
        .copied()
        .filter(|position| *position < instruction.operands.len())
        .collect(),
    };
    let mut rounding: Option<&str> = None;
    for (written, position) in positions.into_iter().enumerate() {
      text.push_str(if written == 0 { " " } else { ", " });
      match &instruction.operands[position] {
        Operand::Immediate(value) => {
          let _ = write!(text, "{value}");
        }
        Operand::Register(index) => {
          let register = &operands.registers[*index];
          let bits = match register.class {
            AsmClass::Vec => instruction.vector_bits,
            _ if form.byte_operands.contains(&position) => 8,
            _ => instruction.scalar_bits,
          };
          text.push_str(&register_name(register.class, register.assigned, bits));
        }
        Operand::Slot(index) => {
          let used = match form.byte_operands.contains(&position) {
            true => 8,
            false => instruction.bits_for(operands.slots[*index].class),
          };
          text.push_str(&slot_name(operands, *index, used));
        }
        Operand::Memory {
          base,
          index,
          scale,
          displacement,
          broadcast,
        } => {
          if !form.bare_memory {
            // A broadcast reads one lane, so what it names is that wide.
            let bits = match broadcast {
              true => form.element_bits,
              false => form.memory_bits.unwrap_or(match form.class {
                AsmClass::Vec => instruction.vector_bits,
                _ => instruction.scalar_bits,
              }),
            };
            text.push_str(memory_size(bits));
          }
          text.push('[');
          text.push_str(&place_name(operands, *base, 64, instruction));
          if let Some(index) = index {
            let _ = write!(
              text,
              " + {}*{scale}",
              place_name(operands, *index, 64, instruction)
            );
          }
          match displacement.cmp(&0) {
            std::cmp::Ordering::Greater => {
              let _ = write!(text, " + {displacement}");
            }
            std::cmp::Ordering::Less => {
              let _ = write!(text, " - {}", -displacement);
            }
            std::cmp::Ordering::Equal => {}
          }
          text.push(']');
          if *broadcast {
            let lanes = instruction.vector_bits / form.element_bits.max(1);
            let _ = write!(text, "{{1to{lanes}}}");
          }
        }
      }
      if let Some(decorations) = instruction.decorations.get(position) {
        if let Some((index, zeroing)) = decorations.mask {
          let register = &operands.registers[index];
          let _ = write!(
            text,
            " {{{}}}",
            register_name(register.class, register.assigned, 0)
          );
          if zeroing {
            text.push_str(" {z}");
          }
        }
        if let Some(mode) = decorations.rounding {
          rounding = Some(match mode {
            None => "{sae}",
            Some(oj_syntax::ast::RoundingMode::Nearest) => "{rn-sae}",
            Some(oj_syntax::ast::RoundingMode::Down) => "{rd-sae}",
            Some(oj_syntax::ast::RoundingMode::Up) => "{ru-sae}",
            Some(oj_syntax::ast::RoundingMode::Zero) => "{rz-sae}",
          });
        }
      }
    }
    if rotate_bytes {
      text.push_str(", 8");
    }
    // The embedded rounding an instruction was told to use is an operand of
    // its own, after the ones it works on (**L§15**).
    if let Some(rounding) = rounding {
      let _ = write!(text, ", {rounding}");
    }
    Some(text)
  }

  /// The mnemonics that carry their widths rather than take them from the
  /// operands: `movzxbw` widens a byte to a word, `rep_movs.q` moves
  /// quadwords.
  fn widened_mnemonic(
    &mut self,
    source: SourceId,
    node: NodeId,
    instruction: &Resolved,
  ) -> Option<String> {
    let written = instruction.written.as_str();
    if written.len() == 7 && (written.starts_with("movzx") || written.starts_with("movsx")) {
      return Some(written[..5].to_string());
    }
    if written == "rep_movs" {
      return Some(format!(
        "rep movs{}",
        match instruction.scalar_bits {
          8 => "b",
          16 => "w",
          32 => "d",
          _ => "q",
        }
      ));
    }
    self.error(
      source,
      node,
      format!("orangejuice has no encoding for the '#asm' instruction '{written}'."),
    );
    None
  }
}

/// x86 lets one operand of an instruction be in memory, so a value the block
/// left to the back end may only be addressed on the stack when it is the only
/// Which of an instruction's operands are actually written out, which is what
/// the assembler sees: a form that picks writes only what it picked.
fn written_positions(instruction: &Resolved) -> Vec<usize> {
  match instruction.form.operands {
    Ops::None => Vec::new(),
    Ops::All => (0..instruction.operands.len()).collect(),
    Ops::Pick(picked) => picked
      .iter()
      .copied()
      .filter(|position| *position < instruction.operands.len())
      .collect(),
  }
}

/// one of its instruction that could be. Everything else has to be a register.
fn pin_to_registers(resolved: &[Resolved], operands: &mut Operands) {
  for instruction in resolved {
    let direct: Vec<usize> = instruction
      .operands
      .iter()
      .filter_map(|operand| match operand {
        Operand::Slot(index) => Some(*index),
        _ => None,
      })
      .collect();
    let addressed = instruction
      .operands
      .iter()
      .any(|operand| matches!(operand, Operand::Memory { .. }));
    for index in direct.iter().copied() {
      // An operand the instructions use at two widths is written with a size
      // modifier, which only a register operand takes.
      let slot = &mut operands.slots[index];
      if slot.bits != instruction.bits_for(slot.class) {
        slot.register_only = true;
      }
    }
    // An instruction whose *written* operands are all slots and immediates has
    // no register to take its width from, so x86 cannot tell how wide a memory
    // operand would be: `imul [rsp + 48]` is ambiguous where `imul rcx` is not
    // (**L§15**). The implicit operands of a `mul` or a `div` are not written,
    // which is what `Basic`'s `ConvertToApollo` runs into.
    let has_register = written_positions(instruction).into_iter().any(|position| {
      matches!(
        instruction.operands.get(position),
        Some(Operand::Register(_))
      )
    });
    if !addressed && direct.len() < 2 && has_register {
      continue;
    }
    for index in direct {
      operands.slots[index].register_only = true;
    }
  }
}

impl From<Place> for Operand {
  fn from(place: Place) -> Self {
    match place {
      Place::Register(index) => Operand::Register(index),
      Place::Slot(index) => Operand::Slot(index),
    }
  }
}

fn place_name(operands: &Operands, place: Place, bits: u32, instruction: &Resolved) -> String {
  match place {
    Place::Register(index) => {
      let register = &operands.registers[index];
      let bits = match register.class {
        AsmClass::Vec => instruction.vector_bits,
        _ => bits,
      };
      register_name(register.class, register.assigned, bits)
    }
    // A base and an index are addresses, whatever width the instruction
    // works at.
    Place::Slot(index) => slot_name(operands, index, bits),
  }
}

/// An operand of the assembly, with the size modifier that says which part of
/// it this instruction means when the block uses it at more than one width.
fn slot_name(operands: &Operands, index: usize, used: u32) -> String {
  let slot = &operands.slots[index];
  if used == slot.bits {
    return format!("${}", slot.position);
  }
  let modifier = match used {
    8 => "b",
    16 => "w",
    32 => "k",
    _ => "q",
  };
  format!("${{{}:{modifier}}}", slot.position)
}

impl Resolved {
  fn bits_for(&self, class: AsmClass) -> u32 {
    match class {
      AsmClass::Vec => self.vector_bits,
      _ => self.scalar_bits,
    }
  }
}

fn immediate_of(constant: &oj_sema::Const) -> Option<i128> {
  match &constant.value {
    oj_sema::Value::Int(value) => Some(*value),
    oj_sema::Value::Bool(value) => Some(i128::from(*value)),
    // A float immediate is opaque to the instruction, so what travels is its
    // bit pattern (**L§15**).
    oj_sema::Value::Float(value) => Some(i128::from(value.to_bits())),
    _ => None,
  }
}

/// Deep enough for a macro that passes a register on to another macro.
const MAX_ALIAS_STEPS: usize = 8;

const GPR_NAMES: [[&str; 4]; 16] = [
  ["al", "ax", "eax", "rax"],
  ["cl", "cx", "ecx", "rcx"],
  ["dl", "dx", "edx", "rdx"],
  ["bl", "bx", "ebx", "rbx"],
  ["spl", "sp", "esp", "rsp"],
  ["bpl", "bp", "ebp", "rbp"],
  ["sil", "si", "esi", "rsi"],
  ["dil", "di", "edi", "rdi"],
  ["r8b", "r8w", "r8d", "r8"],
  ["r9b", "r9w", "r9d", "r9"],
  ["r10b", "r10w", "r10d", "r10"],
  ["r11b", "r11w", "r11d", "r11"],
  ["r12b", "r12w", "r12d", "r12"],
  ["r13b", "r13w", "r13d", "r13"],
  ["r14b", "r14w", "r14d", "r14"],
  ["r15b", "r15w", "r15d", "r15"],
];

/// The registers a block may take without being told to. `rsp` and `rbp` are
/// left out: an `#asm` block may not move the stack pointer (**L§15**), and
/// the frame pointer belongs to the procedure around it.
const GPR_POOL: [u32; 14] = [0, 1, 2, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const VEC_POOL: [u32; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
/// `k0` cannot be a write mask, so the allocator does not hand it out.
const OMR_POOL: [u32; 7] = [1, 2, 3, 4, 5, 6, 7];

fn pool(class: AsmClass) -> &'static [u32] {
  match class {
    AsmClass::Vec => &VEC_POOL,
    AsmClass::Omr => &OMR_POOL,
    _ => &GPR_POOL,
  }
}

fn constraint_name(class: AsmClass, index: Option<u32>) -> String {
  let index = index.unwrap_or(0);
  match class {
    AsmClass::Vec => format!("{{xmm{index}}}"),
    AsmClass::Omr => format!("{{k{index}}}"),
    _ => format!("{{{}}}", GPR_NAMES[(index as usize).min(15)][3]),
  }
}

fn register_name(class: AsmClass, index: Option<u32>, bits: u32) -> String {
  let index = index.unwrap_or(0);
  match class {
    AsmClass::Vec => match bits {
      512 => format!("zmm{index}"),
      256 => format!("ymm{index}"),
      _ => format!("xmm{index}"),
    },
    AsmClass::Omr => format!("k{index}"),
    _ => {
      let slot = match bits {
        8 => 0,
        16 => 1,
        32 => 2,
        _ => 3,
      };
      GPR_NAMES[(index as usize).min(15)][slot].to_string()
    }
  }
}

/// `a b c d si di sp bp` name the first eight general-purpose registers in the
/// order the encoding gives them (**L§15**).
fn pinned_register(name: &str) -> Option<u32> {
  Some(match name {
    "a" => 0,
    "c" => 1,
    "d" => 2,
    "b" => 3,
    "sp" => 4,
    "bp" => 5,
    "si" => 6,
    "di" => 7,
    _ => return None,
  })
}

fn memory_size(bits: u32) -> &'static str {
  match bits {
    8 => "byte ptr ",
    16 => "word ptr ",
    32 => "dword ptr ",
    128 => "xmmword ptr ",
    256 => "ymmword ptr ",
    512 => "zmmword ptr ",
    _ => "qword ptr ",
  }
}

/// How a mnemonic reaches the assembler.
struct Form {
  /// The text to write, empty when the widths are spelled into the mnemonic.
  text: &'static str,
  operands: Ops,
  /// The class a declaration written in this instruction takes.
  class: AsmClass,
  /// The class of the first operand, when the instruction writes its result
  /// somewhere other than its own class.
  destination_class: Option<AsmClass>,
  /// The width a general-purpose operand has, when the instruction fixes it.
  scalar_bits: Option<u32>,
  /// The width of a memory operand, when the instruction fixes it.
  memory_bits: Option<u32>,
  /// The width of one lane, which is what a broadcast memory operand repeats.
  element_bits: u32,
  /// The registers the encoding leaves implicit, which the block has to pin
  /// (**L§15**).
  pins: &'static [(usize, u32)],
  /// The operands the instruction writes, which are the ones whose values go
  /// back to the program when the block ends.
  writes: &'static [usize],
  /// The mnemonic takes a `v` prefix where the block's feature set has VEX.
  vex: bool,
  /// A memory operand is written without a width.
  bare_memory: bool,
  /// The operands the encoding always writes eight bits wide, whatever the
  /// instruction works at: the shift count of a `shl`, `sar`, `shld` or
  /// `shrd` is `cl` and nothing else (**L§15**).
  byte_operands: &'static [usize],
}

enum Ops {
  All,
  None,
  Pick(&'static [usize]),
}

const fn form(text: &'static str, operands: Ops, class: AsmClass) -> Form {
  Form {
    text,
    operands,
    class,
    destination_class: None,
    scalar_bits: None,
    memory_bits: None,
    element_bits: 32,
    pins: &[],
    writes: &[0],
    vex: false,
    bare_memory: false,
    byte_operands: &[],
  }
}

/// An instruction that leaves every operand as it found it.
const fn reads(text: &'static str, operands: Ops, class: AsmClass) -> Form {
  Form {
    writes: &[],
    ..form(text, operands, class)
  }
}

const fn implicit(
  text: &'static str,
  operands: Ops,
  class: AsmClass,
  pins: &'static [(usize, u32)],
) -> Form {
  Form {
    pins,
    ..form(text, operands, class)
  }
}

const fn sized(text: &'static str, operands: Ops, class: AsmClass, scalar_bits: u32) -> Form {
  Form {
    scalar_bits: Some(scalar_bits),
    ..form(text, operands, class)
  }
}

const fn vector(text: &'static str, operands: Ops) -> Form {
  Form {
    vex: true,
    ..form(text, operands, AsmClass::Vec)
  }
}

const DECLARATION_FORM: Form = reads("", Ops::None, AsmClass::Gpr);

/// The instructions orangejuice knows how to write out. A mnemonic that is not
/// here is reported rather than guessed at, so an `#asm` block is never
/// assembled into something it did not say.
#[rustfmt::skip]
static FORMS: &[(&str, Form)] = &[
  // Data movement and arithmetic on general-purpose registers.
  ("mov", form("mov", Ops::All, AsmClass::Gpr)),
  ("movbe", form("movbe", Ops::All, AsmClass::Gpr)),
  ("lea", Form { bare_memory: true, ..form("lea", Ops::All, AsmClass::Gpr) }),
  ("xchg", Form { writes: &[0, 1], ..form("xchg", Ops::All, AsmClass::Gpr) }),
  ("xadd", Form { writes: &[0, 1], ..form("xadd", Ops::All, AsmClass::Gpr) }),
  ("add", form("add", Ops::All, AsmClass::Gpr)),
  ("adc", form("adc", Ops::All, AsmClass::Gpr)),
  ("sub", form("sub", Ops::All, AsmClass::Gpr)),
  ("sbb", form("sbb", Ops::All, AsmClass::Gpr)),
  ("and", form("and", Ops::All, AsmClass::Gpr)),
  ("or", form("or", Ops::All, AsmClass::Gpr)),
  ("xor", form("xor", Ops::All, AsmClass::Gpr)),
  ("cmp", reads("cmp", Ops::All, AsmClass::Gpr)),
  ("test", reads("test", Ops::All, AsmClass::Gpr)),
  ("neg", form("neg", Ops::All, AsmClass::Gpr)),
  ("not", form("not", Ops::All, AsmClass::Gpr)),
  ("inc", form("inc", Ops::All, AsmClass::Gpr)),
  ("dec", form("dec", Ops::All, AsmClass::Gpr)),
  ("bswap", form("bswap", Ops::All, AsmClass::Gpr)),
  ("shl", Form { byte_operands: &[1], ..implicit("shl", Ops::All, AsmClass::Gpr, &[(1, 1)]) }),
  ("shr", Form { byte_operands: &[1], ..implicit("shr", Ops::All, AsmClass::Gpr, &[(1, 1)]) }),
  ("sar", Form { byte_operands: &[1], ..implicit("sar", Ops::All, AsmClass::Gpr, &[(1, 1)]) }),
  ("rol", Form { byte_operands: &[1], ..implicit("rol", Ops::All, AsmClass::Gpr, &[(1, 1)]) }),
  ("ror", Form { byte_operands: &[1], ..implicit("ror", Ops::All, AsmClass::Gpr, &[(1, 1)]) }),
  ("shld", Form { byte_operands: &[2], ..implicit("shld", Ops::All, AsmClass::Gpr, &[(2, 1)]) }),
  ("shrd", Form { byte_operands: &[2], ..implicit("shrd", Ops::All, AsmClass::Gpr, &[(2, 1)]) }),
  ("bt", reads("bt", Ops::All, AsmClass::Gpr)),
  ("bts", form("bts", Ops::All, AsmClass::Gpr)),
  ("btr", form("btr", Ops::All, AsmClass::Gpr)),
  ("btc", form("btc", Ops::All, AsmClass::Gpr)),
  ("bsf", form("bsf", Ops::All, AsmClass::Gpr)),
  ("bsr", form("bsr", Ops::All, AsmClass::Gpr)),
  ("popcnt", form("popcnt", Ops::All, AsmClass::Gpr)),
  ("lzcnt", form("lzcnt", Ops::All, AsmClass::Gpr)),
  ("tzcnt", form("tzcnt", Ops::All, AsmClass::Gpr)),
  ("rdrand", form("rdrand", Ops::All, AsmClass::Gpr)),
  ("rdseed", form("rdseed", Ops::All, AsmClass::Gpr)),
  // `op1:op2 = op2 * op3` lists every register the encoding uses, and the two
  // it leaves implicit are pinned (**L§15**).
  ("mul", Form { writes: &[0, 1], ..implicit("mul", Ops::Pick(&[2]), AsmClass::Gpr, &[(0, 2), (1, 0)]) }),
  ("imul", Form { writes: &[0, 1], ..implicit("imul", Ops::Pick(&[2]), AsmClass::Gpr, &[(0, 2), (1, 0)]) }),
  ("div", Form { writes: &[0, 1], ..implicit("div", Ops::Pick(&[2]), AsmClass::Gpr, &[(0, 2), (1, 0)]) }),
  ("idiv", Form { writes: &[0, 1], ..implicit("idiv", Ops::Pick(&[2]), AsmClass::Gpr, &[(0, 2), (1, 0)]) }),
  ("cmpxchg", Form { writes: &[0, 1], ..implicit("cmpxchg", Ops::Pick(&[1, 2]), AsmClass::Gpr, &[(0, 0)]) }),
  ("cmpxchg16b", Form {
    memory_bits: Some(128),
    writes: &[0, 1],
    ..implicit("cmpxchg16b", Ops::Pick(&[2]), AsmClass::Gpr, &[(0, 2), (1, 0), (3, 1), (4, 3)])
  }),
  // Whole-instruction forms: everything they touch is pinned or implicit.
  ("syscall", Form { writes: &[0, 1, 2], ..implicit("syscall", Ops::None, AsmClass::Gpr, &[(0, 1), (1, 11)]) }),
  ("cpuid", Form { writes: &[0, 1, 2, 3], ..implicit("cpuid", Ops::None, AsmClass::Gpr, &[(0, 0), (1, 3), (2, 1), (3, 2)]) }),
  ("rdtsc", Form { writes: &[0, 1], ..implicit("rdtsc", Ops::None, AsmClass::Gpr, &[(0, 2), (1, 0)]) }),
  ("rdtscp", Form { writes: &[0, 1, 2], ..implicit("rdtscp", Ops::None, AsmClass::Gpr, &[(0, 2), (1, 0), (2, 1)]) }),
  ("rep_movs", Form { writes: &[0, 1, 2], ..implicit("", Ops::None, AsmClass::Gpr, &[(0, 7), (1, 6), (2, 1)]) }),
  ("int3", form("int3", Ops::None, AsmClass::Gpr)),
  ("int", reads("int", Ops::All, AsmClass::Gpr)),
  ("pause", form("pause", Ops::None, AsmClass::Gpr)),
  ("nop", form("nop", Ops::None, AsmClass::Gpr)),
  ("hlt", form("hlt", Ops::None, AsmClass::Gpr)),
  ("ud2", form("ud2", Ops::None, AsmClass::Gpr)),
  ("cld", form("cld", Ops::None, AsmClass::Gpr)),
  ("std", form("std", Ops::None, AsmClass::Gpr)),
  ("cqo", form("cqo", Ops::None, AsmClass::Gpr)),
  ("cdq", form("cdq", Ops::None, AsmClass::Gpr)),
  ("sfence", form("sfence", Ops::None, AsmClass::Gpr)),
  ("lfence", form("lfence", Ops::None, AsmClass::Gpr)),
  ("mfence", form("mfence", Ops::None, AsmClass::Gpr)),
  // The condition-code setters always write a byte.
  ("seta", sized("seta", Ops::All, AsmClass::Gpr, 8)),
  ("setae", sized("setae", Ops::All, AsmClass::Gpr, 8)),
  ("setb", sized("setb", Ops::All, AsmClass::Gpr, 8)),
  ("setbe", sized("setbe", Ops::All, AsmClass::Gpr, 8)),
  ("setc", sized("setc", Ops::All, AsmClass::Gpr, 8)),
  ("sete", sized("sete", Ops::All, AsmClass::Gpr, 8)),
  ("setg", sized("setg", Ops::All, AsmClass::Gpr, 8)),
  ("setge", sized("setge", Ops::All, AsmClass::Gpr, 8)),
  ("setl", sized("setl", Ops::All, AsmClass::Gpr, 8)),
  ("setle", sized("setle", Ops::All, AsmClass::Gpr, 8)),
  ("setna", sized("setna", Ops::All, AsmClass::Gpr, 8)),
  ("setnc", sized("setnc", Ops::All, AsmClass::Gpr, 8)),
  ("setne", sized("setne", Ops::All, AsmClass::Gpr, 8)),
  ("setno", sized("setno", Ops::All, AsmClass::Gpr, 8)),
  ("setns", sized("setns", Ops::All, AsmClass::Gpr, 8)),
  ("setnz", sized("setnz", Ops::All, AsmClass::Gpr, 8)),
  ("seto", sized("seto", Ops::All, AsmClass::Gpr, 8)),
  ("sets", sized("sets", Ops::All, AsmClass::Gpr, 8)),
  ("setz", sized("setz", Ops::All, AsmClass::Gpr, 8)),
  // Prefetches name a location and nothing else.
  ("prefetcht0", Form { bare_memory: true, writes: &[], ..form("prefetcht0", Ops::All, AsmClass::Gpr) }),
  ("prefetcht1", Form { bare_memory: true, writes: &[], ..form("prefetcht1", Ops::All, AsmClass::Gpr) }),
  ("prefetcht2", Form { bare_memory: true, writes: &[], ..form("prefetcht2", Ops::All, AsmClass::Gpr) }),
  ("prefetchnta", Form { bare_memory: true, writes: &[], ..form("prefetchnta", Ops::All, AsmClass::Gpr) }),
  // Vector registers.
  ("movdqu", vector("movdqu", Ops::All)),
  ("movdqa", vector("movdqa", Ops::All)),
  ("movups", vector("movups", Ops::All)),
  ("movaps", vector("movaps", Ops::All)),
  ("movupd", vector("movupd", Ops::All)),
  ("movapd", vector("movapd", Ops::All)),
  ("movss", vector("movss", Ops::All)),
  ("movsd", vector("movsd", Ops::All)),
  ("movd", Form { scalar_bits: Some(32), memory_bits: Some(32), ..vector("movd", Ops::All) }),
  ("movq", Form { scalar_bits: Some(64), memory_bits: Some(64), ..vector("movq", Ops::All) }),
  ("pxor", vector("pxor", Ops::All)),
  ("pand", vector("pand", Ops::All)),
  ("pandn", vector("pandn", Ops::All)),
  ("por", vector("por", Ops::All)),
  ("paddb", vector("paddb", Ops::All)),
  ("paddw", vector("paddw", Ops::All)),
  ("paddd", vector("paddd", Ops::All)),
  ("paddq", vector("paddq", Ops::All)),
  ("psubb", vector("psubb", Ops::All)),
  ("psubw", vector("psubw", Ops::All)),
  ("psubd", vector("psubd", Ops::All)),
  ("psubq", vector("psubq", Ops::All)),
  ("pcmpeqb", vector("pcmpeqb", Ops::All)),
  ("pcmpeqw", vector("pcmpeqw", Ops::All)),
  ("pcmpeqd", vector("pcmpeqd", Ops::All)),
  ("pcmpeqq", vector("pcmpeqq", Ops::All)),
  ("pcmpgtb", vector("pcmpgtb", Ops::All)),
  ("pcmpgtd", vector("pcmpgtd", Ops::All)),
  ("pshufb", vector("pshufb", Ops::All)),
  ("pshufd", vector("pshufd", Ops::All)),
  ("palignr", vector("palignr", Ops::All)),
  ("punpcklbw", vector("punpcklbw", Ops::All)),
  ("punpcklqdq", vector("punpcklqdq", Ops::All)),
  ("aesdec", vector("aesdec", Ops::All)),
  ("aesenc", vector("aesenc", Ops::All)),
  ("aesdeclast", vector("aesdeclast", Ops::All)),
  ("aesenclast", vector("aesenclast", Ops::All)),
  ("cvtps2dq", vector("cvtps2dq", Ops::All)),
  ("cvtdq2ps", vector("cvtdq2ps", Ops::All)),
  ("addps", vector("addps", Ops::All)),
  ("subps", vector("subps", Ops::All)),
  ("mulps", vector("mulps", Ops::All)),
  ("divps", vector("divps", Ops::All)),
  ("xorps", vector("xorps", Ops::All)),
  ("andps", vector("andps", Ops::All)),
  ("sqrtps", vector("sqrtps", Ops::All)),
  ("ptest", Form { writes: &[], ..vector("ptest", Ops::All) }),
  // A vector instruction whose result lands in a general-purpose register.
  ("pmovmskb", Form {
    destination_class: Some(AsmClass::Gpr),
    scalar_bits: Some(32),
    ..vector("pmovmskb", Ops::All)
  }),
  ("movmskps", Form {
    destination_class: Some(AsmClass::Gpr),
    scalar_bits: Some(32),
    ..vector("movmskps", Ops::All)
  }),
  ("pextrd", Form {
    destination_class: Some(AsmClass::Gpr),
    scalar_bits: Some(32),
    ..vector("pextrd", Ops::All)
  }),
  ("pextrq", Form {
    destination_class: Some(AsmClass::Gpr),
    scalar_bits: Some(64),
    ..vector("pextrq", Ops::All)
  }),
  ("pinsrd", Form { scalar_bits: Some(32), ..vector("pinsrd", Ops::All) }),
  ("pinsrq", Form { scalar_bits: Some(64), ..vector("pinsrq", Ops::All) }),
  // A broadcast reads one lane and repeats it; a gather addresses through a
  // vector index, which carries no width of its own (**L§15**).
  ("broadcastss", Form { memory_bits: Some(32), ..vector("broadcastss", Ops::All) }),
  ("broadcastsd", Form {
    memory_bits: Some(64),
    element_bits: 64,
    ..vector("broadcastsd", Ops::All)
  }),
  ("gatherdps", Form { bare_memory: true, ..vector("gatherdps", Ops::All) }),
  ("gatherdpd", Form { bare_memory: true, element_bits: 64, ..vector("gatherdpd", Ops::All) }),
  ("gatherqps", Form { bare_memory: true, ..vector("gatherqps", Ops::All) }),
  ("gatherqpd", Form { bare_memory: true, element_bits: 64, ..vector("gatherqpd", Ops::All) }),
  // The op-mask registers of AVX512.
  ("kmovb", Form { scalar_bits: Some(32), ..form("kmovb", Ops::All, AsmClass::Omr) }),
  ("kmovw", Form { scalar_bits: Some(32), ..form("kmovw", Ops::All, AsmClass::Omr) }),
  ("kmovd", Form { scalar_bits: Some(32), ..form("kmovd", Ops::All, AsmClass::Omr) }),
  ("kmovq", Form { scalar_bits: Some(64), ..form("kmovq", Ops::All, AsmClass::Omr) }),
  ("kandw", form("kandw", Ops::All, AsmClass::Omr)),
  ("korw", form("korw", Ops::All, AsmClass::Omr)),
  ("kxorw", form("kxorw", Ops::All, AsmClass::Omr)),
  ("knotw", form("knotw", Ops::All, AsmClass::Omr)),
  // The widening moves spell their two widths into the mnemonic.
  ("movzxbw", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movzxbd", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movzxbq", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movzxwd", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movzxwq", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movsxbw", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movsxbd", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movsxbq", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movsxwd", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movsxwq", implicit("", Ops::All, AsmClass::Gpr, &[])),
  ("movsxdq", implicit("", Ops::All, AsmClass::Gpr, &[])),
];

/// The form a mnemonic names, and whether it carries the `lock` prefix the
/// reference writes into the opcode name (**L§15**).
fn form_of(written: &str) -> Option<(&'static Form, bool)> {
  let (locked, name) = match written.strip_prefix("lock_") {
    Some(rest) => (true, rest),
    None => (false, written),
  };
  let form = FORMS
    .iter()
    .find(|(text, _)| *text == name)
    .map(|(_, form)| form)?;
  Some((form, locked))
}

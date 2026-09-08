use oj_scope::DeclId;
use oj_types::{TypeId, Types};

/// A virtual register inside one [`Procedure`]. Every value is assigned once,
/// so the back end can hand them to LLVM without renaming; anything that needs
/// to be assigned more than once is a [`Local`] and goes through memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(pub u32);

/// A stack slot: a parameter, a local variable, or a temporary an aggregate
/// expression needed somewhere to live.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalId(pub u32);

/// A constant an instruction materializes. Strings and aggregates become
/// read-only data in the back end; everything else is an immediate.
#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
  Int(i128),
  Float(f64),
  Bool(bool),
  /// A null pointer of the value's own type.
  Null,
  /// The bytes of a string literal. The value it produces is the `string`
  /// itself — `{count, data}` — with `data` pointing at read-only storage.
  String(Box<[u8]>),
  /// The storage of an aggregate a `#run` produced, laid out the way its type
  /// says (**L§12.1**). It becomes read-only data, and the value is its
  /// address.
  Bytes(Box<[u8]>),
  /// All-zero storage of the value's type, which is what an aggregate starts
  /// out as (**L§4.6**).
  Zero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
  Negate,
  BitwiseNot,
  /// `!x`, which is `x == 0` for everything that has a truth value
  /// (**L§5.9**).
  LogicalNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
  Add,
  Subtract,
  Multiply,
  Divide,
  Modulus,
  BitwiseAnd,
  BitwiseOr,
  BitwiseXor,
  ShiftLeft,
  ShiftRight,
  RotateLeft,
  RotateRight,
  Equal,
  NotEqual,
  Less,
  LessOrEqual,
  Greater,
  GreaterOrEqual,
}

impl BinaryOp {
  pub fn is_comparison(self) -> bool {
    matches!(
      self,
      Self::Equal
        | Self::NotEqual
        | Self::Less
        | Self::LessOrEqual
        | Self::Greater
        | Self::GreaterOrEqual
    )
  }

  pub fn name(self) -> &'static str {
    match self {
      Self::Add => "add",
      Self::Subtract => "sub",
      Self::Multiply => "mul",
      Self::Divide => "div",
      Self::Modulus => "mod",
      Self::BitwiseAnd => "and",
      Self::BitwiseOr => "or",
      Self::BitwiseXor => "xor",
      Self::ShiftLeft => "shl",
      Self::ShiftRight => "shr",
      Self::RotateLeft => "rol",
      Self::RotateRight => "ror",
      Self::Equal => "eq",
      Self::NotEqual => "ne",
      Self::Less => "lt",
      Self::LessOrEqual => "le",
      Self::Greater => "gt",
      Self::GreaterOrEqual => "ge",
    }
  }
}

/// How a value of one type becomes a value of another. The kinds are the ones
/// a machine actually has, so the back end never has to look at types again to
/// pick an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvertKind {
  IntegerZeroExtend,
  IntegerSignExtend,
  IntegerTruncate,
  SignedToFloat,
  UnsignedToFloat,
  FloatToSigned,
  FloatToUnsigned,
  FloatExtend,
  FloatTruncate,
  IntegerToPointer,
  PointerToInteger,
  /// A pointer that only changes what it points at, or a bit-identical
  /// reinterpretation of the same width.
  Bitcast,
}

/// What a call names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callee {
  Direct(ProcId),
  /// Through a value of procedure type.
  Indirect(ValueId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Inst {
  Const {
    dest: ValueId,
    value: Constant,
  },
  /// The address of a stack slot.
  LocalAddress {
    dest: ValueId,
    local: LocalId,
  },
  GlobalAddress {
    dest: ValueId,
    global: GlobalId,
  },
  ProcedureAddress {
    dest: ValueId,
    procedure: ProcId,
  },
  Load {
    dest: ValueId,
    address: ValueId,
  },
  Store {
    address: ValueId,
    value: ValueId,
  },
  /// `destination = source` for something that does not fit in a register.
  Copy {
    destination: ValueId,
    source: ValueId,
    size: u64,
    alignment: u64,
  },
  /// Zero storage, which is what a declaration without a value does
  /// (**L§4.6**).
  Clear {
    destination: ValueId,
    size: u64,
    alignment: u64,
  },
  /// A fixed byte offset into a pointer: a struct member (**L§8.1**).
  Offset {
    dest: ValueId,
    base: ValueId,
    offset: i64,
  },
  /// `base + index * stride`, which is what an array subscript and pointer
  /// arithmetic both come to (**L§3.3**).
  Index {
    dest: ValueId,
    base: ValueId,
    index: ValueId,
    stride: u64,
  },
  Unary {
    dest: ValueId,
    operator: UnaryOp,
    operand: ValueId,
  },
  Binary {
    dest: ValueId,
    operator: BinaryOp,
    left: ValueId,
    right: ValueId,
  },
  Convert {
    dest: ValueId,
    kind: ConvertKind,
    operand: ValueId,
  },
  /// Preload's `compare_and_swap` (**L§17**): an atomic compare-exchange that
  /// gives both whether the swap happened and what was there before.
  AtomicCompareExchange {
    success: ValueId,
    previous: ValueId,
    address: ValueId,
    expected: ValueId,
    desired: ValueId,
  },
  Call {
    /// The value returned in registers, when the call has one. A return the
    /// convention passes by pointer is written through an argument instead.
    dest: Option<ValueId>,
    callee: Callee,
    /// The procedure type being called, which is what decides the convention.
    signature: TypeId,
    arguments: Vec<ValueId>,
  },
}

impl Inst {
  pub fn dest(&self) -> Option<ValueId> {
    match self {
      Self::Const { dest, .. }
      | Self::LocalAddress { dest, .. }
      | Self::GlobalAddress { dest, .. }
      | Self::ProcedureAddress { dest, .. }
      | Self::Load { dest, .. }
      | Self::Offset { dest, .. }
      | Self::Index { dest, .. }
      | Self::Unary { dest, .. }
      | Self::Binary { dest, .. }
      | Self::Convert { dest, .. } => Some(*dest),
      Self::Call { dest, .. } => *dest,
      // A compare-exchange writes two values, so neither is "the" one.
      Self::AtomicCompareExchange { .. }
      | Self::Store { .. }
      | Self::Copy { .. }
      | Self::Clear { .. } => None,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Terminator {
  Return(Vec<ValueId>),
  Jump(BlockId),
  Branch {
    condition: ValueId,
    then_block: BlockId,
    else_block: BlockId,
  },
  /// The end of a block the lowering never finished, or code after a `return`.
  Unreachable,
}

#[derive(Clone, Debug)]
pub struct Block {
  pub instructions: Vec<Inst>,
  pub terminator: Terminator,
}

impl Block {
  pub fn new() -> Self {
    Self {
      instructions: Vec::new(),
      terminator: Terminator::Unreachable,
    }
  }
}

impl Default for Block {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Clone, Debug)]
pub struct Local {
  pub name: String,
  pub type_id: TypeId,
  pub size: u64,
  pub alignment: u64,
}

bitflags::bitflags! {
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
  pub struct ProcedureFlags: u32 {
    /// Declared `#foreign`: the body lives in a library, so only the header is
    /// emitted (**L§12.1**).
    const FOREIGN = 0x1;
    /// `#c_call`: the platform C convention rather than the Jai one, and no
    /// hidden context parameter (**L§7.11**).
    const C_CALL = 0x2;
    /// `#no_context`, or a `#c_call`: no hidden context parameter
    /// (**L§10.3**).
    const NO_CONTEXT = 0x4;
    /// `#program_export`: visible to the linker under its own name.
    const EXPORT = 0x8;
    /// Generated by orangejuice rather than written by the program.
    const COMPILER_GENERATED = 0x10;
    /// `#compiler`: the compiler itself is the body, so the symbol binds to a
    /// procedure inside `oj-meta` and the call happens at compile time only
    /// (**C§3.3**).
    const COMPILER = 0x20;
  }
}

/// How one machine-level parameter carries what the source wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterKind {
  /// The value itself, in a register.
  Value,
  /// A pointer to the value, which is how anything that does not fit in a
  /// register is passed (**L§7.6**).
  Pointer,
  /// Storage the callee writes a return value into, for a return that does not
  /// fit in registers and for every return after the first (**L§7.2**).
  ReturnPointer,
  /// The hidden `*#Context` every Jai-convention procedure carries
  /// (**L§10.1**).
  Context,
}

/// One machine-level parameter. The list is the contract between a call site
/// and a body, and both build it from [`Procedure::parameters`] with the same
/// rules, so neither has to know how the other lowered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbiParameter {
  /// The type of the thing being passed — for `Pointer`, `ReturnPointer` and
  /// `Context` that is what is pointed at, not the pointer.
  pub type_id: TypeId,
  pub kind: ParameterKind,
}

/// The machine-level shape of a call: what goes in, and what comes back in
/// registers.
#[derive(Clone, Debug, PartialEq)]
pub struct Abi {
  pub parameters: Vec<AbiParameter>,
  /// The return value passed in registers, when there is one.
  pub direct_return: Option<TypeId>,
}

/// One procedure of the program, in the IR the back end consumes.
#[derive(Clone, Debug)]
pub struct Procedure {
  /// The symbol the linker sees.
  pub symbol: String,
  /// The name diagnostics and `oj dump ir` use.
  pub name: String,
  pub type_id: TypeId,
  pub parameters: Vec<TypeId>,
  pub returns: Vec<TypeId>,
  pub flags: ProcedureFlags,
  /// The library a `#foreign` procedure is bound from, when one was named.
  pub library: Option<String>,
  /// The machine-level parameter list. The first `abi.parameters.len()` values
  /// of the procedure are exactly these, in order, so the back end never has
  /// to reconstruct the mapping.
  pub abi: Abi,
  pub locals: Vec<Local>,
  pub blocks: Vec<Block>,
  pub value_types: Vec<TypeId>,
  pub entry: BlockId,
}

impl Procedure {
  pub fn value_type(&self, value: ValueId) -> TypeId {
    self.value_types[value.0 as usize]
  }

  pub fn block(&self, id: BlockId) -> &Block {
    &self.blocks[id.0 as usize]
  }

  /// Whether the back end has anything to emit for this procedure. A
  /// `#foreign` header never does, and neither does one whose lowering was cut
  /// short by an error — the driver stops before code generation in that case.
  pub fn has_body(&self) -> bool {
    !self.flags.contains(ProcedureFlags::FOREIGN) && !self.blocks.is_empty()
  }
}

/// How a global's storage starts out. Anything the front end cannot fold is
/// left `Zero` and assigned by the generated initializer procedure instead.
#[derive(Clone, Debug, PartialEq)]
pub enum GlobalInit {
  Zero,
  Constant(Constant),
  /// The bytes compile-time execution left in the global, which is what
  /// `#no_reset` writes into the executable (**L§12.3**).
  Bytes(Box<[u8]>),
  /// Bytes that point at themselves: the type table is one block of storage
  /// whose `*Type_Info` fields name other places inside the same block
  /// (**L§17**). Each `(at, target)` says that the eight bytes at `at` hold
  /// the address of `target`, both relative to where the block is placed.
  Image {
    bytes: Box<[u8]>,
    relocations: Box<[(u64, u64)]>,
  },
}

#[derive(Clone, Debug)]
pub struct Global {
  pub symbol: String,
  pub name: String,
  pub type_id: TypeId,
  pub init: GlobalInit,
  pub size: u64,
  pub alignment: u64,
  /// The declaration the global came from. Two lowerings of one program agree
  /// on this where they need not agree on a symbol, which is how the bytes a
  /// `#run` left in a global find their way into the executable.
  pub decl: Option<DeclId>,
  /// `#no_reset`: compile-time writes survive into the executable
  /// (**L§4.7**).
  pub no_reset: bool,
  /// `#program_export` or `#elsewhere`: the symbol is shared with the linker
  /// rather than private to the module.
  pub external: bool,
  /// `#elsewhere`: defined by a library, so only declared here (**L§4.8**).
  pub imported: bool,
}

/// Whether a value of this type travels in a register. Everything else stays
/// in memory and is addressed rather than copied around (**L§3.14**).
pub fn is_scalar(types: &Types, type_id: TypeId) -> bool {
  use oj_types::TypeKind;
  let underlying = types.underlying(type_id);
  matches!(
    types.kind(underlying),
    TypeKind::Bool
      | TypeKind::Integer(_)
      | TypeKind::Float(_)
      | TypeKind::Enum(_)
      | TypeKind::Pointer(_)
      | TypeKind::Procedure(_)
      | TypeKind::Type
      // A `Code` is a handle the compiler hands out, the size of a pointer
      // (**L§13.1**), so it travels in a register like one.
      | TypeKind::Code
  )
}

/// The machine-level parameter list of a procedure type (**L§7.6**). A call
/// site and a body both build it here, so the two cannot disagree.
pub fn abi_of(
  types: &Types,
  signature: TypeId,
  flags: ProcedureFlags,
  context_type: TypeId,
) -> Abi {
  let Some(signature) = types.procedure_of(signature) else {
    return Abi {
      parameters: Vec::new(),
      direct_return: None,
    };
  };

  let mut parameters = Vec::new();
  let direct_return = match signature.returns.first().copied() {
    Some(type_id) if is_scalar(types, type_id) => Some(type_id),
    Some(type_id) => {
      parameters.push(AbiParameter {
        type_id,
        kind: ParameterKind::ReturnPointer,
      });
      None
    }
    None => None,
  };

  for argument in &signature.arguments {
    parameters.push(AbiParameter {
      type_id: *argument,
      kind: if is_scalar(types, *argument) {
        ParameterKind::Value
      } else {
        ParameterKind::Pointer
      },
    });
  }

  // Every return after the first is written through storage the caller
  // supplies (**L§7.2**).
  for extra in signature.returns.iter().skip(1) {
    parameters.push(AbiParameter {
      type_id: *extra,
      kind: ParameterKind::ReturnPointer,
    });
  }

  if !flags.contains(ProcedureFlags::NO_CONTEXT) {
    parameters.push(AbiParameter {
      type_id: context_type,
      kind: ParameterKind::Context,
    });
  }

  Abi {
    parameters,
    direct_return,
  }
}

/// A whole program, lowered. The type table travels with it, because a
/// [`TypeId`] means nothing without one.
#[derive(Debug)]
pub struct Program {
  pub types: Types,
  pub procedures: Vec<Procedure>,
  pub globals: Vec<Global>,
  /// The program's `main`, which the generated entry point calls.
  pub entry: Option<ProcId>,
  /// The generated procedure that runs the global initializers the front end
  /// could not fold into data, called before `main`.
  pub global_init: Option<ProcId>,
  /// The type of `#Context`, which every Jai-convention procedure takes a
  /// pointer to (**L§10.1**).
  pub context_type: TypeId,
  /// The libraries the `#foreign` procedures asked for, in declaration order.
  pub libraries: Vec<Library>,
  /// Where each type's record sits in the type table image, and the symbol the
  /// image took (**L§17**). A `Type` a compile-time program produced is an
  /// address into it, so this is what turns that address back into a type.
  pub type_table: TypeTableImage,
}

/// Where the types the program asked about ended up (**L§17**).
#[derive(Clone, Debug, Default)]
pub struct TypeTableImage {
  pub symbol: Option<String>,
  pub offsets: Vec<(TypeId, u64)>,
}

impl TypeTableImage {
  /// The type whose record sits at `offset`.
  pub fn type_at(&self, offset: u64) -> Option<TypeId> {
    self
      .offsets
      .iter()
      .find(|(_, at)| *at == offset)
      .map(|(type_id, _)| *type_id)
  }
}

/// A `#library` or `#library,system` the program named (**L§12.2**).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
  pub name: String,
  pub system: bool,
  /// The directory the declaring file was in, which is where a non-system
  /// library is looked for.
  pub directory: Option<std::path::PathBuf>,
}

impl Program {
  pub fn procedure(&self, id: ProcId) -> &Procedure {
    &self.procedures[id.0 as usize]
  }

  pub fn global(&self, id: GlobalId) -> &Global {
    &self.globals[id.0 as usize]
  }
}

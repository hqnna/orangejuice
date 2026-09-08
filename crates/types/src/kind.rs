use bitflags::bitflags;
use oj_lexer::Symbol;

/// A type's identity in one [`crate::Types`]. Pointer, array and procedure
/// types are interned structurally; structs, enums and variants are nominal, so
/// two identical-looking declarations are two types (**L§3.13**).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeId(pub u32);

/// A struct or union definition's identity in one [`crate::Types`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructId(pub u32);

/// An enum definition's identity in one [`crate::Types`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnumId(pub u32);

/// A `#type,distinct` / `#type,isa` definition's identity in one [`crate::Types`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariantId(pub u32);

/// A polymorphic type variable's identity in one [`crate::Types`]. Every `$T`
/// introduces its own, so two procedures' `$T` never unify (**L§7.8**).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PolymorphId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntKind {
  S8,
  S16,
  S32,
  S64,
  U8,
  U16,
  U32,
  U64,
}

impl IntKind {
  pub fn size(self) -> u64 {
    match self {
      Self::S8 | Self::U8 => 1,
      Self::S16 | Self::U16 => 2,
      Self::S32 | Self::U32 => 4,
      Self::S64 | Self::U64 => 8,
    }
  }

  pub fn is_signed(self) -> bool {
    matches!(self, Self::S8 | Self::S16 | Self::S32 | Self::S64)
  }

  pub fn name(self) -> &'static str {
    match self {
      Self::S8 => "s8",
      Self::S16 => "s16",
      Self::S32 => "s32",
      Self::S64 => "s64",
      Self::U8 => "u8",
      Self::U16 => "u16",
      Self::U32 => "u32",
      Self::U64 => "u64",
    }
  }

  /// The lowest value the type can hold, as `i128` so that `u64` and `s64`
  /// ranges are both representable.
  pub fn min(self) -> i128 {
    if self.is_signed() {
      -(1i128 << (self.size() * 8 - 1))
    } else {
      0
    }
  }

  pub fn max(self) -> i128 {
    if self.is_signed() {
      (1i128 << (self.size() * 8 - 1)) - 1
    } else {
      (1i128 << (self.size() * 8)) - 1
    }
  }

  pub fn holds(self, value: i128) -> bool {
    (self.min()..=self.max()).contains(&value)
  }

  /// Whether every value of `other` is a value of `self` — the widening rule
  /// implicit conversions use (**L§5.10**).
  pub fn contains_range_of(self, other: Self) -> bool {
    self.min() <= other.min() && self.max() >= other.max()
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatKind {
  F32,
  F64,
}

impl FloatKind {
  pub fn size(self) -> u64 {
    match self {
      Self::F32 => 4,
      Self::F64 => 8,
    }
  }

  pub fn name(self) -> &'static str {
    match self {
      Self::F32 => "float32",
      Self::F64 => "float64",
    }
  }
}

/// The three array kinds of **L§3.3**, which share `a[i]`, `.count` and `.data`
/// but not their layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArrayKind {
  Fixed(u64),
  View,
  Resizable,
}

bitflags! {
  /// `Type_Info_Procedure.Flags` (**L§17**). Varargs are not here: they belong
  /// to the calling convention, not to the flag word (**L§3.7**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
  pub struct ProcedureFlags: u32 {
    const IS_ELSEWHERE = 0x1;
    const IS_COMPILE_TIME_ONLY = 0x2;
    const IS_POLYMORPHIC = 0x4;
    const HAS_NO_CONTEXT = 0x8;
    const IS_C_CALL = 0x20;
    const IS_INTRINSIC = 0x80;
    const IS_SYMMETRIC = 0x100;
    const IS_CPP_METHOD = 0x1000_0000;
    const HAS_CPP_NON_POD_RETURN_TYPE = 0x2000_0000;
  }
}

/// A procedure type: parameter and return types, the calling convention and the
/// header flags that change it (**L§3.7**). Parameter *names* and default
/// values are informational and are not part of the type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProcedureType {
  pub arguments: Vec<TypeId>,
  pub returns: Vec<TypeId>,
  pub flags: ProcedureFlags,
  /// The last parameter is `..T`, which makes the call site build a `[] T`.
  pub varargs: bool,
}

impl ProcedureType {
  pub fn new(arguments: Vec<TypeId>, returns: Vec<TypeId>) -> Self {
    Self {
      arguments,
      returns,
      flags: ProcedureFlags::empty(),
      varargs: false,
    }
  }
}

/// What a [`TypeId`] stands for. The variants line up with `Type_Info_Tag`
/// (**L§17**) so that materializing a `Type_Info` is a projection.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeKind {
  Void,
  Bool,
  Integer(IntKind),
  Float(FloatKind),
  String,
  Any,
  Code,
  Type,
  /// The 128-bit vector `#asm` operates on (**L§3.1**).
  V128,
  Pointer(TypeId),
  Array {
    element: TypeId,
    kind: ArrayKind,
  },
  Procedure(Box<ProcedureType>),
  Struct(StructId),
  Enum(EnumId),
  Variant(VariantId),
  Polymorph(PolymorphId),
  /// An integer literal that has not been given a type yet (**L§5.10**): it
  /// adapts to whatever the context wants, and defaults to `s64`.
  UntypedInt,
  /// A float literal before defaulting; `DEFAULTS_TO_FLOAT64` decides which.
  UntypedFloat(FloatKind),
  /// A `.NAME` whose enum is not known yet (**L§5.12**).
  UntypedEnum,
  /// An undesignated `.{…}` or `.[…]` literal (**L§5.7**, **L§5.8**).
  UntypedLiteral,
  /// The type of a name that stands for several procedures at once. It is not
  /// a storable type (**L§7.5**).
  OverloadSet,
  /// A type orangejuice cannot determine yet, because deciding it needs a
  /// milestone the front end has not reached. It matches everything and is
  /// never reported.
  Unknown,
}

impl TypeKind {
  /// The `Type_Info_Tag` name the reference prints for this kind (**L§17**).
  pub fn tag_name(&self) -> &'static str {
    match self {
      Self::Integer(_) => "INTEGER",
      Self::Float(_) | Self::UntypedFloat(_) => "FLOAT",
      Self::Bool => "BOOL",
      Self::String => "STRING",
      Self::Pointer(_) => "POINTER",
      Self::Procedure(_) => "PROCEDURE",
      Self::Void => "VOID",
      Self::Struct(_) => "STRUCT",
      Self::Array { .. } => "ARRAY",
      Self::OverloadSet => "OVERLOAD_SET",
      Self::Any => "ANY",
      Self::Enum(_) => "ENUM",
      Self::Polymorph(_) => "POLYMORPHIC_VARIABLE",
      Self::Type => "TYPE",
      Self::Code => "CODE",
      Self::UntypedInt | Self::UntypedLiteral => "UNTYPED_LITERAL",
      Self::UntypedEnum => "UNTYPED_ENUM",
      Self::Variant(_) => "VARIANT",
      Self::V128 => "V128",
      Self::Unknown => "UNKNOWN",
    }
  }

  /// Whether the type is one of the literal placeholders that take their type
  /// from context rather than carrying one (**L§5.10**).
  pub fn is_untyped(&self) -> bool {
    matches!(
      self,
      Self::UntypedInt | Self::UntypedFloat(_) | Self::UntypedEnum | Self::UntypedLiteral
    )
  }
}

bitflags! {
  /// `Type_Info_Struct_Member.Flags` (**L§17**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
  pub struct MemberFlags: u32 {
    const CONSTANT = 0x1;
    const IMPORTED = 0x2;
    const USING = 0x4;
    const PROCEDURE_WITH_VOID_POINTER_TYPE_INFO = 0x8;
    const AS = 0x10;
  }
}

bitflags! {
  /// `Type_Info_Struct.textual_flags` (**L§8.7**); the bit values are the
  /// reference's.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
  pub struct StructTextualFlags: u32 {
    const FOREIGN = 0x1;
    const UNION = 0x2;
    const NO_PADDING = 0x4;
    const TYPE_INFO_NONE = 0x8;
    const TYPE_INFO_NO_SIZE_COMPLAINT = 0x10;
    const TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS = 0x20;
  }
}

bitflags! {
  /// `Type_Info_Struct.nontextual_flags` (**L§8.7**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
  pub struct StructNontextualFlags: u32 {
    const NOT_INSTANTIABLE = 0x4;
    const ALL_MEMBERS_UNINITIALIZED = 0x40;
    const POLYMORPHIC = 0x100;
  }
}

bitflags! {
  /// `Type_Info_Enum.enum_type_flags` (**L§9**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
  pub struct EnumTypeFlags: u32 {
    const FLAGS = 0x1;
    const COMPLETE = 0x2;
    const SPECIFIED = 0x4;
  }
}

bitflags! {
  /// `Type_Info_Variant.variant_flags` (**L§3.11**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
  pub struct VariantFlags: u32 {
    const DISTINCT = 0x1;
    const ISA = 0x2;
  }
}

/// One member of a struct, in declaration order (**L§8.1**). Constants have no
/// storage and carry `MemberFlags::CONSTANT`.
#[derive(Clone, Debug, PartialEq)]
pub struct StructMember {
  pub name: Symbol,
  pub type_id: TypeId,
  pub offset: u64,
  pub flags: MemberFlags,
  /// The member this one was reached through, for a name that a `using` member
  /// imported (**L§8.4**). `None` for a member written in the struct itself.
  pub imported_through: Option<usize>,
}

impl StructMember {
  pub fn is_constant(&self) -> bool {
    self.flags.contains(MemberFlags::CONSTANT)
  }
}

/// A struct or union definition. `members` is the flattened view used for name
/// lookup: members a `using` imported appear after the member that imported
/// them, with `IMPORTED` set and their real offsets (**L§8.4**).
#[derive(Clone, Debug, PartialEq)]
pub struct StructInfo {
  pub name: Option<Symbol>,
  pub members: Vec<StructMember>,
  pub textual_flags: StructTextualFlags,
  pub nontextual_flags: StructNontextualFlags,
  pub size: u64,
  pub alignment: u64,
  /// False while the members are still being computed, which is what makes a
  /// struct that contains itself by value an error rather than a hang.
  pub complete: bool,
  pub polymorph_source: Option<StructId>,
}

impl StructInfo {
  pub fn new(name: Option<Symbol>, textual_flags: StructTextualFlags) -> Self {
    Self {
      name,
      members: Vec::new(),
      textual_flags,
      nontextual_flags: StructNontextualFlags::empty(),
      size: 0,
      alignment: 1,
      complete: false,
      polymorph_source: None,
    }
  }

  pub fn is_union(&self) -> bool {
    self.textual_flags.contains(StructTextualFlags::UNION)
  }

  pub fn member(&self, name: Symbol) -> Option<&StructMember> {
    self.members.iter().find(|member| member.name == name)
  }

  /// The members that take part in a positional struct literal: the ones with
  /// storage that no `using` imported (**L§5.7**).
  pub fn settable_members(&self) -> impl Iterator<Item = &StructMember> {
    self.members.iter().filter(|member| {
      !member
        .flags
        .intersects(MemberFlags::CONSTANT | MemberFlags::IMPORTED)
    })
  }

  /// The members marked `#as`, which the struct implicitly converts to
  /// (**L§8.4**).
  pub fn as_members(&self) -> impl Iterator<Item = &StructMember> {
    self
      .members
      .iter()
      .filter(|member| member.flags.contains(MemberFlags::AS))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumMember {
  pub name: Symbol,
  pub value: i64,
}

/// An enum or `enum_flags` definition (**L§9**).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumInfo {
  pub name: Option<Symbol>,
  /// The integer type the members are stored as; `s64` unless one was written.
  pub base: TypeId,
  pub members: Vec<EnumMember>,
  pub flags: EnumTypeFlags,
  pub complete: bool,
}

impl EnumInfo {
  pub fn new(name: Option<Symbol>, base: TypeId, flags: EnumTypeFlags) -> Self {
    Self {
      name,
      base,
      members: Vec::new(),
      flags,
      complete: false,
    }
  }

  pub fn is_flags(&self) -> bool {
    self.flags.contains(EnumTypeFlags::FLAGS)
  }

  pub fn value_of(&self, name: Symbol) -> Option<i64> {
    self
      .members
      .iter()
      .find(|member| member.name == name)
      .map(|member| member.value)
  }
}

/// A `#type,distinct` or `#type,isa` definition (**L§3.11**).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantInfo {
  pub name: Option<Symbol>,
  pub base: TypeId,
  pub flags: VariantFlags,
}

impl VariantInfo {
  pub fn is_isa(&self) -> bool {
    self.flags.contains(VariantFlags::ISA)
  }
}

/// A polymorphic type variable (`$T`), named for diagnostics (**L§7.8**).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolymorphInfo {
  pub name: Symbol,
  /// The `$T/Restriction` a candidate must satisfy, if one was written.
  pub restriction: Option<TypeId>,
}

use std::collections::HashMap;

use oj_lexer::Symbol;

use crate::kind::{
  ArrayKind, EnumId, EnumInfo, FloatKind, IntKind, PolymorphId, PolymorphInfo, ProcedureType,
  StructId, StructInfo, TypeId, TypeKind, VariantId, VariantInfo,
};

impl TypeId {
  pub const VOID: Self = Self(0);
  pub const BOOL: Self = Self(1);
  pub const S8: Self = Self(2);
  pub const S16: Self = Self(3);
  pub const S32: Self = Self(4);
  pub const S64: Self = Self(5);
  pub const U8: Self = Self(6);
  pub const U16: Self = Self(7);
  pub const U32: Self = Self(8);
  pub const U64: Self = Self(9);
  pub const FLOAT32: Self = Self(10);
  pub const FLOAT64: Self = Self(11);
  pub const STRING: Self = Self(12);
  pub const ANY: Self = Self(13);
  pub const CODE: Self = Self(14);
  pub const TYPE: Self = Self(15);
  pub const V128: Self = Self(16);
  pub const UNTYPED_INT: Self = Self(17);
  pub const UNTYPED_FLOAT32: Self = Self(18);
  pub const UNTYPED_FLOAT64: Self = Self(19);
  pub const UNTYPED_ENUM: Self = Self(20);
  pub const UNTYPED_LITERAL: Self = Self(21);
  pub const OVERLOAD_SET: Self = Self(22);
  pub const UNKNOWN: Self = Self(23);
  pub const VOID_POINTER: Self = Self(24);
  pub const U8_POINTER: Self = Self(25);

  /// `int` is `s64` and `float` is `float32`; the aliases are not separate
  /// types (**L§3.1**).
  pub const INT: Self = Self::S64;
  pub const FLOAT: Self = Self::FLOAT32;
}

/// The types of one compilation, interned. Pointer, array and procedure types
/// are structural, so building the same one twice gives the same [`TypeId`];
/// structs, enums, variants and polymorph variables are nominal and get a fresh
/// id per declaration (**L§3.13**).
#[derive(Debug)]
pub struct Types {
  kinds: Vec<TypeKind>,
  interned: HashMap<TypeKind, TypeId>,
  structs: Vec<StructInfo>,
  enums: Vec<EnumInfo>,
  variants: Vec<VariantInfo>,
  polymorphs: Vec<PolymorphInfo>,
}

impl Default for Types {
  fn default() -> Self {
    Self::new()
  }
}

impl Types {
  pub fn new() -> Self {
    let mut types = Self {
      kinds: Vec::new(),
      interned: HashMap::new(),
      structs: Vec::new(),
      enums: Vec::new(),
      variants: Vec::new(),
      polymorphs: Vec::new(),
    };
    let builtins = [
      TypeKind::Void,
      TypeKind::Bool,
      TypeKind::Integer(IntKind::S8),
      TypeKind::Integer(IntKind::S16),
      TypeKind::Integer(IntKind::S32),
      TypeKind::Integer(IntKind::S64),
      TypeKind::Integer(IntKind::U8),
      TypeKind::Integer(IntKind::U16),
      TypeKind::Integer(IntKind::U32),
      TypeKind::Integer(IntKind::U64),
      TypeKind::Float(FloatKind::F32),
      TypeKind::Float(FloatKind::F64),
      TypeKind::String,
      TypeKind::Any,
      TypeKind::Code,
      TypeKind::Type,
      TypeKind::V128,
      TypeKind::UntypedInt,
      TypeKind::UntypedFloat(FloatKind::F32),
      TypeKind::UntypedFloat(FloatKind::F64),
      TypeKind::UntypedEnum,
      TypeKind::UntypedLiteral,
      TypeKind::OverloadSet,
      TypeKind::Unknown,
      TypeKind::Pointer(TypeId::VOID),
      TypeKind::Pointer(TypeId::U8),
    ];
    for kind in builtins {
      types.intern(kind);
    }
    debug_assert_eq!(types.kinds.len(), TypeId::U8_POINTER.0 as usize + 1);
    types
  }

  fn intern(&mut self, kind: TypeKind) -> TypeId {
    if let Some(existing) = self.interned.get(&kind) {
      return *existing;
    }
    let id = self.push(kind.clone());
    self.interned.insert(kind, id);
    id
  }

  fn push(&mut self, kind: TypeKind) -> TypeId {
    let id = TypeId(self.kinds.len() as u32);
    self.kinds.push(kind);
    id
  }

  pub fn kind(&self, id: TypeId) -> &TypeKind {
    &self.kinds[id.0 as usize]
  }

  pub fn len(&self) -> usize {
    self.kinds.len()
  }

  pub fn is_empty(&self) -> bool {
    self.kinds.is_empty()
  }

  pub fn pointer_to(&mut self, pointee: TypeId) -> TypeId {
    self.intern(TypeKind::Pointer(pointee))
  }

  pub fn array(&mut self, element: TypeId, kind: ArrayKind) -> TypeId {
    self.intern(TypeKind::Array { element, kind })
  }

  pub fn procedure(&mut self, signature: ProcedureType) -> TypeId {
    self.intern(TypeKind::Procedure(Box::new(signature)))
  }

  pub fn integer(&self, kind: IntKind) -> TypeId {
    match kind {
      IntKind::S8 => TypeId::S8,
      IntKind::S16 => TypeId::S16,
      IntKind::S32 => TypeId::S32,
      IntKind::S64 => TypeId::S64,
      IntKind::U8 => TypeId::U8,
      IntKind::U16 => TypeId::U16,
      IntKind::U32 => TypeId::U32,
      IntKind::U64 => TypeId::U64,
    }
  }

  pub fn float(&self, kind: FloatKind) -> TypeId {
    match kind {
      FloatKind::F32 => TypeId::FLOAT32,
      FloatKind::F64 => TypeId::FLOAT64,
    }
  }

  /// Declares a struct or union. The definition starts out incomplete: sema
  /// fills its members in and calls [`Types::finish_struct`].
  pub fn new_struct(&mut self, info: StructInfo) -> (StructId, TypeId) {
    let id = StructId(self.structs.len() as u32);
    self.structs.push(info);
    (id, self.push(TypeKind::Struct(id)))
  }

  pub fn new_enum(&mut self, info: EnumInfo) -> (EnumId, TypeId) {
    let id = EnumId(self.enums.len() as u32);
    self.enums.push(info);
    (id, self.push(TypeKind::Enum(id)))
  }

  pub fn new_variant(&mut self, info: VariantInfo) -> (VariantId, TypeId) {
    let id = VariantId(self.variants.len() as u32);
    self.variants.push(info);
    (id, self.push(TypeKind::Variant(id)))
  }

  pub fn new_polymorph(&mut self, info: PolymorphInfo) -> (PolymorphId, TypeId) {
    let id = PolymorphId(self.polymorphs.len() as u32);
    self.polymorphs.push(info);
    (id, self.push(TypeKind::Polymorph(id)))
  }

  pub fn struct_info(&self, id: StructId) -> &StructInfo {
    &self.structs[id.0 as usize]
  }

  pub fn struct_info_mut(&mut self, id: StructId) -> &mut StructInfo {
    &mut self.structs[id.0 as usize]
  }

  pub fn enum_info(&self, id: EnumId) -> &EnumInfo {
    &self.enums[id.0 as usize]
  }

  pub fn enum_info_mut(&mut self, id: EnumId) -> &mut EnumInfo {
    &mut self.enums[id.0 as usize]
  }

  pub fn variant_info(&self, id: VariantId) -> &VariantInfo {
    &self.variants[id.0 as usize]
  }

  pub fn polymorph_info(&self, id: PolymorphId) -> &PolymorphInfo {
    &self.polymorphs[id.0 as usize]
  }

  pub fn finish_struct(&mut self, id: StructId, size: u64, alignment: u64) {
    let info = self.struct_info_mut(id);
    info.size = size;
    info.alignment = alignment;
    info.complete = true;
  }

  /// The struct definition behind a type, seeing through variants and one level
  /// of pointer the way member access does (**L§3.2**, **L§3.11**).
  pub fn struct_of(&self, id: TypeId) -> Option<StructId> {
    match self.kind(id) {
      TypeKind::Struct(id) => Some(*id),
      TypeKind::Variant(variant) => self.struct_of(self.variant_info(*variant).base),
      _ => None,
    }
  }

  pub fn enum_of(&self, id: TypeId) -> Option<EnumId> {
    match self.kind(id) {
      TypeKind::Enum(id) => Some(*id),
      TypeKind::Variant(variant) => self.enum_of(self.variant_info(*variant).base),
      _ => None,
    }
  }

  /// The type a variant stands for, following `isa`/`distinct` chains down to
  /// the underlying representation (**L§3.11**).
  pub fn underlying(&self, id: TypeId) -> TypeId {
    match self.kind(id) {
      TypeKind::Variant(variant) => self.underlying(self.variant_info(*variant).base),
      _ => id,
    }
  }

  pub fn is_integer(&self, id: TypeId) -> bool {
    matches!(
      self.kind(self.underlying(id)),
      TypeKind::Integer(_) | TypeKind::UntypedInt
    )
  }

  pub fn is_float(&self, id: TypeId) -> bool {
    matches!(
      self.kind(self.underlying(id)),
      TypeKind::Float(_) | TypeKind::UntypedFloat(_)
    )
  }

  pub fn is_numeric(&self, id: TypeId) -> bool {
    self.is_integer(id) || self.is_float(id)
  }

  pub fn is_pointer(&self, id: TypeId) -> bool {
    matches!(self.kind(self.underlying(id)), TypeKind::Pointer(_))
  }

  pub fn is_unknown(&self, id: TypeId) -> bool {
    *self.kind(id) == TypeKind::Unknown
  }

  pub fn is_untyped(&self, id: TypeId) -> bool {
    self.kind(id).is_untyped()
  }

  pub fn integer_kind(&self, id: TypeId) -> Option<IntKind> {
    match self.kind(self.underlying(id)) {
      TypeKind::Integer(kind) => Some(*kind),
      TypeKind::Enum(id) => self.integer_kind(self.enum_info(*id).base),
      _ => None,
    }
  }

  pub fn float_kind(&self, id: TypeId) -> Option<FloatKind> {
    match self.kind(self.underlying(id)) {
      TypeKind::Float(kind) | TypeKind::UntypedFloat(kind) => Some(*kind),
      _ => None,
    }
  }

  pub fn pointee(&self, id: TypeId) -> Option<TypeId> {
    match self.kind(self.underlying(id)) {
      TypeKind::Pointer(pointee) => Some(*pointee),
      _ => None,
    }
  }

  /// The element type and kind of an array, `string` counting as a `[] u8`
  /// wherever iteration and subscripting are concerned (**L§3.4**).
  pub fn array_of(&self, id: TypeId) -> Option<(TypeId, ArrayKind)> {
    match self.kind(self.underlying(id)) {
      TypeKind::Array { element, kind } => Some((*element, *kind)),
      TypeKind::String => Some((TypeId::U8, ArrayKind::View)),
      _ => None,
    }
  }

  pub fn procedure_of(&self, id: TypeId) -> Option<&ProcedureType> {
    match self.kind(self.underlying(id)) {
      TypeKind::Procedure(signature) => Some(signature),
      _ => None,
    }
  }

  /// Whether a struct's storage is fully known. A type whose members are still
  /// being computed answers `false`, which is how self-containment is caught.
  pub fn is_complete(&self, id: TypeId) -> bool {
    match self.kind(id) {
      TypeKind::Struct(id) => self.struct_info(*id).complete,
      TypeKind::Enum(id) => self.enum_info(*id).complete,
      TypeKind::Variant(id) => self.is_complete(self.variant_info(*id).base),
      TypeKind::Array {
        element,
        kind: ArrayKind::Fixed(_),
      } => self.is_complete(*element),
      _ => true,
    }
  }

  /// The name of a nominal type, for diagnostics.
  pub fn declared_name(&self, id: TypeId) -> Option<Symbol> {
    match self.kind(id) {
      TypeKind::Struct(id) => self.struct_info(*id).name,
      TypeKind::Enum(id) => self.enum_info(*id).name,
      TypeKind::Variant(id) => self.variant_info(*id).name,
      TypeKind::Polymorph(id) => Some(self.polymorph_info(*id).name),
      _ => None,
    }
  }

  pub fn set_declared_name(&mut self, id: TypeId, name: Symbol) {
    match *self.kind(id) {
      TypeKind::Struct(id) => self.struct_info_mut(id).name = Some(name),
      TypeKind::Enum(id) => self.enum_info_mut(id).name = Some(name),
      TypeKind::Variant(id) => self.variants[id.0 as usize].name = Some(name),
      _ => {}
    }
  }
}

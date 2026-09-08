//! The type representation of orangejuice: interning, layout and printing.
//!
//! Types are `TypeId`s into one [`Types`] table. Pointer, array and procedure
//! types are interned structurally; structs, enums, variants and polymorph
//! variables are nominal, so each declaration is its own type (**L§3.13**).

mod kind;
mod layout;
mod print;
mod table;

pub use kind::{
  ArrayKind, EnumId, EnumInfo, EnumMember, EnumTypeFlags, FloatKind, IntKind, MemberFlags,
  PolymorphId, PolymorphInfo, ProcedureFlags, ProcedureType, StructId, StructInfo, StructMember,
  StructNontextualFlags, StructTextualFlags, TypeId, TypeKind, VariantFlags, VariantId,
  VariantInfo,
};
pub use layout::{Layout, LayoutBuilder, align_forward};
pub use table::Types;

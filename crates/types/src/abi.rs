//! The System V x86-64 classification of a value passed across a `#c_call`
//! boundary (**L§7.11**).
//!
//! Between two Jai procedures the convention is orangejuice's own, and an
//! aggregate goes by pointer. A `#c_call` has to follow the platform's, where
//! an aggregate of at most two eightbytes travels in registers chosen by what
//! its bytes are — so this is where the compiler works out which.

use crate::kind::{ArrayKind, FloatKind, MemberFlags, TypeId, TypeKind};
use crate::table::Types;

/// What the register a value's eightbyte travels in has to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eightbyte {
  /// A general-purpose register.
  Integer,
  /// A vector register, which every byte of this eightbyte being a float is
  /// what earns.
  Sse,
}

/// How a value crosses a `#c_call` boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Classification {
  /// It fits in registers: one per eightbyte, in order.
  Registers(Vec<Eightbyte>),
  /// It is copied into the caller's argument area, or written through storage
  /// the caller supplied.
  Memory,
}

/// The eightbyte classes before merging: `None` is the SysV NO_CLASS.
type Slots = [Option<Eightbyte>; 2];

/// Classifies a value of `type_id` (**L§7.11**). Anything wider than two
/// eightbytes, and anything the compiler cannot measure, goes through memory.
pub fn classify(types: &Types, type_id: TypeId) -> Classification {
  let Some(size) = types.size_of(type_id) else {
    return Classification::Memory;
  };
  if size == 0 || size > 16 {
    return Classification::Memory;
  }
  let mut slots: Slots = [None, None];
  if !classify_into(types, type_id, 0, &mut slots, 0) {
    return Classification::Memory;
  }
  let used = size.div_ceil(8) as usize;
  let mut classes = Vec::with_capacity(used);
  for slot in slots.iter().take(used) {
    // An eightbyte no field reached is padding, and padding travels in a
    // general-purpose register like anything else the caller does not read.
    classes.push(slot.unwrap_or(Eightbyte::Integer));
  }
  Classification::Registers(classes)
}

/// Merges what sits at `offset` into the eightbytes it covers. `false` means
/// the whole value has to go through memory.
fn classify_into(
  types: &Types,
  type_id: TypeId,
  offset: u64,
  slots: &mut Slots,
  depth: u32,
) -> bool {
  if depth > MAX_DEPTH {
    return false;
  }
  let underlying = types.underlying(type_id);
  let Some(size) = types.size_of(underlying) else {
    return false;
  };
  if size == 0 {
    return true;
  }
  if offset + size > 16 {
    return false;
  }
  match types.kind(underlying) {
    TypeKind::Float(_) | TypeKind::UntypedFloat(_) => merge(slots, offset, Eightbyte::Sse),
    TypeKind::Bool
    | TypeKind::Integer(_)
    | TypeKind::UntypedInt
    | TypeKind::Pointer(_)
    | TypeKind::Procedure(_)
    | TypeKind::Type
    | TypeKind::Code => merge(slots, offset, Eightbyte::Integer),
    TypeKind::Enum(id) => {
      let base = types.enum_info(*id).base;
      classify_into(types, base, offset, slots, depth + 1)
    }
    // A `string`, an `Any` and a view are two words: a count and a pointer.
    TypeKind::String | TypeKind::Any => {
      merge(slots, offset, Eightbyte::Integer) && merge(slots, offset + 8, Eightbyte::Integer)
    }
    TypeKind::Array { element, kind } => match kind {
      ArrayKind::Fixed(count) => {
        let (element, count) = (*element, *count);
        let Some(stride) = types.size_of(element) else {
          return false;
        };
        if stride == 0 {
          return true;
        }
        (0..count)
          .all(|index| classify_into(types, element, offset + index * stride, slots, depth + 1))
      }
      // A view is `{count, data}`; a resizable array is wider than two
      // eightbytes and never reaches here.
      _ => merge(slots, offset, Eightbyte::Integer) && merge(slots, offset + 8, Eightbyte::Integer),
    },
    TypeKind::Struct(id) => {
      let members = types.struct_info(*id).members.clone();
      members
        .iter()
        .filter(|member| {
          member.imported_through.is_none()
            && !member
              .flags
              .intersects(MemberFlags::CONSTANT | MemberFlags::IMPORTED)
        })
        .all(|member| {
          classify_into(
            types,
            member.type_id,
            offset + member.offset,
            slots,
            depth + 1,
          )
        })
    }
    // `V128` is a vector register's worth of bytes, which is what it is for.
    TypeKind::V128 => {
      merge(slots, offset, Eightbyte::Sse) && merge(slots, offset + 8, Eightbyte::Sse)
    }
    _ => false,
  }
}

/// SysV's merge: NO_CLASS takes whatever arrives, and INTEGER wins over SSE,
/// because a general-purpose register can carry either.
fn merge(slots: &mut Slots, offset: u64, class: Eightbyte) -> bool {
  let index = (offset / 8) as usize;
  if index >= slots.len() {
    return false;
  }
  slots[index] = match (slots[index], class) {
    (None, class) => Some(class),
    (Some(Eightbyte::Sse), Eightbyte::Sse) => Some(Eightbyte::Sse),
    _ => Some(Eightbyte::Integer),
  };
  true
}

/// Deep enough for the nesting the module tree has, and a value of at most two
/// eightbytes cannot be deeper than this in practice.
const MAX_DEPTH: u32 = 16;

/// The width of one eightbyte's LLVM type: what is left of the value at that
/// offset, capped at eight bytes.
pub fn eightbyte_size(total: u64, index: usize) -> u64 {
  let start = index as u64 * 8;
  (total.saturating_sub(start)).min(8)
}

impl FloatKind {
  /// Whether a float of this kind fills a whole eightbyte, which is what says
  /// an SSE eightbyte is a `double` rather than two `float`s.
  pub fn is_wide(self) -> bool {
    matches!(self, FloatKind::F64)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::kind::{StructInfo, StructMember, StructTextualFlags};

  /// A struct whose members sit where the test says, and whose size the test
  /// gives, so that classification is read off the layout alone.
  fn structure(types: &mut Types, members: &[(TypeId, u64)], size: u64) -> TypeId {
    let (id, type_id) = types.new_struct(StructInfo::new(None, StructTextualFlags::empty()));
    for (type_id, offset) in members {
      types.struct_info_mut(id).members.push(StructMember {
        name: oj_lexer::Symbol(0),
        type_id: *type_id,
        offset: *offset,
        flags: MemberFlags::empty(),
        imported_through: None,
      });
    }
    types.finish_struct(id, size, 8);
    type_id
  }

  #[test]
  fn a_scalar_is_one_register_of_its_own_kind() {
    let types = Types::new();
    assert_eq!(
      classify(&types, TypeId::S64),
      Classification::Registers(vec![Eightbyte::Integer])
    );
  }

  #[test]
  fn two_words_of_integers_travel_in_two_general_purpose_registers() {
    let mut types = Types::new();
    let id = structure(&mut types, &[(TypeId::S64, 0), (TypeId::S64, 8)], 16);
    assert_eq!(
      classify(&types, id),
      Classification::Registers(vec![Eightbyte::Integer, Eightbyte::Integer])
    );
  }

  #[test]
  fn a_pair_of_floats_travels_in_one_vector_register() {
    let mut types = Types::new();
    let float32 = types.float(FloatKind::F32);
    let id = structure(&mut types, &[(float32, 0), (float32, 4)], 8);
    assert_eq!(
      classify(&types, id),
      Classification::Registers(vec![Eightbyte::Sse])
    );
  }

  #[test]
  fn a_float_beside_an_integer_takes_one_register_of_each() {
    let mut types = Types::new();
    let float64 = types.float(FloatKind::F64);
    let id = structure(&mut types, &[(float64, 0), (TypeId::S64, 8)], 16);
    assert_eq!(
      classify(&types, id),
      Classification::Registers(vec![Eightbyte::Sse, Eightbyte::Integer])
    );
  }

  #[test]
  fn an_integer_sharing_a_word_with_a_float_makes_it_a_general_purpose_one() {
    let mut types = Types::new();
    let float32 = types.float(FloatKind::F32);
    let id = structure(&mut types, &[(TypeId::S32, 0), (float32, 4)], 8);
    assert_eq!(
      classify(&types, id),
      Classification::Registers(vec![Eightbyte::Integer])
    );
  }

  #[test]
  fn anything_wider_than_two_words_goes_through_memory() {
    let mut types = Types::new();
    let id = structure(
      &mut types,
      &[(TypeId::S64, 0), (TypeId::S64, 8), (TypeId::S64, 16)],
      24,
    );
    assert_eq!(classify(&types, id), Classification::Memory);
  }

  #[test]
  fn a_string_is_a_count_and_a_pointer() {
    let types = Types::new();
    assert_eq!(
      classify(&types, TypeId::STRING),
      Classification::Registers(vec![Eightbyte::Integer, Eightbyte::Integer])
    );
  }
}

use crate::kind::{ArrayKind, TypeId, TypeKind};
use crate::table::Types;

/// A type's storage: its size including trailing padding, and its alignment
/// (**L§3.14**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
  pub size: u64,
  pub alignment: u64,
}

impl Layout {
  pub const fn new(size: u64, alignment: u64) -> Self {
    Self { size, alignment }
  }

  /// A scalar of `size` bytes, whose natural alignment is its size.
  pub const fn scalar(size: u64) -> Self {
    Self::new(size, if size == 0 { 1 } else { size })
  }
}

/// Rounds `offset` up to the next multiple of `alignment`.
pub fn align_forward(offset: u64, alignment: u64) -> u64 {
  if alignment <= 1 {
    return offset;
  }
  offset.div_ceil(alignment) * alignment
}

/// `{ count: s64; data: *T }` — the shape of `string` and of a view (**L§3.3**).
const VIEW: Layout = Layout::new(16, 8);
/// `{ count; data; allocated; allocator: Allocator }` (**L§3.3**).
const RESIZABLE: Layout = Layout::new(40, 8);

impl Types {
  /// The storage of a type, or `None` while it is not knowable: an incomplete
  /// struct, a literal that has not taken a type yet, a polymorph variable.
  pub fn layout(&self, id: TypeId) -> Option<Layout> {
    match self.kind(id) {
      TypeKind::Void => Some(Layout::new(0, 1)),
      TypeKind::Bool => Some(Layout::scalar(1)),
      TypeKind::Integer(kind) => Some(Layout::scalar(kind.size())),
      TypeKind::Float(kind) => Some(Layout::scalar(kind.size())),
      TypeKind::String => Some(VIEW),
      TypeKind::Any => Some(VIEW),
      TypeKind::Code | TypeKind::Type | TypeKind::Pointer(_) | TypeKind::Procedure(_) => {
        Some(Layout::scalar(8))
      }
      TypeKind::V128 => Some(Layout::new(16, 16)),
      TypeKind::Array { element, kind } => match kind {
        ArrayKind::Fixed(count) => {
          let element = self.layout(*element)?;
          Some(Layout::new(element.size * count, element.alignment))
        }
        ArrayKind::View => Some(VIEW),
        ArrayKind::Resizable => Some(RESIZABLE),
      },
      TypeKind::Struct(id) => {
        let info = self.struct_info(*id);
        info
          .complete
          .then(|| Layout::new(info.size, info.alignment))
      }
      TypeKind::Enum(id) => self.layout(self.enum_info(*id).base),
      TypeKind::Variant(id) => self.layout(self.variant_info(*id).base),
      TypeKind::Polymorph(_)
      | TypeKind::UntypedInt
      | TypeKind::UntypedFloat(_)
      | TypeKind::UntypedEnum
      | TypeKind::UntypedLiteral
      | TypeKind::OverloadSet
      | TypeKind::Unknown => None,
    }
  }

  pub fn size_of(&self, id: TypeId) -> Option<u64> {
    self.layout(id).map(|layout| layout.size)
  }

  pub fn align_of(&self, id: TypeId) -> Option<u64> {
    self.layout(id).map(|layout| layout.alignment)
  }
}

/// Lays members out in declaration order, the way **L§3.14** and **L§8.6**
/// prescribe: never reordered, each padded to its own alignment, all at offset
/// 0 in a union, and rewound to an earlier member's offset by `#place`.
#[derive(Clone, Debug)]
pub struct LayoutBuilder {
  cursor: u64,
  extent: u64,
  alignment: u64,
  union: bool,
  no_padding: bool,
}

impl LayoutBuilder {
  /// `no_padding` is the struct's `#no_padding`, which drops the *trailing*
  /// padding only: members keep their natural offsets and the struct keeps its
  /// alignment, but its size is not rounded up (**L§8.7**).
  pub fn new(union: bool, no_padding: bool) -> Self {
    Self {
      cursor: 0,
      extent: 0,
      alignment: 1,
      union,
      no_padding,
    }
  }

  /// Places one member and returns its offset. `explicit_alignment` is the
  /// member's `#align N`, which replaces the type's natural alignment and may
  /// lower it.
  pub fn place(&mut self, layout: Layout, explicit_alignment: Option<u64>) -> u64 {
    let alignment = explicit_alignment.unwrap_or(layout.alignment).max(1);
    let offset = align_forward(self.cursor, alignment);

    self.alignment = self.alignment.max(alignment);
    self.extent = self.extent.max(offset + layout.size);
    if !self.union {
      self.cursor = offset + layout.size;
    }
    offset
  }

  /// `#place member;` — subsequent members overlay memory from `offset` on
  /// (**L§8.6**).
  pub fn rewind_to(&mut self, offset: u64) {
    self.cursor = offset;
  }

  pub fn cursor(&self) -> u64 {
    self.cursor
  }

  pub fn alignment(&self) -> u64 {
    self.alignment
  }

  pub fn finish(self) -> Layout {
    let size = if self.no_padding {
      self.extent
    } else {
      align_forward(self.extent, self.alignment)
    };
    Layout::new(size, self.alignment)
  }
}

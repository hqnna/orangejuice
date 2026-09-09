//! Materializing the type table (**L§17**, `docs/spec.md` §6.1).
//!
//! `type_info(T)` is a pointer to a `Type_Info` the program can read at
//! runtime, so the compiler has to lay one out for every type anything asks
//! about — and for every type those mention, transitively. The whole graph
//! becomes one image: a byte array with a list of the places inside it that
//! hold a pointer to somewhere else inside it, so that placing the image is a
//! matter of adding its address to each of those slots.
//!
//! The field offsets are not written down here. They are read out of the
//! `Type_Info*` structs the program's own Preload declares, which is what
//! keeps the image right when those structs change.

use std::collections::HashMap;

use oj_lexer::Interner;
use oj_sema::Checker;
use oj_types::{ArrayKind, ProcedureFlags, StructId, TypeId, TypeKind};

/// The size of a `string` and of a `[] T`: a count and a pointer
/// (**L§3.3**, **L§3.4**).
const VIEW_SIZE: u64 = 16;
const VIEW_COUNT: u64 = 0;
const VIEW_DATA: u64 = 8;

/// A pointer's size on the one target orangejuice has (`docs/spec.md` §2).
const POINTER_SIZE: u64 = 8;

/// The image of the type table, and where each type's `Type_Info` sits in it.
#[derive(Default)]
pub(crate) struct TypeTable {
  bytes: Vec<u8>,
  /// `(at, target)`: the eight bytes at `at` hold the address of `target`,
  /// both relative to wherever the image is placed.
  relocations: Vec<(u64, u64)>,
  offsets: HashMap<TypeId, u64>,
  /// Character data, shared between every name that spells the same thing.
  data: HashMap<Vec<u8>, u64>,
  records: Option<Records>,
  /// Whether the `Runtime_Info` header has been reserved at offset 0. The
  /// image *is* `__runtime_info`, so the struct the program reads has to be
  /// the first thing in it (**C§3.3**).
  reserved: bool,
}

/// The size of the `Runtime_Info` the image begins with when the program does
/// not declare one to measure: a `[] *Type_Info` and a `*Global_Data_Info`,
/// which is what this target's ABI makes of them (**C§3.3**).
const RUNTIME_INFO_FALLBACK: u64 = 24;

/// The `Type_Info*` structs Preload declares, and the tag values it gives
/// them. Everything the image knows about its own shape comes from here.
struct Records {
  info: Option<Record>,
  integer: Option<Record>,
  float: Option<Record>,
  string: Option<Record>,
  pointer: Option<Record>,
  procedure: Option<Record>,
  structure: Option<Record>,
  member: Option<Record>,
  array: Option<Record>,
  enumeration: Option<Record>,
  variant: Option<Record>,
  tags: HashMap<String, u32>,
}

/// One `Type_Info*` struct: what it is called in the type table, and the
/// definition whose member offsets say where its fields sit.
#[derive(Clone, Copy)]
struct Record {
  definition: StructId,
  type_id: TypeId,
}

impl TypeTable {
  pub(crate) fn bytes(&self) -> &[u8] {
    &self.bytes
  }

  /// Where each type's record ended up, so that an address into the image can
  /// be read back as the type it belongs to (**L§17**).
  pub(crate) fn placements(&self) -> Vec<(TypeId, u64)> {
    let mut placements: Vec<(TypeId, u64)> = self
      .offsets
      .iter()
      .map(|(type_id, offset)| (*type_id, *offset))
      .collect();
    placements.sort_unstable();
    placements
  }

  pub(crate) fn relocations(&self) -> &[(u64, u64)] {
    &self.relocations
  }

  /// Reserves the `Runtime_Info` the image begins with, whose `type_table` is
  /// what `get_type_table` reads at runtime (**C§3.3**). The size comes from
  /// the program's own declaration when it imports `Compiler`, since the
  /// compiler never writes a distribution struct's layout down.
  fn reserve_header(&mut self, checker: &mut Checker) {
    if self.reserved {
      return;
    }
    self.reserved = true;
    let size = checker
      .type_named("Runtime_Info")
      .and_then(|type_id| checker.layout(type_id))
      .map_or(RUNTIME_INFO_FALLBACK, |layout| {
        layout.size.max(RUNTIME_INFO_FALLBACK)
      });
    let at = self.place(size, POINTER_SIZE);
    debug_assert_eq!(at, 0, "the header is the first thing placed");
  }

  /// Closes the image: every type it holds gets a slot in the `[] *Type_Info`
  /// the header points at, which is the table a program walks (**L§17**).
  pub(crate) fn finish(&mut self, checker: &mut Checker) {
    self.reserve_header(checker);
    let mut placed = self.placements();
    placed.sort_unstable_by_key(|(_, offset)| *offset);
    let (data, count) = self.pointer_array(&placed);
    self.write_i64(VIEW_COUNT, count as i64);
    self.write_pointer(VIEW_DATA, data);
  }

  /// An array of `*Type_Info`, one per type in the image, each pointing at
  /// that type's record.
  fn pointer_array(&mut self, placed: &[(TypeId, u64)]) -> (u64, usize) {
    if placed.is_empty() {
      return (0, 0);
    }
    let at = self.place(placed.len() as u64 * POINTER_SIZE, POINTER_SIZE);
    for (index, (_, offset)) in placed.iter().enumerate() {
      self.write_pointer(at + index as u64 * POINTER_SIZE, *offset);
    }
    (at, placed.len())
  }

  /// Where a type's `Type_Info` sits in the image, laying it — and whatever it
  /// points at — out the first time it is asked for.
  pub(crate) fn offset_of(&mut self, checker: &mut Checker, type_id: TypeId) -> u64 {
    self.reserve_header(checker);
    if let Some(offset) = self.offsets.get(&type_id) {
      return *offset;
    }
    let records = self.records(checker);
    let record = records.record_for(checker.types().kind(type_id));
    let (size, alignment) = match record.and_then(|record| checker.layout(record.type_id)) {
      Some(layout) => (layout.size.max(1), layout.alignment.max(1)),
      // Without Preload there is nothing to lay out against; a bare header
      // still gives the program something with a stable address.
      None => (16, 8),
    };
    let at = self.place(size, alignment);
    // The offset is recorded before the record is filled in, so that a type
    // that mentions itself — `Node` with a `*Node` member — terminates.
    self.offsets.insert(type_id, at);
    self.fill(checker, type_id, at, record);
    at
  }

  fn fill(&mut self, checker: &mut Checker, type_id: TypeId, at: u64, record: Option<Record>) {
    let Some(record) = record else { return };
    let tag = self.tag_of(checker, type_id);
    let size = checker
      .layout(type_id)
      .map_or(-1i64, |layout| layout.size as i64);
    self.write_field_u32(checker, record, at, "type", tag);
    self.write_field_i64(checker, record, at, "runtime_size", size);

    let kind = checker.types().kind(type_id).clone();
    match kind {
      TypeKind::Integer(integer) => {
        let signed = u32::from(integer.is_signed());
        self.write_field_u32(checker, record, at, "signed", signed);
      }
      TypeKind::UntypedInt => self.write_field_u32(checker, record, at, "signed", 1),
      TypeKind::Pointer(pointee) => {
        let target = self.offset_of(checker, pointee);
        self.write_field_pointer(checker, record, at, "pointer_to", target);
      }
      TypeKind::Array { element, kind } => self.fill_array(checker, record, at, element, kind),
      TypeKind::Procedure(signature) => {
        let arguments = self.info_list(checker, &signature.arguments);
        let returns = self.info_list(checker, &signature.returns);
        self.write_field_view(checker, record, at, "argument_types", arguments);
        self.write_field_view(checker, record, at, "return_types", returns);
        let flags = procedure_flags(signature.flags);
        self.write_field_u32(checker, record, at, "procedure_flags", flags);
      }
      TypeKind::Struct(definition) => self.fill_struct(checker, record, at, type_id, definition),
      TypeKind::Enum(definition) => self.fill_enum(checker, record, at, definition),
      TypeKind::Variant(definition) => self.fill_variant(checker, record, at, definition),
      _ => {}
    }
  }

  fn fill_array(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    element: TypeId,
    kind: ArrayKind,
  ) {
    let target = self.offset_of(checker, element);
    self.write_field_pointer(checker, record, at, "element_type", target);
    let (which, count) = match kind {
      ArrayKind::Fixed(count) => (0u32, count as i64),
      ArrayKind::View => (1, -1),
      ArrayKind::Resizable => (2, -1),
    };
    // `Array_Type` is an `enum u16` (**L§17**).
    self.write_field_u16(checker, record, at, "array_type", which as u16);
    self.write_field_i64(checker, record, at, "array_count", count);
  }

  fn fill_struct(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    type_id: TypeId,
    definition: StructId,
  ) {
    let info = checker.types().struct_info(definition).clone();
    let name = info
      .name
      .map(|name| checker.interner().resolve_lossy(name).into_owned())
      .unwrap_or_default();
    self.write_field_string(checker, record, at, "name", name.as_bytes());
    self.write_field_u32(
      checker,
      record,
      at,
      "textual_flags",
      info.textual_flags.bits(),
    );
    self.write_field_u32(
      checker,
      record,
      at,
      "nontextual_flags",
      info.nontextual_flags.bits(),
    );
    // `Struct_Status_Flags.INCOMPLETE` is the only bit the front end can set
    // here; the rest belong to a metaprogram's view of the program.
    let status = u32::from(!info.complete);
    self.write_field_u32(checker, record, at, "status_flags", status);

    // What a `compiler_set_type_info_flags` asked for joins what the struct
    // was declared with; nothing ever clears either (**C§3.3**).
    let set = checker.type_info_flags_of(type_id);
    let none = info
      .textual_flags
      .contains(oj_types::StructTextualFlags::TYPE_INFO_NONE)
      || set & TYPE_INFO_NONE != 0;
    // `#type_info_none` makes the struct look empty at runtime, which is what
    // keeps a big one out of the executable (**L§8.7**); `print` reads the
    // same members, so it shows nothing either.
    if none {
      self.write_field_view(checker, record, at, "members", (0, 0));
      return;
    }
    // `#type_info_procedures_are_void_pointers` keeps the names and the
    // offsets but not the signatures, which is what a struct of 800 procedure
    // declarations wants (**L§8.7**).
    let erase = info
      .textual_flags
      .contains(oj_types::StructTextualFlags::TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS)
      || set & TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS != 0;
    let members = self.member_array(checker, &info.members, erase);
    self.write_field_view(checker, record, at, "members", members);
  }

  /// The `Type_Info_Struct_Member` array a struct's `members` view points at.
  ///
  /// `erase` is `#type_info_procedures_are_void_pointers`: a member of
  /// procedure type is described as a `*void` instead, so its signature never
  /// reaches the executable (**L§8.7**).
  fn member_array(
    &mut self,
    checker: &mut Checker,
    members: &[oj_types::StructMember],
    erase: bool,
  ) -> (u64, u64) {
    let Some(record) = self.records(checker).member else {
      return (0, 0);
    };
    let (size, alignment) = match checker.layout(record.type_id) {
      Some(layout) => (layout.size.max(1), layout.alignment.max(1)),
      None => return (0, 0),
    };
    // A member's type has to be laid out before the array is placed, or the
    // recursion would move the cursor out from under it.
    let entries: Vec<(String, u64, u64, u32)> = members
      .iter()
      .map(|member| {
        let name = checker.interner().resolve_lossy(member.name).into_owned();
        let described = match erase && is_procedure(checker, member.type_id) {
          true => TypeId::VOID_POINTER,
          false => member.type_id,
        };
        let type_offset = self.offset_of(checker, described);
        (name, type_offset, member.offset, member.flags.bits())
      })
      .collect();

    let base = self.place(size * entries.len() as u64, alignment);
    for (index, (name, type_offset, offset, flags)) in entries.into_iter().enumerate() {
      let at = base + size * index as u64;
      self.write_field_string(checker, record, at, "name", name.as_bytes());
      self.write_field_pointer(checker, record, at, "type", type_offset);
      self.write_field_i64(checker, record, at, "offset_in_bytes", offset as i64);
      self.write_field_u32(checker, record, at, "flags", flags);
      self.write_field_i64(checker, record, at, "offset_into_constant_storage", -1);
    }
    (base, members.len() as u64)
  }

  fn fill_enum(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    definition: oj_types::EnumId,
  ) {
    let info = checker.types().enum_info(definition).clone();
    let name = info
      .name
      .map(|name| checker.interner().resolve_lossy(name).into_owned())
      .unwrap_or_default();
    self.write_field_string(checker, record, at, "name", name.as_bytes());

    let base = self.offset_of(checker, info.base);
    self.write_field_pointer(checker, record, at, "internal_type", base);

    let names: Vec<String> = info
      .members
      .iter()
      .map(|member| checker.interner().resolve_lossy(member.name).into_owned())
      .collect();
    let name_array = self.string_array(&names);
    self.write_field_view(checker, record, at, "names", name_array);

    let values: Vec<i64> = info.members.iter().map(|member| member.value).collect();
    let value_array = self.i64_array(&values);
    self.write_field_view(checker, record, at, "values", value_array);

    self.write_field_u16(
      checker,
      record,
      at,
      "status_flags",
      u16::from(!info.complete),
    );
    self.write_field_u16(
      checker,
      record,
      at,
      "enum_type_flags",
      info.flags.bits() as u16,
    );
  }

  fn fill_variant(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    definition: oj_types::VariantId,
  ) {
    let info = checker.types().variant_info(definition).clone();
    let name = info
      .name
      .map(|name| checker.interner().resolve_lossy(name).into_owned())
      .unwrap_or_default();
    self.write_field_string(checker, record, at, "name", name.as_bytes());
    let base = self.offset_of(checker, info.base);
    self.write_field_pointer(checker, record, at, "variant_of", base);
    self.write_field_u32(checker, record, at, "variant_flags", info.flags.bits());
  }

  // ------------------------------------------------------------ arrays ------

  /// A `[] *Type_Info`, laid out after every element's info exists.
  fn info_list(&mut self, checker: &mut Checker, types: &[TypeId]) -> (u64, u64) {
    let targets: Vec<u64> = types
      .iter()
      .map(|type_id| self.offset_of(checker, *type_id))
      .collect();
    let base = self.place(POINTER_SIZE * targets.len() as u64, POINTER_SIZE);
    for (index, target) in targets.iter().enumerate() {
      self.write_pointer(base + POINTER_SIZE * index as u64, *target);
    }
    (base, targets.len() as u64)
  }

  fn string_array(&mut self, values: &[String]) -> (u64, u64) {
    let data: Vec<u64> = values
      .iter()
      .map(|value| self.intern(value.as_bytes()))
      .collect();
    let base = self.place(VIEW_SIZE * data.len() as u64, POINTER_SIZE);
    for (index, offset) in data.iter().enumerate() {
      let at = base + VIEW_SIZE * index as u64;
      self.write_i64(at, values[index].len() as i64);
      self.write_pointer(at + VIEW_DATA, *offset);
    }
    (base, values.len() as u64)
  }

  fn i64_array(&mut self, values: &[i64]) -> (u64, u64) {
    let base = self.place(8 * values.len() as u64, 8);
    for (index, value) in values.iter().enumerate() {
      self.write_i64(base + 8 * index as u64, *value);
    }
    (base, values.len() as u64)
  }

  // ------------------------------------------------------------ writing -----

  fn place(&mut self, size: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1) as usize;
    let padding = (alignment - self.bytes.len() % alignment) % alignment;
    self.bytes.resize(self.bytes.len() + padding, 0);
    let at = self.bytes.len() as u64;
    self.bytes.resize(self.bytes.len() + size as usize, 0);
    at
  }

  fn intern(&mut self, text: &[u8]) -> u64 {
    if let Some(offset) = self.data.get(text) {
      return *offset;
    }
    let at = self.place(text.len().max(1) as u64, 1);
    self.bytes[at as usize..at as usize + text.len()].copy_from_slice(text);
    self.data.insert(text.to_vec(), at);
    at
  }

  fn write_u16(&mut self, at: u64, value: u16) {
    self.write(at, &value.to_le_bytes());
  }

  fn write_u32(&mut self, at: u64, value: u32) {
    self.write(at, &value.to_le_bytes());
  }

  fn write_i64(&mut self, at: u64, value: i64) {
    self.write(at, &value.to_le_bytes());
  }

  fn write_pointer(&mut self, at: u64, target: u64) {
    self.relocations.push((at, target));
  }

  fn write(&mut self, at: u64, source: &[u8]) {
    let at = at as usize;
    let end = (at + source.len()).min(self.bytes.len());
    if at >= end {
      return;
    }
    self.bytes[at..end].copy_from_slice(&source[..end - at]);
  }

  // ------------------------------------------------------------- fields -----

  fn field(&mut self, checker: &mut Checker, record: Record, name: &str) -> Option<u64> {
    let symbol = checker.interner().intern(name.as_bytes());
    checker
      .types()
      .struct_info(record.definition)
      .members
      .iter()
      .find(|member| member.name == symbol && !member.is_constant())
      .map(|member| member.offset)
  }

  fn write_field_u16(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    name: &str,
    value: u16,
  ) {
    if let Some(offset) = self.field(checker, record, name) {
      self.write_u16(at + offset, value);
    }
  }

  fn write_field_u32(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    name: &str,
    value: u32,
  ) {
    if let Some(offset) = self.field(checker, record, name) {
      self.write_u32(at + offset, value);
    }
  }

  fn write_field_i64(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    name: &str,
    value: i64,
  ) {
    if let Some(offset) = self.field(checker, record, name) {
      self.write_i64(at + offset, value);
    }
  }

  fn write_field_pointer(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    name: &str,
    target: u64,
  ) {
    if let Some(offset) = self.field(checker, record, name) {
      self.write_pointer(at + offset, target);
    }
  }

  fn write_field_string(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    name: &str,
    text: &[u8],
  ) {
    let Some(offset) = self.field(checker, record, name) else {
      return;
    };
    let data = self.intern(text);
    self.write_i64(at + offset, text.len() as i64);
    self.write_pointer(at + offset + VIEW_DATA, data);
  }

  fn write_field_view(
    &mut self,
    checker: &mut Checker,
    record: Record,
    at: u64,
    name: &str,
    (data, count): (u64, u64),
  ) {
    let Some(offset) = self.field(checker, record, name) else {
      return;
    };
    self.write_i64(at + offset, count as i64);
    if count > 0 {
      self.write_pointer(at + offset + VIEW_DATA, data);
    }
  }

  // -------------------------------------------------------------- Preload ---

  fn records(&mut self, checker: &mut Checker) -> &Records {
    if self.records.is_none() {
      self.records = Some(Records::resolve(checker));
    }
    self.records.as_ref().expect("the records were just built")
  }

  fn tag_of(&mut self, checker: &mut Checker, type_id: TypeId) -> u32 {
    let name = tag_name(checker.types().kind(type_id));
    self.records(checker).tags.get(name).copied().unwrap_or(0)
  }
}

impl Records {
  fn resolve(checker: &mut Checker) -> Self {
    let lookup = |checker: &mut Checker, name: &str| {
      let type_id = checker.preload_named_type(name);
      match checker.types().kind(type_id) {
        TypeKind::Struct(definition) => Some(Record {
          definition: *definition,
          type_id,
        }),
        _ => None,
      }
    };
    let mut tags = HashMap::new();
    let tag_type = checker.preload_named_type("Type_Info_Tag");
    if let Some(definition) = checker.types().enum_of(tag_type) {
      let interner: &Interner = checker.interner();
      for member in &checker.types().enum_info(definition).members {
        tags.insert(
          interner.resolve_lossy(member.name).into_owned(),
          member.value as u32,
        );
      }
    }
    Self {
      info: lookup(checker, "Type_Info"),
      integer: lookup(checker, "Type_Info_Integer"),
      float: lookup(checker, "Type_Info_Float"),
      string: lookup(checker, "Type_Info_String"),
      pointer: lookup(checker, "Type_Info_Pointer"),
      procedure: lookup(checker, "Type_Info_Procedure"),
      structure: lookup(checker, "Type_Info_Struct"),
      member: lookup(checker, "Type_Info_Struct_Member"),
      array: lookup(checker, "Type_Info_Array"),
      enumeration: lookup(checker, "Type_Info_Enum"),
      variant: lookup(checker, "Type_Info_Variant"),
      tags,
    }
  }

  /// The `Type_Info*` struct a type's info is an instance of. The reference
  /// downcasts `type_info(x)` to the most specific one (**L§17**).
  fn record_for(&self, kind: &TypeKind) -> Option<Record> {
    match kind {
      TypeKind::Integer(_) | TypeKind::UntypedInt => self.integer,
      TypeKind::Float(_) | TypeKind::UntypedFloat(_) => self.float,
      TypeKind::String => self.string,
      TypeKind::Pointer(_) => self.pointer,
      TypeKind::Procedure(_) => self.procedure,
      TypeKind::Struct(_) => self.structure,
      TypeKind::Array { .. } => self.array,
      TypeKind::Enum(_) => self.enumeration,
      TypeKind::Variant(_) => self.variant,
      _ => self.info,
    }
  }
}

/// The `Type_Info_Tag` member a type carries (**L§17**).
fn tag_name(kind: &TypeKind) -> &'static str {
  match kind {
    TypeKind::Integer(_) | TypeKind::UntypedInt => "INTEGER",
    TypeKind::Float(_) | TypeKind::UntypedFloat(_) => "FLOAT",
    TypeKind::Bool => "BOOL",
    TypeKind::String => "STRING",
    TypeKind::Pointer(_) => "POINTER",
    TypeKind::Procedure(_) => "PROCEDURE",
    TypeKind::Void => "VOID",
    TypeKind::Struct(_) => "STRUCT",
    TypeKind::Array { .. } => "ARRAY",
    TypeKind::OverloadSet => "OVERLOAD_SET",
    TypeKind::Any => "ANY",
    TypeKind::Enum(_) => "ENUM",
    TypeKind::Polymorph(_) => "POLYMORPHIC_VARIABLE",
    TypeKind::Type => "TYPE",
    TypeKind::Code => "CODE",
    TypeKind::UntypedLiteral => "UNTYPED_LITERAL",
    TypeKind::UntypedEnum => "UNTYPED_ENUM",
    TypeKind::Variant(_) => "VARIANT",
    _ => "VOID",
  }
}

/// `Type_Info_Procedure.Flags`, which are the same bits `oj-types` keeps
/// (**L§17**).
fn procedure_flags(flags: ProcedureFlags) -> u32 {
  flags.bits()
}

/// Whether a member is a procedure, which is what
/// `#type_info_procedures_are_void_pointers` describes as a `*void` instead
/// (**L§8.7**).
fn is_procedure(checker: &mut Checker, type_id: TypeId) -> bool {
  let underlying = checker.types().underlying(type_id);
  matches!(checker.types().kind(underlying), TypeKind::Procedure(_))
}

/// `Type_Info_Flags` as the distribution numbers it (**C§3.3**). These are not
/// `Struct_Textual_Flags`' bits, which is why they are named apart.
const TYPE_INFO_NONE: u32 = 0x1;
const TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS: u32 = 0x2;

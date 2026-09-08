use oj_lexer::Interner;
use oj_types::{
  ArrayKind, EnumInfo, EnumTypeFlags, Layout, LayoutBuilder, MemberFlags, PolymorphInfo,
  ProcedureFlags, ProcedureType, StructInfo, StructMember, StructTextualFlags, TypeId, TypeKind,
  Types, VariantFlags, VariantInfo, align_forward,
};

fn member(name: oj_lexer::Symbol, type_id: TypeId, offset: u64) -> StructMember {
  StructMember {
    name,
    type_id,
    offset,
    flags: MemberFlags::empty(),
    imported_through: None,
  }
}

/// Lays a struct out the way sema does, so the layout rules of **L§3.14** can
/// be checked without the typechecker.
fn build_struct(
  types: &mut Types,
  name: Option<oj_lexer::Symbol>,
  flags: StructTextualFlags,
  members: &[(oj_lexer::Symbol, TypeId, Option<u64>)],
) -> TypeId {
  let mut info = StructInfo::new(name, flags);
  let mut builder = LayoutBuilder::new(
    flags.contains(StructTextualFlags::UNION),
    flags.contains(StructTextualFlags::NO_PADDING),
  );
  for (name, type_id, alignment) in members {
    let layout = types.layout(*type_id).expect("member type is complete");
    let offset = builder.place(layout, *alignment);
    info.members.push(member(*name, *type_id, offset));
  }
  let layout = builder.finish();
  let (id, type_id) = types.new_struct(info);
  types.finish_struct(id, layout.size, layout.alignment);
  type_id
}

#[test]
fn the_basic_types_have_the_sizes_of_the_table() {
  let types = Types::new();
  for (id, size, alignment) in [
    (TypeId::VOID, 0, 1),
    (TypeId::BOOL, 1, 1),
    (TypeId::S8, 1, 1),
    (TypeId::S16, 2, 2),
    (TypeId::S32, 4, 4),
    (TypeId::S64, 8, 8),
    (TypeId::U64, 8, 8),
    (TypeId::FLOAT32, 4, 4),
    (TypeId::FLOAT64, 8, 8),
    (TypeId::STRING, 16, 8),
    (TypeId::ANY, 16, 8),
    (TypeId::CODE, 8, 8),
    (TypeId::TYPE, 8, 8),
    (TypeId::V128, 16, 16),
    (TypeId::VOID_POINTER, 8, 8),
  ] {
    assert_eq!(
      types.layout(id),
      Some(Layout::new(size, alignment)),
      "layout of type {}",
      id.0
    );
  }
}

#[test]
fn int_is_s64_and_float_is_float32() {
  assert_eq!(TypeId::INT, TypeId::S64);
  assert_eq!(TypeId::FLOAT, TypeId::FLOAT32);
}

#[test]
fn array_views_and_resizable_arrays_have_their_fixed_shapes() {
  let mut types = Types::new();
  let view = types.array(TypeId::S32, ArrayKind::View);
  let resizable = types.array(TypeId::S32, ArrayKind::Resizable);
  let fixed = types.array(TypeId::S32, ArrayKind::Fixed(4));

  assert_eq!(types.layout(view), Some(Layout::new(16, 8)));
  assert_eq!(types.layout(resizable), Some(Layout::new(40, 8)));
  assert_eq!(types.layout(fixed), Some(Layout::new(16, 4)));
}

#[test]
fn structural_types_are_interned_and_nominal_ones_are_not() {
  let mut types = Types::new();
  let interner = Interner::new();
  assert_eq!(types.pointer_to(TypeId::U8), types.pointer_to(TypeId::U8));
  assert_eq!(types.pointer_to(TypeId::U8), TypeId::U8_POINTER);
  assert_eq!(
    types.array(TypeId::S32, ArrayKind::Fixed(2)),
    types.array(TypeId::S32, ArrayKind::Fixed(2))
  );
  assert_ne!(
    types.array(TypeId::S32, ArrayKind::Fixed(2)),
    types.array(TypeId::S32, ArrayKind::Fixed(3))
  );

  let name = interner.intern(b"Vector2");
  let first = build_struct(&mut types, Some(name), StructTextualFlags::empty(), &[]);
  let second = build_struct(&mut types, Some(name), StructTextualFlags::empty(), &[]);
  assert_ne!(first, second);
}

#[test]
fn members_are_padded_to_their_alignment_and_the_struct_to_its_own() {
  let mut types = Types::new();
  let interner = Interner::new();
  let (a, b, c) = (
    interner.intern(b"a"),
    interner.intern(b"b"),
    interner.intern(b"c"),
  );

  let id = build_struct(
    &mut types,
    None,
    StructTextualFlags::empty(),
    &[
      (a, TypeId::U8, None),
      (b, TypeId::S32, None),
      (c, TypeId::U8, None),
    ],
  );

  let definition = types.struct_of(id).unwrap();
  let offsets: Vec<u64> = types
    .struct_info(definition)
    .members
    .iter()
    .map(|member| member.offset)
    .collect();
  assert_eq!(offsets, vec![0, 4, 8]);
  assert_eq!(types.layout(id), Some(Layout::new(12, 4)));
}

#[test]
fn no_padding_drops_trailing_padding_and_a_union_overlays_members() {
  let mut types = Types::new();
  let interner = Interner::new();
  let (a, b, c) = (
    interner.intern(b"a"),
    interner.intern(b"b"),
    interner.intern(b"c"),
  );
  let members = [
    (a, TypeId::U8, None),
    (b, TypeId::S32, None),
    (c, TypeId::U8, None),
  ];

  // The reference keeps members at their natural offsets under `#no_padding`
  // and only stops rounding the size up (**L§8.7**).
  let padded = build_struct(&mut types, None, StructTextualFlags::empty(), &members);
  let packed = build_struct(&mut types, None, StructTextualFlags::NO_PADDING, &members);
  let packed_definition = types.struct_of(packed).unwrap();
  let offsets: Vec<u64> = types
    .struct_info(packed_definition)
    .members
    .iter()
    .map(|member| member.offset)
    .collect();
  assert_eq!(offsets, vec![0, 4, 8]);
  assert_eq!(types.layout(padded), Some(Layout::new(12, 4)));
  assert_eq!(types.layout(packed), Some(Layout::new(9, 4)));

  let union = build_struct(
    &mut types,
    None,
    StructTextualFlags::UNION,
    &[(a, TypeId::U8, None), (b, TypeId::S32, None)],
  );
  let union_definition = types.struct_of(union).unwrap();
  let offsets: Vec<u64> = types
    .struct_info(union_definition)
    .members
    .iter()
    .map(|member| member.offset)
    .collect();
  assert_eq!(offsets, vec![0, 0]);
  assert_eq!(types.layout(union), Some(Layout::new(4, 4)));
}

#[test]
fn an_explicit_alignment_moves_a_member_and_raises_the_struct() {
  let mut types = Types::new();
  let interner = Interner::new();
  let (a, b) = (interner.intern(b"a"), interner.intern(b"b"));

  let id = build_struct(
    &mut types,
    None,
    StructTextualFlags::empty(),
    &[(a, TypeId::U8, None), (b, TypeId::U8, Some(64))],
  );
  let definition = types.struct_of(id).unwrap();
  assert_eq!(types.struct_info(definition).members[1].offset, 64);
  assert_eq!(types.layout(id), Some(Layout::new(128, 64)));
}

#[test]
fn place_rewinds_the_cursor_and_the_size_is_the_furthest_extent() {
  let mut builder = LayoutBuilder::new(false, false);
  assert_eq!(builder.place(Layout::scalar(4), None), 0);
  assert_eq!(builder.place(Layout::scalar(4), None), 4);
  assert_eq!(builder.place(Layout::scalar(4), None), 8);
  builder.rewind_to(0);
  assert_eq!(builder.place(Layout::new(12, 4), None), 0);
  assert_eq!(builder.finish(), Layout::new(12, 4));
}

#[test]
fn an_enum_and_a_variant_have_the_layout_of_what_they_wrap() {
  let mut types = Types::new();
  let interner = Interner::new();
  let (_, color) = types.new_enum(EnumInfo::new(
    Some(interner.intern(b"Color")),
    TypeId::U8,
    EnumTypeFlags::empty(),
  ));
  let (_, handle) = types.new_variant(VariantInfo {
    name: Some(interner.intern(b"Handle")),
    base: TypeId::U32,
    flags: VariantFlags::DISTINCT,
  });

  assert_eq!(types.layout(color), Some(Layout::scalar(1)));
  assert_eq!(types.layout(handle), Some(Layout::scalar(4)));
  assert_eq!(types.underlying(handle), TypeId::U32);
  assert!(types.is_integer(handle));
}

#[test]
fn an_incomplete_struct_has_no_layout() {
  let mut types = Types::new();
  let (definition, id) = types.new_struct(StructInfo::new(None, StructTextualFlags::empty()));
  assert_eq!(types.layout(id), None);
  assert!(!types.is_complete(id));
  types.finish_struct(definition, 8, 8);
  assert_eq!(types.layout(id), Some(Layout::new(8, 8)));
}

#[test]
fn untyped_and_polymorphic_types_have_no_layout() {
  let mut types = Types::new();
  let interner = Interner::new();
  let (_, variable) = types.new_polymorph(PolymorphInfo {
    name: interner.intern(b"T"),
    restriction: None,
  });
  for id in [
    TypeId::UNTYPED_INT,
    TypeId::UNTYPED_ENUM,
    TypeId::UNTYPED_LITERAL,
    TypeId::OVERLOAD_SET,
    TypeId::UNKNOWN,
    variable,
  ] {
    assert_eq!(types.layout(id), None);
  }
}

#[test]
fn types_print_in_source_syntax() {
  let mut types = Types::new();
  let interner = Interner::new();

  let void_pointer_pointer = types.pointer_to(TypeId::VOID_POINTER);
  let array = types.array(void_pointer_pointer, ArrayKind::Fixed(17));
  let pointer = types.pointer_to(array);
  assert_eq!(types.name(pointer, &interner), "*[17] **void");

  let view = types.array(TypeId::STRING, ArrayKind::View);
  assert_eq!(types.name(view, &interner), "[] string");
  let resizable = types.array(TypeId::S64, ArrayKind::Resizable);
  assert_eq!(types.name(resizable, &interner), "[..] s64");

  let mut signature = ProcedureType::new(vec![TypeId::S32], vec![TypeId::BOOL, TypeId::S64]);
  signature.flags = ProcedureFlags::IS_C_CALL;
  let procedure = types.procedure(signature);
  assert_eq!(
    types.name(procedure, &interner),
    "(s32) -> (bool, s64) #c_call"
  );

  let (_, distinct) = types.new_variant(VariantInfo {
    name: None,
    base: TypeId::FLOAT32,
    flags: VariantFlags::DISTINCT,
  });
  assert_eq!(types.name(distinct, &interner), "#type,distinct float32");
}

#[test]
fn integer_ranges_decide_which_widenings_are_lossless() {
  use oj_types::IntKind;
  assert!(IntKind::S64.contains_range_of(IntKind::U32));
  assert!(IntKind::S32.contains_range_of(IntKind::U8));
  assert!(IntKind::U16.contains_range_of(IntKind::U8));
  assert!(!IntKind::U64.contains_range_of(IntKind::S8));
  assert!(!IntKind::S16.contains_range_of(IntKind::S32));
  assert!(IntKind::U8.holds(255));
  assert!(!IntKind::U8.holds(256));
  assert!(!IntKind::U8.holds(-1));
  assert!(IntKind::S8.holds(-128));
}

#[test]
fn align_forward_rounds_up_to_the_next_multiple() {
  assert_eq!(align_forward(0, 8), 0);
  assert_eq!(align_forward(1, 8), 8);
  assert_eq!(align_forward(8, 8), 8);
  assert_eq!(align_forward(9, 4), 12);
  assert_eq!(align_forward(7, 1), 7);
}

#[test]
fn a_type_kind_names_its_type_info_tag() {
  assert_eq!(TypeKind::Void.tag_name(), "VOID");
  assert_eq!(TypeKind::UntypedEnum.tag_name(), "UNTYPED_ENUM");
  assert!(TypeKind::UntypedInt.is_untyped());
  assert!(!TypeKind::Bool.is_untyped());
}

/// Sizes measured with the reference compiler (`size_of` on each shape), so
/// that the layout rules stay pinned to it rather than to our reading of them.
#[test]
fn struct_sizes_match_the_reference_compiler() {
  let mut types = Types::new();
  let interner = Interner::new();
  let name = |text: &str| interner.intern(text.as_bytes());
  let (a, b, c) = (name("a"), name("b"), name("c"));

  let empty = build_struct(&mut types, None, StructTextualFlags::empty(), &[]);
  assert_eq!(types.layout(empty), Some(Layout::new(0, 1)));

  let nested_empty = build_struct(
    &mut types,
    None,
    StructTextualFlags::empty(),
    &[
      (a, TypeId::U8, None),
      (b, empty, None),
      (c, TypeId::U8, None),
    ],
  );
  assert_eq!(types.layout(nested_empty), Some(Layout::new(2, 1)));

  // A `[0] s32` occupies nothing but still aligns to 4 (**L§3.3**).
  let zero_length = types.array(TypeId::S32, ArrayKind::Fixed(0));
  let with_zero_length = build_struct(
    &mut types,
    None,
    StructTextualFlags::empty(),
    &[
      (a, TypeId::U8, None),
      (b, zero_length, None),
      (c, TypeId::U8, None),
    ],
  );
  assert_eq!(types.layout(with_zero_length), Some(Layout::new(8, 4)));

  let with_void = build_struct(
    &mut types,
    None,
    StructTextualFlags::empty(),
    &[
      (a, TypeId::U8, None),
      (b, TypeId::VOID, None),
      (c, TypeId::U8, None),
    ],
  );
  assert_eq!(types.layout(with_void), Some(Layout::new(2, 1)));

  // `#align 4` on an 8-byte member lowers its alignment, as the Windows
  // minidump bindings rely on (**L§3.14**).
  let lowered = build_struct(
    &mut types,
    None,
    StructTextualFlags::empty(),
    &[(a, TypeId::U8, None), (b, TypeId::S64, Some(4))],
  );
  assert_eq!(types.layout(lowered), Some(Layout::new(12, 4)));
}

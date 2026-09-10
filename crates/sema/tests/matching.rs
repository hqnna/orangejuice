use oj_diag::{Severity, SourceMap};
use oj_lexer::Interner;
use oj_scope::{Options, Program};
use oj_sema::Checker;

/// Typechecks one source with no jai distribution behind it and returns the
/// errors it reported, in order.
fn errors(source: &str) -> Vec<String> {
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("main.jai");
  std::fs::write(&path, source).expect("the fixture should be writable");

  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    &path,
    Options {
      load_preload: false,
      ..Options::default()
    },
  );
  assert!(!program.has_errors(), "the fixture should resolve");
  let mut checker = Checker::new(&program);
  checker.check();
  checker
    .diagnostics()
    .iter()
    .filter(|diagnostic| diagnostic.severity == Severity::Error)
    .map(|diagnostic| diagnostic.message.clone())
    .collect()
}

fn accepts(source: &str) {
  let reported = errors(source);
  assert!(reported.is_empty(), "expected no errors, got {reported:?}");
}

#[test]
fn a_declaration_reports_the_type_it_could_not_convert() {
  assert_eq!(
    errors("f :: () { x: string = 5; }"),
    vec!["Type mismatch. Type wanted: string; type given: s64.".to_string()]
  );
  assert_eq!(
    errors("f :: () { a: s32 = 3; b: s8 = a; }"),
    vec![
      "Loss of information (trying to fit 32 bits into 8 bits). Can't do this without an \
     explicit cast. Type wanted: s8; type given: s32."
        .to_string()
    ]
  );
  assert_eq!(
    errors("f :: () { a: s64 = 3; b: u64 = a; }"),
    vec!["Number signedness mismatch. Type wanted: u64; type given: s64.".to_string()]
  );
  // A float never becomes an integer without a cast (**L§5.10**).
  assert_eq!(
    errors("f :: () { w: float = 1.0; v: s64 = w; }"),
    vec!["Type mismatch. Type wanted: s64; type given: float32.".to_string()]
  );
}

#[test]
fn a_declaration_names_itself_in_the_trailing_info() {
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("main.jai");
  std::fs::write(&path, "f :: () { x: string = 5; }").unwrap();
  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    &path,
    Options {
      load_preload: false,
      ..Options::default()
    },
  );
  let mut checker = Checker::new(&program);
  checker.check();
  let messages: Vec<String> = checker
    .diagnostics()
    .iter()
    .map(|diagnostic| diagnostic.message.clone())
    .collect();
  assert_eq!(
    messages,
    vec![
      "Type mismatch. Type wanted: string; type given: s64.".to_string(),
      "... in declaration of 'x' (declared type: string; rvalue type: s64)".to_string(),
    ]
  );
}

#[test]
fn a_literal_converts_when_it_fits_and_not_when_it_does_not() {
  accepts("f :: () { a: s32 = 5; b: u8 = 255; c: float64 = 1; d: float = 2; }");
  assert_eq!(
    errors("f :: () { a: u8 = 256; }"),
    vec!["Number signedness mismatch. Type wanted: u8; type given: s64.".to_string()]
  );
  assert_eq!(
    errors("f :: () { a: s32 = 0x0001_0203_0405_0600; }"),
    vec![
      "Loss of information (trying to fit 64 bits into 32 bits). Can't do this without an \
     explicit cast. Type wanted: s32; type given: s64."
        .to_string()
    ]
  );
  // A hexadecimal literal is a bit pattern, so it only has to fit the width.
  accepts("f :: () { b: s64 = 0xcafebabe00c0ffee; }");
}

#[test]
fn a_constant_adapts_to_the_type_that_asks_for_it() {
  accepts(
    "
    N :: 5;
    f :: () { a: u8 = N; b: s16 = N; c: float = N; }
    ",
  );
  // `size_of` is an `s64` constant, and a constant adapts (**L§5.10** rule 2).
  accepts("f :: (n: s32) { a: s32 = size_of(s64) * n; }");
  assert_eq!(
    errors("BIG :: 100000;\nf :: () { a: u8 = BIG; }"),
    vec!["Number signedness mismatch. Type wanted: u8; type given: s64.".to_string()]
  );
}

#[test]
fn integers_widen_only_when_the_whole_range_fits() {
  accepts("f :: (a: u8) { b: u16 = a; c: s16 = a; d: s64 = a; }");
  accepts("f :: (a: s32) { b: s64 = a; }");
  accepts("f :: (a: u32) { b: s64 = a; }");
  assert_eq!(
    errors("f :: (a: s8) { b: u64 = a; }"),
    vec!["Number signedness mismatch. Type wanted: u64; type given: s8.".to_string()]
  );
}

#[test]
fn arrays_convert_to_views_but_not_the_other_way() {
  accepts(
    "
    f :: (fixed: [4] s64, resizable: [..] s64) {
      a: [] s64 = fixed;
      b: [] s64 = resizable;
      c: *[] s64 = *resizable;
    }
    ",
  );
  assert_eq!(
    errors("f :: (view: [] s64) { a: [..] s64 = view; }"),
    vec!["Type mismatch. Type wanted: [..] s64; type given: [] s64.".to_string()]
  );
  assert_eq!(
    errors("f :: (view: [] s64) { a: [] s32 = view; }"),
    vec!["Type mismatch. Type wanted: [] s32; type given: [] s64.".to_string()]
  );
}

#[test]
fn pointers_convert_through_void_and_never_between_pointees() {
  accepts(
    "
    f :: (p: *s64, v: *void, a: *[4] s64) {
      x: *void = p;
      y: *s64 = v;
      z: *s64 = a;
      w: *s64 = null;
    }
    ",
  );
  assert_eq!(
    errors("f :: (p: *s64) { x: *s32 = p; }"),
    vec!["Type mismatch. Type wanted: *s32; type given: *s64.".to_string()]
  );
}

#[test]
fn an_as_member_makes_a_struct_convert_to_it() {
  accepts(
    "
    Base :: struct { x: s64; }
    Derived :: struct { #as using base: Base; y: s64; }
    Handle :: struct { #as id: u32; }
    f :: (d: Derived, h: Handle) {
      b: Base = d;
      pb: *Base = *d;
      n: u32 = h;
    }
    ",
  );
  assert_eq!(
    errors(
      "
      Base :: struct { x: s64; }
      Other :: struct { using base: Base; }
      f :: (o: Other) { b: Base = o; }
      "
    ),
    vec!["Type mismatch. Type wanted: Base; type given: Other.".to_string()]
  );
}

#[test]
fn an_isa_variant_converts_toward_its_base_and_a_distinct_one_does_not() {
  accepts(
    "
    Filename :: #type,isa string;
    f :: (name: Filename) { s: string = name; }
    ",
  );
  assert_eq!(
    errors(
      "
      Handle :: #type,distinct u32;
      f :: (h: Handle) { n: u32 = h; }
      "
    ),
    vec!["Type mismatch. Type wanted: u32; type given: Handle.".to_string()]
  );
}

#[test]
fn everything_but_an_untyped_aggregate_converts_to_any() {
  accepts(
    "
    Point :: struct { x: s64; }
    f :: (p: Point, n: s32) {
      a: Any = p;
      b: Any = n;
      c: Any = 5;
      d: Any = 1.5;
      e: Any = \"text\";
    }
    ",
  );
  assert_eq!(
    errors("f :: () { a: Any = .{}; }"),
    vec!["Type mismatch. Type wanted: Any; type given: untyped literal.".to_string()]
  );
}

#[test]
fn a_unary_dot_needs_the_enum_to_have_the_member() {
  accepts(
    "
    Fruit :: enum { BANANA; APPLE; }
    f :: () { a: Fruit = .APPLE; b: Fruit = 0; }
    ",
  );
  assert_eq!(
    errors(
      "
      Fruit :: enum { BANANA; APPLE; }
      f :: () { a: Fruit = .CHERRY; }
      "
    ),
    vec!["Type mismatch. Type wanted: Fruit; type given: enum.".to_string()]
  );
  // An enum never becomes an integer, nor an integer variable an enum
  // (**L§9**).
  assert_eq!(
    errors(
      "
      Fruit :: enum { BANANA; APPLE; }
      f :: (fruit: Fruit) { n: s64 = fruit; }
      "
    ),
    vec!["Type mismatch. Type wanted: s64; type given: Fruit.".to_string()]
  );
}

#[test]
fn a_return_is_matched_against_the_declared_return_types() {
  accepts("f :: () -> s64, bool { return 5, true; }");
  assert_eq!(
    errors("f :: () -> s64 { return \"text\"; }"),
    vec!["Type mismatch. Type wanted: s64; type given: string.".to_string()]
  );
  assert_eq!(
    errors("f :: (a: u64) -> u32 { return a; }"),
    vec![
      "Loss of information (trying to fit 64 bits into 32 bits). Can't do this without an \
     explicit cast. Type wanted: u32; type given: u64."
        .to_string()
    ]
  );
}

#[test]
fn an_assignment_is_matched_like_a_declaration() {
  accepts("f :: () { x: s64; x = 5; }");
  assert_eq!(
    errors("f :: () { x: s64; x = \"text\"; }"),
    vec!["Type mismatch. Type wanted: s64; type given: string.".to_string()]
  );
}

#[test]
fn a_condition_needs_a_truth_value() {
  accepts(
    "
    f :: (n: s64, p: *s64, s: string, view: [] s64, b: bool) {
      if n {}
      if p {}
      if s {}
      if view {}
      while b {}
    }
    ",
  );
  // A fixed array's count is a constant, so it has a truth value like any
  // other array (**L§5.9**) — measured against the reference, where a `[4]` is
  // true and a `[0]` is false, and what `Treemap`'s `if node.border` reads.
  accepts("f :: (a: [4] s64) { if a {} }");
}

#[test]
fn an_overload_set_is_narrowed_by_the_arguments() {
  accepts(
    "
    g :: (a: s64) -> s64 { return a; }
    g :: (a: string) -> string { return a; }
    f :: () {
      x: s64 = g(1);
      y: string = g(\"text\");
    }
    ",
  );
  // The `float` overload wins over widening to `float64` (**L§7.5**).
  accepts(
    "
    h :: (a: float) -> float { return a; }
    h :: (a: float64) -> float64 { return a; }
    f :: (v: float) { x: float = h(v); }
    ",
  );
  assert_eq!(
    errors(
      "
      g :: (a: s64) -> s64 { return a; }
      g :: (a: string) -> string { return a; }
      f :: () { y: string = g(1); }
      "
    ),
    vec!["Type mismatch. Type wanted: string; type given: s64.".to_string()]
  );
}

#[test]
fn a_call_that_matches_nothing_has_no_type_to_report_against() {
  // Resolution failed, so nothing downstream is judged: the reference reports
  // the call itself, which is M5's job once every callable is known.
  accepts(
    "
    g :: (a: s64, b: s64) -> s64 { return a; }
    f :: () { x: string = g(1); }
    ",
  );
}

#[test]
fn a_compound_declaration_distributes_values_and_returns() {
  accepts(
    "
    divide :: (a: s64, b: s64) -> quotient: s64, remainder: s64, ok: bool {
      return 0, 0, true;
    }
    f :: () {
      q, r, ok := divide(7, 2);
      n: s64 = q;
      m: s64 = r;
      good: bool = ok;
      x, y := 1, \"text\";
      a: s64 = x;
      b: string = y;
    }
    ",
  );
}

#[test]
fn a_bitwise_operator_keeps_an_explicitly_cast_left_operand() {
  // Measured with the reference compiler: `cast,trunc(u32) a ^ b` is a `u32`,
  // where `cast,trunc(u32) a + b` widens back to `u64` (**L§5.2**).
  accepts("f :: (a: u64, b: u64) -> u32 { return cast,trunc(u32) a ^ b; }");
  assert_eq!(
    errors("f :: (a: u64, b: u64) -> u32 { return cast,trunc(u32) a + b; }"),
    vec![
      "Loss of information (trying to fit 64 bits into 32 bits). Can't do this without an \
     explicit cast. Type wanted: u32; type given: u64."
        .to_string()
    ]
  );
}

#[test]
fn a_polymorphic_procedure_body_is_left_to_its_instantiations() {
  accepts(
    "
    square :: (x: $T) -> T { return x * x; }
    sized :: ($T: Type) -> u8 { return size_of(T) * 8; }
    ",
  );
}

#[test]
fn a_cast_takes_its_target_type() {
  accepts(
    "
    f :: (a: s64, p: *s64) {
      b: u8 = cast,trunc(u8) a;
      c: float64 = cast(float64) a;
      q: *u8 = cast(*u8) p;
      n: s64 = cast(s64) p;
      e: bool = cast(bool) a;
    }
    ",
  );
}

#[test]
fn the_casts_the_reference_rejects_are_reported_in_its_words() {
  assert_eq!(
    errors("f :: () { x := cast(void) 3; }"),
    vec!["Casting a non-zero-sized value to void is invalid.".to_string()]
  );
  assert_eq!(
    errors("f :: () { p: *s64; v := cast([] s64) p; }"),
    vec!["Cannot cast from a pointer to a non-fixed array type.".to_string()]
  );
  assert_eq!(
    errors("f :: () { x := cast,trunc,no_check(u8) 300; }"),
    vec![
      "This cast has inconsistent modifiers. (It is flagged both 'trunc' and 'no_check')."
        .to_string()
    ]
  );
  assert_eq!(
    errors("f :: () { s := \"hi\"; n := cast(s64) s; }"),
    vec!["String cannot cast to this type (the target type is s64.)".to_string()]
  );
  assert_eq!(
    errors(
      "
      A :: struct { a: s64; }
      B :: struct { b: s64; }
      f :: () { a: A; b := cast(B) a; }
      "
    ),
    vec![
      "Cannot cast from one struct to another without force modifiers. \
       Type wanted: B (8 bytes); type given: A (8 bytes)"
        .to_string()
    ]
  );
}

#[test]
fn force_and_a_conversion_the_compiler_would_make_anyway_are_allowed() {
  accepts(
    "
    A :: struct { a: s64; }
    B :: struct { b: s64; }
    Base :: struct { x: s64; }
    Derived :: struct { #as using base: Base; }
    Handle :: #type,distinct A;
    f :: (d: Derived, h: Handle) {
      a: A;
      b := cast,force(B) a;
      base := cast(Base) d;
      plain := cast(A) h;
    }
    ",
  );
}

#[test]
fn an_operator_overload_gives_a_struct_expression_its_type() {
  accepts(
    "
    Vector3 :: struct { x, y, z: float; }
    operator + :: (a: Vector3, b: Vector3) -> Vector3 { return a; }
    operator * :: (a: Vector3, s: float) -> Vector3 #symmetric { return a; }
    operator - :: (a: Vector3) -> Vector3 { return a; }
    f :: (u: Vector3, v: Vector3, k: float) {
      a: Vector3 = u + v;
      b: Vector3 = u * k;
      c: Vector3 = k * u;
      d: Vector3 = -u;
    }
    ",
  );
  assert_eq!(
    errors(
      "
      Vector3 :: struct { x, y, z: float; }
      operator + :: (a: Vector3, b: Vector3) -> Vector3 { return a; }
      f :: (u: Vector3, v: Vector3) { n: float = u + v; }
      "
    ),
    vec!["Type mismatch. Type wanted: float32; type given: Vector3.".to_string()]
  );
}

#[test]
fn an_isa_variant_gets_its_own_type_back_from_the_base_s_operator() {
  // The upcast-back rule: `pa + pb` on two `Position3` is a `Position3`, not a
  // `Vector3` (**L§3.11**).
  accepts(
    "
    Vector3 :: struct { x, y, z: float; }
    Position3 :: #type,isa Vector3;
    operator + :: (a: Vector3, b: Vector3) -> Vector3 { return a; }
    f :: (a: Position3, b: Position3) { p: Position3 = a + b; }
    ",
  );
}

#[test]
fn a_subscript_operator_types_an_element_of_a_struct() {
  accepts(
    "
    Wrapping :: struct { values: [4] s64; }
    operator [] :: (w: Wrapping, i: int) -> int { return 0; }
    Bucket :: struct { count: s64; }
    operator *[] :: (b: *Bucket, i: int) -> *s64 { return null; }
    f :: (w: Wrapping, b: *Bucket) {
      x: s64 = w[0];
      y: s64 = b[1];
    }
    ",
  );
}

#[test]
fn a_designated_struct_literal_fills_the_members_it_names() {
  accepts(
    "
    V :: struct { x, y: float; }
    Body :: struct { name: string; values: [3] s64; }
    f :: () {
      a := V.{1, 2};
      b := V.{y = 3};
      c := V.{};
      d := Body.{name = \"Ginger\", values[1] = 7};
    }
    ",
  );
  assert_eq!(
    errors("V :: struct { x, y: float; }\nf :: () { a := V.{1, 2, 3}; }"),
    vec!["Too many values provided in this struct literal.".to_string()]
  );
  assert_eq!(
    errors("V :: struct { x, y: float; }\nf :: () { a := V.{1}; }"),
    vec!["Not enough values provided in this struct literal.".to_string()]
  );
  assert_eq!(
    errors("V :: struct { x, y: float; }\nf :: () { a := V.{z = 1}; }"),
    vec!["'z' is not a member of V.".to_string()]
  );
  assert_eq!(
    errors("V :: struct { x, y: float; }\nf :: () { a := V.{x = \"hi\"}; }"),
    vec!["Type mismatch. Type wanted: float32; type given: string.".to_string()]
  );
}

#[test]
fn a_place_overlay_is_another_view_rather_than_more_slots() {
  // `Vector4.{1,2,3,4}` fills x, y, z and w; `component` and `floats` are the
  // same bytes seen again (**L§8.6**).
  accepts(
    "
    Vector4 :: struct {
      x, y, z, w: float;
      #place x;
      component: [4] float;
    }
    f :: () { v := Vector4.{1, 2, 3, 4}; }
    ",
  );
}

#[test]
fn a_designated_array_literal_checks_its_elements() {
  accepts("f :: () { a := s64.[1, 2, 3]; b := string.[\"a\", \"b\"]; }");
  assert_eq!(
    errors("f :: () { a := s64.[1, 2, \"x\"]; }"),
    vec!["Type mismatch. Type wanted: s64; type given: string.".to_string()]
  );
  // `*u8.[…]` is an array of `*u8`, not the address of an array of `u8`
  // (**L§5.8**).
  accepts("f :: (p: *u8) { a: [2] *u8 = *u8.[p, p]; }");
}

/// The wordings of **C§12**, measured against the reference: a narrowing
/// integer conversion names the widths it could not fit, a string that is not
/// a literal has no bytes for a `*u8` to point at, and anything else is a
/// plain mismatch.
#[test]
fn a_conversion_reports_the_wording_the_reference_uses() {
  assert_eq!(
    errors("main :: () { x: s8 = 300; }\n"),
    vec![
      "Loss of information (trying to fit 64 bits into 8 bits). Can't do this without an \
       explicit cast. Type wanted: s8; type given: s64."
        .to_string()
    ]
  );
  assert_eq!(
    errors("main :: () { s := \"hi\"; p: *u8 = s; }\n"),
    vec!["Dynamically-computed strings do not cast to *u8; only literals do.".to_string()]
  );
  assert_eq!(
    errors("S :: struct { x: s64; }\nmain :: () { s: S; n: s64 = s; }\n"),
    vec!["Type mismatch. Type wanted: s64; type given: S.".to_string()]
  );
}

/// A string the compiler laid the bytes of down casts to a pointer and to a
/// number; one a program computed casts to neither (**L§3.4**, **L§5.6**).
#[test]
fn only_a_string_literal_casts_to_a_pointer_or_a_number() {
  assert!(errors("main :: () { p := cast(*u8) \"hi\"; }\n").is_empty());
  assert!(errors("main :: () { n := cast(s64) \"hi\"; }\n").is_empty());
  assert_eq!(
    errors("main :: () { s := \"hi\"; n := cast(s64) s; }\n"),
    vec!["String cannot cast to this type (the target type is s64.)".to_string()]
  );
  assert_eq!(
    errors("main :: () { s := \"hi\"; p := cast(*u8) s; }\n"),
    vec![
      "string does not implicitly convert to *u8. If the *u8 is a C-style zero-terminated \
       string, you could call Basic.to_c_string to allocate a zero-terminated copy; otherwise, \
       dereference the string's data field."
        .to_string()
    ]
  );
}

use oj_diag::{Severity, SourceMap};
use oj_lexer::Interner;
use oj_scope::{Options, Program};
use oj_sema::Checker;

struct Fixture {
  directory: tempfile::TempDir,
}

impl Fixture {
  fn new() -> Self {
    Self {
      directory: tempfile::tempdir().expect("a temporary directory"),
    }
  }

  fn write(&self, name: &str, source: &str) -> &Self {
    let path = self.directory.path().join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, source).expect("the fixture should be writable");
    self
  }

  fn path(&self, name: &str) -> std::path::PathBuf {
    self.directory.path().join(name)
  }
}

/// Typechecks one source with no jai distribution behind it, and hands the
/// result to `check`.
fn check<R>(source: &str, body: impl FnOnce(&mut Checker<'_>) -> R) -> R {
  let fixture = Fixture::new();
  fixture.write("main.jai", source);
  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    &fixture.path("main.jai"),
    Options {
      load_preload: false,
      ..Options::default()
    },
  );
  assert!(
    !program.has_errors(),
    "the fixture should resolve: {:?}",
    program
      .diagnostics()
      .iter()
      .map(|diagnostic| &diagnostic.message)
      .collect::<Vec<_>>()
  );
  let mut checker = Checker::new(&program);
  checker.check();
  body(&mut checker)
}

/// The printed type of a top-level declaration, so that a test reads like the
/// source it checks.
fn type_of(checker: &mut Checker<'_>, name: &str) -> String {
  let interner = checker.interner();
  let symbol = interner.intern(name.as_bytes());
  let tree = checker.program().tree();
  let id = (0..tree.declaration_count() as u32)
    .map(oj_scope::DeclId)
    .find(|id| tree.decl(*id).name == symbol)
    .unwrap_or_else(|| panic!("'{name}' should be declared"));
  let resolved = checker.decl_type(id);
  match resolved.denoted {
    Some(denoted) => checker.type_name(denoted),
    None => checker.type_name(resolved.value),
  }
}

fn size_of(checker: &mut Checker<'_>, name: &str) -> Option<u64> {
  let interner = checker.interner();
  let symbol = interner.intern(name.as_bytes());
  let tree = checker.program().tree();
  let id = (0..tree.declaration_count() as u32)
    .map(oj_scope::DeclId)
    .find(|id| tree.decl(*id).name == symbol)
    .unwrap_or_else(|| panic!("'{name}' should be declared"));
  let denoted = checker.decl_type(id).denoted?;
  checker.types().size_of(denoted)
}

fn errors(checker: &Checker<'_>) -> Vec<String> {
  checker
    .diagnostics()
    .iter()
    .filter(|diagnostic| diagnostic.severity == Severity::Error)
    .map(|diagnostic| diagnostic.message.clone())
    .collect()
}

#[test]
fn a_declaration_takes_its_type_slot() {
  check(
    "
    a: s32;
    b: *[4] float64;
    c: [] string;
    d: [..] u8;
    e: () -> bool;
    ",
    |checker| {
      assert_eq!(type_of(checker, "a"), "s32");
      assert_eq!(type_of(checker, "b"), "*[4] float64");
      assert_eq!(type_of(checker, "c"), "[] string");
      assert_eq!(type_of(checker, "d"), "[..] u8");
      assert_eq!(type_of(checker, "e"), "() -> bool");
    },
  );
}

#[test]
fn a_variable_without_a_type_slot_hardens_its_initializer() {
  check(
    "
    a := 5;
    b := 1.5;
    c := \"hello\";
    d := true;
    e := 1 + 2 * 3;
    ",
    |checker| {
      // An integer literal defaults to `s64` and a float literal to `float32`
      // (**L§5.10**).
      assert_eq!(type_of(checker, "a"), "s64");
      assert_eq!(type_of(checker, "b"), "float32");
      assert_eq!(type_of(checker, "c"), "string");
      assert_eq!(type_of(checker, "d"), "bool");
      assert_eq!(type_of(checker, "e"), "s64");
    },
  );
}

#[test]
fn a_constant_naming_a_type_denotes_it() {
  check(
    "
    Vec :: struct { x, y: float; }
    Alias :: Vec;
    Handle :: #type,distinct u32;
    Filename :: #type,isa string;
    Pair :: #type (a: s32, b: s32) -> bool;
    Element : Type : float64;
    ",
    |checker| {
      assert_eq!(type_of(checker, "Vec"), "Vec");
      assert_eq!(type_of(checker, "Alias"), "Vec");
      assert_eq!(type_of(checker, "Handle"), "Handle");
      assert_eq!(type_of(checker, "Filename"), "Filename");
      assert_eq!(type_of(checker, "Pair"), "(s32, s32) -> bool");
      assert_eq!(type_of(checker, "Element"), "float64");
    },
  );
}

#[test]
fn struct_members_are_laid_out_in_declaration_order() {
  check(
    "
    Vector3 :: struct { x, y, z: float; }
    Node :: struct {
      value: int;
      next: *Node;
      using position: Vector3;
      #as handle: u32;
      K :: 16;
      names: [4] string;
    }
    ",
    |checker| {
      assert_eq!(size_of(checker, "Vector3"), Some(12));
      // Measured with the reference compiler.
      assert_eq!(size_of(checker, "Node"), Some(96));

      let interner = checker.interner();
      let node = interner.intern(b"Node");
      let tree = checker.program().tree();
      let id = (0..tree.declaration_count() as u32)
        .map(oj_scope::DeclId)
        .find(|id| tree.decl(*id).name == node)
        .unwrap();
      let denoted = checker.decl_type(id).denoted.unwrap();
      let definition = checker.types().struct_of(denoted).unwrap();
      let members: Vec<(String, u64)> = checker
        .types()
        .struct_info(definition)
        .members
        .iter()
        .map(|member| {
          (
            interner.resolve_lossy(member.name).into_owned(),
            member.offset,
          )
        })
        .collect();
      assert_eq!(
        members,
        vec![
          ("value".to_string(), 0),
          ("next".to_string(), 8),
          ("position".to_string(), 16),
          // `using` flattens the base's members in at their real offsets.
          ("x".to_string(), 16),
          ("y".to_string(), 20),
          ("z".to_string(), 24),
          ("handle".to_string(), 28),
          ("K".to_string(), 0),
          ("names".to_string(), 32),
        ]
      );
    },
  );
}

#[test]
fn a_union_overlays_its_members_and_no_padding_drops_the_tail() {
  check(
    "
    U :: union { i: s32; f: float; b: u8; }
    Packed :: struct { a: u8; b: s32; c: u8; } #no_padding
    Padded :: struct { a: u8; b: s32; c: u8; }
    ",
    |checker| {
      assert_eq!(size_of(checker, "U"), Some(4));
      assert_eq!(size_of(checker, "Packed"), Some(9));
      assert_eq!(size_of(checker, "Padded"), Some(12));
    },
  );
}

#[test]
fn place_overlays_later_members_on_an_earlier_one() {
  check(
    "
    Placed :: struct { a: s32; b: s32; #place a; c: s64; }
    Vector4 :: struct { x, y, z, w: float; #place x; component: [4] float; }
    ",
    |checker| {
      assert_eq!(size_of(checker, "Placed"), Some(8));
      assert_eq!(size_of(checker, "Vector4"), Some(16));
    },
  );
}

#[test]
fn a_struct_may_point_at_itself_but_not_contain_itself() {
  check(
    "
    Fine :: struct { next: *Fine; children: [] Fine; }
    ",
    |checker| {
      assert_eq!(size_of(checker, "Fine"), Some(24));
      assert!(errors(checker).is_empty());
    },
  );

  check("Bad :: struct { self: Bad; }", |checker| {
    assert_eq!(
      errors(checker),
      vec!["The program contains circular dependencies.".to_string()]
    );
  });
}

#[test]
fn a_procedure_type_may_name_a_struct_that_names_it_back() {
  check(
    "
    Thread_Proc :: #type (t: *Thread) -> s64;
    Thread :: struct { index: s64; proc: Thread_Proc; }
    ",
    |checker| {
      assert_eq!(type_of(checker, "Thread_Proc"), "(*Thread) -> s64");
      assert_eq!(size_of(checker, "Thread"), Some(16));
      assert!(errors(checker).is_empty());
    },
  );
}

#[test]
fn enum_members_count_up_and_enum_flags_double() {
  check(
    "
    Fruit :: enum { BANANA; APPLE :: 12; CHERRY; ALIAS :: APPLE; NEXT; }
    Flags :: enum_flags u16 { A; B; C :: 0x40; D; }
    Result :: enum s32 { OK :: 0; FAIL :: -1; }
    ",
    |checker| {
      let values = |checker: &mut Checker<'_>, name: &str| -> Vec<(String, i64)> {
        let interner = checker.interner();
        let symbol = interner.intern(name.as_bytes());
        let tree = checker.program().tree();
        let id = (0..tree.declaration_count() as u32)
          .map(oj_scope::DeclId)
          .find(|id| tree.decl(*id).name == symbol)
          .unwrap();
        let denoted = checker.decl_type(id).denoted.unwrap();
        let definition = checker.types().enum_of(denoted).unwrap();
        checker
          .types()
          .enum_info(definition)
          .members
          .iter()
          .map(|member| {
            (
              interner.resolve_lossy(member.name).into_owned(),
              member.value,
            )
          })
          .collect()
      };

      assert_eq!(
        values(checker, "Fruit"),
        vec![
          ("BANANA".to_string(), 0),
          ("APPLE".to_string(), 12),
          ("CHERRY".to_string(), 13),
          ("ALIAS".to_string(), 12),
          ("NEXT".to_string(), 13),
        ]
      );
      assert_eq!(
        values(checker, "Flags"),
        vec![
          ("A".to_string(), 1),
          ("B".to_string(), 2),
          ("C".to_string(), 64),
          ("D".to_string(), 128),
        ]
      );
      assert_eq!(
        values(checker, "Result"),
        vec![("OK".to_string(), 0), ("FAIL".to_string(), -1)]
      );
      assert_eq!(size_of(checker, "Flags"), Some(2));
      assert_eq!(size_of(checker, "Result"), Some(4));
    },
  );
}

#[test]
fn enum_members_may_be_written_in_terms_of_each_other() {
  check(
    "
    Values :: enum { A :: 1; B :: 2; C :: Values.A | .B; SHIFTED :: 1 << 4; }
    ",
    |checker| {
      let interner = checker.interner();
      let symbol = interner.intern(b"Values");
      let tree = checker.program().tree();
      let id = (0..tree.declaration_count() as u32)
        .map(oj_scope::DeclId)
        .find(|id| tree.decl(*id).name == symbol)
        .unwrap();
      let denoted = checker.decl_type(id).denoted.unwrap();
      let definition = checker.types().enum_of(denoted).unwrap();
      let values: Vec<i64> = checker
        .types()
        .enum_info(definition)
        .members
        .iter()
        .map(|member| member.value)
        .collect();
      assert_eq!(values, vec![1, 2, 3, 16]);
    },
  );
}

#[test]
fn an_array_dimension_is_a_constant_expression() {
  check(
    "
    N :: 4;
    Thing :: struct { numbers: [8] s32; }
    a: [N] u8;
    b: [N * 2 + 1] u8;
    c: [Thing.numbers.count] u8;
    d: [size_of(s64)] u8;
    ",
    |checker| {
      assert_eq!(type_of(checker, "a"), "[4] u8");
      assert_eq!(type_of(checker, "b"), "[9] u8");
      assert_eq!(type_of(checker, "c"), "[8] u8");
      assert_eq!(type_of(checker, "d"), "[8] u8");
    },
  );
}

#[test]
fn a_nested_type_is_reached_before_its_owner_is_laid_out() {
  // The shape `String_Builder` uses: a constant whose value needs the size of a
  // type nested in the struct whose own size needs that constant (**L§8.3**).
  check(
    "
    BUFFER_SIZE :: 4096 - size_of(Builder.Buffer);
    Builder :: struct {
      Buffer :: struct { count: s64; allocated: s64; }
      bytes: [BUFFER_SIZE] u8;
    }
    ",
    |checker| {
      assert_eq!(size_of(checker, "Builder"), Some(4080));
      assert!(errors(checker).is_empty());
    },
  );
}

#[test]
fn a_member_of_a_conditional_branch_the_scope_tree_discarded_is_not_laid_out() {
  check(
    "
    Sized :: struct {
      a: s32;
      #if false { wide: s64; } else { narrow: s32; }
    }
    ",
    |checker| assert_eq!(size_of(checker, "Sized"), Some(8)),
  );
}

#[test]
fn a_polymorphic_struct_has_no_layout_of_its_own() {
  check(
    "Holder :: struct (T: Type, N: s64) { array: [N] T; }",
    |checker| {
      let denoted = {
        let interner = checker.interner();
        let symbol = interner.intern(b"Holder");
        let tree = checker.program().tree();
        let id = (0..tree.declaration_count() as u32)
          .map(oj_scope::DeclId)
          .find(|id| tree.decl(*id).name == symbol)
          .unwrap();
        checker.decl_type(id).denoted.unwrap()
      };
      let definition = checker.types().struct_of(denoted).unwrap();
      assert!(
        checker
          .types()
          .struct_info(definition)
          .nontextual_flags
          .contains(oj_types::StructNontextualFlags::POLYMORPHIC)
      );
      assert!(checker.types().struct_info(definition).members.is_empty());
    },
  );
}

#[test]
fn an_anonymous_union_member_contributes_its_names_to_the_struct() {
  check(
    "
    V :: struct {
      tag: s32;
      union { f: float; i: s32; }
    }
    ",
    |checker| {
      assert_eq!(size_of(checker, "V"), Some(8));
      let interner = checker.interner();
      let symbol = interner.intern(b"V");
      let tree = checker.program().tree();
      let id = (0..tree.declaration_count() as u32)
        .map(oj_scope::DeclId)
        .find(|id| tree.decl(*id).name == symbol)
        .unwrap();
      let denoted = checker.decl_type(id).denoted.unwrap();
      let definition = checker.types().struct_of(denoted).unwrap();
      let members: Vec<(String, u64)> = checker
        .types()
        .struct_info(definition)
        .members
        .iter()
        .map(|member| {
          (
            interner.resolve_lossy(member.name).into_owned(),
            member.offset,
          )
        })
        .collect();
      assert_eq!(
        members,
        vec![
          ("tag".to_string(), 0),
          ("f".to_string(), 4),
          ("i".to_string(), 4),
        ]
      );
    },
  );
}

#[test]
fn a_local_is_not_in_scope_in_its_own_initializer() {
  check(
    "
    outer: s32;
    f :: () {
      {
        outer := outer;
      }
    }
    ",
    |checker| assert!(errors(checker).is_empty()),
  );
}

//! Instantiating polymorphic procedures (**L§7.8**).

use oj_diag::{Severity, SourceMap};
use oj_lexer::Interner;
use oj_scope::{DeclId, Options, Program};
use oj_sema::Checker;

fn build<R>(source: &str, body: impl FnOnce(&mut Checker<'_>) -> R) -> R {
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
  body(&mut checker)
}

/// The printed type of every declaration of that name, in declaration order —
/// a name written once inside a polymorphic body still has one declaration,
/// whatever the instantiations made of it.
fn types_named(checker: &mut Checker<'_>, name: &str) -> Vec<String> {
  let symbol = checker.interner().intern(name.as_bytes());
  let tree = checker.program().tree();
  let ids: Vec<DeclId> = (0..tree.declaration_count() as u32)
    .map(DeclId)
    .filter(|id| tree.decl(*id).name == symbol)
    .collect();
  assert!(!ids.is_empty(), "'{name}' should be declared");
  ids
    .into_iter()
    .map(|id| {
      let resolved = checker.decl_type(id);
      match resolved.denoted {
        Some(denoted) => checker.type_name(denoted),
        None => checker.type_name(resolved.value),
      }
    })
    .collect()
}

fn type_of(checker: &mut Checker<'_>, name: &str) -> String {
  types_named(checker, name).remove(0)
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
fn a_type_variable_takes_the_argument_type() {
  build(
    "
    square :: (x: $T) -> T { return x * x; }
    main :: () {
      a := square(3);
      b := square(2.5);
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      assert_eq!(type_of(checker, "a"), "s64");
      assert_eq!(type_of(checker, "b"), "float32");
    },
  );
}

#[test]
fn a_type_variable_is_solved_through_pointers_and_arrays() {
  build(
    "
    first :: (values: [] $T) -> T { return values[0]; }
    deref :: (p: *$T) -> T { return <<p; }
    main :: () {
      xs: [4] s32;
      a := first(xs);
      y: float64;
      b := deref(*y);
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      assert_eq!(type_of(checker, "a"), "s32");
      assert_eq!(type_of(checker, "b"), "float64");
    },
  );
}

#[test]
fn a_baked_value_parameter_is_a_constant_of_the_instantiation() {
  build(
    "
    Holder :: struct { value: s16; }
    make :: ($T: Type) -> T { result: T; return result; }
    main :: () {
      a := make(Holder);
      b := make(u8);
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      assert_eq!(type_of(checker, "a"), "Holder");
      assert_eq!(type_of(checker, "b"), "u8");
    },
  );
}

#[test]
fn a_polymorphic_candidate_loses_to_an_exact_overload() {
  build(
    "
    f :: (x: $T) -> s32 { return 1; }
    f :: (x: float32) -> s64 { return 2; }
    main :: () {
      a := f(1.5);
      b := f(\"text\");
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      assert_eq!(type_of(checker, "a"), "s64");
      assert_eq!(type_of(checker, "b"), "s32");
    },
  );
}

#[test]
fn a_type_variable_used_twice_has_to_agree() {
  build(
    "
    pair :: (a: $T, b: T) -> T { return a; }
    main :: () {
      ok := pair(1, 2);
      bad := pair(1, \"text\");
    }
    ",
    |checker| {
      assert_eq!(type_of(checker, "ok"), "s64");
      // Nothing satisfies the header, so the call has no type of its own; the
      // reference reports it, which is M7's error wording rather than a
      // silently wrong answer.
      assert_eq!(type_of(checker, "bad"), "unknown");
    },
  );
}

#[test]
fn identical_constants_share_one_instantiation() {
  build(
    "
    square :: (x: $T) -> T { return x * x; }
    main :: () {
      a := square(3);
      b := square(4);
      c := square(1.5);
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      assert_eq!(checker.instance_count(), 2);
    },
  );
}

#[test]
fn an_instantiated_body_is_typechecked_against_its_own_types() {
  build(
    "
    always_text :: (x: $T) -> T { return \"text\"; }
    main :: () {
      a := always_text(3);
    }
    ",
    |checker| {
      // The body's `return` only matches for one `T`, and the instantiation is
      // what finds that out (**L§7.8**).
      assert_eq!(
        errors(checker),
        vec!["Type mismatch. Type wanted: s64; type given: string.".to_string()]
      );
    },
  );
}

#[test]
fn a_using_of_a_value_makes_its_members_names() {
  build(
    "
    Player :: struct { health: s32; name: string; }
    heal :: (using p: *Player, amount: s32) -> s32 {
      health += amount;
      return health;
    }
    local :: () -> string {
      p: Player;
      using p;
      return name;
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      assert_eq!(type_of(checker, "heal"), "(*Player, s32) -> s32");
      assert_eq!(type_of(checker, "local"), "() -> string");
    },
  );
}

#[test]
fn a_using_of_an_enum_type_makes_its_members_names() {
  build(
    "
    Tag :: enum u32 { NONE; STRING; INTEGER; }
    which :: (t: Tag) -> bool {
      using Tag;
      return t == STRING;
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      assert_eq!(type_of(checker, "which"), "(Tag) -> bool");
    },
  );
}

#[test]
fn a_using_of_a_value_reaches_the_constants_of_its_struct() {
  build(
    "
    Builder :: struct {
      Buffer :: struct { count: s64; }
      bytes: [64] u8;
    }
    room :: (using b: *Builder) -> s64 {
      return bytes.count - size_of(Buffer);
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      assert_eq!(type_of(checker, "room"), "(*Builder) -> s64");
    },
  );
}

#[test]
fn a_polymorphic_struct_is_baked_per_argument_set() {
  build(
    "
    Holder :: struct ($T: Type, $N: s64) { array: [N] T; }
    main :: () {
      floats: Holder(float32, 5);
      ints: Holder(s64, 3);
      named: Holder(N = 5, T = float32);
      same := floats;
    }
    ",
    |checker| {
      assert!(errors(checker).is_empty(), "{:?}", errors(checker));
      // The reference names each argument: `Holder(T=float32, N=5)`.
      assert_eq!(type_of(checker, "floats"), "Holder(T=float32, N=5)");
      assert_eq!(type_of(checker, "ints"), "Holder(T=s64, N=3)");
      // The same arguments in another order are the same type (**L§8.5**).
      assert_eq!(type_of(checker, "named"), "Holder(T=float32, N=5)");
      assert_eq!(type_of(checker, "same"), "Holder(T=float32, N=5)");
    },
  );
}

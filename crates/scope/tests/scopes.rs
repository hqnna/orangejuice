use std::path::Path;

use oj_diag::{Severity, SourceMap};
use oj_lexer::Interner;
use oj_scope::{Options, Program, Resolution, ScopeKind};

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

/// Resolves `root` inside `fixture`, without a jai distribution: the fixture
/// supplies whatever modules it imports through its own `modules/` directory.
fn resolve<R>(
  fixture: &Fixture,
  root: &str,
  check: impl FnOnce(&Program<'_>, &Interner) -> R,
) -> R {
  let sources = SourceMap::new();
  let interner = Interner::new();
  let options = Options {
    load_preload: false,
    ..Options::default()
  };
  let program = Program::build(&sources, &interner, &fixture.path(root), options);
  check(&program, &interner)
}

fn errors(program: &Program<'_>) -> Vec<String> {
  program
    .diagnostics()
    .iter()
    .filter(|diagnostic| diagnostic.severity == Severity::Error)
    .map(|diagnostic| diagnostic.message.clone())
    .collect()
}

fn undeclared(program: &Program<'_>) -> Vec<String> {
  oj_scope::undeclared_identifiers(program)
    .into_iter()
    .map(|entry| entry.name)
    .collect()
}

#[test]
fn a_load_becomes_a_sibling_file_scope_under_the_same_module() {
  let fixture = Fixture::new();
  fixture
    .write(
      "main.jai",
      "#load \"other.jai\";\nmain :: () { helper(); }\n",
    )
    .write("other.jai", "helper :: () {}\n");

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), Vec::<String>::new());
    assert_eq!(undeclared(program), Vec::<String>::new());
    assert_eq!(program.unit_count(), 2);

    let tree = program.tree();
    let files: Vec<_> = tree
      .scope_ids()
      .filter(|id| tree.scope_kind(*id) == ScopeKind::File)
      .collect();
    assert_eq!(files.len(), 2);
    for file in files {
      assert_eq!(tree.parent(file), Some(program.main_scope()));
    }
  });
}

#[test]
fn scope_file_keeps_a_declaration_out_of_the_other_files_of_the_module() {
  let fixture = Fixture::new();
  fixture
    .write(
      "main.jai",
      "#load \"other.jai\";\nmain :: () { secret(); }\n",
    )
    .write("other.jai", "#scope_file\nsecret :: () {}\n");

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), ["secret"]);
  });
}

#[test]
fn scope_module_shares_a_declaration_but_does_not_export_it() {
  let fixture = Fixture::new();
  fixture
    .write(
      "modules/Widget/module.jai",
      "#scope_module\ninternal :: () {}\n#scope_export\npublic :: () { internal(); }\n",
    )
    .write(
      "main.jai",
      "#import \"Widget\";\nmain :: () { public(); }\n",
    );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });

  let fixture = Fixture::new();
  fixture
    .write(
      "modules/Widget/module.jai",
      "#scope_module\ninternal :: () {}\n",
    )
    .write(
      "main.jai",
      "#import \"Widget\";\nmain :: () { internal(); }\n",
    );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), ["internal"]);
  });
}

#[test]
fn an_unnamed_import_is_visible_to_every_file_of_the_module_even_under_scope_file() {
  // Verified against the reference compiler: `#scope_file` narrows the
  // declarations a file makes, not the names an unnamed `#import` brings in
  // (**L§11.2**).
  let fixture = Fixture::new();
  fixture
    .write("modules/Widget.jai", "widget :: () {}\n")
    .write(
      "main.jai",
      "#load \"other.jai\";\nmain :: () { widget(); }\n",
    )
    .write("other.jai", "#scope_file\n#import \"Widget\";\n");

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn a_named_import_binds_a_name_without_widening_the_scope() {
  let fixture = Fixture::new();
  fixture
    .write("modules/Widget.jai", "widget :: () {}\n")
    .write(
      "main.jai",
      "W :: #import \"Widget\";\nmain :: () { widget(); }\n",
    );

  resolve(&fixture, "main.jai", |program, interner| {
    assert_eq!(undeclared(program), ["widget"]);

    let name = interner.intern(b"W");
    let Resolution::Found(found) = program.tree().lookup(program.main_scope(), name) else {
      panic!("the named import should be declared");
    };
    assert!(matches!(
      program.tree().decl(found[0]).kind,
      oj_scope::DeclKind::Module(_)
    ));
  });
}

#[test]
fn one_module_is_instantiated_once_per_distinct_parameter_list() {
  let fixture = Fixture::new();
  fixture
    .write(
      "modules/Widget.jai",
      "#module_parameters (VERBOSE := false);\nwidget :: () {}\n",
    )
    .write(
      "main.jai",
      "A :: #import \"Widget\";\nB :: #import \"Widget\";\nC :: #import \"Widget\"(VERBOSE=true);\n",
    );

  resolve(&fixture, "main.jai", |program, interner| {
    let tree = program.tree();
    let module_of = |name: &str| {
      let symbol = interner.intern(name.as_bytes());
      let Resolution::Found(found) = tree.lookup(program.main_scope(), symbol) else {
        panic!("{name} should be declared");
      };
      match tree.decl(found[0]).kind {
        oj_scope::DeclKind::Module(scope) => scope,
        other => panic!("expected a module binding, got {other:?}"),
      }
    };

    assert_eq!(module_of("A"), module_of("B"));
    assert_ne!(module_of("A"), module_of("C"));
  });
}

#[test]
fn a_static_if_on_os_takes_one_branch_and_declares_into_the_enclosing_scope() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "#if OS == .LINUX {\n  PLATFORM :: \"linux\";\n} else {\n  PLATFORM :: \"other\";\n}\nmain :: () { use(PLATFORM); }\nuse :: (s: string) {}\n",
  );

  resolve(&fixture, "main.jai", |program, interner| {
    assert_eq!(errors(program), Vec::<String>::new());
    let name = interner.intern(b"PLATFORM");
    let Resolution::Found(found) = program.tree().lookup(program.main_scope(), name) else {
      panic!("the taken branch should have declared PLATFORM");
    };
    assert_eq!(found.len(), 1);
  });
}

#[test]
fn a_static_switch_takes_the_matching_case() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "#if OS == {\n  case .WINDOWS; A :: 1;\n  case .LINUX;   B :: 2;\n  case;          C :: 3;\n}\n",
  );

  resolve(&fixture, "main.jai", |program, interner| {
    let tree = program.tree();
    let declared = |name: &str| {
      matches!(
        tree.lookup(program.main_scope(), interner.intern(name.as_bytes())),
        Resolution::Found(_)
      )
    };
    assert!(declared("B"));
    assert!(!declared("A"));
    assert!(!declared("C"));
  });
}

#[test]
fn a_static_if_is_retried_until_the_constant_it_reads_is_known() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "#if LATER {\n  CHOSEN :: 1;\n} else {\n  CHOSEN :: 2;\n}\nLATER :: true;\n",
  );

  resolve(&fixture, "main.jai", |program, interner| {
    let name = interner.intern(b"CHOSEN");
    let Resolution::Found(found) = program.tree().lookup(program.main_scope(), name) else {
      panic!("CHOSEN should be declared");
    };
    assert_eq!(found.len(), 1);
    assert!(!program.tree().decl(found[0]).conditional);
  });
}

#[test]
fn an_undecidable_static_if_admits_every_branch_without_a_redeclaration_error() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "#if #run compute() {\n  BOTH :: 1;\n} else {\n  BOTH :: 2;\n}\n",
  );

  resolve(&fixture, "main.jai", |program, interner| {
    assert_eq!(errors(program), Vec::<String>::new());
    let name = interner.intern(b"BOTH");
    let Resolution::Found(found) = program.tree().lookup(program.main_scope(), name) else {
      panic!("both branches should have declared BOTH");
    };
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|id| program.tree().decl(*id).conditional));
  });
}

#[test]
fn redeclaring_a_name_in_one_scope_is_an_error_that_points_at_the_first_one() {
  let fixture = Fixture::new();
  fixture.write("main.jai", "x := 1;\nx := 2;\n");

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), ["Redeclared identifier 'x'."]);
    assert!(
      program
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message == "Here is the previous declaration.")
    );
  });
}

#[test]
fn procedures_overload_and_so_does_a_constant_that_may_alias_one() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "f :: (x: int) {}\nf :: (x: float) {}\nf :: g;\ng :: (x: string) {}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), Vec::<String>::new());
  });
}

#[test]
fn a_missing_module_is_reported_at_the_import() {
  let fixture = Fixture::new();
  fixture.write("main.jai", "#import \"Nowhere\";\n");

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), ["Could not find module 'Nowhere'."]);
  });
}

#[test]
fn a_using_of_an_enum_declaration_widens_the_scope_around_it() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "using Color :: enum { RED; GREEN; }\nmain :: () { pick(RED); }\npick :: (c: Color) {}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn a_using_of_a_value_makes_the_scope_wait_rather_than_fail() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "Player :: struct { health: int; }\nmain :: (using p: Player) { hurt(health); }\nhurt :: (n: int) {}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn a_for_loop_declares_its_iterators_in_the_loop_scope() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "main :: () {\n  for 0..3 use(it, it_index);\n  for value, index: 0..3 use(value, index);\n}\nuse :: (a: int, b: int) {}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn an_anonymous_union_declares_its_members_in_the_scope_around_it() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "main :: () {\n  union {\n    f: float64;\n    u: u64;\n  }\n  f = 1.0;\n  use(u);\n}\nuse :: (n: u64) {}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn a_compound_declaration_declares_only_the_names_that_are_not_marked_as_assignments() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "main :: () {\n  ok: bool;\n  result, ok= := split();\n  use(result, ok);\n}\nsplit :: () -> int, bool { return 1, true; }\nuse :: (a: int, b: bool) {}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), Vec::<String>::new());
  });
}

#[test]
fn a_placeholder_resolves_and_never_collides() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "#placeholder LATER;\nmain :: () { use(LATER); }\nuse :: (n: int) {}\nLATER :: 3;\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), Vec::<String>::new());
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn an_undeclared_identifier_is_reported_once_with_its_near_misses() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "print_line :: () {}\nmain :: () { print_lin(); }\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    let batch = oj_scope::undeclared_identifiers(program);
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].name, "print_lin");
    assert_eq!(batch[0].suggestions, ["print_line"]);

    let rendered = oj_scope::undeclared_diagnostics(&batch);
    assert_eq!(rendered[0].message, "Undeclared identifier 'print_lin'.");
    assert_eq!(rendered[1].message, "Did you mean: print_line?");
  });
}

#[test]
fn undeclared_identifiers_come_back_batched_and_sorted_by_position() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "main :: () {\n  zebra();\n  aardvark();\n  zebra();\n}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    let names: Vec<_> = oj_scope::undeclared_identifiers(program)
      .into_iter()
      .map(|entry| entry.name)
      .collect();
    assert_eq!(names, ["zebra", "aardvark", "zebra"]);
  });
}

#[test]
fn loading_one_file_twice_into_a_scope_is_reported_rather_than_looping() {
  let fixture = Fixture::new();
  fixture
    .write("main.jai", "#load \"other.jai\";\n#load \"other.jai\";\n")
    .write("other.jai", "x :: 1;\n");

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program).len(), 1);
    assert!(errors(program)[0].contains("loaded twice"));
    assert_eq!(program.unit_count(), 2);
  });
}

#[test]
fn a_file_resolved_on_its_own_still_builds_a_scope_tree() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "#import \"Nowhere\";\n#load \"missing.jai\";\nmain :: () {}\n",
  );

  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    &fixture.path("main.jai"),
    Options::single_file(),
  );

  assert_eq!(errors(&program), Vec::<String>::new());
  assert_eq!(program.unit_count(), 1);
  assert!(program.tree().scope_count() > 2);
}

#[test]
fn a_module_name_that_would_escape_the_search_path_is_not_found() {
  let fixture = Fixture::new();
  fixture.write("main.jai", "#import \"../secrets\";\n");

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), ["Could not find module '../secrets'."]);
  });
}

#[test]
fn the_dump_names_every_scope_it_built() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "Point :: struct { x: float; }\nmain :: () { p: Point; }\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    let dump = oj_scope::print_scopes(program);
    assert!(dump.contains("struct members"));
    assert!(dump.contains("procedure arguments"));
    assert!(dump.contains("struct Point"));
    assert_eq!(
      oj_scope::summary(program.tree(), program.unit_count()),
      format!(
        "1 file, {} scopes, {} declarations",
        program.tree().scope_count(),
        program.tree().declaration_count()
      )
    );
  });
}

#[test]
fn the_root_of_the_tree_is_preload_and_the_program_hangs_off_it() {
  let fixture = Fixture::new();
  fixture.write("main.jai", "main :: () {}\n");

  resolve(&fixture, "main.jai", |program, _| {
    let tree = program.tree();
    assert_eq!(tree.scope_kind(program.preload_scope()), ScopeKind::Preload);
    assert_eq!(tree.parent(program.preload_scope()), None);
    assert_eq!(
      tree.parent(program.main_scope()),
      Some(program.preload_scope())
    );
    assert_eq!(
      tree.enclosing_module(program.main_scope()),
      Some(program.main_scope())
    );
  });
}

#[test]
fn the_basic_type_names_are_declared_before_any_source_is_read() {
  let fixture = Fixture::new();
  fixture.write("main.jai", "main :: () { x: int; y: float64; }\n");

  resolve(&fixture, "main.jai", |program, interner| {
    for name in [
      "int", "float64", "string", "bool", "Any", "Code", "Type", "OS",
    ] {
      assert!(
        matches!(
          program
            .tree()
            .lookup(program.main_scope(), interner.intern(name.as_bytes())),
          Resolution::Found(_)
        ),
        "{name} should be declared"
      );
    }
  });
}

#[test]
fn a_missing_root_file_is_reported_and_leaves_no_units() {
  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    Path::new("no/such/file.jai"),
    Options::single_file(),
  );

  assert!(program.has_errors());
  assert!(program.unit_count() == 0);
}

#[test]
fn a_using_of_an_enum_reaches_importers_but_a_plain_import_does_not() {
  // `using E :: enum { … }` makes the members part of the module's scope, so an
  // importer sees them; what the module itself imported stops there
  // (**L§11.2**).
  let fixture = Fixture::new();
  fixture
    .write("modules/Inner.jai", "inner :: () {}\n")
    .write(
      "modules/Outer.jai",
      "#import \"Inner\";\nusing Flags :: enum { RED; GREEN; }\n",
    )
    .write(
      "main.jai",
      "#import \"Outer\";\nmain :: () { pick(RED); }\npick :: (f: Flags) {}\n",
    );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });

  let fixture = Fixture::new();
  fixture
    .write("modules/Inner.jai", "inner :: () {}\n")
    .write("modules/Outer.jai", "#import \"Inner\";\n")
    .write("main.jai", "#import \"Outer\";\nmain :: () { inner(); }\n");

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), ["inner"]);
  });
}

#[test]
fn an_asm_block_lets_the_scope_around_it_gain_register_names() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "main :: () {\n  #asm {\n    mov a:, 12;\n  }\n  use(a);\n}\nuse :: (n: int) {}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn a_static_if_inside_an_enum_body_still_declares_members() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "EXTRA :: true;\nSpells :: enum {\n  REZROV;\n  #if EXTRA {\n    BLORPLE;\n  }\n}\nmain :: () { cast(int) Spells.BLORPLE; }\n",
  );

  resolve(&fixture, "main.jai", |program, interner| {
    assert_eq!(errors(program), Vec::<String>::new());
    let tree = program.tree();
    let blorple = interner.intern(b"BLORPLE");
    let enums: Vec<_> = tree
      .scope_ids()
      .filter(|id| tree.scope_kind(*id) == ScopeKind::Enum)
      .collect();
    assert_eq!(enums.len(), 1);
    assert!(tree.names_in(enums[0], blorple).is_some());
  });
}

#[test]
fn a_context_argument_and_a_struct_literal_field_name_members_not_locals() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "Body :: struct { name: string; values: [4] int; }\nmain :: () {\n  b := Body.{name = \"x\", values[1] = 7};\n  make(1,, allocator = temp);\n}\nmake :: (n: int) {}\ntemp: int;\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn a_procedure_may_name_a_return_value_after_a_parameter() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "copy :: (count: int, s: *u8) -> (n: int, s: *u8) { n = count; return n, s; }\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), Vec::<String>::new());
  });
}

#[test]
fn a_name_a_macro_backticks_into_its_caller_is_a_wait_rather_than_an_error() {
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "walk :: (body: Code) #expand {\n  `index := 0;\n  #insert body;\n}\nmain :: () {\n  walk(#code { use(index); });\n}\nuse :: (n: int) {}\n",
  );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(errors(program), Vec::<String>::new());
    assert_eq!(undeclared(program), Vec::<String>::new());
  });
}

#[test]
fn a_module_parameter_is_a_constant_of_the_module_that_importers_do_not_see() {
  let fixture = Fixture::new();
  fixture
    .write(
      "modules/Widget.jai",
      "#module_parameters (VERBOSE := false);\nreport :: () { if VERBOSE {} }\n",
    )
    .write(
      "main.jai",
      "#import \"Widget\";\nmain :: () { report(); }\n",
    );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), Vec::<String>::new());
  });

  let fixture = Fixture::new();
  fixture
    .write(
      "modules/Widget.jai",
      "#module_parameters (VERBOSE := false);\n",
    )
    .write(
      "main.jai",
      "#import \"Widget\";\nmain :: () { if VERBOSE {} }\n",
    );

  resolve(&fixture, "main.jai", |program, _| {
    assert_eq!(undeclared(program), ["VERBOSE"]);
  });
}

#[test]
fn every_diagnostic_names_a_source_the_map_can_render() {
  // A root file that cannot be read has no `#load` to point at, and the
  // diagnostic still has to render.
  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    Path::new("no/such/file.jai"),
    Options::single_file(),
  );

  assert!(program.has_errors());
  for diagnostic in program.diagnostics().iter() {
    let file = sources.file(diagnostic.source);
    assert_eq!(file.path(), Path::new("no/such/file.jai"));
    assert!(oj_diag::render(diagnostic, &file).contains("Could not read"));
  }
}

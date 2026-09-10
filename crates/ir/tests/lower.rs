//! **M5**: what a body lowers to.
//!
//! Every case builds a whole program — Preload included, because `#Context` is
//! part of every Jai calling convention — and reads the listing back, so the
//! test says what the IR is rather than how it was built.

use std::path::Path;

use oj_diag::SourceMap;
use oj_lexer::Interner;
use oj_testsupport::jai_dir_or_skip;

struct Lowered {
  listing: String,
  errors: Vec<String>,
  warnings: Vec<String>,
}

fn lower(source: &str) -> Option<Lowered> {
  let jai_dir = match oj_testsupport::jai_dir() {
    Some(dir) => dir,
    None => {
      eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
      return None;
    }
  };
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("input.jai");
  std::fs::write(&path, source).expect("the input should be writable");
  Some(lower_file(&path, &jai_dir))
}

fn lower_file(path: &Path, jai_dir: &Path) -> Lowered {
  let sources = SourceMap::new();
  let interner = Interner::new();
  let options = oj_scope::Options {
    jai_dir: Some(jai_dir.to_path_buf()),
    ..oj_scope::Options::default()
  };
  let program = oj_scope::Program::build(&sources, &interner, path, options);
  let mut checker = oj_sema::Checker::new(&program);
  checker.check();
  let lowered = oj_ir::lower(&mut checker);
  Lowered {
    listing: oj_ir::print_ir(&lowered.program, &interner, None),
    errors: lowered
      .diagnostics
      .iter()
      .filter(|diagnostic| diagnostic.is_error())
      .map(|diagnostic| diagnostic.message.clone())
      .collect(),
    warnings: lowered
      .diagnostics
      .iter()
      .filter(|diagnostic| !diagnostic.is_error())
      .map(|diagnostic| diagnostic.message.clone())
      .collect(),
  }
}

fn listing_of(source: &str) -> Option<String> {
  let lowered = lower(source)?;
  assert!(
    lowered.errors.is_empty(),
    "the program should lower cleanly, but: {:#?}",
    lowered.errors
  );
  Some(lowered.listing)
}

#[test]
fn a_program_without_a_main_is_reported() {
  let Some(lowered) = lower("x := 1;\n") else {
    return;
  };
  assert_eq!(
    lowered.errors,
    ["No program entry point was found. (The designated entry point name is 'main'.)"]
  );
}

#[test]
fn only_what_the_entry_point_reaches_is_lowered() {
  let Some(listing) = listing_of("used :: () {}\nunused :: () {}\nmain :: () { used(); }\n") else {
    return;
  };
  assert!(listing.contains("procedure used"), "{listing}");
  assert!(!listing.contains("procedure unused"), "{listing}");
}

#[test]
fn a_jai_procedure_carries_a_hidden_context_pointer_and_a_foreign_one_does_not() {
  let Some(listing) = listing_of(
    "libc :: #library,system \"libc\";\n\
     puts :: (text: *u8) -> s32 #foreign libc;\n\
     helper :: (n: int) -> int { return n; }\n\
     main :: () { helper(1); }\n",
  ) else {
    return;
  };
  assert!(
    listing.contains("procedure helper (%0: s64, %1: context *Context) -> s64"),
    "{listing}"
  );
  assert!(
    listing.contains("procedure __program_main (%0: context *Context)"),
    "{listing}"
  );
}

#[test]
fn a_second_return_value_travels_through_storage_the_caller_supplies() {
  let Some(listing) =
    listing_of("two :: () -> int, int { return 1, 2; }\nmain :: () { a, b := two(); }\n")
  else {
    return;
  };
  assert!(
    listing.contains("procedure two (%0: return *s64, %1: context *Context) -> s64"),
    "{listing}"
  );
}

#[test]
fn an_aggregate_return_is_written_through_a_pointer_rather_than_returned() {
  let Some(listing) = listing_of(
    "Point :: struct { x: int; y: int; }\n\
     origin :: () -> Point { p: Point; return p; }\n\
     main :: () { q := origin(); }\n",
  ) else {
    return;
  };
  assert!(
    listing.contains("procedure origin (%0: return *Point, %1: context *Context) {"),
    "{listing}"
  );
}

#[test]
fn a_struct_member_is_a_byte_offset() {
  let Some(listing) = listing_of(
    "Point :: struct { x: int; y: int; }\n\
     main :: () { p: Point; p.y = 3; }\n",
  ) else {
    return;
  };
  assert!(listing.contains("offset %"), "{listing}");
  assert!(listing.contains("+ 8"), "{listing}");
}

#[test]
fn a_subscript_scales_by_the_element_size() {
  let Some(listing) = listing_of("main :: () { a: [4] int; a[2] = 1; }\n") else {
    return;
  };
  assert!(listing.contains("* 8"), "{listing}");
}

#[test]
fn a_declaration_without_a_value_is_cleared() {
  let Some(listing) = listing_of("main :: () { n: int; }\n") else {
    return;
  };
  assert!(listing.contains("clear %"), "{listing}");
}

#[test]
fn a_struct_member_default_is_applied_after_the_clear() {
  let Some(listing) =
    listing_of("Config :: struct { limit: int = 7; }\nmain :: () { c: Config; }\n")
  else {
    return;
  };
  let clear = listing
    .find("clear %")
    .expect("the storage is zeroed first");
  let seven = listing.find("= 7").expect("then the default is written");
  assert!(clear < seven, "{listing}");
}

#[test]
fn a_global_whose_value_folds_becomes_data() {
  let Some(listing) = listing_of("counter := 12;\nmain :: () { counter = 1; }\n") else {
    return;
  };
  assert!(listing.contains("global counter : s64 = 12"), "{listing}");
  assert!(!listing.contains("__oj_global_init"), "{listing}");
}

#[test]
fn a_global_whose_value_does_not_fold_is_assigned_before_main() {
  let Some(listing) =
    listing_of("seed :: () -> int { return 4; }\nstart := seed();\nmain :: () { start = 0; }\n")
  else {
    return;
  };
  assert!(listing.contains("global start : s64 = zero"), "{listing}");
  assert!(listing.contains("procedure __oj_global_init"), "{listing}");
}

#[test]
fn the_entry_point_keeps_the_symbol_the_c_runtime_calls_free() {
  let Some(listing) = listing_of("main :: () {}\n") else {
    return;
  };
  assert!(listing.contains("procedure __program_main"), "{listing}");
}

#[test]
fn a_defer_runs_at_the_end_of_its_block_and_before_a_return() {
  let Some(listing) = listing_of(
    "libc :: #library,system \"libc\";\n\
     mark :: (n: s32) -> s32 #foreign libc;\n\
     main :: () { n := 0; defer mark(1); if n == 0 return; }\n",
  ) else {
    return;
  };
  assert_eq!(listing.matches("call mark").count(), 2, "{listing}");
}

#[test]
fn a_range_loop_counts_it_and_it_index() {
  let Some(listing) = listing_of("main :: () { total := 0; for i: 1..3  total += i; }\n") else {
    return;
  };
  assert!(listing.contains("local $"), "{listing}");
  assert!(listing.contains("it "), "{listing}");
  assert!(listing.contains("it_index "), "{listing}");
}

#[test]
fn a_construct_a_later_milestone_owns_names_that_milestone() {
  let Some(lowered) = lower("main :: () { #asm { frobnicate a:, 1; } }\n") else {
    return;
  };
  assert_eq!(
    lowered.errors,
    ["orangejuice cannot assemble the '#asm' instruction 'frobnicate' yet (milestone M10)."]
  );
}

#[test]
fn every_how_to_program_reports_only_milestones_it_does_not_own() {
  let jai_dir = jai_dir_or_skip!();
  let how_to = jai_dir.join("how_to");
  let mut entries: Vec<_> = std::fs::read_dir(&how_to)
    .expect("the how_to directory should be readable")
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "jai"))
    .collect();
  entries.sort();

  // Lowering a whole how_to program is not expected to succeed yet — most of
  // them reach `print`, which is a polymorphic call (M7). What must hold is
  // that every complaint either names a milestone or is one of the deviations
  // `docs/spec.md` §10 already documents, rather than crashing or hanging.
  let documented = [
    "A reverse 'for' over a range does not iterate backwards",
    "No program entry point was found",
    "'context' is not available here",
  ];
  for path in entries {
    let lowered = lower_file(&path, &jai_dir);
    for error in &lowered.errors {
      assert!(
        error.contains("milestone") || documented.iter().any(|known| error.contains(known)),
        "{}: {error}",
        path.display()
      );
    }
  }
}

#[test]
fn a_body_that_can_reach_its_end_without_returning_is_a_warning() {
  let _ = jai_dir_or_skip!();
  let warnings = |source: &str| -> Vec<String> {
    lower(source).map_or_else(Vec::new, |lowered| {
      lowered
        .warnings
        .iter()
        .filter(|message| message.contains("control paths"))
        .cloned()
        .collect()
    })
  };
  let expected = vec![String::from("Not all control paths return a value.")];
  assert_eq!(
    warnings("f :: () -> int { }\nmain :: () { f(); }\n"),
    expected
  );
  assert_eq!(
    warnings("f :: (x: bool) -> int { if x return 1; }\nmain :: () { f(true); }\n"),
    expected
  );
  // Both branches return, so the join the lowering left behind is unreachable.
  assert!(
    warnings("f :: (x: bool) -> int { if x return 1; else return 2; }\nmain :: () { f(true); }\n")
      .is_empty()
  );
  assert!(warnings("f :: () { }\nmain :: () { f(); }\n").is_empty());
}

#[test]
fn a_procedure_nothing_calls_is_lowered_only_when_asked_for() {
  let jai_dir = jai_dir_or_skip!();
  const SOURCE: &str = "unreachable_one :: () { }\nmain :: () { }\n";
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("input.jai");
  std::fs::write(&path, SOURCE).expect("the input should be writable");

  let listing = |live: &[(String, String)]| -> String {
    let sources = SourceMap::new();
    let interner = Interner::new();
    let options = oj_scope::Options {
      jai_dir: Some(jai_dir.clone()),
      ..oj_scope::Options::default()
    };
    let program = oj_scope::Program::build(&sources, &interner, &path, options);
    let mut checker = oj_sema::Checker::new(&program);
    checker.check();
    let lowered = oj_ir::lower_with_roots(&mut checker, live, oj_ir::LowerOptions::default());
    assert!(
      !lowered.has_errors(),
      "the program should lower cleanly: {:#?}",
      lowered.diagnostics
    );
    oj_ir::print_ir(&lowered.program, &interner, None)
  };

  // IR generation follows calls from the entry point (**L§11.6**), so nothing
  // reaches this one.
  assert!(!listing(&[]).contains("procedure unreachable_one"));
  // `compiler_make_procedure_live` is how a metaprogram says to lower it
  // anyway (**C§3.3**).
  let asked = listing(&[(String::new(), String::from("unreachable_one"))]);
  assert!(
    asked.contains("procedure unreachable_one"),
    "the procedure a metaprogram made live is lowered:\n{asked}"
  );
}

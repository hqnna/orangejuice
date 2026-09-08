//! **M5**: what the LLVM module looks like.
//!
//! The interesting decisions are the ones `oj-types` already made: aggregates
//! are byte arrays with an explicit alignment, a `bool` is a byte, and the
//! entry point the C runtime calls is generated rather than written.

use oj_diag::SourceMap;
use oj_lexer::Interner;

fn llvm_ir(source: &str) -> Option<String> {
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

  let sources = SourceMap::new();
  let interner = Interner::new();
  let options = oj_scope::Options {
    jai_dir: Some(jai_dir),
    ..oj_scope::Options::default()
  };
  let program = oj_scope::Program::build(&sources, &interner, &path, options);
  assert!(!program.has_errors(), "the program should resolve");
  let mut checker = oj_sema::Checker::new(&program);
  checker.check();
  assert!(!checker.has_errors(), "the program should typecheck");
  let lowered = oj_ir::lower(&mut checker);
  assert!(
    !lowered.has_errors(),
    "the program should lower: {:#?}",
    lowered
      .diagnostics
      .iter()
      .map(|diagnostic| &diagnostic.message)
      .collect::<Vec<_>>()
  );

  Some(
    oj_codegen::compile(
      &lowered.program,
      &oj_codegen::Options::default(),
      oj_codegen::Output::LlvmIr,
    )
    .expect("the module should be valid"),
  )
}

#[test]
fn the_module_targets_linux_x86_64() {
  let Some(module) = llvm_ir("main :: () {}\n") else {
    return;
  };
  assert!(
    module.contains(&format!(
      "target triple = \"{}\"",
      oj_codegen::DEFAULT_TRIPLE
    )),
    "{module}"
  );
  assert!(module.contains("target datalayout ="), "{module}");
}

#[test]
fn the_c_runtime_calls_a_generated_main_that_hands_over_a_context() {
  let Some(module) = llvm_ir("main :: () {}\n") else {
    return;
  };
  assert!(
    module.contains("define i32 @main(i32 %0, ptr %1)"),
    "{module}"
  );
  // The context the program runs in is the one Runtime_Support builds, with
  // its allocator, its logger and its temporary storage (**C§13**).
  assert!(
    module.contains("call ptr @__jai_runtime_init(i32 %0, ptr %1)"),
    "{module}"
  );
  assert!(
    module.contains("call void @__program_main(ptr %"),
    "{module}"
  );
}

#[test]
fn an_aggregate_is_storage_of_its_own_size_and_alignment() {
  let Some(module) =
    llvm_ir("Point :: struct { x: float64; y: float64; }\nmain :: () { p: Point; }\n")
  else {
    return;
  };
  assert!(module.contains("alloca [16 x i8], align 8"), "{module}");
}

#[test]
fn a_scalar_load_takes_the_alignment_of_its_type() {
  let Some(module) = llvm_ir("main :: () { n := 1; m := n; }\n") else {
    return;
  };
  assert!(module.contains("store i64 1, ptr %n, align 8"), "{module}");
}

#[test]
fn a_string_literal_is_read_only_data_and_the_two_words_of_a_view() {
  let Some(module) = llvm_ir("main :: () { greeting := \"hi\"; }\n") else {
    return;
  };
  assert!(module.contains("private"), "{module}");
  assert!(module.contains("c\"hi\""), "{module}");
  assert!(module.contains("{ i64 2, ptr"), "{module}");
}

#[test]
fn a_comparison_produces_a_bit_and_stores_a_byte() {
  // A comparison of two literals folds, so the operands have to be storage.
  let Some(module) = llvm_ir("main :: () { a := 1; b := 2; flag := a < b; }\n") else {
    return;
  };
  assert!(module.contains("icmp slt i64"), "{module}");
  assert!(module.contains("zext i1"), "{module}");
  assert!(module.contains("i8"), "{module}");
}

#[test]
fn a_foreign_procedure_is_only_declared() {
  let Some(module) = llvm_ir(
    "libc :: #library,system \"libc\";\n\
     puts :: (text: *u8) -> s32 #foreign libc;\n\
     main :: () { puts(null); }\n",
  ) else {
    return;
  };
  assert!(module.contains("declare i32 @puts(ptr)"), "{module}");
}

#[test]
fn assembly_can_be_produced_for_the_same_program() {
  let Some(_) = llvm_ir("main :: () {}\n") else {
    return;
  };
  let jai_dir = oj_testsupport::jai_dir().expect("checked above");
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("input.jai");
  std::fs::write(&path, "main :: () {}\n").expect("the input should be writable");

  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = oj_scope::Program::build(
    &sources,
    &interner,
    &path,
    oj_scope::Options {
      jai_dir: Some(jai_dir),
      ..oj_scope::Options::default()
    },
  );
  let mut checker = oj_sema::Checker::new(&program);
  checker.check();
  let lowered = oj_ir::lower(&mut checker);
  let assembly = oj_codegen::compile(
    &lowered.program,
    &oj_codegen::Options::default(),
    oj_codegen::Output::Assembly,
  )
  .expect("the module should assemble");
  assert!(assembly.contains("__program_main"), "{assembly}");
}

#[test]
fn an_object_file_is_written_where_it_was_asked_for() {
  let Some(_) = llvm_ir("main :: () {}\n") else {
    return;
  };
  let jai_dir = oj_testsupport::jai_dir().expect("checked above");
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("input.jai");
  std::fs::write(&path, "main :: () {}\n").expect("the input should be writable");
  let object = directory.path().join("input.o");

  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = oj_scope::Program::build(
    &sources,
    &interner,
    &path,
    oj_scope::Options {
      jai_dir: Some(jai_dir),
      ..oj_scope::Options::default()
    },
  );
  let mut checker = oj_sema::Checker::new(&program);
  checker.check();
  let lowered = oj_ir::lower(&mut checker);
  oj_codegen::compile(
    &lowered.program,
    &oj_codegen::Options::default(),
    oj_codegen::Output::Object(&object),
  )
  .expect("the object should be written");
  let bytes = std::fs::read(&object).expect("the object should exist");
  assert_eq!(&bytes[..4], b"\x7fELF");
}

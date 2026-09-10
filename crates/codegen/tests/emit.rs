//! **M5**: what the LLVM module looks like.
//!
//! The interesting decisions are the ones `oj-types` already made: aggregates
//! are byte arrays with an explicit alignment, a `bool` is a byte, and the
//! entry point the C runtime calls is generated rather than written.

use oj_diag::SourceMap;
use oj_lexer::Interner;

fn llvm_ir(source: &str) -> Option<String> {
  llvm_ir_with(source, &oj_codegen::Options::default())
}

fn llvm_ir_with(source: &str, options: &oj_codegen::Options) -> Option<String> {
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
  let scope_options = oj_scope::Options {
    jai_dir: Some(jai_dir),
    ..oj_scope::Options::default()
  };
  let program = oj_scope::Program::build(&sources, &interner, &path, scope_options);
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
    oj_codegen::compile(&lowered.program, options, oj_codegen::Output::LlvmIr)
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
  // The bytes carry a trailing zero the count does not include, so a literal
  // handed to a C procedure is the string it expects (**L§3.4**).
  assert!(module.contains("c\"hi\\00\""), "{module}");
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

#[test]
fn the_module_describes_the_program_to_a_debugger() {
  // `Build_Options.emit_debug_info` is `.DEFAULT`, which is DWARF on Linux
  // (**C§4**): a compile unit, a subprogram per procedure, a line per
  // instruction and a variable per local.
  let Some(module) = llvm_ir("main :: () { n := 1 + 2; }\n") else {
    return;
  };
  assert!(
    module.contains("!llvm.dbg.cu"),
    "the module should carry a compile unit:\n{module}"
  );
  assert!(
    module.contains("!DISubprogram(name: \"main\""),
    "`main` should have a subprogram:\n{module}"
  );
  assert!(
    module.contains("!DILocalVariable(name: \"n\""),
    "the local should be described:\n{module}"
  );
  assert!(
    module.contains("!DILocation("),
    "instructions should carry lines:\n{module}"
  );
  assert!(
    module.contains("!DIFile(filename: \"input.jai\""),
    "the file should be named:\n{module}"
  );
}

#[test]
fn a_program_built_without_debug_info_carries_none() {
  let options = oj_codegen::Options {
    debug_info: false,
    ..oj_codegen::Options::default()
  };
  let Some(module) = llvm_ir_with("main :: () { n := 1 + 2; }\n", &options) else {
    return;
  };
  assert!(
    !module.contains("!llvm.dbg.cu"),
    "nothing should describe the program:\n{module}"
  );
}

#[test]
fn a_struct_is_described_member_by_member() {
  // The debug types come out of `oj-types`, not out of the byte arrays the
  // module stores an aggregate in (`docs/spec.md` §10).
  let Some(module) =
    llvm_ir("Point :: struct { x: float64; y: s32; }\nmain :: () { p: Point; p.y = 1; }\n")
  else {
    return;
  };
  assert!(
    module.contains("!DICompositeType(tag: DW_TAG_structure_type, name: \"Point\""),
    "the struct should be described:\n{module}"
  );
  assert!(
    module.contains("!DIDerivedType(tag: DW_TAG_member, name: \"y\""),
    "its members should be described:\n{module}"
  );
}

#[test]
fn an_optimized_build_runs_the_pass_pipeline() {
  // `Llvm_Options.bitcode_optimization_setting` is what `-release` raises to
  // `O2` (**C§4**), which is a pipeline over the module rather than a target
  // machine setting.
  let source = "add :: (a: int, b: int) -> int { return a + b; }\nmain :: () { n := add(1, 2); }\n";
  let Some(debug) = llvm_ir(source) else {
    return;
  };
  let optimized = llvm_ir_with(
    source,
    &oj_codegen::Options {
      bitcode: oj_codegen::Bitcode::O2,
      optimization: 2,
      ..oj_codegen::Options::default()
    },
  )
  .expect("the distribution was there for the first build");
  let allocas = |module: &str| module.matches(" = alloca ").count();
  assert!(
    allocas(&optimized) < allocas(&debug),
    "optimizing should promote storage to registers: {} allocas against {}",
    allocas(&optimized),
    allocas(&debug)
  );
  assert!(
    optimized.contains("define") && optimized.contains("main"),
    "the entry point should survive:\n{optimized}"
  );
}

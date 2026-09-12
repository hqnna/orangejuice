//! What the LLVM module looks like.
//!
//! The interesting decisions are the ones `oj-types` already made: aggregates
//! are byte arrays with an explicit alignment, a `bool` is a byte, and the
//! entry point the C runtime calls is generated rather than written.

use oj_diag::SourceMap;
use oj_lexer::Interner;

fn llvm_ir(source: &str) -> String {
  llvm_ir_with(source, &oj_codegen::Options::default())
}

fn llvm_ir_with(source: &str, options: &oj_codegen::Options) -> String {
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("input.jai");
  std::fs::write(&path, source).expect("the input should be writable");

  let sources = SourceMap::new();
  let interner = Interner::new();
  let scope_options = oj_scope::Options {
    distribution: oj_testsupport::distribution().to_path_buf().into(),
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

  oj_codegen::compile(&lowered.program, options, oj_codegen::Output::LlvmIr)
    .expect("the module should be valid")
}

#[test]
fn the_module_targets_the_host_unless_it_is_told_otherwise() {
  let module = llvm_ir("main :: () {}\n");
  assert!(
    module.contains(&format!(
      "target triple = \"{}\"",
      oj_codegen::default_triple()
    )),
    "{module}"
  );
  assert!(module.contains("target datalayout ="), "{module}");
}

#[test]
fn the_c_runtime_calls_a_generated_main_that_hands_over_a_context() {
  let module = llvm_ir("main :: () {}\n");
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
  let module = llvm_ir("Point :: struct { x: float64; y: float64; }\nmain :: () { p: Point; }\n");
  assert!(module.contains("alloca [16 x i8], align 8"), "{module}");
}

#[test]
fn a_scalar_load_takes_the_alignment_of_its_type() {
  let module = llvm_ir("main :: () { n := 1; m := n; }\n");
  assert!(module.contains("store i64 1, ptr %n, align 8"), "{module}");
}

#[test]
fn a_string_literal_is_read_only_data_and_the_two_words_of_a_view() {
  let module = llvm_ir("main :: () { greeting := \"hi\"; }\n");
  assert!(module.contains("private"), "{module}");
  // The bytes carry a trailing zero the count does not include, so a literal
  // handed to a C procedure is the string it expects (**L§3.4**).
  assert!(module.contains("c\"hi\\00\""), "{module}");
  assert!(module.contains("{ i64 2, ptr"), "{module}");
}

#[test]
fn a_comparison_produces_a_bit_and_stores_a_byte() {
  // A comparison of two literals folds, so the operands have to be storage.
  let module = llvm_ir("main :: () { a := 1; b := 2; flag := a < b; }\n");
  assert!(module.contains("icmp slt i64"), "{module}");
  assert!(module.contains("zext i1"), "{module}");
  assert!(module.contains("i8"), "{module}");
}

#[test]
fn a_foreign_procedure_is_only_declared() {
  let module = llvm_ir(
    "libc :: #library,system \"libc\";\n\
     puts :: (text: *u8) -> s32 #foreign libc;\n\
     main :: () { puts(null); }\n",
  );
  assert!(module.contains("declare i32 @puts(ptr)"), "{module}");
}

#[test]
fn assembly_can_be_produced_for_the_same_program() {
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
      distribution: oj_testsupport::distribution().to_path_buf().into(),
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
      distribution: oj_testsupport::distribution().to_path_buf().into(),
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
  // The object format is the target's: ELF everywhere but Darwin, which is
  // Mach-O (`docs/spec.md` §2.1).
  let magic: &[u8] = if oj_types::Target::HOST.is_darwin() {
    &[0xcf, 0xfa, 0xed, 0xfe]
  } else {
    b"\x7fELF"
  };
  assert_eq!(&bytes[..4], magic);
}

#[test]
fn the_module_describes_the_program_to_a_debugger() {
  // `Build_Options.emit_debug_info` is `.DEFAULT`, which is DWARF on Linux
  // (**C§4**): a compile unit, a subprogram per procedure, a line per
  // instruction and a variable per local.
  let module = llvm_ir("main :: () { n := 1 + 2; }\n");
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
  let module = llvm_ir_with("main :: () { n := 1 + 2; }\n", &options);
  assert!(
    !module.contains("!llvm.dbg.cu"),
    "nothing should describe the program:\n{module}"
  );
}

#[test]
fn a_struct_is_described_member_by_member() {
  // The debug types come out of `oj-types`, not out of the byte arrays the
  // module stores an aggregate in (`docs/spec.md` §10).
  let module =
    llvm_ir("Point :: struct { x: float64; y: s32; }\nmain :: () { p: Point; p.y = 1; }\n");
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
  let debug = llvm_ir(source);
  let optimized = llvm_ir_with(
    source,
    &oj_codegen::Options {
      bitcode: oj_codegen::Bitcode::O2,
      optimization: 2,
      ..oj_codegen::Options::default()
    },
  );
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

/// A split program has to be one program: what a unit does not define it
/// declares, and nothing it keeps to itself may reach `.dynsym`, where a name
/// like `errno` would interpose on the C library's own — which is what made a
/// split program die in the dynamic loader before `main`.
#[test]
fn a_split_program_hides_what_it_does_not_export() {
  let options = oj_codegen::Options {
    unit_index: 1,
    unit_count: 2,
    debug_info: false,
    ..oj_codegen::Options::default()
  };
  let text = llvm_ir_with(
    "helper :: (x: int) -> int { return x * 2; }\n\
     main :: () { n := helper(21); }\n",
    &options,
  );
  // The entry point belongs to the primary unit and to nothing else.
  assert!(!text.contains("define i32 @main("), "{text}");
  // Whatever this unit defines is either something the program exports on
  // purpose — `__jai_runtime_init`, which a library publishes — or hidden, so
  // that `-export-dynamic` cannot publish an internal name like `errno` over
  // the C library's own.
  let mut hidden = 0;
  for line in text.lines().filter(|line| line.starts_with("define ")) {
    if line.contains("@__jai_") {
      continue;
    }
    assert!(
      line.contains(" hidden "),
      "a procedure the program keeps to itself escaped into the dynamic symbols: {line}"
    );
    hidden += 1;
  }
  assert!(hidden > 0, "the unit should define something: {text}");
}

/// The whole program in one module is what a small one gets, and then a
/// procedure nobody exports keeps internal linkage.
#[test]
fn one_unit_keeps_internal_linkage() {
  let text = llvm_ir(
    "helper :: (x: int) -> int { return x * 2; }\n\
     main :: () { n := helper(21); }\n",
  );
  assert!(text.contains("define internal"), "{text}");
  assert!(text.contains("define i32 @main("), "{text}");
}

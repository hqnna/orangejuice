//! A Jai program becomes an executable that runs, and what its
//! `#run`s left behind is part of it.
//!
//! These are the acceptance tests: the pipeline is driven end to
//! end and the produced program is executed, so what is asserted is its
//! behaviour rather than any intermediate listing.

use std::process::Command;

/// A program that can print without `Basic`: `print` is a polymorphic call
/// over `Any`, so the tests reach `write(2)` the way `Runtime_Support`
/// itself does.
const PRELUDE: &str = "\
libc :: #library,system \"libc\";
write :: (fd: s32, buffer: *void, count: u64) -> s64 #foreign libc;

put :: (text: string) {
  write(1, text.data, cast(u64) text.count);
}

put_number :: (value: int) {
  digits: [24] u8;
  n := value;
  negative := n < 0;
  if negative  n = -n;
  count := 0;
  while true {
    digits[count] = cast(u8)(n % 10 + 48);
    count += 1;
    n = n / 10;
    if n == 0  break;
  }
  out: [26] u8;
  length := 0;
  if negative {
    out[0] = #char \"-\";
    length = 1;
  }
  i := count - 1;
  while i >= 0 {
    out[length] = digits[i];
    length += 1;
    i -= 1;
  }
  out[length] = #char \"\\n\";
  write(1, out.data, cast(u64)(length + 1));
}
";

struct Built {
  output: String,
  status: i32,
}

fn build_and_run(body: &str) -> Option<Built> {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return None;
  }

  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, format!("{PRELUDE}\n{body}")).expect("the input should be writable");

  let options = oj_driver::BuildOptions::new();
  // SAFETY: the tests in this file are the only ones that read it, and cargo
  // runs each integration test binary in its own process.
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  Some(Built {
    output: String::from_utf8_lossy(&output.stdout).into_owned(),
    status: output.status.code().unwrap_or(-1),
  })
}

fn linker_is_available() -> bool {
  let driver = oj_link::driver();
  if driver.is_absolute() {
    return driver.exists();
  }
  std::env::var_os("PATH")
    .map(|value| std::env::split_paths(&value).any(|directory| directory.join(&driver).exists()))
    .unwrap_or(false)
}

fn assert_output(body: &str, expected: &str) {
  let Some(built) = build_and_run(body) else {
    return;
  };
  assert_eq!(built.status, 0, "the program should exit cleanly");
  assert_eq!(built.output, expected);
}

#[test]
fn a_program_prints_through_a_foreign_procedure() {
  assert_output(
    "main :: () { put(\"hello, world\\n\"); }\n",
    "hello, world\n",
  );
}

#[test]
fn arithmetic_and_calls_produce_the_right_number() {
  assert_output(
    "add :: (a: int, b: int) -> int { return a + b; }\n\
     main :: () { put_number(add(19, 23)); }\n",
    "42\n",
  );
}

#[test]
fn control_flow_runs_in_the_right_order() {
  assert_output(
    "main :: () {\n\
       total := 0;\n\
       for i: 1..5 {\n\
         if i == 3  continue;\n\
         if i == 5  break;\n\
         total += i;\n\
       }\n\
       put_number(total);\n\
     }\n",
    "7\n",
  );
}

#[test]
fn a_while_loop_with_a_defer_runs_the_defer_last() {
  assert_output(
    "main :: () {\n\
       defer put(\"last\\n\");\n\
       n := 0;\n\
       while n < 3  n += 1;\n\
       put_number(n);\n\
     }\n",
    "3\nlast\n",
  );
}

#[test]
fn structs_are_copied_by_value_and_members_keep_their_offsets() {
  assert_output(
    "Point :: struct { x: int; y: int = 5; }\n\
     main :: () {\n\
       p: Point;\n\
       p.x = 2;\n\
       q := p;\n\
       q.x = 100;\n\
       put_number(p.x + p.y);\n\
       put_number(q.x);\n\
     }\n",
    "7\n100\n",
  );
}

#[test]
fn a_struct_is_passed_and_returned_by_pointer_when_it_does_not_fit_a_register() {
  assert_output(
    "Box :: struct { a: int; b: int; c: int; }\n\
     grow :: (box: Box) -> Box {\n\
       result: Box;\n\
       result.a = box.a + 1;\n\
       result.b = box.b + 1;\n\
       result.c = box.c + 1;\n\
       return result;\n\
     }\n\
     main :: () {\n\
       start: Box;\n\
       start.a = 1;\n\
       start.b = 2;\n\
       start.c = 3;\n\
       end := grow(start);\n\
       put_number(end.a + end.b + end.c);\n\
     }\n",
    "9\n",
  );
}

#[test]
fn arrays_index_by_element_and_views_carry_their_count() {
  assert_output(
    "sum :: (values: [] int) -> int {\n\
       total := 0;\n\
       for values  total += it;\n\
       return total;\n\
     }\n\
     main :: () {\n\
       numbers: [4] int;\n\
       for i: 0..3  numbers[i] = i + 1;\n\
       put_number(sum(numbers));\n\
     }\n",
    "10\n",
  );
}

#[test]
fn several_returns_come_back_through_the_storage_the_caller_gave() {
  assert_output(
    "divide :: (a: int, b: int) -> int, int { return a / b, a % b; }\n\
     main :: () {\n\
       quotient, remainder := divide(17, 5);\n\
       put_number(quotient);\n\
       put_number(remainder);\n\
     }\n",
    "3\n2\n",
  );
}

#[test]
fn a_global_is_shared_between_procedures() {
  assert_output(
    "counter := 10;\n\
     bump :: () { counter += 1; }\n\
     main :: () { bump(); bump(); put_number(counter); }\n",
    "12\n",
  );
}

#[test]
fn a_global_whose_value_is_computed_is_ready_before_main() {
  assert_output(
    "seed :: () -> int { return 6; }\n\
     start := seed() * 7;\n\
     main :: () { put_number(start); }\n",
    "42\n",
  );
}

#[test]
fn pointers_read_and_write_the_thing_they_point_at() {
  assert_output(
    "bump :: (target: *int) { target.* = target.* + 1; }\n\
     main :: () { n := 41; bump(*n); put_number(n); }\n",
    "42\n",
  );
}

#[test]
fn enums_compare_against_an_unqualified_member() {
  assert_output(
    "Colour :: enum u8 { RED; GREEN; BLUE; }\n\
     main :: () {\n\
       c := Colour.BLUE;\n\
       if c == .BLUE  put(\"blue\\n\");\n\
       put_number(cast(int) c);\n\
     }\n",
    "blue\n2\n",
  );
}

#[test]
fn a_switch_runs_one_arm_and_falls_through_when_told_to() {
  assert_output(
    "classify :: (n: int) {\n\
       if n == {\n\
         case 1;\n\
           put(\"one\\n\");\n\
           #through;\n\
         case 2;\n\
           put(\"two\\n\");\n\
         case;\n\
           put(\"many\\n\");\n\
       }\n\
     }\n\
     main :: () { classify(1); classify(2); classify(9); }\n",
    "one\ntwo\ntwo\nmany\n",
  );
}

#[test]
fn short_circuit_operators_do_not_evaluate_the_second_side() {
  assert_output(
    "shout :: () -> bool { put(\"evaluated\\n\"); return true; }\n\
     main :: () {\n\
       if false && shout()  put(\"unreachable\\n\");\n\
       if true || shout()   put(\"chosen\\n\");\n\
     }\n",
    "chosen\n",
  );
}

#[test]
fn floats_convert_and_compare() {
  assert_output(
    "main :: () {\n\
       x: float64 = 2.5;\n\
       y := x * 4.0;\n\
       put_number(cast(int) y);\n\
     }\n",
    "10\n",
  );
}

#[test]
fn a_program_the_back_end_cannot_build_names_what_stopped_it() {
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, "main :: () { #asm { frobnicate a:, 1; } }\n")
    .expect("the input should be writable");
  unsafe {
    oj_testsupport::use_own_modules();
  }

  let report = oj_driver::run(
    &path,
    &oj_driver::BuildOptions::new(),
    oj_driver::Stage::Executable,
    None,
  );
  assert!(report.failed);
  let text = report.diagnostics.join("");
  assert!(
    text.contains("no encoding for the '#asm' instruction 'frobnicate'"),
    "{text}"
  );
}

#[test]
fn the_object_lands_in_a_build_directory_beside_the_output() {
  let Some(built) = build_and_run("main :: () {}\n") else {
    return;
  };
  assert_eq!(built.status, 0);
}

#[test]
fn an_output_name_and_path_are_honoured() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, "main :: () {}\n").expect("the input should be writable");
  let out = directory.path().join("out");
  std::fs::create_dir(&out).expect("the output directory should be creatable");
  unsafe {
    oj_testsupport::use_own_modules();
  }

  let parsed = oj_driver::parse(&[
    String::from("-exe"),
    String::from("game"),
    String::from("-output_path"),
    out.display().to_string(),
  ])
  .expect("the option list should parse");
  let report = oj_driver::run(&path, &parsed.options, oj_driver::Stage::Executable, None);
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert_eq!(
    report.executable.as_deref(),
    Some(out.join("game").as_path())
  );
  assert!(out.join("game").exists());
  assert!(out.join(".build").join("game.o").exists());
}

// ------------------------------------------- compile-time execution -------
//
// Compile-time execution. What is asserted is the *program's* behaviour, so a
// `#run` that got the wrong answer, ran twice, or did not run at all shows up
// as wrong output rather than as a listing that reads oddly.

/// Builds a program and returns its diagnostics rather than running it, for
/// the cases where the point is that the build fails.
fn diagnostics_of(body: &str) -> Option<String> {
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, format!("{PRELUDE}\n{body}")).expect("the input should be writable");
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let report = oj_driver::run(
    &path,
    &oj_driver::BuildOptions::new(),
    oj_driver::Stage::Executable,
    None,
  );
  Some(report.diagnostics.join(""))
}

#[test]
fn a_run_computes_the_constant_a_declaration_was_waiting_for() {
  assert_output(
    "factorial :: (x: int) -> int {\n\
       if x <= 1  return 1;\n\
       return x * factorial(x - 1);\n\
     }\n\
     FACT :: #run factorial(5);\n\
     main :: () { put_number(FACT); }\n",
    "120\n",
  );
}

#[test]
fn a_run_is_folded_into_the_code_rather_than_called_again() {
  let Some(built) = build_and_run(
    "counter := 0;\n\
     bump :: () -> int { counter += 1; return counter; }\n\
     A :: #run bump();\n\
     B :: #run bump();\n\
     main :: () { put_number(A); put_number(B); put_number(counter); }\n",
  ) else {
    return;
  };
  // Each `#run` ran once, in order; and the global it wrote at compile time was
  // reset before the executable was written (**L§12.3**).
  assert_eq!(built.output, "1\n2\n0\n");
}

#[test]
fn a_run_block_returns_through_its_declared_type() {
  assert_output(
    "SUM :: #run -> int {\n\
       total := 0;\n\
       for i: 1..10  total += i;\n\
       return total;\n\
     };\n\
     main :: () { put_number(SUM); }\n",
    "55\n",
  );
}

#[test]
fn a_run_produces_a_string_the_program_prints() {
  assert_output(
    "greeting :: () -> string { return \"hello from compile time\\n\"; }\n\
     GREETING :: #run greeting();\n\
     main :: () { put(GREETING); }\n",
    "hello from compile time\n",
  );
}

#[test]
fn a_no_reset_global_keeps_what_compile_time_left_in_it() {
  assert_output(
    "plain: int;\n\
     #no_reset kept: int;\n\
     #no_reset table: [8] u8;\n\
     #run {\n\
       plain = 1;\n\
       kept = 7;\n\
       for i: 0..7  table[i] = cast(u8)(65 + i);\n\
     }\n\
     main :: () {\n\
       put_number(plain);\n\
       put_number(kept);\n\
       write(1, table.data, 8);\n\
       put(\"\\n\");\n\
     }\n",
    "0\n7\nABCDEFGH\n",
  );
}

#[test]
fn two_runs_share_one_global() {
  assert_output(
    "#no_reset total: int;\n\
     #run { total = 20; }\n\
     #run { total += 22; }\n\
     main :: () { put_number(total); }\n",
    "42\n",
  );
}

#[test]
fn a_run_calls_a_foreign_procedure_at_compile_time() {
  // The `#run` writes to standard output while the compiler is running, so the
  // program itself prints nothing.
  assert_output("#run put(\"\");\nmain :: () {}\n", "");
}

#[test]
fn a_static_if_decides_on_the_result_of_a_run() {
  assert_output(
    "answer :: () -> int { return 42; }\n\
     main :: () {\n\
       #if #run answer() == 42 {\n\
         put(\"yes\\n\");\n\
       } else {\n\
         put(\"no\\n\");\n\
       }\n\
     }\n",
    "yes\n",
  );
}

#[test]
fn ordering_two_strings_says_which_operators_strings_have() {
  // A string is a count and a pointer, so `<` has nothing to mean. The
  // wording is the reference's, measured by putting the same program through
  // it (**C§12**).
  let Some(text) = diagnostics_of("main :: () { a := \"x\"; b := \"y\"; c := a > b; }\n") else {
    return;
  };
  assert!(
    text.contains(
      "Error: Operator '>' does not work on strings. \
       (Only == and != can be used on strings.)"
    ),
    "{text}"
  );

  // Equality is what strings do have, and it compares their bytes.
  let Some(text) = diagnostics_of("main :: () { a := \"x\"; b := \"y\"; c := a == b; }\n") else {
    return;
  };
  assert!(!text.contains("does not work on strings"), "{text}");
}

#[test]
fn a_failing_assertion_says_so_the_way_the_reference_does() {
  let Some(text) = diagnostics_of("#assert 1 == 2;\nmain :: () {}\n") else {
    return;
  };
  assert!(
    text.contains("Error: Compile-time assertion failed."),
    "{text}"
  );
}

#[test]
fn a_failing_assertion_quotes_its_message() {
  let Some(text) = diagnostics_of("#assert 1 == 2 \"one is not two\";\nmain :: () {}\n") else {
    return;
  };
  assert!(
    text.contains("Error: Compile-time assertion failed. \"one is not two\""),
    "{text}"
  );
}

#[test]
fn an_assertion_over_a_run_is_executed() {
  let Some(text) = diagnostics_of(
    "answer :: () -> int { return 41; }\n\
     #assert #run answer() == 42 \"the answer moved\";\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(
    text.contains("Error: Compile-time assertion failed. \"the answer moved\""),
    "{text}"
  );
}

#[test]
fn an_assertion_that_holds_reports_nothing() {
  assert_output(
    "#assert 1 + 1 == 2 \"arithmetic\";\n\
     answer :: () -> int { return 42; }\n\
     #assert #run answer() == 42;\n\
     main :: () { put(\"ok\\n\"); }\n",
    "ok\n",
  );
}

#[test]
fn a_run_produces_a_struct_the_program_reads_members_out_of() {
  assert_output(
    "Point :: struct { x: int; y: int; }\n\
     make :: () -> Point { p: Point; p.x = 3; p.y = 4; return p; }\n\
     P :: #run make();\n\
     main :: () { put_number(P.x); put_number(P.y); }\n",
    "3\n4\n",
  );
}

#[test]
fn a_run_produces_an_array_the_program_indexes() {
  assert_output(
    "TABLE :: #run -> [4] u8 {\n\
       t: [4] u8;\n\
       for i: 0..3  t[i] = cast(u8)(97 + i);\n\
       return t;\n\
     };\n\
     main :: () {\n\
       table := TABLE;\n\
       write(1, table.data, 4);\n\
       put(\"\\n\");\n\
     }\n",
    "abcd\n",
  );
}

// ------------------------------------------- compile-time execution -------
//
// The type table. `type_info(T)` is a pointer into data the compiler laid out,
// so what is asserted is what the running program reads back out of it.

#[test]
fn type_info_describes_a_struct() {
  assert_output(
    "Point :: struct { x: int; y: float; }\n\
     main :: () {\n\
       info := type_info(Point);\n\
       if info.type == .STRUCT  put(\"struct\\n\");\n\
       put_number(info.runtime_size);\n\
       put(info.name);\n\
       put(\"\\n\");\n\
       for info.members {\n\
         put(it.name);\n\
         put_number(it.offset_in_bytes);\n\
       }\n\
     }\n",
    "struct\n16\nPoint\nx0\ny8\n",
  );
}

#[test]
fn type_info_describes_the_types_a_type_mentions() {
  assert_output(
    "main :: () {\n\
       array := type_info([4] s32);\n\
       put_number(array.array_count);\n\
       put_number(array.element_type.runtime_size);\n\
       pointer := type_info(*float64);\n\
       put_number(pointer.pointer_to.runtime_size);\n\
       if type_info(int).signed  put(\"signed\\n\");\n\
     }\n",
    "4\n4\n8\nsigned\n",
  );
}

#[test]
fn type_info_describes_an_enum() {
  assert_output(
    "Colour :: enum u8 { RED; GREEN; BLUE :: 7; }\n\
     main :: () {\n\
       info := type_info(Colour);\n\
       put(info.name);\n\
       put(\"\\n\");\n\
       put_number(info.internal_type.runtime_size);\n\
       for info.names { put(it); put(\"\\n\"); }\n\
       for info.values  put_number(it);\n\
     }\n",
    "Colour\n1\nRED\nGREEN\nBLUE\n0\n1\n7\n",
  );
}

#[test]
fn a_type_that_mentions_itself_is_laid_out_once() {
  assert_output(
    // Following `next` leads back to `Node`'s own record, which only
    // terminates because the record is placed before it is filled in.
    "Node :: struct { value: int; next: *Node; }\n\
     main :: () {\n\
       info := type_info(Node);\n\
       next := cast(*Type_Info_Pointer) info.members[1].type;\n\
       around := cast(*Type_Info_Struct) next.pointer_to;\n\
       put(around.name);\n\
       put_number(around.members.count);\n\
     }\n",
    "Node2\n",
  );
}

#[test]
fn a_run_reads_the_type_table_at_compile_time() {
  assert_output(
    "Point :: struct { x: int; y: float; }\n\
     count :: () -> int {\n\
       info := type_info(Point);\n\
       return info.members.count;\n\
     }\n\
     COUNT :: #run count();\n\
     main :: () { put_number(COUNT); }\n",
    "2\n",
  );
}

#[test]
fn an_any_carries_the_type_and_the_value_it_was_made_from() {
  assert_output(
    "describe :: (value: Any) {\n\
       if value.type.type == .INTEGER {\n\
         put(\"integer \");\n\
         put_number((cast(*int) value.value_pointer).*);\n\
       } else if value.type.type == .STRING {\n\
         put(\"string \");\n\
         put((cast(*string) value.value_pointer).*);\n\
         put(\"\\n\");\n\
       } else {\n\
         put(\"other\\n\");\n\
       }\n\
     }\n\
     Point :: struct { x: int; }\n\
     main :: () {\n\
       describe(7);\n\
       describe(\"hello\");\n\
       p: Point;\n\
       describe(p);\n\
     }\n",
    "integer 7\nstring hello\nother\n",
  );
}

#[test]
fn compile_time_is_true_only_while_the_compiler_is_running() {
  assert_output(
    "where :: () {\n\
       if #compile_time  put(\"compile time\\n\"); else put(\"run time\\n\");\n\
     }\n\
     #run where();\n\
     main :: () { where(); }\n",
    "run time\n",
  );
}

#[test]
fn a_run_in_a_polymorphic_body_waits_for_the_instantiation() {
  // The body of a polymorph does not exist until something instantiates it,
  // so the `#run` inside it neither executes nor reports (**L§12.1**).
  assert_output(
    "width :: ($U: Type) -> int { return size_of(U); }\n\
     unreached :: ($T: Type) -> int {\n\
       SIZE :: #run width(T);\n\
       return SIZE;\n\
     }\n\
     main :: () { put(\"ok\\n\"); }\n",
    "ok\n",
  );
}

#[test]
fn a_constant_array_is_data_the_program_reads_out_of() {
  assert_output(
    "TABLE : [4] u64 : .[10, 20, 30, 40];\n\
     main :: () {\n\
       total := 0;\n\
       for i: 0..3  total += cast(int) TABLE[i];\n\
       put_number(total);\n\
     }\n",
    "100\n",
  );
}

#[test]
fn a_fixed_array_converts_to_a_view_with_its_count() {
  assert_output(
    "sum :: (values: [] int) -> int {\n\
       total := 0;\n\
       for values  total += it;\n\
       return total;\n\
     }\n\
     main :: () {\n\
       fixed: [3] int;\n\
       fixed[0] = 1; fixed[1] = 2; fixed[2] = 3;\n\
       view: [] int = fixed;\n\
       put_number(view.count);\n\
       put_number(sum(view));\n\
     }\n",
    "3\n6\n",
  );
}

#[test]
fn a_type_value_is_the_address_of_its_record() {
  assert_output(
    "Point :: struct { x: int; }\n\
     main :: () {\n\
       t := type_of(Point.{1});\n\
       u: Type = Point;\n\
       if t == u  put(\"same\\n\");\n\
       if t != type_of(3)  put(\"different\\n\");\n\
       put((cast(*Type_Info_Struct) t).name);\n\
       put(\"\\n\");\n\
     }\n",
    "same\ndifferent\nPoint\n",
  );
}

#[test]
fn a_location_knows_where_it_was_written() {
  // The character is 1-based and counts from the start of the line, so it is
  // where the `#` of `#location` sits.
  let Some(built) = build_and_run(
    "main :: () {\n\
       here := #location();\n\
       put_number(here.character_number);\n\
       put(here.fully_pathed_filename);\n\
       put(\"\\n\");\n\
     }\n",
  ) else {
    return;
  };
  let mut lines = built.output.lines();
  assert_eq!(lines.next(), Some("9"));
  assert!(
    lines
      .next()
      .is_some_and(|name| name.ends_with("program.jai")),
    "{}",
    built.output
  );
}

#[test]
fn push_context_changes_what_the_block_and_its_callees_see() {
  assert_output(
    "#add_context marker: int;\n\
     show :: () {\n\
       if context.marker == 7  put(\"seven\\n\"); else put(\"zero\\n\");\n\
     }\n\
     main :: () {\n\
       show();\n\
       c := context;\n\
       c.marker = 7;\n\
       push_context c { show(); }\n\
       show();\n\
     }\n",
    "zero\nseven\nzero\n",
  );
}

// --------------------------------------- polymorphs, macros and code ---

#[test]
fn a_polymorphic_procedure_is_instantiated_per_argument_type() {
  assert_output(
    "square :: (x: $T) -> T { return x * x; }\n\
     main :: () {\n\
       put_number(square(7));\n\
       put_number(cast(int) square(cast(float64) 3.0));\n\
     }\n",
    "49\n9\n",
  );
}

#[test]
fn a_type_variable_solved_through_a_pointer_reaches_the_right_body() {
  assert_output(
    "bump :: (p: *$T, by: T) { <<p = <<p + by; }\n\
     main :: () {\n\
       a: int = 40;\n\
       bump(*a, 2);\n\
       put_number(a);\n\
     }\n",
    "42\n",
  );
}

#[test]
fn a_baked_type_parameter_declares_the_bodys_locals() {
  assert_output(
    "Box :: struct { value: int; }\n\
     zeroed :: ($T: Type) -> T { result: T; return result; }\n\
     main :: () {\n\
       b := zeroed(Box);\n\
       put_number(b.value);\n\
       put_number(zeroed(int));\n\
     }\n",
    "0\n0\n",
  );
}

#[test]
fn extra_arguments_are_gathered_into_the_varargs_slot() {
  assert_output(
    "total :: (values: .. int) -> int {\n\
       s := 0;\n\
       for values  s += it;\n\
       return s;\n\
     }\n\
     forward :: (values: .. int) -> int { return total(..values); }\n\
     main :: () {\n\
       put_number(total());\n\
       put_number(total(1, 2, 3, 4));\n\
       put_number(forward(10, 20));\n\
     }\n",
    "0\n10\n30\n",
  );
}

#[test]
fn an_any_vararg_carries_each_arguments_own_type() {
  assert_output(
    "integers :: (args: .. Any) -> int {\n\
       n := 0;\n\
       for args  if it.type.type == .INTEGER  n += 1;\n\
       return n;\n\
     }\n\
     main :: () {\n\
       put_number(integers(1, \"two\", 3, 4.5));\n\
     }\n",
    "2\n",
  );
}

#[test]
fn a_macro_sees_the_locals_of_the_block_it_expands_into() {
  assert_output(
    "tally :: (value: int) #expand {\n\
       total += value;\n\
       count += 1;\n\
     }\n\
     main :: () {\n\
       total := 0;\n\
       count := 0;\n\
       tally(5);\n\
       tally(7);\n\
       put_number(total);\n\
       put_number(count);\n\
     }\n",
    "12\n2\n",
  );
}

#[test]
fn a_return_inside_a_macro_leaves_the_macro_with_its_value() {
  assert_output(
    "maximum :: (a: int, b: int) -> int #expand {\n\
       if a > b  return a;\n\
       return b;\n\
     }\n\
     main :: () {\n\
       put_number(maximum(3, 9));\n\
       put_number(maximum(11, 4) + 1);\n\
     }\n",
    "9\n12\n",
  );
}

#[test]
fn a_backticked_return_inside_a_macro_leaves_the_caller() {
  assert_output(
    "bail_unless :: (ok: bool) #expand {\n\
       if !ok  `return;\n\
     }\n\
     report :: (ok: bool) {\n\
       bail_unless(ok);\n\
       put(\"reached\\n\");\n\
     }\n\
     main :: () {\n\
       report(false);\n\
       report(true);\n\
     }\n",
    "reached\n",
  );
}

#[test]
fn caller_location_names_the_site_the_macro_was_written_at() {
  let Some(built) = build_and_run(
    "here :: (loc := #caller_location) -> int { return loc.line_number; }\n\
     through :: (loc := #caller_location) -> int #expand { return loc.line_number; }\n\
     main :: () {\n\
       put_number(here());\n\
       put_number(through());\n\
     }\n",
  ) else {
    return;
  };
  // The prelude is prepended, so what matters is that both name the line the
  // call was written on, and that they agree.
  let lines: Vec<&str> = built.output.lines().collect();
  assert_eq!(lines.len(), 2);
  assert_eq!(
    lines[0].parse::<i64>().unwrap() + 1,
    lines[1].parse::<i64>().unwrap()
  );
}

#[test]
fn a_using_of_a_value_reads_and_writes_its_members() {
  assert_output(
    "Player :: struct { health: int; armour: int; }\n\
     heal :: (using p: *Player, amount: int) {\n\
       health += amount;\n\
     }\n\
     main :: () {\n\
       p: Player;\n\
       p.health = 10;\n\
       heal(*p, 32);\n\
       put_number(p.health);\n\
       using p;\n\
       armour = 7;\n\
       put_number(p.armour);\n\
     }\n",
    "42\n7\n",
  );
}

#[test]
fn strings_compare_by_their_bytes() {
  assert_output(
    "main :: () {\n\
       a := \"hello\";\n\
       b := \"hello\";\n\
       if a == b  put(\"same\\n\");\n\
       if a != \"help\"  put(\"different\\n\");\n\
       if a  put(\"nonempty\\n\");\n\
       empty: string;\n\
       if !empty  put(\"empty\\n\");\n\
     }\n",
    "same\ndifferent\nnonempty\nempty\n",
  );
}

#[test]
fn a_pointer_advances_by_elements() {
  assert_output(
    "main :: () {\n\
       values: [8] int;\n\
       values[3] = 30;\n\
       p := values.data;\n\
       q := p + 3;\n\
       put_number(q.*);\n\
       put_number(q - p);\n\
       q -= 3;\n\
       put_number(cast(int)(q == values.data));\n\
     }\n",
    "30\n3\n1\n",
  );
}

#[test]
fn an_ifx_of_two_literals_settles_on_what_they_default_to() {
  assert_output(
    "main :: () {\n\
       n := 3;\n\
       put_number(ifx n >= 100 then 5 else 4);\n\
     }\n",
    "4\n",
  );
}

#[test]
fn a_view_is_true_when_it_holds_something() {
  assert_output(
    "main :: () {\n\
       storage: [3] int;\n\
       full: [] int = storage;\n\
       empty: [] int;\n\
       if full  put(\"full\\n\");\n\
       if !empty  put(\"empty\\n\");\n\
     }\n",
    "full\nempty\n",
  );
}

#[test]
fn a_pointer_is_indexed_like_an_array() {
  assert_output(
    "main :: () {\n\
       s := \"hello\";\n\
       p := s.data;\n\
       put_number(cast(int) p[1]);\n\
     }\n",
    "101\n",
  );
}

#[test]
fn a_complement_folds_and_a_shift_keeps_the_left_operand() {
  assert_output(
    "main :: () {\n\
       MASK :: ~(64 - 1);\n\
       put_number(MASK);\n\
       n := 13;\n\
       put_number((n + 7) & ~7);\n\
       shift := 3;\n\
       put_number(1 << shift);\n\
     }\n",
    "-64\n16\n8\n",
  );
}

#[test]
fn a_bool_and_an_enum_cast_to_a_pointer() {
  assert_output(
    "Caps :: enum_flags u32 { A; B; }\n\
     main :: () {\n\
       yes := cast(*void) true;\n\
       both := cast(*void) (Caps.A | .B);\n\
       put_number(cast(int) yes);\n\
       put_number(cast(int) both);\n\
     }\n",
    "1\n3\n",
  );
}

#[test]
fn a_constant_that_holds_a_procedure_is_built_where_it_is_used() {
  assert_output(
    "Handler :: struct { proc: (int) -> int; tag: int; }\n\
     double :: (x: int) -> int { return x * 2; }\n\
     DOUBLER :: Handler.{double, 7};\n\
     main :: () {\n\
       h := DOUBLER;\n\
       put_number(h.proc(21));\n\
       put_number(h.tag);\n\
     }\n",
    "42\n7\n",
  );
}

#[test]
fn an_autocast_takes_the_type_that_asked_for_the_value() {
  assert_output(
    "Box :: struct { value: int; }\n\
     unwrap :: (b: *Box) -> int { return b.value; }\n\
     main :: () {\n\
       b: Box;\n\
       b.value = 42;\n\
       raw: *void = *b;\n\
       boxed := ifx raw then xx raw else cast(*Box) null;\n\
       put_number(unwrap(boxed));\n\
       n: u8 = xx 7;\n\
       put_number(cast(int) n);\n\
     }\n",
    "42\n7\n",
  );
}

#[test]
fn a_compound_assignment_reads_its_right_operand_as_the_place() {
  assert_output(
    "Flags :: enum_flags u32 { A; B; C; }\n\
     main :: () {\n\
       f := Flags.A | .B;\n\
       f &= ~.A;\n\
       put_number(cast(int) f);\n\
     }\n",
    "2\n",
  );
}

#[test]
fn compare_and_swap_is_an_atomic_exchange() {
  assert_output(
    "compare_and_swap :: (pointer: *$T, old: T, new: T) -> (success: bool, old_value: T) #intrinsic;\n\
     main :: () {\n\
       lock: int = 0;\n\
       ok, before := compare_and_swap(*lock, 0, 1);\n\
       put_number(cast(int) ok);\n\
       put_number(before);\n\
       put_number(lock);\n\
       put_number(cast(int) compare_and_swap(*lock, 0, 1));\n\
     }\n",
    "1\n0\n1\n0\n",
  );
}

#[test]
fn a_pointer_argument_is_dereferenced_into_the_member_it_marks_as() {
  assert_output(
    "Base :: struct { tag: int; }\n\
     Derived :: struct { using #as base: Base; extra: int; }\n\
     show :: (b: Base) -> int { return b.tag; }\n\
     main :: () {\n\
       d: Derived;\n\
       d.tag = 42;\n\
       put_number(show(*d));\n\
     }\n",
    "42\n",
  );
}

#[test]
fn a_using_of_a_value_reaches_the_constants_of_its_struct() {
  assert_output(
    "Builder :: struct {\n\
       Buffer :: struct { count: s64; allocated: s64; }\n\
       bytes: [64] u8;\n\
     }\n\
     room :: (using b: *Builder) -> int {\n\
       return bytes.count - size_of(Buffer);\n\
     }\n\
     main :: () {\n\
       b: Builder;\n\
       put_number(room(*b));\n\
     }\n",
    "48\n",
  );
}

#[test]
fn a_baked_value_reaches_the_next_bake_through_the_instantiation() {
  assert_output(
    "fill :: (destination: *u8, count: int, $character: u8) {\n\
       i := 0;\n\
       while i < count { destination[i] = character; i += 1; }\n\
     }\n\
     banner :: (destination: *u8, count: int, $character: u8) {\n\
       fill(destination, count, character);\n\
     }\n\
     main :: () {\n\
       out: [4] u8;\n\
       banner(out.data, 3, #char \"x\");\n\
       out[3] = #char \"\\n\";\n\
       write(1, out.data, 4);\n\
     }\n",
    "xxx\n",
  );
}

#[test]
fn a_bitwise_operator_takes_a_pointer_without_a_cast() {
  assert_output(
    "main :: () {\n\
       values: [8] int;\n\
       p := values.data;\n\
       masked := p & ~7;\n\
       put_number(cast(int)(masked == p));\n\
     }\n",
    "1\n",
  );
}

#[test]
fn a_polymorphic_struct_lays_its_members_out_per_instantiation() {
  assert_output(
    "Holder :: struct ($T: Type, $N: s64) { array: [N] T; }\n\
     sum :: (h: Holder(int, 4)) -> int {\n\
       s := 0;\n\
       for h.array  s += it;\n\
       return s;\n\
     }\n\
     main :: () {\n\
       a: Holder(int, 4);\n\
       a.array[0] = 1;\n\
       a.array[3] = 41;\n\
       b: Holder(N = 4, T = int);\n\
       b = a;\n\
       put_number(sum(b));\n\
       put_number(size_of(Holder(float64, 2)));\n\
     }\n",
    "42\n16\n",
  );
}

#[test]
fn a_for_expansion_iterates_a_container_of_its_own() {
  assert_output(
    "Holder :: struct (count: s64, T: Type) {\n\
       occupied: [count] bool;\n\
       values:   [count] T;\n\
     }\n\
     for_expansion :: (holder: Holder, body: Code, flags: For_Flags) #expand {\n\
       for ok, slot: holder.occupied {\n\
         if !ok continue;\n\
         `it_index := slot;\n\
         `it := holder.values[slot];\n\
         #insert body;\n\
       }\n\
     }\n\
     main :: () {\n\
       h: Holder(8, int);\n\
       h.occupied[2] = true;  h.values[2] = 42;\n\
       h.occupied[5] = true;  h.values[5] = 7;\n\
       for h { put_number(it); put_number(it_index); }\n\
       for v, n: h  put_number(v * 100 + n);\n\
     }\n",
    "42\n2\n7\n5\n4202\n705\n",
  );
}

#[test]
fn a_named_for_expansion_is_chosen_over_the_default() {
  assert_output(
    "Bag :: struct { values: [4] int; }\n\
     for_expansion :: (bag: *Bag, body: Code, flags: For_Flags) #expand {\n\
       for v, i: bag.values {\n\
         `it := v;\n\
         `it_index := i;\n\
         #insert body;\n\
       }\n\
     }\n\
     positive :: (bag: *Bag, body: Code, flags: For_Flags) #expand {\n\
       for v, i: bag.values {\n\
         if v <= 0  continue;\n\
         `it := v;\n\
         `it_index := i;\n\
         #insert body;\n\
       }\n\
     }\n\
     main :: () {\n\
       b: Bag;\n\
       b.values[1] = -3;\n\
       b.values[2] = 9;\n\
       for b  put_number(it);\n\
       put(\"--\\n\");\n\
       for :positive b  put_number(it);\n\
     }\n",
    "0\n-3\n9\n0\n--\n9\n",
  );
}

#[test]
fn a_pointer_to_a_struct_is_indexed_when_nothing_overloads_the_subscript() {
  assert_output(
    "Point :: struct { x: int; y: int; }\n\
     main :: () {\n\
       points: [3] Point;\n\
       points[1].x = 11;\n\
       p := points.data;\n\
       put_number(p[1].x);\n\
     }\n",
    "11\n",
  );
}

#[test]
fn a_quick_lambda_is_a_polymorphic_procedure() {
  assert_output(
    "square :: x => x * x;\n\
     apply :: (f: (int) -> int, v: int) -> int { return f(v); }\n\
     main :: () {\n\
       put_number(square(3));\n\
       put_number(apply(square, 5));\n\
       put_number(apply(x => x + 1, 41));\n\
     }\n",
    "9\n25\n42\n",
  );
}

#[test]
fn a_procedure_held_in_a_member_is_called_through_its_value() {
  assert_output(
    "Handler :: struct { run: (int) -> int; }\n\
     main :: () {\n\
       h: Handler;\n\
       h.run = x => x * 3;\n\
       put_number(h.run(14));\n\
     }\n",
    "42\n",
  );
}

#[test]
fn a_run_produces_a_type_the_program_declares_with() {
  assert_output(
    "pick :: (n: int) -> Type { if n > 0  return s32; return float64; }\n\
     Chosen :: #run pick(1);\n\
     main :: () {\n\
       value: Chosen = 7;\n\
       put_number(size_of(Chosen));\n\
       put_number(cast(int) value);\n\
     }\n",
    "4\n7\n",
  );
}

#[test]
fn a_modify_rejects_a_candidate_and_changes_its_variables() {
  assert_output(
    "widen :: (a: $T) -> T #modify { T = s64; return true; } { return a; }\n\
     only_ints :: (a: $T) -> int\n\
     #modify {\n\
       ti := cast(*Type_Info) T;\n\
       if ti.type == .INTEGER  return true;\n\
       return false, \"not an integer\";\n\
     } { return cast(int) a; }\n\
     only_ints :: (a: float64) -> int { return 999; }\n\
     main :: () {\n\
       small: s8 = 7;\n\
       put_number(widen(small));\n\
       put_number(size_of(type_of(widen(small))));\n\
       put_number(only_ints(cast(s16) 5));\n\
       put_number(only_ints(1.5));\n\
     }\n",
    "7\n8\n5\n999\n",
  );
}

#[test]
fn a_named_return_left_out_takes_its_default() {
  assert_output(
    "split :: (n: int) -> int, ok: bool = true {\n\
       if n < 0  return 0, false;\n\
       return n;\n\
     }\n\
     main :: () {\n\
       a, good := split(7);\n\
       b, bad := split(-1);\n\
       put_number(a + cast(int) good);\n\
       put_number(b + cast(int) bad);\n\
     }\n",
    "8\n0\n",
  );
}

#[test]
fn caller_code_reaches_a_macros_body_as_a_default() {
  assert_output(
    "twice :: (v: int, call := #caller_code) -> int #expand { return v * 2; }\n\
     main :: () { put_number(twice(21)); }\n",
    "42\n",
  );
}

#[test]
fn bake_arguments_gives_a_procedure_some_of_its_arguments() {
  assert_output(
    "scaled :: (value: int, factor: int, offset: int) -> int { return value * factor + offset; }\n\
     doubled :: #bake_arguments scaled(factor = 2);\n\
     main :: () {\n\
       put_number(doubled(20, 2));\n\
       put_number(scaled(20, 2, 2));\n\
     }\n",
    "42\n42\n",
  );
}

#[test]
fn bake_constants_binds_a_polymorph_variable() {
  assert_output(
    "twice :: (x: $T) -> T { return x + x; }\n\
     twice_int :: #bake_constants twice(T = int);\n\
     main :: () { put_number(twice_int(21)); }\n",
    "42\n",
  );
}

#[test]
fn an_inserted_string_declares_names_in_the_block_around_it() {
  assert_output(
    "main :: () {\n\
       #insert \"a := 20; b := 22;\";\n\
       put_number(a + b);\n\
     }\n",
    "42\n",
  );
}

#[test]
fn an_inserted_string_can_be_produced_by_a_run() {
  assert_output(
    "gen :: () -> string { return \"answer := 42;\"; }\n\
     main :: () {\n\
       #insert #run gen();\n\
       put_number(answer);\n\
     }\n",
    "42\n",
  );
}

#[test]
fn an_inserted_string_generates_struct_members_and_enum_values() {
  assert_output(
    "Pair :: struct { #insert \"left: int; right: int;\"; }\n\
     Color :: enum { #insert \"RED; GREEN; BLUE;\"; }\n\
     main :: () {\n\
       p: Pair;\n\
       p.left = 20;\n\
       p.right = 22;\n\
       put_number(p.left + p.right);\n\
       put_number(cast(int) Color.BLUE);\n\
     }\n",
    "42\n2\n",
  );
}

#[test]
fn an_inserted_string_declares_names_at_file_scope() {
  assert_output(
    "#insert \"TWENTY :: 20; TWENTY_TWO :: 22;\";\n\
     main :: () { put_number(TWENTY + TWENTY_TWO); }\n",
    "42\n",
  );
}

#[test]
fn an_inserted_string_can_stand_where_an_expression_goes() {
  assert_output(
    "twice :: (n: int) -> int { return n + n; }\n\
     call :: \"twice(#insert argument);\";\n\
     argument :: \"21;\";\n\
     main :: () { put_number(#insert call); }\n",
    "42\n",
  );
}

// ------------------------------------------ inline assembly and FFI -----

#[test]
fn an_asm_block_computes_with_the_registers_it_names() {
  assert_output(
    "main :: () {\n\
       count := 10;\n\
       #asm {\n\
         mov apple:, 10;\n\
         banana: gpr;\n\
         mov.64 banana, 17;\n\
         sub banana, apple;\n\
         add count, banana;\n\
       }\n\
       put_number(count);\n\
     }\n",
    "17\n",
  );
}

#[test]
fn an_asm_register_outlives_the_block_that_declared_it() {
  assert_output(
    "main :: () {\n\
       #asm { mov a:, 12; mov b:, 18; }\n\
       #asm { add a, b; }\n\
       c: s64 = ---;\n\
       #asm { mov c, a; }\n\
       put_number(c);\n\
     }\n",
    "30\n",
  );
}

#[test]
fn an_asm_block_reaches_memory_through_a_base_and_an_index() {
  assert_output(
    "main :: () {\n\
       values: [4] s64;\n\
       for i: 0..3  values[i] = (i + 1) * 10;\n\
       base := values.data;\n\
       index := 2;\n\
       total: s64 = ---;\n\
       #asm {\n\
         mov total, [base];\n\
         add total, [base + 8];\n\
         add total, [base + index*8];\n\
       }\n\
       put_number(total);\n\
     }\n",
    "60\n",
  );
}

#[test]
fn a_syscall_block_reaches_the_kernel() {
  assert_output(
    "write_directly :: (text: string) -> s64 {\n\
       result: s64 = ---;\n\
       fd: s32 = 1;\n\
       data := text.data;\n\
       count := text.count;\n\
       #asm SYSCALL_SYSRET {\n\
         mov.q rcx: gpr === c,  0;\n\
         mov.q r11: gpr === 11, 0;\n\
         mov.q rax: gpr === a,  1;\n\
         mov.d rdi: gpr === di, fd;\n\
         mov.q rsi: gpr === si, data;\n\
         mov.q rdx: gpr === d,  count;\n\
         syscall rcx, r11, rax, rdi, rsi, rdx;\n\
         mov.q result, rax;\n\
       }\n\
       return result;\n\
     }\n\
     main :: () { put_number(write_directly(\"kernel\\n\")); }\n",
    "kernel\n7\n",
  );
}

#[test]
fn an_asm_block_runs_at_compile_time_too() {
  assert_output(
    "add_them :: (a: int, b: int) -> int {\n\
       x := a;\n\
       #asm { add x, b; }\n\
       return x;\n\
     }\n\
     TOTAL :: #run add_them(19, 23);\n\
     main :: () { put_number(TOTAL); }\n",
    "42\n",
  );
}

#[test]
fn the_standard_print_reaches_the_terminal() {
  assert_output(
    "Basic :: #import \"Basic\";\n\
     main :: () {\n\
       Basic.print(\"% and %\\n\", 42, \"a string\");\n\
       Basic.print(\"%\\n\", 37.0);\n\
     }\n",
    "42 and a string\n37\n",
  );
}

#[test]
fn a_compound_declaration_spreads_its_values_across_its_names() {
  assert_output(
    "main :: () {\n\
       a := 1; b := 2; c := 3; d := 4;\n\
       e, f, g, h := a, b, c, d;\n\
       put_number(e + f * 10 + g * 100 + h * 1000);\n\
       a, b += 10;\n\
       put_number(a + b);\n\
       x, y := 7;\n\
       put_number(x + y);\n\
     }\n",
    "4321\n23\n14\n",
  );
}

/// A C library the test compiles with the platform's own C driver, so that
/// what the classification produces is checked against what a C compiler
/// expects rather than against itself (**L§7.11**).
fn with_c_library(source: &str, body: &str) -> Option<Built> {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return None;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let c_path = directory.path().join("cabi.c");
  std::fs::write(&c_path, source).expect("the C source should be writable");
  let library = directory.path().join("libcabi.so");
  let compiled = Command::new(oj_link::driver())
    .arg("-shared")
    .arg("-fPIC")
    .arg("-o")
    .arg(&library)
    .arg(&c_path)
    .status();
  match compiled {
    Ok(status) if status.success() => {}
    _ => {
      eprintln!("skipping: the C driver could not build the test library");
      return None;
    }
  }

  let path = directory.path().join("program.jai");
  std::fs::write(&path, format!("{PRELUDE}\n{body}")).expect("the input should be writable");
  // SAFETY: as in `build_and_run`.
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let report = oj_driver::run(
    &path,
    &oj_driver::BuildOptions::new(),
    oj_driver::Stage::Executable,
    None,
  );
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  Some(Built {
    output: String::from_utf8_lossy(&output.stdout).into_owned(),
    status: output.status.code().unwrap_or(-1),
  })
}

const C_ABI_LIBRARY: &str = r#"
#include <stdint.h>
struct Pair  { int64_t a; int64_t b; };
struct Mixed { double x; int64_t n; };
struct Two   { float u; float v; };
struct Big   { int64_t a, b, c; };

int64_t take_pair(struct Pair p)   { return p.a * 10 + p.b; }
int64_t take_mixed(struct Mixed m) { return (int64_t)(m.x * 2.0) + m.n; }
int64_t take_two(struct Two t)     { return (int64_t)(t.u * 100.0f + t.v); }
int64_t take_big(struct Big b)     { return b.a + b.b + b.c; }
struct Pair  make_pair(int64_t a, int64_t b) { struct Pair p = {a, b}; return p; }
struct Mixed make_mixed(double x, int64_t n) { struct Mixed m = {x, n}; return m; }
struct Big   make_big(int64_t a)             { struct Big b = {a, a + 1, a + 2}; return b; }
"#;

#[test]
fn a_run_calls_into_a_library_whose_file_the_compiler_named() {
  // The compile-time loader and the link line have to agree about what a
  // library is called (**L§12.2**). A library the compiler built is
  // `<name>.so`, which is not the `lib<name>.so` a loader would guess — so a
  // `#run` calling into one used to link and then fail to resolve.
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let c_path = directory.path().join("answer.c");
  std::fs::write(&c_path, "int answer(void) { return 42; }\n").expect("the C source is writable");
  // `answer.so`, the way the compiler names a library of its own — not
  // `libanswer.so`.
  let built = Command::new(oj_link::driver())
    .arg("-shared")
    .arg("-fPIC")
    .arg("-o")
    .arg(directory.path().join("answer.so"))
    .arg(&c_path)
    .status();
  if !built.is_ok_and(|status| status.success()) {
    eprintln!("skipping: the C driver could not build the test library");
    return;
  }

  let path = directory.path().join("program.jai");
  std::fs::write(
    &path,
    format!(
      "{PRELUDE}\n\
       answer_lib :: #library \"answer\";\n\
       answer :: () -> s32 #foreign answer_lib;\n\
       AT_COMPILE_TIME :: #run answer();\n\
       main :: () {{ put_number(AT_COMPILE_TIME); put_number(answer()); }}\n"
    ),
  )
  .expect("the input should be writable");

  unsafe {
    oj_testsupport::use_own_modules();
  }
  let report = oj_driver::run(
    &path,
    &oj_driver::BuildOptions::new(),
    oj_driver::Stage::Executable,
    None,
  );
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  // Once from the `#run`, once from the program itself.
  assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n42\n");
}

#[test]
fn a_foreign_procedure_takes_and_returns_a_struct_by_value() {
  let Some(built) = with_c_library(
    C_ABI_LIBRARY,
    "cabi :: #library \"cabi\";\n\
     Pair  :: struct { a: s64; b: s64; }\n\
     Mixed :: struct { x: float64; n: s64; }\n\
     Two   :: struct { u: float32; v: float32; }\n\
     Big   :: struct { a: s64; b: s64; c: s64; }\n\
     take_pair  :: (p: Pair) -> s64 #foreign cabi;\n\
     take_mixed :: (m: Mixed) -> s64 #foreign cabi;\n\
     take_two   :: (t: Two) -> s64 #foreign cabi;\n\
     take_big   :: (b: Big) -> s64 #foreign cabi;\n\
     make_pair  :: (a: s64, b: s64) -> Pair #foreign cabi;\n\
     make_mixed :: (x: float64, n: s64) -> Mixed #foreign cabi;\n\
     make_big   :: (a: s64) -> Big #foreign cabi;\n\
     main :: () {\n\
       p: Pair;  p.a = 4; p.b = 2;\n\
       put_number(take_pair(p));\n\
       m: Mixed; m.x = 1.5; m.n = 39;\n\
       put_number(take_mixed(m));\n\
       t: Two;   t.u = 4.0; t.v = 2.0;\n\
       put_number(take_two(t));\n\
       b: Big;   b.a = 20; b.b = 21; b.c = 1;\n\
       put_number(take_big(b));\n\
       q := make_pair(7, 5);\n\
       put_number(q.a * 10 + q.b);\n\
       n := make_mixed(0.5, 41);\n\
       put_number(cast(s64)(n.x * 2.0) + n.n);\n\
       g := make_big(10);\n\
       put_number(g.a + g.b + g.c);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.status, 0, "the program should exit cleanly");
  assert_eq!(built.output, "42\n42\n402\n42\n75\n42\n33\n");
}

#[test]
fn a_c_call_procedure_is_a_callback_a_library_can_call() {
  assert_output(
    "qsort :: (base: *void, count: u64, size: u64, compare: (*void, *void) -> s32 #c_call)\n\
       #foreign libc;\n\
     compare_ints :: (a: *void, b: *void) -> s32 #c_call {\n\
       x := (cast(*s32) a).*;\n\
       y := (cast(*s32) b).*;\n\
       if x < y  return -1;\n\
       if x > y  return 1;\n\
       return 0;\n\
     }\n\
     main :: () {\n\
       values: [5] s32;\n\
       values[0] = 5; values[1] = 3; values[2] = 9; values[3] = 1; values[4] = 7;\n\
       qsort(values.data, 5, size_of(s32), compare_ints);\n\
       for values  put_number(cast(int) it);\n\
     }\n",
    "1\n3\n5\n7\n9\n",
  );
}

#[test]
fn threads_run_the_procedure_they_were_given() {
  assert_output(
    "Thread :: #import \"Thread\";\n\
     Atomics :: #import \"Atomics\";\n\
     counter: s64 = 0;\n\
     worker :: (thread: *Thread.Thread) -> s64 {\n\
       for 1..1000  Atomics.atomic_add(*counter, 1);\n\
       return 0;\n\
     }\n\
     main :: () {\n\
       threads: [4] Thread.Thread;\n\
       for * threads  Thread.thread_init(it, worker);\n\
       for * threads  Thread.thread_start(it);\n\
       for * threads  while !Thread.thread_is_done(it) {}\n\
       put_number(counter);\n\
     }\n",
    "4000\n",
  );
}

#[test]
fn a_constant_that_names_a_polymorphic_procedure_is_that_procedure() {
  assert_output(
    "Basic :: #import \"Basic\";\n\
     assert :: Basic.assert;\n\
     double :: (x: $T) -> T { return x + x; }\n\
     twice :: double;\n\
     main :: () {\n\
       assert(twice(21) == 42);\n\
       put_number(twice(21));\n\
     }\n",
    "42\n",
  );
}

#[test]
fn a_string_literal_reaches_a_c_procedure_as_a_c_string() {
  assert_output(
    "puts :: (s: *u8) -> s32 #foreign libc;\n\
     main :: () { puts(\"through libc\"); }\n",
    "through libc\n",
  );
}

#[test]
fn a_vector_register_survives_between_two_asm_blocks() {
  assert_output(
    "String :: #import \"String\";\n\
     main :: () {\n\
       s := \"hello, world and a longer tail to reach the simd path\";\n\
       put_number(String.find_index_from_left(s, #char \",\"));\n\
       put_number(String.find_index_from_left(s, #char \"z\"));\n\
     }\n",
    "5\n-1\n",
  );
}

#[test]
fn the_file_module_writes_and_reads_a_file() {
  assert_output(
    "File :: #import \"File\";\n\
     main :: () {\n\
       path := \"oj_file_module_test.txt\";\n\
       if !File.write_entire_file(path, \"written\\n\")  return;\n\
       text, ok := File.read_entire_file(path);\n\
       if !ok  return;\n\
       put(text);\n\
       File.file_delete(path);\n\
     }\n",
    "written\n",
  );
}

#[test]
fn a_macro_takes_the_registers_it_is_given() {
  assert_output(
    "reg :: __reg;\n\
     add_the_two_regs :: (left: reg, right: reg) #expand {\n\
       #asm { add left, right; }\n\
     }\n\
     main :: () {\n\
       #asm { mov a:, 12; mov b:, 18; }\n\
       add_the_two_regs(b, a);\n\
       c: s64 = ---;\n\
       #asm { mov c, b; }\n\
       put_number(c);\n\
     }\n",
    "30\n",
  );
}

#[test]
fn an_asm_block_gathers_through_a_vector_index() {
  assert_output(
    "Machine :: #import \"Machine_X64\";\n\
     main :: () {\n\
       info := Machine.get_cpu_info();\n\
       if !Machine.check_feature(info.feature_leaves, Machine.x86_Feature_Flag.AVX2) {\n\
         put_number(8);\n\
         return;\n\
       }\n\
       source := float.[1, 2, 3, 4, 5, 6, 7, 8];\n\
       indices := u32.[7, 6, 5, 4, 3, 2, 1, 0];\n\
       gathered: [8] float;\n\
       source_pointer := source.data;\n\
       index_pointer := indices.data;\n\
       gathered_pointer := gathered.data;\n\
       #asm AVX, AVX2 {\n\
         movdqu vindex:, [index_pointer];\n\
         pcmpeqd gather_mask:, gather_mask, gather_mask;\n\
         gatherdps gather_dest:, [source_pointer + vindex*4], gather_mask;\n\
         movdqu [gathered_pointer], gather_dest;\n\
       }\n\
       put_number(cast(int) gathered[0]);\n\
     }\n",
    "8\n",
  );
}

#[test]
fn a_constant_array_carries_its_count_into_a_view() {
  assert_output(
    "COUNTS :: int.[1, 2, 3, 4];\n\
     total :: (values: [] int) -> int {\n\
       sum := 0;\n\
       for values  sum += it;\n\
       return sum;\n\
     }\n\
     main :: () { put_number(total(COUNTS)); }\n",
    "10\n",
  );
}

#[test]
fn a_case_over_strings_compares_their_bytes() {
  assert_output(
    "classify :: (word: string) {\n\
       if word == {\n\
         case \"debug\";\n\
           put(\"a debug build\\n\");\n\
         case \"release\";\n\
           put(\"a release build\\n\");\n\
         case;\n\
           put(\"something else\\n\");\n\
       }\n\
     }\n\
     main :: () {\n\
       classify(\"debug\");\n\
       classify(\"release\");\n\
       classify(\"debugg\");\n\
       classify(\"\");\n\
     }\n",
    "a debug build\na release build\nsomething else\nsomething else\n",
  );
}

#[test]
fn the_type_table_is_reachable_through_runtime_info() {
  // `get_runtime_info` is declared `#compiler` but has a body that reads
  // `__runtime_info: Runtime_Info #elsewhere` (**C§3.3**); the compiler
  // defines that symbol as the head of the type table image, so walking the
  // table is what the program does rather than what the compiler answers.
  assert_output(
    "Runtime_Info :: struct {\n\
       type_table: [] *Type_Info;\n\
       global_data_info: *void;\n\
     }\n\
     get_runtime_info :: () -> Runtime_Info {\n  \
       __runtime_info: Runtime_Info #elsewhere;\n  \
       return __runtime_info;\n\
     }\n\
     Point :: struct { x: float; y: float; }\n\
     main :: () {\n  \
       wanted := type_info(Point);\n  \
       table := get_runtime_info().type_table;\n  \
       found := 0;\n  \
       for table  if it == wanted  found += 1;\n  \
       put_number(found);\n  \
       put_number(cast(int) (table.count > 0));\n\
     }\n",
    "1\n1\n",
  );
}

#[test]
fn add_global_data_puts_its_bytes_in_the_executable() {
  // `image :: #run add_global_data(…)` is the canonical use (**C§3.3**): the
  // run hands back a slice of storage the compiler owns, and the pointer among
  // those bytes has to name data the *executable* carries. The bytes travel
  // with the slice and are laid down beside it.
  assert_output(
    "Data_Segment_Index :: enum u16 {\n  \
       WRITABLE :: 0x0; WRITABLE_NO_RESET :: 0x1; READ_ONLY :: 0x2; BSS :: 0x3;\n  \
       USER_SEGMENT :: 0x10;\n\
     }\n\
     Data_Segment :: struct {}\n\
     add_global_data :: (data: [] u8, segment: Data_Segment_Index, user_segment: *Data_Segment = null, w: s64 = -1) -> [] u8 #compiler;\n\
     make :: () -> [] u8 {\n  \
       bytes: [4] u8;\n  \
       bytes[0] = 7; bytes[1] = 8; bytes[2] = 9; bytes[3] = 10;\n  \
       return add_global_data(bytes, .READ_ONLY);\n\
     }\n\
     DATA :: #run make();\n\
     main :: () {\n  \
       put_number(DATA.count);\n  \
       for DATA  put_number(it);\n\
     }\n",
    "4\n7\n8\n9\n10\n",
  );
}

#[test]
fn add_data_segment_reports_that_no_segment_of_its_own_is_made() {
  // orangejuice puts what a metaprogram asks for with the rest of the
  // program's data rather than in a section of its own, which is what the
  // second return value is for (**C§3.3**).
  assert_output(
    "Data_Segment :: struct {}\n\
     Data_Segment_Characteristics :: enum_flags u32 { READ :: 0x1; WRITE :: 0x2; }\n\
     add_data_segment :: (section_name: string, characteristics := Data_Segment_Characteristics.READ | .WRITE, alignment: s32 = 16, w: s64 = -1) -> (segment: *Data_Segment, actual_segment_will_be_created: bool) #compiler;\n\
     ask :: () -> int {\n  \
       segment, created := add_data_segment(\"mine\");\n  \
       if segment == null  return 0;\n  \
       if created          return 1;\n  \
       return 2;\n\
     }\n\
     ASKED :: #run ask();\n\
     main :: () { put_number(ASKED); }\n",
    "2\n",
  );
}

/// The same, with one more file beside the program for it to `#load`.
fn assert_output_with_file(name: &str, contents: &str, body: &str, expected: &str) {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  std::fs::write(directory.path().join(name), contents).expect("the input should be writable");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, format!("{PRELUDE}\n{body}")).expect("the input should be writable");
  // SAFETY: as `build_and_run`.
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let options = oj_driver::BuildOptions::new();
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn a_baked_argument_is_read_where_the_bake_was_written() {
  // The default a `#bake_arguments` gives a `$`-marked parameter is a node of
  // the file the *bake* is in, not of the header's (**L§7.10**); reading it
  // out of the header's own AST binds whatever node sits at that index.
  assert_output_with_file(
    "chooser.jai",
    "choose :: ($T: Type, $doubled := false) -> T {\n  \
       #if doubled  return 2;\n  \
       return 1;\n\
     }\n",
    "#load \"chooser.jai\";\n\
     Twice :: #bake_arguments choose(doubled = true);\n\
     main :: () {\n  \
       put_number(choose(int));\n  \
       put_number(Twice(int));\n\
     }\n",
    "1\n2\n",
  );
}

#[test]
fn a_using_of_a_type_named_through_a_dot_brings_in_its_members() {
  assert_output(
    "Outer :: struct {\n  \
       Kind :: enum u8 { A; B; }\n\
     }\n\
     which :: (k: Outer.Kind) -> int {\n  \
       using Outer.Kind;\n  \
       if k == A  return 10;\n  \
       return 20;\n\
     }\n\
     main :: () { put_number(which(.B)); }\n",
    "20\n",
  );
}

#[test]
fn a_parameter_written_as_a_polymorphic_struct_family_takes_any_instantiation() {
  assert_output(
    "Boxed :: struct (T: Type) { value: T; }\n\
     unwrap :: (box: Boxed) -> int { return cast(int) box.value; }\n\
     main :: () {\n  \
       b: Boxed(s32);\n  \
       b.value = 7;\n  \
       put_number(unwrap(b));\n\
     }\n",
    "7\n",
  );
}

#[test]
fn a_foreign_symbol_is_not_taken_by_an_overload_that_can_be_renamed() {
  // Jai has no mangling, so two overloads of one name compete for one symbol
  // (**C§11**); the `#foreign` one cannot be renamed, since the name is what
  // it binds to.
  assert_output(
    "strlen :: (s: *u8) -> u64 #foreign libc;\n\
     strlen :: (a: int, b: int) -> int { return a + b; }\n\
     main :: () {\n  \
       put_number(strlen(19, 23));\n  \
       put_number(cast(int) strlen(\"abcd\\0\".data));\n\
     }\n",
    "42\n4\n",
  );
}

#[test]
fn a_modify_sees_an_untyped_constant_as_the_type_its_header_declared() {
  // A call site that writes `.B` hands over an *untyped* enum constant; inside
  // the `#modify` the variable is a value of the type the header declared, so
  // comparing it to another member of that enum is what it looks like
  // (**L§7.8**).
  assert_output(
    "Kind :: enum u8 { A; B; C; }\n\
     pick :: ($k: Kind = .A) -> int #modify {\n  \
       if k == .B  k = .C;\n  \
       return true;\n\
     } {\n  \
       #if k == .C  return 3;\n  \
       return 1;\n\
     }\n\
     main :: () { put_number(pick(.B)); }\n",
    "3\n",
  );
}

#[test]
fn a_resizable_arrays_allocated_and_allocator_are_fields_of_its_own() {
  // `[..] T` is `Resizable_Array`: `.count` and `.data` first, then the
  // `.allocated` capacity and the `.allocator` that grew it (Preload).
  assert_output(
    "#import \"Basic\";\n\
     main :: () {\n  \
       xs: [..] int;\n  \
       array_add(*xs, 1);\n  \
       array_add(*xs, 2);\n  \
       put_number(xs.count);\n  \
       put_number(xs.allocated);\n  \
       ys: [..] int;\n  \
       ys.allocator = temp;\n  \
       array_add(*ys, 3);\n  \
       put_number(ys.count);\n\
     }\n",
    "2\n8\n1\n",
  );
}

#[test]
fn a_struct_body_sets_a_default_through_a_member_path() {
  // `member.field = value;` after the member's declaration is a default one
  // level down, applied after the member's own type has taken its (**L§8.1**).
  assert_output(
    "Inner :: struct { a := 1; b := 2; }\n\
     Outer :: struct {\n  \
       x: Inner;\n  \
       x.a = 42;\n  \
       y := 7;\n\
     }\n\
     main :: () {\n  \
       o: Outer;\n  \
       put_number(o.x.a);\n  \
       put_number(o.x.b);\n  \
       put_number(o.y);\n\
     }\n",
    "42\n2\n7\n",
  );
}

#[test]
fn a_using_of_a_named_import_widens_the_scope_with_the_modules_names() {
  // `String` is written `Basic :: #import "Basic"; using Basic;`, which is a
  // `using` of a name rather than of the `#import` itself (**L§11.2**).
  assert_output(
    "Basic :: #import \"Basic\";\n\
     using Basic;\n\
     main :: () {\n  \
       s := tprint(\"%-%\", 6, 7);\n  \
       put(s);\n  \
       put(\"\\n\");\n\
     }\n",
    "6-7\n",
  );
}

#[test]
fn a_using_of_a_named_import_written_before_the_import_still_widens_the_scope() {
  assert_output(
    "using Basic;\n\
     Basic :: #import \"Basic\";\n\
     main :: () { put(tprint(\"%\\n\", 42)); }\n",
    "42\n",
  );
}

#[test]
fn an_unqualified_enum_member_takes_the_type_the_operator_it_is_in_was_asked_for() {
  // `.WEST | .EAST` has no type of its own: what the whole expression was
  // asked for is what says which enum the names belong to (**L§5.12**).
  assert_output(
    "Dir :: enum_flags { EAST; WEST; NORTH; }\n\
     Stuff :: enum { FIRST; SECOND; THIRD; }\n\
     main :: () {\n  \
       d: Dir;\n  \
       d = .WEST | .EAST;\n  \
       put_number(cast(int) d);\n  \
       if d & (.WEST | .NORTH)  put(\"masked\\n\");\n  \
       y: Stuff = cast(Stuff) .THIRD;\n  \
       put_number(cast(int) y);\n\
     }\n",
    "3\nmasked\n2\n",
  );
}

#[test]
fn a_struct_operand_picks_the_operator_it_overloads() {
  // `operator +` and friends are constants named by the operator's own text,
  // resolved like any other overload set (**L§7.7**).
  assert_output(
    "Pair :: struct { a: int; b: int; }\n\
     operator + :: (x: Pair, y: Pair) -> Pair { return .{x.a + y.a, x.b + y.b}; }\n\
     operator * :: (x: Pair, s: int) -> Pair #symmetric { return .{x.a * s, x.b * s}; }\n\
     operator - :: (x: Pair) -> Pair { return .{-x.a, -x.b}; }\n\
     operator == :: (x: Pair, y: Pair) -> bool { return x.a == y.a && x.b == y.b; }\n\
     main :: () {\n  \
       p := Pair.{1, 2};\n  \
       q := Pair.{10, 20};\n  \
       r := p + q;\n  \
       put_number(r.a);\n  \
       put_number(r.b);\n  \
       s := 3 * p;\n  \
       put_number(s.a);\n  \
       n := -p;\n  \
       put_number(n.b);\n  \
       if p == Pair.{1, 2}  put(\"equal\\n\");\n  \
       if p != q  put(\"different\\n\");\n\
     }\n",
    "11\n22\n3\n-2\nequal\ndifferent\n",
  );
}

#[test]
fn a_compound_assignment_takes_its_own_operator_or_falls_back_to_the_plain_one() {
  // `a op= b` uses `operator op=` when there is one, and otherwise means
  // `a = a op b` (**L§7.7**).
  assert_output(
    "Pair :: struct { a: int; b: int; }\n\
     operator + :: (x: Pair, y: Pair) -> Pair { return .{x.a + y.a, x.b + y.b}; }\n\
     operator -= :: (x: *Pair, y: Pair) { x.a -= y.a; x.b -= y.b; }\n\
     main :: () {\n  \
       p := Pair.{1, 2};\n  \
       p += Pair.{10, 20};\n  \
       put_number(p.a);\n  \
       p -= Pair.{1, 1};\n  \
       put_number(p.a);\n  \
       put_number(p.b);\n\
     }\n",
    "11\n10\n21\n",
  );
}

#[test]
fn a_subscript_operator_reads_and_writes_through_the_address_it_gives() {
  // `operator *[]` gives the element's address, which is enough for a read, a
  // write and a compound assignment at once (**L§7.7**).
  assert_output(
    "Bucket :: struct { items: [10] int; }\n\
     operator *[] :: (b: *Bucket, index: int) -> *int { return *b.items[index]; }\n\
     Wrap :: struct { items: [4] int; }\n\
     operator [] :: (w: Wrap, index: int) -> int { return w.items[index % 4]; }\n\
     main :: () {\n  \
       b: Bucket;\n  \
       b[3] = 7;\n  \
       b[3] *= 6;\n  \
       put_number(b[3]);\n  \
       w: Wrap;\n  \
       w.items[1] = 5;\n  \
       put_number(w[5]);\n\
     }\n",
    "42\n5\n",
  );
}

#[test]
fn a_polymorphic_struct_takes_the_defaults_of_the_arguments_it_was_not_given() {
  // A struct's parameters are constants even though they are written
  // `N: int = 10`, so the ones an instantiation leaves out take their defaults
  // (**L§8.5**). It is what `Table(string, string)` leans on.
  assert_output(
    "Holder :: struct (T: Type = int, N: int = 10) { values: [N] T; }\n\
     main :: () {\n  \
       b: Holder(float);\n  \
       c: Holder(N = 3);\n  \
       d: Holder(float, 5);\n  \
       put_number(b.values.count);\n  \
       put_number(c.values.count);\n  \
       put_number(d.values.count);\n\
     }\n",
    "10\n3\n5\n",
  );
}

#[test]
fn a_nested_type_of_a_baked_struct_is_one_type_however_it_is_reached() {
  // `Entry` inside `Table(K, V)` belongs to that instantiation; a procedure
  // taking the family resolves it there rather than under its own (**L§8.5**).
  assert_output(
    "Box :: struct (T: Type) {\n  \
       Item :: struct { value: T; }\n  \
       items: [] Item;\n\
     }\n\
     fill :: (box: *Box, storage: [] $I) { box.items = storage; }\n\
     main :: () {\n  \
       b: Box(int);\n  \
       storage: [2] Box(int).Item;\n  \
       storage[0].value = 41;\n  \
       storage[1].value = 1;\n  \
       fill(*b, storage);\n  \
       put_number(b.items[0].value + b.items[1].value);\n\
     }\n",
    "42\n",
  );
}

#[test]
fn an_import_in_a_procedure_body_is_not_a_value() {
  // `#import` in an imperative scope brings names into the block; there is
  // nothing in the executable for the statement itself (**L§11.2**).
  assert_output(
    "main :: () {\n  \
       #import \"Basic\";\n  \
       put(tprint(\"%\\n\", 7));\n\
     }\n",
    "7\n",
  );
}

#[test]
fn a_polymorphic_struct_named_without_arguments_bakes_with_their_defaults() {
  // `a: A_Struct;` on a family whose parameters all have defaults is the
  // instantiation those defaults make, and its parameters are reachable
  // through it (**L§8.5**).
  assert_output(
    "A_Struct :: struct (param := 7) { x := 5; y := param; }\n\
     Holder :: struct (T: Type = int, N: int = 10) { array: [N] T; }\n\
     main :: () {\n  \
       a: A_Struct;\n  \
       put_number(a.x);\n  \
       put_number(a.y);\n  \
       put_number(a.param);\n  \
       floats: Holder(float, 5);\n  \
       put_number(floats.N);\n  \
       h: Holder;\n  \
       put_number(h.array.count);\n\
     }\n",
    "5\n7\n7\n5\n10\n",
  );
}

#[test]
fn a_type_variable_is_solved_through_a_procedure_typed_parameter() {
  // `f: (T) -> $S` decides `S` by unifying the argument's own procedure type
  // with the pattern; a part of the pattern with no variable in it decides
  // nothing (**L§7.8**).
  assert_output(
    "call :: (f: (int) -> $S, x: int) -> S { return f(x); }\n\
     both :: (f: ($T) -> $S, x: T) -> S { return f(x); }\n\
     double :: (x: int) -> int { return x + x; }\n\
     main :: () {\n  \
       put_number(call(double, 4));\n  \
       put_number(both(double, 5));\n\
     }\n",
    "8\n10\n",
  );
}

#[test]
fn a_polymorphic_procedure_passed_as_an_argument_decides_no_type_variable() {
  // `square :: (x: $T) -> T` handed to `(f: (x: X) -> X, x: $X)` takes the
  // shape the *other* argument decides (**L§7.8**).
  assert_output(
    "square :: (x: $T) -> T { return x * x; }\n\
     apply :: (f: (x: X) -> X, x: $X, count: int) -> X {\n  \
       for 1..count x = f(x);\n  \
       return x;\n\
     }\n\
     main :: () { put_number(apply(square, 3, 2)); }\n",
    "81\n",
  );
}

#[test]
fn a_cast_to_bool_is_a_truth_test_whatever_it_casts() {
  // `cast(bool) s` on a string is `s.count != 0` (**L§5.9**).
  assert_output(
    "convert :: (arg: $T) -> bool { return cast(bool) arg; }\n\
     main :: () {\n  \
       if convert(\"hi\")  put(\"string true\\n\");\n  \
       if !convert(\"\")   put(\"empty false\\n\");\n  \
       if convert(3)     put(\"int true\\n\");\n  \
       if !convert(0.0)  put(\"float false\\n\");\n\
     }\n",
    "string true\nempty false\nint true\nfloat false\n",
  );
}

#[test]
fn an_optionally_baked_parameter_takes_a_runtime_value_too() {
  // `$$x` bakes when the call site has a constant and stays an ordinary
  // parameter otherwise; a cast of a numeric constant is one (**L§5.11**,
  // **L§7.8**).
  assert_output(
    "tell :: ($$x: $T) -> int { #if is_constant(x)  return 1; return 0; }\n\
     main :: () {\n  \
       put_number(tell(3));\n  \
       put_number(tell(cast(u8) 42.0));\n  \
       y := 5;\n  \
       put_number(tell(y));\n\
     }\n",
    "1\n1\n0\n",
  );
}

#[test]
fn a_labelled_break_leaves_the_loop_its_condition_variable_names() {
  // `while name := expression` names the loop after the variable it declares,
  // and `break name;` leaves that one (**L§6.4**).
  assert_output(
    "main :: () {\n  \
       j := 0;\n  \
       while j_loop := j < 4 {\n    \
         defer j += 1;\n    \
         i := 0;\n    \
         while i_loop := i < 4 {\n      \
           defer i += 1;\n      \
           put_number(i * 10 + j);\n      \
           if i == 1 && j == 1  break j_loop;\n    \
         }\n  \
       }\n  \
       put_number(99);\n\
     }\n",
    "0\n10\n20\n30\n1\n11\n99\n",
  );
}

#[test]
fn a_labelled_continue_names_a_for_by_its_iterator() {
  assert_output(
    "main :: () {\n  \
       total := 0;\n  \
       for outer: 1..3 {\n    \
         for inner: 1..3 {\n      \
           if inner == 2  continue outer;\n      \
           total += outer * inner;\n    \
         }\n  \
       }\n  \
       put_number(total);\n\
     }\n",
    "6\n",
  );
}

#[test]
fn a_reverse_range_loop_marked_v2_counts_down() {
  // `for #v2 < a..b` visits the numbers of `a..b` in reverse (**L§6.5**).
  assert_output(
    "main :: () {\n  \
       for #v2 < 10..15  put_number(it);\n  \
       for #v2 < 15..10  put_number(0);\n\
     }\n",
    "15\n14\n13\n12\n11\n10\n",
  );
}

#[test]
fn remove_takes_the_last_element_into_the_hole_it_leaves() {
  // Unordered removal: the count comes down and the slot is visited again
  // (**L§6.5**).
  assert_output(
    "#import \"Basic\";\n\
     main :: () {\n  \
       numbers: [..] int;\n  \
       for 1..10  array_add(*numbers, it);\n  \
       for numbers  if (it & 1) == 0  remove it;\n  \
       for numbers  put_number(it);\n  \
       put_number(numbers.count);\n\
     }\n",
    "1\n9\n3\n7\n5\n5\n",
  );
}

#[test]
fn an_ifx_with_no_branches_written_takes_the_value_its_condition_tested() {
  // `ifx cond` is worth what the condition tested — one level of boolean
  // operator down — and the branch that was not written is the default value
  // of that type, struct defaults and all (**L§5.13**).
  assert_output(
    "is_odd :: (value: int) -> bool { return cast(bool)(value & 1); }\n\
     Car :: struct { seats := 4; }\n\
     main :: () {\n  \
       x := 5;\n  \
       put_number(ifx x > 3);\n  \
       put_number(ifx is_odd(x));\n  \
       put_number(ifx is_odd(x + 1));\n  \
       car: *Car;\n  \
       taken := ifx car then car.*;\n  \
       put_number(taken.seats);\n\
     }\n",
    "5\n5\n0\n4\n",
  );
}

#[test]
fn a_static_ifx_lowers_only_the_branch_it_chose() {
  // `#ifx` picks at compile time, and each branch is a block whose earlier
  // statements are work the program does (**L§5.13**, **L§6.10**).
  assert_output(
    "main :: () {\n  \
       value := #ifx OS == .WINDOWS {\n    \
         put(\"windows\\n\");\n    \
         1;\n  \
       } else {\n    \
         put(\"elsewhere\\n\");\n    \
         k := 2;\n    \
         k *= 3;\n    \
         k;\n  \
       }\n  \
       put_number(value);\n\
     }\n",
    "elsewhere\n6\n",
  );
}

#[test]
fn a_literal_converts_to_a_variant_of_the_type_it_is_a_literal_of() {
  // `a: Handle = 5` and `"Hello"` passed where a `#type,distinct string` is
  // wanted; a value the program computed still does not (**L§3.11**).
  assert_output(
    "Handle :: #type,distinct u32;\n\
     Filename :: #type,distinct string;\n\
     name :: (n: Filename) -> int { return n.count; }\n\
     main :: () {\n  \
       a: Handle = 5;\n  \
       put_number(cast(int)(3 * a + 2));\n  \
       put_number(name(\"Hello\"));\n\
     }\n",
    "17\n5\n",
  );
}

#[test]
fn a_macros_varargs_slot_is_the_array_rather_than_its_first_argument() {
  // A macro binds a constant argument into the expansion, but the `..T` slot
  // is the array the call site builds and binds nothing (**L§7.3**).
  assert_output(
    "count :: (args: .. int) -> int { return args.count; }\n\
     forward :: (args: .. int) -> int #expand { return count(..args); }\n\
     main :: () {\n  \
       put_number(forward(1, 2, 3));\n  \
       put_number(forward());\n\
     }\n",
    "3\n0\n",
  );
}

#[test]
fn a_positional_argument_skips_a_slot_a_named_one_claimed() {
  // `f(i = 5, s = "x", v = 1, 2, 3)` puts the trailing arguments in `v`,
  // since `s` and `i` are spoken for (**L§7.3**).
  assert_output(
    "gather :: (s: string, i: int, v: .. int) -> int {\n  \
       total := i;\n  \
       for v  total += it;\n  \
       return total;\n\
     }\n\
     main :: () { put_number(gather(i = 5, s = \"x\", v = 1, 2, 3)); }\n",
    "11\n",
  );
}

#[test]
fn a_struct_literal_designates_a_slot_inside_a_member() {
  // `Body.{values[1] = 7}` names storage inside a member rather than a member
  // of its own (**L§5.7**).
  assert_output(
    "Inner :: struct { a: int; b: int; }\n\
     Body :: struct { values: [3] int; inner: Inner; }\n\
     main :: () {\n  \
       body := Body.{values[1] = 7, inner.b = 9};\n  \
       put_number(body.values[0]);\n  \
       put_number(body.values[1]);\n  \
       put_number(body.inner.b);\n\
     }\n",
    "0\n7\n9\n",
  );
}

#[test]
fn a_struct_literal_of_constants_is_a_constant() {
  // (**L§5.11**) — and an undesignated one takes the type the operator it is
  // in was asked for (**L§5.7**).
  assert_output(
    "V3 :: struct { x, y, z: int; }\n\
     operator + :: (a: V3, b: V3) -> V3 { return .{a.x+b.x, a.y+b.y, a.z+b.z}; }\n\
     main :: () {\n  \
       #if is_constant(V3.{1, 2, 3})  put(\"constant\\n\");\n  \
       result: V3;\n  \
       result = .{3, 2, 1} + .{-1, -2, -3};\n  \
       put_number(result.x);\n  \
       put_number(result.z);\n\
     }\n",
    "constant\n2\n-2\n",
  );
}

#[test]
fn a_parameter_takes_its_type_from_another_parameters_instantiation() {
  // `value: holder.T` needs the `Holder(…)` the call passed, which re-reading
  // the header with the bindings in place is what supplies (**L§7.8**).
  assert_output(
    "Holder :: struct (N: int, T: Type) { array: [N] T; }\n\
     add :: (holder: *Holder, index: int, value: holder.T) {\n  \
       holder.array[index] = value;\n\
     }\n\
     main :: () {\n  \
       ints: Holder(5, int);\n  \
       add(*ints, 2, 9);\n  \
       put_number(ints.array[2]);\n  \
       put_number(ints.N);\n\
     }\n",
    "9\n5\n",
  );
}

#[test]
fn procedure_of_call_is_the_specialization_the_call_would_reach() {
  // `#procedure_of_call f(x)` names the procedure rather than calling it, and
  // two calls that solve the same constants name one (**L§7.10**).
  assert_output(
    "square :: (x: $T) -> T { return x * x; }\n\
     main :: () {\n  \
       a: u8 = 5;\n  \
       b: u8 = 11;\n  \
       p1 :: #procedure_of_call square(a);\n  \
       p2 :: #procedure_of_call square(b);\n  \
       #assert p1 == p2;\n  \
       put_number(cast(int) p1(6));\n\
     }\n",
    "36\n",
  );
}

#[test]
fn a_static_if_over_a_type_variable_picks_one_branch_per_specialization() {
  // Two types compare by identity, so `#if T == string` folds inside the
  // instantiation and the branch it rejects is never lowered (**L§7.8**).
  assert_output(
    "kind :: (element: $T) -> int {\n  \
       #if T == string  return element.count;\n  \
       else             return cast(int)(element * element);\n\
     }\n\
     main :: () {\n  \
       put_number(kind(\"abcd\"));\n  \
       put_number(kind(5));\n\
     }\n",
    "4\n25\n",
  );
}

#[test]
fn a_procedure_nested_in_a_polymorphic_body_sees_that_specialization() {
  // There is one of it per instantiation, since the constants it can see are
  // that specialization's (**L§7.8**).
  assert_output(
    "outer :: (element: $T) -> int {\n  \
       inner :: () -> int { #if T == string  return 1; return 2; }\n  \
       return inner();\n\
     }\n\
     main :: () {\n  \
       put_number(outer(\"x\"));\n  \
       put_number(outer(7));\n\
     }\n",
    "1\n2\n",
  );
}

#[test]
fn a_type_restriction_says_what_a_variable_may_be_solved_as() {
  // `$T/Entity` takes an `Entity` or something that reaches one through
  // `using`/`#as`; `$T/interface R` takes any struct with R's members
  // (**L§7.8**).
  assert_output(
    "Entity :: struct { id: int; }\n\
     Tree :: struct { #as using base: Entity; seeds: int; }\n\
     Other :: struct { id: int; colour: int; }\n\
     take :: (x: $T/Entity) -> int { return x.id; }\n\
     take :: (x: $T/Other) -> int { return x.colour; }\n\
     shape :: (x: $T/interface Other) -> int { return x.colour; }\n\
     main :: () {\n  \
       t: Tree;\n  \
       t.id = 1;\n  \
       o: Other;\n  \
       o.colour = 2;\n  \
       put_number(take(t));\n  \
       put_number(take(o));\n  \
       Backward :: struct { colour: int; id: int; extra: int; }\n  \
       b: Backward;\n  \
       b.colour = 3;\n  \
       put_number(shape(b));\n\
     }\n",
    "1\n2\n3\n",
  );
}

#[test]
fn a_struct_body_defaults_a_member_a_using_brought_in() {
  // `type = Tree;` in a body whose `#as using base: Entity` declared `type`
  // sets that member's default (**L§8.1**, **L§8.4**).
  assert_output(
    "Entity :: struct { id: int; kind: int; }\n\
     Tree :: struct { #as using base: Entity; kind = 7; }\n\
     main :: () {\n  \
       t: Tree;\n  \
       put_number(t.kind);\n  \
       put_number(t.id);\n\
     }\n",
    "7\n0\n",
  );
}

#[test]
fn a_parameter_written_as_an_instantiation_over_variables_solves_them() {
  // `holder: Holder($T, $N)` matches whichever instantiation the call passed,
  // argument by argument — a type into a type variable, a value into a value
  // one (**L§8.5**).
  assert_output(
    "Holder :: struct ($T: Type, $N: s64) { array: [N] T; }\n\
     total :: (holder: Holder($T, $N)) -> int {\n  \
       sum := 0;\n  \
       for 0..N-1  sum += cast(int) holder.array[it];\n  \
       return sum;\n\
     }\n\
     deep :: (p: *[..] *Holder) -> int { return p.count; }\n\
     main :: () {\n  \
       ints: Holder(int, 3);\n  \
       ints.array[0] = 1;\n  \
       ints.array[2] = 41;\n  \
       put_number(total(ints));\n  \
       list: [..] *Holder(int, 3);\n  \
       put_number(deep(*list));\n\
     }\n",
    "42\n0\n",
  );
}

#[test]
fn a_struct_parameters_type_slot_may_declare_a_variable() {
  // `Thing :: struct (x: $T)` declares `T` beside `x`, and the argument's own
  // type is what it bakes to (**L§8.5**).
  assert_output(
    "#import \"Basic\";\n\
     Thing :: struct (x: $T) { y := x; }\n\
     main :: () {\n  \
       t: Thing(\"Hello\");\n  \
       put(t.y);\n  \
       put(\"\\n\");\n  \
       put(tprint(\"%\\n\", type_of(t)));\n\
     }\n",
    "Hello\nThing(T=string, x=\"Hello\")\n",
  );
}

#[test]
fn an_overload_set_passed_as_an_argument_is_narrowed_by_the_parameter() {
  // `map(fruits, to_upper)` means the `to_upper` that takes a string, not the
  // one that takes a `u8` (**L§7.5**).
  assert_output(
    "shout :: (c: u8) -> u8 { return c; }\n\
     shout :: (s: string) -> int { return s.count; }\n\
     each :: (values: [] $T, f: (T) -> $R) -> R {\n  \
       total: R;\n  \
       for values  total += f(it);\n  \
       return total;\n\
     }\n\
     main :: () {\n  \
       words :: string.[\"ab\", \"cde\"];\n  \
       put_number(each(words, shout));\n\
     }\n",
    "5\n",
  );
}

#[test]
fn arithmetic_widens_an_integer_into_a_float() {
  // Measured with the reference: `f * j` is a `float32` and `d * f` a
  // `float64`, where a *comparison* between the two would be an error
  // (**L§5.2**, **L§5.10**).
  assert_output(
    "main :: () {\n  \
       j: s64 = 3;\n  \
       f: float32 = 1.5;\n  \
       d: float64 = 2.25;\n  \
       put_number(cast(int)(f * j));\n  \
       put_number(cast(int)(d * f * 2.0));\n  \
       put_number(cast(int)(cast(float) j * j));\n\
     }\n",
    "4\n6\n9\n",
  );
}

#[test]
fn a_non_varargs_overload_wins_over_a_varargs_one() {
  // `array_add(*xs)` means the one that returns a `*T`, not the one whose
  // `..T` slot it would leave empty (**L§7.5**).
  assert_output(
    "take :: (values: *[..] $T) -> int { return 1; }\n\
     take :: (values: *[..] $T, more: ..T) -> int { return 2; }\n\
     main :: () {\n  \
       xs: [..] int;\n  \
       put_number(take(*xs));\n  \
       put_number(take(*xs, 1, 2));\n\
     }\n",
    "1\n2\n",
  );
}

#[test]
fn a_double_comma_call_runs_with_a_copy_of_the_context() {
  // `f(a,, allocator = x)` sets the named members for the duration of the
  // call, and the caller's own context is untouched (**L§10.1**).
  assert_output(
    "#import \"Basic\";\n\
     counted := 0;\n\
     counting :: (mode: Allocator_Mode, requested: s64, old: s64, memory: *void, data: *void) -> *void {\n  \
       if mode == .ALLOCATE  counted += 1;\n  \
       return context.default_allocator.proc(mode, requested, old, memory, data);\n\
     }\n\
     take :: () -> int { p := alloc(8); free(p); return counted; }\n\
     main :: () {\n  \
       mine: Allocator;\n  \
       mine.proc = counting;\n  \
       put_number(take());\n  \
       put_number(take(,, allocator = mine));\n  \
       put_number(take());\n\
     }\n",
    "0\n1\n1\n",
  );
}

#[test]
fn a_constant_a_using_member_imported_is_the_value_it_was_declared_with() {
  // `context.default_allocator` is a constant of `Context_Base`, reached
  // through the `#as using base` that `#Context` is built from (**L§8.4**).
  assert_output(
    "Base :: struct { x: int; K :: 7; }\n\
     Outer :: struct { #as using base: Base; y: int; }\n\
     main :: () {\n  \
       o: Outer;\n  \
       put_number(o.K);\n  \
       put_number(Outer.K);\n\
     }\n",
    "7\n7\n",
  );
}

#[test]
fn an_array_literal_assigned_to_a_view_has_storage_of_its_own() {
  // `b: [] s32 = .[1, 2, 3]` needs somewhere for the elements to live, and a
  // literal all of whose members fold is read-only data — which is what lets
  // a view over one outlive the procedure that returned it (**L§5.8**).
  assert_output(
    "#import \"Basic\";\n\
     words :: (odd: bool) -> [] string {\n  \
       if odd  return .[\"one\", \"three\"];\n  \
       return .[];\n\
     }\n\
     main :: () {\n  \
       b: [] s32;\n  \
       b = .[1, 1, 2, 3];\n  \
       total := 0;\n  \
       for b  total += it;\n  \
       put_number(total);\n  \
       for words(true)  { put(it); put(\"\\n\"); }\n  \
       put_number(words(false).count);\n\
     }\n",
    "7\none\nthree\n0\n",
  );
}

#[test]
fn a_using_declaration_still_declares_its_own_variable() {
  // `using cream: Ice_Cream;` widens the scope *and* declares `cream`, which
  // starts life with its type's defaults (**L§6.8**, **L§4.6**).
  assert_output(
    "Ice_Cream :: struct { flavour := 7; scoops := 2; }\n\
     main :: () {\n  \
       using cream: Ice_Cream;\n  \
       put_number(flavour);\n  \
       put_number(scoops);\n  \
       put_number(cream.scoops);\n\
     }\n",
    "7\n2\n2\n",
  );
}

#[test]
fn an_added_context_member_is_laid_out_after_the_ones_before_it() {
  // Every `#add_context` member takes its own storage, so a module's cannot
  // land on top of another's (**L§10.2**).
  assert_output(
    "#import \"Basic\";\n\
     Random :: #import \"Random\";\n\
     main :: () {\n  \
       base := cast(s64) cast(*u8) *context;\n  \
       style := (cast(s64) cast(*u8) *context.print_style) - base;\n  \
       state := (cast(s64) cast(*u8) *context.random_state) - base;\n  \
       if state >= style + size_of(type_of(context.print_style))  put(\"apart\\n\");\n  \
       context.print_style.default_format_int.base = 16;\n  \
       put(tprint(\"%\\n\", 255));\n\
     }\n",
    "apart\nff\n",
  );
}

#[test]
fn an_added_context_member_takes_the_default_it_was_declared_with() {
  // `#add_context random_state := Random_State.{1, 0};` is what makes the
  // first `random_get()` of a program the number it is (**L§10.2**).
  assert_output_with_file(
    "seeded.jai",
    "#add_context seed := 41;\n\
     bump :: () -> int { context.seed += 1; return context.seed; }\n",
    "#load \"seeded.jai\";\n\
     main :: () { put_number(bump()); }\n",
    "42\n",
  );
}

#[test]
fn a_named_for_expansion_is_an_ordinary_name_in_the_loops_scope() {
  // `for :every_other s` names the macro itself, which applies whatever the
  // container is; a macro that declares `` `it `` and assigns to it later
  // still exports the loop's own (**L§7.14**).
  assert_output(
    "every_other :: (s: string, body: Code, flags: For_Flags) #expand {\n  \
       `it: u8;\n  \
       `it_index: s64;\n  \
       i := 0;\n  \
       while i < s.count {\n    \
         `it = s[i];\n    \
         `it_index = i;\n    \
         #insert body;\n    \
         i += 2;\n  \
       }\n\
     }\n\
     main :: () {\n  \
       for :every_other \"abcdef\"  { put_number(it_index); put_number(cast(int) it); }\n\
     }\n",
    "0\n97\n2\n99\n4\n101\n",
  );
}

#[test]
fn a_return_may_name_the_values_it_gives() {
  // `return second = "Dolly";` fills the slot it names; the ones it does not
  // name take the header's defaults (**L§7.2**).
  assert_output(
    "fun :: (x: int) -> first: int = 1, second: int = 2 {\n  \
       if x == 0  return;\n  \
       if x == 1  return 10;\n  \
       if x == 2  return second = 20, first = 30;\n  \
       return second = 40;\n\
     }\n\
     main :: () {\n  \
       for 0..3 {\n    \
         a, b := fun(it);\n    \
         put_number(a);\n    \
         put_number(b);\n  \
       }\n\
     }\n",
    "1\n2\n10\n2\n30\n20\n1\n40\n",
  );
}

#[test]
fn a_parameter_defaulted_to_an_overloaded_name_takes_the_first_overload() {
  // Nothing at the declaration says which one is wanted (**L§7.5**).
  assert_output(
    "#import \"Basic\";\n\
     pick :: () -> int { return 1; }\n\
     pick :: (x: int) -> int { return x; }\n\
     Holder :: struct { p: (a: int, c := pick) -> int; }\n\
     main :: () { put(tprint(\"%\\n\", type_of(Holder.p))); }\n",
    "procedure (s64, procedure () -> s64) -> s64\n",
  );
}

#[test]
fn an_insert_in_a_procedure_body_reads_the_blocks_own_locals() {
  // The scope an `#insert` expands in is the one it was written in, which for
  // one written among statements is that block (**L§13.2**).
  assert_output(
    "main :: () {\n  \
       x := 1;\n  \
       #insert #code x = 7;\n  \
       put_number(x);\n  \
       #insert \"x = 9;\";\n  \
       put_number(x);\n\
     }\n",
    "7\n9\n",
  );
}

#[test]
fn code_a_run_hands_back_is_spliced_where_the_insert_stands() {
  // `#insert -> Code { … }` builds its program somewhere that is not part of
  // the program, so its names mean everything at the `#insert` (**L§13.1**).
  assert_output(
    "main :: () {\n  \
       x := 3;\n  \
       #insert -> Code { return #code x = (x * 10) + 4; }\n  \
       put_number(x);\n\
     }\n",
    "34\n",
  );
}

#[test]
fn a_macros_code_parameter_is_the_program_the_call_site_quoted() {
  // The `#code` directive is how the program was quoted, not part of it, so
  // what an `#insert` splices is what stands under it (**L§13.1**).
  assert_output(
    "twice :: (code: Code) #expand {\n  \
       #insert code;\n  \
       #insert code;\n\
     }\n\
     main :: () {\n  \
       x := 1;\n  \
       twice(#code { x *= 3; put_number(x); });\n\
     }\n",
    "3\n9\n",
  );
}

#[test]
fn an_insert_in_a_polymorphic_body_expands_once_per_instantiation() {
  // The text is whatever the constants make it, and the names one expansion
  // declares are not ones another can see (**L§13.2**).
  assert_output(
    "bookend :: ($body: string, n: int) {\n  \
       #insert \"counter := 0;\";\n  \
       for 1..n  #insert body;\n  \
       put_number(counter);\n\
     }\n\
     main :: () {\n  \
       bookend(\"counter += 1;\", 2);\n  \
       bookend(\"counter += 10;\", 3);\n\
     }\n",
    "2\n30\n",
  );
}

#[test]
fn a_run_in_a_polymorphic_struct_runs_for_every_instantiation() {
  // One `#run` per specialization, and the members it inserts are laid out
  // with the ones written by hand (**L§12.1**, **L§13.2**).
  assert_output(
    "gen :: (n: int) -> string {\n  \
       if n == 2  return \"a := 20;\";\n  \
       return \"a := 30;\";\n\
     }\n\
     Holder :: struct (N: int) {\n  \
       xs: [N] int;\n  \
       #insert #run gen(N);\n\
     }\n\
     main :: () {\n  \
       two: Holder(2);\n  \
       three: Holder(3);\n  \
       put_number(two.a);\n  \
       put_number(three.a);\n\
     }\n",
    "20\n30\n",
  );
}

#[test]
fn a_struct_body_defaults_one_element_of_an_array_member() {
  // A designator may subscript as well as name: the default lands at the
  // element's own offset (**L§8.1**).
  assert_output(
    "#import \"Basic\";\n\
     Matrix :: struct (N: int) {\n  \
       elements: [N * N] int;\n  \
       #insert #run diagonal(N);\n\
     }\n\
     diagonal :: (n: int) -> string {\n  \
       out := \"\";\n  \
       for 0..n-1  out = tprint(\"%1elements[%2] = 1;\\n\", out, it * n + it);\n  \
       return out;\n\
     }\n\
     main :: () {\n  \
       m: Matrix(3);\n  \
       for m.elements  put_number(it);\n\
     }\n",
    "1\n0\n0\n0\n1\n0\n0\n0\n1\n",
  );
}

#[test]
fn a_procedure_written_where_a_value_goes_is_its_own_address() {
  // An anonymous procedure is generated like any other, and the expression it
  // was written as is a pointer to it (**L§7.9**).
  assert_output(
    "main :: () {\n  \
       f := (a: int, b: int) -> int { return a * b; };\n  \
       put_number(f(6, 7));\n\
     }\n",
    "42\n",
  );
}

#[test]
fn a_quick_lambda_takes_the_shape_a_polymorphic_parameter_asks_for() {
  // `$S` is decided by what the lambda returns once `$T` has said what its
  // own parameter is (**L§7.8**, **L§7.9**).
  assert_output(
    "#import \"Basic\";\n\
     map :: (array: [] $T, f: (T) -> $S) -> [..] S {\n  \
       results: [..] S;\n  \
       for array  array_add(*results, f(it));\n  \
       return results;\n\
     }\n\
     main :: () {\n  \
       words :: string.[\"aa\", \"bbb\", \"cccc\"];\n  \
       for map(words, x => x.count)  put_number(it);\n\
     }\n",
    "2\n3\n4\n",
  );
}

#[test]
fn a_parameter_typed_through_a_family_parameter_takes_the_instantiations_type() {
  // `b: Poly` is a family, so `k: b.T` is only a type once the call has said
  // which instantiation `b` is — which makes the header polymorphic even
  // though it has no `$` in it (**L§7.8**, **L§8.5**).
  assert_output(
    "Poly :: struct (T: Type) { x: T; }\n\
     twice :: (b: Poly, k: b.T) -> int { return k * 2; }\n\
     Poly2 :: struct (T: Type) { x: T; LIMIT :: 9; }\n\
     hold :: (b: *Poly2) -> int { y: b.T = 3; return y + b.LIMIT; }\n\
     main :: () {\n  \
       q: Poly(int);\n  \
       put_number(twice(q, 21));\n  \
       r: Poly2(int);\n  \
       put_number(hold(*r));\n\
     }\n",
    "42\n12\n",
  );
}

#[test]
fn a_range_loop_declares_only_the_one_iterator_it_counts_with() {
  // A container loop declares `it_index` beside a named `it`; a range loop
  // has nothing to index, so one written outside it is not shadowed
  // (**L§6.6**), measured against the reference compiler.
  assert_output(
    "main :: () {\n  \
       it_index := 9;\n  \
       xs := int.[10, 20];\n  \
       for x: xs  put_number(it_index);\n  \
       for j: 0..1  put_number(it_index);\n  \
       for 0..1  put_number(it_index);\n  \
       for xs  put_number(it_index);\n\
     }\n",
    "0\n1\n9\n9\n9\n9\n0\n1\n",
  );
}

#[test]
fn a_macro_reads_the_names_it_exports_by_their_plain_names() {
  // A `` `x `` a macro declares belongs to the block it was expanded into, so
  // the macro's own body and a `Code` argument it splices back in both see it
  // (**L§7.13**, **L§7.14**).
  assert_output(
    "Coord :: struct { x, y: s64; }\n\
     Grid :: struct { values: [3] int; }\n\
     for_expansion :: (grid: Grid, body: Code, flags: For_Flags) #expand {\n  \
       for i: 0..2 {\n    \
         `it := grid.values[i];\n    \
         `it_index: Coord;\n    \
         it_index.x = i;\n    \
         it_index.y = i * 2;\n    \
         #insert body;\n  \
       }\n\
     }\n\
     twice :: (code: Code) #expand {\n  \
       `slot := 7;\n  \
       #insert code;\n  \
       slot += 1;\n  \
       #insert code;\n\
     }\n\
     main :: () {\n  \
       g: Grid;\n  \
       g.values[1] = 5;\n  \
       for g  put_number(it + it_index.y);\n  \
       twice(#code { put_number(slot); });\n\
     }\n",
    "0\n7\n4\n7\n8\n",
  );
}

#[test]
fn a_for_expansion_places_the_loops_iterators_with_a_backticked_for() {
  // `for `it, `it_index: xs` declares the loop's own iterators, which are the
  // storage the loop that expanded the macro reads (**L§7.14**).
  assert_output(
    "Holder :: struct { data: [3] int; }\n\
     for_expansion :: (a: *Holder, body: Code, flags: For_Flags) #expand {\n  \
       for `it, `it_index: a.data {\n    \
         #insert body;\n  \
       }\n\
     }\n\
     main :: () {\n  \
       h: Holder;\n  \
       h.data[0] = 74;\n  \
       h.data[1] = 65;\n  \
       h.data[2] = 73;\n  \
       for h  put_number(it_index * 100 + it);\n\
     }\n",
    "74\n165\n273\n",
  );
}

#[test]
fn an_insert_says_what_a_loop_control_in_the_program_it_splices_means() {
  // `#insert(break=break y) body` makes the user's `break` leave the loop the
  // expansion meant it to (**L§13.2**); one inside a loop the body opened for
  // itself is that loop's.
  assert_output(
    "Grid :: struct { rows: [2] int; }\n\
     for_expansion :: (grid: Grid, body: Code, flags: For_Flags) #expand {\n  \
       for y: 0..1 {\n    \
         for x: 0..1 {\n      \
           `it := grid.rows[y] * 10 + x;\n      \
           `it_index := y;\n      \
           #insert(break=break y) body;\n    \
         }\n  \
       }\n\
     }\n\
     main :: () {\n  \
       g: Grid;\n  \
       g.rows[0] = 1;\n  \
       g.rows[1] = 2;\n  \
       for g {\n    \
         put_number(it);\n    \
         for 0..0  break;\n    \
         if it == 11  break;\n  \
       }\n  \
       put_number(999);\n\
     }\n",
    "10\n11\n999\n",
  );
}

#[test]
fn a_for_decides_a_computed_modifier_at_compile_time() {
  // `for *= cond, <= cond xs` is what a `for_expansion` handed its caller's
  // `For_Flags` writes; both fold before anything is lowered (**L§6.6**).
  assert_output(
    "walk :: (xs: [] int, $pointers: bool, $backwards: bool) {\n  \
       for *= pointers, <= backwards xs {\n    \
         #if pointers  put_number(it.*);\n    \
         #if !pointers put_number(it);\n  \
       }\n\
     }\n\
     main :: () {\n  \
       xs := int.[1, 2, 3];\n  \
       walk(xs, false, false);\n  \
       walk(xs, true, true);\n\
     }\n",
    "1\n2\n3\n3\n2\n1\n",
  );
}

#[test]
fn a_modify_decides_the_specialization_a_call_site_gets() {
  // The header is read again with what `#modify` left, so a parameter written
  // `$T` takes the type the block chose rather than the one the call solved
  // (**L§7.8**).
  assert_output(
    "proc :: (a: $T)\n\
     #modify { T = s64; return true; }\n\
     { put_number(a); }\n\
     main :: () {\n  \
       small: s8 = 1;\n  \
       medium: s16 = 2;\n  \
       proc(small);\n  \
       proc(medium);\n  \
       proc(3);\n\
     }\n",
    "1\n2\n3\n",
  );
}

#[test]
fn a_modify_fills_a_variable_only_it_can_reach() {
  // `$R` lives in the return list, where nothing an argument says reaches it
  // (**L§7.8**).
  assert_output(
    "#import \"Basic\";\n\
     widen :: (a: $T) -> $R\n\
     #modify {\n  \
       R = T;\n  \
       ti := cast(*Type_Info) T;\n  \
       if ti.type == .INTEGER {\n    \
         info := cast(*Type_Info_Integer) T;\n    \
         if info.runtime_size < 4  R = s32;\n  \
       }\n  \
       return true;\n\
     }\n\
     { return cast(R) a; }\n\
     main :: () {\n  \
       small: u8 = 7;\n  \
       big: s64 = 9;\n  \
       put(tprint(\"% %\\n\", type_of(widen(small)), type_of(widen(big))));\n\
     }\n",
    "s32 s64\n",
  );
}

#[test]
fn a_polymorphic_struct_runs_its_own_modify() {
  // `Holder :: struct (N: int) #modify { if N < 8 N = 8; }` decides its own
  // arguments, and deduplication happens after it (**L§8.5**).
  assert_output(
    "Holder :: struct (N: int)\n\
     #modify { if N < 8 N = 8;  return true; }\n\
     { values: [N] int; }\n\
     main :: () {\n  \
       a: Holder(9);\n  \
       b: Holder(3);\n  \
       put_number(a.values.count);\n  \
       put_number(b.values.count);\n\
     }\n",
    "9\n8\n",
  );
}

#[test]
fn a_run_statement_takes_the_whole_statement() {
  // `#run stmt_or_block;` runs a statement, so the assignment after it is
  // part of the run rather than something done to what it produced
  // (**L§6.11**), and it runs once per instantiation (**L§12.1**).
  assert_output(
    "#no_reset counter: int;\n\
     p :: (v: $T) { #run counter += 1; }\n\
     main :: () {\n  \
       p(1);\n  \
       p(\"a\");\n  \
       p(2.5);\n  \
       put_number(counter);\n\
     }\n",
    "3\n",
  );
}

#[test]
fn a_struct_constant_reaches_its_as_member_through_its_offset() {
  // The bytes are laid out the way the struct says, so taking the member it
  // marked `#as` out of them is a conversion (**L§8.4**).
  assert_output(
    "V :: struct { x: int; y: int; }\n\
     Early :: struct { #as using v: V; name: string; }\n\
     Late :: struct { favorite: int; #as using v: V; }\n\
     show :: (a: V) { put_number(a.x); put_number(a.y); }\n\
     main :: () {\n  \
       show(Early.{x=5});\n  \
       show(Late.{y=7});\n\
     }\n",
    "5\n0\n0\n7\n",
  );
}

#[test]
fn a_baked_parameter_takes_a_procedure_name_as_the_constant_it_is() {
  // A procedure declared with `::` is a constant, so `$$x` bakes one and the
  // body can ask whether it got one (**L§5.11**).
  assert_output(
    "#import \"Basic\";\n\
     tell :: ($$x: $T, name: string) {\n  \
       if is_constant(x)  put(tprint(\"constant %\\n\", name));\n  \
       else               put(tprint(\"variable %\\n\", name));\n\
     }\n\
     other :: () {}\n\
     main :: () {\n  \
       tell(other, \"a procedure\");\n  \
       tell(3, \"a number\");\n  \
       n := 4;\n  \
       tell(n, \"a local\");\n\
     }\n",
    "constant a procedure\nconstant a number\nvariable a local\n",
  );
}

#[test]
fn a_baked_default_the_call_site_cannot_fold_stays_an_ordinary_parameter() {
  // `$x := #caller_location` is filled where the call was written, like any
  // other default (**L§7.13**).
  assert_output(
    "#import \"Basic\";\n\
     where :: ($x := #caller_location) -> int { return x.line_number; }\n\
     main :: () {\n  \
       a := where();\n  \
       b := where();\n  \
       put_number(b - a);\n\
     }\n",
    "1\n",
  );
}

#[test]
fn a_string_literal_casts_to_a_fixed_array_of_bytes_constantly() {
  // The bytes are the literal's, so the cast folds (**L§5.6**, **L§5.11**).
  assert_output(
    "main :: () {\n  \
       a :: cast([5] u8) \"Hello\";\n  \
       for a  put_number(it);\n\
     }\n",
    "72\n101\n108\n108\n111\n",
  );
}

#[test]
fn a_family_parameter_makes_one_specialization_per_instantiation() {
  // A header whose only variable is a parameter typed by a polymorphic struct
  // family binds no constants at all, so what tells two specializations apart
  // is the types the call gave those parameters (**L§7.8**, **L§8.5**).
  assert_output(
    "Box :: struct (T: Type) { x: T; }\n\
     first :: (b: *Box) -> b.T { return b.x; }\n\
     main :: () {\n  \
       a: Box(int);\n  \
       a.x = 7;\n  \
       s: Box(string);\n  \
       s.x = \"hi\\n\";\n  \
       put_number(first(*a));\n  \
       put(first(*s));\n\
     }\n",
    "7\nhi\n",
  );
}

#[test]
fn a_polymorphic_struct_takes_a_procedure_as_an_argument() {
  // A procedure name is a constant, so it bakes like any other (**L§8.5**).
  assert_output(
    "Thing :: struct (x: $T) { y := x; }\n\
     greet :: () { put(\"hi\\n\"); }\n\
     main :: () {\n  \
       a: Thing(\"hello\\n\");\n  \
       b: Thing(greet);\n  \
       put(a.y);\n  \
       b.y();\n\
     }\n",
    "hello\nhi\n",
  );
}

#[test]
fn this_names_the_procedure_or_type_that_contains_it() {
  // `#this` is a compile-time constant: the procedure it was written in, or
  // the struct whose body it stands in (**L§5.11**).
  assert_output(
    "#import \"Basic\";\n\
     factorial :: (x: int) -> int {\n  \
       f :: #this;\n  \
       if x <= 0  return 1;\n  \
       return x * f(x-1);\n\
     }\n\
     countdown :: (x: int) -> int {\n  \
       if x <= 0  return 1;\n  \
       return x * #this(x-1);\n\
     }\n\
     Self_Referential :: struct {\n  \
       pointer: *Self_Referential;\n  \
       other:   *#this;\n\
     }\n\
     main :: () {\n  \
       put_number(factorial(5));\n  \
       put_number(countdown(4));\n  \
       r: Self_Referential;\n  \
       put(tprint(\"% %\\n\", type_of(r.pointer) == type_of(r.other), is_constant(#this)));\n  \
       THIS :: #this;\n\
     }\n",
    "120\n24\ntrue true\n",
  );
}

#[test]
fn a_call_through_a_type_of_annotation_takes_that_headers_defaults() {
  // The type is the other procedure's either way; what the annotation carries
  // is its parameter names and defaults, which is what the reference does
  // — and calls strange (**L§7.2**).
  assert_output(
    "with_defaults :: (a := 1, b := 2) -> int { return a * 10 + b; }\n\
     without :: (a: int, b: int) -> int { return a * 10 + b; }\n\
     main :: () {\n  \
       f: type_of(with_defaults);\n  \
       f = without;\n  \
       put_number(f());\n  \
       put_number(f(7));\n\
     }\n",
    "12\n72\n",
  );
}

#[test]
fn a_name_reached_through_an_instantiation_resolves_under_it() {
  // `proc :: #bake_constants printer(T=#this)` in a polymorphic struct's body
  // bakes the specialization, not the family, so the call site has to resolve
  // it under the instantiation it reached it through (**L§8.5**).
  assert_output(
    "#import \"Basic\";\n\
     printer :: (x: $T) { put(tprint(\"% %\\n\", T, x.values)); }\n\
     Poly :: struct (E: Type) {\n  \
       values: E;\n  \
       proc :: #bake_constants printer(T=#this);\n\
       }\n\
     main :: () {\n  \
       a: Poly(float32);\n  \
       a.values = 1.5;\n  \
       a.proc(a);\n  \
       b: Poly(int);\n  \
       b.values = 7;\n  \
       b.proc(b);\n\
     }\n",
    "Poly(E=float32) 1.5\nPoly(E=s64) 7\n",
  );
}

#[test]
fn a_fixed_array_parameter_binds_its_dimension() {
  // `[$N] T` takes `N` from how long the argument is — from its type when it
  // has one, and from what was written when it is a `.[…]` (**L§7.8**).
  assert_output(
    "sum :: (a: [$N] float) -> int { total := 0.0; for a  total += it; return xx (total * 10 + N); }\n\
     baked :: ($a: [$N] float) -> int { return N * 100; }\n\
     main :: () {\n  \
       xs: [4] float;\n  \
       xs[0] = 1.5;\n  \
       put_number(sum(xs));\n  \
       put_number(sum(.[1.0, 2.0, 3.0]));\n  \
       put_number(baked(.[1.0, 2.0]));\n\
     }\n",
    "19\n63\n200\n",
  );
}

#[test]
fn an_array_parameter_binds_its_dimension_and_its_element_type_together() {
  // `[$N] $T` writes two variables at once. The length has to be bound before
  // the parameter is an array type at all — the header could not fold `N` —
  // and `$T` is bound by unifying what that produced against the argument, so
  // the order the two are solved in is what makes this work (**L§7.8**).
  assert_output(
    "#import \"Basic\";\n\
     first :: (a: [$N] $T) -> T { return a[0]; }\n\
     shape :: (a: [$N] $T) { print(\"% % %\\n\", N, T, a); }\n\
     main :: () {\n  \
       xs: [3] int;\n  \
       xs[0] = 7;\n  \
       print(\"%\\n\", first(xs));\n  \
       shape(xs);\n  \
       shape(float.[1.5, 2.5]);\n\
     }\n",
    "7\n3 s64 [7, 0, 0]\n2 float32 [1.5, 2.5]\n",
  );
}

#[test]
fn a_constant_array_of_procedures_is_data_the_back_end_fills_in() {
  // A procedure name is a constant, so a literal made of them is data whose
  // addresses the module that generates them writes (**L§5.11**).
  assert_output(
    "Simple :: #type () -> ();\n\
     one :: () { put(\"one\\n\"); }\n\
     two :: () { put(\"two\\n\"); }\n\
     Bling :: struct (x: [$N] $T) { y := x; }\n\
     main :: () {\n  \
       procs :: Simple.[one, two];\n  \
       for procs  it();\n  \
       b: Bling(Simple.[two, one]);\n  \
       put_number(b.N);\n  \
       for b.y  it();\n\
     }\n",
    "one\ntwo\n2\ntwo\none\n",
  );
}

#[test]
fn a_procedure_type_in_a_type_slot_needs_no_arrow() {
  // `f: (T)` is a procedure taking a `T` and returning nothing, not a
  // parenthesized `T` (**L§3.7**).
  assert_output(
    "call_with :: (arg: $T, f: (T)) { f(arg); }\n\
     named :: (x: int) { put_number(x); }\n\
     main :: () {\n  \
       call_with(6, named);\n  \
       call_with(7, x => { put_number(x * 10); });\n\
     }\n",
    "6\n70\n",
  );
}

#[test]
fn a_backticked_defer_belongs_to_the_block_the_macro_expanded_into() {
  // It runs when the caller's scope ends, and still names the expansion's own
  // locals (**L§7.13**).
  assert_output(
    "trace :: (n: int) #expand {\n  \
       mine := n * 10;\n  \
       put_number(mine);\n  \
       `defer put_number(mine + 1);\n\
     }\n\
     work :: () {\n  \
       trace(1);\n  \
       trace(2);\n  \
       put_number(999);\n\
     }\n\
     main :: () { work(); }\n",
    "10\n20\n999\n21\n11\n",
  );
}

#[test]
fn this_names_the_procedure_a_macro_expanded_into() {
  // A macro is spliced into whoever expanded it, so `#this` inside one is the
  // caller's, and `#location(#this)` is where that procedure was written
  // (**L§5.11**, **L§7.13**).
  assert_output(
    "#import \"Basic\";\n\
     Profile :: () #expand {\n  \
       put(tprint(\"in % at line %\\n\", type_of(#this), #location(#this).line_number));\n\
     }\n\
     watched :: () { Profile(); }\n\
     main :: () { watched(); }\n",
    "in procedure () at line 40\n",
  );
}

#[test]
fn a_caller_reads_what_a_macro_left_in_its_block() {
  // A `` `x `` a macro declares belongs to the block it was expanded into, so
  // the caller's code written after the expansion reads it (**L§7.13**).
  assert_output(
    "leave :: (n: int) #expand {\n  \
       `slot := n * 10;\n\
     }\n\
     main :: () {\n  \
       leave(4);\n  \
       put_number(slot);\n  \
       slot += 1;\n  \
       put_number(slot);\n\
     }\n",
    "40\n41\n",
  );
}

#[test]
fn a_compound_assignment_through_a_subscript_operator_reads_first() {
  // `w[x] += 10` is `w[x] = w[x] + 10` with the index evaluated once
  // (**L§6.7**). The reference compiler at 0.2.009 passes the right-hand side
  // alone (**L§19**).
  assert_output(
    "SIZE :: 4;\n\
     Wrapping :: struct { data: [SIZE] int; }\n\
     operator [] :: (w: Wrapping, index: int) -> int { return w.data[index % SIZE]; }\n\
     operator []= :: (w: *Wrapping, index: int, value: int) { w.data[index % SIZE] = value; }\n\
     main :: () {\n  \
       w: Wrapping;\n  \
       for 0..SIZE-1  w[it] = 10;\n  \
       w[0] += 10;\n  \
       w[1] *= 30;\n  \
       w[2] /= 2;\n  \
       for w.data  put_number(it);\n\
     }\n",
    "20\n300\n5\n10\n",
  );
}

#[test]
fn a_push_context_with_nothing_to_push_makes_a_default_one() {
  // `push_context { … }` with no expression runs the block under a
  // default-initialized `#Context` (**L§6.9**), which is the only way a
  // `#c_call` reaches one at all.
  assert_output(
    "callback :: (x: int) -> int #c_call {\n  \
       push_context {\n    \
         put_number(x);\n  \
       }\n  \
       return x;\n\
     }\n\
     main :: () { put_number(callback(7)); }\n",
    "7\n7\n",
  );
}

#[test]
fn a_context_knows_the_shape_it_was_built_with() {
  // `Context_Base.context_info` always names the `#Context` the compilation
  // settled on (**C§13**), so a library handed one can read its layout.
  assert_output(
    "#import \"Basic\";\n\
     main :: () {\n  \
       info := context.context_info;\n  \
       if info != null  put(\"named\\n\");\n  \
       if info == type_info(#Context)  put(\"itself\\n\");\n\
     }\n",
    "named\nitself\n",
  );
}

#[test]
fn a_lone_allocator_context_argument_that_is_null_pushes_nothing() {
  // `f(…,, allocator)` whose `proc` is null pushes nothing, so the caller's
  // allocator stays (**L§5.10**) — which is what lets a container hand its own
  // unset allocator on.
  assert_output(
    "#import \"Basic\";\n\
     Holder :: struct { allocator: Allocator; }\n\
     main :: () {\n  \
       holder: Holder;\n  \
       data := NewArray(4, int,, holder.allocator);\n  \
       put_number(data.count);\n\
     }\n",
    "4\n",
  );
}

#[test]
fn a_declaration_in_this_file_beats_one_an_import_brought_in() {
  // An overload set gathers outwards, and a tie is broken by the nearer scope
  // (**L§7.5**): a `#scope_file` procedure of a name `Basic` also declares is
  // the one a call in this file reaches.
  assert_output(
    "#import \"Basic\";\n\
     #scope_file\n\
     print_stack_trace :: (node: *Stack_Trace_Node) { put(\"ours\\n\"); }\n\
     #scope_export\n\
     main :: () { print_stack_trace(context.stack_trace); }\n",
    "ours\n",
  );
}

#[test]
fn every_procedure_keeps_a_stack_trace_node() {
  // `Build_Options.stack_trace` links a `Stack_Trace_Node` per call, with the
  // procedure's own name and the line the call above it was written at
  // (**C§13**).
  assert_output(
    "#import \"Basic\";\n\
     inner :: () {\n  \
       node := context.stack_trace;\n  \
       while node {\n    \
         if node.info  put(node.info.name);\n    \
         put(\"\\n\");\n    \
         node = node.next;\n  \
       }\n\
     }\n\
     outer :: () { inner(); }\n\
     main :: () { outer(); }\n",
    "inner\nouter\nmain\n",
  );
}

#[test]
fn a_stack_trace_node_counts_its_depth_and_names_the_calling_line() {
  // The node's `call_depth` counts from the outermost frame and its
  // `line_number` is the line of the call that frame is making (**C§13**).
  // The lines themselves depend on how much prelude the test prepends, so what
  // is asserted is the gap between the two call sites, which is three.
  assert_output(
    "#import \"Basic\";\n\
     inner :: () {\n  \
       from_outer := context.stack_trace.next;\n  \
       from_main := from_outer.next;\n  \
       put_number(from_outer.call_depth);\n  \
       put_number(from_main.call_depth);\n  \
       put_number(from_main.line_number - from_outer.line_number);\n\
     }\n\
     outer :: () {\n  \
       inner();\n\
     }\n\
     main :: () {\n  \
       outer();\n\
     }\n",
    "2\n1\n3\n",
  );
}

#[test]
fn the_things_that_are_constant_are_the_ones_the_reference_says() {
  // **L§5.11** lists them: `.count` and `.data` of a constant string or array
  // literal, `[i]` of one, a global fixed array's `.data` and that plus an
  // offset, the address of a global, `type_info`, `initializer_of`, and a
  // constant pointer cast to another pointer type. A local's address and the
  // `.data` of a global view are not.
  assert_output(
    "global_buffer: [8] u8;\n\
     global_view: [] u8;\n\
     global_int := 123;\n\
     TEXT :: \"Pay Respects\";\n\
     NUMBERS :: s16.[5, 4, 3, 2, 2];\n\
     Point :: struct { x: int; }\n\
     yes :: (c: bool) { put(ifx c then \"yes\\n\" else \"no\\n\"); }\n\
     main :: () {\n  \
       local: [8] u8;\n  \
       yes(is_constant(TEXT.count));\n  \
       yes(is_constant(TEXT.data));\n  \
       yes(is_constant(TEXT[5]));\n  \
       yes(is_constant(NUMBERS[1]));\n  \
       yes(is_constant(global_buffer.data));\n  \
       yes(is_constant(global_buffer.data + 3));\n  \
       yes(is_constant(*global_int));\n  \
       yes(is_constant(cast(*u8) *global_int));\n  \
       yes(is_constant(type_info(Point)));\n  \
       yes(is_constant(type_info(Point).runtime_size));\n  \
       yes(is_constant(initializer_of(Point)));\n  \
       yes(is_constant(global_view.data));\n  \
       yes(is_constant(*local));\n  \
       yes(is_constant(cast(s64) *global_int));\n\
     }\n",
    "yes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\nno\nno\nno\n",
  );
}

#[test]
fn a_constant_read_out_of_a_constant_is_the_value_it_holds() {
  // The folded value is the one the program sees, not just a claim that it
  // folded (**L§5.11**).
  assert_output(
    "TEXT :: \"Pay Respects\";\n\
     NUMBERS :: s16.[5, 4, 3, 2, 9];\n\
     Point :: struct { x: int; y: int; }\n\
     main :: () {\n  \
       put_number(TEXT.count);\n  \
       put_number(TEXT[5]);\n  \
       put_number(NUMBERS[4]);\n  \
       put_number(type_info(Point).runtime_size);\n\
     }\n",
    "12\n101\n9\n16\n",
  );
}

#[test]
fn a_location_is_a_constant_wherever_it_is_written() {
  // `#location` and `#caller_location` are constant struct literals
  // (**L§5.11**), and a `$$x` baked from one keeps the *call site* apart from
  // the header the default was written in (**L§7.13**).
  assert_output(
    "at :: ($$x: $T) -> int { #if is_constant(x)  return x.line_number;  return -1; }\n\
     here :: (loc := #caller_location) -> int { return at(loc); }\n\
     main :: () {\n  \
       put_number(at(#location()) - at(#location()));\n  \
       put_number(here() - here());\n\
     }\n",
    "0\n0\n",
  );
}

#[test]
fn a_location_of_a_name_is_where_a_constant_was_declared() {
  // Measured against the reference: `#location(x)` is where `x` was declared
  // when `x` is a constant, a procedure or a type, and where the `#location`
  // itself stands when `x` is a variable (**L§5.14**).
  assert_output(
    "CONSTANT :: 42;\n\
     procedure :: () {}\n\
     Shape :: struct {}\n\
     main :: () {\n  \
       variable := 7;\n  \
       base := #location().line_number;\n  \
       put_number(#location(CONSTANT).line_number - base);\n  \
       put_number(#location(procedure).line_number - base);\n  \
       put_number(#location(Shape).line_number - base);\n  \
       put_number(#location(variable).line_number - base);\n\
     }\n",
    "-5\n-4\n-3\n4\n",
  );
}

#[test]
fn a_break_inside_a_case_leaves_the_loop_around_the_switch() {
  // An `if ==` case has no fallthrough to break out of (**L§6.4**), so a
  // `break` written in one leaves the enclosing loop — measured against the
  // reference, and what `examples/output_types` relies on to leave its message
  // loop.
  assert_output(
    "main :: () {\n  \
       n := 0;\n  \
       while true {\n    \
         n += 1;\n    \
         if n == {\n      \
           case 1; put(\"one\\n\");\n      \
           case 2; put(\"two\\n\"); break;\n      \
           case;   put(\"more\\n\");\n    \
         }\n    \
         if n > 5  break;\n  \
       }\n  \
       put_number(n);\n\
     }\n",
    "one\ntwo\n2\n",
  );
}

/// Builds a program that imports a module written beside it, which is what a
/// `#module_parameters` test needs: the arguments live at the `#import`.
fn assert_output_with_module(module: &str, body: &str, expected: &str) {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let modules = directory.path().join("modules").join("Parameterized");
  std::fs::create_dir_all(&modules).expect("the module directory should be creatable");
  std::fs::write(modules.join("module.jai"), module).expect("the module should be writable");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, format!("{PRELUDE}\n{body}")).expect("the input should be writable");

  let mut options = oj_driver::BuildOptions::new();
  options.import_dirs = vec![directory.path().join("modules")];
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn a_module_parameter_takes_the_argument_the_import_gave_it() {
  // `#import "M"(N = 7)` is what `N` is inside the module, not the default its
  // declaration wrote (**L§11.3**).
  assert_output_with_module(
    "#module_parameters(N := 1, M := 2);\n\
     #scope_export\n\
     sum :: () -> int { return N * 10 + M; }\n",
    "Given :: #import \"Parameterized\"(7, M = 3);\n\
     main :: () { put_number(Given.sum()); }\n",
    "73\n",
  );
}

#[test]
fn a_module_parameter_a_default_covers_keeps_that_default() {
  assert_output_with_module(
    "#module_parameters(N := 1, M := 2);\n\
     #scope_export\n\
     sum :: () -> int { return N * 10 + M; }\n",
    "Given :: #import \"Parameterized\";\n\
     main :: () { put_number(Given.sum()); }\n",
    "12\n",
  );
}

#[test]
fn a_static_if_in_a_module_folds_its_module_parameters() {
  // The `#if` that decides which half of a module exists reads the parameters
  // as the constants they are, which is what keeps a `#import "Windows"` in
  // the branch nobody took out of a Linux build (**L§11.3**).
  assert_output_with_module(
    "#module_parameters(ENABLED := false);\n\
     #scope_export\n\
     #if !ENABLED {\n  \
       answer :: () -> int { return 1; }\n\
     } else {\n  \
       W :: #import \"Windows\";\n  \
       answer :: () -> int { return 2; }\n\
     }\n",
    "Given :: #import \"Parameterized\";\n\
     main :: () { put_number(Given.answer()); }\n",
    "1\n",
  );
}

#[test]
fn a_name_that_misses_in_a_body_nothing_instantiates_is_not_reported() {
  // A polymorphic body only exists once something instantiates it, so nothing
  // written there is checked until then (**L§7.8**) — which is what lets
  // `executable_formats` compile with a `Ranlib_Symbol` that is file-scope in
  // the file it was declared in.
  assert_output(
    "never_called :: ($T: Type) -> int {\n  \
       X :: Nonexistent_Name(T);\n  \
       return size_of(X);\n\
     }\n\
     main :: () { put_number(1); }\n",
    "1\n",
  );
}

#[test]
fn a_name_that_misses_in_a_body_a_call_site_instantiated_is_reported() {
  let Some(diagnostics) = diagnostics_of(
    "is_called :: ($T: Type) -> int {\n  \
       X :: Nonexistent_Name(T);\n  \
       return size_of(X);\n\
     }\n\
     main :: () { put_number(is_called(s32)); }\n",
  ) else {
    return;
  };
  assert!(
    diagnostics.contains("Undeclared identifier 'Nonexistent_Name'."),
    "{diagnostics}"
  );
}

#[test]
fn a_code_constant_handed_to_a_macro_is_the_program_it_quotes() {
  // A macro's `Code` parameter takes the argument unevaluated, except when the
  // argument already *is* a `Code`: then it hands over what it holds
  // (**L§13.1**).
  assert_output(
    "play :: (c: Code) #expand { #insert c; }\n\
     main :: () {\n  \
       quoted :: #code put(\"named\\n\");\n  \
       play(quoted);\n  \
       play(#code put(\"inline\\n\"));\n\
     }\n",
    "named\ninline\n",
  );
}

#[test]
fn an_inserted_code_block_declares_its_own_names() {
  // A `#code`'s contents are not walked where they were written, so the
  // `#insert` is what opens the block's scope and declares what it holds
  // (**L§13.1**).
  assert_output(
    "main :: () {\n  \
       quoted :: #code { n := 41; put_number(n + 1); };\n  \
       #insert quoted;\n\
     }\n",
    "42\n",
  );
}

#[test]
fn a_compound_constant_takes_one_value_of_the_run_it_was_given() {
  // `A, B :: #run f();` runs `f` once and gives each name one of its returns
  // (**L§4.5**, **L§12.1**), which is what `Basic.clamp_to_another_integer_type`
  // reads its three constants out of.
  assert_output(
    "two :: () -> (a: int, b: int) { return 4, 5; }\n\
     A, B :: #run two();\n\
     main :: () {\n  \
       #assert A == 4;\n  \
       #assert B == 5;\n  \
       put_number(A * 10 + B);\n\
     }\n",
    "45\n",
  );
}

#[test]
fn a_compound_constant_from_a_run_takes_each_return_type() {
  assert_output(
    "shape :: () -> (yes: bool, size: int) { return true, 12; }\n\
     main :: () {\n  \
       YES, SIZE :: #run shape();\n  \
       #assert YES;\n  \
       put_number(SIZE);\n\
     }\n",
    "12\n",
  );
}

#[test]
fn a_name_declared_as_a_bake_is_that_bake() {
  // `h :: formatHex;` over a `#bake_arguments` is callable the way the bake
  // itself is (**L§5.11**, **L§7.10**), which is what `debug_info` writes its
  // size assertions with.
  assert_output(
    "add :: (a: int, b: int) -> int { return a + b; }\n\
     add_ten :: #bake_arguments add(b = 10);\n\
     h :: add_ten;\n\
     main :: () {\n  \
       put_number(add_ten(1));\n  \
       put_number(h(2));\n\
     }\n",
    "11\n12\n",
  );
}

#[test]
fn file_info_directives_name_the_file_they_were_written_in() {
  // `#file`, `#filepath` and `#line` are where the directive stands
  // (**L§5.14**) — the fully-pathed name, the directory with its trailing
  // separator, and the line — which is what `GetRect` loads its images with.
  let Some(built) = build_and_run(
    "main :: () {\n  \
       put(#filepath);\n  \
       put(\"\\n\");\n  \
       put(#file);\n  \
       put(\"\\n\");\n  \
       put_number(#line);\n\
     }\n",
  ) else {
    return;
  };
  let mut lines = built.output.lines();
  let directory = lines.next().expect("the directory");
  let file = lines.next().expect("the file");
  let line = lines.next().expect("the line");
  assert!(
    directory.ends_with(std::path::MAIN_SEPARATOR),
    "'{directory}' should keep its trailing separator"
  );
  assert_eq!(file, format!("{directory}program.jai"));
  // The prelude is in front of the body, so the `#line` is the one the whole
  // file has rather than the one the body was written at.
  assert!(line.parse::<u32>().expect("a line number") > 1);
}

#[test]
fn a_member_a_using_brought_in_is_reachable_through_the_type() {
  // A member the flattening added is not in the struct's own scope
  // (**L§8.4**), and `type_of(Header.magic)` still finds it — which is what
  // `debug_info` asserts its header sizes with.
  assert_output(
    "Header :: union {\n  \
       using data: struct { magic: [30] u8; page: s32; }\n  \
       raw: [64] u8;\n\
     }\n\
     main :: () {\n  \
       put_number(size_of(type_of(Header.magic)));\n  \
       put_number(size_of(type_of(Header.raw)));\n\
     }\n",
    "30\n64\n",
  );
}

#[test]
fn a_layout_the_compiler_asked_for_while_running_is_not_a_cycle() {
  // A `#modify` is lowered against a type table image that measures every type
  // of the program, including whichever struct body is being resolved right
  // now — that is not the struct containing itself (**L§8.3**), which is what
  // `Bindings_Generator`'s context struct runs into.
  let Some(built) = build_and_run(
    "#import \"Hash_Table\";\n\
     Thing :: struct { name: string; }\n\
     #add_context generator: *struct {\n  \
       by_name: Table(string, *Thing);\n  \
       count: s64;\n\
     };\n\
     main :: () {\n  \
       put_number(ifx context.generator then 1 else 0);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "0\n");
}

#[test]
fn a_forced_cast_between_two_aggregates_reinterprets_the_bytes() {
  // `cast,force(T) x` over two structs the same size over is the storage read
  // as the other type (**L§5.6**), which is how `Basic`'s `S128` and `U128`
  // are the same bytes.
  assert_output(
    "Signed :: struct { low: u64; high: s64; }\n\
     Unsigned :: struct { low: u64; high: u64; }\n\
     main :: () {\n  \
       a: Signed;\n  \
       a.low = 5;\n  \
       a.high = -1;\n  \
       b := cast,force(Unsigned) a;\n  \
       put_number(cast(int) b.low);\n  \
       put_number(cast,no_check(int)(b.high == 0xffff_ffff_ffff_ffff));\n\
     }\n",
    "5\n1\n",
  );
}

#[test]
fn an_asm_instruction_with_nothing_but_slots_narrows_them_to_registers() {
  // `imul [rsp + 48]` is ambiguous where `imul rcx` is not, and a shift count
  // is `cl` whatever the instruction works at (**L§15**) — both of which
  // `Basic`'s 128-bit arithmetic writes.
  assert_output(
    "main :: () {\n  \
       m: s64 = 7;\n  \
       count: s64 = 3;\n  \
       low: s64 = ---;\n  \
       high: s64 = ---;\n  \
       #asm {\n    \
         result_high: gpr === d;\n    \
         result_low:  gpr === a;\n    \
         mov result_low, 11;\n    \
         imul result_high, result_low, m;\n    \
         mov low, result_low;\n    \
         mov high, result_high;\n  \
       }\n  \
       put_number(low);\n  \
       put_number(high);\n  \
       shifted: s64 = 1;\n  \
       #asm { count === c; }\n  \
       #asm { shl shifted, count; }\n  \
       put_number(shifted);\n\
     }\n",
    "77\n0\n8\n",
  );
}

#[test]
fn a_return_in_inserted_code_leaves_the_procedure_it_was_written_in() {
  // Quoted code means what it meant where it was written (**L§13.1**), so a
  // `return` in it leaves the procedure however many macros it was inserted
  // through — which is what `Hash_Table`'s `Walk_Table` searches with.
  assert_output(
    "walk :: (code: Code) #expand {\n  \
       `index := 0;\n  \
       while `index < 4 {\n    \
         #insert code;\n    \
         `index += 1;\n  \
       }\n\
     }\n\
     find :: (values: [] int, wanted: int) -> int {\n  \
       walk(#code {\n    \
         entry := values[index];\n    \
         if entry == wanted  return index;\n  \
       });\n  \
       return -1;\n\
     }\n\
     main :: () {\n  \
       v: [4] int;\n  \
       v[0] = 10; v[1] = 20; v[2] = 30; v[3] = 40;\n  \
       put_number(find(v, 30));\n  \
       put_number(find(v, 99));\n\
     }\n",
    "2\n-1\n",
  );
}

#[test]
fn a_hash_table_finds_what_was_added_to_it() {
  // `Table`'s own `compare_function` is a constant of the specialization
  // wherever it is read from (**L§8.5**), and `Walk_Table`'s inserted `#code`
  // returns from the procedure it was written in (**L§13.1**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     #import \"Hash_Table\";\n\
     main :: () {\n  \
       t: Table(string, int);\n  \
       init(*t, 16);\n  \
       table_add(*t, \"abc\", 7);\n  \
       table_add(*t, \"def\", 9);\n  \
       a, found_a := table_find(*t, \"abc\");\n  \
       b, found_b := table_find(*t, \"def\");\n  \
       _, missing := table_find(*t, \"ghi\");\n  \
       print(\"% % % % %\\n\", a, found_a, b, found_b, missing);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "7 true 9 true false\n");
}

#[test]
fn a_procedure_pattern_is_solved_after_the_variables_it_mentions() {
  // `reduce :: (operation: (x: T, total: T) -> T, values: ..$T)` asks the
  // first argument for a shape it cannot know until `T` is bound by the rest
  // (**L§7.9**), so a procedure pattern is the last parameter a call site
  // solves — which is what `examples/reduce.jai` is.
  assert_output(
    "sum :: (x, total) => x + total;\n\
     add :: (a: int, b: int) -> int { return a + b; }\n\
     reduce :: (operation: (x: T, total: T) -> T, values: .. $T) -> T {\n  \
       total: T;\n  \
       for values  total = operation(it, total);\n  \
       return total;\n\
     }\n\
     main :: () {\n  \
       put_number(reduce(add, 1, 2, 3));\n  \
       put_number(reduce(sum, 1, 2, 3));\n\
     }\n",
    "6\n6\n",
  );
}

#[test]
fn a_local_whose_initializer_waits_for_a_macro_is_typed_when_the_body_is() {
  // A `` `x `` a macro declares only exists once it has expanded (**L§7.13**),
  // so a local made from it could not be typed in the pass before the bodies
  // were checked — which is what `Simp` reaches its own state through.
  assert_output(
    "State :: struct { width: int; }\n\
     #add_context state_pointer: *State;\n\
     check_init :: () #expand {\n  \
       `state := context.state_pointer;\n\
     }\n\
     use :: () {\n  \
       check_init();\n  \
       held := state;\n  \
       slot := *state.width;\n  \
       slot.* = 42;\n  \
       put_number(held.width);\n\
     }\n\
     main :: () {\n  \
       storage: State;\n  \
       context.state_pointer = *storage;\n  \
       use();\n\
     }\n",
    "42\n",
  );
}

#[test]
fn an_anonymous_union_statement_shares_one_piece_of_storage() {
  // `union { a: float64; b: u64; }` written as a statement declares names for
  // one piece of storage (**L§8.6**), which is how `Math`'s `frexp` reads a
  // `float64` as its bits.
  assert_output(
    "main :: () {\n  \
       union {\n    \
         f: float64;\n    \
         u: u64;\n  \
       }\n  \
       f = 1.0;\n  \
       put_number(cast,no_check(int)(u == 0x3ff0_0000_0000_0000));\n  \
       u = 0x4000_0000_0000_0000;\n  \
       put_number(cast(int) f);\n\
     }\n",
    "1\n2\n",
  );
}

#[test]
fn a_literal_prefers_the_overload_of_the_type_it_defaults_to() {
  // A float literal fits both a `float32` and a `float64` parameter, so what
  // settles the tie is the type it would have taken on its own (**L§5.10**) —
  // which is what keeps `Math`'s `sqrt :: (x: float)` from calling itself.
  assert_output(
    "width :: (x: float32) -> int { return 32; }\n\
     width :: (x: float64) -> int { return 64; }\n\
     main :: () {\n  \
       put_number(width(1.5));\n  \
       put_number(width(cast(float64) 1.5));\n  \
       n := 3;\n  \
       put_number(width(cast(float32) n));\n\
     }\n",
    "32\n64\n32\n",
  );
}

#[test]
fn a_restriction_written_as_an_array_of_types_allows_any_of_them() {
  // `$T/MyVectors` over `MyVectors :: Type.[Vector2, Vector3]` allows any one
  // of them and nothing else (**L§7.8**), and a pointer is not the thing it
  // points at — which is what tells `Math.normalize`'s two overloads apart.
  assert_output(
    "V2 :: struct { x: float; }\n\
     V3 :: struct { x: float; y: float; }\n\
     Mine :: Type.[V2, V3];\n\
     scale :: (v: *$T/Mine, by := 2.0, fallback := T.{}) -> float {\n  \
       v.x *= by;\n  \
       return v.x;\n\
     }\n\
     scale :: (v: $T/Mine, by := 2.0, fallback := T.{}) -> T {\n  \
       w := v;\n  \
       inline scale(*w, by, fallback);\n  \
       return w;\n\
     }\n\
     main :: () {\n  \
       a: V2; a.x = 5;\n  \
       put_number(cast(int) scale(*a));\n  \
       b: V3; b.x = 5;\n  \
       put_number(cast(int) scale(b).x);\n\
     }\n",
    "10\n10\n",
  );
}

#[test]
fn a_backticked_name_a_static_if_chose_is_the_branch_it_kept() {
  // A macro may declare the same `` `x `` in each branch of a `#if`, the way
  // `Hash_Table`'s `for_expansion` writes `` `it := *entry.value `` under
  // `.POINTER` and `` `it := entry.value `` otherwise (**L§6.10**,
  // **L§7.13**): what the caller reads is the branch the condition kept,
  // decided under the expansion that wrote it.
  assert_output(
    "give :: ($want_pointer: bool, v: *int) #expand {\n  \
       #if want_pointer {\n    \
         `chosen := v;\n  \
       } else {\n    \
         `chosen := v.*;\n  \
       }\n\
     }\n\
     main :: () {\n  \
       n := 7;\n  \
       give(false, *n);\n  \
       put_number(chosen);\n\
     }\n",
    "7\n",
  );
}

#[test]
fn a_for_expansion_chooses_its_iterator_by_the_flags_it_was_given() {
  assert_output(
    "Bag :: struct { values: [3] int; }\n\
     for_expansion :: (bag: *Bag, body: Code, flags: For_Flags) #expand {\n  \
       for v, i: bag.values {\n    \
         #if flags & .POINTER {\n      \
           `it := *v;\n    \
         } else {\n      \
           `it := v;\n    \
         }\n    \
         `it_index := i;\n    \
         #insert body;\n  \
       }\n\
     }\n\
     main :: () {\n  \
       b: Bag;\n  \
       b.values[1] = 7;\n  \
       for b  put_number(it * 10 + it_index);\n\
     }\n",
    "0\n71\n2\n",
  );
}

#[test]
fn a_library_the_compiler_built_is_linked_by_the_file_it_is() {
  // A Jai dynamic library is `<name>.so`, which `-l<name>` would never find,
  // so a non-system `#library` names the file that is actually there
  // (**L§12.2**) — which is what `examples/dll` links against.
  let request = oj_link::Request {
    objects: vec![std::path::PathBuf::from("program.o")],
    libraries: vec![oj_ir::Library {
      name: String::from("helper"),
      system: false,
      directory: Some(std::path::PathBuf::from("/nonexistent")),
    }],
    output: std::path::PathBuf::from("program"),
    output_type: oj_link::OutputType::Executable,
    additional_arguments: Vec::new(),
  };
  // Nothing is there, so it stays the plain spelling.
  let line = oj_link::link_line(&request);
  assert!(line.arguments.iter().any(|argument| argument == "-lhelper"));
}

#[test]
fn a_using_of_a_value_widens_the_scope_of_a_module_for_its_importers() {
  // A module that writes `using gl;` hands its importers the members of `gl`
  // the way it hands them any other name (**L§6.8**, **L§11.2**), which is
  // what makes `glClearColor` a name after `#import "GL"`.
  assert_output_with_module(
    "Procs :: struct {\n  \
       count: int;\n  \
       twice :: (x: int) -> int { return x * 2; }\n\
     }\n\
     procs: Procs;\n\
     using procs;\n",
    "#import \"Parameterized\";\n\
     main :: () {\n  \
       put_number(twice(21));\n  \
       put_number(count);\n\
     }\n",
    "42\n0\n",
  );
}

#[test]
fn a_procedure_name_is_true_where_a_static_if_reads_it() {
  // `#if given_hash_function` in `Hash_Table` asks whether the specialization
  // was handed one: a procedure name is never null, whatever address the back
  // end gives it (**L§5.9**), and the constant that aliases it is that
  // specialization's (**L§8.5**).
  assert_output(
    "seven :: (k: int) -> int { return k * 7; }\n\
     Box :: struct (Key: Type, given: (Key) -> int = null) {\n  \
       #if given {\n    \
         chosen :: given;\n  \
       } else {\n    \
         chosen :: x => 999;\n  \
       }\n  \
       value: int;\n\
     }\n\
     main :: () {\n  \
       given: Box(int, seven);\n  \
       put_number(given.chosen(3));\n  \
       fallback: Box(int);\n  \
       put_number(fallback.chosen(3));\n\
     }\n",
    "21\n999\n",
  );
}

#[test]
fn a_hash_table_with_its_own_hash_function_uses_it() {
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     #import \"Hash_Table\";\n\
     seven :: (k: int) -> u32 { return cast(u32)(k * 7); }\n\
     main :: () {\n  \
       t: Table(int, int, seven);\n  \
       print(\"%\\n\", t.hash_function(3));\n  \
       init(*t, 8);\n  \
       table_add(*t, 3, 30);\n  \
       v, ok := table_find(*t, 3);\n  \
       print(\"% %\\n\", v, ok);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "21\n30 true\n");
}

#[test]
fn two_tables_of_different_keys_do_not_share_one_compare_function() {
  // A procedure declared inside a baked polymorphic struct is one per
  // specialization, so `Table(int, int)`'s `compare_function` and
  // `Table(float, int)`'s are two symbols rather than one (**L§8.5**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     #import \"Hash_Table\";\n\
     main :: () {\n  \
       a: Table(int, int);\n  \
       init(*a);\n  \
       table_add(*a, 1, 2);\n  \
       b: Table(float, int);\n  \
       init(*b);\n  \
       table_add(*b, 1.0, 3);\n  \
       v1, ok1 := table_find(*a, 1);\n  \
       v2, ok2 := table_find(*b, 1.0);\n  \
       print(\"% % % %\\n\", v1, ok1, v2, ok2);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "2 true 3 true\n");
}

#[test]
fn a_baked_code_parameter_takes_the_expression_the_call_site_wrote() {
  // `$c: Code` bakes the syntax rather than what it evaluates to, and `c.type`
  // is the type of the expression it quotes (**L§13.1**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     f :: ($c: Code) -> u32 {\n  \
       T :: c.type;\n  \
       print(\"T is %\\n\", T);\n  \
       return 42;\n\
     }\n\
     main :: () { f(2 + 3 + 4); f(\"Hello, Sailor!\"); }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "T is s64\nT is string\n");
}

#[test]
fn an_untyped_struct_literal_tells_two_overloads_apart() {
  // A `.{…}` converts to every struct, so what it was written as is what
  // chooses between two candidates (**L§5.7**, **L§7.5**).
  assert_output(
    "V2 :: struct { x: int; y: int; }\n\
     V3 :: struct { x: int; y: int; z: int; }\n\
     f :: (p: V2) -> int { return 2; }\n\
     f :: (p: V3) -> int { return 3; }\n\
     main :: () {\n  \
       put_number(f(.{1, 2}));\n  \
       put_number(f(.{1, 2, 3}));\n\
     }\n",
    "2\n3\n",
  );
}

#[test]
fn an_untyped_struct_literal_reaches_a_polymorphic_parameter() {
  // `array_add(*things, .{…})` takes `T` from the pointer and lets the literal
  // be whatever the parameter says (**L§5.7**, **L§7.8**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Thing :: struct { name: string; n: int; }\n\
     main :: () {\n  \
       things: [..] Thing;\n  \
       array_add(*things, .{\"x\", 3});\n  \
       print(\"% %\\n\", things.count, things[0].n);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "1 3\n");
}

#[test]
fn a_shift_by_a_narrow_integer_is_an_s64() {
  // A shift keeps its left operand's type, and a literal there defaults the
  // way an untyped integer does anywhere else (**L§5.10**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     FLAG :: 1 << 3;\n\
     main :: () {\n  \
       a: u8 = 3;\n  \
       b := 1 << a;\n  \
       x: u8 = FLAG;\n  \
       print(\"% % %\\n\", type_of(b), b, x);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "s64 8 8\n");
}

#[test]
fn an_autocast_operand_takes_the_other_ones_type() {
  // `xx n - 1` has no type of its own: `for i: a..xx n-1` is what asks for one
  // (**L§5.6**).
  assert_output(
    "main :: () {\n  \
       required: u32 = 4;\n  \
       for i: 2..xx required-1 put_number(i);\n  \
       start: s64 = 10;\n  \
       put_number(start + xx required);\n\
     }\n",
    "2\n3\n14\n",
  );
}

#[test]
fn two_integer_constants_of_unlike_types_agree_on_the_wider_one() {
  // `Math.U64_MAX / (size_of(Key) + size_of(Value))` mixes a `u64` with an
  // `s64` and is a `u64`, where the same operator on two variables is
  // rejected (**L§5.10**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Math :: #import \"Math\";\n\
     M :: Math.U64_MAX / (size_of(int) + size_of(string));\n\
     main :: () { print(\"% %\\n\", type_of(M), M); }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "u64 768614336404564650\n");
}

#[test]
fn a_null_constant_takes_the_pointer_type_it_was_declared_with() {
  // `offset: *T : null;` is a `*T`, which is what makes `*offset.member` the
  // member's offset (**L§3.2**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     V :: struct { a: float; b: float; c: float; }\n\
     main :: () {\n  \
       offset: *V : null;\n  \
       q := *offset.c;\n  \
       print(\"% %\\n\", type_of(q), cast(int) q);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "*float32 8\n");
}

#[test]
fn a_call_through_a_procedure_type_takes_the_defaults_it_was_written_with() {
  // The defaults live on the annotation: `f: (enable := false);` is callable
  // with nothing (**L§7.2**).
  assert_output(
    "Procs :: struct { f: (enable := false); }\n\
     procs: Procs;\n\
     real :: (enable := false) { put_number(cast(int) enable); }\n\
     main :: () {\n  \
       procs.f = real;\n  \
       procs.f();\n  \
       procs.f(true);\n\
     }\n",
    "0\n1\n",
  );
}

#[test]
fn a_using_of_a_value_shadows_a_name_declared_further_out() {
  // Inside `{ using procs; … }`, `f` is the member rather than the procedure
  // of that name (**L§6.8**).
  assert_output(
    "Procs :: struct { f: (a: int); }\n\
     procs: Procs;\n\
     f :: (r: string) { put(\"outer\\n\"); }\n\
     real :: (a: int) { put_number(a); }\n\
     main :: () {\n  \
       using procs;\n  \
       if !f  f = real;\n  \
       f(7);\n\
     }\n",
    "7\n",
  );
}

#[test]
fn a_name_declares_itself_after_the_value_it_is_given() {
  // A local is not in scope in its own initializer: `type, ok := get_type(t,
  // type)` reads the parameter, not the name it is declaring (**L§6.13**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Id :: #type,distinct u32;\n\
     get_one :: (xs: [] int, id: Id) -> int, bool {\n  \
       if id >= 1  return xs[0], true;\n  \
       return -1, false;\n\
     }\n\
     outer :: (xs: [] int, id: Id) -> int, bool {\n  \
       id2, ok := get_one(xs, id);\n  \
       return id2, ok;\n\
     }\n\
     main :: () {\n  \
       xs := int.[7];\n  \
       v, ok := outer(xs, 2);\n  \
       print(\"% %\\n\", v, ok);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "7 true\n");
}

#[test]
fn one_name_of_a_compound_declaration_may_declare_itself() {
  // `declaration:, offset, success = f();` declares the first and assigns to
  // the rest (**L§4.5**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     g :: () -> string, int, bool { return \"a\", 1, true; }\n\
     main :: () {\n  \
       offset := 0;\n  \
       success := false;\n  \
       declaration:, offset, success = g();\n  \
       print(\"% % %\\n\", declaration, offset, success);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "a 1 true\n");
}

#[test]
fn a_pointer_is_indexed_beside_a_subscript_operator() {
  // Only a struct can overload `[]`, so `str.data[i]` on a `*u8` is an index
  // whatever `operator []`s the program declares elsewhere (**L§5.4**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Boxed :: struct { values: [4] int; }\n\
     operator [] :: (b: Boxed, index: int) -> int { return b.values[index]; }\n\
     main :: () {\n  \
       text := \"hello\";\n  \
       print(\"%\\n\", text.data[1]);\n  \
       b: Boxed;\n  \
       b.values[2] = 9;\n  \
       print(\"%\\n\", b[2]);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "101\n9\n");
}

#[test]
fn a_module_alias_reaches_the_module_it_names() {
  // `P :: Posix;` names whatever `Posix` names (**L§11.2**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     B :: #import \"Basic\";\n\
     main :: () {\n  \
       P :: B;\n  \
       P.print(\"hi %\\n\", 1);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "hi 1\n");
}

#[test]
fn a_struct_pointer_reaches_a_parameter_written_as_the_family() {
  // `max_load_factor(map)` hands a `*Pdb_Map(K, V)` to a `Pdb_Map`
  // (**L§7.6**, **L§8.5**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Map :: struct (K: Type, V: Type) { keys: [] K; values: [] V; }\n\
     load :: (m: Map) -> int { return m.keys.count * 2 / 3 + 1; }\n\
     use :: (m: *Map($K, $V)) -> int { return load(m); }\n\
     main :: () {\n  \
       m: Map(int, string);\n  \
       print(\"%\\n\", use(*m));\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "1\n");
}

#[test]
fn a_for_expansion_inside_a_polymorphic_body_names_its_own_it() {
  // The loop's `it` belongs to the instantiation the loop was written in, not
  // to the expansion (**L§7.8**, **L§7.14**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     #import \"Hash_Table\";\n\
     to_array_by_pointer :: (table: $T/Table) -> [..] *T.Value_Type {\n  \
       result: [..] *T.Value_Type;\n  \
       for * table  array_add(*result, it);\n  \
       return result;\n\
     }\n\
     main :: () {\n  \
       t: Table(int, int);\n  \
       init(*t);\n  \
       table_add(*t, 1, 2);\n  \
       a := to_array_by_pointer(t);\n  \
       print(\"% %\\n\", a.count, a[0].*);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "1 2\n");
}

#[test]
fn a_sixteen_bit_byte_swap_is_a_rotate() {
  // `bswap` has no 16-bit form the integrated assembler will take, and a
  // 16-bit byte swap *is* a rotate by eight (**L§15**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     #import \"Bit_Operations\";\n\
     main :: () {\n  \
       a: u16 = 0x1234;\n  \
       b: u32 = 0x12345678;\n  \
       print(\"% %\\n\", formatInt(byte_swap(a), base = 16), formatInt(byte_swap(b), base = 16));\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "3412 78563412\n");
}

#[test]
fn a_library_is_found_from_the_file_the_foreign_header_was_written_in() {
  // A binding written before a `#scope_file` belongs to the module, while the
  // `#library` it names is written after it (**L§11.1**, **L§12.2**). Its path
  // is relative, and what it is relative *to* is the file it was written in —
  // `../../native` from inside the module reaches the library beside the
  // program. Resolving it against the program instead would climb out of the
  // temporary directory entirely.
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let module = directory.path().join("modules").join("Native");
  std::fs::create_dir_all(&module).expect("the module directory should be creatable");

  // A static archive rather than a shared object, so that what is being
  // tested is where the *compiler* looked for the library and not whether the
  // loader can find it afterwards.
  let c_path = module.join("native.c");
  std::fs::write(&c_path, "int answer(void) { return 42; }\n").expect("the C source is writable");
  let object = directory.path().join("native.o");
  let compiled = Command::new(oj_link::driver())
    .arg("-c")
    .arg("-o")
    .arg(&object)
    .arg(&c_path)
    .status();
  if !compiled.is_ok_and(|status| status.success()) {
    eprintln!("skipping: the C driver could not build the test library");
    return;
  }
  let archived = Command::new("ar")
    .arg("rcs")
    .arg(directory.path().join("native.a"))
    .arg(&object)
    .status();
  if !archived.is_ok_and(|status| status.success()) {
    eprintln!("skipping: no 'ar' to build the test library with");
    return;
  }

  // The header is the module's, exported to whoever imports it; the
  // `#library` that binds it is written after a `#scope_file`, so the name
  // `native` never leaves this file.
  std::fs::write(
    module.join("module.jai"),
    "answer :: () -> s32 #foreign native;\n#scope_file\nnative :: #library \"../../native\";\n",
  )
  .expect("the module is writable");

  let path = directory.path().join("program.jai");
  std::fs::write(
    &path,
    format!(
      "{PRELUDE}\nNative :: #import \"Native\";\nmain :: () {{ put_number(Native.answer()); }}\n"
    ),
  )
  .expect("the input should be writable");

  let mut options = oj_driver::BuildOptions::new();
  options.import_dirs = vec![directory.path().join("modules")];
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
}

#[test]
fn a_foreign_header_with_any_varargs_is_a_c_variadic_call() {
  // `syscall :: (__sysno: s64, __args: ..Any) -> s64 #foreign libc;` is C's
  // `...`: each argument past the fixed ones is passed on its own rather than
  // gathered into a `[] Any`, which is what `stat` needs (**L§7.11**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     #import \"File_Utilities\";\n\
     main :: () {\n  \
       print(\"% %\\n\", file_exists(\"/tmp\"), is_directory(\"/tmp\"));\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "true true\n");
}

#[test]
fn an_insert_with_an_empty_scope_resolves_where_it_is_written() {
  // `#insert,scope() code` resolves the quoted names at the insertion point
  // rather than where the `#code` was written, and a `Code` that names a piece
  // of program tests true (**L§13.1**, **L§13.2**).
  assert_output(
    "f :: (x: int, $code := #code,null) -> int {\n  \
       #if code {\n    \
         v := #insert,scope() code;\n  \
       } else {\n    \
         v := x;\n  \
       }\n  \
       return v * 2;\n\
     }\n\
     main :: () {\n  \
       put_number(f(3));\n  \
       put_number(f(3, #code x + 10));\n\
     }\n",
    "6\n26\n",
  );
}

#[test]
fn a_run_inside_an_unsolved_specialization_does_not_execute() {
  // A call written inside a polymorphic body is solved against *that* body's
  // variables while it is checked on its own; the shape that produces is not a
  // call site's, so nothing in it runs (**L§7.8**, **L§12.1**).
  assert_output(
    "needs_a_size :: ($T: Type) -> bool {\n  \
       #assert size_of(T) > 0;\n  \
       return true;\n\
     }\n\
     outer :: (value: *$T) -> bool {\n  \
       return needs_a_size(T);\n\
     }\n\
     main :: () {\n  \
       put_number(1);\n\
     }\n",
    "1\n",
  );
}

#[test]
fn a_procedure_declared_beside_an_import_joins_its_overloads() {
  // A procedure a scope declares and one it imported are the same overload
  // set, not a name shadowing another: `Jails` declares `contains` over its
  // own type beside a `#import "String"` (**L§7.5**, **L§11.2**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     #import \"String\";\n\
     Loc :: struct { l0: int; }\n\
     contains :: (a: Loc, b: Loc) -> bool { return a.l0 == b.l0; }\n\
     main :: () {\n  \
       x, y: Loc;\n  \
       print(\"% % %\\n\", contains(\"a/b\", \"/\"), contains(\"a/b\", #char \"/\"), contains(x, y));\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "true true true\n");
}

#[test]
fn a_null_argument_decides_a_variable_nothing_else_did() {
  // `null` fits every pointer, so it keeps the `T` an earlier argument gave
  // it and settles one nothing has: `lsp_respond(id, null)` is a `*void`
  // (**L§7.8**), measured against the reference.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     f :: (id: u32, data: $T) { print(\"% \", type_of(data)); }\n\
     g :: (a: *$T, b: *T) { print(\"% \", type_of(a)); }\n\
     main :: () {\n  \
       p: *int;\n  \
       f(1, null);\n  \
       f(2, 7);\n  \
       g(p, null);\n  \
       g(null, p);\n  \
       print(\"\\n\");\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "*void s64 *s64 *void \n");
}

#[test]
fn a_labelled_control_names_the_loop_a_for_expansion_stands_in_for() {
  // `for file: table { … continue file; }` opens no loop of its own — the
  // `for_expansion` opens one for it — and the name the caller gave its
  // iterator reaches that one (**L§6.5**, **L§7.14**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     #import \"Hash_Table\";\n\
     main :: () {\n  \
       t: Table(int, int);\n  \
       init(*t);\n  \
       table_add(*t, 1, 1);\n  \
       table_add(*t, 2, 2);\n  \
       table_add(*t, 3, 3);\n  \
       inner := int.[10, 20];\n  \
       total := 0;\n  \
       for v: t {\n    \
         for x: inner {\n      \
           if v == 2 && x == 10 continue v;\n      \
           if v == 3 && x == 20 break v;\n      \
           total += v * x;\n    \
         }\n    \
         total += 1000;\n  \
       }\n  \
       print(\"%\\n\", total);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "30\n");
}

#[test]
fn a_cast_to_a_fixed_array_reads_the_array_at_the_pointer() {
  // `cast([4] u8) p` is the four bytes *at* `p`, not four bytes made out of
  // the pointer itself (**L§5.6**) — so advancing the pointer by one advances
  // the window by one. Measured against the reference, and what `Adpcm`'s
  // decoder takes each block of encoded samples with.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     main :: () {\n  \
       bytes: [8] u8;\n  \
       bytes[0] = 11; bytes[1] = 22; bytes[2] = 33; bytes[3] = 44; bytes[4] = 55;\n  \
       p := bytes.data;\n  \
       print(\"% \", cast([4] u8) p);\n  \
       p += 1;\n  \
       print(\"%\\n\", cast([4] u8) p);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "[11, 22, 33, 44] [22, 33, 44, 55]\n");
}

#[test]
fn a_macro_passes_on_a_register_it_declared_itself() {
  // A macro taking a `__reg` binds the name to the caller's register
  // (**L§15**). Handing that name to *another* macro means the argument
  // resolves to the parameter, which is an alias rather than a register — so
  // following the alias is what lets a chain of `#asm` macros pass registers
  // down. `meow_hash` is written that way throughout.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     add_into :: (a: __reg, b: __reg) #expand { #asm { paddq a, b; } }\n\
     mix :: (a: __reg) #expand {\n  \
       #asm { one: vec; movq one, ones; }\n  \
       add_into(a, one);\n\
     }\n\
     main :: () {\n  \
       ones: s64 = 1;\n  \
       start: s64 = 40;\n  \
       #asm { acc: vec; movq acc, start; }\n  \
       mix(acc);\n  \
       mix(acc);\n  \
       out: s64;\n  \
       #asm { movq out, acc; }\n  \
       print(\"%\\n\", out);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "42\n");
}

#[test]
fn an_enum_literal_reaches_a_variable_another_argument_bound() {
  // `.NAME` has no type of its own: the parameter says what it is
  // (**L§5.12**). When that parameter is a `$T` some earlier argument already
  // decided, the literal has nothing left to say — so solving must let it
  // through rather than trying to unify an untyped enum against the binding.
  // `table_add(*t, key, .SOME_KIND)` is the shape that needs it.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Kind :: enum { ASSIGN; DECLARATION; }\n\
     Box :: struct (K: Type, V: Type) { k: K; v: V; }\n\
     take_two :: (a: $T, b: T) { print(\"two %\\n\", b); }\n\
     put :: (box: *Box($K, $V), key: K, value: V) { box.k = key; box.v = value; }\n\
     main :: () {\n  \
       take_two(Kind.ASSIGN, .DECLARATION);\n  \
       b: Box(int, Kind);\n  \
       put(*b, 0, .ASSIGN);\n  \
       print(\"box %\\n\", b.v);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "two DECLARATION\nbox ASSIGN\n");
}

#[test]
fn a_return_forwards_every_value_a_call_produced() {
  // `return f();` where `f` returns as many values as the header declares
  // hands all of them on (**L§7.2**), rather than taking the first and
  // defaulting the rest — which a named return list has no defaults for.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     inner :: (n: int) -> (value: int, ok: bool) { return n * 2, n > 0; }\n\
     outer :: (n: int) -> (value: int, ok: bool) { return inner(n); }\n\
     main :: () {\n  \
       value, ok := outer(21);\n  \
       print(\"% %\\n\", value, ok);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "42 true\n");
}

#[test]
fn type_info_lists_only_the_members_a_struct_declares() {
  // A member brought in by `using` is reachable through the member it came
  // through, which is listed, so the reference does not repeat it in
  // `Type_Info_Struct.members` (**L§8.4**, **L§17**). A walker that recurses
  // into a `USING` member would otherwise see the same field several times —
  // which is what made a reflective JSON writer emit one key four times.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Base  :: struct { a: int; }\n\
     Middle :: struct { using base: Base; }\n\
     Outer :: struct { using middle: Middle; b: int; struct { c: int; } }\n\
     main :: () {\n  \
       info := cast(*Type_Info_Struct) type_info(Outer);\n  \
       print(\"%\\n\", info.members.count);\n  \
       for * m: info.members  print(\"[%] %\\n\", m.name, m.flags);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "3\n[middle] USING\n[b] 0\n[] USING\n");
}

#[test]
fn a_named_for_expansion_reached_through_a_bake_iterates() {
  // `for :only_set a` names a `#bake_arguments` over a `for_expansion`, which
  // is how `Bit_Array` writes `only_set`/`only_unset` (**L§7.10**,
  // **L§13.2**). The baked slot is filled by the bake rather than by the loop,
  // so the specialization has to carry that default and the value has to reach
  // the expansion as a constant.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Box :: struct { xs: [4] int; }\n\
     picked :: (b: *Box, body: Code, flags: For_Flags, want_even: bool) #expand {\n  \
       for i: 0..b.xs.count - 1 {\n    \
         if (b.xs[i] % 2 == 0) != want_even  continue;\n    \
         `it := b.xs[i];\n    \
         `it_index := i;\n    \
         #insert body;\n  \
       }\n\
     }\n\
     only_even :: #bake_arguments picked(want_even = true);\n\
     only_odd  :: #bake_arguments picked(want_even = false);\n\
     main :: () {\n  \
       b := Box.{.[1, 2, 3, 4]};\n  \
       for :only_even b  print(\"% \", it);\n  \
       for :only_odd  b  print(\"% \", it);\n  \
       print(\"\\n\");\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "2 4 1 3 \n");
}

#[test]
fn an_alias_of_an_overload_set_is_the_whole_set() {
  // `operator- :: Basic.operator-;` is how `Thread` reaches the one `Basic`
  // declares for `Apollo_Time` without importing every name that module has
  // (**L§7.5**). The alias has no signature of its own, so what it stands for
  // is every declaration the name it was given reaches — and it must not
  // shadow them, which is what made the subtraction stop resolving.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Mod :: #import \"Basic\";\n\
     operator- :: Mod.operator-;\n\
     main :: () {\n  \
       a := current_time_consensus();\n  \
       b := current_time_consensus();\n  \
       print(\"%\\n\", to_milliseconds(b - a) >= 0);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "true\n");
}

#[test]
fn a_polymorphic_parameter_takes_its_type_from_the_default_it_was_given() {
  // `platform_code: $T = 0` left out by the call site still has to say what
  // `T` is, and the only thing left to say it is the header's own default
  // (**L§7.8**) — which is what `File_Async`'s `error(.Incomplete)` needs.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     E :: struct { code: enum { A :: 0; B; }; n: s64; }\n\
     make :: (code: type_of(E.code), n: $T = 0) -> E {\n  \
       r: E = ---;\n  \
       r.code = code;\n  \
       r.n = xx n;\n  \
       return r;\n\
     }\n\
     main :: () {\n  \
       one := make(.B);\n  \
       two := make(.A, 5);\n  \
       print(\"% % % %\\n\", one.code, one.n, two.code, two.n);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "B 0 A 5\n");
}

#[test]
fn a_static_if_contributes_its_statements_to_the_block_around_it() {
  // A `#if` splices the branch it kept into the block it was written in, the
  // way it contributes declarations at file scope (**L§6.10**), so a `defer`
  // inside one belongs to the enclosing block — which is what `Soa`'s
  // `for_expansion` needs to scatter an item back at the end of the loop body
  // rather than the moment the `#if` closes.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     COND :: true;\n\
     main :: () {\n  \
       for i: 0..1 {\n    \
         #if COND {\n      \
           defer print(\"defer %\\n\", i);\n    \
         }\n    \
         print(\"body %\\n\", i);\n  \
       }\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "body 0\ndefer 0\nbody 1\ndefer 1\n");
}

#[test]
fn an_insert_replacement_block_declares_names_of_its_own() {
  // `#insert (remove = { … })` writes its replacement where the `#insert`
  // stands (**L§13.2**), which is what `Bucket_Array`'s `for_expansion` builds
  // a `Bucket_Locator` in. Neither the scope tree nor the checker was looking
  // at the replacements, so anything one declared had nowhere to live.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Holder :: struct { items: [4] int; live: [4] bool; }\n\
     drop :: (h: *Holder, i: int) { h.live[i] = false; }\n\
     for_expansion :: (h: *Holder, body: Code, flags: For_Flags) #expand {\n  \
       for `it, i: h.items {\n    \
         if !h.live[i]  continue;\n    \
         `it_index := i;\n    \
         #insert (remove = { index := i; drop(h, index); }) body;\n  \
       }\n\
     }\n\
     main :: () {\n  \
       h: Holder;\n  \
       for * h.live  it.* = true;\n  \
       for * h.items  it.* = it_index * 3;\n  \
       for h  { if it > 3  remove; }\n  \
       for h  print(\"% \", it);\n  \
       print(\"\\n\");\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "0 3 \n");
}

#[test]
fn a_using_of_a_polymorphic_struct_brings_in_its_parameters() {
  // `operator *[] :: (using array: Bucket_Array, index: int)` divides by
  // `items_per_bucket`, which is the struct's own *parameter* rather than one
  // of its members (**L§6.8**, **L§8.5**).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     Holder :: struct (T: Type, N: int) { count: s64; items: [N] T; }\n\
     take_value :: (using h: Holder) -> int { return N + items.count; }\n\
     take_pointer :: (using h: *Holder) -> int { return N + items.count; }\n\
     main :: () {\n  \
       h: Holder(int, 5);\n  \
       print(\"% %\\n\", take_value(h), take_pointer(*h));\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "10 10\n");
}

// ------------------------------------------------------- the command line ---

#[test]
fn add_and_run_compile_alongside_the_files() {
  // `-add CODE` and `-run EXPR` are `add_build_string` of the text the
  // reference wraps them in (**C§2.1**); the driver adds them itself when no
  // metaprogram is in the way.
  if !linker_is_available() {
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(
    &path,
    format!("{PRELUDE}\nmain :: () {{ put_number(EXTRA); }}\n"),
  )
  .expect("the input should be writable");

  // SAFETY: cargo runs each integration test binary in its own process.
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let mut options = oj_driver::BuildOptions::new();
  options.build_strings = vec![
    String::from("EXTRA :: 7;"),
    String::from("#run put(\"ran\\n\");"),
  ];
  let files = vec![path.clone()];
  let input = oj_driver::Input {
    files: files.clone(),
    strings: oj_driver::command_line_strings(&files, &options),
  };
  let report = oj_driver::run_input(&input, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "7\n");
}

#[test]
fn context_size_is_what_the_context_is_padded_out_to() {
  // `-context_size N` is `Build_Options.context_size_max` (**C§4**), which is
  // what `size_of(#Context)` reports whatever a program imports (**L§10.1**).
  if !linker_is_available() {
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(
    &path,
    format!("{PRELUDE}\nmain :: () {{ put_number(size_of(#Context)); }}\n"),
  )
  .expect("the input should be writable");

  // SAFETY: cargo runs each integration test binary in its own process.
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let mut options = oj_driver::BuildOptions::new();
  options.context_size_max = 8192;
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "8192\n");
}

#[test]
fn a_wide_literal_in_a_shift_takes_the_width_that_asked_for_it() {
  // A shift's result is its left operand's type (**L§5.10**). One written as
  // a literal defaults to `s64` where nothing else asks — `1 << n` is an
  // `s64` — but a declaration that wants a `u64` gets one, rather than the
  // default first and a loss of information afterwards.
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     main :: () {\n  \
       n := 5;\n  \
       m: u64 = 0xffff_ffff_ffff_ffff << n;\n  \
       a: u32 = 0xff00;\n  \
       b: u8 = 0x0f;\n  \
       print(\"% % %\\n\", m, a ^ b, 1 << n);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "18446744073709551584 65295 32\n");
}

#[test]
fn a_global_a_run_reads_has_had_its_initializer_run() {
  // A global whose initializer does not fold is assigned by generated code
  // (**L§12.3**); compile time needs that to have happened before a `#run`
  // reads it. Two runs share one set of globals, so the second sees what the
  // first left rather than a fresh initialization (`docs/spec.md` §6.5).
  let Some(built) = build_and_run(
    "#import \"Basic\";\n\
     make :: () -> [] int { a: [..] int; array_add(*a, 1); return a; }\n\
     TABLE : [] int = make();\n\
     bump :: () -> int { TABLE[0] += 10; return TABLE[0]; }\n\
     A :: #run bump();\n\
     B :: #run bump();\n\
     main :: () { print(\"% % %\\n\", A, B, TABLE[0]); }\n",
  ) else {
    return;
  };
  assert_eq!(built.output, "11 21 1\n");
}

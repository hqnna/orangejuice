//! A Jai program becomes an executable that runs (**M5**), and what its
//! `#run`s left behind is part of it (**M6**).
//!
//! These are the milestones' acceptance tests: the pipeline is driven end to
//! end and the produced program is executed, so what is asserted is its
//! behaviour rather than any intermediate listing.

use std::process::Command;

/// A program that can print without `Basic`: `print` is a polymorphic call
/// over `Any` (M7), so the tests reach `write(2)` the way `Runtime_Support`
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
  let jai_dir = match oj_testsupport::jai_dir() {
    Some(dir) => dir,
    None => {
      eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
      return None;
    }
  };
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
    std::env::set_var(oj_testsupport::JAI_DIR_ENV, &jai_dir);
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
fn a_program_that_needs_a_later_milestone_says_which_one() {
  let Some(jai_dir) = oj_testsupport::jai_dir() else {
    eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
    return;
  };
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, "main :: () { #asm { frobnicate a:, 1; } }\n")
    .expect("the input should be writable");
  unsafe {
    std::env::set_var(oj_testsupport::JAI_DIR_ENV, &jai_dir);
  }

  let report = oj_driver::run(
    &path,
    &oj_driver::BuildOptions::new(),
    oj_driver::Stage::Executable,
    None,
  );
  assert!(report.failed);
  let text = report.diagnostics.join("");
  assert!(text.contains("milestone"), "{text}");
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
  let Some(jai_dir) = oj_testsupport::jai_dir() else {
    eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
    return;
  };
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
    std::env::set_var(oj_testsupport::JAI_DIR_ENV, &jai_dir);
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

// --------------------------------------------------------------- M6 -------
//
// Compile-time execution. What is asserted is the *program's* behaviour, so a
// `#run` that got the wrong answer, ran twice, or did not run at all shows up
// as wrong output rather than as a listing that reads oddly.

/// Builds a program and returns its diagnostics rather than running it, for
/// the cases where the point is that the build fails.
fn diagnostics_of(body: &str) -> Option<String> {
  let jai_dir = match oj_testsupport::jai_dir() {
    Some(dir) => dir,
    None => {
      eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
      return None;
    }
  };
  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, format!("{PRELUDE}\n{body}")).expect("the input should be writable");
  unsafe {
    std::env::set_var(oj_testsupport::JAI_DIR_ENV, &jai_dir);
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

// --------------------------------------------------------------- M6 -------
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

// ------------------------------------------------------------------- M7 ---

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

// ------------------------------------------------------------------- M9 -----

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
  let jai_dir = match oj_testsupport::jai_dir() {
    Some(dir) => dir,
    None => {
      eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
      return None;
    }
  };
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
    std::env::set_var(oj_testsupport::JAI_DIR_ENV, &jai_dir);
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
  let jai_dir = match oj_testsupport::jai_dir() {
    Some(dir) => dir,
    None => {
      eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
      return;
    }
  };
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
    std::env::set_var(oj_testsupport::JAI_DIR_ENV, &jai_dir);
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

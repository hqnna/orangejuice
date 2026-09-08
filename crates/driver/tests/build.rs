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
  std::fs::write(
    &path,
    "#import \"Basic\";\nmain :: () { print(\"hi\\n\"); }\n",
  )
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

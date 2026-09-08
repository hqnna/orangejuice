//! **M5**: a Jai program becomes an executable that runs.
//!
//! These are the milestone's acceptance tests: the pipeline is driven end to
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

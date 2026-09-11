//! The distribution orangejuice ships (`modules/`).
//!
//! `oj` is a Jai distribution of its own: `modules/` sits at the root of this
//! repository, and the compiler finds it from its own binary rather than from
//! anything a caller has to set. What is asserted here is that the modules a
//! program cannot do without — Preload, Basic, Runtime_Support — hold up under
//! a real build.

use std::path::Path;
use std::process::Command;

fn linker_is_available() -> bool {
  let driver = oj_link::driver();
  Command::new(&driver)
    .arg("--version")
    .output()
    .is_ok_and(|output| output.status.success())
}

/// Builds `body` against orangejuice's own modules and runs it.
fn build_and_run(body: &str) -> Option<String> {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return None;
  }

  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("program.jai");
  std::fs::write(&path, body).expect("the input should be writable");

  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe {
    oj_testsupport::use_own_modules();
  }

  let options = oj_driver::BuildOptions::new();
  let report = oj_driver::run(&path, &options, oj_driver::Stage::Executable, None);
  assert!(
    !report.failed,
    "the program should build against our own modules, but:\n{}",
    report.diagnostics.join("")
  );

  let executable = report.executable.expect("a successful build has one");
  let output = Command::new(&executable)
    .output()
    .expect("the produced program should run");
  assert!(
    output.status.success(),
    "the program should run, but exited {:?}",
    output.status.code()
  );
  Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[test]
fn a_program_builds_and_runs_with_no_jai_distribution() {
  // The milestone: Preload, Runtime_Support and Basic are ours, and nothing
  // the reference ships is involved.
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     main :: () { print(\"Hello, %!\\n\", \"world\"); }\n",
  ) else {
    return;
  };
  assert_eq!(output, "Hello, world!\n");
}

#[test]
fn print_formats_what_the_type_table_describes() {
  // `print` decides what to write by walking the `Type_Info` the compiler put
  // in the table, so this is as much a test of the table as of the module.
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     Colour :: enum { RED; GREEN; BLUE; }\n\
     Point :: struct { x: int; y: int; name: string; }\n\
     main :: () {\n  \
       print(\"% % %\\n\", 42, -17, 9223372036854775807);\n  \
       print(\"% %\\n\", true, false);\n  \
       print(\"% %\\n\", Colour.GREEN, Colour.BLUE);\n  \
       print(\"%\\n\", Point.{3, 4, \"origin\"});\n  \
       print(\"%\\n\", int.[1, 2, 3]);\n  \
       print(\"100%%\\n\");\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(
    output,
    "42 -17 9223372036854775807\n\
     true false\n\
     GREEN BLUE\n\
     {3, 4, \"origin\"}\n\
     [1, 2, 3]\n\
     100%\n"
  );
}

#[test]
fn a_context_member_a_module_added_keeps_the_default_it_declared() {
  // `__jai_runtime_init` builds the context from a declaration without a
  // `__jai_runtime_init` builds the context from a declaration without a
  // value, so `#add_context` members arrive with their defaults rather than
  // zeroed (**L§4.6**, **L§10.2**). `Print_Style`'s own defaults are nested
  // two structs deep, which is what makes this worth checking: the pointer
  // base of `default_format_absolute_pointer` is set in the declaration.
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     main :: () {\n  \
       print(\"%\\n\", context.print_style.default_format_absolute_pointer.base);\n  \
       print(\"%\\n\", context.print_style.default_format_array.stop_printing_after_this_many_elements);\n  \
       print(\"%\\n\", 0.5);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(output, "16\n100\n0.5\n");
}

#[test]
fn a_resizable_array_grows_through_the_context_allocator() {
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     main :: () {\n  \
       xs: [..] int;\n  \
       for 1..100  array_add(*xs, it);\n  \
       total := 0;\n  \
       for xs  total += it;\n  \
       print(\"% %\\n\", xs.count, total);\n  \
       array_free(*xs);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(output, "100 5050\n");
}

#[test]
fn temporary_storage_is_reset_in_one_go() {
  // `tprint` allocates out of the arena; resetting hands it all back, so a
  // loop that prints a great many strings does not grow without bound
  // (**L§10.4**).
  let Some(output) = build_and_run(
    "#import \"Basic\";\n\
     main :: () {\n  \
       for 1..1000 {\n    \
         s := tprint(\"%-%\", it, it * 2);\n    \
         if it == 1000  print(\"%\\n\", s);\n    \
         reset_temporary_storage();\n  \
       }\n  \
       print(\"%\\n\", context.temporary_storage.total_bytes_occupied);\n\
     }\n",
  ) else {
    return;
  };
  assert_eq!(output, "1000-2000\n0\n");
}

#[test]
fn the_distribution_is_found_without_being_pointed_at() {
  // What makes `oj` usable on its own: the modules are found from the
  // executable rather than from an environment variable.
  let found = oj_driver::distribution();
  assert!(
    found.is_some_and(|directory| directory.join("modules").join("Preload.jai").is_file()),
    "orangejuice should find the modules it ships"
  );
}

/// The declarations the compiler reads out of Preload by name (**L§17**). A
/// rename here is a rename the compiler cannot follow, so it is worth failing
/// loudly rather than as a mystery somewhere in the type table.
#[test]
fn preload_declares_what_the_compiler_looks_up() {
  let preload = oj_testsupport::modules().join("Preload.jai");
  let text = std::fs::read_to_string(&preload).expect("Preload should be readable");
  for name in [
    "Allocator",
    "Type_Info_Tag",
    "Type_Info_Struct_Member",
    "Stack_Trace_Node",
    "Stack_Trace_Procedure_Info",
    "Source_Code_Location",
    "Any_Struct",
    "__reg",
    "memcmp",
  ] {
    assert!(
      text.contains(name),
      "Preload should declare '{name}', which the compiler looks up by name"
    );
  }
  for field in [
    "runtime_size",
    "offset_in_bytes",
    "offset_into_constant_storage",
    "polymorph_source_struct",
    "array_count",
    "internal_type",
  ] {
    assert!(
      text.contains(field),
      "Preload's type info should carry '{field}', which the type table image writes by name"
    );
  }
  let _: &Path = preload.as_path();
}

#[test]
fn a_crash_names_itself_and_walks_the_stack() {
  // A program that dies of a signal should say what happened rather than
  // leaving a silent core file (**C§13**). The generated entry point installs
  // `Runtime_Support_Crash_Handler` unless `-no_backtrace_on_crash` asks it
  // not to, and the wording is the reference's.
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }

  let directory = tempfile::tempdir().expect("a temporary directory");
  let path = directory.path().join("crash.jai");
  std::fs::write(
    &path,
    "#import \"Basic\";\n\
     read :: (p: *int) -> int { return p.*; }\n\
     main :: () {\n  \
       print(\"before\\n\");\n  \
       p: *int;\n  \
       print(\"%\\n\", read(p));\n\
     }\n",
  )
  .expect("the input should be writable");

  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
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
  assert!(
    !output.status.success(),
    "the program should die of the signal it got"
  );

  let errors = String::from_utf8_lossy(&output.stderr);
  assert!(
    errors.contains("Program Exception: SEGV_MAPERR"),
    "the fault should be named, but stderr was:\n{errors}"
  );
  assert!(
    errors.contains("Null Pointer Exception. Attempt to dereference a null pointer."),
    "a null dereference should say so, but stderr was:\n{errors}"
  );
  assert!(
    errors.lines().filter(|line| line.contains("[0x")).count() >= 2,
    "the stack should be walked, but stderr was:\n{errors}"
  );
}

/// Running the compiler from the directory its own `modules/` sits in is the
/// shape the portable tarball unpacks into, and it used to fail: the import
/// path holds `<dir of the first file>/modules` and `<distribution>/modules`,
/// which are then the same directory spelled relatively and absolutely. The
/// compiler imports `Runtime_Support` on the program's behalf under one
/// spelling and the program imports it under the other, and two copies of one
/// module declare two nominal types of the same name — which surfaces far from
/// the cause, as `Type wanted: *Temporary_Storage; type given:
/// *Temporary_Storage`.
#[test]
fn a_module_reached_two_ways_is_one_module() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let directory = tempfile::tempdir().expect("a temporary directory");
  // A distribution laid out the way the release tarball is: `modules/` beside
  // the program being compiled.
  let modules = directory.path().join("modules");
  std::os::unix::fs::symlink(oj_testsupport::modules(), &modules)
    .expect("the modules link should be creatable");
  let path = directory.path().join("main.jai");
  std::fs::write(
    &path,
    "#import \"Basic\";\nmain :: () { print(\"one module\\n\"); }\n",
  )
  .expect("the input should be writable");

  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe { std::env::set_var(oj_testsupport::MODULES_ENV, &modules) };

  let options = oj_driver::BuildOptions {
    output_path: Some(directory.path().to_path_buf()),
    ..oj_driver::BuildOptions::new()
  };
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
  assert_eq!(String::from_utf8_lossy(&output.stdout), "one module\n");
}

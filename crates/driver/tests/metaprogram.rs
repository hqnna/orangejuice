//! A metaprogram drives the build (**M8**).
//!
//! The `Compiler` module of the distribution declares its procedures
//! `#compiler` and the compiler answers them; these tests declare the same
//! headers in the program itself, so that what is exercised is the compiler's
//! half rather than the distribution's — importing the real module pulls in
//! `Basic` and `Thread`, whose `#asm` blocks are M9's.

use std::path::{Path, PathBuf};

/// The `Compiler` declarations these tests use, spelled exactly as
/// `vendor/jai/modules/Compiler/Compiler.jai` spells them.
const COMPILER: &str = "\
Workspace :: s64;

Build_Options :: struct {
  output_executable_name: string;
  output_path:            string;
}

Report :: enum u8 { ERROR; ERROR_CONTINUABLE; WARNING; INFO; }
Version_Info :: struct { major: s32; minor: s32; micro: s32; }

compiler_create_workspace :: (name := \"\") -> Workspace #compiler;
compiler_destroy_workspace :: (w: Workspace) #compiler;
get_name :: (w: Workspace = -1) -> string #compiler;
get_build_options :: (w: Workspace = -1) -> Build_Options #compiler;
set_build_options :: (options: Build_Options, w: Workspace = -1, loc := #caller_location) #compiler;
add_build_file :: (filename: string, w: Workspace, loc := #caller_location) #compiler;
add_build_string :: (data: string, w: Workspace, code := #code,null, loc := #caller_location) #compiler;
compiler_report :: (message: string, loc := #caller_location, mode := Report.ERROR) #compiler;
compiler_get_version_info :: (version_info_return: *Version_Info) -> string #compiler;
compiler_get_base_path :: () -> string #compiler;
";

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
    std::fs::write(self.directory.path().join(name), source).expect("the fixture is writable");
    self
  }

  fn path(&self, name: &str) -> PathBuf {
    self.directory.path().join(name)
  }
}

/// Builds `main.jai`, with the `Compiler` declarations in front of `body`.
fn build(fixture: &Fixture, body: &str) -> Option<oj_driver::Report> {
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
  fixture.write("main.jai", &format!("{COMPILER}\n{body}"));
  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe {
    std::env::set_var(oj_testsupport::JAI_DIR_ENV, &jai_dir);
  }
  let options = oj_driver::BuildOptions::new();
  Some(oj_driver::run(
    &fixture.path("main.jai"),
    &options,
    oj_driver::Stage::Executable,
    None,
  ))
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

fn assert_built(report: &oj_driver::Report, path: &Path) {
  assert!(
    !report.failed,
    "the program should build, but:\n{}",
    report.diagnostics.join("")
  );
  assert!(
    path.exists(),
    "{} should have been produced; diagnostics:\n{}",
    path.display(),
    report.diagnostics.join("")
  );
}

#[test]
fn a_workspace_compiles_the_file_a_metaprogram_added_to_it() {
  let fixture = Fixture::new();
  fixture.write("other.jai", "main :: () {}\n");
  let Some(report) = build(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"target\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"generated\";\n\
       set_build_options(options, w);\n\
       add_build_file(\"other.jai\", w);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("generated"));
  assert!(fixture.path("main").exists(), "the host program too");
}

#[test]
fn a_workspace_compiles_the_string_a_metaprogram_added_to_it() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"target\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"from_a_string\";\n\
       set_build_options(options, w);\n\
       add_build_string(\"main :: () { }\", w);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("from_a_string"));
}

#[test]
fn a_destroyed_workspace_is_never_built() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"target\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"never\";\n\
       set_build_options(options, w);\n\
       add_build_string(\"main :: () { }\", w);\n\
       compiler_destroy_workspace(w);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert!(!fixture.path("never").exists());
}

#[test]
fn a_workspace_remembers_the_name_it_was_created_with() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"the name\");\n\
       compiler_report(get_name(w), mode = .INFO);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert!(
    report
      .diagnostics
      .iter()
      .any(|line| line.contains("Info: the name")),
    "{:?}",
    report.diagnostics
  );
}

#[test]
fn compiler_get_version_info_names_the_reference_distribution() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n\
       info: Version_Info;\n\
       text := compiler_get_version_info(*info);\n\
       compiler_report(text, mode = .INFO);\n\
       if info.major != 0  compiler_report(\"wrong major\");\n\
       if info.minor != 2  compiler_report(\"wrong minor\");\n\
       if info.micro != 9  compiler_report(\"wrong micro\");\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert!(
    report
      .diagnostics
      .iter()
      .any(|line| line.contains(oj_driver::JAI_VERSION)),
    "{:?}",
    report.diagnostics
  );
}

#[test]
fn an_error_a_metaprogram_reports_fails_the_compilation() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run compiler_report(\"the metaprogram is unhappy\");\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(report.failed);
  assert!(
    report
      .diagnostics
      .iter()
      .any(|line| line.contains("Error: the metaprogram is unhappy")),
    "{:?}",
    report.diagnostics
  );
  assert!(!fixture.path("main").exists());
}

#[test]
fn compiler_get_base_path_names_the_distribution() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run compiler_report(compiler_get_base_path(), mode = .INFO);\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert!(
    report.diagnostics.iter().any(|line| line.contains("jai")),
    "{:?}",
    report.diagnostics
  );
}

/// The message structs a metaprogram casts what it is handed to (**C§3.2**),
/// spelled as `Compiler.jai` spells them.
const MESSAGES: &str = "\
Intercept_Flags :: enum_flags u32 { SKIP_ALL :: 0x3f; }

Message :: struct {
  kind: enum u8 {
    UNINITIALIZED :: 0;
    FILE; IMPORT; FAILED_IMPORT; PHASE; TYPECHECKED; COMPLETE; DEBUG_DUMP; ERROR; PERFORMANCE_REPORT;
  }
  workspace: Workspace;
}

Message_Import :: struct {
  #as using m: Message;
  module_type: enum u8 { UNINITIALIZED :: 0; PRELOAD :: 1; RUNTIME_SUPPORT :: 2; MAIN_PROGRAM :: 3; FILE :: 4; };
  module_name: string;
  fully_pathed_filename: string;
}

Message_Phase :: struct {
  #as using m: Message;
  phase: enum u32 {
    ALL_SOURCE_CODE_PARSED :: 0;
    TYPECHECKED_ALL_WE_CAN :: 1;
    ALL_TARGET_CODE_BUILT  :: 2;
    PRE_WRITE_EXECUTABLE   :: 3;
    POST_WRITE_EXECUTABLE  :: 4;
    READY_FOR_CUSTOM_LINK_COMMAND :: 5;
  };
  executable_name: string;
  executable_write_failed := false;
  linker_exit_code: s32;
  num_items_waiting_to_typecheck: s32;
  compiler_generated_object_files: [] string;
  support_object_files:            [] string;
  system_libraries:                [] string;
  user_libraries:                  [] string;
}

Message_Complete :: struct {
  #as using m: Message;
  error_code: enum u8 { NONE :: 0; COMPILATION_FAILED :: 1; COMPILER_SHUTDOWN :: 2; }
}

compiler_begin_intercept :: (w: Workspace, flags: Intercept_Flags = 0) #compiler;
compiler_end_intercept :: (w: Workspace) #compiler;
compiler_wait_for_message :: () -> *Message #compiler;
";

/// Builds a program that also has the message declarations in front of it.
fn build_watching(fixture: &Fixture, body: &str) -> Option<oj_driver::Report> {
  build(fixture, &format!("{MESSAGES}\n{body}"))
}

/// A program for a workspace to compile that says so, once, through its own
/// `compiler_report`. It is a here-string in the source that adds it, so the
/// nested program can have quotes of its own.
const WATCHED_PROGRAM: &str = "\
WATCHED_PROGRAM :: #string OJ_DONE
Report :: enum u8 { ERROR; ERROR_CONTINUABLE; WARNING; INFO; }
compiler_report :: (message: string, loc := #caller_location, mode := Report.ERROR) #compiler;
#run compiler_report(\"compiled once\", mode = .INFO);
main :: () { }
OJ_DONE
";

#[test]
fn a_metaprogram_watches_the_workspace_it_created_compile() {
  let fixture = Fixture::new();
  let Some(report) = build_watching(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"target\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"watched\";\n\
       set_build_options(options, w);\n\
       compiler_begin_intercept(w);\n\
       add_build_string(\"main :: () { }\", w);\n\
       imports := 0;\n\
       files := 0;\n\
       built := false;\n\
       while true {\n\
         message := compiler_wait_for_message();\n\
         if message.kind == .IMPORT   imports += 1;\n\
         if message.kind == .FILE     files += 1;\n\
         if message.kind == .PHASE {\n\
           phase := cast(*Message_Phase) message;\n\
           if phase.phase == .POST_WRITE_EXECUTABLE  built = true;\n\
         }\n\
         if message.kind == .COMPLETE {\n\
           complete := cast(*Message_Complete) message;\n\
           if complete.error_code != .NONE  compiler_report(\"the workspace failed\");\n\
           break;\n\
         }\n\
       }\n\
       compiler_end_intercept(w);\n\
       if imports == 0  compiler_report(\"no imports were reported\");\n\
       if files == 0    compiler_report(\"no files were reported\");\n\
       if !built        compiler_report(\"the executable phase was never reported\");\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("watched"));
}

#[test]
fn a_watched_workspace_is_not_compiled_a_second_time() {
  let fixture = Fixture::new();
  let Some(report) = build_watching(
    &fixture,
    &(WATCHED_PROGRAM.to_string()
      + "#run {\n\
       w := compiler_create_workspace(\"target\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"once\";\n\
       set_build_options(options, w);\n\
       compiler_begin_intercept(w);\n\
       add_build_string(WATCHED_PROGRAM, w);\n\
       while true {\n\
         message := compiler_wait_for_message();\n\
         if message.kind == .COMPLETE  break;\n\
       }\n\
       compiler_end_intercept(w);\n\
     }\n\
     main :: () {}\n"),
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("once"));
  // The workspace compiled once: the `#run` inside it reported exactly one
  // line, and nothing rebuilt it afterwards.
  let reported = report
    .diagnostics
    .iter()
    .filter(|line| line.contains("compiled once"))
    .count();
  assert_eq!(reported, 1, "{:?}", report.diagnostics);
}

#[test]
fn a_workspace_that_fails_reports_a_failed_completion() {
  let fixture = Fixture::new();
  let Some(report) = build_watching(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"target\");\n\
       compiler_begin_intercept(w);\n\
       add_build_string(\"main :: () { undefined_name(); }\", w);\n\
       failed := false;\n\
       while true {\n\
         message := compiler_wait_for_message();\n\
         if message.kind == .COMPLETE {\n\
           complete := cast(*Message_Complete) message;\n\
           failed = complete.error_code == .COMPILATION_FAILED;\n\
           break;\n\
         }\n\
       }\n\
       compiler_end_intercept(w);\n\
       if !failed  compiler_report(\"the failure was not reported\");\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(
    report
      .diagnostics
      .iter()
      .any(|line| line.contains("undefined_name")),
    "{:?}",
    report.diagnostics
  );
  assert!(
    !report
      .diagnostics
      .iter()
      .any(|line| line.contains("the failure was not reported")),
    "{:?}",
    report.diagnostics
  );
}

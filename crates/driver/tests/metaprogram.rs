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

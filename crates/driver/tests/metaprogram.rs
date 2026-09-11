//! A metaprogram drives the build (**M8**).
//!
//! The `Compiler` module of the distribution declares its procedures
//! `#compiler` and the compiler answers them; these tests declare the same
//! headers in the program itself, so that what is exercised is the compiler's
//! half rather than the distribution's. The last one imports the real module,
//! which reaches `Basic` and `Thread` and everything M9 assembles.

use std::path::{Path, PathBuf};

/// The `Compiler` declarations these tests use, spelled exactly as
/// `modules/Compiler/Compiler.jai` spells them.
const COMPILER: &str = "\
Workspace :: s64;

Build_Options :: struct {
  output_type: enum u8 {
    NO_OUTPUT       :: 0;
    EXECUTABLE      :: 1;
    DYNAMIC_LIBRARY :: 2;
    STATIC_LIBRARY  :: 3;
    OBJECT_FILE     :: 4;
  } = .EXECUTABLE;
  output_executable_name: string;
  output_path:            string;
  append_executable_filename_extension := true;
  use_custom_link_command := false;
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

Build_Options_During_Compile :: struct {
  do_output := true;
  write_added_strings := true;
  append_executable_filename_extension := true;
  interactive_bytecode_debugger := false;
  append_linker_arguments: [] string;
  output_executable_name: string;
  output_path           : string;
}
set_build_options_dc :: (options: Build_Options_During_Compile, w: Workspace = -1) #compiler;
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
    let path = self.directory.path().join(name);
    if let Some(directory) = path.parent() {
      std::fs::create_dir_all(directory).expect("the fixture is writable");
    }
    std::fs::write(path, source).expect("the fixture is writable");
    self
  }

  fn path(&self, name: &str) -> PathBuf {
    self.directory.path().join(name)
  }
}

/// Builds `main.jai`, with the `Compiler` declarations in front of `body`.
fn build(fixture: &Fixture, body: &str) -> Option<oj_driver::Report> {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return None;
  }
  // A body that imports the real module must not also get the headers, or the
  // program would declare `Build_Options` twice.
  let source = match body.contains("#import \"Compiler\"") {
    true => body.to_string(),
    false => format!("{COMPILER}\n{body}"),
  };
  fixture.write("main.jai", &source);
  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe {
    oj_testsupport::use_own_modules();
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

/// `remap_import` and `get_current_workspace`, spelled as the distribution
/// spells them.
const REMAPPING: &str = "\
remap_import :: (w: Workspace, host_module_name: string, import_name: string, replacement_name: string) #compiler;
get_current_workspace :: () -> Workspace #compiler;
";

#[test]
fn a_metaprogram_remaps_an_import_of_the_workspace_it_creates() {
  let fixture = Fixture::new();
  fixture.write(
    "other.jai",
    "#import \"Dimwit\";\nmain :: () { greet(); }\n",
  );
  std::fs::create_dir_all(fixture.path("modules")).expect("the fixture is writable");
  fixture.write("modules/Flathead.jai", "greet :: () {}\n");
  let Some(report) = build(
    &fixture,
    &(REMAPPING.to_string()
      + "#run {\n\
       w := compiler_create_workspace(\"target\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"remapped\";\n\
       set_build_options(options, w);\n\
       remap_import(w, \"\", \"Dimwit\", \"Flathead\");\n\
       add_build_file(\"other.jai\", w);\n\
     }\n\
     main :: () {}\n"),
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("remapped"));
}

#[test]
fn get_current_workspace_names_the_program_being_compiled() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &(REMAPPING.to_string()
      + "#run {\n\
       here := get_current_workspace();\n\
       w := compiler_create_workspace(\"target\");\n\
       if here == w  compiler_report(\"a new workspace should not be this one\");\n\
       if here == 0  compiler_report(\"compile-time code belongs to a workspace\");\n\
     }\n\
     main :: () {}\n"),
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
}

#[test]
fn a_workspace_produces_the_output_type_its_options_ask_for() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"object\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"just_an_object\";\n\
       options.output_type = .OBJECT_FILE;\n\
       set_build_options(options, w);\n\
       add_build_string(\"main :: () { }\", w);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("just_an_object.o"));
  assert!(!fixture.path("just_an_object").exists());
}

#[test]
fn no_output_produces_nothing() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"quiet\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"nothing\";\n\
       options.output_type = .NO_OUTPUT;\n\
       set_build_options(options, w);\n\
       add_build_string(\"main :: () { }\", w);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert!(!fixture.path("nothing").exists());
}

#[test]
fn a_workspace_can_be_a_library_with_no_main() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"library\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"shared\";\n\
       options.output_type = .DYNAMIC_LIBRARY;\n\
       set_build_options(options, w);\n\
       add_build_string(\"#program_export answer :: () -> int { return 42; }\", w);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("shared.so"));
}

#[test]
fn set_build_options_dc_can_turn_this_compilations_own_output_off() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run set_build_options_dc(.{do_output = false});\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert!(
    !fixture.path("main").exists(),
    "a metaprogram that asked for no output should get none"
  );
}

#[test]
fn set_build_options_dc_renames_the_output() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run set_build_options_dc(.{output_executable_name = \"renamed\"});\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("renamed"));
}

/// The `Compiler` module itself, rather than the headers the tests above
/// declare: importing it reaches `Basic` and `Thread`, whose `#asm` blocks the
/// back end assembles since M9.
#[test]
fn a_metaprogram_imports_the_compiler_module_of_the_distribution() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "Basic :: #import \"Basic\";\n\
     C :: #import \"Compiler\";\n\
     drive :: () {\n\
       w := C.compiler_create_workspace(\"target\");\n\
       options := C.get_build_options(w);\n\
       options.output_executable_name = \"generated\";\n\
       C.set_build_options(options, w);\n\
       C.add_build_string(\"main :: () { }\", w);\n\
       Basic.print(\"workspace %\\n\", w);\n\
     }\n\
     #run drive();\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("main"));
  assert!(
    fixture.path("generated").exists(),
    "the workspace should have produced its own executable; diagnostics:\n{}",
    report.diagnostics.join("")
  );
}

/// `get_build_options()` on the compilation the program itself is: it exists
/// before the compiler has folded the defaults, so its options are empty and
/// what it hands back has to be the defaults rather than the caller's own
/// storage (**C§3.1**). A metaprogram reports what it saw, since that is what
/// reaches the driver.
#[test]
fn the_options_of_the_programs_own_workspace_are_the_defaults() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "Basic :: #import \"Basic\";\n\
     C :: #import \"Compiler\";\n\
     look :: () {\n\
       options := C.get_build_options();\n\
       C.compiler_report(\n\
         Basic.tprint(\"saw count % type %\", options.compile_time_command_line.count,\n\
                      options.output_type),\n\
         mode = .WARNING);\n\
     }\n\
     #run look();\n\
     main :: () {}\n",
  ) else {
    return;
  };
  let output = report.diagnostics.join("");
  assert!(output.contains("saw count 0 type EXECUTABLE"), "{output}");
}

/// The arguments after a lone `-` reach a metaprogram through
/// `Build_Options.compile_time_command_line` (**C§2.1**).
#[test]
fn the_compile_time_command_line_reaches_a_metaprogram() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let fixture = Fixture::new();
  fixture.write(
    "main.jai",
    "Basic :: #import \"Basic\";\n\
     C :: #import \"Compiler\";\n\
     look :: () {\n\
       for arg: C.get_build_options().compile_time_command_line {\n\
         C.compiler_report(Basic.tprint(\"saw %\", arg), mode = .WARNING);\n\
       }\n\
     }\n\
     #run look();\n\
     main :: () {}\n",
  );
  // SAFETY: as in `build`.
  unsafe {
    oj_testsupport::use_own_modules();
  }
  let mut options = oj_driver::BuildOptions::new();
  options.compile_time_command_line = vec![String::from("debug"), String::from("release")];
  let report = oj_driver::run(
    &fixture.path("main.jai"),
    &options,
    oj_driver::Stage::Executable,
    None,
  );
  let output = report.diagnostics.join("");
  assert!(output.contains("saw debug"), "{output}");
  assert!(output.contains("saw release"), "{output}");
}

/// A metaprogram that turns the output off is the whole program: there is
/// nothing to lower and no entry point to ask for (**C§4**).
#[test]
fn a_program_that_outputs_nothing_needs_no_entry_point() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "C :: #import \"Compiler\";\n\
     drive :: () { C.set_build_options_dc(.{do_output = false}); }\n\
     #run drive();\n",
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert!(!fixture.path("main").exists(), "nothing should be written");
  // A metaprogram watching this compilation is told it compiled: producing
  // nothing is what it asked for, not a failure (**C§3.2**).
  assert!(
    !report.compiled.failed,
    "a workspace that produces nothing still compiled"
  );
}

/// The `Code_*` declarations the export tests read the compiler's answers
/// with, spelled the way `Compiler.jai` spells them (**C§5.3**).
const NODES: &str = "\
Code_Node :: struct {
  kind: enum u8 {
    UNINITIALIZED :: 0;
    BLOCK :: 1; LITERAL :: 2; IDENT :: 3; UNARY_OPERATOR :: 4; BINARY_OPERATOR :: 5;
    PROCEDURE_BODY :: 6; PROCEDURE_CALL :: 7; RETURN :: 14; DECLARATION :: 25; PROCEDURE_HEADER :: 19;
    STRUCT :: 20;
  }
  node_flags: u32;
  type: *void;
  enclosing_load: *void;
  l0, c0, l1, c1: s32;
  serial: s64;
}

Code_Scope_Entry :: struct {
  #as using base: Code_Node;
  name: string;
  import_target: *void;
}

Code_Declaration :: struct {
  #as using entry: Code_Scope_Entry;
  type_inst:  *void;
  expression: *Code_Node;
  flags: u32;
  alignment_expression: *void;
  notes: [] *void;
  program_export_name: string;
}

Code_Binary_Operator :: struct {
  #as using base: Code_Node;
  operator_type: s32;
  flags: u16;
  left:  *Code_Node;
  right: *Code_Node;
}

Typechecked :: struct {
  expression:     *Code_Node;
  subexpressions: [] *Code_Node;
}

Message_Typechecked :: struct {
  #as using m: Message;
  declarations:      [] Typechecked;
  procedure_headers: [] Typechecked;
  procedure_bodies:  [] Typechecked;
  structs:           [] Typechecked;
  others:            [] Typechecked;
  all:               [] Typechecked;
}

compiler_get_nodes :: (code: Code) -> (root: *Code_Node, expressions: [] *Code_Node) #compiler;
";

#[test]
fn compiler_get_nodes_hands_back_the_tree_a_code_names() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{NODES}\n\
       #run {{\n  \
         root, expressions := compiler_get_nodes(#code 1 + 2);\n  \
         if root == null  compiler_report(\"the code has no root\");\n  \
         if root.kind != .BINARY_OPERATOR  compiler_report(\"the root is not the operator\");\n  \
         if expressions.count != 3  compiler_report(\"the tree is not three nodes\");\n  \
         if root.l0 == 0  compiler_report(\"the root has no line\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("main"));
}

#[test]
fn a_watching_metaprogram_is_sent_the_declarations_that_typechecked() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n{NODES}\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"typechecked\";\n  \
         set_build_options(options, w);\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(\"MARKER :: 42;\\nmain :: () {{ }}\", w);\n  \
         found := false;\n  \
         bodies := 0;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .TYPECHECKED {{\n      \
             batch := cast(*Message_Typechecked) message;\n      \
             for batch.declarations {{\n        \
               declaration := cast(*Code_Declaration) it.expression;\n        \
               if declaration.name == \"MARKER\"  found = true;\n      \
             }}\n      \
             bodies += batch.procedure_bodies.count;\n      \
             if batch.all.count < batch.declarations.count  compiler_report(\"all is short\");\n    \
             }}\n    \
           if message.kind == .COMPLETE  break;\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if !found     compiler_report(\"the added declaration was never reported\");\n  \
         if bodies == 0  compiler_report(\"no procedure bodies were reported\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("typechecked"));
}

#[test]
fn the_distributions_default_metaprogram_drives_the_build() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe { oj_testsupport::use_own_modules() };
  let metaprogram =
    oj_driver::default_metaprogram().expect("the distribution ships a Default_Metaprogram");

  let fixture = Fixture::new();
  fixture.write(
    "greet.jai",
    "#import \"Basic\";\nmain :: () { print(\"driven\\n\"); }\n",
  );
  let report = oj_driver::run_through_metaprogram(
    &metaprogram,
    &[fixture.path("greet.jai")],
    &[],
    &oj_driver::BuildOptions::new(),
    oj_driver::Stage::Executable,
  );
  assert!(
    !report.failed,
    "the metaprogram should build the program it was given, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = report
    .executable
    .expect("the workspace the metaprogram created produced an executable");
  let output = std::process::Command::new(&executable)
    .output()
    .expect("the produced program runs");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "driven\n");
}

#[test]
fn a_program_the_default_metaprogram_cannot_compile_fails_the_build() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  // SAFETY: as above.
  unsafe { oj_testsupport::use_own_modules() };
  let metaprogram =
    oj_driver::default_metaprogram().expect("the distribution ships a Default_Metaprogram");

  let fixture = Fixture::new();
  fixture.write("wrong.jai", "main :: () { x: int = \"no\"; }\n");
  let report = oj_driver::run_through_metaprogram(
    &metaprogram,
    &[fixture.path("wrong.jai")],
    &[],
    &oj_driver::BuildOptions::new(),
    oj_driver::Stage::Executable,
  );
  assert!(report.failed, "the workspace failed, so the build did");
  assert!(
    report
      .diagnostics
      .iter()
      .any(|diagnostic| diagnostic.contains("Type mismatch")),
    "the workspace's own diagnostics reach the compiler's output:\n{}",
    report.diagnostics.join("")
  );
}

/// Lowers a library and hands back its IR listing, so that what it exports can
/// be read off the symbols.
fn library_listing(source: &str, runtime_support: oj_driver::RuntimeSupport) -> String {
  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe { oj_testsupport::use_own_modules() };
  let fixture = Fixture::new();
  fixture.write("library.jai", source);
  let options = oj_driver::BuildOptions {
    output_type: oj_link::OutputType::DynamicLibrary,
    runtime_support,
    ..oj_driver::BuildOptions::new()
  };
  let report = oj_driver::run(
    &fixture.path("library.jai"),
    &options,
    oj_driver::Stage::Ir,
    None,
  );
  assert!(
    !report.failed,
    "the library should lower, but:\n{}",
    report.diagnostics.join("")
  );
  report.output
}

#[test]
fn a_library_takes_the_runtime_init_its_build_options_ask_for() {
  const SOURCE: &str = "#program_export\ngreet :: () { }\n";
  let listing = library_listing(SOURCE, oj_driver::RuntimeSupport::OnlyInit);
  assert!(
    listing.contains("procedure __jai_runtime_init"),
    "a library that takes Runtime_Support's init exports it under its own name"
  );
  assert!(listing.contains("procedure greet"), "and its own exports");
}

#[test]
fn a_library_that_omits_runtime_support_defines_no_init() {
  const SOURCE: &str = "#program_export\ngreet :: () { }\n";
  let listing = library_listing(SOURCE, oj_driver::RuntimeSupport::Omit);
  assert!(
    !listing.contains("procedure __jai_runtime_init"),
    "OMIT means neither the entry point nor the runtime's init is defined"
  );
  assert!(listing.contains("procedure greet"));
}

#[test]
fn a_workspace_with_a_custom_link_command_stops_at_its_objects() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n\
       compiler_custom_link_command_is_complete :: (w: Workspace) #compiler;\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"linked_by_hand\";\n  \
         options.use_custom_link_command = true;\n  \
         set_build_options(options, w);\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(\"main :: () {{ }}\", w);\n  \
         asked := false;\n  \
         wrote := false;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .PHASE {{\n      \
             phase := cast(*Message_Phase) message;\n      \
             if phase.phase == .READY_FOR_CUSTOM_LINK_COMMAND {{\n        \
               asked = true;\n        \
               if phase.compiler_generated_object_files.count == 0  compiler_report(\"no objects were reported\");\n        \
               compiler_custom_link_command_is_complete(w);\n      \
             }}\n      \
             if phase.phase == .POST_WRITE_EXECUTABLE  wrote = true;\n    \
           }}\n    \
           if message.kind == .COMPLETE  break;\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if !asked  compiler_report(\"the link was never handed over\");\n  \
         if wrote   compiler_report(\"the compiler linked it anyway\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
  assert!(
    !fixture.path("linked_by_hand").exists(),
    "the compiler stops at the object it made; linking is the metaprogram's"
  );
}

/// The `Message_Failed_Import` half of the message declarations, plus
/// `provide_import`, spelled the way `Compiler.jai` spells them.
const IMPORTS: &str = "\
Message_Failed_Import :: struct {
  #as using m: Message;
  status: enum u8 { UNINITIALIZED :: 0; BLOCKED :: 1; NOT_FOUND :: 2; }
  host_module_name:   string;
  target_module_name: string;
  import_code: *void;
}

Provided_Import_Type :: enum u8 { SHORT_NAME :: 0; PATH_TO_FILE :: 1; PATH_TO_DIRECTORY :: 2; FULL_TEXT :: 3; }
provide_import :: (w: Workspace, message: *Message_Failed_Import, type: Provided_Import_Type, value: string) #compiler;
";

#[test]
fn a_metaprogram_answers_an_import_the_compiler_could_not_find() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n{IMPORTS}\n\
       PROGRAM :: #string OJ_DONE\n\
       #import \"Nowhere\";\n\
       main :: () {{ }}\n\
       OJ_DONE\n\
       REPLACEMENT :: #string OJ_DONE\n\
       #scope_export\n\
       answered :: () {{ }}\n\
       OJ_DONE\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"provided\";\n  \
         set_build_options(options, w);\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(PROGRAM, w);\n  \
         asked := 0;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .FAILED_IMPORT {{\n      \
             failed := cast(*Message_Failed_Import) message;\n      \
             if failed.target_module_name != \"Nowhere\"  compiler_report(\"the wrong import was reported\");\n      \
             if failed.status != .NOT_FOUND  compiler_report(\"the wrong status was reported\");\n      \
             if failed.import_code == null   compiler_report(\"the #import was not exported\");\n      \
             asked += 1;\n      \
             if asked == 1  provide_import(w, failed, .FULL_TEXT, REPLACEMENT);\n    \
           }}\n    \
           if message.kind == .COMPLETE {{\n      \
             complete := cast(*Message_Complete) message;\n      \
             if complete.error_code != .NONE  compiler_report(\"the workspace failed anyway\");\n      \
             break;\n    \
           }}\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if asked != 1  compiler_report(\"the import was reported more than once\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("provided"));
}

#[test]
fn a_metaprogram_answers_an_import_it_blocked_itself() {
  let fixture = Fixture::new();
  fixture.write("modules/Real.jai", "#scope_export\nreal :: () { }\n");
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n{IMPORTS}\n\
       remap_import :: (w: Workspace, host_module_name: string, import_name: string, replacement_name: string) #compiler;\n\
       PROGRAM :: #string OJ_DONE\n\
       #import \"Real\";\n\
       main :: () {{ real(); }}\n\
       OJ_DONE\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"unblocked\";\n  \
         set_build_options(options, w);\n  \
         remap_import(w, \"\", \"Real\", \"\");\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(PROGRAM, w);\n  \
         blocked := false;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .FAILED_IMPORT {{\n      \
             failed := cast(*Message_Failed_Import) message;\n      \
             if failed.status == .BLOCKED && !blocked {{\n        \
               blocked = true;\n        \
               provide_import(w, failed, .SHORT_NAME, \"Real\");\n      \
             }}\n    \
           }}\n    \
           if message.kind == .COMPLETE  break;\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if !blocked  compiler_report(\"the blocked import was never reported\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("unblocked"));
}

#[test]
fn get_type_gives_back_the_type_a_type_info_belongs_to() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "\
     get_type :: (ti: *Type_Info) -> Type #compiler;\n\
     compiler_set_memory_breakpoint :: (pointer: *void) #compiler;\n\
     developer_debug :: (x: *void) #compiler;\n\
     Type_Info_Flags :: enum u32 { NO_TYPE_INFO :: 0x1; }\n\
     compiler_set_type_info_flags :: (type: Type, flags: Type_Info_Flags) #compiler;\n\
     compiler_report_errors_for_unresolved_identifiers :: (filename: string, w: s64 = -1) #compiler;\n\
     #run {\n  \
       info := type_info(float64);\n  \
       if get_type(info) != float64  compiler_report(\"get_type did not give the type back\");\n  \
       compiler_set_type_info_flags(float64, .NO_TYPE_INFO);\n  \
       compiler_set_memory_breakpoint(null);\n  \
       developer_debug(null);\n  \
       compiler_report_errors_for_unresolved_identifiers(\"nowhere.txt\");\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("main"));
}

#[test]
fn an_import_may_be_answered_only_once() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n{IMPORTS}\n\
       PROGRAM :: #string OJ_DONE\n\
       #import \"Nowhere\";\n\
       main :: () {{ }}\n\
       OJ_DONE\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"answered_once\";\n  \
         set_build_options(options, w);\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(PROGRAM, w);\n  \
         answers := 0;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .FAILED_IMPORT {{\n      \
             failed := cast(*Message_Failed_Import) message;\n      \
             answers += 1;\n      \
             provide_import(w, failed, .SHORT_NAME, \"Still_Nowhere\");\n    \
           }}\n    \
           if message.kind == .COMPLETE  break;\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if answers < 2  compiler_report(\"the replacement should fail too, and be reported\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  // The workspace never compiles: the replacement is not there either, and a
  // second answer for the same import is ignored, so this terminates.
  assert!(report.failed, "the workspace could not be compiled");
  assert!(
    report
      .diagnostics
      .iter()
      .any(|diagnostic| diagnostic.contains("Still_Nowhere")),
    "the replacement is what was looked for the second time:\n{}",
    report.diagnostics.join("")
  );
}

#[test]
fn compiler_get_struct_location_names_where_the_struct_was_written() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "compiler_get_struct_location :: (w: Workspace, info: *Type_Info_Struct) -> Source_Code_Location #compiler;\n\
     get_current_workspace :: () -> Workspace #compiler;\n\
     Point :: struct { x: float; y: float; }\n\
     Later :: struct { z: float; }\n\
     #run {\n  \
       w := get_current_workspace();\n  \
       first  := compiler_get_struct_location(w, cast(*Type_Info_Struct) type_info(Point));\n  \
       second := compiler_get_struct_location(w, cast(*Type_Info_Struct) type_info(Later));\n  \
       if first.fully_pathed_filename == \"\"  compiler_report(\"the file a struct is in is reported\");\n  \
       if first.character_number != 10  compiler_report(\"and the character the struct starts at\");\n  \
       if second.line_number != first.line_number + 1  compiler_report(\"and its own line, not another struct's\");\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("main"));
}

#[test]
fn a_metaprogram_adds_a_string_to_the_scope_a_message_names() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n\
       add_build_string_by_message :: (data: string, w: Workspace, message: *Message, loc := #caller_location) #compiler \"add_build_string_scoped_by_message\";\n\
       PROGRAM :: #string OJ_DONE\n\
       main :: () {{ if ADDED != 7  bad(); }}\n\
       bad :: () {{ }}\n\
       OJ_DONE\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"scoped\";\n  \
         set_build_options(options, w);\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(PROGRAM, w);\n  \
         added := false;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .FILE && !added {{\n      \
             added = true;\n      \
             add_build_string_by_message(\"ADDED :: 7;\", w, message);\n    \
           }}\n    \
           if message.kind == .COMPLETE {{\n      \
             complete := cast(*Message_Complete) message;\n      \
             if complete.error_code != .NONE  compiler_report(\"the string never reached the file's scope\");\n      \
             break;\n    \
           }}\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if !added  compiler_report(\"no file was reported to add to\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("scoped"));
}

#[test]
fn a_metaprogram_makes_a_procedure_live_that_nothing_calls() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n{NODES}\n\
       compiler_make_procedure_live :: (w: Workspace, header: *Code_Procedure_Header) #compiler;\n\
       Code_Procedure_Header :: struct {{\n  \
         #as using base: Code_Node;\n  \
         constants_block: *void;\n  \
         arguments: [] *void;\n  \
         returns:   [] *void;\n  \
         parameter_usings: [] *void;\n  \
         name: string;\n\
       }}\n\
       PROGRAM :: #string OJ_DONE\n\
       nobody_calls_me :: () {{ }}\n\
       main :: () {{ }}\n\
       OJ_DONE\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"made_live\";\n  \
         set_build_options(options, w);\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(PROGRAM, w);\n  \
         asked := false;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .TYPECHECKED && !asked {{\n      \
             batch := cast(*Message_Typechecked) message;\n      \
             for batch.procedure_headers {{\n        \
               header := cast(*Code_Procedure_Header) it.expression;\n        \
               if header.name == \"nobody_calls_me\" {{\n          \
                 asked = true;\n          \
                 compiler_make_procedure_live(w, header);\n        \
               }}\n      \
             }}\n    \
           }}\n    \
           if message.kind == .COMPLETE  break;\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if !asked  compiler_report(\"the header was never reported\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("made_live"));
}

/// The `Code_*` declarations `compiler_modify_procedure` needs, laid out the
/// way `oj-meta`'s mirrors are (**C§5.3**).
const BODIES: &str = "\
Code_Block :: struct {
  #as using base: Code_Node;
  parent: *void;
  block_type: s32;
  block_flags: u32;
  belongs_to_struct: *void;
  members:    [] *void;
  statements: [] *Code_Node;
  owning_statement: *void;
}

Code_Procedure_Header :: struct {
  #as using base: Code_Node;
  constants_block: *void;
  arguments: [] *void;
  returns:   [] *void;
  parameter_usings: [] *void;
  name: string;
}

Code_Procedure_Body :: struct {
  #as using base: Code_Node;
  block:  *Code_Block;
  header: *Code_Procedure_Header;
  body_flags: u32;
}

compiler_modify_procedure :: (w: Workspace, body: *Code_Procedure_Body) #compiler;
";

#[test]
fn a_metaprogram_modifies_a_procedure_body_and_the_program_changes() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n{NODES}\n{BODIES}\n\
       PROGRAM :: #string OJ_DONE\n\
       #import \"Basic\";\n\
       main :: () {{\n    \
         print(\"first\\n\");\n    \
         print(\"second\\n\");\n\
       }}\n\
       OJ_DONE\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"modified\";\n  \
         set_build_options(options, w);\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(PROGRAM, w);\n  \
         asked := false;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .TYPECHECKED && !asked {{\n      \
             batch := cast(*Message_Typechecked) message;\n      \
             for batch.procedure_bodies {{\n        \
               body := cast(*Code_Procedure_Body) it.expression;\n        \
               if body.header.name == \"main\" {{\n          \
                 asked = true;\n          \
                 body.block.statements.count -= 1;\n          \
                 compiler_modify_procedure(w, body);\n        \
               }}\n      \
             }}\n    \
           }}\n    \
           if message.kind == .COMPLETE  break;\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if !asked  compiler_report(\"the body was never reported\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  let executable = fixture.path("modified");
  assert_built(&report, &executable);
  let output = std::process::Command::new(&executable)
    .output()
    .expect("the modified program runs");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "first\n");
}

#[test]
fn a_statement_a_metaprogram_made_itself_is_compiled_back_to_source() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    &format!(
      "{MESSAGES}\n{NODES}\n{BODIES}\n\
       Code_Return :: struct {{\n  \
         #as using base: Code_Node;\n  \
         arguments_unsorted: [] *void;\n  \
         arguments_sorted:   [] *Code_Node;\n  \
         return_flags: u32;\n\
       }}\n\
       PROGRAM :: #string OJ_DONE\n\
       #import \"Basic\";\n\
       main :: () {{\n    \
         print(\"unreachable\\n\");\n\
       }}\n\
       OJ_DONE\n\
       #run {{\n  \
         w := compiler_create_workspace(\"target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"rebuilt\";\n  \
         set_build_options(options, w);\n  \
         compiler_begin_intercept(w);\n  \
         add_build_string(PROGRAM, w);\n  \
         made: Code_Return;\n  \
         statements: [1] *Code_Node;\n  \
         asked := false;\n  \
         while true {{\n    \
           message := compiler_wait_for_message();\n    \
           if message.kind == .TYPECHECKED && !asked {{\n      \
             batch := cast(*Message_Typechecked) message;\n      \
             for batch.procedure_bodies {{\n        \
               body := cast(*Code_Procedure_Body) it.expression;\n        \
               if body.header.name == \"main\" {{\n          \
                 asked = true;\n          \
                 made.kind = .RETURN;\n          \
                 statements[0] = cast(*Code_Node) *made;\n          \
                 body.block.statements.data  = *statements[0];\n          \
                 body.block.statements.count = 1;\n          \
                 compiler_modify_procedure(w, body);\n        \
               }}\n      \
             }}\n    \
           }}\n    \
           if message.kind == .COMPLETE  break;\n  \
         }}\n  \
         compiler_end_intercept(w);\n  \
         if !asked  compiler_report(\"the body was never reported\");\n\
       }}\n\
       main :: () {{}}\n"
    ),
  ) else {
    return;
  };
  let executable = fixture.path("rebuilt");
  assert_built(&report, &executable);
  let output = std::process::Command::new(&executable)
    .output()
    .expect("the rebuilt program runs");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn the_check_plugin_reports_a_format_string_that_does_not_match_its_arguments() {
  // `Default_Metaprogram` loads `modules/Check` unless `-no_check`, and Check
  // is what turns a call's `Code_*` tree into this diagnostic. The wording is
  // the reference compiler's, measured by running the same program through it.
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  // SAFETY: as the test above.
  unsafe { oj_testsupport::use_own_modules() };
  let metaprogram =
    oj_driver::default_metaprogram().expect("the distribution ships a Default_Metaprogram");

  let fixture = Fixture::new();
  fixture.write(
    "bad.jai",
    "#import \"Basic\";\nmain :: () {\n    print(\"% and %\\n\", 1);\n}\n",
  );
  let build = |arguments: &[String]| {
    oj_driver::run_through_metaprogram(
      &metaprogram,
      &[fixture.path("bad.jai")],
      arguments,
      &oj_driver::BuildOptions::new(),
      oj_driver::Stage::Executable,
    )
  };

  let report = build(&[]);
  assert!(report.failed, "the plugin should stop the build");
  let reported = report.diagnostics.join("");
  assert!(
    reported.contains(
      "Error: Incorrect number of arguments supplied to 'print': \
       The format string requires 2 arguments, but 1 argument is given."
    ),
    "{reported}"
  );

  // `-no_check` is what says not to load the plugin, and then nothing checks.
  let report = build(&[String::from("-no_check")]);
  assert!(
    !report.failed,
    "without the plugin the same program builds, but:\n{}",
    report.diagnostics.join("")
  );
}

#[test]
fn a_metaprogram_fills_a_placeholder_of_the_compilation_it_is_part_of() {
  // `Metaprogram_Plugins` bootstraps this way: it fills the `#placeholder` its
  // own body waits on from inside a `#run`. The reference stalls that `#run`
  // until the declaration arrives (**C§3.3**); orangejuice runs the whole
  // compilation again with it in place (`docs/spec.md` §6.5).
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#import \"Basic\";\n\
     #placeholder ANSWER;\n\
     HERE :: #code {};\n\
     #run add_build_string(\"ANSWER :: 42;\", -1, HERE);\n\
     main :: () {\n  \
       print(\"%\\n\", ANSWER);\n\
     }\n",
  ) else {
    return;
  };
  let executable = fixture.path("main");
  assert_built(&report, &executable);
  let output = std::process::Command::new(&executable)
    .output()
    .expect("the program runs");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
}

#[test]
fn a_workspace_is_compiled_once_and_numbered_by_the_one_above_it() {
  // A metaprogram that fills a `#placeholder` of its own compilation makes
  // that compilation run again with the declaration in place (`docs/spec.md`
  // §6.5). The workspace it drives must not be compiled in the round that is
  // going to be replayed, or every `#run` of the program under it would happen
  // twice — the reference stalls the one `#run` and compiles once.
  let fixture = Fixture::new();
  let counted = fixture.path("counted.jai");
  fixture.write(
    "counted.jai",
    // The reference numbers workspaces once for the whole compiler: the
    // metaprogram is 1 and the program it builds is 2 (**C§3.1**), so a
    // compilation that is itself a workspace carries its number.
    "#import \"Basic\";\n\
     #import \"Compiler\";\n\
     #no_reset times := 0;\n\
     #run {\n  \
       times += 1;\n  \
       if get_current_workspace() != 2  compiler_report(\"this workspace is not 2\");\n\
     }\n\
     main :: () { print(\"%\\n\", times); }\n",
  );
  let Some(report) = build(
    &fixture,
    &format!(
      "#import \"Basic\";\n\
       #placeholder READY;\n\
       HERE :: #code {{}};\n\
       #run {{\n  \
         add_build_string(\"READY :: true;\", -1, HERE);\n  \
         w := compiler_create_workspace(\"Target\");\n  \
         options := get_build_options(w);\n  \
         options.output_executable_name = \"counted\";\n  \
         options.output_path = \"{output}\";\n  \
         set_build_options(options, w);\n  \
         add_build_file(\"{source}\", w);\n\
       }}\n\
       #run READY;\n\
       main :: () {{}}\n",
      output = fixture.path("").display(),
      source = counted.display(),
    ),
  ) else {
    return;
  };
  assert!(
    !report.failed,
    "the metaprogram should build its workspace, but:\n{}",
    report.diagnostics.join("")
  );
  let executable = fixture.path("counted");
  let output = std::process::Command::new(&executable)
    .output()
    .expect("the program the workspace produced runs");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
}

#[test]
fn several_workspaces_share_the_one_message_stream() {
  // The reference has one message stream per compiler, so a metaprogram may
  // intercept several workspaces and read all of their messages from it
  // (**C§3.1**) — which is what `examples/output_types` does with four.
  let fixture = Fixture::new();
  let Some(report) = build_watching(
    &fixture,
    "TEXT :: \"main :: () {}\";\n\
     #run {\n  \
       first := compiler_create_workspace(\"first\");\n  \
       second := compiler_create_workspace(\"second\");\n  \
       compiler_begin_intercept(first);\n  \
       add_build_string(TEXT, first);\n  \
       compiler_begin_intercept(second);\n  \
       add_build_string(TEXT, second);\n  \
       completed := 0;\n  \
       while true {\n    \
         message := compiler_wait_for_message();\n    \
         if !message  compiler_report(\"the stream ran out\");\n    \
         if !message  break;\n    \
         if message.kind == .COMPLETE {\n      \
           completed += 1;\n      \
           if completed >= 2  break;\n    \
         }\n  \
       }\n  \
       if completed != 2  compiler_report(\"only one workspace completed\");\n  \
       set_build_options_dc(.{do_output=false});\n\
     }\n",
  ) else {
    return;
  };
  assert!(!report.failed, "{}", report.diagnostics.join(""));
}

#[test]
fn a_watched_workspaces_diagnostics_say_which_workspace_they_are() {
  // The compilation a diagnostic came from is not the one the command line
  // named, so the compiler says which workspace it belongs to first
  // (**C§12**), measured against the reference.
  let fixture = Fixture::new();
  let Some(report) = build_watching(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"Target Program\");\n\
       compiler_begin_intercept(w);\n\
       add_build_string(\"main :: () { undefined_thing(); }\", w);\n\
       while true {\n\
         message := compiler_wait_for_message();\n\
         if message.kind == .COMPLETE  break;\n\
       }\n\
       compiler_end_intercept(w);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(report.failed, "the workspace should fail");
  assert_eq!(
    report.diagnostics.first().map(String::as_str),
    Some("\nIn Workspace 2 (\"Target Program\"):\n"),
    "got {:#?}",
    report.diagnostics
  );
}

#[test]
fn an_insert_splices_the_code_a_metaprogram_wrote_to() {
  // `compiler_get_nodes` hands out a tree the metaprogram may edit in place,
  // and what the `#insert` splices is what it left there rather than the
  // program the address was exported from (**C§3.3**) — which is what
  // a plugin uppercases a string literal with.
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#import \"Basic\";\n\
     #import \"Compiler\";\n\
     shout :: (code: Code) -> Code {\n  \
       root, expressions := compiler_get_nodes(code);\n  \
       for expressions {\n    \
         if it.kind != .LITERAL  continue;\n    \
         literal := cast(*Code_Literal) it;\n    \
         if literal.value_type != .STRING  continue;\n    \
         if literal._string == \"quiet\"  literal._string = \"LOUD\";\n    \
         if literal._string == \"also quiet\"  literal._string = \"ALSO LOUD\";\n  \
       }\n  \
       return compiler_get_code(root);\n\
     }\n\
     play :: (c: Code) #expand {\n  \
       modified :: #run shout(c);\n  \
       #insert,scope() modified;\n\
     }\n\
     main :: () {\n  \
       quoted :: #code { word := \"quiet\"; print(\"%\\n\", word); };\n  \
       play(quoted);\n  \
       play(#code print(\"%\\n\", \"also quiet\"));\n\
     }\n",
  ) else {
    return;
  };
  let executable = fixture.path("main");
  assert_built(&report, &executable);
  let output = std::process::Command::new(&executable)
    .output()
    .expect("the program runs");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "LOUD\nALSO LOUD\n");
}

#[test]
fn a_rewritten_call_keeps_the_names_its_arguments_were_written_under() {
  // A named argument means something else read back as a positional one
  // (**L§7.4**), so the printer that rebuilds an edited tree keeps the name.
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#import \"Basic\";\n\
     #import \"Compiler\";\n\
     shout :: (code: Code) -> Code {\n  \
       root, expressions := compiler_get_nodes(code);\n  \
       for expressions {\n    \
         if it.kind != .LITERAL  continue;\n    \
         literal := cast(*Code_Literal) it;\n    \
         if literal.value_type != .STRING  continue;\n    \
         if literal._string == \"a\"  literal._string = \"A\";\n  \
       }\n  \
       return compiler_get_code(root);\n\
     }\n\
     joined :: (first: string, second := \"b\", separator := \"-\") {\n  \
       print(\"%1%2%3\\n\", first, separator, second);\n\
     }\n\
     play :: (c: Code) #expand {\n  \
       modified :: #run shout(c);\n  \
       #insert,scope() modified;\n\
     }\n\
     main :: () {\n  \
       play(#code joined(\"a\", separator = \"+\"));\n\
     }\n",
  ) else {
    return;
  };
  let executable = fixture.path("main");
  assert_built(&report, &executable);
  let output = std::process::Command::new(&executable)
    .output()
    .expect("the program runs");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "A+b\n");
}

#[test]
fn a_warning_in_a_watched_workspace_is_not_reported_as_an_error() {
  // A metaprogram gets one `Message.ERROR` per error (**C§3.2**), and every
  // build file in the distribution's own examples `exit(1)` the moment it sees
  // one — so a program that merely warns must not send any. `skeletal
  // -animation` is the one that found this: it reaches a `Text_File_Handler`
  // whose control paths do not all return a value.
  let fixture = Fixture::new();
  let Some(report) = build_watching(
    &fixture,
    "#run {\n\
       w := compiler_create_workspace(\"target\");\n\
       options := get_build_options(w);\n\
       options.output_executable_name = \"warned\";\n\
       set_build_options(options, w);\n\
       compiler_begin_intercept(w);\n\
       add_build_string(\"f :: (x: bool) -> int { if x return 1; }\\nmain :: () { f(true); }\", w);\n\
       errors := 0;\n\
       while true {\n\
         message := compiler_wait_for_message();\n\
         if message.kind == .ERROR  errors += 1;\n\
         if message.kind == .COMPLETE  break;\n\
       }\n\
       compiler_end_intercept(w);\n\
       if errors  compiler_report(\"a warning was reported as an error\");\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert_built(&report, &fixture.path("warned"));
}

#[test]
fn a_plain_error_report_stops_the_metaprogram() {
  // `Report.ERROR` stops the workspace at the next opportunity, so nothing a
  // metaprogram says after one is ever printed (**C§3.3**). That is why the
  // reference reports one bad format string rather than every one in a file,
  // and it is what `ERROR_CONTINUABLE` exists to opt out of.
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n  \
       compiler_report(\"the first complaint\");\n  \
       compiler_report(\"the second complaint\");\n  \
       compiler_report(\"an afterthought\", mode = .WARNING);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(report.failed);
  let reported = report.diagnostics.join("");
  assert!(
    reported.contains("Error: the first complaint"),
    "{reported}"
  );
  assert!(!reported.contains("the second complaint"), "{reported}");
  assert!(!reported.contains("an afterthought"), "{reported}");
}

#[test]
fn error_continuable_lets_the_next_report_through() {
  let fixture = Fixture::new();
  let Some(report) = build(
    &fixture,
    "#run {\n  \
       compiler_report(\"the first complaint\", mode = .ERROR_CONTINUABLE);\n  \
       compiler_report(\"the second complaint\", mode = .ERROR_CONTINUABLE);\n\
     }\n\
     main :: () {}\n",
  ) else {
    return;
  };
  assert!(report.failed);
  let reported = report.diagnostics.join("");
  assert!(
    reported.contains("Error: the first complaint"),
    "{reported}"
  );
  assert!(
    reported.contains("Error: the second complaint"),
    "{reported}"
  );
}

/// `-help` is answered by the metaprogram rather than by the compiler
/// (**C§2.1**), and it is asked for instead of a file to compile — so the
/// metaprogram has to run, print, and finish without a workspace, rather than
/// the compiler refusing to start because nothing was named.
#[test]
fn the_metaprogram_answers_help_with_no_file_to_compile() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe { oj_testsupport::use_own_modules() };
  let metaprogram =
    oj_driver::default_metaprogram().expect("the distribution ships a Default_Metaprogram");

  for spelling in ["-help", "-?"] {
    let report = oj_driver::run_through_metaprogram(
      &metaprogram,
      &[],
      &[String::from(spelling)],
      &oj_driver::BuildOptions::new(),
      oj_driver::Stage::Executable,
    );
    assert!(
      !report.failed,
      "{spelling} should be answered, but:\n{}",
      report.diagnostics.join("")
    );
    assert!(
      report.executable.is_none(),
      "{spelling} asks for help, not for a build"
    );
  }
}

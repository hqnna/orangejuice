//! The acceptance suite (`docs/spec.md` §8).
//!
//! Every program under `examples/` is compiled the way `oj` compiles one,
//! run, and its output compared against the `.out` file beside it. A golden
//! was measured against the reference compiler when the program was written,
//! so what these assert is Jai's behaviour and not orangejuice's habits.
//!
//! `OJ_BLESS_EXAMPLES=1` rewrites the goldens from what the programs print,
//! which is for *adding* one — never for making a failure go away.

use std::path::{Path, PathBuf};
use std::process::Command;

fn linker_is_available() -> bool {
  Command::new(oj_link::driver(oj_types::Target::HOST))
    .arg("--version")
    .output()
    .is_ok_and(|output| output.status.success())
}

/// The examples whose subject is `#asm`, which the language has on x86-64 and
/// nowhere else (**L§15**). On another target they are not a failure and not a
/// thing to rewrite: there is no program there to run, the way there is no
/// Windows example to run on Linux.
const X64_ONLY: &[&str] = &["220_inline_assembly"];

fn is_x64_only(program: &Path) -> bool {
  program
    .file_stem()
    .and_then(|stem| stem.to_str())
    .is_some_and(|stem| X64_ONLY.contains(&stem))
}

fn programs() -> Vec<PathBuf> {
  let mut found: Vec<PathBuf> = std::fs::read_dir(oj_testsupport::examples())
    .expect("the examples directory should be readable")
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "jai"))
    .collect();
  found.sort();
  found
}

/// Builds one example into a directory of its own and runs it. The source is
/// left where it is: an example `#load`s and `#import`s relative to itself,
/// and copying it somewhere else would be testing a different program.
fn run(program: &Path, output: &Path) -> Result<String, String> {
  let options = oj_driver::BuildOptions {
    output_path: Some(output.to_path_buf()),
    ..oj_driver::BuildOptions::new()
  };
  // SAFETY: cargo runs each integration test binary in its own process, and
  // nothing else in this one reads it.
  unsafe { oj_testsupport::use_own_modules() };

  let report = oj_driver::run(program, &options, oj_driver::Stage::Executable, None);
  if report.failed {
    return Err(format!("did not build:\n{}", report.diagnostics.join("")));
  }
  let executable = report.executable.ok_or("a successful build has one")?;
  let finished = Command::new(&executable)
    .output()
    .map_err(|error| format!("did not run: {error}"))?;
  if !finished.status.success() {
    return Err(format!(
      "exited {}:\n{}",
      finished.status,
      String::from_utf8_lossy(&finished.stderr)
    ));
  }
  Ok(String::from_utf8_lossy(&finished.stdout).into_owned())
}

#[test]
fn every_example_prints_what_its_golden_says() {
  if !linker_is_available() {
    eprintln!("skipping: no C driver on PATH to link with");
    return;
  }
  let bless = std::env::var_os("OJ_BLESS_EXAMPLES").is_some_and(|value| !value.is_empty());
  let programs = programs();
  assert!(programs.len() > 10, "expected the examples suite");

  let directory = tempfile::tempdir().expect("a temporary directory");
  let mut failures = Vec::new();
  let mut skipped = Vec::new();
  for program in &programs {
    if is_x64_only(program) && !oj_types::Target::HOST.is_x64() {
      skipped.push(program.display().to_string());
      continue;
    }
    let name = program.file_stem().expect("a named file");
    let output = directory.path().join(name);
    std::fs::create_dir_all(&output).expect("the output directory should be creatable");

    let printed = match run(program, &output) {
      Ok(printed) => printed,
      Err(why) => {
        failures.push(format!("{}: {why}", program.display()));
        continue;
      }
    };

    let golden = program.with_extension("out");
    if bless {
      std::fs::write(&golden, &printed).expect("the golden should be writable");
      continue;
    }
    let expected = match std::fs::read_to_string(&golden) {
      Ok(expected) => expected,
      Err(_) => {
        failures.push(format!(
          "{} has no golden beside it; write one, or run with OJ_BLESS_EXAMPLES=1",
          program.display()
        ));
        continue;
      }
    };
    if printed != expected {
      failures.push(format!(
        "{}:\n--- expected ---\n{expected}--- printed ---\n{printed}",
        program.display()
      ));
    }
  }

  for program in &skipped {
    eprintln!("skipping {program}: its subject is '#asm', which this target has none of");
  }
  assert!(
    failures.is_empty(),
    "{} of {} examples did not print what they should:\n\n{}",
    failures.len(),
    programs.len() - skipped.len(),
    failures.join("\n\n")
  );
}

/// A golden with no program beside it is a program somebody deleted and a
/// check that quietly stopped running.
#[test]
fn every_golden_has_a_program_beside_it() {
  let orphans: Vec<String> = std::fs::read_dir(oj_testsupport::examples())
    .expect("the examples directory should be readable")
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "out"))
    .filter(|path| !path.with_extension("jai").is_file())
    .map(|path| path.display().to_string())
    .collect();
  assert!(orphans.is_empty(), "goldens with no program:\n{orphans:#?}");
}

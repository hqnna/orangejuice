//! Sweeps over other people's Jai code, for the regression net the 2026
//! review asked for (`docs/spec.md` §8). Ignored by default, since they need a
//! checkout nothing here ships: point the variable at one and run
//!
//! ```console
//! $ OJ_TTWJ_DIR=/path/to/The_Way_to_Jai cargo test -p oj-driver --test sweep -- --ignored --nocapture
//! ```
//!
//! What each program must print comes only from the project itself — the
//! book's `// =>` annotations, a test suite's own exit status — and never
//! from what orangejuice printed last time, so a sweep cannot pass by
//! remembering its own mistakes.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How one program fared.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Verdict {
  Pass,
  /// It built and ran, but what it printed is not what the book says.
  Mismatch(String),
  /// It did not build, with the first diagnostic.
  BuildFailed(String),
  /// It built and then failed or hung.
  RunFailed(String),
  Skipped(&'static str),
}

/// The examples of The Way to Jai that cannot be judged by running them here,
/// each with the reason. Everything else is expected to build, run and print
/// what its annotations say.
const TTWJ_SKIPS: &[(&str, &str)] = &[
  (
    "08/part1.jai",
    "a file the chapter's main program #loads, not a program",
  ),
  (
    "08/part3.jai",
    "a file the chapter's main program #loads, not a program",
  ),
  (
    "30/dynlib.jai",
    "the library the chapter's build script compiles, not a program",
  ),
  (
    "30/main4.jai",
    "a file the chapter's build script compiles, not a program",
  ),
  (
    "30/30.11_using_notes.jai",
    "has no main: a metaprogram reading notes at compile time",
  ),
  ("19/19.8_windows_input.jai", "Windows only"),
  ("29/29.2_call_c_windows.jai", "Windows only"),
  ("50/50.1_guessing_game_windows.jai", "Windows only"),
  (
    "20/20.2_assert.jai",
    "fails a compile-time assertion on purpose",
  ),
  ("20/20.3_crash.jai", "crashes on purpose"),
  ("11/11.6_read_input.jai", "reads standard input"),
  ("50/50.1_guessing_game_linux.jai", "reads standard input"),
  ("50/50.2_guessing_game.jai", "reads standard input"),
  ("31/31.2_build_threads.jai", "needs SDL"),
  ("31/build_gui.jai", "needs SDL"),
  ("33/33.4_sdl_test.jai", "needs SDL"),
  ("33/33.5_gl_sdl.jai", "needs SDL"),
  ("35/35.1_raylib_sample.jai", "needs raylib"),
  ("52/52.2_jai_raylib_pong.jai", "needs raylib"),
  ("34/34.7_mailing.jai", "sends mail"),
  (
    "21/21.3_allocators_check.jai",
    "needs the reference's rpmalloc module",
  ),
  (
    "30/main7.jai",
    "the program the chapter's build script compiles against its library, not one of its own",
  ),
  (
    "30/30.9_house_rules.jai",
    "its plugin reports the triple pointer in main5.jai on purpose",
  ),
  (
    "30/30.13_dynamic_libraries.jai",
    "a build script: what it builds is dynlib.jai and main7.jai, not a program of its own",
  ),
  (
    "30/30.14_build_inlining.jai",
    "a build script that takes the program to build on its command line",
  ),
  (
    "27/27.4_del_dirs.jai",
    "takes the directories to delete on its command line",
  ),
  ("19/19.7_linux_input.jai", "reads standard input"),
  ("20/20.1_crash.jai", "crashes on purpose"),
];

/// Chapters whose programs open a window, which a sweep cannot look at.
const TTWJ_GRAPHICAL_CHAPTERS: &[&str] = &["33", "51", "52"];

/// The lines a program must print, in order, as the book annotates them:
/// whatever follows `// =>` on a line, up to a later `//`.
///
/// Some annotations are not output, and are left out rather than guessed at:
/// one on a line that is itself commented out, or inside a `/* */` block,
/// describes code the book took out; one that starts `Error` or `Warning` is
/// the compiler's complaint about such code; and one that names an address,
/// a Windows path or a time is the author's machine on the day, not the
/// program.
fn annotations(source: &str) -> Vec<String> {
  annotation_groups(source).into_iter().flatten().collect()
}

/// The same, one list per top-level declaration. The book annotates a
/// procedure where it is written, so what one declared above `main` prints is
/// expected in the order of that procedure's own lines but wherever `main`
/// calls it from.
fn annotation_groups(source: &str) -> Vec<Vec<String>> {
  let mut groups: Vec<Vec<String>> = vec![Vec::new()];
  let mut block_depth = 0usize;
  let mut brace_depth = 0usize;
  for line in source.lines() {
    let depth_before = block_depth;
    block_depth = block_comment_depth_after(line, block_depth);
    if depth_before > 0 {
      continue;
    }
    let code = code_of(line);
    if brace_depth == 0 && !code.trim().is_empty() && groups.last().is_some_and(|group| !group.is_empty()) {
      groups.push(Vec::new());
    }
    brace_depth = braces_after(code, brace_depth);
    let Some(at) = line.find("// =>") else {
      continue;
    };
    let before = line[..at].trim();
    if before.starts_with("//") || before.contains("/*") {
      continue;
    }
    let after = &line[at + "// =>".len()..];
    let after = match after.find("//") {
      Some(end) => &after[..end],
      None => after,
    };
    let text = normalize(after);
    if text.is_empty() || machine_dependent(after) {
      continue;
    }
    if let Some(group) = groups.last_mut() {
      group.push(text);
    }
  }
  groups.retain(|group| !group.is_empty());
  groups
}

/// A line without its `//` comment, with what its string literals say left
/// out, so that the braces left are the program's.
fn code_of(line: &str) -> String {
  let mut code = String::new();
  let mut in_string = false;
  let mut escaped = false;
  let mut previous = '\0';
  for c in line.chars() {
    if in_string {
      match (escaped, c) {
        (true, _) => escaped = false,
        (false, '\\') => escaped = true,
        (false, '"') => in_string = false,
        _ => {}
      }
      continue;
    }
    if c == '/' && previous == '/' {
      code.pop();
      break;
    }
    if c == '"' {
      in_string = true;
    } else {
      code.push(c);
    }
    previous = c;
  }
  code
}

fn braces_after(code: String, depth: usize) -> usize {
  code.chars().fold(depth, |depth, c| match c {
    '{' => depth + 1,
    '}' => depth.saturating_sub(1),
    _ => depth,
  })
}

/// How deep in `/* */` comments the source is after `line`, given how deep it
/// was before. Jai's block comments nest.
fn block_comment_depth_after(line: &str, mut depth: usize) -> usize {
  let bytes = line.as_bytes();
  let mut at = 0;
  while at + 1 < bytes.len() {
    if depth == 0 && bytes[at] == b'/' && bytes[at + 1] == b'/' {
      break;
    }
    if bytes[at] == b'/' && bytes[at + 1] == b'*' {
      depth += 1;
      at += 2;
      continue;
    }
    if depth > 0 && bytes[at] == b'*' && bytes[at + 1] == b'/' {
      depth -= 1;
      at += 2;
      continue;
    }
    at += 1;
  }
  depth
}

/// Whether an annotation records something of the machine the book was run
/// on rather than of the program.
fn machine_dependent(text: &str) -> bool {
  let trimmed = text.trim_start();
  if trimmed.starts_with("Error") || trimmed.starts_with("Warning") {
    return true;
  }
  let lower = text.to_ascii_lowercase();
  if [
    ":/",
    ":\\",
    "0x7",
    " ms",
    "ms ",
    "seconds",
    "nanoseconds",
    "cycles",
    "time",
  ]
  .iter()
  .any(|marker| lower.contains(marker))
  {
    return true;
  }
  // An address as `print` writes one: hex digits in groups of four.
  lower
    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
    .any(|word| {
      let groups: Vec<&str> = word.split('_').collect();
      groups.len() >= 3
        && groups
          .iter()
          .all(|group| !group.is_empty() && group.chars().all(|c| c.is_ascii_hexdigit()))
        && groups.iter().skip(1).all(|group| group.len() == 4)
    })
}

/// The words of a piece of text, without the punctuation between them. The
/// book's annotations are written by hand beside the `print` that produces
/// them — `// => y is 42;`, `Hello Sailor!` for `Hello, Sailor!` — so what is
/// compared is the words and their order, not every comma.
fn normalize(text: &str) -> String {
  text
    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '-')
    .map(|word| word.trim_matches(|c: char| c == '.' || c == '-'))
    .filter(|word| !word.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

/// Whether every annotation appears in the output, in order, word for word.
fn matches_in_order(output: &str, expected: &[String]) -> Result<(), String> {
  let output = format!(" {} ", normalize(output));
  let mut from = 0;
  for line in expected {
    let wanted = format!(" {line} ");
    match output[from..].find(wanted.as_str()) {
      Some(at) => from += at + wanted.len() - 1,
      None => return Err(line.clone()),
    }
  }
  Ok(())
}

/// The same, letter for letter with the spaces left out: a `print` with no
/// newline runs into whatever the next one prints, which the book annotates
/// as two lines.
fn matches_run_together(output: &str, expected: &[String]) -> Result<(), String> {
  let squash = |text: &str| text.chars().filter(|c| *c != ' ').collect::<String>();
  let output = squash(&normalize(output));
  let mut from = 0;
  for line in expected {
    let wanted = squash(line);
    match output[from..].find(wanted.as_str()) {
      Some(at) => from += at + wanted.len(),
      None => return Err(line.clone()),
    }
  }
  Ok(())
}

/// Whether each declaration's annotations appear in the output in order, word
/// for word where the output has the words apart and letter for letter where
/// it ran them together.
fn matches_groups(output: &str, groups: &[Vec<String>]) -> Result<(), String> {
  for group in groups {
    if let Err(missing) = matches_in_order(output, group) {
      matches_run_together(output, group).map_err(|_| missing)?;
    }
  }
  Ok(())
}

/// Runs `executable` in `directory` with nothing on standard input, giving up
/// after `limit`.
fn run_for(executable: &Path, directory: &Path, limit: Duration) -> Result<(i32, String), String> {
  let mut child = Command::new(executable)
    .current_dir(directory)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()
    .map_err(|error| format!("could not start: {error}"))?;
  let mut stdout = child.stdout.take().expect("piped");
  let reader = std::thread::spawn(move || {
    let mut text = Vec::new();
    let _ = stdout.read_to_end(&mut text);
    text
  });
  let started = Instant::now();
  let status = loop {
    if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
      break status;
    }
    if started.elapsed() > limit {
      let _ = child.kill();
      let _ = child.wait();
      return Err(format!("still running after {}s", limit.as_secs()));
    }
    std::thread::sleep(Duration::from_millis(20));
  };
  let output = reader.join().unwrap_or_default();
  Ok((
    status.code().unwrap_or(-1),
    String::from_utf8_lossy(&output).into_owned(),
  ))
}

/// Builds one program the way `oj <file> -- no_metaprogram` would, into a
/// directory of its own.
fn build(path: &Path, output: &Path) -> Result<PathBuf, String> {
  build_with(path, output, &[])
}

/// The same, with directories added to the import path the way
/// `-import_dir` adds them.
fn build_with(path: &Path, output: &Path, import_dirs: &[PathBuf]) -> Result<PathBuf, String> {
  let mut options = oj_driver::BuildOptions::new();
  options.output_path = Some(output.to_path_buf());
  options.import_dirs.extend(import_dirs.iter().cloned());
  let report = oj_driver::run(path, &options, oj_driver::Stage::Executable, None);
  if report.failed {
    let text = report.diagnostics.join("");
    let first = text
      .lines()
      .find(|line| line.contains("Error") || line.contains("error"))
      .unwrap_or("it did not build")
      .trim()
      .to_string();
    return Err(first);
  }
  report
    .executable
    .ok_or_else(|| String::from("no executable came out"))
}

unsafe extern "C" {
  fn dup(fd: i32) -> i32;
  fn dup2(from: i32, to: i32) -> i32;
  fn close(fd: i32) -> i32;
}

/// Runs `body` with standard output going to a file, and hands back what was
/// written there along with its result. What a `#run` prints at compile time
/// is written straight to file descriptor 1, so that is what is redirected.
fn capturing_stdout<T>(body: impl FnOnce() -> T) -> (T, String) {
  use std::io::{Seek, Write};
  use std::os::fd::AsRawFd;
  let Ok(mut file) = tempfile::tempfile() else {
    return (body(), String::new());
  };
  let _ = std::io::stdout().flush();
  // SAFETY: the sweep runs one program at a time, and file descriptor 1 is
  // put back the way it was before anything else writes to it.
  let saved = unsafe { dup(1) };
  if saved < 0 || unsafe { dup2(file.as_raw_fd(), 1) } < 0 {
    return (body(), String::new());
  }
  let result = body();
  let _ = std::io::stdout().flush();
  unsafe {
    dup2(saved, 1);
    close(saved);
  }
  let mut text = String::new();
  let _ = file.rewind();
  let _ = file.read_to_string(&mut text);
  (result, text)
}

/// Builds a program that drives the compiler itself the way the book does,
/// with `jai file.jai`: through `Default_Metaprogram`, so that the
/// workspaces it creates are numbered from there (**C§2.1**).
fn build_through_metaprogram(path: &Path, import_dirs: &[PathBuf]) -> Result<PathBuf, String> {
  let Some(metaprogram) = oj_driver::default_metaprogram() else {
    return Err(String::from("no Default_Metaprogram to build with"));
  };
  let mut options = oj_driver::BuildOptions::new();
  options.import_dirs.extend(import_dirs.iter().cloned());
  let named = path.display().to_string();
  let report = oj_driver::run_through_metaprogram(
    &metaprogram,
    &[path.to_path_buf()],
    &[named],
    &options,
    oj_driver::Stage::Executable,
  );
  if report.failed {
    let text = report.diagnostics.join("");
    let first = text
      .lines()
      .find(|line| line.contains("Error") || line.contains("error"))
      .unwrap_or("it did not build")
      .trim()
      .to_string();
    return Err(first);
  }
  report
    .executable
    .ok_or_else(|| String::from("no executable came out"))
}

fn judge_ttwj(root: &Path, path: &Path, scratch: &Path) -> Verdict {
  let relative = path
    .strip_prefix(root.join("examples"))
    .unwrap_or(path)
    .to_string_lossy()
    .replace('\\', "/");
  if let Some((_, reason)) = TTWJ_SKIPS.iter().find(|(name, _)| *name == relative) {
    return Verdict::Skipped(reason);
  }
  if TTWJ_GRAPHICAL_CHAPTERS
    .iter()
    .any(|chapter| relative.starts_with(&format!("{chapter}/")))
  {
    return Verdict::Skipped("opens a window");
  }
  let source = std::fs::read_to_string(path).unwrap_or_default();
  let expected = annotation_groups(&source);
  let output = scratch.join(relative.replace('/', "_"));
  let _ = std::fs::create_dir_all(&output);
  // The book compiles the chapters that import a module of its own with
  // `-import_dir my_modules` (§8.6). What a `#run` prints while it compiles
  // is part of what the book shows, so it is read along with the program's
  // own output; a program that creates workspaces of its own is compiled the
  // way the book compiles it, through the metaprogram.
  let import_dirs = [root.join("my_modules")];
  let drives_the_compiler = source.contains("#import \"Compiler\"");
  let (built, compiled) = capturing_stdout(|| match drives_the_compiler {
    true => build_through_metaprogram(path, &import_dirs),
    false => build_with(path, &output, &import_dirs),
  });
  let executable = match built {
    Ok(executable) => executable,
    Err(first) => return Verdict::BuildFailed(first),
  };
  let directory = path.parent().unwrap_or(root);
  match run_for(&executable, directory, Duration::from_secs(10)) {
    Err(reason) => Verdict::RunFailed(reason),
    Ok((status, output)) => match matches_groups(&format!("{compiled}\n{output}"), &expected) {
      Ok(()) if status == 0 => Verdict::Pass,
      Ok(()) => Verdict::RunFailed(format!("exited {status}")),
      Err(missing) => Verdict::Mismatch(missing),
    },
  }
}

/// The `.jai` files under `directory`, at most `depth` levels down. A
/// chapter's own programs sit directly in its directory; what is deeper is a
/// module one of them imports.
fn programs_under(directory: &Path, depth: usize) -> Vec<PathBuf> {
  let mut found: Vec<PathBuf> = walkdir::WalkDir::new(directory)
    .max_depth(depth)
    .into_iter()
    .filter_map(Result::ok)
    .map(|entry| entry.into_path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "jai"))
    .filter(|path| !path.components().any(|part| part.as_os_str() == ".build"))
    .collect();
  found.sort();
  found
}

fn report(name: &str, verdicts: &[(PathBuf, Verdict)]) {
  let mut pass = 0;
  let mut skip = 0;
  let mut fail = 0;
  for (path, verdict) in verdicts {
    match verdict {
      Verdict::Pass => pass += 1,
      Verdict::Skipped(reason) => {
        skip += 1;
        println!("SKIP {} ({reason})", path.display());
      }
      Verdict::Mismatch(missing) => {
        fail += 1;
        println!("FAIL {}: did not print `{missing}`", path.display());
      }
      Verdict::BuildFailed(first) => {
        fail += 1;
        println!("FAIL {}: {first}", path.display());
      }
      Verdict::RunFailed(reason) => {
        fail += 1;
        println!("FAIL {}: {reason}", path.display());
      }
    }
  }
  println!("{name}: {pass} passed, {fail} failed, {skip} skipped");
}

fn checkout(variable: &str) -> Option<PathBuf> {
  let directory = PathBuf::from(std::env::var_os(variable)?);
  // SAFETY: cargo runs each integration test binary in its own process.
  unsafe { oj_testsupport::use_own_modules() };
  Some(directory)
}

#[test]
#[ignore = "needs a checkout of Ivo-Balbaert/The_Way_to_Jai in OJ_TTWJ_DIR"]
fn the_way_to_jai() {
  let Some(root) = checkout("OJ_TTWJ_DIR") else {
    return;
  };
  let scratch = tempfile::tempdir().expect("a scratch directory");
  let verdicts: Vec<(PathBuf, Verdict)> = programs_under(&root.join("examples"), 2)
    .into_iter()
    .map(|path| {
      let verdict = judge_ttwj(&root, &path, scratch.path());
      (path, verdict)
    })
    .collect();
  report("The Way to Jai", &verdicts);
}

#[test]
#[ignore = "needs a checkout of mimhufford/AdventOfCode in OJ_AOC_DIR"]
fn advent_of_code() {
  // Each year is one program, `YYYY/YYYY.jai`, which `#load`s the days it
  // finds and runs every one against `input/`. The answers are nowhere in the
  // repository, so a year passes when it builds and runs to the end.
  let Some(root) = checkout("OJ_AOC_DIR") else {
    return;
  };
  let scratch = tempfile::tempdir().expect("a scratch directory");
  let mut years: Vec<PathBuf> = std::fs::read_dir(&root)
    .map(|entries| {
      entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
    })
    .unwrap_or_default();
  years.sort();
  let verdicts: Vec<(PathBuf, Verdict)> = years
    .into_iter()
    .filter_map(|year| {
      let name = year.file_name()?.to_string_lossy().into_owned();
      let program = year.join(format!("{name}.jai"));
      program.is_file().then_some(program)
    })
    .map(|path| {
      let output = scratch.path().join(path.file_stem().unwrap_or_default());
      let _ = std::fs::create_dir_all(&output);
      if path.ends_with("2019/2019.jai") {
        return (
          path,
          Verdict::Skipped("imports a 'Queue' module the repository does not include"),
        );
      }
      let verdict = match build(&path, &output) {
        Err(first) => Verdict::BuildFailed(first),
        Ok(executable) => {
          let directory = path.parent().unwrap_or(&root).to_path_buf();
          match run_for(&executable, &directory, Duration::from_secs(120)) {
            Ok((0, _)) => Verdict::Pass,
            Ok((status, _)) => Verdict::RunFailed(format!("exited {status}")),
            Err(reason) => Verdict::RunFailed(reason),
          }
        }
      };
      (path, verdict)
    })
    .collect();
  report("Advent of Code", &verdicts);
}

#[test]
#[ignore = "needs a checkout of sjorsdonkers/toml-jai in OJ_TOML_JAI_DIR"]
fn toml_jai_tests() {
  // The suite checks itself: each example exits 0 when it passes and 2 when
  // it skipped itself, which is what its own `tests.jai` reads.
  let Some(root) = checkout("OJ_TOML_JAI_DIR") else {
    return;
  };
  let scratch = tempfile::tempdir().expect("a scratch directory");
  let programs = [
    "examples/first.jai",
    "examples/union_and_overlay.jai",
    "examples/file_examples.jai",
    "examples/validation.jai",
    "examples/custom_handlers.jai",
    "examples/formatting_control.jai",
    "tools/toml-test/run_toml-tests.jai",
  ];
  let verdicts: Vec<(PathBuf, Verdict)> = programs
    .iter()
    .enumerate()
    .map(|(index, relative)| {
      let path = root.join(relative);
      let output = scratch.path().join(index.to_string());
      let _ = std::fs::create_dir_all(&output);
      let verdict = match build(&path, &output) {
        Err(first) => Verdict::BuildFailed(first),
        Ok(executable) => {
          let directory = path.parent().unwrap_or(&root).to_path_buf();
          match run_for(&executable, &directory, Duration::from_secs(60)) {
            Ok((0, _)) => Verdict::Pass,
            Ok((2, _)) => Verdict::Skipped("the test skipped itself"),
            Ok((status, _)) => Verdict::RunFailed(format!("exited {status}")),
            Err(reason) => Verdict::RunFailed(reason),
          }
        }
      };
      (path, verdict)
    })
    .collect();
  report("toml-jai", &verdicts);
}

#[test]
fn an_annotation_is_what_follows_the_arrow_on_a_live_line() {
  let source = "print(\"a is %\\n\", a);  // => a is   5;\n\
                // => second\n\
                // print(\"x\"); // => Error: not output\n\
                print(\"%\", *x); // => 54_8d14_fb50\n\
                /* print(1); // => inside a comment */\n\
                rand(); // => 41    // (3)\n\
                print(\"\"); // =>\n";
  assert_eq!(annotations(source), ["a is 5", "second", "41"]);
  assert_eq!(
    matches_in_order("a is 5\nsecond and 41\n", &annotations(source)),
    Ok(())
  );
  assert_eq!(
    matches_in_order("second\na is 5\n41\n", &annotations(source)),
    Err(String::from("second"))
  );
  // Words, not substrings of them.
  assert_eq!(
    matches_in_order("a is 50 second 41", &annotations(source)),
    Err(String::from("a is 5"))
  );
}

#[test]
fn a_procedure_above_main_is_expected_where_main_calls_it() {
  let source = "helper :: () {\n\
                  print(\"in helper {\\n\"); // => in helper\n\
                }\n\
                main :: () {\n\
                  print(\"first\\n\"); // => first\n\
                  helper();\n\
                  print(\"%\", 42); // => 42\n\
                  print(\"after\\n\"); // => after\n\
                }\n";
  let groups = annotation_groups(source);
  assert_eq!(groups, [vec!["in helper"], vec!["first", "42", "after"]]);
  assert_eq!(matches_groups("first\nin helper\n42after\n", &groups), Ok(()));
  assert_eq!(
    matches_groups("after\nin helper\nfirst 42\n", &groups),
    Err(String::from("after"))
  );
}

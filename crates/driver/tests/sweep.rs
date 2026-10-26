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
use std::process::{Child, Command, Stdio};
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
/// what its annotations say, the way its plan in [`TTWJ_PLANS`] runs it.
const TTWJ_SKIPS: &[(&str, &str)] = &[
  ("19/19.8_windows_input.jai", "Windows only"),
  ("29/29.2_call_c_windows.jai", "Windows only"),
  ("50/50.1_guessing_game_windows.jai", "Windows only"),
  (
    "33/33.2C_simp_window.jai",
    "Windows only: its #run imports Windows_Resources",
  ),
  (
    "33/33.6_window_creation_events.jai",
    "Windows only: it imports the Windows module",
  ),
  (
    "20/20.2_assert.jai",
    "fails a compile-time assertion on purpose",
  ),
  ("20/20.3_crash.jai", "crashes on purpose"),
  ("20/20.1_crash.jai", "crashes on purpose"),
  (
    "30/30.9_house_rules.jai",
    "its plugin reports the triple pointer in main5.jai on purpose",
  ),
  ("31/31.2_build_threads.jai", "needs SDL"),
  ("31/build_gui.jai", "needs SDL"),
  ("33/33.4_sdl_test.jai", "needs SDL"),
  ("33/33.5_gl_sdl.jai", "needs SDL"),
  ("33/33.1_glfw_window.jai", "needs GLFW"),
  ("35/35.1_raylib_sample.jai", "needs raylib"),
  ("52/52.2_jai_raylib_pong.jai", "needs raylib"),
  (
    "21/21.3_allocators_check.jai",
    "needs the reference's rpmalloc module",
  ),
  (
    "34/34.7_mailing.jai",
    "imports Mail, which the distribution does not ship (the reference's binds libcurl); \
     and what it would do is send mail to a server named by placeholders",
  ),
  (
    "12/12.8_inner_module_test.jai",
    "imports a TestInside module the book asks the reader to write",
  ),
  (
    "05/5.5_swapping.jai",
    "calls swap(*a, *b), an older Basic's pointer form; Basic's swap takes values and returns them #must",
  ),
  (
    "22/22.4_polyproc4.jai",
    "calls swap(*a, *b), an older Basic's pointer form; Basic's swap takes values and returns them #must",
  ),
  (
    "15/15.8_for_reverse.jai",
    "expects the pre-#v2 order of `for <`, and v2 is the default (docs/changes.md A2)",
  ),
  (
    "19/19.5_cstrings.jai",
    "prints and frees the pointer after its own loop has advanced it to the terminator",
  ),
  (
    "22/22.1B_polyproc.jai",
    "its annotations name s32 for an s8 argument and [] for a static array, contradicting each other",
  ),
  (
    "33/33.12_drawing_circle.jai.jai",
    "`2*PI/30 * it` is a float64 under L§2.6, and make_vector2 takes float32 (an open question in TODO.md)",
  ),
];

/// How a program is run when building it and running it with nothing on its
/// standard input is not the whole story. What it reads from standard input
/// otherwise comes from the book's own `// <=` lines.
#[derive(Clone, Copy, Debug)]
enum Plan {
  /// Built and run; the default.
  Program,
  /// Each time the program prints `prompt`, the next of `replies` is typed
  /// in; it must then print `expected` as well as its annotations.
  Converse {
    prompt: &'static str,
    replies: fn() -> Vec<String>,
    expected: &'static [&'static str],
  },
  /// The book's 27.4, which deletes the directories of a name under the one
  /// it is run in: run on a tree made for it.
  DeleteDirectories,
  /// A build script, compiled in a copy of its chapter the way the book
  /// compiles it, with `arguments` after a lone `-`; it builds `runs`, which
  /// must print `expected`, the output the book's text gives.
  BuildScript {
    arguments: &'static [&'static str],
    runs: &'static str,
    expected: &'static [&'static str],
  },
  /// Not a program of its own: a file the named one `#load`s or builds,
  /// judged as that program.
  PartOf(&'static str),
  /// Opens a window. Run on a virtual display of its own, and asked to close
  /// the way a window manager asks; `closes` says whether the book's loop
  /// listens, and one that does not is stopped once its window is up.
  Window { closes: bool },
  /// Runs until it is stopped, printing as it goes.
  UntilStopped,
}

/// The guesses typed into the guessing game: one it cannot read, then every
/// number from the bottom up, so it ends whatever it picked.
fn guesses() -> Vec<String> {
  std::iter::once(String::from("seven"))
    .chain((1..=100).map(|guess| guess.to_string()))
    .collect()
}

/// The programs that are not run the default way, each with its plan.
const TTWJ_PLANS: &[(&str, Plan)] = &[
  (
    "50/50.1_guessing_game_linux.jai",
    Plan::Converse {
      prompt: "Guess a number between 1 and 100",
      replies: guesses,
      expected: &["I didn't understand", "Total input errors: 1"],
    },
  ),
  ("27/27.4_del_dirs.jai", Plan::DeleteDirectories),
  ("08/part1.jai", Plan::PartOf("08/8.1_main.jai")),
  ("08/part3.jai", Plan::PartOf("08/8.1_main.jai")),
  (
    "30/30.11_using_notes.jai",
    Plan::BuildScript {
      arguments: &[],
      runs: "notes",
      // 30B_Manipulating_the_build_process.md: "Then run the generated
      // executable `notes.exe`: which produces as output".
      expected: &["apple", "banana", "cherry", "dog", "elephant"],
    },
  ),
  ("30/main4.jai", Plan::PartOf("30/30.11_using_notes.jai")),
  (
    "30/30.13_dynamic_libraries.jai",
    Plan::BuildScript {
      arguments: &[],
      runs: "main7",
      // The same chapter: "and see the result of: `main7` which outputs:
      // `Hello Sailor!`".
      expected: &["Hello Sailor!"],
    },
  ),
  (
    "30/dynlib.jai",
    Plan::PartOf("30/30.13_dynamic_libraries.jai"),
  ),
  (
    "30/main7.jai",
    Plan::PartOf("30/30.13_dynamic_libraries.jai"),
  ),
  (
    "30/30.14_build_inlining.jai",
    Plan::BuildScript {
      // The book runs `jai 30.14_build_inlining.jai - main8` and prints
      // `args: ["30.14_build_inlining.jai", "-", "main8"]`: 0.2.024 handed
      // a build script the whole line. 0.2.009 hands it what follows the
      // `-` (**C§2.1**), so the sweep writes the line the book printed there.
      arguments: &["30.14_build_inlining.jai", "-", "main8"],
      runs: "main8",
      // The same chapter prints the arguments, and "When we run `main8`, we
      // get the output: `This program was built with meta-program …`" — a
      // sentence that drops main8.jai's "a", so the words before it.
      expected: &[
        "args: [\"30.14_build_inlining.jai\", \"-\", \"main8\"]",
        "This program was built with",
      ],
    },
  ),
  ("33/33.2A_simp_window.jai", Plan::Window { closes: true }),
  (
    "33/33.2B_bouncing_square.jai",
    Plan::Window { closes: true },
  ),
  ("33/33.3_getrect_buttons.jai", Plan::Window { closes: true }),
  ("33/33.10_play_sound.jai", Plan::Window { closes: false }),
  (
    "33/33.11_colored_triangle.jai",
    Plan::Window { closes: true },
  ),
  ("33/33.13_drawing_board.jai", Plan::Window { closes: true }),
  ("33/33.14_moving_shape.jai", Plan::Window { closes: true }),
  (
    "33/33.15_drawing_texture.jai",
    Plan::Window { closes: true },
  ),
  ("51/51.1_game_of_life.jai", Plan::UntilStopped),
  (
    "51/51.2_game_of_life_gui.jai",
    Plan::Window { closes: true },
  ),
  ("52/52.1_simp_pong.jai", Plan::Window { closes: true }),
];

fn plan_of(relative: &str) -> Plan {
  TTWJ_PLANS
    .iter()
    .find(|(name, _)| *name == relative)
    .map_or(Plan::Program, |(_, plan)| *plan)
}

/// Annotations the book writes that are not what the program prints — prose,
/// a typo against the program's own format string, a value of uninitialized
/// or freed memory, or the author's machine — each with the reason. The rest
/// of the program's annotations are still checked.
const TTWJ_ERRATA: &[(&str, &str, &str)] = &[
  (
    "05/5.3_variable_declarations.jai",
    "31525674139189348",
    "a variable left uninitialized with ---",
  ),
  (
    "06/6.1_bools.jai",
    "changes true to false and vice-versa",
    "prose",
  ),
  (
    "06/6.2_assert.jai",
    "With line 5 uncommented",
    "prose about a commented-out line",
  ),
  (
    "06/6.3_numbers.jai",
    "d is 5068898",
    "a variable left uninitialized with ---",
  ),
  ("09/9.1_types.jai", "4 bytes", "prose"),
  (
    "29/29.3_c_call.jai",
    "You have been logged!",
    "annotated after the print the program runs after it",
  ),
  (
    "06/6.6_random.jai",
    "15685560710023859762",
    "drawn after seeding from the clock",
  ),
  (
    "06/6.6_random.jai",
    "0.571844",
    "drawn after seeding from the clock",
  ),
  (
    "06/6.6_random.jai",
    "29.475393",
    "drawn after seeding from the clock",
  ),
  ("09/9.1_types.jai", "s64 int is an alias for s64", "prose"),
  ("13/13.1_unions.jai", "same as 6", "prose"),
  (
    "14/14.2_ifx.jai",
    "y4 is 7",
    "a typo against the format string, which prints y5",
  ),
  (
    "19/19.1_strings.jai",
    "chg_str is Hello",
    "a typo against the format string",
  ),
  (
    "19/19.1_strings.jai",
    "chg_str2 is Hello",
    "a typo against the format string",
  ),
  (
    "19/19.1_strings.jai",
    "chg_str3 is Hello",
    "a typo against the format string",
  ),
  (
    "10/10.1_pointers.jai",
    "3 prints out the value of a2 which is 3",
    "prose",
  ),
  (
    "10/10.4_dangling_pointers.jai",
    "5",
    "reads memory after freeing it",
  ),
  (
    "12/12.13_anonymous_struct.jai",
    "variable is u64",
    "the name a beta printed for an anonymous struct type, which L does not specify",
  ),
  (
    "13/13.1_unions.jai",
    "3.0",
    "the book's own recorded output says 3",
  ),
  (
    "16/16.2_type_info_proc.jai",
    "Type of infor is Type_Info_Struct",
    "a typo against the format string",
  ),
  (
    "18/18.2_static_arrays.jai",
    "arr2 is Hello 42 1 2 3 zero-initialized Any",
    "prose for how an empty Any prints",
  ),
  (
    "18/18.1_array_literals.jai",
    "numbers[2] = 5",
    "written before numbers[2] = 42 above it",
  ),
  (
    "18/18.3_dynamic_arrays.jai",
    "arrdyn is 13 9",
    "written before the pop above it removed 13",
  ),
  (
    "18/18.3_dynamic_arrays.jai",
    "another_array is 13 9",
    "written before the pop above it removed 13",
  ),
  (
    "18/18.3_dynamic_arrays.jai",
    "another_array2 is 13 9",
    "written before the pop above it removed 13",
  ),
  (
    "18/18.8_array_view_misuse.jai",
    "2487748255840 2487748277696 3 4 5 6 7 8 9 10",
    "reads past a view on purpose",
  ),
  ("19/19.2_bytes.jai", "65 ASCII value of A", "prose"),
  ("19/19.2_bytes.jai", "97 ASCII value of a", "prose"),
  (
    "23/23.6_interface.jai",
    "x has a name Kurt Russell and is the color RED",
    "a typo against the format string",
  ),
  ("26/26.10_soa.jai", "see in after program", "prose"),
  (
    "26/26.1_type_table.jai",
    "The type table has 355 entries",
    "the size of the reference's own type table",
  ),
  (
    "27/27.2_working_with_csv.jai",
    "see in after program",
    "prose",
  ),
  (
    "28/28.5_get_cpu_info_feature_flags.jai",
    "cpu_info INTEL 3219913727 4294636431 0 739248128 289 0 10250153 0 3154117632 0 15",
    "the author's CPU",
  ),
  (
    "29/29.1_call_c_linux.jai",
    "2446",
    "clock() on the author's machine",
  ),
  (
    "29/29.4_get_computer_name.jai",
    "OS is Windows",
    "the author's machine",
  ),
  (
    "26/26.5_if.jai",
    "name is Microsoft Windows",
    "the author's machine",
  ),
  (
    "30/30.12_placeholder.jai",
    "TRUTH is true is a constant true",
    "a typo against the format string",
  ),
  (
    "30/30.3_build.jai",
    "The workspace is 3",
    "a typo against the format string",
  ),
  (
    "31/31.5_num_threads.jai",
    "This machine reports 6 CPUs starting 5 threads",
    "the author's machine",
  ),
];

/// The lines a program must print, in order, as the book annotates them:
/// whatever follows `// =>` on a line, up to a later `//`, and each line of a
/// `/* => … */` block.
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

/// What the program reads from standard input, as the book writes it: the
/// `// <=` lines, each one a line typed in.
fn standard_input(source: &str) -> Option<String> {
  let typed: Vec<&str> = source
    .lines()
    .filter_map(|line| line.trim_start().strip_prefix("// <="))
    .map(str::trim)
    .collect();
  (!typed.is_empty()).then(|| typed.iter().map(|line| format!("{line}\n")).collect())
}

/// The same, one list per top-level declaration. The book annotates a
/// procedure where it is written, so what one declared above `main` prints is
/// expected in the order of that procedure's own lines but wherever `main`
/// calls it from.
fn annotation_groups(source: &str) -> Vec<Vec<String>> {
  let mut groups: Vec<Vec<String>> = vec![Vec::new()];
  let mut block_depth = 0usize;
  let mut brace_depth = 0usize;
  let mut after_commented_out = false;
  let mut in_output_block = false;
  for line in source.lines() {
    let depth_before = block_depth;
    block_depth = block_comment_depth_after(line, block_depth);
    if in_output_block {
      let text = line.split("*/").next().unwrap_or_default();
      let text = normalize(text);
      if !text.is_empty()
        && !machine_dependent(line)
        && let Some(group) = groups.last_mut()
      {
        group.push(text);
      }
      in_output_block = block_depth > 0;
      continue;
    }
    if depth_before > 0 {
      continue;
    }
    // `/* =>` opens a block of output, one line each, whatever follows the
    // arrow on its own line being a heading (`/* => during compile-time`).
    if line.trim_start().starts_with("/* =>") && block_depth == 1 {
      if brace_depth == 0 && groups.last().is_some_and(|group| !group.is_empty()) {
        groups.push(Vec::new());
      }
      in_output_block = true;
      continue;
    }
    let code = code_of(line);
    let standalone = line.trim_start().starts_with("// =>");
    let starts_declaration = brace_depth == 0 && !code.trim().is_empty();
    if (starts_declaration || (standalone && brace_depth == 0))
      && groups.last().is_some_and(|group| !group.is_empty())
    {
      groups.push(Vec::new());
    }
    brace_depth = braces_after(code, brace_depth);
    // A line of its own under code the book commented out describes that
    // code, not the program.
    if standalone && after_commented_out {
      continue;
    }
    if !standalone && !line.trim().is_empty() {
      after_commented_out = line.trim_start().starts_with("//");
    }
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

/// The words of a piece of text, without the punctuation between them and
/// without regard to case. The
/// book's annotations are written by hand beside the `print` that produces
/// them — `// => y is 42;`, `Hello Sailor!` for `Hello, Sailor!` — so what is
/// compared is the words and their order, not every comma.
fn normalize(text: &str) -> String {
  text
    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '-')
    .map(|word| word.trim_matches(|c: char| c == '.' || c == '-'))
    .filter(|word| !word.is_empty())
    .map(str::to_ascii_lowercase)
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

/// What a program is given on standard input.
enum Input {
  Nothing,
  Text(String),
  /// Each time `prompt` has been printed once more, the next reply is typed
  /// in; when they run out, standard input is closed.
  Converse {
    prompt: &'static str,
    replies: Vec<String>,
  },
}

/// How a run is expected to end.
enum Ending<'a> {
  /// On its own, within the limit.
  Exits(Duration),
  /// It is stopped after this long, and must still be running then.
  StoppedAfter(Duration),
  /// Once it has had `settle` to open its window, `close` asks every window
  /// on the display to close and says how many there were. A program that
  /// `exits` must then end within `limit`; one that does not is stopped
  /// after it.
  Closed {
    settle: Duration,
    limit: Duration,
    exits: bool,
    close: &'a dyn Fn() -> Result<usize, String>,
  },
}

/// How a run went: its exit status, or none when it was stopped.
struct Ran {
  status: Option<i32>,
  output: String,
}

/// Runs `command` with its standard output read back, feeding it `input`,
/// until it ends the way `ending` says.
fn run_program(mut command: Command, input: Input, ending: Ending<'_>) -> Result<Ran, String> {
  use std::io::Write;
  use std::sync::{Arc, Mutex};
  command
    .stdin(match input {
      Input::Nothing => Stdio::null(),
      _ => Stdio::piped(),
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
  let mut child = command
    .spawn()
    .map_err(|error| format!("could not start: {error}"))?;
  let mut stdout = child.stdout.take().expect("piped");
  let printed = Arc::new(Mutex::new(Vec::new()));
  let reader = {
    let printed = Arc::clone(&printed);
    std::thread::spawn(move || {
      let mut chunk = [0u8; 4096];
      while let Ok(count) = stdout.read(&mut chunk) {
        if count == 0 {
          break;
        }
        printed
          .lock()
          .unwrap_or_else(std::sync::PoisonError::into_inner)
          .extend_from_slice(&chunk[..count]);
      }
    })
  };
  let mut stdin = child.stdin.take();
  let (prompt, mut replies) = match input {
    Input::Nothing => (None, Vec::new().into_iter()),
    Input::Text(text) => {
      if let Some(mut pipe) = stdin.take() {
        let _ = pipe.write_all(text.as_bytes());
      }
      (None, Vec::new().into_iter())
    }
    Input::Converse { prompt, replies } => (Some(prompt), replies.into_iter()),
  };
  let stop = |child: &mut std::process::Child| {
    let _ = child.kill();
    let _ = child.wait();
  };
  let started = Instant::now();
  let mut answered = 0;
  let mut closed_at: Option<Instant> = None;
  let status = loop {
    if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
      if let Ending::Closed { .. } = ending
        && closed_at.is_none()
      {
        return Err(format!(
          "exited {:?} before its window was closed",
          status.code()
        ));
      }
      break status.code().or(Some(-1));
    }
    if let Some(prompt) = prompt {
      let asked = {
        let printed = printed
          .lock()
          .unwrap_or_else(std::sync::PoisonError::into_inner);
        String::from_utf8_lossy(&printed).matches(prompt).count()
      };
      while answered < asked {
        answered += 1;
        match (replies.next(), stdin.as_mut()) {
          (Some(reply), Some(pipe)) => {
            let _ = pipe.write_all(format!("{reply}\n").as_bytes());
            let _ = pipe.flush();
          }
          _ => stdin = None,
        }
      }
    }
    match &ending {
      Ending::Exits(limit) if started.elapsed() > *limit => {
        stop(&mut child);
        return Err(format!("still running after {}s", limit.as_secs()));
      }
      Ending::StoppedAfter(after) if started.elapsed() > *after => {
        stop(&mut child);
        break None;
      }
      Ending::Closed {
        settle,
        limit,
        exits,
        close,
      } => match closed_at {
        None if started.elapsed() > *settle => {
          let windows = close();
          if windows != Ok(0) && windows.is_ok() {
            closed_at = Some(Instant::now());
          } else {
            stop(&mut child);
            return Err(match windows {
              Err(reason) => format!("could not ask its window to close: {reason}"),
              Ok(_) => String::from("opened no window"),
            });
          }
        }
        Some(at) if at.elapsed() > *limit => {
          stop(&mut child);
          if *exits {
            return Err(format!(
              "still running {}s after its window was asked to close",
              limit.as_secs()
            ));
          }
          break None;
        }
        _ => {}
      },
      _ => {}
    }
    std::thread::sleep(Duration::from_millis(20));
  };
  drop(stdin);
  let _ = reader.join();
  let output = printed
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner)
    .clone();
  Ok(Ran {
    status,
    output: String::from_utf8_lossy(&output).into_owned(),
  })
}

/// Runs `executable` in `directory` with nothing on standard input, giving up
/// after `limit`.
fn run_for(executable: &Path, directory: &Path, limit: Duration) -> Result<(i32, String), String> {
  let mut command = Command::new(executable);
  command.current_dir(directory);
  let ran = run_program(command, Input::Nothing, Ending::Exits(limit))?;
  Ok((ran.status.unwrap_or(-1), ran.output))
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
  compile_through_metaprogram(path, import_dirs, &[])?
    .ok_or_else(|| String::from("no executable came out"))
}

/// The same with `arguments` after a lone `-`, for a build script, where what
/// came out is whatever the workspaces it made wrote rather than one program.
fn compile_through_metaprogram(
  path: &Path,
  import_dirs: &[PathBuf],
  arguments: &[&str],
) -> Result<Option<PathBuf>, String> {
  let Some(metaprogram) = oj_driver::default_metaprogram() else {
    return Err(String::from("no Default_Metaprogram to build with"));
  };
  let mut options = oj_driver::BuildOptions::new();
  options.import_dirs.extend(import_dirs.iter().cloned());
  let mut command_line = vec![path.display().to_string()];
  if !arguments.is_empty() {
    command_line.push(String::from("-"));
    command_line.extend(arguments.iter().map(|argument| argument.to_string()));
  }
  let report = oj_driver::run_through_metaprogram(
    &metaprogram,
    &[path.to_path_buf()],
    &command_line,
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
  Ok(report.executable)
}

/// Runs `body` with `directory` as the working directory, the way `jai` is
/// run from beside the program: a `#run` that reads `pixel.png` reads it from
/// there.
fn in_directory<T>(directory: &Path, body: impl FnOnce() -> T) -> T {
  let saved = std::env::current_dir().ok();
  let _ = std::env::set_current_dir(directory);
  let result = body();
  if let Some(saved) = saved {
    let _ = std::env::set_current_dir(saved);
  }
  result
}

/// The working directory and file descriptor 1 are the process's, so the
/// sweeps take turns rather than running side by side.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A virtual X server for the sweep's window programs, stopped when it is
/// dropped, with the helper that looks at and closes their windows.
struct Screen {
  server: Child,
  display: String,
  /// The system's dynamic loader, when the GL driver needs a newer C library
  /// than the dev shell's (see `crates/driver/tests/simp.rs`).
  loader: Option<(PathBuf, String)>,
  helper: PathBuf,
  /// An ALSA configuration whose default device is the null one, so that a
  /// program that plays sound plays it nowhere.
  quiet: PathBuf,
}

impl Drop for Screen {
  fn drop(&mut self) {
    let _ = self.server.kill();
    let _ = self.server.wait();
  }
}

/// Opens a window and says whether Simp got a GL context for it, or, with
/// `close`, asks every window on the display to close and prints how many it
/// asked.
const WINDOW_HELPER: &str = r#"#import "Basic";
#import "Simp";
#import "Window_Creation";
X :: #import "X11";

main :: () {
    args := get_command_line_arguments();
    if args.count > 1 && args[1] == "probe" {
        window := create_window(64, 48, "probe");
        set_render_target(window);
        print("%\n", ifx context.simp.current_window then "context" else "no context");
        return;
    }
    display := X.XOpenDisplay(null);
    if !display {
        print("no display\n");
        exit(1);
    }
    root := X.XDefaultRootWindow(display);
    root_return, parent: X.Window;
    children: *X.Window;
    count: u32;
    X.XQueryTree(display, root, *root_return, *parent, *children, *count);
    protocols := X.XInternAtom(display, "WM_PROTOCOLS", X.False);
    delete := X.XInternAtom(display, "WM_DELETE_WINDOW", X.False);
    closed := 0;
    for i: 0..cast(s64) count - 1 {
        window := children[i];
        attributes: X.XWindowAttributes;
        X.XGetWindowAttributes(display, window, *attributes);
        if attributes.map_state != X.IsViewable  continue;
        event: X.XEvent;
        event.xclient.type = X.ClientMessage;
        event.xclient.window = window;
        event.xclient.message_type = protocols;
        event.xclient.format = 32;
        event.xclient.data.l[0] = xx delete;
        X.XSendEvent(display, window, X.False, 0, *event);
        closed += 1;
    }
    X.XFlush(display);
    X.XCloseDisplay(display);
    print("%\n", closed);
}
"#;

impl Screen {
  /// A display with GLX a program can use, or why there is none.
  fn start(scratch: &Path) -> Result<Screen, &'static str> {
    use std::io::{BufRead, BufReader};
    let mut server = Command::new("Xvfb")
      .args([
        "-displayfd",
        "1",
        "-screen",
        "0",
        "1280x1024x24",
        "-nolisten",
        "tcp",
      ])
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|_| "there is no Xvfb to open it on")?;
    let mut line = String::new();
    if let Some(stdout) = server.stdout.take() {
      let _ = BufReader::new(stdout).read_line(&mut line);
    }
    let number = line.trim().to_string();
    let directory = scratch.join("screen");
    let _ = std::fs::create_dir_all(&directory);
    let quiet = directory.join("alsa.conf");
    let _ = std::fs::write(
      &quiet,
      "pcm.!default { type null }\nctl.!default { type hw card 0 }\n",
    );
    let mut screen = Screen {
      server,
      display: format!(":{number}"),
      loader: None,
      helper: PathBuf::new(),
      quiet,
    };
    if number.is_empty() {
      return Err("Xvfb did not start");
    }
    let source = directory.join("window_helper.jai");
    let _ = std::fs::write(&source, WINDOW_HELPER);
    screen.helper =
      build(&source, &directory).map_err(|_| "the helper that closes its windows did not build")?;
    if screen.probe() {
      return Ok(screen);
    }
    if let Some(loader) = std::env::var_os("OJ_TEST_GL_LOADER") {
      let library_path = std::env::var("OJ_TEST_GL_LIBRARY_PATH")
        .unwrap_or_else(|_| String::from("/run/opengl-driver/lib"));
      screen.loader = Some((PathBuf::from(loader), library_path));
      if screen.probe() {
        return Ok(screen);
      }
    }
    Err("the virtual display has no GLX a program can use (see OJ_TEST_GL_LOADER)")
  }

  fn probe(&self) -> bool {
    let mut command = self.command(&self.helper);
    command.arg("probe");
    run_program(
      command,
      Input::Nothing,
      Ending::Exits(Duration::from_secs(10)),
    )
    .is_ok_and(|ran| ran.output.trim() == "context")
  }

  /// `executable` as a program on this display runs, through the loader
  /// when it needs one.
  fn command(&self, executable: &Path) -> Command {
    let mut command = match &self.loader {
      Some((loader, library_path)) => {
        let mut command = Command::new(loader);
        command
          .arg("--library-path")
          .arg(library_path)
          .arg(executable);
        command
      }
      None => Command::new(executable),
    };
    command
      .env("DISPLAY", &self.display)
      .env("ALSA_CONFIG_PATH", &self.quiet);
    command
  }

  fn close_windows(&self) -> Result<usize, String> {
    let mut command = Command::new(&self.helper);
    command.env("DISPLAY", &self.display);
    let ran = run_program(
      command,
      Input::Nothing,
      Ending::Exits(Duration::from_secs(10)),
    )?;
    ran
      .output
      .trim()
      .parse()
      .map_err(|_| format!("the helper printed {:?}", ran.output))
  }
}

/// What one sweep of the book shares between its programs.
struct Sweep {
  root: PathBuf,
  scratch: PathBuf,
  screen: Option<Result<Screen, &'static str>>,
}

impl Sweep {
  fn screen(&mut self) -> Result<&Screen, &'static str> {
    let scratch = self.scratch.clone();
    self
      .screen
      .get_or_insert_with(|| Screen::start(&scratch))
      .as_ref()
      .map_err(|reason| *reason)
  }
}

fn judge_ttwj(sweep: &mut Sweep, path: &Path) -> Verdict {
  let root = sweep.root.clone();
  let examples = root.join("examples");
  let relative = path
    .strip_prefix(&examples)
    .unwrap_or(path)
    .to_string_lossy()
    .replace('\\', "/");
  if let Some((_, reason)) = TTWJ_SKIPS.iter().find(|(name, _)| *name == relative) {
    return Verdict::Skipped(reason);
  }
  let plan = plan_of(&relative);
  match plan {
    Plan::PartOf(host) => return judge_ttwj(sweep, &examples.join(host)),
    Plan::BuildScript {
      arguments,
      runs,
      expected,
    } => return judge_build_script(sweep, path, &relative, arguments, runs, expected),
    _ => {}
  }
  let source = std::fs::read_to_string(path).unwrap_or_default();
  let mut expected = annotation_groups(&source);
  for group in &mut expected {
    group.retain(|text| {
      !TTWJ_ERRATA
        .iter()
        .any(|(name, annotation, _)| *name == relative && normalize(annotation) == *text)
    });
  }
  let output = sweep.scratch.join(relative.replace('/', "_"));
  let _ = std::fs::create_dir_all(&output);
  // The book compiles the chapters that import a module of its own with
  // `-import_dir my_modules` (§8.6). What a `#run` prints while it compiles
  // is part of what the book shows, so it is read along with the program's
  // own output; a program that creates workspaces of its own is compiled the
  // way the book compiles it, through the metaprogram.
  let import_dirs = [root.join("my_modules")];
  let drives_the_compiler = source.contains("#import \"Compiler\"");
  let directory = path.parent().unwrap_or(&root).to_path_buf();
  let (built, compiled) = in_directory(&directory, || {
    capturing_stdout(|| match drives_the_compiler {
      true => build_through_metaprogram(path, &import_dirs),
      false => build_with(path, &output, &import_dirs),
    })
  });
  let executable = match built {
    Ok(executable) => executable,
    Err(first) => return Verdict::BuildFailed(first),
  };
  let input = match standard_input(&source) {
    Some(text) => Input::Text(text),
    None => Input::Nothing,
  };
  let limit = Duration::from_secs(10);
  let ran = match plan {
    Plan::Converse {
      prompt,
      replies,
      expected: also,
    } => {
      expected.push(also.iter().map(|line| normalize(line)).collect());
      let mut command = Command::new(&executable);
      command.current_dir(&directory);
      let input = Input::Converse {
        prompt,
        replies: replies(),
      };
      run_program(command, input, Ending::Exits(limit))
    }
    Plan::DeleteDirectories => return judge_delete_directories(sweep, &executable, &relative),
    Plan::UntilStopped => {
      let mut command = Command::new(&executable);
      command.current_dir(&directory);
      match run_program(command, input, Ending::StoppedAfter(Duration::from_secs(3))) {
        Ok(Ran {
          status: Some(status),
          ..
        }) => return Verdict::RunFailed(format!("exited {status} rather than running on")),
        other => other,
      }
    }
    Plan::Window { closes } => {
      let screen = match sweep.screen() {
        Ok(screen) => screen,
        Err(reason) => return Verdict::Skipped(reason),
      };
      let mut command = screen.command(&executable);
      command.current_dir(&directory);
      let close = || screen.close_windows();
      let ending = Ending::Closed {
        settle: Duration::from_secs(3),
        limit: Duration::from_secs(5),
        exits: closes,
        close: &close,
      };
      run_program(command, input, ending)
    }
    _ => {
      let mut command = Command::new(&executable);
      command.current_dir(&directory);
      run_program(command, input, Ending::Exits(limit))
    }
  };
  match ran {
    Err(reason) => Verdict::RunFailed(reason),
    Ok(ran) => match matches_groups(&format!("{compiled}\n{}", ran.output), &expected) {
      Ok(()) if matches!(ran.status, None | Some(0)) => Verdict::Pass,
      Ok(()) => Verdict::RunFailed(format!("exited {}", ran.status.unwrap_or(-1))),
      Err(missing) => Verdict::Mismatch(missing),
    },
  }
}

/// A build script, compiled in a copy of its chapter so that what it writes
/// does not land in the checkout, and then the program it built.
fn judge_build_script(
  sweep: &Sweep,
  path: &Path,
  relative: &str,
  arguments: &[&str],
  runs: &str,
  expected: &[&str],
) -> Verdict {
  let directory = sweep.scratch.join(relative.replace('/', "_"));
  let _ = std::fs::create_dir_all(&directory);
  let chapter = path.parent().unwrap_or(&sweep.root);
  for entry in std::fs::read_dir(chapter).into_iter().flatten().flatten() {
    let from = entry.path();
    if from.is_file() {
      let _ = std::fs::copy(&from, directory.join(entry.file_name()));
    }
  }
  let script = directory.join(path.file_name().unwrap_or_default());
  let import_dirs = [sweep.root.join("my_modules")];
  let (built, compiled) = in_directory(&directory, || {
    capturing_stdout(|| compile_through_metaprogram(&script, &import_dirs, arguments))
  });
  if let Err(first) = built {
    return Verdict::BuildFailed(first);
  }
  let program = directory.join(runs);
  if !program.is_file() {
    return Verdict::BuildFailed(format!("it built no {runs}"));
  }
  let expected = [expected.iter().map(|line| normalize(line)).collect()];
  let mut command = Command::new(&program);
  command.current_dir(&directory);
  match run_program(
    command,
    Input::Nothing,
    Ending::Exits(Duration::from_secs(10)),
  ) {
    Err(reason) => Verdict::RunFailed(reason),
    Ok(ran) => match matches_groups(&format!("{compiled}\n{}", ran.output), &expected) {
      Ok(()) if ran.status == Some(0) => Verdict::Pass,
      Ok(()) => Verdict::RunFailed(format!("{runs} exited {}", ran.status.unwrap_or(-1))),
      Err(missing) => Verdict::Mismatch(missing),
    },
  }
}

/// The book's 27.4 run the way its text shows it: with no arguments it says
/// how to use it and exits 1, and run in a tree with a name it removes every
/// directory of that name below, and nothing else.
fn judge_delete_directories(sweep: &Sweep, executable: &Path, relative: &str) -> Verdict {
  let tree = sweep
    .scratch
    .join(format!("{}_tree", relative.replace('/', "_")));
  let doomed = [tree.join("keep/doomed"), tree.join("a/b/doomed")];
  let kept = [tree.join("keep"), tree.join("a/b"), tree.join("a/kept")];
  for directory in doomed.iter().chain(&kept) {
    let _ = std::fs::create_dir_all(directory.join("inner"));
  }
  let run = |arguments: &[&str]| {
    let mut command = Command::new(executable);
    command.args(arguments).current_dir(&tree);
    run_program(
      command,
      Input::Nothing,
      Ending::Exits(Duration::from_secs(10)),
    )
  };
  // 27A_Working_with_Files.md: "Here are some invocations of the program and
  // their output", from a directory of the author's.
  let usage = [normalize(
    "[to-be-deleted_directory] [start_directory] Start directory will be working directory if not set.",
  )];
  match run(&[]) {
    Err(reason) => return Verdict::RunFailed(reason),
    Ok(ran) if ran.status != Some(1) => {
      return Verdict::RunFailed(format!(
        "exited {} without arguments rather than 1",
        ran.status.unwrap_or(-1)
      ));
    }
    Ok(ran) => {
      if let Err(missing) = matches_in_order(&ran.output, &usage) {
        return Verdict::Mismatch(missing);
      }
    }
  }
  let here = tree.display().to_string();
  let removed = [
    format!("Working directory: {here}"),
    String::from("Removing directories named 'doomed'..."),
    format!("Removed all directories named 'doomed' from {here}!"),
  ]
  .map(|line| normalize(&line));
  match run(&["doomed"]) {
    Err(reason) => Verdict::RunFailed(reason),
    Ok(ran) if ran.status != Some(0) => {
      Verdict::RunFailed(format!("exited {}", ran.status.unwrap_or(-1)))
    }
    Ok(ran) => {
      if let Err(missing) = matches_in_order(&ran.output, &removed) {
        return Verdict::Mismatch(missing);
      }
      let left: Vec<String> = doomed
        .iter()
        .filter(|path| path.exists())
        .chain(kept.iter().filter(|path| !path.exists()))
        .map(|path| path.display().to_string())
        .collect();
      match left.is_empty() {
        true => Verdict::Pass,
        false => Verdict::RunFailed(format!("removed the wrong directories: {left:?}")),
      }
    }
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

fn checkout(variable: &str) -> Option<(PathBuf, std::sync::MutexGuard<'static, ()>)> {
  let directory = PathBuf::from(std::env::var_os(variable)?);
  let turn = ONE_AT_A_TIME
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);
  // SAFETY: cargo runs each integration test binary in its own process.
  unsafe { oj_testsupport::use_own_modules() };
  Some((directory, turn))
}

#[test]
#[ignore = "needs a checkout of Ivo-Balbaert/The_Way_to_Jai in OJ_TTWJ_DIR"]
fn the_way_to_jai() {
  let Some((root, _turn)) = checkout("OJ_TTWJ_DIR") else {
    return;
  };
  let scratch = tempfile::tempdir().expect("a scratch directory");
  let mut sweep = Sweep {
    root: root.clone(),
    scratch: scratch.path().to_path_buf(),
    screen: None,
  };
  let verdicts: Vec<(PathBuf, Verdict)> = programs_under(&root.join("examples"), 2)
    .into_iter()
    .map(|path| {
      let verdict = judge_ttwj(&mut sweep, &path);
      (path, verdict)
    })
    .collect();
  drop(sweep);
  report("The Way to Jai", &verdicts);
}

#[test]
#[ignore = "needs a checkout of mimhufford/AdventOfCode in OJ_AOC_DIR"]
fn advent_of_code() {
  // Each year is one program, `YYYY/YYYY.jai`, which `#load`s the days it
  // finds and runs every one against `input/`. The answers are nowhere in the
  // repository, so a year passes when it builds and runs to the end.
  let Some((root, _turn)) = checkout("OJ_AOC_DIR") else {
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
  let Some((root, _turn)) = checkout("OJ_TOML_JAI_DIR") else {
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
      if *relative == "tools/toml-test/run_toml-tests.jai" {
        return (
          path,
          Verdict::Skipped("builds its decoder by running `jai`, then runs the toml-test binary"),
        );
      }
      let output = scratch.path().join(index.to_string());
      let _ = std::fs::create_dir_all(&output);
      let directory = path.parent().unwrap_or(&root).to_path_buf();
      let verdict = match in_directory(&directory, || build(&path, &output)) {
        Err(first) => Verdict::BuildFailed(first),
        Ok(executable) => match run_for(&executable, &directory, Duration::from_secs(60)) {
          Ok((0, _)) => Verdict::Pass,
          Ok((2, _)) => Verdict::Skipped("the test skipped itself"),
          Ok((status, _)) => Verdict::RunFailed(format!("exited {status}")),
          Err(reason) => Verdict::RunFailed(reason),
        },
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
  // One under a line the book commented out is about that line, and one
  // outside every declaration is a group of its own.
  let source = "main :: () {\n\
                  // print(\"gone\");\n\
                  // => gone\n\
                  print(\"kept\");\n\
                  // => kept\n\
                }\n\
                // => whole\n";
  assert_eq!(annotation_groups(source), [vec!["kept"], vec!["whole"]]);
  assert_eq!(
    matches_groups("first\nin helper\n42after\n", &groups),
    Ok(())
  );
  assert_eq!(
    matches_groups("after\nin helper\nfirst 42\n", &groups),
    Err(String::from("after"))
  );
}

#[test]
fn a_block_after_an_arrow_is_output_and_a_reverse_arrow_is_input() {
  let source = "main :: () {\n\
                  print(\"one\\n\");\n\
                  /* =>\n\
                  numbers[0] = 1\n\
                  numbers[1] = 3\n\
                  */\n\
                  /*\n\
                  not output\n\
                  */\n\
                  // <= typed in\n\
                }\n\
                /* => during compile-time\n\
                The code is: a := 1\n\
                */\n";
  assert_eq!(
    annotation_groups(source),
    [vec!["numbers 0 1", "numbers 1 3"], vec!["the code is a 1"]]
  );
  assert_eq!(standard_input(source).as_deref(), Some("typed in\n"));
  assert_eq!(standard_input("print(1); // <= not input\n"), None);
}

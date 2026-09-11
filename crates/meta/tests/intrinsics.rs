//! Every symbol this compiler answers has to be one the distribution actually
//! declares `#compiler`, under exactly that name (**C§3.3**). A typo here
//! would leave the JIT unable to bind a call, so the vendored module is the
//! test.

use std::collections::HashSet;

/// The symbols `Compiler.jai` declares `#compiler`: the procedure's own name,
/// or the string a `#compiler "name"` gave it.
fn declared_symbols(text: &str) -> HashSet<String> {
  let mut symbols = HashSet::new();
  for line in text.lines() {
    // A trailing comment may say `#compiler` about something that is not a
    // declaration — `SYNTACTICALLY_MARKED_AS_COMPILER` explains itself that
    // way — so only the code counts.
    let line = line.trim_start();
    let line = line.split_once("//").map_or(line, |(code, _)| code);
    if !line.contains("#compiler") {
      continue;
    }
    let Some((head, _)) = line.split_once("::") else {
      continue;
    };
    let name = head.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
      continue;
    }
    // Only what stands between `#compiler` and the `;` renames it; a trailing
    // comment may have quotes of its own.
    let tail = &line[line.find("#compiler").expect("just checked") + "#compiler".len()..];
    let tail = tail.split_once(';').map_or(tail, |(head, _)| head);
    let renamed = tail
      .split_once('"')
      .and_then(|(_, rest)| rest.split_once('"'))
      .map(|(name, _)| name.to_string());
    symbols.insert(renamed.unwrap_or_else(|| name.to_string()));
  }
  symbols
}

#[test]
fn every_symbol_the_compiler_answers_is_one_the_module_declares() {
  // Preload declares a couple of them too, `get_current_workspace` among
  // them, and so does Runtime_Support: `write_string` is the compiler's at
  // compile time and the program's own at runtime.
  let paths = [
    oj_testsupport::modules().join("Compiler/Compiler.jai"),
    oj_testsupport::modules().join("Preload.jai"),
    oj_testsupport::modules().join("Runtime_Support.jai"),
  ];
  let mut declared = HashSet::new();
  for path in &paths {
    let text = std::fs::read_to_string(path).expect("the module should be readable");
    declared.extend(declared_symbols(&text));
  }
  assert!(
    declared.contains("compiler_create_workspace"),
    "the scan should find the obvious ones: {declared:?}"
  );

  for (symbol, _) in oj_meta::intrinsics() {
    assert!(
      declared.contains(*symbol),
      "'{symbol}' is not declared #compiler anywhere in the distribution"
    );
  }
}

#[test]
fn no_symbol_is_answered_twice() {
  let mut seen = HashSet::new();
  for (symbol, _) in oj_meta::intrinsics() {
    assert!(seen.insert(*symbol), "'{symbol}' is answered twice");
  }
}

#[test]
fn a_call_with_no_compilation_installed_does_nothing() {
  assert!(oj_meta::uninstall().is_none());
  assert!(oj_meta::with(|meta| meta.workspaces.len()).is_none());
}

/// The `#compiler` procedures the distribution declares and this compiler does
/// not answer yet. A call to one of these fails the `#run` that made it,
/// naming the procedure, rather than being silently wrong — so the list is
/// pinned here, and shrinking it is what finishing M8 means.
#[test]
fn the_symbols_still_unanswered_are_the_ones_the_spec_names() {
  let paths = [
    oj_testsupport::modules().join("Compiler/Compiler.jai"),
    oj_testsupport::modules().join("Preload.jai"),
    oj_testsupport::modules().join("Runtime_Support.jai"),
  ];
  let mut declared = HashSet::new();
  for path in &paths {
    let text = std::fs::read_to_string(path).expect("the module should be readable");
    declared.extend(declared_symbols(&text));
  }
  let answered: HashSet<&str> = oj_meta::intrinsics()
    .iter()
    .map(|(symbol, _)| *symbol)
    .collect();
  let mut unanswered: Vec<&str> = declared
    .iter()
    .map(String::as_str)
    .filter(|symbol| !answered.contains(symbol))
    .collect();
  unanswered.sort_unstable();

  // `get_runtime_info` is the one symbol left, and it needs no answer: it is
  // declared `#compiler` but *has a body*, which reads `__runtime_info:
  // Runtime_Info #elsewhere`. Nothing binds that symbol — the compiler defines
  // the data instead, as the head of the type table image — so the body is
  // what runs.
  assert_eq!(unanswered, ["get_runtime_info"]);
}

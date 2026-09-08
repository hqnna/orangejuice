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
    let line = line.trim_start();
    if line.starts_with("//") || !line.contains("#compiler") {
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
  let Some(jai_dir) = oj_testsupport::jai_dir() else {
    eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
    return;
  };
  // Preload declares a couple of them too, `get_current_workspace` among them.
  let paths = [
    jai_dir.join("modules/Compiler/Compiler.jai"),
    jai_dir.join("modules/Preload.jai"),
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

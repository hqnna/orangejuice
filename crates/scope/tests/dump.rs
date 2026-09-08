use std::path::Path;

use oj_diag::SourceMap;
use oj_lexer::Interner;
use oj_scope::{Options, Program};

/// The scope tree `oj dump scopes` prints, over a file that uses each kind of
/// scope and each way of getting a name into one.
#[test]
fn the_scope_tree_of_a_tour() {
  let root = Path::new(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/inputs/tour.jai"
  ));
  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    root,
    Options {
      load_preload: false,
      ..Options::default()
    },
  );

  let errors: Vec<String> = program
    .diagnostics()
    .iter()
    .filter(|diagnostic| diagnostic.is_error())
    .map(|diagnostic| diagnostic.message.clone())
    .collect();
  assert_eq!(errors, Vec::<String>::new());

  // The dump names files by absolute path, which differs per checkout.
  let directory = root.parent().unwrap().display().to_string();
  let dump = oj_scope::print_scopes(&program).replace(&directory, "<inputs>");
  insta::assert_snapshot!("the_scope_tree_of_a_tour", dump);
}

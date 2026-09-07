use oj_diag::SourceMap;
use oj_lexer::Interner;

/// The `oj dump tokens` rendering, over a file that exercises every part of
/// **L§2**: comments, notes, here-strings, every literal form, backticks,
/// backslash continuations and the operator table.
#[test]
fn the_token_dump_of_a_lexical_tour() {
  let source = include_bytes!("inputs/tour.jai");
  let sources = SourceMap::new();
  let id = sources.add_bytes("tour.jai", source.to_vec());
  let file = sources.file(id);
  let interner = Interner::new();

  let lexed = oj_lexer::tokenize(file.bytes(), id, &interner);
  assert!(
    lexed.diagnostics.is_empty(),
    "the tour should lex cleanly: {:?}",
    lexed.diagnostics
  );

  insta::assert_snapshot!(oj_lexer::dump_tokens(&lexed.tokens, &file, &interner));
}

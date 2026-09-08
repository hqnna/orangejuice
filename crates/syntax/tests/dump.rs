use oj_diag::{Severity, SourceId};
use oj_lexer::Interner;

const TOUR: &[u8] = include_bytes!("inputs/tour.jai");

fn parse(source: &[u8], interner: &Interner, id: u32) -> oj_syntax::Parsed {
  let parsed = oj_syntax::parse(source, SourceId(id), interner);
  let errors: Vec<&oj_diag::Diagnostic> = parsed
    .diagnostics
    .iter()
    .filter(|diagnostic| diagnostic.severity == Severity::Error)
    .collect();
  assert!(errors.is_empty(), "the tour should parse: {errors:#?}");
  parsed
}

/// The source form of `oj dump ast`, over a file that exercises the grammar of
/// **L§18**.
#[test]
fn the_source_form_of_a_grammar_tour() {
  let interner = Interner::new();
  let parsed = parse(TOUR, &interner, 0);

  insta::assert_snapshot!(oj_syntax::print_source(&parsed.ast, parsed.root, &interner));
}

/// The tree form of `oj dump ast --tree` over the same file.
#[test]
fn the_tree_form_of_a_grammar_tour() {
  let interner = Interner::new();
  let parsed = parse(TOUR, &interner, 0);

  insta::assert_snapshot!(oj_syntax::print_tree(&parsed.ast, parsed.root, &interner));
}

/// The printed form parses back to the same tree (`docs/spec.md` §8).
#[test]
fn the_tour_round_trips_through_its_printed_form() {
  let interner = Interner::new();
  let parsed = parse(TOUR, &interner, 0);
  let printed = oj_syntax::print_source(&parsed.ast, parsed.root, &interner);

  let reparsed = parse(printed.as_bytes(), &interner, 1);

  assert_eq!(
    oj_syntax::print_tree(&parsed.ast, parsed.root, &interner),
    oj_syntax::print_tree(&reparsed.ast, reparsed.root, &interner)
  );
}

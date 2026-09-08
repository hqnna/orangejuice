use std::collections::BTreeMap;

use oj_diag::{Diagnostic, SourceId, Span};
use oj_lexer::Symbol;

use crate::program::{Program, Reference};
use crate::tree::Resolution;

/// One identifier that resolved to nothing, and the closest names that were in
/// scope where it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Undeclared {
  pub name: String,
  pub source: SourceId,
  pub span: Span,
  pub suggestions: Vec<String>,
}

/// Resolves every recorded reference and returns the ones that failed, sorted
/// by file and position. Undeclared identifiers are reported in bulk rather
/// than one at a time, the way the reference batches them (**L§11.7**,
/// **C§6.1**); a reference in a scope that could still gain names is a wait,
/// not a failure.
pub fn undeclared_identifiers(program: &Program<'_>) -> Vec<Undeclared> {
  let tree = program.tree();
  let interner = program.interner();
  let mut batched: BTreeMap<(SourceId, Span, Symbol), Undeclared> = BTreeMap::new();

  for reference in program.references().iter() {
    if reference.speculative
      || program.is_macro_injected(reference.name)
      || program.is_in_macro(reference.scope)
      || tree.lookup(reference.scope, reference.name) != Resolution::Undeclared
    {
      continue;
    }
    let name = interner.resolve_lossy(reference.name).into_owned();
    let suggestions = suggestions(program, reference, &name);
    batched.insert(
      (reference.source, reference.span, reference.name),
      Undeclared {
        name,
        source: reference.source,
        span: reference.span,
        suggestions,
      },
    );
  }

  batched.into_values().collect()
}

/// Renders the batch the way the compiler reports it: one error per identifier,
/// each followed by the near misses that were in scope.
pub fn undeclared_diagnostics(undeclared: &[Undeclared]) -> Vec<Diagnostic> {
  let mut diagnostics = Vec::new();
  for entry in undeclared {
    diagnostics.push(Diagnostic::error(
      entry.source,
      entry.span,
      format!("Undeclared identifier '{}'.", entry.name),
    ));
    if !entry.suggestions.is_empty() {
      diagnostics.push(Diagnostic::info(
        entry.source,
        entry.span,
        format!("Did you mean: {}?", entry.suggestions.join(", ")),
      ));
    }
  }
  diagnostics
}

const MAX_SUGGESTIONS: usize = 3;

fn suggestions(program: &Program<'_>, reference: &Reference, name: &str) -> Vec<String> {
  let tree = program.tree();
  let interner = program.interner();
  let mut scored: Vec<(usize, String)> = Vec::new();
  let mut scope = Some(reference.scope);

  while let Some(id) = scope {
    for candidate in tree.names(id) {
      let candidate = interner.resolve_lossy(candidate).into_owned();
      if candidate == name {
        continue;
      }
      let distance = edit_distance(name, &candidate);
      if distance <= tolerance(name) {
        scored.push((distance, candidate));
      }
    }
    scope = tree.parent(id);
  }

  scored.sort();
  scored.dedup_by(|left, right| left.1 == right.1);
  scored.truncate(MAX_SUGGESTIONS);
  scored.into_iter().map(|(_, name)| name).collect()
}

fn tolerance(name: &str) -> usize {
  match name.chars().count() {
    0..=3 => 1,
    4..=7 => 2,
    _ => 3,
  }
}

/// Levenshtein distance over bytes; identifiers are ASCII (**L§2.3**).
fn edit_distance(left: &str, right: &str) -> usize {
  let (left, right) = (left.as_bytes(), right.as_bytes());
  let mut previous: Vec<usize> = (0..=right.len()).collect();
  let mut current = vec![0; right.len() + 1];

  for (i, left_byte) in left.iter().enumerate() {
    current[0] = i + 1;
    for (j, right_byte) in right.iter().enumerate() {
      let substitution = previous[j] + usize::from(left_byte != right_byte);
      current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
    }
    std::mem::swap(&mut previous, &mut current);
  }
  previous[right.len()]
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn edit_distance_counts_single_edits() {
    assert_eq!(edit_distance("print", "print"), 0);
    assert_eq!(edit_distance("print", "prints"), 1);
    assert_eq!(edit_distance("print", "pront"), 1);
    assert_eq!(edit_distance("", "abc"), 3);
  }

  #[test]
  fn short_names_tolerate_less_than_long_ones() {
    assert_eq!(tolerance("ab"), 1);
    assert_eq!(tolerance("abcd"), 2);
    assert_eq!(tolerance("abcdefghij"), 3);
  }
}

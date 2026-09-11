use std::path::{Path, PathBuf};

use oj_diag::{Severity, SourceMap};
use oj_lexer::Interner;
use walkdir::WalkDir;

fn jai_files(root: &Path) -> Vec<PathBuf> {
  let mut files: Vec<PathBuf> = WalkDir::new(root)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_file())
    .map(|entry| entry.into_path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "jai"))
    // A `.build` directory holds what a compiler left beside a program,
    // not part of the distribution: whoever has built one of its programs in
    // place has one.
    .filter(|path| {
      !path
        .components()
        .any(|component| component.as_os_str() == ".build")
    })
    .collect();
  files.sort();
  files
}

/// Every file of the distribution parses, and the printed form
/// of its tree parses back to the same tree (`docs/spec.md` §8).
#[test]
fn every_file_of_the_distribution_parses_and_round_trips() {
  let mut files = jai_files(&oj_testsupport::modules());
  files.extend(jai_files(&oj_testsupport::examples()));
  files.sort();
  assert!(
    files.len() > 70,
    "expected the distribution's own .jai corpus, found {} files",
    files.len()
  );

  let sources = SourceMap::new();
  let interner = Interner::new();
  let mut failures = Vec::new();
  let mut mismatches = Vec::new();

  for path in &files {
    let id = oj_source::load_file(&sources, path).expect("a corpus file should be readable");
    let file = sources.file(id);
    let parsed = oj_syntax::parse(file.bytes(), id, &interner);

    let errors: Vec<String> = parsed
      .diagnostics
      .iter()
      .filter(|diagnostic| diagnostic.severity == Severity::Error)
      .map(|diagnostic| oj_diag::render(diagnostic, &file).trim_end().to_string())
      .collect();
    if !errors.is_empty() {
      failures.push(format!("{}:\n{}", path.display(), errors.join("\n")));
      continue;
    }

    let printed = oj_syntax::print_source(&parsed.ast, parsed.root, &interner);
    let printed_id = sources.add_string(path, &printed);
    let printed_file = sources.file(printed_id);
    let reparsed = oj_syntax::parse(printed_file.bytes(), printed_id, &interner);

    let reparse_errors: Vec<String> = reparsed
      .diagnostics
      .iter()
      .filter(|diagnostic| diagnostic.severity == Severity::Error)
      .map(|diagnostic| {
        oj_diag::render(diagnostic, &printed_file)
          .trim_end()
          .to_string()
      })
      .collect();
    if !reparse_errors.is_empty() {
      mismatches.push(format!(
        "{} (printed form does not parse):\n{}",
        path.display(),
        reparse_errors.join("\n")
      ));
      continue;
    }

    let before = oj_syntax::print_tree(&parsed.ast, parsed.root, &interner);
    let after = oj_syntax::print_tree(&reparsed.ast, reparsed.root, &interner);
    if before != after {
      mismatches.push(format!(
        "{} (round trip changed the tree):\n{}",
        path.display(),
        first_difference(&before, &after)
      ));
    }
  }

  assert!(
    failures.is_empty(),
    "{} of {} corpus files failed to parse:\n{}",
    failures.len(),
    files.len(),
    failures.join("\n\n")
  );
  assert!(
    mismatches.is_empty(),
    "{} of {} corpus files did not round-trip:\n{}",
    mismatches.len(),
    files.len(),
    mismatches.join("\n\n")
  );
}

fn first_difference(before: &str, after: &str) -> String {
  for (index, (left, right)) in before.lines().zip(after.lines()).enumerate() {
    if left != right {
      return format!("line {}:\n  before: {left}\n  after:  {right}", index + 1);
    }
  }
  format!(
    "line counts differ: {} before, {} after",
    before.lines().count(),
    after.lines().count()
  )
}

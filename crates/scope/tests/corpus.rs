use std::path::{Path, PathBuf};

use oj_diag::{Severity, SourceMap};
use oj_lexer::Interner;
use oj_scope::{Options, Program};
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

fn rendered_errors(program: &Program<'_>, sources: &SourceMap) -> Vec<String> {
  program
    .diagnostics()
    .iter()
    .filter(|diagnostic| diagnostic.severity == Severity::Error)
    .map(|diagnostic| {
      oj_diag::render(diagnostic, &sources.file(diagnostic.source))
        .trim_end()
        .to_string()
    })
    .collect()
}

/// Every file of the distribution resolves into a scope tree
/// (`docs/spec.md` §8). Each file is resolved on its own, so nothing is looked
/// up across an import: what this checks is that the tree builds and that no
/// file contradicts itself — a redeclaration, a doubled `#load`, an
/// unresolvable module path.
#[test]
fn every_file_of_the_distribution_resolves_into_a_scope_tree() {
  let mut files = jai_files(&oj_testsupport::modules());
  files.extend(jai_files(&oj_testsupport::examples()));
  files.sort();
  assert!(
    files.len() > 70,
    "expected the distribution's own .jai corpus, found {} files",
    files.len()
  );

  let mut failures = Vec::new();
  let mut scopes = 0;
  let mut declarations = 0;

  for path in &files {
    let sources = SourceMap::new();
    let interner = Interner::new();
    let program = Program::build(&sources, &interner, path, Options::single_file());

    let errors = rendered_errors(&program, &sources);
    if !errors.is_empty() {
      failures.push(format!("{}:\n{}", path.display(), errors.join("\n")));
      continue;
    }
    assert_eq!(
      program.unit_count(),
      1,
      "{} should have resolved as exactly one file",
      path.display()
    );
    scopes += program.tree().scope_count();
    declarations += program.tree().declaration_count();
  }

  assert!(
    failures.is_empty(),
    "{} of {} corpus files failed to resolve:\n{}",
    failures.len(),
    files.len(),
    failures.join("\n\n")
  );
  assert!(scopes > files.len() * 4, "{scopes} scopes looks too small");
  assert!(
    declarations > 10_000,
    "{declarations} declarations looks too small"
  );
}

/// The `examples` suite is the acceptance suite (`docs/spec.md` §8): each of
/// its programs is resolved as a whole — its `#load`s, its modules and Preload —
/// and every identifier in it has to resolve.
#[test]
fn every_example_program_resolves_every_identifier() {
  let examples = oj_testsupport::examples();
  let mut roots: Vec<PathBuf> = jai_files(&examples)
    .into_iter()
    .filter(|path| path.parent() == Some(examples.as_path()))
    .collect();
  roots.sort();
  assert!(roots.len() > 10, "expected the examples suite");

  let mut failures = Vec::new();
  for path in &roots {
    let sources = SourceMap::new();
    let interner = Interner::new();
    let program = Program::build(
      &sources,
      &interner,
      path,
      Options {
        distribution: Some(oj_testsupport::distribution().to_path_buf()),
        ..Options::default()
      },
    );

    let mut errors = rendered_errors(&program, &sources);
    errors.extend(
      oj_scope::undeclared_diagnostics(&oj_scope::undeclared_identifiers(&program))
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| {
          oj_diag::render(diagnostic, &sources.file(diagnostic.source))
            .trim_end()
            .to_string()
        }),
    );
    if !errors.is_empty() {
      failures.push(format!("{}:\n{}", path.display(), errors.join("\n")));
    }
  }

  assert!(
    failures.is_empty(),
    "{} of {} example programs did not resolve:\n{}",
    failures.len(),
    roots.len(),
    failures.join("\n\n")
  );
}

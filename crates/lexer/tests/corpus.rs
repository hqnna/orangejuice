use std::path::{Path, PathBuf};

use oj_diag::{Severity, SourceMap};
use oj_lexer::{Interner, TokenKind};
use walkdir::WalkDir;

/// Set to a directory to write one cross-check dump per corpus file, for
/// comparing the stream against the reference `Jai_Lexer`.
const STREAM_DIR_ENV: &str = "OJ_TOKEN_STREAM_DIR";

fn jai_files(root: &Path) -> Vec<PathBuf> {
  WalkDir::new(root)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_file())
    .map(|entry| entry.into_path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "jai"))
    // A `.build` directory holds what a compiler left beside a program,
    // not part of the distribution: whoever has built a vendor program has one.
    .filter(|path| {
      !path
        .components()
        .any(|component| component.as_os_str() == ".build")
    })
    .collect()
}

#[test]
fn every_file_of_the_distribution_lexes_without_an_error() {
  let stream_dir = std::env::var_os(STREAM_DIR_ENV).map(PathBuf::from);
  if let Some(dir) = &stream_dir {
    std::fs::create_dir_all(dir).expect("the stream directory should be creatable");
  }

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

  for path in &files {
    let id = oj_source::load_file(&sources, path).expect("a corpus file should be readable");
    let file = sources.file(id);
    let lexed = oj_lexer::tokenize(file.bytes(), id, &interner);

    assert_eq!(
      lexed.tokens.last().map(|token| token.kind),
      Some(TokenKind::END_OF_INPUT),
      "{} did not lex to completion",
      path.display()
    );

    for diagnostic in &lexed.diagnostics {
      if diagnostic.severity == Severity::Error {
        failures.push(oj_diag::render(diagnostic, &file).trim_end().to_string());
      }
    }

    if let Some(dir) = &stream_dir {
      let relative = path
        .strip_prefix(oj_testsupport::distribution())
        .unwrap_or(path);
      let name = relative.to_string_lossy().replace('/', "%");
      std::fs::write(
        dir.join(format!("{name}.tokens")),
        oj_lexer::dump_token_stream(&lexed.tokens, &interner),
      )
      .expect("the stream dump should be writable");
    }
  }

  assert!(
    failures.is_empty(),
    "{} of {} corpus files produced lexer errors:\n{}",
    failures.len(),
    files.len(),
    failures.join("\n")
  );
}

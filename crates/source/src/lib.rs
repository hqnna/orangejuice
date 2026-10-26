mod modules;

use std::io;
use std::path::Path;

use oj_diag::{SourceId, SourceMap};

pub use modules::{
  ImportPath, ModuleError, ModuleKind, ResolvedModule, is_valid_module_name,
  resolve_directory_module, resolve_file_module, resolve_load, resolve_module,
};

/// Reads `path` into `map` and returns its id. The bytes are copied in and the
/// map owns them: a `#run` or a metaprogram may rewrite a source file while
/// the compilation that read it is still going — a bindings generator does
/// exactly that — and a mapping of a file that changes underneath it is
/// undefined behaviour, where a copy is just the text as it was.
pub fn load_file(map: &SourceMap, path: &Path) -> io::Result<SourceId> {
  let bytes = std::fs::read(path)?;
  Ok(map.add_bytes(path, bytes))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_file_rewritten_after_it_was_read_keeps_the_text_that_was_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("generated.jai");
    std::fs::write(&path, b"A :: 1;\n").unwrap();

    let map = SourceMap::new();
    let id = load_file(&map, &path).unwrap();
    std::fs::write(&path, b"").unwrap();
    std::fs::write(&path, b"B :: 2; C :: 3;\n").unwrap();

    assert_eq!(map.file(id).bytes(), b"A :: 1;\n");
  }

  #[test]
  fn a_loaded_file_reads_back_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.jai");
    std::fs::write(&path, b"main :: () {}\n").unwrap();

    let map = SourceMap::new();
    let id = load_file(&map, &path).unwrap();

    assert_eq!(map.file(id).bytes(), b"main :: () {}\n");
    assert_eq!(map.file(id).path(), path);
  }

  #[test]
  fn an_empty_file_loads_as_an_empty_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.jai");
    std::fs::write(&path, b"").unwrap();

    let map = SourceMap::new();
    let id = load_file(&map, &path).unwrap();

    assert_eq!(map.file(id).bytes(), b"");
  }

  #[test]
  fn a_missing_file_reports_the_io_error() {
    let map = SourceMap::new();
    let error = load_file(&map, Path::new("no/such/file.jai")).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
  }
}

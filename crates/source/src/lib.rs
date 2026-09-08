mod modules;

use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;
use oj_diag::{SourceId, SourceMap};

pub use modules::{
  ImportPath, ModuleError, ModuleKind, ResolvedModule, is_valid_module_name,
  resolve_directory_module, resolve_file_module, resolve_load, resolve_module,
};

/// Reads `path` into `map` and returns its id. Files are memory-mapped, which
/// is what makes lexing a whole module tree cheap; an empty file cannot be
/// mapped, so it becomes an empty buffer instead.
pub fn load_file(map: &SourceMap, path: &Path) -> io::Result<SourceId> {
  let file = File::open(path)?;
  if file.metadata()?.len() == 0 {
    return Ok(map.add_bytes(path, Vec::new()));
  }

  // SAFETY: the mapping is only read, and the compiler owns the source tree for
  // the duration of a run. Truncating a source file from another process while
  // it is mapped would fault, as it would for any mmap-based reader.
  let mapped = unsafe { Mmap::map(&file) }?;
  Ok(map.add(path, Arc::new(mapped)))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_mapped_file_reads_back_byte_for_byte() {
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

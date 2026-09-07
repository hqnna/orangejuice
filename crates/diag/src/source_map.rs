use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use crate::span::{Location, SourceId, Span};

/// The bytes of a source, however they were obtained: a memory map from
/// `oj-source`, or a buffer for generated code such as `add_build_string`.
pub type SourceBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

pub struct SourceFile {
  id: SourceId,
  path: PathBuf,
  text: SourceBytes,
  line_starts: OnceLock<Vec<u32>>,
}

impl SourceFile {
  pub fn id(&self) -> SourceId {
    self.id
  }

  pub fn path(&self) -> &Path {
    &self.path
  }

  pub fn bytes(&self) -> &[u8] {
    (*self.text).as_ref()
  }

  /// The byte offset of the first character of every line, computed on first
  /// use so that files nobody reports about never pay for it.
  pub fn line_starts(&self) -> &[u32] {
    self.line_starts.get_or_init(|| {
      let bytes = self.bytes();
      let mut starts = Vec::with_capacity(1 + bytes.len() / 32);
      starts.push(0);
      starts.extend(
        bytes
          .iter()
          .enumerate()
          .filter(|(_, byte)| **byte == b'\n')
          .map(|(index, _)| index as u32 + 1),
      );
      starts
    })
  }

  pub fn location(&self, offset: u32) -> Location {
    let starts = self.line_starts();
    let line = starts.partition_point(|start| *start <= offset).max(1);
    Location::new(line as u32, offset - starts[line - 1] + 1)
  }

  /// The bytes of `line` (1-based) without its line terminator.
  pub fn line_text(&self, line: u32) -> &[u8] {
    let starts = self.line_starts();
    let Some(start) = starts.get(line as usize - 1).copied() else {
      return &[];
    };
    let end = starts
      .get(line as usize)
      .copied()
      .unwrap_or(self.bytes().len() as u32);
    let text = &self.bytes()[start as usize..end as usize];
    let text = text.strip_suffix(b"\n").unwrap_or(text);
    text.strip_suffix(b"\r").unwrap_or(text)
  }

  pub fn text_of(&self, span: Span) -> &[u8] {
    &self.bytes()[span.range()]
  }
}

/// The compiler's set of sources. Files are added once and never removed, so a
/// [`SourceId`] stays valid for the whole run.
#[derive(Default)]
pub struct SourceMap {
  files: RwLock<Vec<Arc<SourceFile>>>,
}

impl SourceMap {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn add(&self, path: impl Into<PathBuf>, text: SourceBytes) -> SourceId {
    let mut files = self.files.write().expect("the source map is not poisoned");
    let id = SourceId(files.len() as u32);
    files.push(Arc::new(SourceFile {
      id,
      path: path.into(),
      text,
      line_starts: OnceLock::new(),
    }));
    id
  }

  pub fn add_bytes(&self, path: impl Into<PathBuf>, bytes: Vec<u8>) -> SourceId {
    self.add(path, Arc::new(bytes))
  }

  pub fn add_string(&self, path: impl Into<PathBuf>, text: &str) -> SourceId {
    self.add_bytes(path, text.as_bytes().to_vec())
  }

  pub fn file(&self, id: SourceId) -> Arc<SourceFile> {
    let files = self.files.read().expect("the source map is not poisoned");
    files
      .get(id.0 as usize)
      .cloned()
      .expect("the source id was issued by this source map")
  }

  pub fn len(&self) -> usize {
    self
      .files
      .read()
      .expect("the source map is not poisoned")
      .len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn file(text: &str) -> Arc<SourceFile> {
    let map = SourceMap::new();
    let id = map.add_string("test.jai", text);
    map.file(id)
  }

  #[test]
  fn offsets_map_to_one_based_lines_and_columns() {
    let file = file("abc\ndef\n\nghi");

    assert_eq!(file.location(0), Location::new(1, 1));
    assert_eq!(file.location(2), Location::new(1, 3));
    assert_eq!(file.location(3), Location::new(1, 4));
    assert_eq!(file.location(4), Location::new(2, 1));
    assert_eq!(file.location(8), Location::new(3, 1));
    assert_eq!(file.location(9), Location::new(4, 1));
    assert_eq!(file.location(12), Location::new(4, 4));
  }

  #[test]
  fn columns_count_bytes_the_way_the_reference_lexer_does() {
    let file = file("x := \"é\";");

    assert_eq!(file.location(8), Location::new(1, 9));
  }

  #[test]
  fn line_text_drops_either_line_terminator() {
    let file = file("one\r\ntwo\nthree");

    assert_eq!(file.line_text(1), b"one");
    assert_eq!(file.line_text(2), b"two");
    assert_eq!(file.line_text(3), b"three");
    assert_eq!(file.line_text(4), b"");
  }

  #[test]
  fn an_empty_file_still_has_a_first_line() {
    let file = file("");

    assert_eq!(file.location(0), Location::new(1, 1));
    assert_eq!(file.line_text(1), b"");
  }

  #[test]
  fn ids_are_handed_out_in_insertion_order() {
    let map = SourceMap::new();

    assert_eq!(map.add_string("a.jai", "a"), SourceId(0));
    assert_eq!(map.add_string("b.jai", "b"), SourceId(1));
    assert_eq!(map.file(SourceId(1)).path(), Path::new("b.jai"));
    assert_eq!(map.len(), 2);
  }
}

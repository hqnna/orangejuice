use std::fmt;
use std::ops::Range;

/// Identifies one source in a [`crate::SourceMap`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(pub u32);

/// A half-open byte range inside one source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
  pub start: u32,
  pub end: u32,
}

impl Span {
  pub fn new(start: u32, end: u32) -> Self {
    debug_assert!(start <= end);
    Self { start, end }
  }

  pub fn at(offset: u32) -> Self {
    Self::new(offset, offset)
  }

  pub fn from_offsets(start: usize, end: usize) -> Self {
    Self::new(start as u32, end as u32)
  }

  pub fn point(offset: usize) -> Self {
    Self::at(offset as u32)
  }

  pub fn len(self) -> u32 {
    self.end - self.start
  }

  pub fn is_empty(self) -> bool {
    self.start == self.end
  }

  pub fn to(self, other: Self) -> Self {
    Self::new(self.start.min(other.start), self.end.max(other.end))
  }

  pub fn range(self) -> Range<usize> {
    self.start as usize..self.end as usize
  }
}

/// A 1-based line and byte column, the pair jai reports in diagnostics and
/// stores in `Source_Code_Location`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location {
  pub line: u32,
  pub column: u32,
}

impl Location {
  pub fn new(line: u32, column: u32) -> Self {
    Self { line, column }
  }
}

impl fmt::Display for Location {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{},{}", self.line, self.column)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn spans_merge_to_their_outer_bounds() {
    assert_eq!(Span::new(3, 5).to(Span::new(9, 11)), Span::new(3, 11));
    assert_eq!(Span::new(9, 11).to(Span::new(3, 5)), Span::new(3, 11));
  }

  #[test]
  fn a_location_prints_the_way_jai_reports_it() {
    assert_eq!(Location::new(12, 4).to_string(), "12,4");
  }
}

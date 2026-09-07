use std::fmt;

use crate::span::{SourceId, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
  Error,
  Warning,
  Info,
}

impl Severity {
  /// The word the reference compiler prints before the message.
  pub fn label(self) -> &'static str {
    match self {
      Self::Error => "Error",
      Self::Warning => "Warning",
      Self::Info => "Info",
    }
  }
}

impl fmt::Display for Severity {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.label())
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
  pub severity: Severity,
  pub source: SourceId,
  pub span: Span,
  pub message: String,
}

impl Diagnostic {
  pub fn new(severity: Severity, source: SourceId, span: Span, message: impl Into<String>) -> Self {
    Self {
      severity,
      source,
      span,
      message: message.into(),
    }
  }

  pub fn error(source: SourceId, span: Span, message: impl Into<String>) -> Self {
    Self::new(Severity::Error, source, span, message)
  }

  pub fn warning(source: SourceId, span: Span, message: impl Into<String>) -> Self {
    Self::new(Severity::Warning, source, span, message)
  }

  pub fn info(source: SourceId, span: Span, message: impl Into<String>) -> Self {
    Self::new(Severity::Info, source, span, message)
  }

  pub fn is_error(&self) -> bool {
    self.severity == Severity::Error
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn severities_print_the_reference_labels() {
    assert_eq!(Severity::Error.to_string(), "Error");
    assert_eq!(Severity::Warning.to_string(), "Warning");
    assert_eq!(Severity::Info.to_string(), "Info");
  }

  #[test]
  fn only_errors_count_as_failures() {
    let span = Span::at(0);
    assert!(Diagnostic::error(SourceId(0), span, "boom").is_error());
    assert!(!Diagnostic::warning(SourceId(0), span, "hmm").is_error());
  }
}

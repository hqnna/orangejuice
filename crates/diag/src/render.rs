use std::fmt::Write as _;

use crate::diagnostic::Diagnostic;
use crate::source_map::SourceFile;

const EXCERPT_INDENT: &str = "    ";

/// Which layout a diagnostic's first line takes (**C§12**).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style {
  /// The reference's own: `path:line,column: Severity: message`.
  #[default]
  Plain,
  /// `Build_Options.use_visual_studio_message_format`, which an editor that
  /// parses MSVC output reads: `path(line,column): severity: message`.
  VisualStudio,
}

/// A diagnostic that is about the program rather than about a place in it:
/// the reference writes those as a bare `Error: …` line (**C§12**).
pub fn render_unplaced(diagnostic: &Diagnostic) -> String {
  format!("{}: {}\n", diagnostic.severity, diagnostic.message)
}

/// Renders one diagnostic in the reference compiler's plain-text layout:
/// `path:line,column: Severity: message`, a blank line, the offending source
/// line indented by four spaces, and a caret under the column.
///
/// Colors and multi-line highlighting are not implemented (`docs/spec.md` §2);
/// an "Info:" trailer is a diagnostic of its own rather than part of this one.
pub fn render(diagnostic: &Diagnostic, file: &SourceFile) -> String {
  render_with(diagnostic, file, Style::Plain)
}

/// The same, in whichever layout `Build_Options` asked for (**C§4**).
pub fn render_with(diagnostic: &Diagnostic, file: &SourceFile, style: Style) -> String {
  let location = file.location(diagnostic.span.start);
  let line = String::from_utf8_lossy(file.line_text(location.line));

  let mut out = String::new();
  let _ = match style {
    Style::Plain => writeln!(
      out,
      "{}:{}: {}: {}",
      file.path().display(),
      location,
      diagnostic.severity,
      diagnostic.message
    ),
    Style::VisualStudio => writeln!(
      out,
      "{}({},{}): {}: {}",
      file.path().display(),
      location.line,
      location.column,
      diagnostic.severity.to_string().to_lowercase(),
      diagnostic.message
    ),
  };
  let _ = writeln!(out);
  let _ = writeln!(out, "{EXCERPT_INDENT}{line}");

  let padding = caret_padding(line.as_bytes(), location.column);
  let _ = writeln!(out, "{EXCERPT_INDENT}{padding}^");
  out
}

/// Tabs in the source line are copied verbatim into the excerpt, so the caret
/// is padded with the line's own leading bytes rather than with plain spaces.
fn caret_padding(line: &[u8], column: u32) -> String {
  (0..column.saturating_sub(1) as usize)
    .map(|index| {
      if line.get(index) == Some(&b'\t') {
        '\t'
      } else {
        ' '
      }
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::span::Span;
  use crate::{Severity, SourceMap};

  #[test]
  fn a_diagnostic_renders_like_the_reference_compiler() {
    let map = SourceMap::new();
    let id = map.add_string("tp.jai", "x :: 1.2.3;\n");
    let file = map.file(id);
    let diagnostic = Diagnostic::new(
      Severity::Error,
      id,
      Span::at(9),
      "Can't have two decimals in a number!",
    );

    assert_eq!(
      render(&diagnostic, &file),
      concat!(
        "tp.jai:1,10: Error: Can't have two decimals in a number!\n",
        "\n",
        "    x :: 1.2.3;\n",
        "             ^\n",
      )
    );
  }

  #[test]
  fn the_visual_studio_format_is_the_one_an_editor_parses() {
    // `use_visual_studio_message_format` (**C§4**, **C§12**), which `-msvc_format`
    // sets: the location is parenthesized and the severity is lower case.
    let map = SourceMap::new();
    let id = map.add_string("tp.jai", "x :: 1.2.3;\n");
    let file = map.file(id);
    let diagnostic = Diagnostic::new(
      Severity::Error,
      id,
      Span::at(9),
      "Can't have two decimals in a number!",
    );

    assert_eq!(
      render_with(&diagnostic, &file, Style::VisualStudio),
      concat!(
        "tp.jai(1,10): error: Can't have two decimals in a number!\n",
        "\n",
        "    x :: 1.2.3;\n",
        "             ^\n",
      )
    );
  }

  #[test]
  fn a_tab_in_the_excerpt_keeps_the_caret_aligned() {
    let map = SourceMap::new();
    let id = map.add_string("tp.jai", "\tx := 1;\n");
    let file = map.file(id);
    let diagnostic = Diagnostic::error(id, Span::at(1), "here");

    assert_eq!(
      render(&diagnostic, &file),
      "tp.jai:1,2: Error: here\n\n    \tx := 1;\n    \t^\n"
    );
  }
}

use std::fmt::Write as _;

use crate::diagnostic::Diagnostic;
use crate::source_map::SourceFile;

const EXCERPT_INDENT: &str = "    ";

/// Renders one diagnostic in the reference compiler's plain-text layout:
/// `path:line,column: Severity: message`, a blank line, the offending source
/// line indented by four spaces, and a caret under the column.
///
/// Colors, multi-line highlighting, `use_visual_studio_message_format` and the
/// "Info:" trailers of **C§12** are not implemented yet.
/// A diagnostic that is about the program rather than about a place in it:
/// the reference writes those as a bare `Error: …` line (**C§12**).
pub fn render_unplaced(diagnostic: &Diagnostic) -> String {
  format!("{}: {}\n", diagnostic.severity, diagnostic.message)
}

pub fn render(diagnostic: &Diagnostic, file: &SourceFile) -> String {
  let location = file.location(diagnostic.span.start);
  let line = String::from_utf8_lossy(file.line_text(location.line));

  let mut out = String::new();
  let _ = writeln!(
    out,
    "{}:{}: {}: {}",
    file.path().display(),
    location,
    diagnostic.severity,
    diagnostic.message
  );
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

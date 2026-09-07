use std::fmt::Write as _;

use oj_diag::SourceFile;

use crate::interner::Interner;
use crate::token::{Token, TokenValue};

const LOCATION_WIDTH: usize = 15;
const KIND_WIDTH: usize = 22;

/// Renders a token stream the way `oj dump tokens` prints it: one token per
/// line, `line,column-line,column`, the kind, and the value and flags it
/// carries.
pub fn dump_tokens(tokens: &[Token], file: &SourceFile, interner: &Interner) -> String {
  let mut out = String::new();
  for token in tokens {
    let start = file.location(token.span.start);
    let end = file.location(token.span.end);
    let location = format!("{start}-{end}");
    let detail = detail(token, interner);

    let line = format!(
      "{location:LOCATION_WIDTH$} {kind:KIND_WIDTH$} {detail}",
      kind = token.kind
    );
    let _ = writeln!(out, "{}", line.trim_end());
  }
  out
}

fn detail(token: &Token, interner: &Interner) -> String {
  let mut detail = match &token.value {
    TokenValue::None => String::new(),
    TokenValue::Name(symbol) => interner.resolve_lossy(*symbol).into_owned(),
    TokenValue::Integer(value) => value.to_string(),
    TokenValue::Float(value) => format!("{value:?}"),
    TokenValue::Text(text) => format!("\"{}\"", text.escape_ascii()),
  };

  if token.backticked {
    detail.push_str(" (backticked)");
  }
  if !token.flags.is_empty() {
    let _ = write!(detail, " [{}]", token.flags.names().join("|"));
  }
  detail
}

/// The cross-check form: every field of a token, encoded so that it can be
/// compared byte for byte against the same dump taken from the reference
/// `Jai_Lexer` over the vendor corpus (`docs/spec.md` §9, M1).
pub fn dump_token_stream(tokens: &[Token], interner: &Interner) -> String {
  let mut out = String::new();
  for token in tokens {
    let _ = write!(out, "{} {}", token.kind.0, token.flags.bits());
    if token.backticked {
      out.push_str(" backtick");
    }
    match &token.value {
      TokenValue::None => {}
      TokenValue::Name(symbol) => {
        let _ = write!(out, " name={}", hex(interner.resolve(*symbol)));
      }
      TokenValue::Integer(value) => {
        let _ = write!(out, " int={value}");
      }
      TokenValue::Float(value) => {
        let _ = write!(out, " f64={:016x}", value.to_bits());
      }
      TokenValue::Text(text) => {
        let _ = write!(out, " str={}", hex(text));
      }
    }
    out.push('\n');
  }
  out
}

fn hex(bytes: &[u8]) -> String {
  bytes.iter().fold(String::new(), |mut out, byte| {
    let _ = write!(out, "{byte:02x}");
    out
  })
}

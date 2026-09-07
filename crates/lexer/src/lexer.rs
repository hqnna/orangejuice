use oj_diag::{Diagnostic, SourceId, Span};

use crate::interner::Interner;
use crate::token::{Token, TokenKind, TokenValue, ValueFlags};

/// **L§2.3**: identifiers are at most 512 bytes. The reference lexer collects
/// the digits of a number in the same scratch buffer, so numbers share the cap.
const MAX_IDENTIFIER_LENGTH: usize = 512;
const MAX_NUMBER_DIGITS: usize = MAX_IDENTIFIER_LENGTH - 1;
const MAX_HEX_FLOAT_DIGITS: usize = 16;

/// The keywords the reference lexer can produce with a backtick (**L§2.8**).
const BACKTICKABLE: [TokenKind; 5] = [
  TokenKind::IDENT,
  TokenKind::KEYWORD_DEFER,
  TokenKind::KEYWORD_RETURN,
  TokenKind::KEYWORD_OPERATOR,
  TokenKind::KEYWORD_PUSH_CONTEXT,
];

pub struct Lexer<'a> {
  input: &'a [u8],
  cursor: usize,
  source: SourceId,
  interner: &'a Interner,
  diagnostics: Vec<Diagnostic>,
}

pub struct Tokenized {
  pub tokens: Vec<Token>,
  pub diagnostics: Vec<Diagnostic>,
}

impl Tokenized {
  pub fn has_errors(&self) -> bool {
    self.diagnostics.iter().any(Diagnostic::is_error)
  }
}

/// Lexes a whole source, always ending with an `END_OF_INPUT` token.
pub fn tokenize(input: &[u8], source: SourceId, interner: &Interner) -> Tokenized {
  let mut lexer = Lexer::new(input, source, interner);
  let mut tokens = Vec::new();
  loop {
    let token = lexer.next_token();
    let done = token.kind == TokenKind::END_OF_INPUT;
    tokens.push(token);
    if done {
      break;
    }
  }
  Tokenized {
    tokens,
    diagnostics: lexer.into_diagnostics(),
  }
}

impl<'a> Lexer<'a> {
  pub fn new(input: &'a [u8], source: SourceId, interner: &'a Interner) -> Self {
    Self {
      input,
      cursor: 0,
      source,
      interner,
      diagnostics: Vec::new(),
    }
  }

  pub fn diagnostics(&self) -> &[Diagnostic] {
    &self.diagnostics
  }

  pub fn into_diagnostics(self) -> Vec<Diagnostic> {
    self.diagnostics
  }

  pub fn next_token(&mut self) -> Token {
    loop {
      self.skip_whitespace();

      let start = self.cursor;
      let Some(byte) = self.peek() else {
        return self.token(TokenKind::END_OF_INPUT, start);
      };

      if starts_identifier(byte) {
        return self.identifier_or_keyword(start);
      }
      if byte.is_ascii_digit() {
        return self.number(start);
      }

      match byte {
        b'`' => return self.backticked(start),
        b'.' => return self.dot(start),
        b'"' => return self.string_literal(start),
        b'@' => return self.note(start),
        b'#' => match self.hash(start) {
          Some(token) => return token,
          None => continue,
        },
        b'/' => match self.slash(start) {
          Some(token) => return token,
          None => continue,
        },
        0xe2 => {
          self.skip_invisible_unicode(start);
          continue;
        }
        0xc2 if self.peek_at(1) == Some(0xa0) => {
          self.cursor += 2;
          self.warning(
            self.span_from(start),
            "A Unicode non-breaking space character is here, outside of a string. Skipping.",
          );
          continue;
        }
        b'&' => {
          self.bump();
          return if self.eat(b'&') {
            self.or_equals(TokenKind::LOGICAL_AND, TokenKind::LOGICAL_AND_EQUALS, start)
          } else {
            self.or_equals(TokenKind::BITWISE_AND, TokenKind::BITWISE_AND_EQUALS, start)
          };
        }
        b'|' => {
          self.bump();
          return if self.eat(b'|') {
            self.or_equals(TokenKind::LOGICAL_OR, TokenKind::LOGICAL_OR_EQUALS, start)
          } else {
            self.or_equals(TokenKind::BITWISE_OR, TokenKind::BITWISE_OR_EQUALS, start)
          };
        }
        b'+' => {
          self.bump();
          return self.or_equals(TokenKind::PLUS, TokenKind::PLUSEQUALS, start);
        }
        b'-' => {
          self.bump();
          if self.eat(b'>') {
            return self.token(TokenKind::RIGHT_ARROW, start);
          }
          if self.eat(b'-') {
            let kind = if self.eat(b'-') {
              TokenKind::TRIPLE_MINUS
            } else {
              TokenKind::DOUBLE_MINUS
            };
            return self.token(kind, start);
          }
          return self.or_equals(TokenKind::MINUS, TokenKind::MINUSEQUALS, start);
        }
        b'*' => {
          self.bump();
          return self.or_equals(TokenKind::ASTERISK, TokenKind::TIMESEQUALS, start);
        }
        b'%' => {
          self.bump();
          return self.or_equals(TokenKind::PERCENT, TokenKind::MODEQUALS, start);
        }
        b'!' => {
          self.bump();
          return self.or_equals(TokenKind::BANG, TokenKind::ISNOTEQUAL, start);
        }
        b'^' => {
          self.bump();
          return self.or_equals(TokenKind::BITWISE_XOR, TokenKind::BITWISE_XOR_EQUALS, start);
        }
        b'~' => {
          self.bump();
          return self.token(TokenKind::BITWISE_NOT, start);
        }
        b',' => {
          self.bump();
          let kind = if self.eat(b',') {
            TokenKind::DOUBLE_COMMA
          } else {
            TokenKind::COMMA
          };
          return self.token(kind, start);
        }
        b'$' => {
          self.bump();
          let kind = if self.eat(b'$') {
            TokenKind::DOUBLE_DOLLAR
          } else {
            TokenKind::DOLLAR
          };
          return self.token(kind, start);
        }
        b'=' => {
          self.bump();
          if self.eat(b'=') {
            let kind = if self.eat(b'=') {
              TokenKind::TRIPLE_EQUALS
            } else {
              TokenKind::ISEQUAL
            };
            return self.token(kind, start);
          }
          if self.eat(b'>') {
            return self.token(TokenKind::QUICK_LAMBDA, start);
          }
          return self.token(TokenKind::EQUALS, start);
        }
        b'<' => {
          self.bump();
          if self.eat(b'<') {
            return if self.eat(b'<') {
              self.or_equals(TokenKind::ROTATE_LEFT, TokenKind::ROTATE_LEFT_EQUALS, start)
            } else {
              self.or_equals(
                TokenKind::POINTER_DEREFERENCE_OR_SHIFT_LEFT,
                TokenKind::SHIFT_LEFT_EQUALS,
                start,
              )
            };
          }
          return self.or_equals(TokenKind::LESS_THAN, TokenKind::LESSEQUALS, start);
        }
        b'>' => {
          self.bump();
          if self.eat(b'>') {
            return if self.eat(b'>') {
              self.or_equals(
                TokenKind::ROTATE_RIGHT,
                TokenKind::ROTATE_RIGHT_EQUALS,
                start,
              )
            } else {
              self.or_equals(TokenKind::SHIFT_RIGHT, TokenKind::SHIFT_RIGHT_EQUALS, start)
            };
          }
          return self.or_equals(TokenKind::GREATER_THAN, TokenKind::GREATEREQUALS, start);
        }
        _ => {
          self.bump();
          return self.token(TokenKind(i16::from(byte)), start);
        }
      }
    }
  }
}

impl Lexer<'_> {
  fn peek(&self) -> Option<u8> {
    self.input.get(self.cursor).copied()
  }

  fn peek_at(&self, ahead: usize) -> Option<u8> {
    self.input.get(self.cursor + ahead).copied()
  }

  fn bump(&mut self) {
    self.cursor += 1;
  }

  fn eat(&mut self, byte: u8) -> bool {
    let matched = self.peek() == Some(byte);
    if matched {
      self.bump();
    }
    matched
  }

  fn skip_whitespace(&mut self) {
    while self.peek().is_some_and(is_space) {
      self.bump();
    }
  }

  fn here(&self) -> Span {
    Span::point(self.cursor)
  }

  fn span_from(&self, start: usize) -> Span {
    Span::from_offsets(start, self.cursor)
  }

  fn token(&self, kind: TokenKind, start: usize) -> Token {
    Token::new(kind, self.span_from(start))
  }

  fn or_equals(&mut self, plain: TokenKind, augmented: TokenKind, start: usize) -> Token {
    let kind = if self.eat(b'=') { augmented } else { plain };
    self.token(kind, start)
  }

  fn error(&mut self, span: Span, message: impl Into<String>) {
    self
      .diagnostics
      .push(Diagnostic::error(self.source, span, message));
  }

  fn warning(&mut self, span: Span, message: impl Into<String>) {
    self
      .diagnostics
      .push(Diagnostic::warning(self.source, span, message));
  }

  fn detail(&mut self, span: Span, message: impl Into<String>) {
    self
      .diagnostics
      .push(Diagnostic::info(self.source, span, message));
  }

  fn identifier_or_keyword(&mut self, start: usize) -> Token {
    let (name, end, too_long) = scan_identifier(self.input, self.cursor);
    self.cursor = end;
    if too_long {
      self.error(
        self.span_from(start),
        format!(
          "Identifier is too long. The maximum identifier length is {MAX_IDENTIFIER_LENGTH} bytes."
        ),
      );
    }

    let kind = keyword(&name).unwrap_or(TokenKind::IDENT);
    let mut token = self.token(kind, start);
    token.value = TokenValue::Name(self.interner.intern(&name));
    token
  }

  fn backticked(&mut self, start: usize) -> Token {
    self.bump();

    if !self.peek().is_some_and(starts_identifier) {
      self.error(self.span_from(start), "Expected an identifier after '`'.");
      let mut token = self.token(TokenKind::IDENT, start);
      token.value = TokenValue::Name(self.interner.intern(b""));
      token.backticked = true;
      return token;
    }

    let mut token = self.identifier_or_keyword(start);
    if BACKTICKABLE.contains(&token.kind) {
      token.backticked = true;
      return token;
    }

    let name = token
      .name()
      .map(|symbol| self.interner.resolve_lossy(symbol))
      .unwrap_or_default();
    self.error(
      token.span,
      format!(
        "Expected an identifier after '`', but '{name}' is a keyword. ('defer', 'return', \
         'push_context' and overloaded operator functions are the only keywords that can be \
         backticked.)"
      ),
    );
    token
  }

  fn dot(&mut self, start: usize) -> Token {
    self.bump();
    match self.peek() {
      Some(b'.') => {
        self.bump();
        self.token(TokenKind::DOUBLE_DOT, start)
      }
      Some(b'*') => {
        self.bump();
        self.token(TokenKind::POSTFIX_DEREFERENCE, start)
      }
      Some(b'[') => {
        self.bump();
        self.token(TokenKind::BEGIN_ARRAY_LITERAL, start)
      }
      Some(b'{') => {
        self.bump();
        self.token(TokenKind::BEGIN_STRUCT_LITERAL, start)
      }
      Some(byte) if byte.is_ascii_digit() => {
        self.cursor = start;
        self.number(start)
      }
      _ => self.token(TokenKind::DOT, start),
    }
  }

  /// `#` introduces a directive, but the lexer itself has to recognise the
  /// hashbang line and `#string`, which is not a directive but a literal.
  /// Returns `None` when the input consumed was skipped rather than tokenized.
  fn hash(&mut self, start: usize) -> Option<Token> {
    self.bump();

    let at_line_start = start == 0 || self.input[start - 1] == b'\n';
    if at_line_start && self.peek() == Some(b'!') {
      self.skip_to_end_of_line();
      return None;
    }

    if let Some(end) = self.here_string_directive_end() {
      self.cursor = end;
      return Some(self.here_string(start));
    }

    Some(self.token(TokenKind::HASH, start))
  }

  fn here_string_directive_end(&self) -> Option<usize> {
    let mut cursor = self.cursor;
    while self.input.get(cursor).copied().is_some_and(is_space) {
      cursor += 1;
    }
    if !self
      .input
      .get(cursor)
      .copied()
      .is_some_and(starts_identifier)
    {
      return None;
    }

    let (name, end, _) = scan_identifier(self.input, cursor);
    (name == b"string").then_some(end)
  }

  fn slash(&mut self, start: usize) -> Option<Token> {
    self.bump();
    match self.peek() {
      Some(b'/') => {
        self.skip_to_end_of_line();
        None
      }
      Some(b'*') => {
        self.bump();
        self.skip_block_comment(start);
        None
      }
      Some(b'=') => {
        self.bump();
        Some(self.token(TokenKind::DIVEQUALS, start))
      }
      _ => Some(self.token(TokenKind::SLASH, start)),
    }
  }

  fn skip_to_end_of_line(&mut self) {
    while let Some(byte) = self.peek() {
      if byte == b'\n' {
        break;
      }
      self.bump();
    }
  }

  /// **L§2.2**: block comments nest, and the depth is counted.
  fn skip_block_comment(&mut self, start: usize) {
    let mut depth = 1usize;
    while depth > 0 {
      let Some(byte) = self.peek() else {
        self.error(self.span_from(start), "End of input from within a comment.");
        return;
      };
      self.bump();
      match byte {
        b'/' if self.peek() == Some(b'*') => {
          self.bump();
          depth += 1;
        }
        b'*' if self.peek() == Some(b'/') => {
          self.bump();
          depth -= 1;
        }
        _ => {}
      }
    }
  }

  /// **L§2.1**: zero-width spaces and directional formatting characters are
  /// skipped with a warning; every other non-ASCII character outside a string
  /// or comment is an error.
  fn skip_invisible_unicode(&mut self, start: usize) {
    let Some((code, length)) = decode_utf8(&self.input[self.cursor..]) else {
      self.bump();
      self.error(
        self.span_from(start),
        "Attempted to parse a Unicode character here, but was unable to read a whole valid \
         character.",
      );
      return;
    };

    self.cursor += length;
    let span = self.span_from(start);
    match code {
      0x200b => self.warning(
        span,
        "Illegal Unicode zero-width space found in the program text. This could be a problem with \
         pasting the data from another source.",
      ),
      0x2066..=0x2069 | 0x202a..=0x202e => self.warning(
        span,
        "Illegal Unicode directional-formatting character found in the program text. This could \
         be a problem with pasting the data from another source.",
      ),
      _ => self.error(
        span,
        format!(
          "Unicode characters in this range are not supported by the parser at lexer time. \
           (Offending character code was U+{code:X}.)"
        ),
      ),
    }
  }

  fn note(&mut self, start: usize) -> Token {
    self.bump();

    if self.peek() == Some(b'"') {
      let literal = self.string_literal(self.cursor);
      let name = self.interner.intern(literal.text().unwrap_or_default());
      let mut token = self.token(TokenKind::NOTE, start);
      token.value = TokenValue::Name(name);
      return token;
    }

    let name_start = self.cursor;
    while self
      .peek()
      .is_some_and(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0 | b';'))
    {
      self.bump();
    }
    if self.cursor == name_start {
      self.error(self.span_from(start), "Empty note.");
    }

    let name = self.interner.intern(&self.input[name_start..self.cursor]);
    let mut token = self.token(TokenKind::NOTE, start);
    token.value = TokenValue::Name(name);
    token
  }

  fn string_literal(&mut self, start: usize) -> Token {
    self.bump();

    let mut text = Vec::new();
    loop {
      let Some(byte) = self.peek() else {
        self.error(self.here(), "End of file during string constant!");
        break;
      };
      self.bump();
      match byte {
        b'"' => break,
        b'\n' => {
          self.error(self.here(), "Newline in string constant!");
          break;
        }
        b'\\' => self.escape(&mut text),
        _ => text.push(byte),
      }
    }

    let mut token = self.token(TokenKind::STRING, start);
    token.value = TokenValue::Text(text.into_boxed_slice());
    token
  }

  /// The escapes of **L§2.6**. An unknown escape warns and yields the character
  /// itself.
  fn escape(&mut self, text: &mut Vec<u8>) {
    let Some(byte) = self.peek() else {
      return;
    };

    match byte {
      b'n' => self.take_escaped(text, b'\n'),
      b'r' => self.take_escaped(text, b'\r'),
      b't' => self.take_escaped(text, b'\t'),
      b'0' => self.take_escaped(text, 0),
      b'e' => self.take_escaped(text, 0x1b),
      b'"' => self.take_escaped(text, b'"'),
      b'\\' => self.take_escaped(text, b'\\'),
      b'%' => self.take_escaped(text, 31),
      b'x' => {
        self.bump();
        let high = self.hex_digit(HexDigits::Byte).unwrap_or(0);
        let low = self.hex_digit(HexDigits::Byte).unwrap_or(0);
        text.push(high * 16 + low);
      }
      b'd' => {
        self.bump();
        self.decimal_escape(text);
      }
      b'u' | b'U' => {
        self.bump();
        self.unicode_escape(text, byte == b'U');
      }
      _ => {
        self.warning(
          self.here(),
          format!(
            "Unknown escape sequence '\\{}' in string constant!",
            byte.escape_ascii()
          ),
        );
        self.bump();
        text.push(byte);
      }
    }
  }

  fn take_escaped(&mut self, text: &mut Vec<u8>, byte: u8) {
    self.bump();
    text.push(byte);
  }

  fn decimal_escape(&mut self, text: &mut Vec<u8>) {
    let Some(high) = self.decimal_digit() else {
      return;
    };
    let Some(middle) = self.decimal_digit() else {
      return;
    };
    let low = self.decimal_digit().unwrap_or(0);

    let value = u32::from(high) * 100 + u32::from(middle) * 10 + u32::from(low);
    if value > 255 {
      let span = Span::from_offsets(self.cursor.saturating_sub(3), self.cursor);
      self.error(
        span,
        format!("Decimal value of {value} exceeds the limit of 255."),
      );
    }
    text.push(value as u8);
  }

  fn unicode_escape(&mut self, text: &mut Vec<u8>, wide: bool) {
    let (count, kind) = if wide {
      (8, HexDigits::Unicode32)
    } else {
      (4, HexDigits::Unicode16)
    };

    let start = self.cursor;
    let mut value: u32 = 0;
    for _ in 0..count {
      let Some(digit) = self.hex_digit(kind) else {
        break;
      };
      value = (value << 4) + u32::from(digit);
    }

    match char::from_u32(value) {
      Some(character) => {
        let mut buffer = [0u8; 4];
        text.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
      }
      None => self.error(
        Span::from_offsets(start, self.cursor),
        format!("U+{value:X} is not a Unicode code point."),
      ),
    }
  }

  fn hex_digit(&mut self, kind: HexDigits) -> Option<u8> {
    let value = match self.peek() {
      Some(byte @ b'a'..=b'f') => 10 + byte - b'a',
      Some(byte @ b'A'..=b'F') => 10 + byte - b'A',
      Some(byte @ b'0'..=b'9') => byte - b'0',
      _ => {
        self.error(self.here(), "A hex digit is required here.");
        self.detail(self.here(), kind.requirement());
        return None;
      }
    };
    self.bump();
    Some(value)
  }

  fn decimal_digit(&mut self) -> Option<u8> {
    match self.peek() {
      Some(byte @ b'0'..=b'9') => {
        self.bump();
        Some(byte - b'0')
      }
      _ => {
        self.error(self.here(), "A decimal digit is required here.");
        self.detail(
          self.here(),
          "\\d must be followed by exactly three decimal digits.",
        );
        None
      }
    }
  }

  fn number(&mut self, start: usize) -> Token {
    let mut digits: Vec<u8> = Vec::new();
    let mut flags = ValueFlags::empty();
    let mut accumulator: u64 = 0;
    let mut only_unsigned = false;
    let mut after_decimal: Option<usize> = None;
    let mut exponent_at: Option<usize> = None;

    while digits.len() < MAX_NUMBER_DIGITS {
      let Some(byte) = self.peek() else {
        break;
      };

      if digits.len() == 1 && digits[0] == b'0' && flags.is_empty() {
        match byte {
          b'x' | b'X' => {
            flags |= ValueFlags::HEX;
            self.bump();
            digits.clear();
            continue;
          }
          b'b' | b'B' => {
            flags |= ValueFlags::BINARY;
            self.bump();
            digits.clear();
            continue;
          }
          b'h' | b'H' => {
            self.bump();
            return self.hex_float(start);
          }
          _ => {}
        }
      }

      if byte == b'_' {
        self.bump();
        continue;
      }

      if byte == b'.' {
        self.bump();
        if self.peek() == Some(b'.') {
          // `0..7` is a range, not a decimal point (**L§2.6**).
          self.cursor -= 1;
          break;
        }
        if after_decimal.is_some() {
          self.error(self.here(), "Can't have two decimals in a number!");
          break;
        }
        if flags.contains(ValueFlags::HEX) {
          self.error(
            self.here(),
            "Cannot use a decimal point in a hexadecimal number!",
          );
        }
        if flags.contains(ValueFlags::BINARY) {
          self.error(
            self.here(),
            "Cannot use a decimal point in a binary number!",
          );
        }
        digits.push(b'.');
        after_decimal = Some(digits.len());
        flags |= ValueFlags::FLOAT;
        continue;
      }

      if after_decimal.is_some() {
        if !byte.is_ascii_digit() {
          if byte == b'f' {
            self.error(
              self.span_from(start),
              "In this language, we don't suffix our float constants with f.",
            );
          }
          if byte | 32 != b'e' {
            break;
          }
          if exponent_at.is_some() {
            self.error(
              self.span_from(start),
              "Can't have two exponents in a number!",
            );
            break;
          }

          exponent_at = Some(digits.len());
          digits.push(b'e');
          self.bump();
          match self.peek() {
            Some(sign @ (b'+' | b'-')) => {
              digits.push(sign);
              self.bump();
              continue;
            }
            Some(next) if next.is_ascii_digit() => continue,
            _ => {
              self.error(
                self.span_from(start),
                "'e' in a float literal must be followed by + or - or a numerical digit.",
              );
              break;
            }
          }
        }
      } else {
        let mut digit_value = byte.is_ascii_digit().then(|| u32::from(byte - b'0'));
        let base: u64 = if flags.contains(ValueFlags::HEX) {
          match byte {
            b'a'..=b'f' => digit_value = Some(u32::from(10 + byte - b'a')),
            b'A'..=b'F' => digit_value = Some(u32::from(10 + byte - b'A')),
            _ => {}
          }
          16
        } else if flags.contains(ValueFlags::BINARY) {
          2
        } else {
          10
        };

        if base == 2 && digit_value.is_some_and(|value| value > 1) {
          self.error(self.here(), "Invalid digit in a binary number.");
          break;
        }
        let Some(digit_value) = digit_value else {
          break;
        };

        if accumulator > u64::MAX / base {
          flags |= ValueFlags::OVERFLOWED;
        }
        accumulator = accumulator.wrapping_mul(base);
        if base == 10 && accumulator > 0x7fff_ffff_ffff_ffff {
          only_unsigned = true;
        }
        let before_add = accumulator;
        accumulator = accumulator.wrapping_add(u64::from(digit_value));
        if accumulator < before_add {
          flags |= ValueFlags::OVERFLOWED;
        }
      }

      self.bump();
      digits.push(byte);
    }

    let span = self.span_from(start);
    let mut token = Token::new(TokenKind::NUMBER, span);

    match after_decimal {
      Some(after_decimal) => {
        let value = String::from_utf8_lossy(&digits)
          .parse::<f64>()
          .unwrap_or(0.0);
        flags |= float_width_flags(&digits, after_decimal, exponent_at, value);
        token.value = TokenValue::Float(value);
      }
      None => {
        if only_unsigned {
          // The reference has no flag for "u64 only", and reuses HEX to make
          // `Program_Print` render the literal unsigned.
          flags |= ValueFlags::HEX;
        }
        if flags.contains(ValueFlags::OVERFLOWED) {
          self.error(
            span,
            "Integer literal is too big. All integer literals must fit into 64 bits.",
          );
        }
        token.value = TokenValue::Integer(accumulator);
      }
    }

    token.flags = flags | ValueFlags::NUMBER;
    token
  }

  /// `0h` followed by 4, 8 or 16 hex digits is an IEEE bit pattern (**L§2.6**).
  fn hex_float(&mut self, start: usize) -> Token {
    let mut accumulator: u64 = 0;
    let mut count = 0usize;
    let mut too_long = false;

    while let Some(byte) = self.peek() {
      if byte == b'_' {
        self.bump();
        continue;
      }
      let digit = match byte {
        b'a'..=b'f' => u64::from(10 + byte - b'a'),
        b'A'..=b'F' => u64::from(10 + byte - b'A'),
        b'0'..=b'9' => u64::from(byte - b'0'),
        _ => break,
      };
      self.bump();
      if count == MAX_HEX_FLOAT_DIGITS {
        too_long = true;
      }
      accumulator = accumulator.wrapping_mul(16).wrapping_add(digit);
      count += 1;
    }

    if too_long {
      self.error(self.here(), "Hexfloat is too long! (16 character maximum.)");
    }
    if count == 0 {
      self.error(
        self.here(),
        "Expected some hexadecimal characters after \"0h\", but found none.",
      );
    } else if !matches!(count, 4 | 8 | 16) {
      self.error(
        self.here(),
        format!("Numbers designated by 0h must be 4, 8 or 16 digits; this one is {count} digits."),
      );
    }

    let mut flags = ValueFlags::NUMBER | ValueFlags::FLOAT | ValueFlags::HEX;
    let value = if count > 8 {
      flags |= ValueFlags::REQUIRES_FLOAT64;
      f64::from_bits(accumulator)
    } else if count == 8 {
      f64::from(f32::from_bits(accumulator as u32))
    } else {
      f64::from(f32::from_bits(u32::from(accumulator as u16)))
    };

    let mut token = Token::new(TokenKind::NUMBER, self.span_from(start));
    token.value = TokenValue::Float(value);
    token.flags = flags;
    token
  }

  /// A here-string (**L§2.6**) runs verbatim from the line after
  /// `#string IDENT` to a line whose first non-blank text is `IDENT`.
  fn here_string(&mut self, start: usize) -> Token {
    self.skip_whitespace();

    let mut want_cr = false;
    if self.eat(b',') {
      self.skip_whitespace();
      if !self.peek().is_some_and(starts_identifier) {
        self.error(self.span_from(start), "Expected an identifier after ','");
        return self.token(TokenKind::ERROR, start);
      }

      let (modifier, end, _) = scan_identifier(self.input, self.cursor);
      self.cursor = end;
      if modifier != b"cr" {
        self.error(self.span_from(start), "Invalid #string modifier after ','");
        return self.token(TokenKind::ERROR, start);
      }
      want_cr = true;
      self.skip_whitespace();
    }

    if !self.peek().is_some_and(starts_identifier) {
      self.error(self.span_from(start), "Expected identifier after #string.");
      return self.token(TokenKind::ERROR, start);
    }
    let (terminator, end, _) = scan_identifier(self.input, self.cursor);
    self.cursor = end;

    loop {
      let Some(byte) = self.peek() else {
        self.error(self.span_from(start), "Reached end-of-file in #string.");
        return self.token(TokenKind::ERROR, start);
      };
      if !is_space(byte) {
        self.error(
          self.span_from(start),
          "Unexpected non-whitespace characters after #string identifier.",
        );
        return self.token(TokenKind::ERROR, start);
      }
      self.bump();
      if byte == b'\n' {
        break;
      }
    }

    let body_start = self.cursor;
    let mut body_end = self.cursor;
    loop {
      if self.cursor >= self.input.len() {
        self.error(
          self.span_from(start),
          "Unexpected end of file inside #string.",
        );
        return self.token(TokenKind::ERROR, start);
      }

      // The terminator line may be indented.
      while let Some(byte) = self.peek() {
        if byte == b'\n' || !is_space(byte) {
          break;
        }
        self.bump();
      }

      if self.input[self.cursor..].starts_with(&terminator) {
        let after = self.cursor + terminator.len();
        if self
          .input
          .get(after)
          .is_none_or(|byte| !continues_identifier(*byte))
        {
          self.cursor = after;
          break;
        }
      }

      while self.cursor < self.input.len() {
        let byte = self.input[self.cursor];
        self.cursor += 1;
        if byte == b'\n' {
          body_end = self.cursor;
          break;
        }
      }
    }

    let raw = &self.input[body_start..body_end];
    let text = if want_cr {
      normalize_to_crlf(raw)
    } else if raw.contains(&b'\r') {
      normalize_to_lf(raw)
    } else {
      raw.to_vec()
    };

    let mut token = self.token(TokenKind::STRING, start);
    token.value = TokenValue::Text(text.into_boxed_slice());
    token.flags = ValueFlags::HERE_STRING;
    token
  }
}

#[derive(Clone, Copy)]
enum HexDigits {
  Byte,
  Unicode16,
  Unicode32,
}

impl HexDigits {
  fn requirement(self) -> &'static str {
    match self {
      Self::Byte => "\\x must be followed by exactly two hex digits.",
      Self::Unicode16 => "\\u must be followed by exactly four hex digits.",
      Self::Unicode32 => "\\U must be followed by exactly eight hex digits.",
    }
  }
}

fn is_space(byte: u8) -> bool {
  matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn starts_identifier(byte: u8) -> bool {
  byte.is_ascii_alphabetic() || byte == b'_'
}

fn continues_identifier(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Scans one identifier, resolving the `\`-then-spaces continuations that the
/// standard modules use for column alignment (**L§2.3**): `time\    _report`
/// is the identifier `time_report`.
fn scan_identifier(input: &[u8], mut cursor: usize) -> (Vec<u8>, usize, bool) {
  let mut name = Vec::new();
  let mut too_long = false;

  loop {
    match input.get(cursor).copied() {
      Some(byte) if continues_identifier(byte) => {
        if name.len() == MAX_IDENTIFIER_LENGTH {
          too_long = true;
        } else {
          name.push(byte);
        }
        cursor += 1;
      }
      Some(b'\\') => {
        cursor += 1;
        while input.get(cursor) == Some(&b' ') {
          cursor += 1;
        }
      }
      _ => break,
    }
  }

  (name, cursor, too_long)
}

fn keyword(name: &[u8]) -> Option<TokenKind> {
  Some(match name {
    b"if" => TokenKind::KEYWORD_IF,
    b"xx" => TokenKind::KEYWORD_AUTO_CAST,
    b"ifx" => TokenKind::KEYWORD_IFX,
    b"for" => TokenKind::KEYWORD_FOR,
    b"then" => TokenKind::KEYWORD_THEN,
    b"else" => TokenKind::KEYWORD_ELSE,
    b"null" => TokenKind::KEYWORD_NULL,
    b"case" => TokenKind::KEYWORD_CASE,
    b"enum" => TokenKind::KEYWORD_ENUM,
    b"true" => TokenKind::KEYWORD_TRUE,
    b"cast" => TokenKind::KEYWORD_CAST,
    b"while" => TokenKind::KEYWORD_WHILE,
    b"break" => TokenKind::KEYWORD_BREAK,
    b"using" => TokenKind::KEYWORD_USING,
    b"defer" => TokenKind::KEYWORD_DEFER,
    b"false" => TokenKind::KEYWORD_FALSE,
    b"union" => TokenKind::KEYWORD_UNION,
    b"return" => TokenKind::KEYWORD_RETURN,
    b"struct" => TokenKind::KEYWORD_STRUCT,
    b"remove" => TokenKind::KEYWORD_REMOVE,
    b"inline" => TokenKind::KEYWORD_INLINE,
    b"size_of" => TokenKind::KEYWORD_SIZE_OF,
    b"type_of" => TokenKind::KEYWORD_TYPE_OF,
    b"code_of" => TokenKind::KEYWORD_CODE_OF,
    b"context" => TokenKind::KEYWORD_CONTEXT,
    b"continue" => TokenKind::KEYWORD_CONTINUE,
    b"operator" => TokenKind::KEYWORD_OPERATOR,
    b"type_info" => TokenKind::KEYWORD_TYPE_INFO,
    b"no_inline" => TokenKind::KEYWORD_NO_INLINE,
    b"interface" => TokenKind::KEYWORD_INTERFACE,
    b"enum_flags" => TokenKind::KEYWORD_ENUM_FLAGS,
    b"is_constant" => TokenKind::KEYWORD_IS_CONSTANT,
    b"push_context" => TokenKind::KEYWORD_PUSH_CONTEXT,
    b"initializer_of" => TokenKind::KEYWORD_INITIALIZER_OF,
    _ => return None,
  })
}

/// **L§2.6**: a float literal needs `float64` when it carries more than eight
/// significant digits or an exponent outside the `float32` range, and defaults
/// to `float64` at exactly eight.
fn float_width_flags(
  digits: &[u8],
  after_decimal: usize,
  exponent_at: Option<usize>,
  value: f64,
) -> ValueFlags {
  let mut end = exponent_at.unwrap_or(digits.len());
  while end > after_decimal && digits[end - 1] == b'0' {
    end -= 1;
  }
  let mut begin = 0;
  while begin < end && matches!(digits[begin], b'0' | b'.') {
    begin += 1;
  }
  let significant = digits[begin..end]
    .iter()
    .filter(|byte| byte.is_ascii_digit())
    .count();

  if significant > 8 {
    ValueFlags::REQUIRES_FLOAT64_DUE_TO_SIGNIFICANT_DIGITS | ValueFlags::REQUIRES_FLOAT64
  } else if has_a_big_exponent(value) {
    ValueFlags::REQUIRES_FLOAT64
  } else if significant == 8 {
    ValueFlags::DEFAULTS_TO_FLOAT64
  } else {
    ValueFlags::empty()
  }
}

/// True when `value`'s exponent does not fit a `float32`.
fn has_a_big_exponent(value: f64) -> bool {
  let exponent = ((value.to_bits() >> 52) & 0x7ff) as i32;
  if exponent == 0 || exponent == 0x7ff {
    return false;
  }
  let exponent = exponent - 1023;
  !(-126..=127).contains(&exponent)
}

fn decode_utf8(input: &[u8]) -> Option<(u32, usize)> {
  let length = match input.first()? {
    byte if *byte < 0x80 => 1,
    byte if byte >> 5 == 0b110 => 2,
    byte if byte >> 4 == 0b1110 => 3,
    byte if byte >> 3 == 0b11110 => 4,
    _ => return None,
  };
  let character = std::str::from_utf8(input.get(..length)?)
    .ok()?
    .chars()
    .next()?;
  Some((character as u32, length))
}

fn normalize_to_lf(raw: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(raw.len());
  let mut index = 0;
  while index < raw.len() {
    if raw[index] == b'\r' && raw.get(index + 1) == Some(&b'\n') {
      index += 1;
      continue;
    }
    out.push(raw[index]);
    index += 1;
  }
  out
}

fn normalize_to_crlf(raw: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(raw.len());
  for (index, byte) in raw.iter().enumerate() {
    if *byte == b'\n' && (index == 0 || raw[index - 1] != b'\r') {
      out.push(b'\r');
    }
    out.push(*byte);
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use oj_diag::Severity;

  fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>, Interner) {
    let interner = Interner::new();
    let lexed = tokenize(source.as_bytes(), SourceId(0), &interner);
    (lexed.tokens, lexed.diagnostics, interner)
  }

  /// Every kind but the trailing `END_OF_INPUT`.
  fn kinds(source: &str) -> Vec<TokenKind> {
    let (tokens, _, _) = lex(source);
    tokens[..tokens.len() - 1]
      .iter()
      .map(|token| token.kind)
      .collect()
  }

  fn first(source: &str) -> Token {
    lex(source).0.into_iter().next().expect("a token")
  }

  fn names(source: &str) -> Vec<String> {
    let (tokens, _, interner) = lex(source);
    tokens
      .iter()
      .filter_map(|token| token.name())
      .map(|symbol| interner.resolve_lossy(symbol).into_owned())
      .collect()
  }

  fn messages(source: &str, severity: Severity) -> Vec<String> {
    lex(source)
      .1
      .into_iter()
      .filter(|diagnostic| diagnostic.severity == severity)
      .map(|diagnostic| diagnostic.message)
      .collect()
  }

  fn errors(source: &str) -> Vec<String> {
    messages(source, Severity::Error)
  }

  fn warnings(source: &str) -> Vec<String> {
    messages(source, Severity::Warning)
  }

  fn text(source: &str) -> Vec<u8> {
    first(source).text().expect("a string token").to_vec()
  }

  #[test]
  fn an_empty_source_is_just_end_of_input() {
    let (tokens, diagnostics, _) = lex("");

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::END_OF_INPUT);
    assert_eq!(tokens[0].span, Span::new(0, 0));
    assert!(diagnostics.is_empty());
  }

  #[test]
  fn every_keyword_is_recognised() {
    let source = "if ifx then else case for while break continue remove return struct union enum \
                  enum_flags interface using defer null true false cast xx inline no_inline \
                  operator context push_context size_of type_of code_of type_info initializer_of \
                  is_constant";

    assert_eq!(
      kinds(source),
      [
        TokenKind::KEYWORD_IF,
        TokenKind::KEYWORD_IFX,
        TokenKind::KEYWORD_THEN,
        TokenKind::KEYWORD_ELSE,
        TokenKind::KEYWORD_CASE,
        TokenKind::KEYWORD_FOR,
        TokenKind::KEYWORD_WHILE,
        TokenKind::KEYWORD_BREAK,
        TokenKind::KEYWORD_CONTINUE,
        TokenKind::KEYWORD_REMOVE,
        TokenKind::KEYWORD_RETURN,
        TokenKind::KEYWORD_STRUCT,
        TokenKind::KEYWORD_UNION,
        TokenKind::KEYWORD_ENUM,
        TokenKind::KEYWORD_ENUM_FLAGS,
        TokenKind::KEYWORD_INTERFACE,
        TokenKind::KEYWORD_USING,
        TokenKind::KEYWORD_DEFER,
        TokenKind::KEYWORD_NULL,
        TokenKind::KEYWORD_TRUE,
        TokenKind::KEYWORD_FALSE,
        TokenKind::KEYWORD_CAST,
        TokenKind::KEYWORD_AUTO_CAST,
        TokenKind::KEYWORD_INLINE,
        TokenKind::KEYWORD_NO_INLINE,
        TokenKind::KEYWORD_OPERATOR,
        TokenKind::KEYWORD_CONTEXT,
        TokenKind::KEYWORD_PUSH_CONTEXT,
        TokenKind::KEYWORD_SIZE_OF,
        TokenKind::KEYWORD_TYPE_OF,
        TokenKind::KEYWORD_CODE_OF,
        TokenKind::KEYWORD_TYPE_INFO,
        TokenKind::KEYWORD_INITIALIZER_OF,
        TokenKind::KEYWORD_IS_CONSTANT,
      ]
    );
  }

  #[test]
  fn words_that_look_like_keywords_are_not() {
    assert_eq!(
      kinds("it it_index type default extern _ __reg iff forr"),
      [TokenKind::IDENT; 9]
    );
  }

  #[test]
  fn a_directive_is_a_hash_followed_by_an_ordinary_identifier() {
    assert_eq!(
      kinds("#import \"Basic\""),
      [TokenKind::HASH, TokenKind::IDENT, TokenKind::STRING]
    );
    assert_eq!(names("#if #run #foreign"), ["if", "run", "foreign"]);
    assert_eq!(
      kinds("#if"),
      [TokenKind::HASH, TokenKind::KEYWORD_IF],
      "a directive named after a keyword still lexes as that keyword"
    );
  }

  #[test]
  fn a_backslash_continues_an_identifier_across_alignment_spaces() {
    assert_eq!(names("read\\_package"), ["read_package"]);
    assert_eq!(names("time\\        _report"), ["time_report"]);
    assert_eq!(names("frame\\  _color"), ["frame_color"]);
    assert_eq!(names("foo\\   bar"), ["foobar"]);
    assert_eq!(
      names("i\\f"),
      ["if"],
      "the keyword check runs on the joined name"
    );
    assert_eq!(kinds("i\\f"), [TokenKind::KEYWORD_IF]);
  }

  #[test]
  fn a_backslash_continuation_does_not_cross_a_tab_or_a_newline() {
    assert_eq!(names("foo\\\n  bar"), ["foo", "bar"]);
    assert_eq!(names("foo\\\tbar"), ["foo", "bar"]);
  }

  #[test]
  fn operators_take_the_longest_match() {
    assert_eq!(
      kinds("<<< >>> << >> <<= >>= <<<= >>>= < > <= >="),
      [
        TokenKind::ROTATE_LEFT,
        TokenKind::ROTATE_RIGHT,
        TokenKind::POINTER_DEREFERENCE_OR_SHIFT_LEFT,
        TokenKind::SHIFT_RIGHT,
        TokenKind::SHIFT_LEFT_EQUALS,
        TokenKind::SHIFT_RIGHT_EQUALS,
        TokenKind::ROTATE_LEFT_EQUALS,
        TokenKind::ROTATE_RIGHT_EQUALS,
        TokenKind::LESS_THAN,
        TokenKind::GREATER_THAN,
        TokenKind::LESSEQUALS,
        TokenKind::GREATEREQUALS,
      ]
    );

    assert_eq!(
      kinds("=== == => = != ! && || &&= ||= & | &= |= ^ ^= ~"),
      [
        TokenKind::TRIPLE_EQUALS,
        TokenKind::ISEQUAL,
        TokenKind::QUICK_LAMBDA,
        TokenKind::EQUALS,
        TokenKind::ISNOTEQUAL,
        TokenKind::BANG,
        TokenKind::LOGICAL_AND,
        TokenKind::LOGICAL_OR,
        TokenKind::LOGICAL_AND_EQUALS,
        TokenKind::LOGICAL_OR_EQUALS,
        TokenKind::BITWISE_AND,
        TokenKind::BITWISE_OR,
        TokenKind::BITWISE_AND_EQUALS,
        TokenKind::BITWISE_OR_EQUALS,
        TokenKind::BITWISE_XOR,
        TokenKind::BITWISE_XOR_EQUALS,
        TokenKind::BITWISE_NOT,
      ]
    );

    assert_eq!(
      kinds("--- -- -> - -= += *= /= %= ,, , $$ $ .. .* .[ .{ ."),
      [
        TokenKind::TRIPLE_MINUS,
        TokenKind::DOUBLE_MINUS,
        TokenKind::RIGHT_ARROW,
        TokenKind::MINUS,
        TokenKind::MINUSEQUALS,
        TokenKind::PLUSEQUALS,
        TokenKind::TIMESEQUALS,
        TokenKind::DIVEQUALS,
        TokenKind::MODEQUALS,
        TokenKind::DOUBLE_COMMA,
        TokenKind::COMMA,
        TokenKind::DOUBLE_DOLLAR,
        TokenKind::DOLLAR,
        TokenKind::DOUBLE_DOT,
        TokenKind::POSTFIX_DEREFERENCE,
        TokenKind::BEGIN_ARRAY_LITERAL,
        TokenKind::BEGIN_STRUCT_LITERAL,
        TokenKind::DOT,
      ]
    );
  }

  #[test]
  fn brackets_and_punctuation_are_their_own_ascii_codes() {
    assert_eq!(
      kinds("( ) [ ] { } ; : ? ' @a"),
      [
        TokenKind::OPEN_PAREN,
        TokenKind::CLOSE_PAREN,
        TokenKind::OPEN_BRACKET,
        TokenKind::CLOSE_BRACKET,
        TokenKind::OPEN_BRACE,
        TokenKind::CLOSE_BRACE,
        TokenKind::SEMICOLON,
        TokenKind::COLON,
        TokenKind::QUESTION,
        TokenKind::SINGLE_QUOTE,
        TokenKind::NOTE,
      ]
    );
  }

  #[test]
  fn comments_are_skipped_and_block_comments_nest() {
    assert_eq!(kinds("a // b c\nd"), [TokenKind::IDENT; 2]);
    assert_eq!(kinds("a /* b /* c */ d */ e"), [TokenKind::IDENT; 2]);
    assert_eq!(kinds("a /**/ b"), [TokenKind::IDENT; 2]);
    assert_eq!(names("a /* b /* c */ d */ e"), ["a", "e"]);
  }

  #[test]
  fn an_unterminated_block_comment_is_an_error() {
    assert_eq!(
      errors("a /* b /* c */"),
      ["End of input from within a comment."]
    );
  }

  #[test]
  fn a_hashbang_line_is_skipped() {
    assert_eq!(names("#!/usr/bin/env jai\nmain"), ["main"]);
    assert_eq!(
      kinds("a\n#!not_a_hashbang"),
      [TokenKind::IDENT],
      "a `#!` at the start of a later line is skipped too"
    );
    assert_eq!(
      kinds("a #!b"),
      [
        TokenKind::IDENT,
        TokenKind::HASH,
        TokenKind::BANG,
        TokenKind::IDENT
      ],
      "`#!` away from the start of a line is not a hashbang"
    );
  }

  #[test]
  fn decimal_hexadecimal_and_binary_integers() {
    assert_eq!(first("123").integer(), Some(123));
    assert_eq!(first("1_000_000").integer(), Some(1_000_000));
    assert_eq!(first("0xffff_ffff_ffff_ffff").integer(), Some(u64::MAX));
    assert_eq!(first("0xaBadBabe").integer(), Some(0xabad_babe));
    assert_eq!(first("0XFF").integer(), Some(255));
    assert_eq!(first("0b11").integer(), Some(3));
    assert_eq!(first("0B1010").integer(), Some(10));

    assert_eq!(first("0xff").flags, ValueFlags::NUMBER | ValueFlags::HEX);
    assert_eq!(first("0b11").flags, ValueFlags::NUMBER | ValueFlags::BINARY);
    assert_eq!(first("7").flags, ValueFlags::NUMBER);
  }

  #[test]
  fn a_range_after_a_number_is_not_a_decimal_point() {
    let (tokens, _, _) = lex("0..7");

    assert_eq!(tokens[0].integer(), Some(0));
    assert_eq!(tokens[1].kind, TokenKind::DOUBLE_DOT);
    assert_eq!(tokens[2].integer(), Some(7));
  }

  #[test]
  fn an_integer_that_does_not_fit_in_64_bits_is_an_error() {
    assert_eq!(
      errors("99999999999999999999999"),
      ["Integer literal is too big. All integer literals must fit into 64 bits."]
    );
    assert!(errors("18446744073709551615").is_empty());
    assert!(
      !first("18446744073709551615")
        .flags
        .contains(ValueFlags::OVERFLOWED)
    );
  }

  #[test]
  fn a_decimal_literal_above_the_signed_range_is_marked_unsigned_only() {
    // The reference has no dedicated flag and reuses HEX so that the literal
    // prints unsigned. It tests the accumulator before adding the last digit,
    // so the mark starts a little above 2^63 rather than exactly at it.
    assert!(first("9999999999999999999").flags.contains(ValueFlags::HEX));
    assert!(!first("9223372036854775807").flags.contains(ValueFlags::HEX));
  }

  #[test]
  fn a_bad_digit_or_decimal_point_is_reported() {
    assert_eq!(errors("0b12"), ["Invalid digit in a binary number."]);
    assert_eq!(
      errors("0x1.2"),
      ["Cannot use a decimal point in a hexadecimal number!"]
    );
    assert_eq!(
      errors("0b1.0"),
      ["Cannot use a decimal point in a binary number!"]
    );
    assert_eq!(errors("1.2.3"), ["Can't have two decimals in a number!"]);
  }

  #[test]
  fn float_literal_forms() {
    assert_eq!(first("1.0").float(), Some(1.0));
    assert_eq!(first(".5").float(), Some(0.5));
    assert_eq!(first("2340.").float(), Some(2340.0));
    assert_eq!(first("1.5e-3").float(), Some(1.5e-3));
    assert_eq!(first("1.5e3").float(), Some(1500.0));
    assert_eq!(first("6.0E+2").float(), Some(600.0));
    assert_eq!(first("1_0.5_5").float(), Some(10.55));
    assert!(first("1.0").flags.contains(ValueFlags::FLOAT));
  }

  #[test]
  fn an_exponent_without_a_decimal_point_is_not_a_float() {
    let (tokens, _, interner) = lex("1e10");

    assert_eq!(tokens[0].integer(), Some(1));
    assert_eq!(tokens[1].kind, TokenKind::IDENT);
    assert_eq!(interner.resolve(tokens[1].name().unwrap()), b"e10");
  }

  #[test]
  fn a_malformed_exponent_or_suffix_is_reported() {
    assert_eq!(
      errors("1.5e"),
      ["'e' in a float literal must be followed by + or - or a numerical digit."]
    );
    assert_eq!(
      errors("1.0f"),
      ["In this language, we don't suffix our float constants with f."]
    );
    assert_eq!(errors("1.0e1e1"), ["Can't have two exponents in a number!"]);
  }

  #[test]
  fn float_width_follows_the_significant_digits_and_the_exponent() {
    assert_eq!(first("1.5").flags, ValueFlags::NUMBER | ValueFlags::FLOAT);

    assert!(
      first("1.2345678")
        .flags
        .contains(ValueFlags::DEFAULTS_TO_FLOAT64),
      "exactly eight significant digits defaults to float64"
    );
    assert!(
      first("1.23456789")
        .flags
        .contains(ValueFlags::REQUIRES_FLOAT64_DUE_TO_SIGNIFICANT_DIGITS),
      "more than eight significant digits requires float64"
    );
    assert!(
      first("1.23456789")
        .flags
        .contains(ValueFlags::REQUIRES_FLOAT64)
    );
    assert!(
      first("1.0e40").flags.contains(ValueFlags::REQUIRES_FLOAT64),
      "an exponent past the float32 range requires float64"
    );
    assert!(
      first("1.0e-40")
        .flags
        .contains(ValueFlags::REQUIRES_FLOAT64)
    );
    assert!(
      !first("1.000000000")
        .flags
        .contains(ValueFlags::REQUIRES_FLOAT64),
      "trailing zeroes are not significant"
    );
  }

  #[test]
  fn hexfloats_are_ieee_bit_patterns() {
    assert_eq!(first("0hff80_0000").float(), Some(f32::NEG_INFINITY.into()));
    assert_eq!(first("0h7FF00000_00000000").float(), Some(f64::INFINITY));
    assert_eq!(first("0h3f80_0000").float(), Some(1.0));
    assert_eq!(
      first("0h3C00").float(),
      Some(f64::from(f32::from_bits(0x3c00)))
    );

    assert_eq!(
      first("0h3f80_0000").flags,
      ValueFlags::NUMBER | ValueFlags::FLOAT | ValueFlags::HEX
    );
    assert!(
      first("0h7FF00000_00000000")
        .flags
        .contains(ValueFlags::REQUIRES_FLOAT64)
    );
  }

  #[test]
  fn a_hexfloat_must_have_four_eight_or_sixteen_digits() {
    assert_eq!(
      errors("0h123"),
      ["Numbers designated by 0h must be 4, 8 or 16 digits; this one is 3 digits."]
    );
    assert_eq!(
      errors("0h"),
      ["Expected some hexadecimal characters after \"0h\", but found none."]
    );
    assert_eq!(
      errors("0h1234567890abcdef0")[0],
      "Hexfloat is too long! (16 character maximum.)"
    );
  }

  #[test]
  fn string_escapes() {
    assert_eq!(text(r#""abc""#), b"abc");
    assert_eq!(text(r#""a\nb\rc\td\0e""#), b"a\nb\rc\td\0e");
    assert_eq!(text(r#""\e""#), [0x1b]);
    assert_eq!(text(r#""\"\\""#), br#""\"#);
    assert_eq!(text(r#""\x41\xff""#), [0x41, 0xff]);
    assert_eq!(text(r#""\d010\d013""#), [10, 13]);
    assert_eq!(text(r#""é""#), "é".as_bytes());
    assert_eq!(text(r#""\U0001F600""#), "\u{1f600}".as_bytes());
    assert_eq!(text(r#""\%""#), [31]);
    assert_eq!(text(r#""""#), b"");
  }

  #[test]
  fn an_unknown_escape_warns_and_yields_the_character() {
    assert_eq!(
      warnings(r#""a\q b""#),
      ["Unknown escape sequence '\\q' in string constant!"]
    );
    assert_eq!(text(r#""a\q b""#), b"aq b");
  }

  #[test]
  fn a_string_may_not_span_a_line_or_the_end_of_the_file() {
    assert_eq!(errors("\"abc\ndef;"), ["Newline in string constant!"]);
    assert_eq!(errors("\"abc"), ["End of file during string constant!"]);
  }

  #[test]
  fn a_malformed_numeric_escape_is_reported() {
    assert_eq!(errors(r#""\xzz""#)[0], "A hex digit is required here.");
    assert_eq!(errors(r#""\dz""#)[0], "A decimal digit is required here.");
    assert_eq!(
      errors(r#""\d300""#),
      ["Decimal value of 300 exceeds the limit of 255."]
    );
  }

  #[test]
  fn notes_are_an_at_sign_and_a_name_or_a_string() {
    let (tokens, _, interner) = lex("@PrintLike @--- @\"a string note\" @Bar, ;");
    let notes: Vec<_> = tokens
      .iter()
      .filter(|token| token.kind == TokenKind::NOTE)
      .map(|token| interner.resolve_lossy(token.name().unwrap()).into_owned())
      .collect();

    assert_eq!(
      notes,
      ["PrintLike", "---", "a string note", "Bar,"],
      "a note runs to whitespace or a semicolon, so a comma is part of it"
    );
  }

  #[test]
  fn a_note_needs_a_name() {
    assert_eq!(errors("@ x"), ["Empty note."]);
    assert_eq!(errors("@;"), ["Empty note."]);
  }

  #[test]
  fn a_backtick_marks_an_identifier_or_one_of_four_keywords() {
    for source in ["`it", "`defer", "`return", "`operator", "`push_context"] {
      let token = first(source);
      assert!(token.backticked, "{source} should be backticked");
      assert!(errors(source).is_empty(), "{source} should not error");
    }
  }

  #[test]
  fn a_backtick_on_any_other_keyword_is_an_error() {
    assert_eq!(
      errors("`while"),
      [
        "Expected an identifier after '`', but 'while' is a keyword. ('defer', 'return', \
         'push_context' and overloaded operator functions are the only keywords that can be \
         backticked.)"
      ]
    );
    assert_eq!(errors("`1"), ["Expected an identifier after '`'."]);
  }

  #[test]
  fn a_here_string_runs_to_its_terminator_line() {
    let token = first("#string DONE\nline one\n  line two\nDONE");

    assert_eq!(token.kind, TokenKind::STRING);
    assert_eq!(token.flags, ValueFlags::HERE_STRING);
    assert_eq!(token.text(), Some(b"line one\n  line two\n".as_slice()));
  }

  #[test]
  fn a_here_string_spans_the_whole_construct() {
    let source = "#string DONE\nbody\nDONE";
    let token = first(source);

    assert_eq!(token.span, Span::from_offsets(0, source.len()));
  }

  #[test]
  fn a_here_string_terminator_may_be_indented_but_must_end_the_word() {
    assert_eq!(
      first("#string END\na\n   END").text(),
      Some(b"a\n".as_slice())
    );
    assert_eq!(
      first("#string END\nENDING\nEND").text(),
      Some(b"ENDING\n".as_slice()),
      "a longer word starting with the terminator does not terminate"
    );
  }

  #[test]
  fn a_here_string_normalizes_its_line_endings() {
    assert_eq!(
      first("#string D\r\na\r\nb\r\nD").text(),
      Some(b"a\nb\n".as_slice())
    );
    assert_eq!(
      first("#string,cr D\na\nD").text(),
      Some(b"a\r\n".as_slice())
    );
    assert_eq!(
      first("#string,cr D\r\na\r\nD").text(),
      Some(b"a\r\n".as_slice()),
      "`,cr` does not double an existing carriage return"
    );
  }

  #[test]
  fn a_here_string_processes_no_escapes() {
    assert_eq!(
      first("#string D\n\\n \\% \"q\"\nD").text(),
      Some(b"\\n \\% \"q\"\n".as_slice())
    );
  }

  #[test]
  fn a_malformed_here_string_is_reported() {
    assert_eq!(
      errors("#string 1\n"),
      ["Expected identifier after #string."]
    );
    assert_eq!(
      errors("#string,xx D\n"),
      ["Invalid #string modifier after ','"]
    );
    assert_eq!(
      errors("#string D x\n"),
      ["Unexpected non-whitespace characters after #string identifier."]
    );
    assert_eq!(
      errors("#string D\nbody\n"),
      ["Unexpected end of file inside #string."]
    );
    assert_eq!(errors("#string D"), ["Reached end-of-file in #string."]);
  }

  #[test]
  fn invisible_unicode_is_skipped_with_a_warning() {
    assert_eq!(names("a\u{200b}b"), ["a", "b"]);
    assert_eq!(warnings("a\u{200b}b").len(), 1);
    assert_eq!(names("a\u{202a}b"), ["a", "b"]);
    assert_eq!(warnings("a\u{202a}b").len(), 1);
    assert_eq!(names("a\u{a0}b"), ["a", "b"]);
    assert_eq!(warnings("a\u{a0}b").len(), 1);
  }

  #[test]
  fn any_other_non_ascii_character_outside_a_string_is_an_error() {
    assert_eq!(
      errors("a\u{2603}b"),
      [
        "Unicode characters in this range are not supported by the parser at lexer time. \
         (Offending character code was U+2603.)"
      ]
    );
    assert!(errors("\"a\u{2603}b\"").is_empty());
    assert!(errors("// a\u{2603}b").is_empty());
  }

  #[test]
  fn spans_cover_the_text_a_token_was_made_from() {
    let (tokens, _, _) = lex("main :: () {}");

    assert_eq!(tokens[0].span, Span::new(0, 4));
    assert_eq!(tokens[1].span, Span::new(5, 6));
    assert_eq!(tokens[3].span, Span::new(8, 9));
    assert_eq!(tokens[6].span, Span::new(12, 13));
    assert_eq!(tokens[7].span, Span::new(13, 13));

    assert_eq!(
      first("`foo").span,
      Span::new(0, 4),
      "a backtick is in the span"
    );
    assert_eq!(first("@Note").span, Span::new(0, 5), "an @ is in the span");
  }
}

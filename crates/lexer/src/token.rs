use std::fmt;

use bitflags::bitflags;
use oj_diag::Span;

use crate::interner::Symbol;

/// A token kind, numbered exactly as `Jai_Lexer.Token_Type` (**C§5.1**):
/// single-character tokens are their own ASCII code, everything else has the
/// value the reference enum gives it. Keeping the numbers means `Jai_Lexer`,
/// `Program_Print` and metaprograms agree with us without a translation table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenKind(pub i16);

impl TokenKind {
  pub const BANG: Self = Self(b'!' as i16);
  pub const HASH: Self = Self(b'#' as i16);
  pub const DOLLAR: Self = Self(b'$' as i16);
  pub const PERCENT: Self = Self(b'%' as i16);
  pub const BITWISE_AND: Self = Self(b'&' as i16);
  pub const SINGLE_QUOTE: Self = Self(b'\'' as i16);
  pub const OPEN_PAREN: Self = Self(b'(' as i16);
  pub const CLOSE_PAREN: Self = Self(b')' as i16);
  pub const ASTERISK: Self = Self(b'*' as i16);
  pub const PLUS: Self = Self(b'+' as i16);
  pub const COMMA: Self = Self(b',' as i16);
  pub const MINUS: Self = Self(b'-' as i16);
  pub const DOT: Self = Self(b'.' as i16);
  pub const SLASH: Self = Self(b'/' as i16);
  pub const COLON: Self = Self(b':' as i16);
  pub const SEMICOLON: Self = Self(b';' as i16);
  pub const LESS_THAN: Self = Self(b'<' as i16);
  pub const EQUALS: Self = Self(b'=' as i16);
  pub const GREATER_THAN: Self = Self(b'>' as i16);
  pub const QUESTION: Self = Self(b'?' as i16);
  pub const OPEN_BRACKET: Self = Self(b'[' as i16);
  pub const CLOSE_BRACKET: Self = Self(b']' as i16);
  pub const BITWISE_XOR: Self = Self(b'^' as i16);
  pub const OPEN_BRACE: Self = Self(b'{' as i16);
  pub const BITWISE_OR: Self = Self(b'|' as i16);
  pub const CLOSE_BRACE: Self = Self(b'}' as i16);
  pub const BITWISE_NOT: Self = Self(b'~' as i16);

  pub const IDENT: Self = Self(256);
  pub const NUMBER: Self = Self(257);
  pub const STRING: Self = Self(258);

  pub const PLUSEQUALS: Self = Self(259);
  pub const MINUSEQUALS: Self = Self(260);
  pub const TIMESEQUALS: Self = Self(261);
  pub const DIVEQUALS: Self = Self(262);
  pub const MODEQUALS: Self = Self(263);
  pub const ISEQUAL: Self = Self(264);
  pub const ISNOTEQUAL: Self = Self(265);
  pub const LOGICAL_AND: Self = Self(266);
  pub const LOGICAL_OR: Self = Self(267);
  pub const LESSEQUALS: Self = Self(268);
  pub const GREATEREQUALS: Self = Self(269);

  pub const RIGHT_ARROW: Self = Self(270);
  pub const DOUBLE_DOT: Self = Self(271);
  pub const DOUBLE_DOLLAR: Self = Self(272);

  pub const SHIFT_LEFT: Self = Self(273);
  pub const SHIFT_RIGHT: Self = Self(274);
  pub const ROTATE_LEFT: Self = Self(275);
  pub const ROTATE_RIGHT: Self = Self(276);
  pub const TRIPLE_EQUALS: Self = Self(277);

  pub const SHIFT_LEFT_EQUALS: Self = Self(290);
  pub const SHIFT_RIGHT_EQUALS: Self = Self(291);
  pub const ROTATE_LEFT_EQUALS: Self = Self(292);
  pub const ROTATE_RIGHT_EQUALS: Self = Self(293);
  pub const BITWISE_AND_EQUALS: Self = Self(294);
  pub const BITWISE_OR_EQUALS: Self = Self(295);
  pub const BITWISE_XOR_EQUALS: Self = Self(296);
  pub const LOGICAL_AND_EQUALS: Self = Self(297);
  pub const LOGICAL_OR_EQUALS: Self = Self(298);

  pub const POINTER_DEREFERENCE: Self = Self(310);
  pub const POSTFIX_DEREFERENCE: Self = Self(311);
  pub const ISEQUAL_FOR_SWITCH_STATEMENT: Self = Self(312);
  pub const DOUBLE_MINUS: Self = Self(313);
  pub const TRIPLE_MINUS: Self = Self(314);
  pub const DOUBLE_COMMA: Self = Self(315);
  pub const BEGIN_STRUCT_LITERAL: Self = Self(316);
  pub const BEGIN_ARRAY_LITERAL: Self = Self(317);

  pub const KEYWORD_FOR: Self = Self(318);
  pub const KEYWORD_IF: Self = Self(319);
  pub const KEYWORD_IFX: Self = Self(320);
  pub const KEYWORD_THEN: Self = Self(321);
  pub const KEYWORD_ELSE: Self = Self(322);
  pub const KEYWORD_CASE: Self = Self(323);
  pub const KEYWORD_RETURN: Self = Self(324);
  pub const KEYWORD_STRUCT: Self = Self(325);
  pub const KEYWORD_WHILE: Self = Self(326);
  pub const KEYWORD_BREAK: Self = Self(327);
  pub const KEYWORD_CONTINUE: Self = Self(328);
  pub const KEYWORD_REMOVE: Self = Self(329);
  pub const KEYWORD_USING: Self = Self(330);
  pub const KEYWORD_DEFER: Self = Self(331);
  pub const KEYWORD_SIZE_OF: Self = Self(332);
  pub const KEYWORD_TYPE_OF: Self = Self(333);
  pub const KEYWORD_CODE_OF: Self = Self(334);
  pub const KEYWORD_INITIALIZER_OF: Self = Self(335);
  pub const KEYWORD_TYPE_INFO: Self = Self(336);
  pub const KEYWORD_NULL: Self = Self(337);
  pub const KEYWORD_ENUM: Self = Self(338);
  pub const KEYWORD_TRUE: Self = Self(339);
  pub const KEYWORD_FALSE: Self = Self(340);
  pub const KEYWORD_INLINE: Self = Self(341);
  pub const KEYWORD_NO_INLINE: Self = Self(342);
  pub const KEYWORD_CAST: Self = Self(343);
  pub const KEYWORD_AUTO_CAST: Self = Self(344);
  pub const KEYWORD_CONTEXT: Self = Self(345);
  pub const KEYWORD_PUSH_CONTEXT: Self = Self(346);
  pub const KEYWORD_OPERATOR: Self = Self(347);
  pub const KEYWORD_IS_CONSTANT: Self = Self(348);
  pub const KEYWORD_ENUM_FLAGS: Self = Self(349);
  pub const KEYWORD_UNION: Self = Self(350);
  pub const KEYWORD_INTERFACE: Self = Self(351);

  pub const FIRST_KEYWORD: Self = Self::KEYWORD_FOR;
  pub const LAST_KEYWORD: Self = Self::KEYWORD_INTERFACE;

  pub const QUICK_LAMBDA: Self = Self(352);
  pub const NOTE: Self = Self(353);
  pub const END_OF_INPUT: Self = Self(354);
  pub const POINTER_DEREFERENCE_OR_SHIFT_LEFT: Self = Self(355);

  pub const OPERATOR_ARRAY_SUBSCRIPT: Self = Self(500);
  pub const OPERATOR_ASSIGNMENT_TO_ARRAY_SUBSCRIPT: Self = Self(501);
  pub const ERROR: Self = Self(502);

  pub fn is_keyword(self) -> bool {
    (Self::FIRST_KEYWORD.0..=Self::LAST_KEYWORD.0).contains(&self.0)
  }

  /// The single ASCII byte this kind stands for, if it is one.
  pub fn as_ascii(self) -> Option<u8> {
    (0..=0x7f).contains(&self.0).then_some(self.0 as u8)
  }

  /// The reference name of a kind that has one; single-character tokens do not.
  pub fn name(self) -> Option<&'static str> {
    Some(match self {
      Self::IDENT => "IDENT",
      Self::NUMBER => "NUMBER",
      Self::STRING => "STRING",
      Self::PLUSEQUALS => "PLUSEQUALS",
      Self::MINUSEQUALS => "MINUSEQUALS",
      Self::TIMESEQUALS => "TIMESEQUALS",
      Self::DIVEQUALS => "DIVEQUALS",
      Self::MODEQUALS => "MODEQUALS",
      Self::ISEQUAL => "ISEQUAL",
      Self::ISNOTEQUAL => "ISNOTEQUAL",
      Self::LOGICAL_AND => "LOGICAL_AND",
      Self::LOGICAL_OR => "LOGICAL_OR",
      Self::LESSEQUALS => "LESSEQUALS",
      Self::GREATEREQUALS => "GREATEREQUALS",
      Self::RIGHT_ARROW => "RIGHT_ARROW",
      Self::DOUBLE_DOT => "DOUBLE_DOT",
      Self::DOUBLE_DOLLAR => "DOUBLE_DOLLAR",
      Self::SHIFT_LEFT => "SHIFT_LEFT",
      Self::SHIFT_RIGHT => "SHIFT_RIGHT",
      Self::ROTATE_LEFT => "ROTATE_LEFT",
      Self::ROTATE_RIGHT => "ROTATE_RIGHT",
      Self::TRIPLE_EQUALS => "TRIPLE_EQUALS",
      Self::SHIFT_LEFT_EQUALS => "SHIFT_LEFT_EQUALS",
      Self::SHIFT_RIGHT_EQUALS => "SHIFT_RIGHT_EQUALS",
      Self::ROTATE_LEFT_EQUALS => "ROTATE_LEFT_EQUALS",
      Self::ROTATE_RIGHT_EQUALS => "ROTATE_RIGHT_EQUALS",
      Self::BITWISE_AND_EQUALS => "BITWISE_AND_EQUALS",
      Self::BITWISE_OR_EQUALS => "BITWISE_OR_EQUALS",
      Self::BITWISE_XOR_EQUALS => "BITWISE_XOR_EQUALS",
      Self::LOGICAL_AND_EQUALS => "LOGICAL_AND_EQUALS",
      Self::LOGICAL_OR_EQUALS => "LOGICAL_OR_EQUALS",
      Self::POINTER_DEREFERENCE => "POINTER_DEREFERENCE",
      Self::POSTFIX_DEREFERENCE => "POSTFIX_DEREFERENCE",
      Self::ISEQUAL_FOR_SWITCH_STATEMENT => "ISEQUAL_FOR_SWITCH_STATEMENT",
      Self::DOUBLE_MINUS => "DOUBLE_MINUS",
      Self::TRIPLE_MINUS => "TRIPLE_MINUS",
      Self::DOUBLE_COMMA => "DOUBLE_COMMA",
      Self::BEGIN_STRUCT_LITERAL => "BEGIN_STRUCT_LITERAL",
      Self::BEGIN_ARRAY_LITERAL => "BEGIN_ARRAY_LITERAL",
      Self::KEYWORD_FOR => "KEYWORD_FOR",
      Self::KEYWORD_IF => "KEYWORD_IF",
      Self::KEYWORD_IFX => "KEYWORD_IFX",
      Self::KEYWORD_THEN => "KEYWORD_THEN",
      Self::KEYWORD_ELSE => "KEYWORD_ELSE",
      Self::KEYWORD_CASE => "KEYWORD_CASE",
      Self::KEYWORD_RETURN => "KEYWORD_RETURN",
      Self::KEYWORD_STRUCT => "KEYWORD_STRUCT",
      Self::KEYWORD_WHILE => "KEYWORD_WHILE",
      Self::KEYWORD_BREAK => "KEYWORD_BREAK",
      Self::KEYWORD_CONTINUE => "KEYWORD_CONTINUE",
      Self::KEYWORD_REMOVE => "KEYWORD_REMOVE",
      Self::KEYWORD_USING => "KEYWORD_USING",
      Self::KEYWORD_DEFER => "KEYWORD_DEFER",
      Self::KEYWORD_SIZE_OF => "KEYWORD_SIZE_OF",
      Self::KEYWORD_TYPE_OF => "KEYWORD_TYPE_OF",
      Self::KEYWORD_CODE_OF => "KEYWORD_CODE_OF",
      Self::KEYWORD_INITIALIZER_OF => "KEYWORD_INITIALIZER_OF",
      Self::KEYWORD_TYPE_INFO => "KEYWORD_TYPE_INFO",
      Self::KEYWORD_NULL => "KEYWORD_NULL",
      Self::KEYWORD_ENUM => "KEYWORD_ENUM",
      Self::KEYWORD_TRUE => "KEYWORD_TRUE",
      Self::KEYWORD_FALSE => "KEYWORD_FALSE",
      Self::KEYWORD_INLINE => "KEYWORD_INLINE",
      Self::KEYWORD_NO_INLINE => "KEYWORD_NO_INLINE",
      Self::KEYWORD_CAST => "KEYWORD_CAST",
      Self::KEYWORD_AUTO_CAST => "KEYWORD_AUTO_CAST",
      Self::KEYWORD_CONTEXT => "KEYWORD_CONTEXT",
      Self::KEYWORD_PUSH_CONTEXT => "KEYWORD_PUSH_CONTEXT",
      Self::KEYWORD_OPERATOR => "KEYWORD_OPERATOR",
      Self::KEYWORD_IS_CONSTANT => "KEYWORD_IS_CONSTANT",
      Self::KEYWORD_ENUM_FLAGS => "KEYWORD_ENUM_FLAGS",
      Self::KEYWORD_UNION => "KEYWORD_UNION",
      Self::KEYWORD_INTERFACE => "KEYWORD_INTERFACE",
      Self::QUICK_LAMBDA => "QUICK_LAMBDA",
      Self::NOTE => "NOTE",
      Self::END_OF_INPUT => "END_OF_INPUT",
      Self::POINTER_DEREFERENCE_OR_SHIFT_LEFT => "POINTER_DEREFERENCE_OR_SHIFT_LEFT",
      Self::OPERATOR_ARRAY_SUBSCRIPT => "OPERATOR_ARRAY_SUBSCRIPT",
      Self::OPERATOR_ASSIGNMENT_TO_ARRAY_SUBSCRIPT => "OPERATOR_ASSIGNMENT_TO_ARRAY_SUBSCRIPT",
      Self::ERROR => "ERROR",
      _ => return None,
    })
  }
}

impl fmt::Display for TokenKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match (self.name(), self.as_ascii()) {
      (Some(name), _) => f.write_str(name),
      (None, Some(byte)) if byte.is_ascii_graphic() => write!(f, "'{}'", byte as char),
      _ => write!(f, "TOKEN({})", self.0),
    }
  }
}

impl fmt::Debug for TokenKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(self, f)
  }
}

bitflags! {
  /// `Jai_Lexer.Value_Flags`, with the reference bit values.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct ValueFlags: u32 {
    const HERE_STRING = 0x1;
    const NUMBER = 0x2;
    const HEX = 0x4;
    const BINARY = 0x8;
    const FLOAT = 0x10;
    const REQUIRES_FLOAT64 = 0x10_0000;
    const DEFAULTS_TO_FLOAT64 = 0x20_0000;
    const REQUIRES_FLOAT64_DUE_TO_SIGNIFICANT_DIGITS = 0x40_0000;
    const OVERFLOWED = 0x1000_0000;
  }
}

impl ValueFlags {
  const NAMES: [(ValueFlags, &'static str); 9] = [
    (ValueFlags::HERE_STRING, "HERE_STRING"),
    (ValueFlags::NUMBER, "NUMBER"),
    (ValueFlags::HEX, "HEX"),
    (ValueFlags::BINARY, "BINARY"),
    (ValueFlags::FLOAT, "FLOAT"),
    (ValueFlags::REQUIRES_FLOAT64, "REQUIRES_FLOAT64"),
    (ValueFlags::DEFAULTS_TO_FLOAT64, "DEFAULTS_TO_FLOAT64"),
    (
      ValueFlags::REQUIRES_FLOAT64_DUE_TO_SIGNIFICANT_DIGITS,
      "REQUIRES_FLOAT64_DUE_TO_SIGNIFICANT_DIGITS",
    ),
    (ValueFlags::OVERFLOWED, "OVERFLOWED"),
  ];

  pub fn names(self) -> Vec<&'static str> {
    Self::NAMES
      .iter()
      .filter(|(flag, _)| self.contains(*flag))
      .map(|(_, name)| *name)
      .collect()
  }
}

/// The payload of a token, mirroring the `Jai_Lexer.Token` union.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenValue {
  None,
  /// An `IDENT` or `NOTE` name, interned.
  Name(Symbol),
  Integer(u64),
  Float(f64),
  /// A string literal's bytes, after escape processing. Jai strings are byte
  /// strings, and `\xff` or `\d255` can put any byte in one.
  Text(Box<[u8]>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
  pub kind: TokenKind,
  /// The source text this token was made from, including a leading backtick and
  /// the `#string IDENT … IDENT` framing of a here-string.
  pub span: Span,
  pub value: TokenValue,
  pub flags: ValueFlags,
  pub backticked: bool,
}

impl Token {
  pub fn new(kind: TokenKind, span: Span) -> Self {
    Self {
      kind,
      span,
      value: TokenValue::None,
      flags: ValueFlags::empty(),
      backticked: false,
    }
  }

  pub fn name(&self) -> Option<Symbol> {
    match self.value {
      TokenValue::Name(symbol) => Some(symbol),
      _ => None,
    }
  }

  pub fn integer(&self) -> Option<u64> {
    match self.value {
      TokenValue::Integer(value) => Some(value),
      _ => None,
    }
  }

  pub fn float(&self) -> Option<f64> {
    match self.value {
      TokenValue::Float(value) => Some(value),
      _ => None,
    }
  }

  pub fn text(&self) -> Option<&[u8]> {
    match &self.value {
      TokenValue::Text(bytes) => Some(bytes),
      _ => None,
    }
  }

  pub fn is_float(&self) -> bool {
    self.flags.contains(ValueFlags::FLOAT)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn kinds_carry_the_reference_numbers() {
    assert_eq!(TokenKind::IDENT.0, 256);
    assert_eq!(TokenKind::TRIPLE_EQUALS.0, 277);
    assert_eq!(TokenKind::LOGICAL_OR_EQUALS.0, 298);
    assert_eq!(TokenKind::BEGIN_ARRAY_LITERAL.0, 317);
    assert_eq!(TokenKind::KEYWORD_FOR.0, 318);
    assert_eq!(TokenKind::KEYWORD_INTERFACE.0, 351);
    assert_eq!(TokenKind::END_OF_INPUT.0, 354);
    assert_eq!(TokenKind::POINTER_DEREFERENCE_OR_SHIFT_LEFT.0, 355);
    assert_eq!(TokenKind::ERROR.0, 502);
  }

  #[test]
  fn single_character_kinds_are_their_own_ascii_code() {
    assert_eq!(TokenKind::OPEN_PAREN.0, 40);
    assert_eq!(TokenKind::BITWISE_XOR.0, 94);
    assert_eq!(TokenKind::BITWISE_NOT.0, 126);
    assert_eq!(TokenKind::SEMICOLON.as_ascii(), Some(b';'));
    assert_eq!(TokenKind::IDENT.as_ascii(), None);
  }

  #[test]
  fn the_keyword_range_covers_exactly_the_keywords() {
    assert!(TokenKind::KEYWORD_FOR.is_keyword());
    assert!(TokenKind::KEYWORD_INTERFACE.is_keyword());
    assert!(!TokenKind::BEGIN_ARRAY_LITERAL.is_keyword());
    assert!(!TokenKind::QUICK_LAMBDA.is_keyword());
  }

  #[test]
  fn kinds_display_by_name_or_as_the_character() {
    assert_eq!(TokenKind::IDENT.to_string(), "IDENT");
    assert_eq!(TokenKind::OPEN_PAREN.to_string(), "'('");
    assert_eq!(TokenKind::COLON.to_string(), "':'");
    assert_eq!(TokenKind(0x80).to_string(), "TOKEN(128)");
  }

  #[test]
  fn value_flags_carry_the_reference_bits() {
    assert_eq!(ValueFlags::HERE_STRING.bits(), 0x1);
    assert_eq!(ValueFlags::FLOAT.bits(), 0x10);
    assert_eq!(ValueFlags::REQUIRES_FLOAT64.bits(), 0x10_0000);
    assert_eq!(ValueFlags::OVERFLOWED.bits(), 0x1000_0000);
    assert_eq!(
      (ValueFlags::NUMBER | ValueFlags::HEX).names(),
      ["NUMBER", "HEX"]
    );
  }
}

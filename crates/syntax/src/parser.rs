use oj_diag::{Diagnostic, SourceId, Span};
use oj_lexer::{Interner, Token, TokenKind, TokenValue, ValueFlags};

use crate::ast::*;

/// One parsed file: the arena, the file's top-level block and everything the
/// lexer and the parser had to say about it.
pub struct Parsed {
  pub ast: Ast,
  pub root: NodeId,
  pub diagnostics: Vec<Diagnostic>,
}

impl Parsed {
  pub fn has_errors(&self) -> bool {
    self.diagnostics.iter().any(Diagnostic::is_error)
  }
}

/// Lexes and parses one source. Errors are recorded rather than raised: the
/// parser recovers at the next `;` or `}` so that one bad statement does not
/// hide the rest of the file.
pub fn parse(input: &[u8], source: SourceId, interner: &Interner) -> Parsed {
  let lexed = oj_lexer::tokenize(input, source, interner);
  let mut parser = Parser {
    tokens: lexed.tokens,
    input,
    position: 0,
    ast: Ast::new(),
    source,
    interner,
    diagnostics: lexed.diagnostics,
    allow_switch: false,
    parameter_depth: 0,
  };
  let root = parser.parse_file();
  Parsed {
    ast: parser.ast,
    root,
    diagnostics: parser.diagnostics,
  }
}

struct Parser<'a> {
  tokens: Vec<Token>,
  input: &'a [u8],
  position: usize,
  ast: Ast,
  source: SourceId,
  interner: &'a Interner,
  diagnostics: Vec<Diagnostic>,
  /// Set while parsing an `if`/`#if` condition, where `== {` starts a switch
  /// (**L§6.4**) instead of continuing the expression.
  allow_switch: bool,
  /// Nesting depth of parameter lists: a procedure type written inside one
  /// ends its return list at the comma that separates parameters.
  parameter_depth: usize,
}

impl<'a> Parser<'a> {
  fn token(&self) -> &Token {
    &self.tokens[self.position.min(self.tokens.len() - 1)]
  }

  fn token_at(&self, ahead: usize) -> &Token {
    &self.tokens[(self.position + ahead).min(self.tokens.len() - 1)]
  }

  fn kind(&self) -> TokenKind {
    self.token().kind
  }

  fn kind_at(&self, ahead: usize) -> TokenKind {
    self.token_at(ahead).kind
  }

  fn at(&self, kind: TokenKind) -> bool {
    self.kind() == kind
  }

  fn at_end(&self) -> bool {
    self.at(TokenKind::END_OF_INPUT)
  }

  fn span(&self) -> Span {
    self.token().span
  }

  fn previous_end(&self) -> u32 {
    if self.position == 0 {
      0
    } else {
      self.tokens[self.position - 1].span.end
    }
  }

  fn bump(&mut self) -> Token {
    let token = self.tokens[self.position.min(self.tokens.len() - 1)].clone();
    if self.position < self.tokens.len() - 1 {
      self.position += 1;
    }
    token
  }

  fn eat(&mut self, kind: TokenKind) -> bool {
    if self.at(kind) {
      self.bump();
      true
    } else {
      false
    }
  }

  fn expect(&mut self, kind: TokenKind, what: &str) -> Option<Token> {
    if self.at(kind) {
      return Some(self.bump());
    }
    self.error_here(format!("Expected {what}, but got '{}'.", self.token_text()));
    None
  }

  fn token_text(&self) -> String {
    let span = self.span();
    if span.is_empty() || self.at_end() {
      return "end of file".to_string();
    }
    String::from_utf8_lossy(&self.input[span.range()]).into_owned()
  }

  fn error_here(&mut self, message: impl Into<String>) {
    let span = self.span();
    self.error(span, message);
  }

  fn error(&mut self, span: Span, message: impl Into<String>) {
    self
      .diagnostics
      .push(Diagnostic::error(self.source, span, message));
  }

  fn name_of(&self, token: &Token) -> &'static [u8] {
    match token.value {
      TokenValue::Name(symbol) => self.interner.resolve(symbol),
      _ => b"",
    }
  }

  /// The word after a `#`, if the parser is standing on a directive.
  fn directive(&self) -> Option<&'static [u8]> {
    if !self.at(TokenKind::HASH) {
      return None;
    }
    match self.token_at(1).value {
      TokenValue::Name(symbol) => Some(self.interner.resolve(symbol)),
      _ => None,
    }
  }

  fn at_directive(&self, name: &[u8]) -> bool {
    self.directive() == Some(name)
  }

  /// Consumes `#name`, which is always two tokens.
  fn eat_directive(&mut self, name: &[u8]) -> bool {
    if self.at_directive(name) {
      self.bump();
      self.bump();
      true
    } else {
      false
    }
  }

  /// True when the parser stands on `, word`, the shape every modifier list in
  /// the language uses (`cast,trunc`, `#import,file`, `>>,logical`, ...).
  fn at_modifier(&self, word: &[u8]) -> bool {
    self.at(TokenKind::COMMA) && self.name_of(self.token_at(1)) == word
  }

  fn eat_modifier(&mut self, word: &[u8]) -> bool {
    if self.at_modifier(word) {
      self.bump();
      self.bump();
      true
    } else {
      false
    }
  }

  fn push(&mut self, span: Span, data: NodeData) -> NodeId {
    self.ast.push(span, data)
  }

  fn span_from(&self, start: u32) -> Span {
    Span::new(start, self.previous_end().max(start))
  }

  fn ident_node(&mut self, token: &Token) -> NodeId {
    let mut flags = IdentFlags::empty();
    if token.backticked {
      flags |= IdentFlags::HAS_SCOPE_MODIFIER;
    }
    let name = match token.value {
      TokenValue::Name(symbol) => symbol,
      _ => self.interner.intern(b""),
    };
    self.push(token.span, NodeData::Ident(Ident { name, flags }))
  }

  fn string_value(&mut self, what: &str) -> Option<Box<[u8]>> {
    let token = self.expect(TokenKind::STRING, what)?;
    match token.value {
      TokenValue::Text(text) => Some(text),
      _ => Some(Box::default()),
    }
  }
}

// ---------------------------------------------------------------------------
// Files, blocks and statements
// ---------------------------------------------------------------------------

impl Parser<'_> {
  fn parse_file(&mut self) -> NodeId {
    let mut statements = Vec::new();
    while !self.at_end() {
      let before = self.position;
      match self.parse_statement_in(BlockType::DataDeclarations) {
        Some(statement) => {
          self.attach_trailing_notes(statement);
          statements.push(statement);
        }
        None => self.recover(),
      }
      if self.position == before {
        self.bump();
      }
    }

    let span = Span::new(0, self.input.len() as u32);
    self.push(
      span,
      NodeData::Block(Block {
        block_type: BlockType::DataDeclarations,
        block_flags: BlockFlags::empty(),
        statements,
      }),
    )
  }

  /// Skips to just past the next `;` or to a closing `}`, so that a broken
  /// statement costs one diagnostic instead of a cascade.
  fn recover(&mut self) {
    let mut depth = 0i32;
    while !self.at_end() {
      match self.kind() {
        TokenKind::OPEN_BRACE => depth += 1,
        TokenKind::CLOSE_BRACE => {
          if depth == 0 {
            return;
          }
          depth -= 1;
        }
        TokenKind::SEMICOLON if depth == 0 => {
          self.bump();
          return;
        }
        _ => {}
      }
      self.bump();
    }
  }

  fn parse_braced_block(&mut self, block_type: BlockType, flags: BlockFlags) -> Option<NodeId> {
    let start = self.span().start;
    self.expect(TokenKind::OPEN_BRACE, "'{'")?;
    let mut statements = Vec::new();
    while !self.at(TokenKind::CLOSE_BRACE) && !self.at_end() {
      let before = self.position;
      if self.eat(TokenKind::SEMICOLON) {
        continue;
      }
      match self.parse_statement_in(block_type) {
        Some(statement) => {
          self.attach_trailing_notes(statement);
          statements.push(statement);
        }
        None => self.recover(),
      }
      if self.position == before {
        self.bump();
      }
    }
    self.expect(TokenKind::CLOSE_BRACE, "'}'")?;

    let block = self.push(
      self.span_from(start),
      NodeData::Block(Block {
        block_type,
        block_flags: flags,
        statements,
      }),
    );
    self.ast.add_flags(block, NodeFlags::IS_PARENTHESIZED);
    Some(block)
  }

  /// A block written where a single statement is allowed: `if c x();` keeps the
  /// statement in an unbraced block, which is how the reference stores it.
  fn parse_statement_as_block(&mut self, block_type: BlockType) -> Option<NodeId> {
    if self.at(TokenKind::OPEN_BRACE) {
      return self.parse_braced_block(block_type, BlockFlags::empty());
    }
    if let Some(flags) = self.block_check_flags() {
      return self.parse_braced_block(block_type, flags);
    }

    let start = self.span().start;
    let statement = self.parse_statement_in(block_type)?;
    Some(self.push(
      self.span_from(start),
      NodeData::Block(Block {
        block_type,
        block_flags: BlockFlags::empty(),
        statements: vec![statement],
      }),
    ))
  }

  fn block_check_flags(&mut self) -> Option<BlockFlags> {
    let mut flags = BlockFlags::empty();
    loop {
      if self.at_directive(b"no_abc") {
        flags |= BlockFlags::NO_ARRAY_BOUNDS_CHECK;
      } else if self.at_directive(b"no_aoc") {
        flags |= BlockFlags::NO_ARITHMETIC_OVERFLOW_CHECK;
      } else {
        break;
      }
      self.bump();
      self.bump();
    }
    (!flags.is_empty()).then_some(flags)
  }

  fn attach_trailing_notes(&mut self, statement: NodeId) {
    while self.at(TokenKind::NOTE) {
      let token = self.bump();
      let name = match token.value {
        TokenValue::Name(symbol) => symbol,
        _ => self.interner.intern(b""),
      };
      let note = self.push(token.span, NodeData::Note { text: name });
      self.add_note(statement, note);
    }
  }

  fn add_note(&mut self, statement: NodeId, note: NodeId) {
    match &mut self.ast.node_mut(statement).data {
      NodeData::Declaration(declaration) => declaration.notes.push(note),
      NodeData::CompoundDeclaration(compound) => {
        let properties = compound.declaration_properties;
        match &mut self.ast.node_mut(properties).data {
          NodeData::Declaration(declaration) => declaration.notes.push(note),
          _ => unreachable!("compound declarations hold a declaration"),
        }
      }
      NodeData::Struct(structure) => structure.notes.push(note),
      NodeData::Enum(enumeration) => enumeration.notes.push(note),
      NodeData::ProcedureHeader(header) => header.notes.push(note),
      _ => {}
    }
  }

  fn finish_statement(&mut self, statement: NodeId) {
    if !crate::rules::semicolon_is_optional(&self.ast, statement) {
      if !self.eat(TokenKind::SEMICOLON) {
        self.error_here(format!("Expected ';', but got '{}'.", self.token_text()));
      }
    } else {
      self.eat(TokenKind::SEMICOLON);
    }
  }

  fn parse_statement_in(&mut self, block_type: BlockType) -> Option<NodeId> {
    let statement = self.parse_statement(block_type)?;
    self.attach_trailing_notes(statement);
    self.finish_statement(statement);
    Some(statement)
  }

  fn parse_statement(&mut self, block_type: BlockType) -> Option<NodeId> {
    if block_type == BlockType::Constants {
      return self.parse_enum_member();
    }

    if let Some(flags) = self.block_check_flags() {
      return self.parse_braced_block(BlockType::Imperative, flags);
    }

    match self.kind() {
      TokenKind::OPEN_BRACE => self.parse_braced_block(BlockType::Imperative, BlockFlags::empty()),
      TokenKind::KEYWORD_IF => self.parse_if(false),
      TokenKind::KEYWORD_WHILE => self.parse_while(),
      TokenKind::KEYWORD_FOR => self.parse_for(),
      TokenKind::KEYWORD_RETURN => self.parse_return(),
      TokenKind::KEYWORD_BREAK => self.parse_loop_control(LoopControlType::Break),
      TokenKind::KEYWORD_CONTINUE => self.parse_loop_control(LoopControlType::Continue),
      TokenKind::KEYWORD_REMOVE => self.parse_loop_control(LoopControlType::Remove),
      TokenKind::KEYWORD_DEFER => self.parse_defer(),
      TokenKind::KEYWORD_USING => self.parse_using(),
      TokenKind::KEYWORD_PUSH_CONTEXT => self.parse_push_context(),
      TokenKind::KEYWORD_OPERATOR => self.parse_declaration_statement(),
      TokenKind::HASH => self.parse_directive_statement(block_type),
      _ => self.parse_declaration_statement(),
    }
  }
}

// ---------------------------------------------------------------------------
// Control flow
// ---------------------------------------------------------------------------

impl Parser<'_> {
  fn parse_if(&mut self, is_static: bool) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();

    let mut flags = IfFlags::empty();
    if is_static {
      flags |= IfFlags::IS_STATIC;
    }
    if self.eat_directive(b"complete") {
      flags |= IfFlags::MARKED_AS_COMPLETE;
    }

    let condition = self.parse_condition()?;

    if self.at(TokenKind::ISEQUAL) && self.kind_at(1) == TokenKind::OPEN_BRACE {
      self.bump();
      flags |= IfFlags::IS_SWITCH_STATEMENT;
      let then_block = self.parse_switch_body()?;
      return Some(self.push(
        self.span_from(start),
        NodeData::If(Box::new(IfNode {
          condition,
          then_block: Some(then_block),
          else_block: None,
          if_flags: flags,
        })),
      ));
    }

    self.eat(TokenKind::KEYWORD_THEN);
    if self.at(TokenKind::SEMICOLON) {
      self.error_here("An 'if' statement cannot be followed directly by ';'.");
      return None;
    }

    let then_block = self.parse_statement_as_block(BlockType::Imperative)?;
    let else_block = if self.eat(TokenKind::KEYWORD_ELSE) {
      Some(self.parse_else_block(is_static)?)
    } else {
      None
    };

    Some(self.push(
      self.span_from(start),
      NodeData::If(Box::new(IfNode {
        condition,
        then_block: Some(then_block),
        else_block,
        if_flags: flags,
      })),
    ))
  }

  fn parse_else_block(&mut self, is_static: bool) -> Option<NodeId> {
    if is_static && self.at(TokenKind::HASH) && self.kind_at(1) == TokenKind::KEYWORD_IF {
      let start = self.span().start;
      self.bump();
      let inner = self.parse_if(true)?;
      return Some(self.push(
        self.span_from(start),
        NodeData::Block(Block {
          block_type: BlockType::Imperative,
          block_flags: BlockFlags::empty(),
          statements: vec![inner],
        }),
      ));
    }
    self.parse_statement_as_block(BlockType::Imperative)
  }

  fn parse_condition(&mut self) -> Option<NodeId> {
    let was = std::mem::replace(&mut self.allow_switch, true);
    let condition = self.parse_expression();
    self.allow_switch = was;
    condition
  }

  fn parse_switch_body(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    self.expect(TokenKind::OPEN_BRACE, "'{'")?;

    let mut cases = Vec::new();
    while !self.at(TokenKind::CLOSE_BRACE) && !self.at_end() {
      if self.eat(TokenKind::SEMICOLON) {
        continue;
      }
      let before = self.position;
      match self.parse_case() {
        Some(case) => cases.push(case),
        None => self.recover(),
      }
      if self.position == before {
        self.bump();
      }
    }
    self.expect(TokenKind::CLOSE_BRACE, "'}'")?;

    let block = self.push(
      self.span_from(start),
      NodeData::Block(Block {
        block_type: BlockType::Imperative,
        block_flags: BlockFlags::empty(),
        statements: cases,
      }),
    );
    self.ast.add_flags(block, NodeFlags::IS_PARENTHESIZED);
    Some(block)
  }

  fn parse_case(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    self.expect(TokenKind::KEYWORD_CASE, "'case'")?;

    let condition = if self.at(TokenKind::SEMICOLON) {
      None
    } else {
      Some(self.parse_expression()?)
    };
    self.expect(TokenKind::SEMICOLON, "';' after a case value")?;

    let body_start = self.span().start;
    let mut statements = Vec::new();
    let mut fallthrough = false;
    while !self.at(TokenKind::CLOSE_BRACE) && !self.at(TokenKind::KEYWORD_CASE) && !self.at_end() {
      if self.eat(TokenKind::SEMICOLON) {
        continue;
      }
      if self.at_directive(b"through") {
        self.bump();
        self.bump();
        self.eat(TokenKind::SEMICOLON);
        fallthrough = true;
        continue;
      }
      let before = self.position;
      match self.parse_statement_in(BlockType::Imperative) {
        Some(statement) => statements.push(statement),
        None => self.recover(),
      }
      if self.position == before {
        self.bump();
      }
    }

    let block = self.push(
      self.span_from(body_start),
      NodeData::Block(Block {
        block_type: BlockType::Imperative,
        block_flags: BlockFlags::empty(),
        statements,
      }),
    );

    Some(self.push(
      self.span_from(start),
      NodeData::Case(Box::new(CaseNode {
        condition,
        then_block: block,
        marked_as_fallthrough: fallthrough,
      })),
    ))
  }

  fn parse_while(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();

    let condition = if self.at(TokenKind::IDENT) && self.kind_at(1) == TokenKind::COLON {
      self.parse_single_declaration()?
    } else {
      self.parse_expression()?
    };

    let block = self.parse_statement_as_block(BlockType::Imperative)?;
    Some(self.push(self.span_from(start), NodeData::While { condition, block }))
  }

  fn parse_for(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();

    let mut for_flags = ForFlags::empty();
    let mut want_replacement = None;
    let mut want_pointer = None;
    let mut want_reverse = None;

    if self.eat(TokenKind::COLON) {
      let token = self.expect(TokenKind::IDENT, "a for_expansion name")?;
      want_replacement = Some(self.ident_node(&token));
    }
    if self.at_directive(b"v2") {
      self.bump();
      self.bump();
      for_flags |= ForFlags::TEMPORARY_V2;
    }

    loop {
      if self.eat(TokenKind::LESS_THAN) {
        for_flags |= ForFlags::REVERSE;
      } else if self.eat(TokenKind::LESSEQUALS) {
        want_reverse = Some(self.parse_unary()?);
      } else if self.eat(TokenKind::ASTERISK) {
        for_flags |= ForFlags::POINTER;
      } else if self.eat(TokenKind::TIMESEQUALS) {
        want_pointer = Some(self.parse_unary()?);
      } else {
        break;
      }
      self.eat(TokenKind::COMMA);
    }

    let (ident_it, ident_it_index) = self.parse_for_names();

    let iteration_expression = self.parse_expression()?;
    let iteration_expression_right = if self.eat(TokenKind::DOUBLE_DOT) {
      Some(self.parse_expression()?)
    } else {
      None
    };

    let block = self.parse_statement_as_block(BlockType::Imperative)?;

    Some(self.push(
      self.span_from(start),
      NodeData::For(Box::new(ForNode {
        iteration_expression,
        iteration_expression_right,
        block,
        ident_it,
        ident_it_index,
        for_flags,
        want_replacement_for_expansion: want_replacement,
        want_pointer_expression: want_pointer,
        want_reverse_expression: want_reverse,
      })),
    ))
  }

  /// `for name: expr` and `for name, index: expr` name the iterator variables;
  /// anything else is the iteration expression itself.
  fn parse_for_names(&mut self) -> (Option<NodeId>, Option<NodeId>) {
    let names_follow = self.at(TokenKind::IDENT)
      && (self.kind_at(1) == TokenKind::COLON
        || (self.kind_at(1) == TokenKind::COMMA
          && self.kind_at(2) == TokenKind::IDENT
          && self.kind_at(3) == TokenKind::COLON));
    if !names_follow {
      return (None, None);
    }

    let token = self.bump();
    let it = self.ident_node(&token);
    let mut it_index = None;
    if self.eat(TokenKind::COMMA) {
      let token = self.bump();
      it_index = Some(self.ident_node(&token));
    }
    self.eat(TokenKind::COLON);
    (Some(it), it_index)
  }

  fn parse_return(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let token = self.bump();
    let mut flags = ReturnFlags::empty();
    if token.backticked {
      flags |= ReturnFlags::IS_BACKTICKED;
    }

    let mut arguments = Vec::new();
    if !self.at(TokenKind::SEMICOLON) && !self.at(TokenKind::CLOSE_BRACE) && !self.at_end() {
      arguments = self.parse_argument_list(TokenKind::SEMICOLON)?;
    }

    Some(self.push(self.span_from(start), NodeData::Return { arguments, flags }))
  }

  fn parse_loop_control(&mut self, control_type: LoopControlType) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();
    let target_ident = if self.at(TokenKind::IDENT) {
      let token = self.bump();
      Some(self.ident_node(&token))
    } else {
      None
    };
    Some(self.push(
      self.span_from(start),
      NodeData::LoopControl {
        control_type,
        target_ident,
      },
    ))
  }

  fn parse_defer(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let token = self.bump();
    let block = self.parse_statement_as_block(BlockType::Imperative)?;
    Some(self.push(
      self.span_from(start),
      NodeData::Defer {
        block,
        is_backticked: token.backticked,
      },
    ))
  }

  fn parse_push_context(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let token = self.bump();
    let mut flags = PushContextFlags::empty();
    if token.backticked {
      flags |= PushContextFlags::IS_BACKTICKED;
    }
    if self.eat_modifier(b"defer_pop") {
      flags |= PushContextFlags::DEFER_POP;
    }

    let to_push = if self.at(TokenKind::OPEN_BRACE) || self.at(TokenKind::SEMICOLON) {
      None
    } else {
      Some(self.parse_expression()?)
    };

    let block = if self.at(TokenKind::OPEN_BRACE) {
      Some(self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())?)
    } else {
      None
    };

    Some(self.push(
      self.span_from(start),
      NodeData::PushContext {
        to_push,
        block,
        flags,
      },
    ))
  }

  fn parse_using(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();

    let mut filter_type = FilterType::None;
    let mut filter_expression = None;
    let mut no_parameters = false;
    loop {
      let filter = if self.at_modifier(b"only") {
        FilterType::Only
      } else if self.at_modifier(b"except") {
        FilterType::Except
      } else if self.at_modifier(b"map") {
        FilterType::Map
      } else if self.eat_modifier(b"no_parameters") {
        no_parameters = true;
        continue;
      } else {
        break;
      };
      self.bump();
      self.bump();
      filter_type = filter;
      filter_expression = Some(self.parse_filter_expression()?);
    }

    let expression = self.parse_using_target()?;
    Some(self.push(
      self.span_from(start),
      NodeData::Using(Box::new(Using {
        expression,
        filter_type,
        filter_expression,
        no_parameters,
      })),
    ))
  }

  fn parse_using_target(&mut self) -> Option<NodeId> {
    let marked_as_as = self.at_directive(b"as");
    if marked_as_as {
      self.bump();
      self.bump();
    }

    if self.at(TokenKind::IDENT) && self.kind_at(1) == TokenKind::COLON {
      let declaration = self.parse_single_declaration()?;
      if marked_as_as {
        self.mark_as(declaration);
      }
      return Some(declaration);
    }

    let expression = self.parse_expression()?;
    if marked_as_as {
      self.mark_as(expression);
    }
    Some(expression)
  }

  fn mark_as(&mut self, declaration: NodeId) {
    if let NodeData::Declaration(inner) = &mut self.ast.node_mut(declaration).data {
      inner.flags |= DeclarationFlags::IS_MARKED_AS_AS;
    }
  }
}

// ---------------------------------------------------------------------------
// Declarations and assignments
// ---------------------------------------------------------------------------

impl Parser<'_> {
  /// The statement form that starts with an expression: a declaration, an
  /// assignment (possibly compound) or a bare expression (**L§4.5**, **L§6.1**).
  fn parse_declaration_statement(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let mut elements = Vec::new();

    loop {
      let node = self.parse_declaration_target()?;
      let modifier = if self.at(TokenKind::EQUALS)
        && matches!(self.kind_at(1), TokenKind::COMMA | TokenKind::COLON)
      {
        self.bump();
        CommaModifier::Assign
      } else if self.at(TokenKind::COLON) && self.kind_at(1) == TokenKind::COMMA {
        self.bump();
        CommaModifier::Declare
      } else {
        CommaModifier::None
      };
      elements.push(CommaArgument { node, modifier });

      if !self.eat(TokenKind::COMMA) {
        break;
      }
    }

    let compound = elements.len() > 1
      || elements
        .iter()
        .any(|element| element.modifier != CommaModifier::None);

    if self.at(TokenKind::COLON) {
      return self.parse_declaration_tail(start, elements, compound);
    }

    if let Some(operator) = self.assignment_operator() {
      self.bump();
      let value = self.parse_assignment_value(start)?;
      if compound {
        return Some(self.finish_compound(start, elements, Some(operator), None, value));
      }
      let left = elements[0].node;
      return Some(self.push(
        self.span_from(start),
        NodeData::BinaryOperator {
          operator,
          flags: BinaryFlags::empty(),
          left,
          right: value,
        },
      ));
    }

    if compound {
      self.error_here(format!(
        "Expected ':' or an assignment operator, but got '{}'.",
        self.token_text()
      ));
      return None;
    }

    Some(elements[0].node)
  }

  /// One element of a declaration's name list. `operator +` declares an
  /// operator overload (**L§7.7**), whose name is the operator's text.
  fn parse_declaration_target(&mut self) -> Option<NodeId> {
    if self.at(TokenKind::KEYWORD_OPERATOR) {
      return self.parse_operator_name();
    }
    self.parse_expression()
  }

  fn parse_operator_name(&mut self) -> Option<NodeId> {
    let keyword = self.bump();
    let start = self.span().start;
    let mut end = start;
    while !matches!(
      self.kind(),
      TokenKind::COLON
        | TokenKind::SEMICOLON
        | TokenKind::COMMA
        | TokenKind::CLOSE_PAREN
        | TokenKind::CLOSE_BRACE
        | TokenKind::END_OF_INPUT
    ) {
      end = self.span().end;
      self.bump();
    }
    if end == start {
      self.error(keyword.span, "Expected an operator after 'operator'.");
      return None;
    }

    let text: Vec<u8> = self.input[start as usize..end as usize]
      .iter()
      .copied()
      .filter(|byte| !byte.is_ascii_whitespace())
      .collect();
    let name = self.interner.intern(&text);
    let mut flags = IdentFlags::empty();
    if keyword.backticked {
      flags |= IdentFlags::HAS_SCOPE_MODIFIER;
    }
    Some(self.push(
      Span::new(keyword.span.start, end),
      NodeData::Ident(Ident { name, flags }),
    ))
  }

  fn assignment_operator(&self) -> Option<OperatorType> {
    Some(match self.kind() {
      TokenKind::EQUALS => OperatorType::ASSIGN,
      TokenKind::PLUSEQUALS => OperatorType::PLUS_ASSIGN,
      TokenKind::MINUSEQUALS => OperatorType::MINUS_ASSIGN,
      TokenKind::TIMESEQUALS => OperatorType::TIMES_ASSIGN,
      TokenKind::DIVEQUALS => OperatorType::DIV_ASSIGN,
      TokenKind::MODEQUALS => OperatorType::MOD_ASSIGN,
      TokenKind::SHIFT_LEFT_EQUALS => OperatorType::SHIFT_LEFT_ASSIGN,
      TokenKind::SHIFT_RIGHT_EQUALS => OperatorType::SHIFT_RIGHT_ASSIGN,
      TokenKind::ROTATE_LEFT_EQUALS => OperatorType::ROTATE_LEFT_ASSIGN,
      TokenKind::ROTATE_RIGHT_EQUALS => OperatorType::ROTATE_RIGHT_ASSIGN,
      TokenKind::BITWISE_AND_EQUALS => OperatorType::BITWISE_AND_ASSIGN,
      TokenKind::BITWISE_OR_EQUALS => OperatorType::BITWISE_OR_ASSIGN,
      TokenKind::BITWISE_XOR_EQUALS => OperatorType::BITWISE_XOR_ASSIGN,
      TokenKind::LOGICAL_AND_EQUALS => OperatorType::LOGICAL_AND_ASSIGN,
      TokenKind::LOGICAL_OR_EQUALS => OperatorType::LOGICAL_OR_ASSIGN,
      _ => return None,
    })
  }

  /// The values on the right of a declaration or assignment: one expression, or
  /// a comma-separated list which becomes `COMMA_SEPARATED_ARGUMENTS`.
  fn parse_assignment_value(&mut self, start: u32) -> Option<NodeId> {
    let first = self.parse_expression()?;
    if !self.at(TokenKind::COMMA) {
      return Some(first);
    }

    let mut arguments = vec![CommaArgument {
      node: first,
      modifier: CommaModifier::None,
    }];
    while self.eat(TokenKind::COMMA) {
      if self.at(TokenKind::SEMICOLON) || self.at(TokenKind::CLOSE_BRACE) {
        break;
      }
      let node = self.parse_expression()?;
      arguments.push(CommaArgument {
        node,
        modifier: CommaModifier::None,
      });
    }
    Some(self.push(
      self.span_from(start),
      NodeData::CommaSeparatedArguments { arguments },
    ))
  }

  fn parse_declaration_tail(
    &mut self,
    start: u32,
    elements: Vec<CommaArgument>,
    compound: bool,
  ) -> Option<NodeId> {
    self.expect(TokenKind::COLON, "':'")?;

    let mut type_inst = None;
    let mut is_constant = false;
    let mut has_value = false;
    let mut alignment = None;
    let mut elsewhere_library = None;
    let mut elsewhere_symbol = None;
    let mut is_elsewhere = false;

    if self.eat(TokenKind::COLON) {
      is_constant = true;
      has_value = true;
    } else if self.eat(TokenKind::EQUALS) {
      has_value = true;
    } else {
      type_inst = Some(self.parse_type()?);

      // Directives written after a member's type: `x: u8 #align 64;` and
      // `v: s32 #elsewhere libc "environ";` (**L§4.8**).
      loop {
        if self.at_directive(b"align") {
          self.bump();
          self.bump();
          alignment = Some(self.parse_unary()?);
        } else if self.at_directive(b"elsewhere") {
          self.bump();
          self.bump();
          is_elsewhere = true;
          if self.at(TokenKind::IDENT) {
            let token = self.bump();
            elsewhere_library = Some(self.ident_node(&token));
          }
          if self.at(TokenKind::STRING) {
            elsewhere_symbol = self.string_value("a symbol name");
          }
        } else {
          break;
        }
      }

      if self.eat(TokenKind::COLON) {
        is_constant = true;
        has_value = true;
      } else if self.eat(TokenKind::EQUALS) {
        has_value = true;
      }
    }

    let mut uninitialized = false;
    let mut expression = None;
    if has_value {
      if self.at(TokenKind::TRIPLE_MINUS) {
        self.bump();
        uninitialized = true;
      } else {
        expression = Some(self.parse_assignment_value(start)?);
      }
    }

    while self.at_directive(b"align") {
      self.bump();
      self.bump();
      alignment = Some(self.parse_unary()?);
    }

    let mut flags = DeclarationFlags::empty();
    if is_constant {
      flags |= DeclarationFlags::IS_CONSTANT;
    }
    if uninitialized {
      flags |= DeclarationFlags::IS_UNINITIALIZED;
    }
    if is_elsewhere {
      flags |= DeclarationFlags::ELSEWHERE;
    }

    if compound {
      let value = expression.unwrap_or_else(|| {
        self.push(
          self.span_from(start),
          NodeData::CommaSeparatedArguments {
            arguments: Vec::new(),
          },
        )
      });
      let properties = self.make_declaration(start, None, type_inst, Some(value), flags);
      if expression.is_none()
        && let NodeData::Declaration(declaration) = &mut self.ast.node_mut(properties).data
      {
        declaration.expression = None;
      }
      self.apply_declaration_directives(properties, alignment, elsewhere_library, elsewhere_symbol);
      return Some(self.finish_compound_with(start, elements, None, properties));
    }

    let name = elements[0].node;
    let declaration = self.make_declaration(start, Some(name), type_inst, expression, flags);
    self.apply_declaration_directives(declaration, alignment, elsewhere_library, elsewhere_symbol);
    Some(declaration)
  }

  fn apply_declaration_directives(
    &mut self,
    id: NodeId,
    alignment: Option<NodeId>,
    elsewhere_library: Option<NodeId>,
    elsewhere_symbol: Option<Box<[u8]>>,
  ) {
    if let NodeData::Declaration(declaration) = &mut self.ast.node_mut(id).data {
      if alignment.is_some() {
        declaration.alignment_expression = alignment;
      }
      declaration.elsewhere_library = elsewhere_library;
      declaration.elsewhere_symbol = elsewhere_symbol;
    }
  }

  fn make_declaration(
    &mut self,
    start: u32,
    name: Option<NodeId>,
    type_inst: Option<NodeId>,
    expression: Option<NodeId>,
    flags: DeclarationFlags,
  ) -> NodeId {
    let mut declaration = Declaration::empty();
    declaration.name = name;
    declaration.type_inst = type_inst;
    declaration.expression = expression;
    declaration.flags = flags;
    if let Some(name) = name
      && let NodeData::Ident(ident) = self.ast.data(name)
      && ident.flags.contains(IdentFlags::HAS_SCOPE_MODIFIER)
    {
      declaration.flags |= DeclarationFlags::HAS_SCOPE_MODIFIER;
    }
    self.push(
      self.span_from(start),
      NodeData::Declaration(Box::new(declaration)),
    )
  }

  fn finish_compound(
    &mut self,
    start: u32,
    elements: Vec<CommaArgument>,
    operator: Option<OperatorType>,
    type_inst: Option<NodeId>,
    value: NodeId,
  ) -> NodeId {
    let properties = self.make_declaration(
      start,
      None,
      type_inst,
      Some(value),
      DeclarationFlags::empty(),
    );
    self.finish_compound_with(start, elements, operator, properties)
  }

  fn finish_compound_with(
    &mut self,
    start: u32,
    elements: Vec<CommaArgument>,
    operator: Option<OperatorType>,
    properties: NodeId,
  ) -> NodeId {
    let list = self.push(
      self.span_from(start),
      NodeData::CommaSeparatedArguments {
        arguments: elements,
      },
    );
    self.push(
      self.span_from(start),
      NodeData::CompoundDeclaration(Box::new(CompoundDeclaration {
        comma_separated_assignment: list,
        declaration_properties: properties,
        operator_type: operator,
      })),
    )
  }

  /// `name: T = value` in the places that allow exactly one name: `while`
  /// conditions, `using` targets and `#add_context`.
  fn parse_single_declaration(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let token = self.expect(TokenKind::IDENT, "a name")?;
    let name = self.ident_node(&token);
    self.parse_declaration_tail(
      start,
      vec![CommaArgument {
        node: name,
        modifier: CommaModifier::None,
      }],
      false,
    )
  }

  fn parse_enum_member(&mut self) -> Option<NodeId> {
    if self.at(TokenKind::HASH) || self.at(TokenKind::KEYWORD_USING) {
      return self.parse_statement(BlockType::Imperative);
    }

    let start = self.span().start;
    let token = self.expect(TokenKind::IDENT, "an enum member name")?;
    let name = self.ident_node(&token);

    if self.at(TokenKind::COLON) {
      return self.parse_declaration_tail(
        start,
        vec![CommaArgument {
          node: name,
          modifier: CommaModifier::None,
        }],
        false,
      );
    }

    Some(self.make_declaration(start, Some(name), None, None, DeclarationFlags::IS_CONSTANT))
  }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

impl Parser<'_> {
  /// A type slot always produces a `TYPE_INSTANTIATION` (**C§5.3**), whose
  /// shape says whether it is a pointer, an array, a `#type` directive or an
  /// arbitrary type-valued expression.
  fn parse_type(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let mut inst = TypeInstantiation::empty();

    if self.at(TokenKind::ASTERISK) {
      self.bump();
      inst.pointer_to = Some(self.parse_type()?);
      return Some(self.push(
        self.span_from(start),
        NodeData::TypeInstantiation(Box::new(inst)),
      ));
    }

    if self.at(TokenKind::DOUBLE_DOT) {
      self.bump();
      inst.inst_flags |= InstFlags::VARARGS;
      inst.array_element_type = Some(self.parse_type()?);
      return Some(self.push(
        self.span_from(start),
        NodeData::TypeInstantiation(Box::new(inst)),
      ));
    }

    if self.at(TokenKind::OPEN_BRACKET) {
      self.bump();
      if self.eat(TokenKind::CLOSE_BRACKET) {
        inst.inst_flags |= InstFlags::ARRAY_VIEW;
      } else if self.at(TokenKind::DOUBLE_DOT) {
        self.bump();
        self.expect(TokenKind::CLOSE_BRACKET, "']'")?;
        inst.inst_flags |= InstFlags::RESIZABLE;
      } else {
        inst.array_dimension = Some(self.parse_expression()?);
        self.expect(TokenKind::CLOSE_BRACKET, "']'")?;
      }
      inst.array_element_type = Some(self.parse_type()?);
      return Some(self.push(
        self.span_from(start),
        NodeData::TypeInstantiation(Box::new(inst)),
      ));
    }

    if self.at_directive(b"type") {
      self.bump();
      self.bump();
      inst.inst_flags |= InstFlags::TYPE_DIRECTIVE;
      if self.eat_modifier(b"distinct") {
        inst.inst_flags |= InstFlags::TYPE_DIRECTIVE_DISTINCT;
      } else if self.eat_modifier(b"isa") {
        inst.inst_flags |= InstFlags::TYPE_DIRECTIVE_ISA;
      }
      inst.type_directive_target = Some(self.parse_type()?);
      return Some(self.push(
        self.span_from(start),
        NodeData::TypeInstantiation(Box::new(inst)),
      ));
    }

    // `$T/Base` and `$T/interface I` (**L§7.8**): the `/` is a restriction, not
    // a division, so the polymorph variable is read before the binary loop.
    if self.at(TokenKind::DOLLAR) && self.kind_at(1) == TokenKind::IDENT {
      let variable = self.parse_primary()?;
      inst.type_valued_expression = Some(variable);
      if self.eat(TokenKind::SLASH) {
        if self.eat(TokenKind::KEYWORD_INTERFACE) {
          inst.inst_flags |= InstFlags::INTERFACE;
        }
        inst.must_implement = Some(self.parse_unary()?);
      }
      return Some(self.push(
        self.span_from(start),
        NodeData::TypeInstantiation(Box::new(inst)),
      ));
    }

    // In a type slot a `(` opens a procedure type whether or not its parameters
    // are named and whether or not it returns anything: `sa_handler: (sig: s32)
    // #c_call;` and `f: (T)` (**L§3.7**).
    let expression = match self.at(TokenKind::OPEN_PAREN) {
      true => self.parse_procedure(start)?,
      false => self.parse_expression()?,
    };
    inst.type_valued_expression = Some(expression);

    Some(self.push(
      self.span_from(start),
      NodeData::TypeInstantiation(Box::new(inst)),
    ))
  }

  /// Wraps an already-parsed expression as a type instantiation, for the
  /// literal forms (`T.[...]`, `T.{...}`) where the type comes first.
  fn wrap_type(&mut self, expression: NodeId) -> NodeId {
    let mut inst = TypeInstantiation::empty();
    inst.type_valued_expression = Some(expression);
    let span = self.ast.node(expression).span;
    self.push(span, NodeData::TypeInstantiation(Box::new(inst)))
  }
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

const PRECEDENCE_LOGICAL_OR: u8 = 1;
const PRECEDENCE_LOGICAL_AND: u8 = 2;
const PRECEDENCE_COMPARISON: u8 = 3;
const PRECEDENCE_BITWISE_OR: u8 = 4;
const PRECEDENCE_BITWISE_XOR: u8 = 5;
const PRECEDENCE_BITWISE_AND: u8 = 6;
const PRECEDENCE_SHIFT: u8 = 7;
const PRECEDENCE_ADDITIVE: u8 = 8;
const PRECEDENCE_MULTIPLICATIVE: u8 = 9;

impl Parser<'_> {
  fn parse_expression(&mut self) -> Option<NodeId> {
    if self.at(TokenKind::KEYWORD_IFX) {
      return self.parse_ifx(false);
    }
    if self.at(TokenKind::HASH) && self.kind_at(1) == TokenKind::KEYWORD_IFX {
      self.bump();
      return self.parse_ifx(true);
    }
    self.parse_binary(PRECEDENCE_LOGICAL_OR)
  }

  fn parse_ifx(&mut self, is_static: bool) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();

    let mut flags = IfFlags::IS_IFX;
    if is_static {
      flags |= IfFlags::IS_STATIC;
    }

    let condition = self.parse_binary(PRECEDENCE_LOGICAL_OR)?;
    // `ifx c` with neither branch takes its value from the condition, and
    // `ifx c else b` has no then-branch (**L§5.13**).
    let then_block = if self.at(TokenKind::KEYWORD_ELSE) || self.ends_an_expression() {
      None
    } else {
      self.eat(TokenKind::KEYWORD_THEN);
      Some(self.parse_branch_value()?)
    };
    let else_block = if self.eat(TokenKind::KEYWORD_ELSE) {
      Some(self.parse_branch_value()?)
    } else {
      None
    };

    Some(self.push(
      self.span_from(start),
      NodeData::If(Box::new(IfNode {
        condition,
        then_block,
        else_block,
        if_flags: flags,
      })),
    ))
  }

  fn ends_an_expression(&self) -> bool {
    matches!(
      self.kind(),
      TokenKind::SEMICOLON
        | TokenKind::COMMA
        | TokenKind::CLOSE_PAREN
        | TokenKind::CLOSE_BRACKET
        | TokenKind::CLOSE_BRACE
        | TokenKind::END_OF_INPUT
    )
  }

  /// An `ifx` branch is either a block or a single expression; both are stored
  /// as blocks, the way `Code_If` wants them.
  fn parse_branch_value(&mut self) -> Option<NodeId> {
    if self.at(TokenKind::OPEN_BRACE) {
      return self.parse_braced_block(BlockType::Imperative, BlockFlags::empty());
    }
    let start = self.span().start;
    let expression = self.parse_expression()?;
    Some(self.push(
      self.span_from(start),
      NodeData::Block(Block {
        block_type: BlockType::Imperative,
        block_flags: BlockFlags::empty(),
        statements: vec![expression],
      }),
    ))
  }

  fn binary_operator(&self) -> Option<(u8, OperatorType)> {
    Some(match self.kind() {
      TokenKind::ASTERISK => (PRECEDENCE_MULTIPLICATIVE, OperatorType::TIMES),
      TokenKind::SLASH => (PRECEDENCE_MULTIPLICATIVE, OperatorType::DIVIDE),
      TokenKind::PERCENT => (PRECEDENCE_MULTIPLICATIVE, OperatorType::MODULUS),
      TokenKind::PLUS => (PRECEDENCE_ADDITIVE, OperatorType::PLUS),
      TokenKind::MINUS => (PRECEDENCE_ADDITIVE, OperatorType::MINUS),
      TokenKind::POINTER_DEREFERENCE_OR_SHIFT_LEFT => (PRECEDENCE_SHIFT, OperatorType::SHIFT_LEFT),
      TokenKind::SHIFT_RIGHT => (PRECEDENCE_SHIFT, OperatorType::SHIFT_RIGHT),
      TokenKind::ROTATE_LEFT => (PRECEDENCE_SHIFT, OperatorType::ROTATE_LEFT),
      TokenKind::ROTATE_RIGHT => (PRECEDENCE_SHIFT, OperatorType::ROTATE_RIGHT),
      TokenKind::BITWISE_AND => (PRECEDENCE_BITWISE_AND, OperatorType::BITWISE_AND),
      TokenKind::BITWISE_XOR => (PRECEDENCE_BITWISE_XOR, OperatorType::BITWISE_XOR),
      TokenKind::BITWISE_OR => (PRECEDENCE_BITWISE_OR, OperatorType::BITWISE_OR),
      TokenKind::ISEQUAL => (PRECEDENCE_COMPARISON, OperatorType::IS_EQUAL),
      TokenKind::ISNOTEQUAL => (PRECEDENCE_COMPARISON, OperatorType::IS_NOT_EQUAL),
      TokenKind::LESS_THAN => (PRECEDENCE_COMPARISON, OperatorType::LESS),
      TokenKind::LESSEQUALS => (PRECEDENCE_COMPARISON, OperatorType::LESS_OR_EQUAL),
      TokenKind::GREATER_THAN => (PRECEDENCE_COMPARISON, OperatorType::GREATER),
      TokenKind::GREATEREQUALS => (PRECEDENCE_COMPARISON, OperatorType::GREATER_OR_EQUAL),
      TokenKind::LOGICAL_AND => (PRECEDENCE_LOGICAL_AND, OperatorType::LOGICAL_AND),
      TokenKind::LOGICAL_OR => (PRECEDENCE_LOGICAL_OR, OperatorType::LOGICAL_OR),
      _ => return None,
    })
  }

  fn parse_binary(&mut self, minimum: u8) -> Option<NodeId> {
    let start = self.span().start;
    let mut left = self.parse_unary()?;

    while let Some((precedence, operator)) = self.binary_operator() {
      if precedence < minimum {
        break;
      }
      if self.allow_switch
        && operator == OperatorType::IS_EQUAL
        && self.kind_at(1) == TokenKind::OPEN_BRACE
      {
        break;
      }

      self.bump();
      let mut flags = BinaryFlags::empty();
      if precedence == PRECEDENCE_SHIFT {
        if self.eat_modifier(b"logical") {
          flags |= BinaryFlags::SHIFT_MARKED_AS_LOGICAL;
        }
        if self.eat_modifier(b"small") {
          flags |= BinaryFlags::SHIFT_MARKED_AS_SMALL;
        }
      }

      let was = std::mem::replace(&mut self.allow_switch, false);
      let right = self.parse_binary(precedence + 1);
      self.allow_switch = was;
      let right = right?;

      left = self.push(
        self.span_from(start),
        NodeData::BinaryOperator {
          operator,
          flags,
          left,
          right,
        },
      );
    }

    Some(left)
  }

  /// Moves a `*` written before a designated array or struct literal into the
  /// literal's type, so that `*u8.[a, b]` is a `[2] *u8` (**L§5.8**). Returns
  /// the literal when it applies.
  fn point_literal_type_at(&mut self, operand: NodeId, start: u32) -> Option<NodeId> {
    let designation = match &self.ast.node(operand).data {
      NodeData::Literal(literal) => match &literal.value {
        LiteralValue::Array(array) => array.element_type,
        LiteralValue::Struct(structure) => structure.type_expression,
        _ => None,
      },
      _ => None,
    }?;

    let mut inst = TypeInstantiation::empty();
    inst.pointer_to = Some(designation);
    let pointer = self.push(
      self.span_from(start),
      NodeData::TypeInstantiation(Box::new(inst)),
    );
    if let NodeData::Literal(literal) = &mut self.ast.node_mut(operand).data {
      match &mut literal.value {
        LiteralValue::Array(array) => array.element_type = Some(pointer),
        LiteralValue::Struct(structure) => structure.type_expression = Some(pointer),
        _ => {}
      }
    }
    self.ast.node_mut(operand).span = self.span_from(start);
    Some(operand)
  }

  fn parse_unary(&mut self) -> Option<NodeId> {
    let start = self.span().start;

    let operator = match self.kind() {
      TokenKind::MINUS => Some(OperatorType::MINUS),
      TokenKind::BANG => Some(OperatorType::NOT),
      TokenKind::BITWISE_NOT => Some(OperatorType::BITWISE_NOT),
      TokenKind::ASTERISK => Some(OperatorType::TIMES),
      TokenKind::PLUS => Some(OperatorType::PLUS),
      TokenKind::POINTER_DEREFERENCE_OR_SHIFT_LEFT => Some(OperatorType::POINTER_DEREFERENCE),
      _ => None,
    };
    if let Some(operator) = operator {
      self.bump();
      let operand = self.parse_unary()?;
      // `*u8.[a, b]` is an array of `*u8`, not the address of an array of
      // `u8`: a `*` before a literal's type designation belongs to the type
      // (**L§5.8**).
      if operator == OperatorType::TIMES
        && let Some(literal) = self.point_literal_type_at(operand, start)
      {
        return Some(literal);
      }
      let node = self.push(
        self.span_from(start),
        NodeData::UnaryOperator { operator, operand },
      );
      return Some(node);
    }

    // `(.*) p` is the prefix dereference of L§3.2.
    if self.at(TokenKind::OPEN_PAREN)
      && self.kind_at(1) == TokenKind::POSTFIX_DEREFERENCE
      && self.kind_at(2) == TokenKind::CLOSE_PAREN
    {
      self.bump();
      self.bump();
      self.bump();
      let operand = self.parse_unary()?;
      return Some(self.push(
        self.span_from(start),
        NodeData::UnaryOperator {
          operator: OperatorType::POINTER_DEREFERENCE,
          operand,
        },
      ));
    }

    if self.at(TokenKind::DOUBLE_DOT) {
      self.bump();
      let operand = self.parse_unary()?;
      self.ast.add_flags(operand, NodeFlags::EXPRESSION_IS_SPREAD);
      return Some(operand);
    }

    if self.at(TokenKind::KEYWORD_CAST) {
      return self.parse_cast();
    }

    if self.at(TokenKind::KEYWORD_AUTO_CAST) {
      self.bump();
      let mut flags = CastFlags::IS_AUTO;
      loop {
        if self.eat_modifier(b"no_check") {
          flags |= CastFlags::NO_BOUNDS_CHECK;
        } else if self.eat_modifier(b"trunc") {
          flags |= CastFlags::TRUNCATE;
        } else if self.eat_modifier(b"force") {
          flags |= CastFlags::FORCE;
        } else if self.eat_modifier(b"FORCE") {
          flags |= CastFlags::VERY_FORCE;
        } else {
          break;
        }
      }
      let expression = self.parse_unary()?;
      return Some(self.push(
        self.span_from(start),
        NodeData::Cast(Box::new(Cast {
          target_type: None,
          expression,
          cast_flags: flags,
        })),
      ));
    }

    if self.at(TokenKind::KEYWORD_INLINE) || self.at(TokenKind::KEYWORD_NO_INLINE) {
      let inline = self.at(TokenKind::KEYWORD_INLINE);
      if self.looks_like_procedure(1) {
        self.bump();
        let procedure = self.parse_procedure(start)?;
        if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(procedure).data {
          header.procedure_flags |= if inline {
            ProcedureFlags::SYNTACTICALLY_MARKED_AS_INLINE_YES
          } else {
            ProcedureFlags::SYNTACTICALLY_MARKED_AS_INLINE_NO
          };
        }
        return Some(procedure);
      }
      self.bump();
      let call = self.parse_unary()?;
      if let NodeData::ProcedureCall(inner) = &mut self.ast.node_mut(call).data {
        inner.flags |= if inline {
          CallFlags::INLINE_YES
        } else {
          CallFlags::INLINE_NO
        };
      }
      return Some(call);
    }

    let primary = self.parse_primary()?;
    self.parse_postfix(start, primary)
  }

  fn parse_cast(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();

    let mut flags = CastFlags::empty();
    loop {
      if self.eat_modifier(b"no_check") {
        flags |= CastFlags::NO_BOUNDS_CHECK;
      } else if self.eat_modifier(b"trunc") {
        flags |= CastFlags::TRUNCATE;
      } else if self.eat_modifier(b"force") {
        flags |= CastFlags::FORCE;
      } else if self.eat_modifier(b"FORCE") {
        flags |= CastFlags::VERY_FORCE;
      } else {
        break;
      }
    }

    self.expect(TokenKind::OPEN_PAREN, "'(' after 'cast'")?;
    let target_type = self.parse_type()?;

    if self.eat(TokenKind::COMMA) {
      flags |= CastFlags::HAS_FUNCTION_SYNTAX;
      let expression = self.parse_expression()?;
      while self.eat(TokenKind::COMMA) {
        if self.at(TokenKind::CLOSE_PAREN) {
          break;
        }
        let token = self.bump();
        match self.name_of(&token) {
          b"no_check" => flags |= CastFlags::NO_BOUNDS_CHECK,
          b"trunc" => flags |= CastFlags::TRUNCATE,
          b"force" => flags |= CastFlags::FORCE,
          b"FORCE" => flags |= CastFlags::VERY_FORCE,
          _ => self.error(token.span, "Unknown cast modifier."),
        }
      }
      self.expect(TokenKind::CLOSE_PAREN, "')'")?;
      let node = self.push(
        self.span_from(start),
        NodeData::Cast(Box::new(Cast {
          target_type: Some(target_type),
          expression,
          cast_flags: flags,
        })),
      );
      return self.parse_postfix(start, node);
    }

    self.expect(TokenKind::CLOSE_PAREN, "')'")?;
    if self.eat(TokenKind::POSTFIX_DEREFERENCE) {
      flags |= CastFlags::HAS_DEREFERENCE;
    }

    let expression = self.parse_unary()?;
    Some(self.push(
      self.span_from(start),
      NodeData::Cast(Box::new(Cast {
        target_type: Some(target_type),
        expression,
        cast_flags: flags,
      })),
    ))
  }

  fn parse_postfix(&mut self, start: u32, mut expression: NodeId) -> Option<NodeId> {
    loop {
      match self.kind() {
        TokenKind::DOT => {
          if self.kind_at(1) == TokenKind::OPEN_PAREN {
            self.bump();
            self.bump();
            expression = self.parse_postfix_cast(start, expression)?;
            continue;
          }
          self.bump();
          let right = if self.at(TokenKind::HASH) || self.at(TokenKind::KEYWORD_OPERATOR) {
            self.parse_primary()?
          } else {
            let token = if self.at(TokenKind::KEYWORD_INTERFACE) {
              self.bump()
            } else {
              self.expect(TokenKind::IDENT, "a member name")?
            };
            let ident = self.ident_node(&token);
            if let NodeData::Ident(inner) = &mut self.ast.node_mut(ident).data {
              inner.flags |= IdentFlags::IS_RHS_OF_DOT_DEREFERENCE;
            }
            ident
          };
          expression = self.push(
            self.span_from(start),
            NodeData::BinaryOperator {
              operator: OperatorType::DOT,
              flags: BinaryFlags::empty(),
              left: expression,
              right,
            },
          );
        }
        TokenKind::POSTFIX_DEREFERENCE => {
          self.bump();
          expression = self.push(
            self.span_from(start),
            NodeData::UnaryOperator {
              operator: OperatorType::POSTFIX_DEREFERENCE,
              operand: expression,
            },
          );
        }
        TokenKind::OPEN_BRACKET => {
          self.bump();
          let index = self.parse_expression()?;
          self.expect(TokenKind::CLOSE_BRACKET, "']'")?;
          expression = self.push(
            self.span_from(start),
            NodeData::BinaryOperator {
              operator: OperatorType::ARRAY_SUBSCRIPT,
              flags: BinaryFlags::empty(),
              left: expression,
              right: index,
            },
          );
        }
        TokenKind::OPEN_PAREN => {
          expression = self.parse_call(start, expression)?;
        }
        TokenKind::BEGIN_STRUCT_LITERAL => {
          let type_expression = self.wrap_type(expression);
          expression = self.parse_struct_literal(start, Some(type_expression))?;
        }
        TokenKind::BEGIN_ARRAY_LITERAL => {
          let element_type = self.wrap_type(expression);
          expression = self.parse_array_literal(start, Some(element_type))?;
        }
        _ => break,
      }
    }
    Some(expression)
  }

  fn parse_postfix_cast(&mut self, start: u32, expression: NodeId) -> Option<NodeId> {
    let mut flags = CastFlags::HAS_POSTFIX_SYNTAX;
    let target_type = self.parse_type()?;
    while self.eat(TokenKind::COMMA) {
      let token = self.bump();
      match self.name_of(&token) {
        b"no_check" => flags |= CastFlags::NO_BOUNDS_CHECK,
        b"trunc" => flags |= CastFlags::TRUNCATE,
        b"force" => flags |= CastFlags::FORCE,
        b"FORCE" => flags |= CastFlags::VERY_FORCE,
        _ => self.error(token.span, "Unknown cast modifier."),
      }
    }
    self.expect(TokenKind::CLOSE_PAREN, "')'")?;
    Some(self.push(
      self.span_from(start),
      NodeData::Cast(Box::new(Cast {
        target_type: Some(target_type),
        expression,
        cast_flags: flags,
      })),
    ))
  }

  fn parse_call(&mut self, start: u32, procedure_expression: NodeId) -> Option<NodeId> {
    self.expect(TokenKind::OPEN_PAREN, "'('")?;

    let mut arguments = Vec::new();
    let mut context_modification = None;
    while !self.at(TokenKind::CLOSE_PAREN) && !self.at_end() {
      if self.at(TokenKind::DOUBLE_COMMA) {
        self.bump();
        context_modification = Some(self.parse_context_arguments()?);
        break;
      }
      arguments.push(self.parse_argument()?);
      if self.at(TokenKind::DOUBLE_COMMA) {
        self.bump();
        context_modification = Some(self.parse_context_arguments()?);
        break;
      }
      if !self.eat(TokenKind::COMMA) {
        break;
      }
    }
    self.expect(TokenKind::CLOSE_PAREN, "')'")?;

    let mut flags = CallFlags::empty();
    if self.at_directive(b"no_debug") {
      self.bump();
      self.bump();
      flags |= CallFlags::NO_DEBUG;
    }

    Some(self.push(
      self.span_from(start),
      NodeData::ProcedureCall(Box::new(ProcedureCall {
        procedure_expression,
        arguments,
        context_modification,
        flags,
      })),
    ))
  }

  fn parse_context_arguments(&mut self) -> Option<Vec<NodeId>> {
    let mut expressions = Vec::new();
    while !self.at(TokenKind::CLOSE_PAREN) && !self.at_end() {
      let start = self.span().start;
      let expression = self.parse_expression()?;
      let expression = if self.eat(TokenKind::EQUALS) {
        let value = self.parse_expression()?;
        self.push(
          self.span_from(start),
          NodeData::BinaryOperator {
            operator: OperatorType::ASSIGN,
            flags: BinaryFlags::empty(),
            left: expression,
            right: value,
          },
        )
      } else {
        expression
      };
      expressions.push(expression);
      if !self.eat(TokenKind::COMMA) {
        break;
      }
    }
    Some(expressions)
  }

  fn parse_argument(&mut self) -> Option<Argument> {
    let name = if self.at(TokenKind::IDENT)
      && self.kind_at(1) == TokenKind::EQUALS
      && self.kind_at(2) != TokenKind::EQUALS
    {
      let token = self.bump();
      let ident = self.ident_node(&token);
      self.bump();
      Some(ident)
    } else {
      None
    };
    let expression = self.parse_expression()?;
    Some(Argument { name, expression })
  }

  fn parse_argument_list(&mut self, terminator: TokenKind) -> Option<Vec<Argument>> {
    let mut arguments = Vec::new();
    loop {
      if self.at(terminator) || self.at_end() {
        break;
      }
      arguments.push(self.parse_argument()?);
      if !self.eat(TokenKind::COMMA) {
        break;
      }
    }
    Some(arguments)
  }

  fn parse_struct_literal(
    &mut self,
    start: u32,
    type_expression: Option<NodeId>,
  ) -> Option<NodeId> {
    self.expect(TokenKind::BEGIN_STRUCT_LITERAL, "'.{'")?;
    let mut arguments = Vec::new();
    while !self.at(TokenKind::CLOSE_BRACE) && !self.at_end() {
      arguments.push(self.parse_struct_literal_argument()?);
      if !self.eat(TokenKind::COMMA) {
        break;
      }
    }
    self.expect(TokenKind::CLOSE_BRACE, "'}'")?;

    Some(self.push(
      self.span_from(start),
      NodeData::Literal(Literal {
        value: LiteralValue::Struct(StructLiteral {
          type_expression,
          arguments,
        }),
        flags: LiteralFlags::empty(),
      }),
    ))
  }

  /// A named field of a struct literal may be any assignable expression
  /// (`values[1] = 7`, `e.z = 9`), not only a bare name (**L§5.7**).
  fn parse_struct_literal_argument(&mut self) -> Option<Argument> {
    let start = self.span().start;
    let first = self.parse_expression()?;
    if self.at(TokenKind::EQUALS) && self.kind_at(1) != TokenKind::EQUALS {
      self.bump();
      let value = self.parse_expression()?;
      if let NodeData::Ident(_) = self.ast.data(first) {
        return Some(Argument {
          name: Some(first),
          expression: value,
        });
      }
      let assignment = self.push(
        self.span_from(start),
        NodeData::BinaryOperator {
          operator: OperatorType::ASSIGN,
          flags: BinaryFlags::empty(),
          left: first,
          right: value,
        },
      );
      return Some(Argument {
        name: None,
        expression: assignment,
      });
    }
    Some(Argument {
      name: None,
      expression: first,
    })
  }

  fn parse_array_literal(&mut self, start: u32, element_type: Option<NodeId>) -> Option<NodeId> {
    self.expect(TokenKind::BEGIN_ARRAY_LITERAL, "'.['")?;
    let mut members = Vec::new();
    while !self.at(TokenKind::CLOSE_BRACKET) && !self.at_end() {
      members.push(self.parse_expression()?);
      if !self.eat(TokenKind::COMMA) {
        break;
      }
    }
    self.expect(TokenKind::CLOSE_BRACKET, "']'")?;

    Some(self.push(
      self.span_from(start),
      NodeData::Literal(Literal {
        value: LiteralValue::Array(ArrayLiteral {
          element_type,
          members,
        }),
        flags: LiteralFlags::empty(),
      }),
    ))
  }

  fn parse_primary(&mut self) -> Option<NodeId> {
    let start = self.span().start;

    match self.kind() {
      TokenKind::NUMBER => {
        let token = self.bump();
        Some(self.number_literal(&token))
      }
      TokenKind::STRING => {
        let token = self.bump();
        let text = match token.value {
          TokenValue::Text(text) => text,
          _ => Box::default(),
        };
        let mut flags = LiteralFlags::empty();
        if token.flags.contains(ValueFlags::HERE_STRING) {
          flags |= LiteralFlags::HERE_STRING;
        }
        Some(self.push(
          token.span,
          NodeData::Literal(Literal {
            value: LiteralValue::Text(text),
            flags,
          }),
        ))
      }
      TokenKind::KEYWORD_TRUE | TokenKind::KEYWORD_FALSE => {
        let value = self.at(TokenKind::KEYWORD_TRUE);
        let token = self.bump();
        Some(self.push(
          token.span,
          NodeData::Literal(Literal {
            value: LiteralValue::Bool(value),
            flags: LiteralFlags::empty(),
          }),
        ))
      }
      TokenKind::KEYWORD_NULL => {
        let token = self.bump();
        Some(self.push(
          token.span,
          NodeData::Literal(Literal {
            value: LiteralValue::Null,
            flags: LiteralFlags::empty(),
          }),
        ))
      }
      TokenKind::KEYWORD_CONTEXT => {
        let token = self.bump();
        Some(self.push(token.span, NodeData::Context))
      }
      // `interface` is a keyword only in a type restriction (**L§7.8**); the
      // bindings modules use it as an ordinary member name.
      TokenKind::IDENT | TokenKind::KEYWORD_INTERFACE => {
        if self.kind_at(1) == TokenKind::QUICK_LAMBDA {
          return self.parse_quick_lambda(start);
        }
        let token = self.bump();
        Some(self.ident_node(&token))
      }
      TokenKind::KEYWORD_OPERATOR => self.parse_operator_name(),
      TokenKind::DOLLAR => {
        self.bump();
        let token = self.expect(TokenKind::IDENT, "a polymorph variable name")?;
        let ident = self.ident_node(&token);
        if let NodeData::Ident(inner) = &mut self.ast.node_mut(ident).data {
          inner.flags |= IdentFlags::DEFINES_POLYMORPH_VARIABLE;
        }
        self.ast.node_mut(ident).span = self.span_from(start);
        Some(ident)
      }
      TokenKind::DOT => {
        self.bump();
        let token = self.expect(TokenKind::IDENT, "a name after '.'")?;
        let ident = self.ident_node(&token);
        if let NodeData::Ident(inner) = &mut self.ast.node_mut(ident).data {
          inner.flags |= IdentFlags::IS_RHS_OF_DOT_DEREFERENCE;
        }
        Some(self.push(
          self.span_from(start),
          NodeData::UnaryOperator {
            operator: OperatorType::DOT,
            operand: ident,
          },
        ))
      }
      TokenKind::BEGIN_STRUCT_LITERAL => self.parse_struct_literal(start, None),
      TokenKind::BEGIN_ARRAY_LITERAL => self.parse_array_literal(start, None),
      TokenKind::OPEN_PAREN => {
        if self.looks_like_procedure(0) {
          return self.parse_procedure(start);
        }
        self.bump();
        let inner = self.parse_expression()?;
        self.expect(TokenKind::CLOSE_PAREN, "')'")?;
        self.ast.add_flags(inner, NodeFlags::IS_PARENTHESIZED);
        Some(inner)
      }
      TokenKind::OPEN_BRACKET | TokenKind::ASTERISK => self.parse_type(),
      TokenKind::KEYWORD_STRUCT | TokenKind::KEYWORD_UNION => self.parse_struct(),
      TokenKind::KEYWORD_ENUM | TokenKind::KEYWORD_ENUM_FLAGS => self.parse_enum(),
      TokenKind::KEYWORD_SIZE_OF => self.parse_type_query(TypeQueryKind::SizeOf),
      TokenKind::KEYWORD_TYPE_INFO => self.parse_type_query(TypeQueryKind::TypeInfo),
      TokenKind::KEYWORD_INITIALIZER_OF => self.parse_type_query(TypeQueryKind::InitializerOf),
      TokenKind::KEYWORD_TYPE_OF => self.parse_expression_query(ExpressionQueryKind::TypeOf),
      TokenKind::KEYWORD_IS_CONSTANT => {
        self.parse_expression_query(ExpressionQueryKind::IsConstant)
      }
      TokenKind::KEYWORD_CODE_OF => self.parse_expression_query(ExpressionQueryKind::CodeOf),
      TokenKind::KEYWORD_IFX => self.parse_ifx(false),
      TokenKind::HASH => self.parse_directive_expression(),
      _ => {
        self.error_here(format!(
          "Expected an expression, but got '{}'.",
          self.token_text()
        ));
        None
      }
    }
  }

  fn number_literal(&mut self, token: &Token) -> NodeId {
    let mut flags = LiteralFlags::IS_A_NUMBER;
    if token.flags.contains(ValueFlags::HEX) {
      flags |= LiteralFlags::HEX;
    }
    if token.flags.contains(ValueFlags::BINARY) {
      flags |= LiteralFlags::BINARY;
    }
    if token.flags.contains(ValueFlags::FLOAT) {
      flags |= LiteralFlags::FLOAT;
    }
    if token.flags.contains(ValueFlags::DEFAULTS_TO_FLOAT64) {
      flags |= LiteralFlags::DEFAULTS_TO_FLOAT64;
    }
    if token.flags.contains(ValueFlags::REQUIRES_FLOAT64) {
      flags |= LiteralFlags::REQUIRES_FLOAT64;
    }

    let value = match token.value {
      TokenValue::Float(value) => LiteralValue::Float(value),
      TokenValue::Integer(value) => LiteralValue::Integer(value),
      _ => LiteralValue::Integer(0),
    };
    self.push(token.span, NodeData::Literal(Literal { value, flags }))
  }

  fn parse_type_query(&mut self, query_kind: TypeQueryKind) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();
    self.expect(TokenKind::OPEN_PAREN, "'('")?;
    let type_to_query = self.parse_type()?;
    self.expect(TokenKind::CLOSE_PAREN, "')'")?;
    Some(self.push(
      self.span_from(start),
      NodeData::TypeQuery {
        query_kind,
        type_to_query,
      },
    ))
  }

  fn parse_expression_query(&mut self, query_kind: ExpressionQueryKind) -> Option<NodeId> {
    let start = self.span().start;
    self.bump();
    self.expect(TokenKind::OPEN_PAREN, "'('")?;
    let expression_to_query = self.parse_expression()?;
    self.expect(TokenKind::CLOSE_PAREN, "')'")?;
    Some(self.push(
      self.span_from(start),
      NodeData::ExpressionQuery {
        query_kind,
        expression_to_query,
      },
    ))
  }
}

// ---------------------------------------------------------------------------
// Procedures
// ---------------------------------------------------------------------------

impl Parser<'_> {
  /// Decides whether the `(` at `ahead` opens a parameter list rather than a
  /// parenthesized expression (**C§5.2**).
  fn looks_like_procedure(&self, ahead: usize) -> bool {
    let mut index = self.position + ahead;
    if self.tokens.get(index).map(|token| token.kind) != Some(TokenKind::OPEN_PAREN) {
      return false;
    }

    let mut depth = 0i32;
    let mut saw_colon = false;
    let mut count = 0usize;
    loop {
      let Some(token) = self.tokens.get(index) else {
        return false;
      };
      match token.kind {
        TokenKind::OPEN_PAREN | TokenKind::OPEN_BRACKET | TokenKind::OPEN_BRACE => depth += 1,
        TokenKind::CLOSE_PAREN | TokenKind::CLOSE_BRACKET | TokenKind::CLOSE_BRACE => {
          depth -= 1;
          if depth == 0 {
            break;
          }
        }
        TokenKind::COLON if depth == 1 => saw_colon = true,
        TokenKind::END_OF_INPUT => return false,
        _ => {}
      }
      index += 1;
      count += 1;
    }

    if count == 1 {
      return true;
    }
    if saw_colon {
      return true;
    }

    match self.tokens.get(index + 1).map(|token| token.kind) {
      Some(TokenKind::RIGHT_ARROW) | Some(TokenKind::QUICK_LAMBDA) => true,
      Some(TokenKind::HASH) => self
        .tokens
        .get(index + 2)
        .map(|token| is_procedure_directive(self.name_of(token)))
        .unwrap_or(false),
      _ => false,
    }
  }

  /// A lambda's parameter list holds names, not types (**L§7.9**), but the
  /// parameter parser cannot know that until it reaches the `=>` or the `->`
  /// that has no return types; `(x, y)` is repaired here.
  fn name_untyped_parameters(&mut self, entries: Vec<NodeId>) -> Vec<NodeId> {
    for entry in &entries {
      let NodeData::Declaration(declaration) = self.ast.data(*entry) else {
        continue;
      };
      if declaration.name.is_some() || declaration.expression.is_some() {
        continue;
      }
      let Some(type_inst) = declaration.type_inst else {
        continue;
      };
      let NodeData::TypeInstantiation(inst) = self.ast.data(type_inst) else {
        continue;
      };
      let Some(expression) = inst.type_valued_expression else {
        continue;
      };
      if !matches!(self.ast.data(expression), NodeData::Ident(_)) {
        continue;
      }
      if let NodeData::Declaration(declaration) = &mut self.ast.node_mut(*entry).data {
        declaration.name = Some(expression);
        declaration.type_inst = None;
      }
    }
    entries
  }

  fn parse_quick_lambda(&mut self, start: u32) -> Option<NodeId> {
    let token = self.bump();
    let argument = self.quick_lambda_argument(&token);
    self.expect(TokenKind::QUICK_LAMBDA, "'=>'")?;
    self.finish_quick_lambda(start, vec![argument])
  }

  fn quick_lambda_argument(&mut self, token: &Token) -> NodeId {
    let name = self.ident_node(token);
    let start = token.span.start;
    self.make_declaration(start, Some(name), None, None, DeclarationFlags::empty())
  }

  fn finish_quick_lambda(&mut self, start: u32, arguments: Vec<NodeId>) -> Option<NodeId> {
    let mut header = ProcedureHeader::empty();
    header.arguments = arguments;
    header.procedure_flags = ProcedureFlags::QUICK | ProcedureFlags::POLYMORPHIC;

    let block_start = self.span().start;
    let (block, in_block_form) = if self.at(TokenKind::OPEN_BRACE) {
      (
        self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())?,
        true,
      )
    } else {
      let expression = self.parse_expression()?;
      let value = self.push(
        self.span_from(block_start),
        NodeData::Return {
          arguments: vec![Argument {
            name: None,
            expression,
          }],
          flags: ReturnFlags::AUTO_INSERTED_FOR_QUICK_LAMBDA,
        },
      );
      (
        self.push(
          self.span_from(block_start),
          NodeData::Block(Block {
            block_type: BlockType::Imperative,
            block_flags: BlockFlags::empty(),
            statements: vec![value],
          }),
        ),
        false,
      )
    };
    if in_block_form {
      header.procedure_flags |= ProcedureFlags::QUICK_IN_BLOCK_FORM;
    }

    let header_id = self.push(
      self.span_from(start),
      NodeData::ProcedureHeader(Box::new(header)),
    );
    let body = self.push(
      self.span_from(start),
      NodeData::ProcedureBody {
        header: header_id,
        block,
      },
    );
    if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
      header.body_or_null = Some(body);
    }
    self.parse_header_directives(header_id);
    Some(header_id)
  }

  fn parse_procedure(&mut self, start: u32) -> Option<NodeId> {
    let arguments = self.parse_parameter_list()?;

    if self.at(TokenKind::QUICK_LAMBDA) {
      self.bump();
      let entries = self.name_untyped_parameters(arguments.entries);
      return self.finish_quick_lambda(start, entries);
    }

    let mut header = ProcedureHeader::empty();
    header.arguments = arguments.entries;

    let mut implicit_returns = false;
    if self.eat(TokenKind::RIGHT_ARROW) {
      if self.at(TokenKind::OPEN_BRACE) {
        implicit_returns = true;
        header.arguments = self.name_untyped_parameters(header.arguments);
      } else {
        let returns = self.parse_return_types()?;
        header.returns = returns.declarations;
        header.parenthesized_returns = returns.parenthesized;
      }
    }
    if implicit_returns {
      header.procedure_flags |= ProcedureFlags::HAS_IMPLICIT_RETURN_VALUE;
    }

    let header_id = self.push(
      self.span_from(start),
      NodeData::ProcedureHeader(Box::new(header)),
    );
    self.parse_header_directives(header_id);

    let block_flags = self.block_check_flags();
    if self.at(TokenKind::OPEN_BRACE) {
      let flags = block_flags.unwrap_or_default();
      let block_start = self.span().start;
      let block = self.parse_braced_block(BlockType::Imperative, flags)?;
      let body = self.push(
        self.span_from(block_start),
        NodeData::ProcedureBody {
          header: header_id,
          block,
        },
      );
      if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
        header.body_or_null = Some(body);
      }
      self.attach_trailing_notes(header_id);
    }

    self.ast.node_mut(header_id).span = self.span_from(start);
    Some(header_id)
  }

  fn parse_header_directives(&mut self, header_id: NodeId) {
    while let Some(word) = self.directive() {
      let mut flags = ProcedureFlags::empty();
      match word {
        b"c_call" => flags |= ProcedureFlags::C_CALL,
        b"no_context" => flags |= ProcedureFlags::SYNTACTICALLY_MARKED_AS_NO_CONTEXT,
        b"expand" => flags |= ProcedureFlags::MACRO,
        b"symmetric" => flags |= ProcedureFlags::SYMMETRIC,
        b"no_call" => flags |= ProcedureFlags::NO_CALL,
        b"no_debug" => flags |= ProcedureFlags::NO_DEBUG,
        b"entry_point" => flags |= ProcedureFlags::ENTRY_POINT_HOOK,
        b"compile_time" => flags |= ProcedureFlags::SYNTACTICALLY_MARKED_AS_COMPILE_TIME,
        b"runtime_support" => flags |= ProcedureFlags::RUNTIME_SUPPORT,
        b"cpp_method" => flags |= ProcedureFlags::CPP_METHOD,
        b"cpp_return_type_is_non_pod" => flags |= ProcedureFlags::CPP_RETURN_TYPE_IS_NON_POD,
        b"dump" => flags |= ProcedureFlags::DEBUG_DUMP,
        b"no_alias" => flags |= ProcedureFlags::NO_ALIAS,
        b"no_abc" | b"no_aoc" => break,
        b"modify" => {
          self.bump();
          self.bump();
          let start = self.previous_end();
          let Some(block) = self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())
          else {
            return;
          };
          let modify = self.push(self.span_from(start), NodeData::DirectiveModify { block });
          if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
            header.modify_directives.push(modify);
          }
          continue;
        }
        b"foreign" | b"elsewhere" => {
          let is_foreign = word == b"foreign";
          self.bump();
          self.bump();
          let mut library = None;
          let mut symbol = None;
          if self.at(TokenKind::IDENT) {
            let token = self.bump();
            library = Some(self.ident_node(&token));
          }
          if self.at(TokenKind::STRING) {
            symbol = self.string_value("a foreign symbol name");
          }
          if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
            header.procedure_flags |= ProcedureFlags::ELSEWHERE;
            if is_foreign {
              header.procedure_flags |= ProcedureFlags::C_CALL;
            }
            header.library_identifier = library;
            header.foreign_function_name = symbol;
          }
          continue;
        }
        b"intrinsic" => {
          self.bump();
          self.bump();
          let name = if self.at(TokenKind::STRING) {
            self.string_value("an intrinsic name")
          } else {
            None
          };
          if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
            header.procedure_flags |= ProcedureFlags::INTRINSIC;
            header.intrinsic_name = name;
          }
          continue;
        }
        b"compiler" => {
          self.bump();
          self.bump();
          let name = if self.at(TokenKind::STRING) {
            self.string_value("a compiler procedure name")
          } else {
            None
          };
          if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
            header.procedure_flags |= ProcedureFlags::SYNTACTICALLY_MARKED_AS_COMPILER;
            if name.is_some() {
              header.intrinsic_name = name;
            }
          }
          continue;
        }
        b"deprecated" => {
          self.bump();
          self.bump();
          let message = if self.at(TokenKind::STRING) {
            self.string_value("a deprecation message")
          } else {
            None
          };
          if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
            header.procedure_flags |= ProcedureFlags::DEPRECATED;
            header.deprecation_string = message;
          }
          continue;
        }
        _ => break,
      }

      self.bump();
      self.bump();
      if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
        header.procedure_flags |= flags;
      }
    }
  }

  fn parse_parameter_list(&mut self) -> Option<ParameterList> {
    self.expect(TokenKind::OPEN_PAREN, "'('")?;
    let mut entries = Vec::new();
    self.parameter_depth += 1;

    while !self.at(TokenKind::CLOSE_PAREN) && !self.at_end() {
      let start = self.span().start;
      let using = if self.at(TokenKind::KEYWORD_USING) {
        Some(self.parse_using_modifiers()?)
      } else {
        None
      };

      let declaration = self.parse_parameter(start)?;
      match using {
        Some((filter_type, filter_expression, no_parameters)) => {
          let node = self.push(
            self.span_from(start),
            NodeData::Using(Box::new(Using {
              expression: declaration,
              filter_type,
              filter_expression,
              no_parameters,
            })),
          );
          entries.push(node);
        }
        None => entries.push(declaration),
      }

      if !self.eat(TokenKind::COMMA) {
        break;
      }
    }
    self.parameter_depth -= 1;
    self.expect(TokenKind::CLOSE_PAREN, "')'")?;

    Some(ParameterList { entries })
  }

  fn parse_using_modifiers(&mut self) -> Option<(FilterType, Option<NodeId>, bool)> {
    self.bump();
    let mut filter_type = FilterType::None;
    let mut filter_expression = None;
    let mut no_parameters = false;
    loop {
      let filter = if self.at_modifier(b"only") {
        FilterType::Only
      } else if self.at_modifier(b"except") {
        FilterType::Except
      } else if self.at_modifier(b"map") {
        FilterType::Map
      } else if self.eat_modifier(b"no_parameters") {
        no_parameters = true;
        continue;
      } else {
        break;
      };
      self.bump();
      self.bump();
      filter_type = filter;
      filter_expression = Some(self.parse_filter_expression()?);
    }
    Some((filter_type, filter_expression, no_parameters))
  }

  /// `using,except(a, b)` names a list; `using,except CONST` names one
  /// expression (**L§6.8**).
  fn parse_filter_expression(&mut self) -> Option<NodeId> {
    if !self.at(TokenKind::OPEN_PAREN) {
      return self.parse_unary();
    }

    let start = self.span().start;
    self.bump();
    let first = self.parse_expression()?;
    if !self.at(TokenKind::COMMA) {
      self.expect(TokenKind::CLOSE_PAREN, "')'")?;
      return Some(first);
    }

    let mut arguments = vec![CommaArgument {
      node: first,
      modifier: CommaModifier::None,
    }];
    while self.eat(TokenKind::COMMA) {
      if self.at(TokenKind::CLOSE_PAREN) {
        break;
      }
      let node = self.parse_expression()?;
      arguments.push(CommaArgument {
        node,
        modifier: CommaModifier::None,
      });
    }
    self.expect(TokenKind::CLOSE_PAREN, "')'")?;
    Some(self.push(
      self.span_from(start),
      NodeData::CommaSeparatedArguments { arguments },
    ))
  }

  fn parse_parameter(&mut self, start: u32) -> Option<NodeId> {
    let mut flags = DeclarationFlags::empty();
    if self.at_directive(b"discard") {
      self.bump();
      self.bump();
      flags |= DeclarationFlags::IS_MARKED_AS_DISCARD;
    }

    if self.at(TokenKind::DOLLAR) && self.kind_at(1) == TokenKind::IDENT {
      let bake_start = self.span().start;
      self.bump();
      let token = self.bump();
      if self.at(TokenKind::COLON) {
        let name = self.ident_node(&token);
        flags |= DeclarationFlags::AUTO_VALUE_BAKE_IS_REQUIRED;
        return self.parse_parameter_tail(bake_start, name, flags);
      }
      self.position -= 2;
    }

    if self.at(TokenKind::DOUBLE_DOLLAR) && self.kind_at(1) == TokenKind::IDENT {
      let bake_start = self.span().start;
      self.bump();
      let token = self.bump();
      let name = self.ident_node(&token);
      flags |= DeclarationFlags::AUTO_VALUE_BAKE;
      return self.parse_parameter_tail(bake_start, name, flags);
    }

    if self.at(TokenKind::IDENT) && self.kind_at(1) == TokenKind::COLON {
      let token = self.bump();
      let name = self.ident_node(&token);
      return self.parse_parameter_tail(start, name, flags);
    }

    let type_inst = self.parse_type()?;
    Some(self.make_declaration(start, None, Some(type_inst), None, flags))
  }

  fn parse_parameter_tail(
    &mut self,
    start: u32,
    name: NodeId,
    mut flags: DeclarationFlags,
  ) -> Option<NodeId> {
    self.expect(TokenKind::COLON, "':'")?;

    let mut type_inst = None;
    let mut expression = None;
    if self.eat(TokenKind::EQUALS) {
      expression = Some(self.parse_expression()?);
    } else if self.eat(TokenKind::COLON) {
      flags |= DeclarationFlags::IS_CONSTANT;
      expression = Some(self.parse_expression()?);
    } else {
      type_inst = Some(self.parse_type()?);
      if self.eat(TokenKind::EQUALS) {
        expression = Some(self.parse_expression()?);
      }
    }

    Some(self.make_declaration(start, Some(name), type_inst, expression, flags))
  }

  /// A `(` after `->` opens the parenthesized *return list* of **L§7.2**; only
  /// a following `->` makes it the parameter list of a returned procedure type,
  /// since `-> (s32) #c_call { }` is a `#c_call` procedure returning one s32.
  fn parenthesized_is_procedure_type(&self) -> bool {
    let Some(close) = self.matching_paren(self.position) else {
      return false;
    };
    self.tokens.get(close + 1).map(|token| token.kind) == Some(TokenKind::RIGHT_ARROW)
  }

  /// Whether the `(` the parser stands on holds a top-level comma.
  fn paren_holds_a_comma(&self) -> bool {
    let Some(close) = self.matching_paren(self.position) else {
      return false;
    };
    let mut depth = 0i32;
    for token in &self.tokens[self.position..close] {
      match token.kind {
        TokenKind::OPEN_PAREN | TokenKind::OPEN_BRACKET | TokenKind::OPEN_BRACE => depth += 1,
        TokenKind::CLOSE_PAREN | TokenKind::CLOSE_BRACKET | TokenKind::CLOSE_BRACE => depth -= 1,
        TokenKind::COMMA if depth == 1 => return true,
        _ => {}
      }
    }
    false
  }

  /// The index of the `)` that closes the `(` at `open`.
  fn matching_paren(&self, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut index = open;
    loop {
      let token = self.tokens.get(index)?;
      match token.kind {
        TokenKind::OPEN_PAREN | TokenKind::OPEN_BRACKET | TokenKind::OPEN_BRACE => depth += 1,
        TokenKind::CLOSE_PAREN | TokenKind::CLOSE_BRACKET | TokenKind::CLOSE_BRACE => {
          depth -= 1;
          if depth == 0 {
            return Some(index);
          }
        }
        TokenKind::END_OF_INPUT => return None,
        _ => {}
      }
      index += 1;
    }
  }

  fn parse_return_types(&mut self) -> Option<ReturnList> {
    let mut parenthesized = false;
    if self.at(TokenKind::OPEN_PAREN) && !self.parenthesized_is_procedure_type() {
      if self.kind_at(1) == TokenKind::CLOSE_PAREN {
        self.bump();
        self.bump();
        return Some(ReturnList {
          declarations: Vec::new(),
          parenthesized: true,
        });
      }
      parenthesized = true;
      self.bump();
    }

    let mut declarations = Vec::new();
    loop {
      let declaration = self.parse_return_value()?;
      declarations.push(declaration);
      if !parenthesized && self.parameter_depth > 0 {
        break;
      }
      if !self.eat(TokenKind::COMMA) {
        break;
      }
      if parenthesized && self.at(TokenKind::CLOSE_PAREN) {
        break;
      }
    }
    if parenthesized {
      self.expect(TokenKind::CLOSE_PAREN, "')'")?;
    }

    Some(ReturnList {
      declarations,
      parenthesized,
    })
  }

  fn parse_return_value(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let mut flags = DeclarationFlags::empty();

    let named = self.at(TokenKind::IDENT) && self.kind_at(1) == TokenKind::COLON;
    let (name, type_inst, expression) = if named {
      let token = self.bump();
      let name = self.ident_node(&token);
      self.bump();
      if self.eat(TokenKind::EQUALS) {
        (Some(name), None, Some(self.parse_expression()?))
      } else {
        let type_inst = self.parse_type()?;
        let expression = if self.eat(TokenKind::EQUALS) {
          Some(self.parse_expression()?)
        } else {
          None
        };
        (Some(name), Some(type_inst), expression)
      }
    } else {
      (None, Some(self.parse_type()?), None)
    };

    if self.at_directive(b"must") {
      self.bump();
      self.bump();
      flags |= DeclarationFlags::MUST_BE_RECEIVED;
    }

    Some(self.make_declaration(start, name, type_inst, expression, flags))
  }
}

struct ParameterList {
  entries: Vec<NodeId>,
}

struct ReturnList {
  declarations: Vec<NodeId>,
  parenthesized: bool,
}

fn is_procedure_directive(name: &[u8]) -> bool {
  matches!(
    name,
    b"c_call"
      | b"no_context"
      | b"expand"
      | b"foreign"
      | b"elsewhere"
      | b"intrinsic"
      | b"compiler"
      | b"runtime_support"
      | b"symmetric"
      | b"no_call"
      | b"entry_point"
      | b"deprecated"
      | b"no_debug"
      | b"compile_time"
      | b"cpp_method"
      | b"cpp_return_type_is_non_pod"
      | b"modify"
      | b"dump"
      | b"no_alias"
      | b"must"
      | b"no_abc"
      | b"no_aoc"
  )
}

// ---------------------------------------------------------------------------
// Structs and enums
// ---------------------------------------------------------------------------

impl Parser<'_> {
  fn parse_struct(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let is_union = self.at(TokenKind::KEYWORD_UNION);
    self.bump();

    let mut structure = StructNode {
      arguments: Vec::new(),
      has_argument_list: false,
      block: None,
      modify_directives: Vec::new(),
      notes: Vec::new(),
      textual_flags: StructFlags::empty(),
      alignment_expression: None,
    };
    if is_union {
      structure.textual_flags |= StructFlags::UNION;
    }

    if self.at(TokenKind::OPEN_PAREN) {
      let parameters = self.parse_parameter_list()?;
      structure.arguments = parameters.entries;
      structure.has_argument_list = true;
    }

    let mut modify_blocks = Vec::new();
    while let Some(word) = self.directive() {
      match word {
        b"type_info_none" => structure.textual_flags |= StructFlags::TYPE_INFO_NONE,
        b"type_info_no_size_complaint" => {
          structure.textual_flags |= StructFlags::TYPE_INFO_NO_SIZE_COMPLAINT
        }
        b"type_info_procedures_are_void_pointers" => {
          structure.textual_flags |= StructFlags::TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS
        }
        b"no_padding" => structure.textual_flags |= StructFlags::NO_PADDING,
        b"foreign" => structure.textual_flags |= StructFlags::FOREIGN,
        b"align" => {
          self.bump();
          self.bump();
          structure.alignment_expression = Some(self.parse_unary()?);
          continue;
        }
        b"modify" => {
          self.bump();
          self.bump();
          let modify_start = self.span().start;
          let block = self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())?;
          let modify = self.push(
            self.span_from(modify_start),
            NodeData::DirectiveModify { block },
          );
          modify_blocks.push(modify);
          continue;
        }
        _ => break,
      }
      self.bump();
      self.bump();
    }
    structure.modify_directives = modify_blocks;

    let notes = self.parse_notes();
    structure.notes = notes;

    if self.at(TokenKind::OPEN_BRACE) {
      structure.block =
        Some(self.parse_braced_block(BlockType::DataDeclarations, BlockFlags::empty())?);
    }

    // `} #no_padding;` — the layout directives may also follow the body, which
    // is how `Program_Print` writes them back out.
    loop {
      if self.eat_directive(b"no_padding") {
        structure.textual_flags |= StructFlags::NO_PADDING;
      } else if self.eat_directive(b"foreign") {
        structure.textual_flags |= StructFlags::FOREIGN;
      } else if self.eat_directive(b"type_info_none") {
        structure.textual_flags |= StructFlags::TYPE_INFO_NONE;
      } else if self.eat_directive(b"type_info_no_size_complaint") {
        structure.textual_flags |= StructFlags::TYPE_INFO_NO_SIZE_COMPLAINT;
      } else if self.eat_directive(b"type_info_procedures_are_void_pointers") {
        structure.textual_flags |= StructFlags::TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS;
      } else {
        break;
      }
    }

    let node = self.push(self.span_from(start), NodeData::Struct(Box::new(structure)));
    self.attach_trailing_notes(node);
    Some(node)
  }

  fn parse_enum(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let is_flags = self.at(TokenKind::KEYWORD_ENUM_FLAGS);
    self.bump();

    let mut enumeration = EnumNode {
      internal_type_inst: None,
      block: None,
      notes: Vec::new(),
      is_flags,
      marked_as_complete: false,
      marked_as_specified: false,
    };

    if !self.at(TokenKind::OPEN_BRACE) && !self.at(TokenKind::HASH) && !self.at(TokenKind::NOTE) {
      enumeration.internal_type_inst = Some(self.parse_type()?);
    }

    loop {
      if self.eat_directive(b"specified") {
        enumeration.marked_as_specified = true;
      } else if self.eat_directive(b"complete") {
        enumeration.marked_as_complete = true;
      } else {
        break;
      }
    }

    enumeration.notes = self.parse_notes();

    if self.at(TokenKind::OPEN_BRACE) {
      enumeration.block = Some(self.parse_braced_block(BlockType::Constants, BlockFlags::empty())?);
    }

    let node = self.push(self.span_from(start), NodeData::Enum(Box::new(enumeration)));
    self.attach_trailing_notes(node);
    Some(node)
  }

  fn parse_notes(&mut self) -> Vec<NodeId> {
    let mut notes = Vec::new();
    while self.at(TokenKind::NOTE) {
      let token = self.bump();
      let name = match token.value {
        TokenValue::Name(symbol) => symbol,
        _ => self.interner.intern(b""),
      };
      notes.push(self.push(token.span, NodeData::Note { text: name }));
    }
    notes
  }
}

// ---------------------------------------------------------------------------
// Directives
// ---------------------------------------------------------------------------

impl Parser<'_> {
  fn parse_directive_statement(&mut self, block_type: BlockType) -> Option<NodeId> {
    let start = self.span().start;
    let Some(word) = self.directive() else {
      self.error_here("Expected a directive after '#'.");
      return None;
    };

    match word {
      b"if" => {
        self.bump();
        self.parse_if(true)
      }
      b"scope_export" | b"scope_module" | b"scope_file" => {
        let scope_type = match word {
          b"scope_export" => ScopeType::Export,
          b"scope_file" => ScopeType::File,
          _ => ScopeType::Internal,
        };
        self.bump();
        self.bump();
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveScope { scope_type },
        ))
      }
      b"load" => {
        self.bump();
        self.bump();
        let name = self.string_value("a file name after '#load'")?;
        Some(self.push(self.span_from(start), NodeData::DirectiveLoad { name }))
      }
      b"placeholder" => {
        self.bump();
        self.bump();
        let token = self.expect(TokenKind::IDENT, "a name after '#placeholder'")?;
        let name = self.ident_node(&token);
        let placeholder = self.push(self.span_from(start), NodeData::Placeholder);
        Some(self.make_declaration(
          start,
          Some(name),
          None,
          Some(placeholder),
          DeclarationFlags::IS_CONSTANT,
        ))
      }
      b"poke_name" => {
        self.bump();
        self.bump();
        let module_token = self.expect(TokenKind::IDENT, "a module name")?;
        let module = self.ident_node(&module_token);
        let name_token = self.expect(TokenKind::IDENT, "a name to poke")?;
        let name = self.ident_node(&name_token);
        Some(self.push(
          self.span_from(start),
          NodeData::DirectivePokeName { module, name },
        ))
      }
      b"add_context" => {
        self.bump();
        self.bump();
        let expression = self.parse_statement(block_type)?;
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveAddContext { expression },
        ))
      }
      b"module_parameters" => self.parse_module_parameters(start),
      b"place" => {
        self.bump();
        self.bump();
        let token = self.expect(TokenKind::IDENT, "a member name after '#place'")?;
        let ident = self.ident_node(&token);
        Some(self.push(self.span_from(start), NodeData::DirectivePlace { ident }))
      }
      b"through" => {
        self.bump();
        self.bump();
        Some(self.push(self.span_from(start), NodeData::DirectiveThrough))
      }
      b"bytes" => {
        self.bump();
        self.bump();
        let expression = self.parse_expression()?;
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveBytes { expression },
        ))
      }
      b"asm" => self.parse_asm(start),
      b"no_reset" => {
        self.bump();
        self.bump();
        let declaration = self.parse_statement(block_type)?;
        self.set_declaration_flags(declaration, DeclarationFlags::NO_RESET);
        Some(declaration)
      }
      b"as" => {
        self.bump();
        self.bump();
        let declaration = self.parse_statement(block_type)?;
        self.set_declaration_flags(declaration, DeclarationFlags::IS_MARKED_AS_AS);
        Some(declaration)
      }
      b"align" => {
        self.bump();
        self.bump();
        let alignment = self.parse_unary()?;
        let declaration = self.parse_statement(block_type)?;
        self.set_alignment(declaration, alignment);
        Some(declaration)
      }
      b"program_export" => {
        self.bump();
        self.bump();
        let name = if self.at(TokenKind::STRING) {
          self.string_value("an export name")
        } else {
          None
        };
        let declaration = self.parse_statement(block_type)?;
        self.set_declaration_flags(declaration, DeclarationFlags::PROGRAM_EXPORT);
        if let Some(name) = name {
          self.set_export_name(declaration, name);
        }
        Some(declaration)
      }
      _ => self.parse_declaration_statement(),
    }
  }

  fn set_declaration_flags(&mut self, statement: NodeId, flags: DeclarationFlags) {
    let target = self.declaration_of(statement);
    if let Some(target) = target
      && let NodeData::Declaration(declaration) = &mut self.ast.node_mut(target).data
    {
      declaration.flags |= flags;
    }
  }

  fn set_alignment(&mut self, statement: NodeId, alignment: NodeId) {
    if let Some(target) = self.declaration_of(statement) {
      match &mut self.ast.node_mut(target).data {
        NodeData::Declaration(declaration) => declaration.alignment_expression = Some(alignment),
        NodeData::Struct(structure) => structure.alignment_expression = Some(alignment),
        _ => {}
      }
    }
  }

  fn set_export_name(&mut self, statement: NodeId, name: Box<[u8]>) {
    if let Some(target) = self.declaration_of(statement)
      && let NodeData::Declaration(declaration) = &mut self.ast.node_mut(target).data
    {
      declaration.program_export_name = Some(name);
    }
  }

  fn declaration_of(&self, statement: NodeId) -> Option<NodeId> {
    match self.ast.data(statement) {
      NodeData::Declaration(_) => Some(statement),
      NodeData::CompoundDeclaration(compound) => Some(compound.declaration_properties),
      NodeData::Using(using) => self.declaration_of(using.expression),
      _ => None,
    }
  }

  fn parse_module_parameters(&mut self, start: u32) -> Option<NodeId> {
    self.bump();
    self.bump();

    let module_start = self.span().start;
    let module_list = self.parse_parameter_list()?;
    let mut module_header = ProcedureHeader::empty();
    module_header.arguments = module_list.entries;
    let module_parameters = self.push(
      self.span_from(module_start),
      NodeData::ProcedureHeader(Box::new(module_header)),
    );

    let program_parameters = if self.at(TokenKind::OPEN_PAREN) {
      let program_start = self.span().start;
      let program_list = self.parse_parameter_list()?;
      let mut program_header = ProcedureHeader::empty();
      program_header.arguments = program_list.entries;
      Some(self.push(
        self.span_from(program_start),
        NodeData::ProcedureHeader(Box::new(program_header)),
      ))
    } else {
      None
    };

    let common_code = if self.at(TokenKind::OPEN_BRACE) {
      Some(self.parse_braced_block(BlockType::DataDeclarations, BlockFlags::empty())?)
    } else {
      None
    };

    Some(self.push(
      self.span_from(start),
      NodeData::DirectiveModuleParameters(Box::new(DirectiveModuleParameters {
        module_parameters,
        program_parameters,
        common_code,
      })),
    ))
  }

  /// `#asm [features] { instruction; ... }` (**L§15**). The block is parsed
  /// rather than kept as text, because its operands name high-level variables
  /// and constants that the scope tree and the checker have to see.
  fn parse_asm(&mut self, start: u32) -> Option<NodeId> {
    self.bump();
    self.bump();

    let mut features = Vec::new();
    while self.at(TokenKind::IDENT) {
      let token = self.bump();
      if let TokenValue::Name(symbol) = token.value {
        features.push(symbol);
      }
      if !self.eat(TokenKind::COMMA) {
        break;
      }
    }

    self.expect(TokenKind::OPEN_BRACE, "'{' after '#asm'")?;
    let mut instructions = Vec::new();
    loop {
      if self.eat(TokenKind::SEMICOLON) {
        continue;
      }
      if self.at(TokenKind::CLOSE_BRACE) || self.at_end() {
        break;
      }
      let instruction = self.parse_asm_instruction()?;
      instructions.push(instruction);
      if !self.eat(TokenKind::SEMICOLON) && !self.at(TokenKind::CLOSE_BRACE) {
        self.error_here(format!(
          "Expected ';' after an '#asm' instruction, but got '{}'.",
          self.token_text()
        ));
        return None;
      }
    }
    self.expect(TokenKind::CLOSE_BRACE, "'}' to close an '#asm' block")?;

    Some(self.push(
      self.span_from(start),
      NodeData::Asm(Box::new(AsmNode {
        features,
        instructions,
      })),
    ))
  }

  fn parse_asm_instruction(&mut self) -> Option<AsmInstruction> {
    let start = self.span().start;
    // `t: gpr === a;` and `x === a;` are statements without a mnemonic: what
    // follows the name says so.
    let is_bare_operand = self.at(TokenKind::IDENT)
      && matches!(self.kind_at(1), TokenKind::COLON | TokenKind::TRIPLE_EQUALS);
    if is_bare_operand {
      let operand = self.parse_asm_operand()?;
      return Some(AsmInstruction {
        span: self.span_from(start),
        mnemonic: None,
        size: AsmSize::Inferred,
        operands: vec![operand],
      });
    }

    let token = self.expect(TokenKind::IDENT, "an '#asm' instruction mnemonic")?;
    let TokenValue::Name(mnemonic) = token.value else {
      return None;
    };
    let size = self.parse_asm_size()?;

    let mut operands = Vec::new();
    if !self.at(TokenKind::SEMICOLON) && !self.at(TokenKind::CLOSE_BRACE) {
      loop {
        operands.push(self.parse_asm_operand()?);
        if !self.eat(TokenKind::COMMA) {
          break;
        }
      }
    }
    Some(AsmInstruction {
      span: self.span_from(start),
      mnemonic: Some(mnemonic),
      size,
      operands,
    })
  }

  /// The `.64`/`.q`/`?T` a mnemonic can carry. A size written in digits
  /// reaches the parser as a float literal, since the lexer sees `.64` before
  /// it sees the mnemonic it belongs to, so its text is read back instead of
  /// its value.
  fn parse_asm_size(&mut self) -> Option<AsmSize> {
    // `mov?T [input], temp` sizes the mnemonic by `T` alone: the operand list
    // starts at the space, so the `[input]` after it is not a subscript.
    if self.eat(TokenKind::QUESTION) {
      let expression = self.parse_primary()?;
      return Some(AsmSize::Of(expression));
    }
    if self.at(TokenKind::NUMBER) {
      let span = self.span();
      let text = &self.input[span.range()];
      if let Some(digits) = text.strip_prefix(b".")
        && let Some(bits) = std::str::from_utf8(digits)
          .ok()
          .and_then(|digits| digits.parse::<u32>().ok())
      {
        self.bump();
        return Some(AsmSize::Bits(bits));
      }
    }
    if self.at(TokenKind::DOT) && self.kind_at(1) == TokenKind::IDENT {
      let letter = self.name_of(self.token_at(1));
      if let Some(bits) = asm_size_letter(letter) {
        self.bump();
        self.bump();
        return Some(AsmSize::Bits(bits));
      }
      let letter = String::from_utf8_lossy(letter).into_owned();
      self.error_here(format!("'{letter}' is not an '#asm' operand size."));
      return None;
    }
    Some(AsmSize::Inferred)
  }

  fn parse_asm_operand(&mut self) -> Option<AsmOperand> {
    let start = self.span().start;
    let kind = if self.at(TokenKind::OPEN_BRACKET) {
      AsmOperandKind::Memory(Box::new(self.parse_asm_memory()?))
    } else {
      let expression = self.parse_unary()?;
      if self.eat(TokenKind::COLON) {
        let class = self.parse_asm_class();
        let register = if self.eat(TokenKind::TRIPLE_EQUALS) {
          Some(self.parse_asm_register()?)
        } else {
          None
        };
        AsmOperandKind::Declaration(AsmDeclaration {
          name: expression,
          class,
          register,
        })
      } else if self.eat(TokenKind::TRIPLE_EQUALS) {
        AsmOperandKind::Pin {
          name: expression,
          register: self.parse_asm_register()?,
        }
      } else {
        AsmOperandKind::Expression(expression)
      }
    };

    let mask = if self.at(TokenKind::BITWISE_AND) {
      self.bump();
      let zeroing = self.eat(TokenKind::ASTERISK);
      let register = self.parse_unary()?;
      Some(AsmMask { register, zeroing })
    } else {
      None
    };

    let flag = if self.eat(TokenKind::BANG) {
      match self.kind() {
        TokenKind::IDENT => {
          let letter = self.name_of(self.token());
          match asm_rounding_mode(letter) {
            Some(mode) => {
              self.bump();
              Some(AsmFlag::Rounding(mode))
            }
            None => Some(AsmFlag::Plain),
          }
        }
        _ => Some(AsmFlag::Plain),
      }
    } else {
      None
    };

    Some(AsmOperand {
      span: self.span_from(start),
      kind,
      mask,
      flag,
    })
  }

  fn parse_asm_class(&mut self) -> Option<AsmClass> {
    if !self.at(TokenKind::IDENT) {
      return None;
    }
    let class = match self.name_of(self.token()) {
      b"gpr" => AsmClass::Gpr,
      b"str" => AsmClass::Str,
      b"vec" => AsmClass::Vec,
      b"omr" => AsmClass::Omr,
      _ => return None,
    };
    self.bump();
    Some(class)
  }

  fn parse_asm_register(&mut self) -> Option<AsmRegister> {
    if self.at(TokenKind::NUMBER) {
      let token = self.bump();
      return match token.value {
        TokenValue::Integer(number) => Some(AsmRegister::Numbered(number as u32)),
        _ => {
          self.error(token.span, "An '#asm' register number must be an integer.");
          None
        }
      };
    }
    let token = self.expect(TokenKind::IDENT, "an '#asm' register after '==='")?;
    match token.value {
      TokenValue::Name(symbol) => Some(AsmRegister::Named(symbol)),
      _ => None,
    }
  }

  /// `[base + index*scale + displacement]`, in that order and no other
  /// (**L§15**). A term written as a number or in parentheses is the
  /// displacement; anything else in the second slot is the index.
  fn parse_asm_memory(&mut self) -> Option<AsmMemory> {
    self.expect(TokenKind::OPEN_BRACKET, "'[' to start a memory operand")?;
    let by_reference = self.eat(TokenKind::ASTERISK);
    let base = self.parse_unary()?;

    let mut index = None;
    let mut scale = None;
    let mut displacement = None;
    let mut displacement_is_negative = false;
    while !self.at(TokenKind::CLOSE_BRACKET) && !self.at_end() {
      let negative = if self.eat(TokenKind::MINUS) {
        true
      } else {
        self.expect(TokenKind::PLUS, "'+' or '-' in a memory operand")?;
        false
      };
      let term = self.parse_unary()?;
      let is_displacement = negative
        || displacement.is_some()
        || index.is_some()
        || matches!(self.ast.data(term), NodeData::Literal(_))
        || self.ast.flags(term).contains(NodeFlags::IS_PARENTHESIZED);
      if is_displacement {
        displacement = Some(term);
        displacement_is_negative = negative;
        continue;
      }
      index = Some(term);
      if self.eat(TokenKind::ASTERISK) {
        scale = Some(self.parse_unary()?);
      }
    }
    self.expect(TokenKind::CLOSE_BRACKET, "']' to close a memory operand")?;

    Some(AsmMemory {
      by_reference,
      base,
      index,
      scale,
      displacement,
      displacement_is_negative,
    })
  }

  /// Whether the directive the parser stands on can appear in an expression.
  fn at_expression_directive(&self) -> bool {
    let Some(word) = self.directive() else {
      return false;
    };
    matches!(
      word,
      b"run"
        | b"assert"
        | b"insert"
        | b"code"
        | b"char"
        | b"this"
        | b"Context"
        | b"compile_time"
        | b"caller_code"
        | b"file"
        | b"filepath"
        | b"line"
        | b"caller_location"
        | b"location"
        | b"procedure_name"
        | b"exists"
        | b"import"
        | b"library"
        | b"system_library"
        | b"type"
        | b"bake_arguments"
        | b"bake_constants"
        | b"dynamic_specialize"
        | b"procedure_of_call"
        | b"modify"
        | b"asm"
        | b"bytes"
    )
  }

  fn parse_directive_expression(&mut self) -> Option<NodeId> {
    let start = self.span().start;
    let Some(word) = self.directive() else {
      self.error_here("Expected a directive after '#'.");
      return None;
    };

    match word {
      b"run" | b"assert" => self.parse_run(start, word == b"assert"),
      b"insert" => self.parse_insert(start),
      b"code" => self.parse_code(start),
      b"char" => {
        self.bump();
        self.bump();
        let token = self.expect(TokenKind::STRING, "a string after '#char'")?;
        let byte = match &token.value {
          TokenValue::Text(text) => text.first().copied().unwrap_or(0),
          _ => 0,
        };
        Some(self.push(
          self.span_from(start),
          NodeData::Literal(Literal {
            value: LiteralValue::Integer(u64::from(byte)),
            flags: LiteralFlags::IS_A_NUMBER,
          }),
        ))
      }
      b"this" => {
        self.bump();
        self.bump();
        Some(self.push(self.span_from(start), NodeData::DirectiveThis))
      }
      b"Context" => {
        self.bump();
        self.bump();
        Some(self.push(self.span_from(start), NodeData::DirectiveContextType))
      }
      b"compile_time" => {
        self.bump();
        self.bump();
        Some(self.push(self.span_from(start), NodeData::DirectiveCompileTime))
      }
      b"caller_code" => {
        self.bump();
        self.bump();
        Some(self.push(self.span_from(start), NodeData::DirectiveCallerCode))
      }
      b"file" | b"filepath" | b"line" => {
        let which = match word {
          b"file" => FileInfoKind::File,
          b"filepath" => FileInfoKind::Filepath,
          _ => FileInfoKind::Line,
        };
        self.bump();
        self.bump();
        Some(self.push(self.span_from(start), NodeData::DirectiveFileInfo { which }))
      }
      b"caller_location" => {
        self.bump();
        self.bump();
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveLocation(Box::new(DirectiveLocation {
            expression: None,
            is_caller_location: true,
            has_parentheses: false,
          })),
        ))
      }
      b"location" => {
        self.bump();
        self.bump();
        let mut expression = None;
        let mut has_parentheses = false;
        if self.eat(TokenKind::OPEN_PAREN) {
          has_parentheses = true;
          if !self.at(TokenKind::CLOSE_PAREN) {
            expression = Some(self.parse_expression()?);
          }
          self.expect(TokenKind::CLOSE_PAREN, "')'")?;
        }
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveLocation(Box::new(DirectiveLocation {
            expression,
            is_caller_location: false,
            has_parentheses,
          })),
        ))
      }
      b"procedure_name" => {
        self.bump();
        self.bump();
        self.expect(TokenKind::OPEN_PAREN, "'('")?;
        let argument = if self.at(TokenKind::CLOSE_PAREN) {
          None
        } else {
          Some(self.parse_expression()?)
        };
        self.expect(TokenKind::CLOSE_PAREN, "')'")?;
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveProcedureName { argument },
        ))
      }
      b"exists" => {
        self.bump();
        self.bump();
        self.expect(TokenKind::OPEN_PAREN, "'('")?;
        let query_expression = self.parse_expression()?;
        let sync_expression = if self.eat(TokenKind::COMMA) {
          Some(self.parse_expression()?)
        } else {
          None
        };
        self.expect(TokenKind::CLOSE_PAREN, "')'")?;
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveExists(Box::new(DirectiveExists {
            query_expression,
            sync_expression,
          })),
        ))
      }
      b"import" => self.parse_import(start),
      b"library" | b"system_library" => self.parse_library(start, word == b"system_library"),
      b"type" => self.parse_type(),
      b"bake_arguments" | b"bake_constants" | b"dynamic_specialize" => {
        let bake_type = match word {
          b"bake_arguments" => BakeType::ParameterValue,
          b"bake_constants" => BakeType::Constants,
          _ => BakeType::DynamicSpecialize,
        };
        self.bump();
        self.bump();
        let procedure_call = self.parse_unary()?;
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveBake {
            procedure_call,
            bake_type,
          },
        ))
      }
      b"procedure_of_call" => {
        self.bump();
        self.bump();
        let call = self.parse_unary()?;
        if let NodeData::ProcedureCall(inner) = &mut self.ast.node_mut(call).data {
          inner.flags |= CallFlags::RETURNS_PROCEDURE_POINTER_ONLY;
        }
        Some(call)
      }
      b"modify" => {
        self.bump();
        self.bump();
        let block = self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())?;
        Some(self.push(self.span_from(start), NodeData::DirectiveModify { block }))
      }
      b"asm" => self.parse_asm(start),
      b"bytes" => {
        self.bump();
        self.bump();
        let expression = self.parse_expression()?;
        Some(self.push(
          self.span_from(start),
          NodeData::DirectiveBytes { expression },
        ))
      }
      b"if" => {
        self.bump();
        self.parse_if(true)
      }
      _ => {
        self.error_here(format!(
          "Unknown directive '#{}'.",
          String::from_utf8_lossy(word)
        ));
        None
      }
    }
  }

  fn parse_run(&mut self, start: u32, is_assertion: bool) -> Option<NodeId> {
    self.bump();
    self.bump();

    let mut flags = if is_assertion {
      RunFlags::ASSERTION
    } else {
      RunFlags::empty()
    };
    loop {
      if self.eat_modifier(b"stallable") {
        flags |= RunFlags::STALLABLE;
      } else if self.eat_modifier(b"host") {
        flags |= RunFlags::HOST;
      } else {
        break;
      }
    }

    let mut header = ProcedureHeader::empty();
    if !is_assertion && self.eat(TokenKind::RIGHT_ARROW) {
      let returns = self.parse_return_types()?;
      header.returns = returns.declarations;
      header.parenthesized_returns = returns.parenthesized;
    } else {
      flags |= RunFlags::HAS_IMPLICIT_RETURN_TYPES;
    }

    // `#assert(cond, "message")` is the parenthesized form of **L§12.2**; a
    // plain parenthesized condition has no comma inside.
    let mut assertion_message = None;
    let block_start = self.span().start;
    let block = if is_assertion && self.at(TokenKind::OPEN_PAREN) && self.paren_holds_a_comma() {
      self.bump();
      let condition = self.parse_expression()?;
      if self.eat(TokenKind::COMMA) {
        assertion_message = Some(self.parse_expression()?);
      }
      self.expect(TokenKind::CLOSE_PAREN, "')'")?;
      self.push(
        self.span_from(block_start),
        NodeData::Block(Block {
          block_type: BlockType::Imperative,
          block_flags: BlockFlags::empty(),
          statements: vec![condition],
        }),
      )
    } else if self.at(TokenKind::OPEN_BRACE) {
      self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())?
    } else {
      let mut statement = self.parse_expression()?;
      // `#run stmt_or_block;` takes a whole *statement* (**L§6.11**), so an
      // assignment written after it is part of the run rather than something
      // done to what the run produced: `#run counter += 1;` counts at compile
      // time. Nothing else can follow an expression with one of these.
      if !is_assertion && let Some(operator) = self.assignment_operator() {
        self.bump();
        let value = self.parse_assignment_value(block_start)?;
        statement = self.push(
          self.span_from(block_start),
          NodeData::BinaryOperator {
            operator,
            flags: BinaryFlags::empty(),
            left: statement,
            right: value,
          },
        );
      }
      self.push(
        self.span_from(block_start),
        NodeData::Block(Block {
          block_type: BlockType::Imperative,
          block_flags: BlockFlags::empty(),
          statements: vec![statement],
        }),
      )
    };

    let assertion_string = if assertion_message.is_some() {
      assertion_message
    } else if is_assertion && self.at(TokenKind::STRING) {
      let token = self.bump();
      let text = match token.value {
        TokenValue::Text(text) => text,
        _ => Box::default(),
      };
      let mut literal_flags = LiteralFlags::empty();
      if token.flags.contains(ValueFlags::HERE_STRING) {
        literal_flags |= LiteralFlags::HERE_STRING;
      }
      Some(self.push(
        token.span,
        NodeData::Literal(Literal {
          value: LiteralValue::Text(text),
          flags: literal_flags,
        }),
      ))
    } else {
      None
    };

    let header_id = self.push(
      self.span_from(start),
      NodeData::ProcedureHeader(Box::new(header)),
    );
    let body = self.push(
      self.span_from(block_start),
      NodeData::ProcedureBody {
        header: header_id,
        block,
      },
    );
    if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
      header.body_or_null = Some(body);
    }

    Some(self.push(
      self.span_from(start),
      NodeData::DirectiveRun(Box::new(DirectiveRun {
        procedure: header_id,
        flags,
        assertion_string,
      })),
    ))
  }

  fn parse_insert(&mut self, start: u32) -> Option<NodeId> {
    self.bump();
    self.bump();

    let mut scope_redirection = None;
    let mut has_scope_redirection = false;
    if self.at_modifier(b"scope") {
      self.bump();
      self.bump();
      has_scope_redirection = true;
      self.expect(TokenKind::OPEN_PAREN, "'('")?;
      if !self.at(TokenKind::CLOSE_PAREN) {
        scope_redirection = Some(self.parse_expression()?);
      }
      self.expect(TokenKind::CLOSE_PAREN, "')'")?;
    }

    let mut break_replacement = None;
    let mut continue_replacement = None;
    let mut remove_replacement = None;
    if self.at(TokenKind::OPEN_PAREN) {
      self.bump();
      while !self.at(TokenKind::CLOSE_PAREN) && !self.at_end() {
        let keyword = self.bump();
        self.expect(TokenKind::EQUALS, "'=' in an '#insert' remapping")?;
        let replacement = match self.kind() {
          TokenKind::KEYWORD_BREAK => self.parse_loop_control(LoopControlType::Break)?,
          TokenKind::KEYWORD_CONTINUE => self.parse_loop_control(LoopControlType::Continue)?,
          TokenKind::KEYWORD_REMOVE => self.parse_loop_control(LoopControlType::Remove)?,
          TokenKind::OPEN_BRACE => {
            self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())?
          }
          _ => self.parse_expression()?,
        };
        match keyword.kind {
          TokenKind::KEYWORD_BREAK => break_replacement = Some(replacement),
          TokenKind::KEYWORD_CONTINUE => continue_replacement = Some(replacement),
          TokenKind::KEYWORD_REMOVE => remove_replacement = Some(replacement),
          _ => self.error(
            keyword.span,
            "Expected 'break', 'continue' or 'remove' in an '#insert' remapping.",
          ),
        }
        if !self.eat(TokenKind::COMMA) {
          break;
        }
      }
      self.expect(TokenKind::CLOSE_PAREN, "')'")?;
    }

    let expression = if self.at(TokenKind::RIGHT_ARROW) {
      self.parse_implicit_run(start)?
    } else {
      self.parse_expression()?
    };

    Some(self.push(
      self.span_from(start),
      NodeData::DirectiveInsert(Box::new(DirectiveInsert {
        expression,
        scope_redirection,
        has_scope_redirection,
        break_replacement,
        continue_replacement,
        remove_replacement,
      })),
    ))
  }

  /// `#insert -> T { ... }` is a `#run` the parser writes out for the user
  /// (`SYNTACTICALLY_IMPLICIT`, **C§5.4**).
  fn parse_implicit_run(&mut self, start: u32) -> Option<NodeId> {
    self.expect(TokenKind::RIGHT_ARROW, "'->'")?;
    let returns = self.parse_return_types()?;

    let mut header = ProcedureHeader::empty();
    header.returns = returns.declarations;
    header.parenthesized_returns = returns.parenthesized;

    let block_start = self.span().start;
    let block = self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())?;
    let header_id = self.push(
      self.span_from(start),
      NodeData::ProcedureHeader(Box::new(header)),
    );
    let body = self.push(
      self.span_from(block_start),
      NodeData::ProcedureBody {
        header: header_id,
        block,
      },
    );
    if let NodeData::ProcedureHeader(header) = &mut self.ast.node_mut(header_id).data {
      header.body_or_null = Some(body);
    }

    Some(self.push(
      self.span_from(start),
      NodeData::DirectiveRun(Box::new(DirectiveRun {
        procedure: header_id,
        flags: RunFlags::SYNTACTICALLY_IMPLICIT,
        assertion_string: None,
      })),
    ))
  }

  fn parse_code(&mut self, start: u32) -> Option<NodeId> {
    self.bump();
    self.bump();

    let mut flags = CodeFlags::empty();
    if self.eat_modifier(b"typed") {
      flags |= CodeFlags::TYPED;
    } else if self.at(TokenKind::COMMA) && self.kind_at(1) == TokenKind::KEYWORD_NULL {
      self.bump();
      self.bump();
      flags |= CodeFlags::NULL;
      return Some(self.push(
        self.span_from(start),
        NodeData::DirectiveCode {
          expression: None,
          flags,
        },
      ));
    }

    // `#code` wraps a statement as readily as an expression: `#code x = 3;`
    // and `#code #add_context c: int;` are both legal (**L§13.1**).
    let expression = if self.at(TokenKind::OPEN_BRACE) {
      self.parse_braced_block(BlockType::Imperative, BlockFlags::empty())?
    } else if self.at(TokenKind::HASH) && !self.at_expression_directive() {
      self.parse_statement(BlockType::Imperative)?
    } else {
      let statement_start = self.position;
      let expression = self.parse_expression()?;
      if self.at(TokenKind::EQUALS) || self.at(TokenKind::COLON) {
        self.position = statement_start;
        self.parse_declaration_statement()?
      } else {
        expression
      }
    };

    Some(self.push(
      self.span_from(start),
      NodeData::DirectiveCode {
        expression: Some(expression),
        flags,
      },
    ))
  }

  fn parse_import(&mut self, start: u32) -> Option<NodeId> {
    self.bump();
    self.bump();

    let mut flags = ImportFlags::empty();
    let mut import_type = ImportType::ShortName;
    loop {
      if self.eat_modifier(b"file") {
        import_type = ImportType::PathToFile;
      } else if self.eat_modifier(b"dir") {
        import_type = ImportType::PathToDirectory;
      } else if self.eat_modifier(b"string") {
        import_type = ImportType::FullText;
      } else if self.eat_modifier(b"unshared") {
        flags |= ImportFlags::UNSHARED;
      } else {
        break;
      }
    }

    let name = self.string_value("a module name after '#import'")?;

    let module_parameters = if self.at(TokenKind::OPEN_PAREN) {
      self.bump();
      let arguments = self.parse_argument_list(TokenKind::CLOSE_PAREN)?;
      self.expect(TokenKind::CLOSE_PAREN, "')'")?;
      Some(arguments)
    } else {
      None
    };
    let program_parameters = if self.at(TokenKind::OPEN_PAREN) {
      self.bump();
      let arguments = self.parse_argument_list(TokenKind::CLOSE_PAREN)?;
      self.expect(TokenKind::CLOSE_PAREN, "')'")?;
      Some(arguments)
    } else {
      None
    };

    Some(self.push(
      self.span_from(start),
      NodeData::DirectiveImport(Box::new(DirectiveImport {
        name,
        flags,
        import_type,
        module_parameters,
        program_parameters,
      })),
    ))
  }

  fn parse_library(&mut self, start: u32, is_system: bool) -> Option<NodeId> {
    self.bump();
    self.bump();

    let mut library_flags = if is_system {
      LibraryFlags::IS_SYSTEM_LIBRARY
    } else {
      LibraryFlags::empty()
    };
    loop {
      if self.eat_modifier(b"system") {
        library_flags |= LibraryFlags::IS_SYSTEM_LIBRARY;
      } else if self.eat_modifier(b"no_dll") {
        library_flags |= LibraryFlags::DYNAMIC_LIBRARY_UNAVAILABLE;
      } else if self.eat_modifier(b"no_static_library") {
        library_flags |= LibraryFlags::STATIC_LIBRARY_UNAVAILABLE;
      } else if self.eat_modifier(b"link_always") {
        library_flags |= LibraryFlags::LINK_ALWAYS;
      } else {
        break;
      }
    }

    let name = self.string_value("a library name")?;
    Some(self.push(
      self.span_from(start),
      NodeData::DirectiveLibrary {
        name,
        library_flags,
      },
    ))
  }
}

/// The letter spellings of an `#asm` operand size, which are still accepted
/// alongside the digits (**L§15**).
fn asm_size_letter(name: &[u8]) -> Option<u32> {
  Some(match name {
    b"b" => 8,
    b"w" => 16,
    b"d" => 32,
    b"q" => 64,
    b"x" => 128,
    b"y" => 256,
    b"z" => 512,
    _ => return None,
  })
}

fn asm_rounding_mode(name: &[u8]) -> Option<RoundingMode> {
  Some(match name {
    b"n" => RoundingMode::Nearest,
    b"d" => RoundingMode::Down,
    b"u" => RoundingMode::Up,
    b"z" => RoundingMode::Zero,
    _ => return None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use oj_diag::Severity;

  fn parsed(source: &str) -> (Parsed, Interner) {
    let interner = Interner::new();
    let parsed = parse(source.as_bytes(), SourceId(0), &interner);
    (parsed, interner)
  }

  /// The canonical source form of a snippet: parse it, print it, and check that
  /// the printed form parses back to the very same tree.
  fn printed(source: &str) -> String {
    let (parsed, interner) = parsed(source);
    let errors: Vec<&Diagnostic> = parsed
      .diagnostics
      .iter()
      .filter(|diagnostic| diagnostic.severity == Severity::Error)
      .collect();
    assert!(errors.is_empty(), "{source} did not parse: {errors:?}");

    let printed = crate::print::print_source(&parsed.ast, parsed.root, &interner);
    let before = crate::tree::print_tree(&parsed.ast, parsed.root, &interner);

    let reparsed = parse(printed.as_bytes(), SourceId(1), &interner);
    let after = crate::tree::print_tree(&reparsed.ast, reparsed.root, &interner);
    assert_eq!(
      before, after,
      "{source} did not round-trip through {printed}"
    );
    printed
  }

  fn errors(source: &str) -> Vec<String> {
    let (parsed, _) = parsed(source);
    parsed
      .diagnostics
      .iter()
      .filter(|diagnostic| diagnostic.severity == Severity::Error)
      .map(|diagnostic| diagnostic.message.clone())
      .collect()
  }

  fn tree(source: &str) -> String {
    let (parsed, interner) = parsed(source);
    crate::tree::print_tree(&parsed.ast, parsed.root, &interner)
  }

  #[test]
  fn declarations_take_all_the_forms_of_l_4_1() {
    assert_eq!(printed("x: int = 5;"), "x: int = 5;\n");
    assert_eq!(printed("x: int;"), "x: int;\n");
    assert_eq!(printed("x := 5;"), "x := 5;\n");
    assert_eq!(printed("x: int : 5;"), "x: int : 5;\n");
    assert_eq!(printed("x :: 5;"), "x :: 5;\n");
    assert_eq!(printed("x: int = ---;"), "x: int = ---;\n");
    assert_eq!(printed("x: *[4] Thing;"), "x: *[4] Thing;\n");
    assert_eq!(printed("x: [] int;"), "x: [] int;\n");
    assert_eq!(printed("x: [..] int;"), "x: [..] int;\n");
    assert_eq!(
      printed("x: #type,distinct u32;"),
      "x: #type,distinct u32;\n"
    );
  }

  #[test]
  fn compound_declarations_keep_their_per_name_modifiers() {
    assert_eq!(printed("x, y, z: float;"), "x, y, z: float;\n");
    assert_eq!(printed("a, b := 1, 2;"), "a, b := 1, 2;\n");
    assert_eq!(printed("i, j = x + 1, y + 1;"), "i, j = x + 1, y + 1;\n");
    assert_eq!(printed("a, b= , c := 1, 2, 3;"), "a, b=, c := 1, 2, 3;\n");
    assert_eq!(printed("a, d: , c = 4, 5, 6;"), "a, d:, c = 4, 5, 6;\n");
    assert_eq!(printed("a, b += 1;"), "a, b += 1;\n");
  }

  #[test]
  fn precedence_follows_l_5_1() {
    assert_eq!(printed("x := a + b * c;"), "x := a + b * c;\n");
    assert_eq!(printed("x := (a + b) * c;"), "x := (a + b) * c;\n");
    assert_eq!(
      printed("x := mode & S_IFMT == S_IFDIR;"),
      "x := mode & S_IFMT == S_IFDIR;\n"
    );
    assert!(tree("x := a + b * c;").contains("left: IDENT a"));
    assert!(tree("x := a + b * c;").contains("right: BINARY_OPERATOR *"));
  }

  #[test]
  fn a_leading_shift_left_token_is_a_dereference() {
    assert_eq!(printed("x := <<p;"), "x := (.*) p;\n");
    assert_eq!(printed("x := a << b;"), "x := a << b;\n");
    assert_eq!(printed("x := a >>,logical b;"), "x := a >>,logical b;\n");
    assert_eq!(printed("x := p.*;"), "x := p.*;\n");
    assert_eq!(printed("x := (.*) p;"), "x := (.*) p;\n");
  }

  #[test]
  fn the_three_cast_syntaxes_of_l_5_6_all_parse() {
    assert_eq!(printed("x := cast(u8) y;"), "x := cast(u8) y;\n");
    assert_eq!(
      printed("x := cast,trunc(u32) a ^ b;"),
      "x := cast,trunc(u32) a ^ b;\n"
    );
    assert_eq!(printed("x := cast(T, y);"), "x := cast(T, y);\n");
    assert_eq!(
      printed("x := cast(T, y, trunc);"),
      "x := cast(T, y, trunc);\n"
    );
    assert_eq!(printed("x := y.(*u8);"), "x := y.(*u8);\n");
    assert_eq!(printed("x := xx y;"), "x := xx y;\n");
    assert_eq!(printed("x := xx,no_check y;"), "x := xx,no_check y;\n");
    assert_eq!(printed("x := cast(T).* y;"), "x := cast(T) .*y;\n");
  }

  #[test]
  fn statements_cover_l_6() {
    assert_eq!(printed("f :: () { x();\n }"), "f :: () {\n    x();\n}\n");
    assert!(printed("f :: () { if a then b(); }").contains("if a then b();"));
    assert!(printed("f :: () { if a { b(); } else { c(); } }").contains("} else {"));
    assert!(printed("f :: () { while x < 3 { x += 1; } }").contains("while x < 3 {"));
    assert!(printed("f :: () { defer free(p); }").contains("defer free(p);"));
    assert!(printed("f :: () { return 1, 2; }").contains("return 1, 2;"));
    assert!(printed("f :: () { break outer; }").contains("break outer;"));
    assert!(printed("f :: () { push_context c { g(); } }").contains("push_context c {"));
    assert!(printed("f :: () { using v; }").contains("using v;"));
  }

  #[test]
  fn the_switch_form_of_l_6_4_is_an_if_with_cases() {
    let source = "f :: () { if #complete x == { case .A; g(); #through; case; h(); } }";
    let printed = printed(source);
    assert!(printed.contains("if #complete x == {"));
    assert!(printed.contains("case .A;"));
    assert!(printed.contains("#through;"));
    assert!(printed.contains("case;"));
    assert!(tree(source).contains("IF IfFlags(IS_SWITCH_STATEMENT | MARKED_AS_COMPLETE)"));
  }

  #[test]
  fn for_loops_keep_their_modifiers_and_names() {
    assert!(printed("f :: () { for 0..7 { g(); } }").contains("for 0..7 {"));
    assert!(printed("f :: () { for i: 0..n-1 { g(); } }").contains("for i: 0..n - 1 {"));
    assert!(printed("f :: () { for < array { g(); } }").contains("for < array {"));
    assert!(printed("f :: () { for <* array { g(); } }").contains("for <* array {"));
    assert!(printed("f :: () { for #v2 < a..b { g(); } }").contains("for #v2 < a..b {"));
    assert!(printed("f :: () { for :named v, i: c { g(); } }").contains("for :named v, i: c {"));
    assert!(
      printed("f :: () { for `it, `it_index: a { g(); } }").contains("for `it, `it_index: a {")
    );
  }

  #[test]
  fn procedure_headers_carry_their_directives() {
    assert_eq!(
      printed("f :: (a: int, b := 2) -> int { return a; }"),
      "f :: (a: int, b := 2) -> int {\n    return a;\n}\n"
    );
    assert_eq!(
      printed("f :: (a: s32) -> s32 #foreign libc \"real\";"),
      "f :: (a: s32) -> s32 #foreign libc \"real\";\n"
    );
    assert!(printed("f :: () #expand { g(); }").contains("#expand"));
    assert!(printed("f :: (x: $T/interface I) -> T { return x; }").contains("$T/interface I"));
    assert!(
      printed("f :: (a: int) -> (x: int, y: string) { return 1, \"\"; }")
        .contains("-> (x: int, y: string)")
    );
    assert!(printed("f :: inline () { g(); }").starts_with("f :: inline ("));
    assert!(printed("f :: () -> int #must { return 1; }").contains("-> int #must"));
    assert!(printed("f :: (using v: V, $T: Type) { g(); }").contains("using v: V, $T: Type"));
  }

  #[test]
  fn quick_lambdas_keep_their_untyped_parameters() {
    assert_eq!(
      printed("square :: x => x * x;"),
      "square :: (x) => x * x;\n"
    );
    assert_eq!(
      printed("sum :: (x, total) => x + total;"),
      "sum :: (x, total) => x + total;\n"
    );
    assert!(printed("f :: x => { g(x); };").contains("(x) => {"));
  }

  #[test]
  fn structs_and_enums_keep_their_directives_and_notes() {
    assert!(printed("S :: struct { a: int; b, c: float; }").contains("struct {"));
    assert!(printed("S :: struct #no_padding { a: int; }").contains("#no_padding"));
    assert!(printed("S :: struct { a: int; } @Note").contains("@Note"));
    assert!(
      printed("S :: struct (T: Type, N: s64) { a: [N] T; }").contains("struct (T: Type, N: s64)")
    );
    assert!(printed("S :: union { a: u32; b: float; }").contains("union {"));
    assert!(printed("E :: enum u8 { A; B :: 5; }").contains("enum u8 {"));
    assert!(printed("E :: enum_flags #specified { A :: 1; }").contains("enum_flags #specified"));
    assert!(printed("E :: enum @Hi { A; }").contains("enum @Hi"));
    assert!(printed("S :: struct { #place a; b: int; }").contains("#place a;"));
    assert!(printed("S :: struct { #as using base: Base; }").contains("using #as base: Base;"));
  }

  #[test]
  fn directives_that_are_statements_round_trip() {
    assert_eq!(printed("#import \"Basic\";"), "#import \"Basic\";\n");
    assert_eq!(
      printed("Math :: #import \"Math\"(F=1)(G=2);"),
      "Math :: #import \"Math\"(F = 1)(G = 2);\n"
    );
    assert_eq!(printed("#load \"other.jai\";"), "#load \"other.jai\";\n");
    assert_eq!(printed("#scope_file"), "#scope_file\n");
    assert_eq!(printed("#placeholder NAME;"), "#placeholder NAME;\n");
    assert_eq!(printed("#poke_name Mod name;"), "#poke_name Mod name;\n");
    assert_eq!(printed("#add_context x: int;"), "#add_context x: int;\n");
    assert_eq!(
      printed("libc :: #system_library \"libc\";"),
      "libc :: #library,system \"libc\";\n"
    );
    assert_eq!(printed("#run f();"), "#run f();\n");
    assert_eq!(
      printed("#assert x > 0 \"nope\";"),
      "#assert x > 0 \"nope\";\n"
    );
    assert_eq!(
      printed("#assert(x > 0, \"nope\");"),
      "#assert x > 0 \"nope\";\n"
    );
    assert!(
      printed("#if OS == .LINUX { x :: 1; } else { x :: 2; }").starts_with("#if OS == .LINUX {")
    );
    assert_eq!(printed("#insert code;"), "#insert code;\n");
    assert!(printed("#insert,scope() code;").contains(",scope()"));
    assert!(printed("x :: #run -> string { return \"\"; }").contains("#run -> string {"));
  }

  #[test]
  fn literals_keep_their_lexical_shape() {
    assert_eq!(printed("x :: 0xff;"), "x :: 0xff;\n");
    assert_eq!(printed("x :: 0b101;"), "x :: 0b101;\n");
    assert_eq!(printed("x :: 1.5;"), "x :: 1.5;\n");
    assert_eq!(printed("x :: 1.5e-3;"), "x :: 0.0015;\n");
    assert_eq!(printed("x :: 0hff800000;"), "x :: 0hff800000;\n");
    assert_eq!(printed("x :: \"a\\nb\";"), "x :: \"a\\nb\";\n");
    assert_eq!(printed("x :: #char \"A\";"), "x :: 65;\n");
    assert_eq!(printed("x :: .{a = 1, b = 2};"), "x :: .{a = 1, b = 2};\n");
    assert_eq!(printed("x :: int.[1, 2];"), "x :: int.[1, 2];\n");
    assert_eq!(printed("x :: (*u8).[];"), "x :: (*u8).[];\n");
    assert_eq!(
      printed("x :: Vector3.{1, 2, 3};"),
      "x :: Vector3.{1, 2, 3};\n"
    );
  }

  #[test]
  fn a_here_string_is_written_back_as_a_here_string() {
    let printed = printed("x :: #string DONE\nhello\nDONE\n");
    assert!(printed.starts_with("x :: #string OJ_STRING\nhello\n"));
  }

  #[test]
  fn calls_keep_named_arguments_spreads_and_context_arguments() {
    assert!(printed("f :: () { g(a, name = b); }").contains("g(a, name = b);"));
    assert!(printed("f :: () { g(..array); }").contains("g(..array);"));
    assert!(printed("f :: () { g(a,, allocator = temp); }").contains("g(a,, allocator = temp);"));
    assert!(printed("f :: () { inline g(); }").contains("inline g();"));
    assert!(printed("f :: () { g() #no_debug; }").contains("g() #no_debug;"));
  }

  #[test]
  fn operator_overloads_are_declarations_named_by_the_operator() {
    assert!(printed("operator + :: (a: V, b: V) -> V { return a; }").starts_with("operator + ::"));
    assert!(
      printed("operator [] :: (a: V, i: int) -> int { return 0; }").starts_with("operator [] ::")
    );
    assert!(printed("f :: () { g :: Basic.operator-; }").contains("Basic.operator -"));
  }

  #[test]
  fn a_missing_semicolon_is_reported_once_and_parsing_continues() {
    let source = "x := 1\ny := 2;\nz := 3;\n";
    let messages = errors(source);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].starts_with("Expected ';'"));

    let (parsed, _) = parsed(source);
    let NodeData::Block(block) = parsed.ast.data(parsed.root) else {
      panic!("the file is a block");
    };
    assert_eq!(block.statements.len(), 3);
  }

  #[test]
  fn broken_statements_report_where_they_broke() {
    assert!(errors("f :: () { if x; }")[0].contains("cannot be followed directly by ';'"));
    assert!(errors("x := ;")[0].contains("Expected an expression"));
    assert!(errors("f :: () { g(a; }")[0].contains("Expected ')'"));
    assert!(errors("#frobnicate x;")[0].contains("Unknown directive"));
  }
  #[test]
  fn asm_blocks_keep_their_operands_as_expressions() {
    assert_eq!(
      printed("f :: () { #asm { mov apple:, 10; banana: gpr; mov.64 banana, apple; } }"),
      "f :: () {\n    #asm {\n        mov apple:, 10;\n        banana: gpr;\n        mov.64 banana, apple;\n    }\n}\n"
    );
  }

  #[test]
  fn asm_sizes_are_written_in_bits_whichever_spelling_they_had() {
    assert!(
      printed("f :: () { #asm { mov.q a:, 1; mov.8 b:, 2; popcnt?T c, d; } }")
        .contains("mov.64 a:, 1;")
    );
    assert!(printed("f :: () { #asm { movdqu.x v:, w; } }").contains("movdqu.128 v:, w;"));
    assert!(printed("f :: () { #asm { popcnt?BITS r, v; } }").contains("popcnt?BITS r, v;"));
  }

  #[test]
  fn asm_registers_are_pinned_by_name_or_by_number() {
    let printed =
      printed("f :: () { #asm { t: gpr === a; v: vec === 9; x === a; mov w: gpr === 15, 10; } }");
    assert!(printed.contains("t: gpr === a;"));
    assert!(printed.contains("v: vec === 9;"));
    assert!(printed.contains("x === a;"));
    assert!(printed.contains("mov w: gpr === 15, 10;"));
  }

  #[test]
  fn asm_memory_operands_keep_the_rigid_order_of_l_15() {
    let printed = printed(
      "f :: () { #asm { mov t:, [b]; mov t, [b + 10]; mov t, [b - 10]; mov t, [b + i]; \
       mov t, [b + i*4 + 10]; mov t, [*p + 8]; mov t, [b + (SIZE + 1)]; } }",
    );
    assert!(printed.contains("mov t:, [b];"));
    assert!(printed.contains("mov t, [b + 10];"));
    assert!(printed.contains("mov t, [b - 10];"));
    assert!(printed.contains("mov t, [b + i];"));
    assert!(printed.contains("mov t, [b + i*4 + 10];"));
    assert!(printed.contains("mov t, [*p + 8];"));
    assert!(printed.contains("mov t, [b + (SIZE + 1)];"));
  }

  #[test]
  fn asm_operands_carry_their_evex_masks_and_flags() {
    let printed =
      printed("f :: () { #asm AVX512F { cvtps2dq v1:, [ptr]!; cvtps2dq v5: &* mask, v4 !z; } }");
    assert!(printed.contains("#asm AVX512F {"));
    assert!(printed.contains("cvtps2dq v1:, [ptr]!;"));
    assert!(printed.contains("cvtps2dq v5: &* mask, v4 !z;"));
  }
}

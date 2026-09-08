use std::fmt::Write as _;

use oj_lexer::{Interner, Symbol};

use crate::ast::*;
use crate::rules::needs_semicolon;

const INDENT: &str = "    ";

/// Prints a tree back as Jai source, in the shape `Program_Print` uses
/// (**C§5.3**). Re-parsing the result yields the same tree, which is what the
/// corpus round-trip test checks.
pub fn print_source(ast: &Ast, root: NodeId, interner: &Interner) -> String {
  let mut printer = Printer {
    ast,
    interner,
    out: String::new(),
    depth: 0,
  };
  printer.file(root);
  printer.out
}

struct Printer<'a> {
  ast: &'a Ast,
  interner: &'a Interner,
  out: String,
  depth: usize,
}

/// How the statements of an unbraced block are written: as statements ending in
/// `;`, or as the single value of an `ifx` branch or a quick lambda.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockForm {
  Statements,
  Value,
}

impl Printer<'_> {
  fn write(&mut self, text: &str) {
    self.out.push_str(text);
  }

  fn indent(&mut self) {
    for _ in 0..self.depth {
      self.out.push_str(INDENT);
    }
  }

  fn name(&mut self, symbol: Symbol) {
    let bytes = self.interner.resolve(symbol);
    self.out.push_str(&String::from_utf8_lossy(bytes));
  }

  fn file(&mut self, root: NodeId) {
    let NodeData::Block(block) = self.ast.data(root) else {
      return;
    };
    for statement in block.statements.clone() {
      self.statement(statement);
    }
  }

  fn statement(&mut self, statement: NodeId) {
    self.indent();
    self.expression(statement);
    if needs_semicolon(self.ast, statement) {
      self.write(";");
    }
    self.write("\n");
  }

  fn block(&mut self, id: NodeId, form: BlockForm) {
    let NodeData::Block(block) = self.ast.data(id) else {
      self.expression(id);
      return;
    };

    if block
      .block_flags
      .contains(BlockFlags::NO_ARRAY_BOUNDS_CHECK)
    {
      self.write("#no_abc ");
    }
    if block
      .block_flags
      .contains(BlockFlags::NO_ARITHMETIC_OVERFLOW_CHECK)
    {
      self.write("#no_aoc ");
    }

    let braced = self.ast.flags(id).contains(NodeFlags::IS_PARENTHESIZED);
    let statements = block.statements.clone();

    if !braced {
      match form {
        BlockForm::Value => {
          if let Some(first) = statements.first() {
            self.expression(*first);
          }
        }
        BlockForm::Statements => {
          for statement in &statements {
            self.expression(*statement);
            if needs_semicolon(self.ast, *statement) {
              self.write(";");
            }
          }
        }
      }
      return;
    }

    self.write("{\n");
    self.depth += 1;
    for statement in &statements {
      self.statement(*statement);
    }
    self.depth -= 1;
    self.indent();
    self.write("}");
  }

  fn expression(&mut self, id: NodeId) {
    let flags = self.ast.flags(id);
    if flags.contains(NodeFlags::EXPRESSION_IS_SPREAD) {
      self.write("..");
    }

    let parenthesized = flags.contains(NodeFlags::IS_PARENTHESIZED)
      && !matches!(self.ast.data(id), NodeData::Block(_));
    if parenthesized {
      self.write("(");
    }
    self.node(id);
    if parenthesized {
      self.write(")");
    }
  }

  fn node(&mut self, id: NodeId) {
    match self.ast.data(id) {
      NodeData::Block(_) => self.block(id, BlockForm::Statements),
      NodeData::Literal(literal) => self.literal(literal),
      NodeData::Ident(ident) => {
        let ident = ident.clone();
        self.ident(&ident);
      }
      NodeData::UnaryOperator { operator, operand } => {
        let (operator, operand) = (*operator, *operand);
        if operator == OperatorType::POSTFIX_DEREFERENCE {
          self.expression(operand);
          self.write(".*");
        } else if operator == OperatorType::POINTER_DEREFERENCE {
          self.write("(.*) ");
          self.expression(operand);
        } else {
          self.write(operator.text());
          self.expression(operand);
        }
      }
      NodeData::BinaryOperator {
        operator,
        flags,
        left,
        right,
      } => {
        let (operator, flags, left, right) = (*operator, *flags, *left, *right);
        self.expression(left);
        if operator == OperatorType::ARRAY_SUBSCRIPT {
          self.write("[");
          self.expression(right);
          self.write("]");
          return;
        }
        if operator == OperatorType::DOT {
          self.write(".");
          self.expression(right);
          return;
        }
        self.write(" ");
        self.write(operator.text());
        if flags.contains(BinaryFlags::SHIFT_MARKED_AS_LOGICAL) {
          self.write(",logical");
        }
        if flags.contains(BinaryFlags::SHIFT_MARKED_AS_SMALL) {
          self.write(",small");
        }
        self.write(" ");
        self.expression(right);
      }
      NodeData::ProcedureBody { block, .. } => {
        let block = *block;
        self.block(block, BlockForm::Statements);
      }
      NodeData::ProcedureCall(call) => {
        let call = call.clone();
        self.call(&call);
      }
      NodeData::Context => self.write("context"),
      NodeData::While { condition, block } => {
        let (condition, block) = (*condition, *block);
        self.write("while ");
        self.expression(condition);
        self.write(" ");
        self.block(block, BlockForm::Statements);
      }
      NodeData::If(node) => {
        let node = node.clone();
        self.if_node(&node);
      }
      NodeData::LoopControl {
        control_type,
        target_ident,
      } => {
        let (control_type, target_ident) = (*control_type, *target_ident);
        self.write(match control_type {
          LoopControlType::Break => "break",
          LoopControlType::Continue => "continue",
          LoopControlType::Remove => "remove",
        });
        if let Some(target) = target_ident {
          self.write(" ");
          self.expression(target);
        }
      }
      NodeData::Case(case) => {
        let case = case.clone();
        match case.condition {
          Some(condition) => {
            self.write("case ");
            self.expression(condition);
            self.write(";");
          }
          None => self.write("case;"),
        }
        let NodeData::Block(block) = self.ast.data(case.then_block) else {
          return;
        };
        self.depth += 1;
        for statement in block.statements.clone() {
          self.write("\n");
          self.indent();
          self.expression(statement);
          if needs_semicolon(self.ast, statement) {
            self.write(";");
          }
        }
        if case.marked_as_fallthrough {
          self.write("\n");
          self.indent();
          self.write("#through;");
        }
        self.depth -= 1;
      }
      NodeData::Return { arguments, flags } => {
        let (arguments, flags) = (arguments.clone(), *flags);
        if flags.contains(ReturnFlags::AUTO_INSERTED_FOR_QUICK_LAMBDA) {
          self.arguments(&arguments);
          return;
        }
        if flags.contains(ReturnFlags::IS_BACKTICKED) {
          self.write("`");
        }
        self.write("return");
        if !arguments.is_empty() {
          self.write(" ");
          self.arguments(&arguments);
        }
      }
      NodeData::For(node) => {
        let node = node.clone();
        self.for_node(&node);
      }
      NodeData::TypeInstantiation(inst) => {
        let inst = inst.clone();
        self.type_instantiation(&inst);
      }
      NodeData::Enum(node) => {
        let node = node.clone();
        self.enum_node(&node);
      }
      NodeData::ProcedureHeader(header) => {
        let header = header.clone();
        self.procedure_header(&header);
      }
      NodeData::Struct(node) => {
        let node = node.clone();
        self.struct_node(&node);
      }
      NodeData::CommaSeparatedArguments { arguments } => {
        let arguments = arguments.clone();
        self.comma_separated(&arguments);
      }
      NodeData::DirectiveBytes { expression } => {
        let expression = *expression;
        self.write("#bytes ");
        self.expression(expression);
      }
      NodeData::Declaration(_) => self.declaration(id, true),
      NodeData::Cast(cast) => {
        let cast = cast.clone();
        self.cast(&cast);
      }
      NodeData::DirectiveImport(import) => {
        let import = import.clone();
        self.import(&import);
      }
      NodeData::DirectiveThis => self.write("#this"),
      NodeData::DirectiveThrough => self.write("#through"),
      NodeData::DirectiveLoad { name } => {
        let name = name.clone();
        self.write("#load ");
        self.string_literal(&name);
      }
      NodeData::DirectiveRun(run) => {
        let run = run.clone();
        self.run(&run);
      }
      NodeData::DirectiveCode { expression, flags } => {
        let (expression, flags) = (*expression, *flags);
        if flags.contains(CodeFlags::NULL) || expression.is_none() {
          self.write("#code,null");
          return;
        }
        if flags.contains(CodeFlags::TYPED) {
          self.write("#code,typed ");
        } else {
          self.write("#code ");
        }
        if let Some(expression) = expression {
          self.expression(expression);
        }
      }
      NodeData::DirectivePokeName { module, name } => {
        let (module, name) = (*module, *name);
        self.write("#poke_name ");
        self.expression(module);
        self.write(" ");
        self.expression(name);
      }
      NodeData::Asm(node) => {
        let node = node.clone();
        self.asm(&node);
      }
      NodeData::DirectiveBake {
        procedure_call,
        bake_type,
      } => {
        let (procedure_call, bake_type) = (*procedure_call, *bake_type);
        self.write(match bake_type {
          BakeType::Constants => "#bake_constants ",
          BakeType::ParameterValue => "#bake_arguments ",
          BakeType::DynamicSpecialize => "#dynamic_specialize ",
        });
        self.expression(procedure_call);
      }
      NodeData::DirectiveModify { block } => {
        let block = *block;
        self.write("#modify ");
        self.block(block, BlockForm::Statements);
      }
      NodeData::DirectiveLibrary {
        name,
        library_flags,
      } => {
        let (name, library_flags) = (name.clone(), *library_flags);
        self.write("#library");
        if library_flags.contains(LibraryFlags::IS_SYSTEM_LIBRARY) {
          self.write(",system");
        }
        if library_flags.contains(LibraryFlags::DYNAMIC_LIBRARY_UNAVAILABLE) {
          self.write(",no_dll");
        }
        if library_flags.contains(LibraryFlags::STATIC_LIBRARY_UNAVAILABLE) {
          self.write(",no_static_library");
        }
        if library_flags.contains(LibraryFlags::LINK_ALWAYS) {
          self.write(",link_always");
        }
        self.write(" ");
        self.string_literal(&name);
      }
      NodeData::ExpressionQuery {
        query_kind,
        expression_to_query,
      } => {
        let (query_kind, expression_to_query) = (*query_kind, *expression_to_query);
        self.write(match query_kind {
          ExpressionQueryKind::TypeOf => "type_of(",
          ExpressionQueryKind::IsConstant => "is_constant(",
          ExpressionQueryKind::CodeOf => "code_of(",
        });
        self.expression(expression_to_query);
        self.write(")");
      }
      NodeData::PushContext {
        to_push,
        block,
        flags,
      } => {
        let (to_push, block, flags) = (*to_push, *block, *flags);
        if flags.contains(PushContextFlags::IS_BACKTICKED) {
          self.write("`");
        }
        self.write("push_context");
        if flags.contains(PushContextFlags::DEFER_POP) {
          self.write(",defer_pop");
        }
        if let Some(to_push) = to_push {
          self.write(" ");
          self.expression(to_push);
        }
        if let Some(block) = block {
          self.write(" ");
          self.block(block, BlockForm::Statements);
        }
      }
      NodeData::Note { text } => {
        let text = *text;
        self.write("@");
        // A note's name runs to the next space or `;` (**L§2.7**), so one that
        // holds either is written back in its string form.
        let bytes = self.interner.resolve(text);
        if bytes.is_empty()
          || bytes
            .iter()
            .any(|byte| byte.is_ascii_whitespace() || *byte == b';' || *byte == b'"')
        {
          self.string_literal(bytes);
        } else {
          self.name(text);
        }
      }
      NodeData::DirectivePlace { ident } => {
        let ident = *ident;
        self.write("#place ");
        self.expression(ident);
      }
      NodeData::DirectiveScope { scope_type } => self.write(match scope_type {
        ScopeType::Export => "#scope_export",
        ScopeType::File => "#scope_file",
        ScopeType::Internal => "#scope_module",
      }),
      NodeData::TypeQuery {
        query_kind,
        type_to_query,
      } => {
        let (query_kind, type_to_query) = (*query_kind, *type_to_query);
        self.write(match query_kind {
          TypeQueryKind::SizeOf => "size_of(",
          TypeQueryKind::TypeInfo => "type_info(",
          TypeQueryKind::InitializerOf => "initializer_of(",
        });
        self.expression(type_to_query);
        self.write(")");
      }
      NodeData::DirectiveLocation(location) => {
        let location = location.clone();
        if location.is_caller_location {
          self.write("#caller_location");
          return;
        }
        self.write("#location");
        if location.has_parentheses {
          self.write("(");
          if let Some(expression) = location.expression {
            self.expression(expression);
          }
          self.write(")");
        }
      }
      NodeData::DirectiveModuleParameters(parameters) => {
        let parameters = parameters.clone();
        self.write("#module_parameters ");
        self.parameter_list(parameters.module_parameters);
        if let Some(program) = parameters.program_parameters {
          self.write(" ");
          self.parameter_list(program);
        }
        if let Some(common) = parameters.common_code {
          self.write(" ");
          self.block(common, BlockForm::Statements);
        }
      }
      NodeData::DirectiveAddContext { expression } => {
        let expression = *expression;
        self.write("#add_context ");
        self.expression(expression);
      }
      NodeData::DirectiveCompileTime => self.write("#compile_time"),
      NodeData::CompoundDeclaration(compound) => {
        let compound = compound.clone();
        self.compound_declaration(&compound);
      }
      NodeData::Defer {
        block,
        is_backticked,
      } => {
        let (block, is_backticked) = (*block, *is_backticked);
        if is_backticked {
          self.write("`");
        }
        self.write("defer ");
        self.block(block, BlockForm::Statements);
      }
      NodeData::Using(using) => {
        let using = using.clone();
        match using.filter_type {
          FilterType::None => self.write("using"),
          FilterType::Only => self.write("using,only"),
          FilterType::Except => self.write("using,except"),
          FilterType::Map => self.write("using,map"),
        }
        if let Some(filter) = using.filter_expression {
          self.write("(");
          self.expression(filter);
          self.write(")");
        }
        if using.no_parameters {
          self.write(",no_parameters");
        }
        self.write(" ");
        self.expression(using.expression);
      }
      NodeData::Placeholder => self.write("#placeholder"),
      NodeData::DirectiveInsert(insert) => {
        let insert = insert.clone();
        self.insert(&insert);
      }
      NodeData::DirectiveProcedureName { argument } => {
        let argument = *argument;
        self.write("#procedure_name(");
        if let Some(argument) = argument {
          self.expression(argument);
        }
        self.write(")");
      }
      NodeData::DirectiveWildcard { index } => {
        let index = *index;
        let _ = write!(self.out, "#wildcard({index})");
      }
      NodeData::DirectiveExists(exists) => {
        let exists = exists.clone();
        self.write("#exists(");
        self.expression(exists.query_expression);
        if let Some(sync) = exists.sync_expression {
          self.write(", ");
          self.expression(sync);
        }
        self.write(")");
      }
      NodeData::DirectiveContextType => self.write("#Context"),
      NodeData::DirectiveFileInfo { which } => {
        let text = which.text();
        self.write(text);
      }
      NodeData::DirectiveCallerCode => self.write("#caller_code"),
    }
  }

  fn ident(&mut self, ident: &Ident) {
    if ident.flags.contains(IdentFlags::DEFINES_POLYMORPH_VARIABLE) {
      self.write("$");
    }
    if ident.flags.contains(IdentFlags::HAS_SCOPE_MODIFIER) {
      self.write("`");
    }
    let bytes = self.interner.resolve(ident.name);
    if !bytes
      .first()
      .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
    {
      self.write("operator ");
    }
    self.out.push_str(&String::from_utf8_lossy(bytes));
  }

  fn literal(&mut self, literal: &Literal) {
    match &literal.value {
      LiteralValue::Integer(value) => {
        let value = *value;
        if literal.flags.contains(LiteralFlags::HEX) {
          let _ = write!(self.out, "0x{value:x}");
        } else if literal.flags.contains(LiteralFlags::BINARY) {
          let _ = write!(self.out, "0b{value:b}");
        } else {
          let _ = write!(self.out, "{value}");
        }
      }
      LiteralValue::Float(value) => {
        let text = format_float(*value, literal.flags);
        self.write(&text);
      }
      LiteralValue::Text(text) => {
        let text = text.clone();
        if crate::rules::prints_as_here_string(literal) {
          self.here_string(&text);
        } else {
          self.string_literal(&text);
        }
      }
      LiteralValue::Bool(value) => self.write(if *value { "true" } else { "false" }),
      LiteralValue::Null => self.write("null"),
      LiteralValue::Array(array) => {
        let array = array.clone();
        if let Some(element_type) = array.element_type {
          self.expression(element_type);
        }
        self.write(".[");
        for (index, member) in array.members.iter().enumerate() {
          if index > 0 {
            self.write(", ");
          }
          self.expression(*member);
        }
        self.write("]");
      }
      LiteralValue::Struct(structure) => {
        let structure = structure.clone();
        if let Some(type_expression) = structure.type_expression {
          self.expression(type_expression);
        }
        self.write(".{");
        for (index, argument) in structure.arguments.iter().enumerate() {
          if index > 0 {
            self.write(", ");
          }
          self.argument(argument);
        }
        self.write("}");
      }
    }
  }

  /// String literals are written the way `Program_Print.print_string_literal`
  /// writes them; bytes that would not survive a UTF-8 round trip are escaped.
  fn string_literal(&mut self, text: &[u8]) {
    let utf8 = std::str::from_utf8(text).is_ok();
    let mut out: Vec<u8> = vec![b'"'];
    for byte in text {
      match byte {
        0x00 => out.extend_from_slice(b"\\0"),
        0x1b => out.extend_from_slice(b"\\e"),
        b'\n' => out.extend_from_slice(b"\\n"),
        b'\r' => out.extend_from_slice(b"\\r"),
        b'\t' => out.extend_from_slice(b"\\t"),
        0x1f => out.extend_from_slice(b"\\%"),
        b'"' => out.extend_from_slice(b"\\\""),
        b'\\' => out.extend_from_slice(b"\\\\"),
        byte if *byte < 32 || (*byte >= 0x80 && !utf8) => {
          out.extend_from_slice(format!("\\x{byte:02x}").as_bytes());
        }
        byte => out.push(*byte),
      }
    }
    out.push(b'"');
    self.out.push_str(&String::from_utf8_lossy(&out));
  }

  fn here_string(&mut self, text: &[u8]) {
    let mut terminator = String::from("OJ_STRING");
    let mut suffix = 0;
    while text
      .windows(terminator.len())
      .any(|window| window == terminator.as_bytes())
    {
      suffix += 1;
      terminator = format!("OJ_STRING{suffix}");
    }

    let carriage_returns = text.windows(2).any(|window| window == b"\r\n");
    self.write("#string");
    if carriage_returns {
      self.write(",cr");
    }
    self.write(" ");
    self.write(&terminator);
    self.write("\n");
    let body: Vec<u8> = if carriage_returns {
      text
        .iter()
        .copied()
        .filter(|byte| *byte != b'\r')
        .collect::<Vec<u8>>()
    } else {
      text.to_vec()
    };
    self.out.push_str(&String::from_utf8_lossy(&body));
    self.write(&terminator);
  }

  fn argument(&mut self, argument: &Argument) {
    if let Some(name) = argument.name {
      self.expression(name);
      self.write(" = ");
    }
    self.expression(argument.expression);
  }

  fn arguments(&mut self, arguments: &[Argument]) {
    for (index, argument) in arguments.iter().enumerate() {
      if index > 0 {
        self.write(", ");
      }
      self.argument(argument);
    }
  }

  fn call(&mut self, call: &ProcedureCall) {
    if call
      .flags
      .contains(CallFlags::RETURNS_PROCEDURE_POINTER_ONLY)
    {
      self.write("#procedure_of_call ");
    }
    if call.flags.contains(CallFlags::INLINE_YES) {
      self.write("inline ");
    }
    if call.flags.contains(CallFlags::INLINE_NO) {
      self.write("no_inline ");
    }
    self.expression(call.procedure_expression);
    self.write("(");
    self.arguments(&call.arguments);
    if let Some(modification) = &call.context_modification {
      self.write(",,");
      for (index, expression) in modification.iter().enumerate() {
        self.write(if index == 0 { " " } else { ", " });
        self.expression(*expression);
      }
    }
    self.write(")");
    if call.flags.contains(CallFlags::NO_DEBUG) {
      self.write(" #no_debug");
    }
  }

  fn if_node(&mut self, node: &IfNode) {
    if node.if_flags.contains(IfFlags::IS_STATIC) {
      self.write("#");
    }
    if node.if_flags.contains(IfFlags::IS_IFX) {
      self.write("ifx ");
    } else {
      self.write("if ");
    }
    if node.if_flags.contains(IfFlags::MARKED_AS_COMPLETE) {
      self.write("#complete ");
    }
    self.expression(node.condition);
    if node.if_flags.contains(IfFlags::IS_SWITCH_STATEMENT) {
      self.write(" ==");
    }

    let form = if node.if_flags.contains(IfFlags::IS_IFX) {
      BlockForm::Value
    } else {
      BlockForm::Statements
    };
    if let Some(then_block) = node.then_block {
      // Without braces the branch needs `then`, or re-reading would let the
      // condition swallow a branch that starts with `*`, `-` or `.`.
      let braced = self
        .ast
        .flags(then_block)
        .contains(NodeFlags::IS_PARENTHESIZED);
      if node.if_flags.contains(IfFlags::IS_SWITCH_STATEMENT) || braced {
        self.write(" ");
      } else {
        self.write(" then ");
      }
      self.block(then_block, form);
    }
    if let Some(else_block) = node.else_block {
      self.write(" else ");
      self.block(else_block, form);
    }
  }

  fn for_node(&mut self, node: &ForNode) {
    self.write("for ");
    if let Some(replacement) = node.want_replacement_for_expansion {
      self.write(":");
      self.expression(replacement);
      self.write(" ");
    }
    if node.for_flags.contains(ForFlags::TEMPORARY_V2) {
      self.write("#v2 ");
    }

    let mut wrote_modifier = false;
    if node.for_flags.contains(ForFlags::REVERSE) {
      self.write("<");
      wrote_modifier = true;
    }
    if let Some(expression) = node.want_reverse_expression {
      self.write("<=");
      self.expression(expression);
      self.write(",");
      wrote_modifier = true;
    }
    if node.for_flags.contains(ForFlags::POINTER) {
      self.write("*");
      wrote_modifier = true;
    }
    if let Some(expression) = node.want_pointer_expression {
      self.write("*=");
      self.expression(expression);
      wrote_modifier = true;
    }
    if wrote_modifier {
      self.write(" ");
    }

    if let Some(it) = node.ident_it {
      self.expression(it);
      if let Some(it_index) = node.ident_it_index {
        self.write(", ");
        self.expression(it_index);
      }
      self.write(": ");
    }

    self.expression(node.iteration_expression);
    if let Some(right) = node.iteration_expression_right {
      self.write("..");
      self.expression(right);
    }
    self.write(" ");
    self.block(node.block, BlockForm::Statements);
  }

  fn type_instantiation(&mut self, inst: &TypeInstantiation) {
    if inst.inst_flags.contains(InstFlags::TYPE_DIRECTIVE) {
      self.write("#type");
      if inst.inst_flags.contains(InstFlags::TYPE_DIRECTIVE_DISTINCT) {
        self.write(",distinct");
      }
      if inst.inst_flags.contains(InstFlags::TYPE_DIRECTIVE_ISA) {
        self.write(",isa");
      }
      self.write(" ");
      if let Some(target) = inst.type_directive_target {
        self.expression(target);
      }
      return;
    }

    if let Some(pointer_to) = inst.pointer_to {
      self.write("*");
      self.expression(pointer_to);
      return;
    }

    if let Some(element_type) = inst.array_element_type {
      if inst.inst_flags.contains(InstFlags::VARARGS) {
        self.write("..");
      } else if inst.inst_flags.contains(InstFlags::RESIZABLE) {
        self.write("[..] ");
      } else if let Some(dimension) = inst.array_dimension {
        self.write("[");
        self.expression(dimension);
        self.write("] ");
      } else {
        self.write("[] ");
      }
      self.expression(element_type);
      return;
    }

    if let Some(expression) = inst.type_valued_expression {
      self.expression(expression);
      if let Some(must_implement) = inst.must_implement {
        self.write("/");
        if inst.inst_flags.contains(InstFlags::INTERFACE) {
          self.write("interface ");
        }
        self.expression(must_implement);
      }
    }
  }

  fn cast(&mut self, cast: &Cast) {
    if cast.cast_flags.contains(CastFlags::IS_AUTO) {
      self.write("xx");
      self.cast_modifiers(cast.cast_flags);
      self.write(" ");
      self.expression(cast.expression);
      return;
    }

    if cast.cast_flags.contains(CastFlags::HAS_POSTFIX_SYNTAX) {
      self.expression(cast.expression);
      self.write(".(");
      if let Some(target) = cast.target_type {
        self.expression(target);
      }
      self.cast_modifiers(cast.cast_flags);
      self.write(")");
      return;
    }

    if cast.cast_flags.contains(CastFlags::HAS_FUNCTION_SYNTAX) {
      self.write("cast(");
      if let Some(target) = cast.target_type {
        self.expression(target);
      }
      self.write(", ");
      self.expression(cast.expression);
      for modifier in cast_modifier_names(cast.cast_flags) {
        self.write(", ");
        self.write(modifier);
      }
      self.write(")");
      return;
    }

    self.write("cast");
    self.cast_modifiers(cast.cast_flags);
    self.write("(");
    if let Some(target) = cast.target_type {
      self.expression(target);
    }
    self.write(") ");
    if cast.cast_flags.contains(CastFlags::HAS_DEREFERENCE) {
      self.write(".*");
    }
    self.expression(cast.expression);
  }

  fn cast_modifiers(&mut self, flags: CastFlags) {
    for modifier in cast_modifier_names(flags) {
      self.write(",");
      self.write(modifier);
    }
  }

  fn import(&mut self, import: &DirectiveImport) {
    self.write("#import");
    match import.import_type {
      ImportType::ShortName => {}
      ImportType::PathToFile => self.write(",file"),
      ImportType::PathToDirectory => self.write(",dir"),
      ImportType::FullText => self.write(",string"),
    }
    if import.flags.contains(ImportFlags::UNSHARED) {
      self.write(",unshared");
    }
    self.write(" ");
    self.string_literal(&import.name);
    if let Some(parameters) = &import.module_parameters {
      self.write("(");
      self.arguments(parameters);
      self.write(")");
    }
    if let Some(parameters) = &import.program_parameters {
      self.write("(");
      self.arguments(parameters);
      self.write(")");
    }
  }

  fn run(&mut self, run: &DirectiveRun) {
    if run.flags.contains(RunFlags::SYNTACTICALLY_IMPLICIT) {
      self.implicit_run(run);
      return;
    }

    if run.flags.contains(RunFlags::ASSERTION) {
      self.write("#assert");
    } else {
      self.write("#run");
    }
    if run.flags.contains(RunFlags::STALLABLE) {
      self.write(",stallable");
    }
    if run.flags.contains(RunFlags::HOST) {
      self.write(",host");
    }

    let NodeData::ProcedureHeader(header) = self.ast.data(run.procedure) else {
      return;
    };
    let header = header.clone();
    if !run.flags.contains(RunFlags::HAS_IMPLICIT_RETURN_TYPES) {
      self.returns(&header);
    }
    self.write(" ");

    if let Some(body) = header.body_or_null
      && let NodeData::ProcedureBody { block, .. } = self.ast.data(body)
    {
      let block = *block;
      self.block(block, BlockForm::Value);
    }

    if let Some(assertion_string) = run.assertion_string {
      self.write(" ");
      self.expression(assertion_string);
    }
  }

  fn implicit_run(&mut self, run: &DirectiveRun) {
    let NodeData::ProcedureHeader(header) = self.ast.data(run.procedure) else {
      return;
    };
    let header = header.clone();
    self.returns(&header);
    self.write(" ");
    if let Some(body) = header.body_or_null
      && let NodeData::ProcedureBody { block, .. } = self.ast.data(body)
    {
      let block = *block;
      self.block(block, BlockForm::Statements);
    }
  }

  fn insert(&mut self, insert: &DirectiveInsert) {
    self.write("#insert");
    if insert.has_scope_redirection {
      self.write(",scope(");
      if let Some(redirection) = insert.scope_redirection {
        self.expression(redirection);
      }
      self.write(")");
    }
    if insert.break_replacement.is_some()
      || insert.continue_replacement.is_some()
      || insert.remove_replacement.is_some()
    {
      self.write("(");
      let mut first = true;
      for (keyword, replacement) in [
        ("break=", insert.break_replacement),
        ("continue=", insert.continue_replacement),
        ("remove=", insert.remove_replacement),
      ] {
        let Some(replacement) = replacement else {
          continue;
        };
        if !first {
          self.write(", ");
        }
        first = false;
        self.write(keyword);
        self.expression(replacement);
      }
      self.write(")");
    }
    // `#insert -> string { ... }` writes its own leading space with the arrow.
    let implicit = matches!(
      self.ast.data(insert.expression),
      NodeData::DirectiveRun(run) if run.flags.contains(RunFlags::SYNTACTICALLY_IMPLICIT)
    );
    if !implicit {
      self.write(" ");
    }
    self.expression(insert.expression);
  }

  fn comma_separated(&mut self, arguments: &[CommaArgument]) {
    for (index, argument) in arguments.iter().enumerate() {
      if index > 0 {
        self.write(", ");
      }
      self.expression(argument.node);
      match argument.modifier {
        CommaModifier::None => {}
        CommaModifier::Declare => self.write(":"),
        CommaModifier::Assign => self.write("="),
      }
    }
  }

  fn compound_declaration(&mut self, compound: &CompoundDeclaration) {
    let NodeData::Declaration(properties) = self.ast.data(compound.declaration_properties) else {
      return;
    };
    let properties = properties.clone();

    if let Some(alignment) = properties.alignment_expression {
      self.write("#align ");
      self.expression(alignment);
      self.write(" ");
    }
    if properties.flags.contains(DeclarationFlags::IS_MARKED_AS_AS) {
      self.write("#as ");
    }
    self.expression(compound.comma_separated_assignment);

    if let Some(operator) = compound.operator_type {
      self.write(" ");
      self.write(operator.text());
      self.write(" ");
      if let Some(expression) = properties.expression {
        self.expression(expression);
      }
      return;
    }

    self.declaration_tail(&properties, true, true);
    for note in &properties.notes {
      self.write(" ");
      self.expression(*note);
    }
  }

  fn declaration(&mut self, id: NodeId, print_type: bool) {
    let NodeData::Declaration(declaration) = self.ast.data(id) else {
      return;
    };
    let declaration = declaration.clone();

    if let Some(expression) = declaration.expression
      && matches!(self.ast.data(expression), NodeData::Placeholder)
    {
      self.write("#placeholder ");
      if let Some(name) = declaration.name {
        self.expression(name);
      }
      return;
    }

    if declaration.flags.contains(DeclarationFlags::PROGRAM_EXPORT) {
      self.write("#program_export ");
      if let Some(name) = &declaration.program_export_name {
        let name = name.clone();
        self.string_literal(&name);
        self.write(" ");
      }
    }
    if declaration.flags.contains(DeclarationFlags::NO_RESET) {
      self.write("#no_reset ");
    }
    if let Some(alignment) = declaration.alignment_expression {
      self.write("#align ");
      self.expression(alignment);
      self.write(" ");
    }
    if declaration
      .flags
      .contains(DeclarationFlags::IS_MARKED_AS_AS)
    {
      self.write("#as ");
    }
    if declaration
      .flags
      .contains(DeclarationFlags::IS_MARKED_AS_DISCARD)
    {
      self.write("#discard ");
    }
    if declaration
      .flags
      .contains(DeclarationFlags::AUTO_VALUE_BAKE_IS_REQUIRED)
    {
      self.write("$");
    } else if declaration
      .flags
      .contains(DeclarationFlags::AUTO_VALUE_BAKE)
    {
      self.write("$$");
    }

    if let Some(name) = declaration.name {
      self.expression(name);
    }

    self.declaration_tail(&declaration, print_type, false);

    if declaration
      .flags
      .contains(DeclarationFlags::MUST_BE_RECEIVED)
    {
      self.write(" #must");
    }
    for note in &declaration.notes {
      self.write(" ");
      self.expression(*note);
    }
  }

  fn declaration_tail(&mut self, declaration: &Declaration, print_type: bool, force_colon: bool) {
    let constant = declaration.flags.contains(DeclarationFlags::IS_CONSTANT);
    let uninitialized = declaration
      .flags
      .contains(DeclarationFlags::IS_UNINITIALIZED);

    if let Some(type_inst) = declaration.type_inst {
      if !print_type {
        return;
      }
      if declaration.name.is_some() || force_colon {
        self.write(": ");
      }
      self.expression(type_inst);
      if declaration.flags.contains(DeclarationFlags::ELSEWHERE) {
        self.write(" #elsewhere");
        if let Some(library) = declaration.elsewhere_library {
          self.write(" ");
          self.expression(library);
        }
        if let Some(symbol) = &declaration.elsewhere_symbol {
          let symbol = symbol.clone();
          self.write(" ");
          self.string_literal(&symbol);
        }
      }
      if uninitialized {
        self.write(" = ---");
        return;
      }
      if let Some(expression) = declaration.expression {
        self.write(if constant { " : " } else { " = " });
        self.expression(expression);
      }
      return;
    }

    if uninitialized {
      self.write(" = ---");
      return;
    }
    if let Some(expression) = declaration.expression {
      self.write(if constant { " :: " } else { " := " });
      self.expression(expression);
    }
  }

  fn parameter_list(&mut self, header_id: NodeId) {
    let NodeData::ProcedureHeader(header) = self.ast.data(header_id) else {
      return;
    };
    let arguments = header.arguments.clone();
    self.write("(");
    for (index, argument) in arguments.iter().enumerate() {
      if index > 0 {
        self.write(", ");
      }
      self.expression(*argument);
    }
    self.write(")");
  }

  fn procedure_header(&mut self, header: &ProcedureHeader) {
    let quick = header.procedure_flags.contains(ProcedureFlags::QUICK);
    if header
      .procedure_flags
      .contains(ProcedureFlags::SYNTACTICALLY_MARKED_AS_INLINE_YES)
    {
      self.write("inline ");
    }
    if header
      .procedure_flags
      .contains(ProcedureFlags::SYNTACTICALLY_MARKED_AS_INLINE_NO)
    {
      self.write("no_inline ");
    }

    self.write("(");
    for (index, argument) in header.arguments.iter().enumerate() {
      if index > 0 {
        self.write(", ");
      }
      if quick {
        self.declaration(*argument, false);
      } else {
        self.expression(*argument);
      }
    }
    self.write(")");

    if quick {
      self.write(" =>");
    } else if header
      .procedure_flags
      .contains(ProcedureFlags::HAS_IMPLICIT_RETURN_VALUE)
    {
      self.write(" ->");
    } else {
      self.returns(header);
    }

    self.header_directives(header);

    if let Some(body) = header.body_or_null {
      self.write(" ");
      let form = if quick
        && !header
          .procedure_flags
          .contains(ProcedureFlags::QUICK_IN_BLOCK_FORM)
      {
        BlockForm::Value
      } else {
        BlockForm::Statements
      };
      if let NodeData::ProcedureBody { block, .. } = self.ast.data(body) {
        let block = *block;
        self.block(block, form);
      }
      for note in &header.notes {
        self.write(" ");
        self.expression(*note);
      }
    }
  }

  fn returns(&mut self, header: &ProcedureHeader) {
    if header.returns.is_empty() {
      if header.parenthesized_returns {
        self.write(" -> ()");
      }
      return;
    }
    self.write(" -> ");
    if header.parenthesized_returns {
      self.write("(");
    }
    for (index, declaration) in header.returns.iter().enumerate() {
      if index > 0 {
        self.write(", ");
      }
      self.expression(*declaration);
    }
    if header.parenthesized_returns {
      self.write(")");
    }
  }

  fn header_directives(&mut self, header: &ProcedureHeader) {
    let flags = header.procedure_flags;
    let mut c_call_written = false;

    if flags.contains(ProcedureFlags::ELSEWHERE) {
      if flags.contains(ProcedureFlags::C_CALL) {
        self.write(" #foreign");
        c_call_written = true;
      } else {
        self.write(" #elsewhere");
      }
      if let Some(library) = header.library_identifier {
        self.write(" ");
        self.expression(library);
      }
      if let Some(name) = &header.foreign_function_name {
        let name = name.clone();
        self.write(" ");
        self.string_literal(&name);
      }
    }

    if flags.contains(ProcedureFlags::C_CALL) && !c_call_written {
      self.write(" #c_call");
    }
    if flags.contains(ProcedureFlags::SYNTACTICALLY_MARKED_AS_NO_CONTEXT) {
      self.write(" #no_context");
    }
    if flags.contains(ProcedureFlags::MACRO) {
      self.write(" #expand");
    }
    if flags.contains(ProcedureFlags::SYMMETRIC) {
      self.write(" #symmetric");
    }
    if flags.contains(ProcedureFlags::NO_CALL) {
      self.write(" #no_call");
    }
    if flags.contains(ProcedureFlags::ENTRY_POINT_HOOK) {
      self.write(" #entry_point");
    }
    if flags.contains(ProcedureFlags::SYNTACTICALLY_MARKED_AS_COMPILE_TIME) {
      self.write(" #compile_time");
    }
    if flags.contains(ProcedureFlags::SYNTACTICALLY_MARKED_AS_COMPILER) {
      self.write(" #compiler");
      if !flags.contains(ProcedureFlags::INTRINSIC)
        && let Some(name) = &header.intrinsic_name
      {
        let name = name.clone();
        self.write(" ");
        self.string_literal(&name);
      }
    }
    if flags.contains(ProcedureFlags::RUNTIME_SUPPORT) {
      self.write(" #runtime_support");
    }
    if flags.contains(ProcedureFlags::INTRINSIC) {
      self.write(" #intrinsic");
      if let Some(name) = &header.intrinsic_name {
        let name = name.clone();
        self.write(" ");
        self.string_literal(&name);
      }
    }
    if flags.contains(ProcedureFlags::CPP_METHOD) {
      self.write(" #cpp_method");
    }
    if flags.contains(ProcedureFlags::CPP_RETURN_TYPE_IS_NON_POD) {
      self.write(" #cpp_return_type_is_non_pod");
    }
    if flags.contains(ProcedureFlags::DEPRECATED) {
      self.write(" #deprecated");
      if let Some(message) = &header.deprecation_string {
        let message = message.clone();
        self.write(" ");
        self.string_literal(&message);
      }
    }
    if flags.contains(ProcedureFlags::NO_DEBUG) {
      self.write(" #no_debug");
    }
    if flags.contains(ProcedureFlags::DEBUG_DUMP) {
      self.write(" #dump");
    }
    if flags.contains(ProcedureFlags::NO_ALIAS) {
      self.write(" #no_alias");
    }
    for modify in &header.modify_directives {
      self.write(" ");
      self.expression(*modify);
    }
  }

  fn struct_node(&mut self, node: &StructNode) {
    if node.textual_flags.contains(StructFlags::UNION) {
      self.write("union");
    } else {
      self.write("struct");
    }

    if node.has_argument_list {
      self.write(" (");
      for (index, argument) in node.arguments.iter().enumerate() {
        if index > 0 {
          self.write(", ");
        }
        self.expression(*argument);
      }
      self.write(")");
    }

    if node.textual_flags.contains(StructFlags::TYPE_INFO_NONE) {
      self.write(" #type_info_none");
    }
    if node
      .textual_flags
      .contains(StructFlags::TYPE_INFO_NO_SIZE_COMPLAINT)
    {
      self.write(" #type_info_no_size_complaint");
    }
    if node
      .textual_flags
      .contains(StructFlags::TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS)
    {
      self.write(" #type_info_procedures_are_void_pointers");
    }
    if node.textual_flags.contains(StructFlags::NO_PADDING) {
      self.write(" #no_padding");
    }
    if node.textual_flags.contains(StructFlags::FOREIGN) {
      self.write(" #foreign");
    }
    if let Some(alignment) = node.alignment_expression {
      self.write(" #align ");
      self.expression(alignment);
    }
    for modify in &node.modify_directives {
      self.write(" ");
      self.expression(*modify);
    }
    for note in &node.notes {
      self.write(" ");
      self.expression(*note);
    }

    if let Some(block) = node.block {
      self.write(" ");
      self.block(block, BlockForm::Statements);
    }
  }

  fn enum_node(&mut self, node: &EnumNode) {
    self.write(if node.is_flags { "enum_flags" } else { "enum" });
    if let Some(internal) = node.internal_type_inst {
      self.write(" ");
      self.expression(internal);
    }
    if node.marked_as_specified {
      self.write(" #specified");
    }
    if node.marked_as_complete {
      self.write(" #complete");
    }
    for note in &node.notes {
      self.write(" ");
      self.expression(*note);
    }
    if let Some(block) = node.block {
      self.write(" ");
      self.block(block, BlockForm::Statements);
    }
  }

  /// An `#asm` block, one instruction per line (**L§15**). Sizes are written
  /// in bits, which is the spelling that survives a round trip: the letters
  /// mean the same thing.
  fn asm(&mut self, node: &AsmNode) {
    self.write("#asm");
    for (index, feature) in node.features.iter().enumerate() {
      self.write(if index == 0 { " " } else { ", " });
      self.name(*feature);
    }
    self.write(" {\n");
    self.depth += 1;
    for instruction in &node.instructions {
      self.indent();
      if let Some(mnemonic) = instruction.mnemonic {
        self.name(mnemonic);
        match instruction.size {
          AsmSize::Inferred => {}
          AsmSize::Bits(bits) => {
            let _ = write!(self.out, ".{bits}");
          }
          AsmSize::Of(expression) => {
            self.write("?");
            self.expression(expression);
          }
        }
        if !instruction.operands.is_empty() {
          self.write(" ");
        }
      }
      for (index, operand) in instruction.operands.iter().enumerate() {
        if index > 0 {
          self.write(", ");
        }
        self.asm_operand(operand);
      }
      self.write(";\n");
    }
    self.depth -= 1;
    self.indent();
    self.write("}");
  }

  fn asm_operand(&mut self, operand: &AsmOperand) {
    match &operand.kind {
      AsmOperandKind::Declaration(declaration) => {
        self.expression(declaration.name);
        self.write(":");
        if let Some(class) = declaration.class {
          self.write(" ");
          self.write(asm_class_name(class));
        }
        if let Some(register) = declaration.register {
          self.write(" === ");
          self.asm_register(register);
        }
      }
      AsmOperandKind::Pin { name, register } => {
        self.expression(*name);
        self.write(" === ");
        self.asm_register(*register);
      }
      AsmOperandKind::Expression(expression) => self.expression(*expression),
      AsmOperandKind::Memory(memory) => self.asm_memory(memory),
    }
    if let Some(mask) = &operand.mask {
      self.write(if mask.zeroing { " &* " } else { " & " });
      self.expression(mask.register);
    }
    // A `!` binds tightly to a memory operand and stands apart from a register
    // one, which is how the reference writes each (**L§15**).
    if operand.flag.is_some() && !matches!(operand.kind, AsmOperandKind::Memory(_)) {
      self.write(" ");
    }
    match operand.flag {
      None => {}
      Some(AsmFlag::Plain) => self.write("!"),
      Some(AsmFlag::Rounding(mode)) => {
        self.write("!");
        self.write(match mode {
          RoundingMode::Nearest => "n",
          RoundingMode::Down => "d",
          RoundingMode::Up => "u",
          RoundingMode::Zero => "z",
        });
      }
    }
  }

  /// `[base + index*scale + displacement]`. A displacement that is not a bare
  /// number is written in parentheses, which is what tells it apart from an
  /// index when the operand is read back (**L§15**).
  fn asm_memory(&mut self, memory: &AsmMemory) {
    self.write("[");
    if memory.by_reference {
      self.write("*");
    }
    self.expression(memory.base);
    if let Some(index) = memory.index {
      self.write(" + ");
      self.expression(index);
      if let Some(scale) = memory.scale {
        self.write("*");
        self.expression(scale);
      }
    }
    if let Some(displacement) = memory.displacement {
      self.write(if memory.displacement_is_negative {
        " - "
      } else {
        " + "
      });
      let bare = memory.index.is_some()
        || memory.displacement_is_negative
        || matches!(self.ast.data(displacement), NodeData::Literal(_))
        || self
          .ast
          .flags(displacement)
          .contains(NodeFlags::IS_PARENTHESIZED);
      if bare {
        self.expression(displacement);
      } else {
        self.write("(");
        self.expression(displacement);
        self.write(")");
      }
    }
    self.write("]");
  }

  fn asm_register(&mut self, register: AsmRegister) {
    match register {
      AsmRegister::Named(symbol) => self.name(symbol),
      AsmRegister::Numbered(number) => {
        let _ = write!(self.out, "{number}");
      }
    }
  }
}

fn asm_class_name(class: AsmClass) -> &'static str {
  match class {
    AsmClass::Gpr => "gpr",
    AsmClass::Str => "str",
    AsmClass::Vec => "vec",
    AsmClass::Omr => "omr",
  }
}

/// Floats print so that re-lexing them yields the same value *and* the same
/// width flags (**L§2.6**): hex-float literals keep their bit pattern, and a
/// decimal literal keeps a decimal point so it is not lexed as an integer.
fn format_float(value: f64, flags: LiteralFlags) -> String {
  if flags.contains(LiteralFlags::HEX) || !value.is_finite() {
    if flags.contains(LiteralFlags::REQUIRES_FLOAT64) {
      return format!("0h{:016x}", value.to_bits());
    }
    return format!("0h{:08x}", narrow_to_float32_bits(value.to_bits()));
  }

  let text = format!("{value:?}");
  match text.find(['e', 'E']) {
    Some(index) if !text[..index].contains('.') => {
      format!("{}.0{}", &text[..index], &text[index..])
    }
    Some(_) => text,
    None if text.contains('.') => text,
    None => format!("{text}.0"),
  }
}

fn cast_modifier_names(flags: CastFlags) -> Vec<&'static str> {
  [
    (CastFlags::NO_BOUNDS_CHECK, "no_check"),
    (CastFlags::TRUNCATE, "trunc"),
    (CastFlags::FORCE, "force"),
    (CastFlags::VERY_FORCE, "FORCE"),
  ]
  .into_iter()
  .filter(|(flag, _)| flags.contains(*flag))
  .map(|(_, name)| name)
  .collect()
}

/// The inverse of the widening the lexer does for an 8-digit `0h` literal.
/// Going through `as f32` would lose a NaN's payload, so the bits are moved
/// directly.
fn narrow_to_float32_bits(bits: u64) -> u32 {
  let sign = ((bits >> 63) as u32) << 31;
  let exponent = ((bits >> 52) & 0x7ff) as i32;
  let mantissa = ((bits >> 29) & 0x7f_ffff) as u32;
  let exponent = match exponent {
    0x7ff => 0xff,
    0 => 0,
    exponent => (exponent - 1023 + 127).clamp(0, 0xff) as u32,
  };
  sign | (exponent << 23) | mantissa
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn floats_always_print_with_a_decimal_point() {
    assert_eq!(format_float(1.5, LiteralFlags::FLOAT), "1.5");
    assert_eq!(format_float(600.0, LiteralFlags::FLOAT), "600.0");
    assert_eq!(format_float(1e20, LiteralFlags::FLOAT), "1.0e20");
  }

  #[test]
  fn hex_floats_keep_their_bit_pattern() {
    let bits = f32::from_bits(0xff80_0000);
    assert_eq!(
      format_float(f64::from(bits), LiteralFlags::HEX | LiteralFlags::FLOAT),
      "0hff800000"
    );
    assert_eq!(
      format_float(
        f64::INFINITY,
        LiteralFlags::HEX | LiteralFlags::FLOAT | LiteralFlags::REQUIRES_FLOAT64
      ),
      "0h7ff0000000000000"
    );
  }
}

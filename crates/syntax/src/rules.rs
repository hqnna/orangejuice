use crate::ast::{
  Ast, DirectiveRun, Literal, LiteralFlags, LiteralValue, NodeData, NodeFlags, NodeId,
  ProcedureFlags, RunFlags,
};

/// A here-string can only be written back as one when its text ends with the
/// newline that precedes the terminator line (**L§2.6**); anything else is
/// printed as an ordinary string literal, and then it needs its `;`.
pub(crate) fn prints_as_here_string(literal: &Literal) -> bool {
  literal.flags.contains(LiteralFlags::HERE_STRING)
    && matches!(&literal.value, LiteralValue::Text(text) if text.is_empty() || text.ends_with(b"\n"))
}

/// Whether the parser may accept a statement that is not followed by a `;`.
/// It is the printer's rule, widened by the here-strings that end a statement
/// at their terminator line even when we would write them back quoted.
pub fn semicolon_is_optional(ast: &Ast, statement: NodeId) -> bool {
  !needs_semicolon(ast, statement) || ends_with_a_here_string(ast, statement)
}

fn ends_with_a_here_string(ast: &Ast, statement: NodeId) -> bool {
  match ast.data(final_node(ast, statement)) {
    NodeData::Literal(literal) => literal.flags.contains(LiteralFlags::HERE_STRING),
    _ => false,
  }
}

/// The node a statement's `;` rule actually depends on: a declaration is
/// decided by its value, and a value that is a type instantiation by the type.
fn final_node(ast: &Ast, statement: NodeId) -> NodeId {
  let mut node = statement;
  match ast.data(statement) {
    NodeData::Declaration(declaration) => match (declaration.expression, declaration.type_inst) {
      (Some(expression), _) => node = expression,
      (None, Some(type_inst)) => node = type_inst,
      (None, None) => return statement,
    },
    NodeData::CompoundDeclaration(compound) => match ast.data(compound.declaration_properties) {
      NodeData::Declaration(declaration) => match declaration.expression {
        Some(expression) => node = expression,
        None => return statement,
      },
      _ => return statement,
    },
    _ => {}
  }

  if let NodeData::TypeInstantiation(inst) = ast.data(node)
    && let Some(expression) = inst.type_valued_expression
  {
    node = expression;
  }
  node
}

/// Whether a statement must be followed by a `;` (**C§5.2**). The printer
/// writes one exactly where this says so, which is what makes the printed form
/// re-parse to the same tree.
pub fn needs_semicolon(ast: &Ast, statement: NodeId) -> bool {
  match ast.data(final_node(ast, statement)) {
    // A quick lambda in its expression form ends with an expression, so the
    // declaration around it still needs its `;` (**L§7.9**).
    NodeData::ProcedureHeader(header) => {
      header.body_or_null.is_none()
        || (header.procedure_flags.contains(ProcedureFlags::QUICK)
          && !header
            .procedure_flags
            .contains(ProcedureFlags::QUICK_IN_BLOCK_FORM))
    }
    NodeData::Block(_)
    | NodeData::ProcedureBody { .. }
    | NodeData::Case(_)
    | NodeData::While { .. }
    | NodeData::For(_)
    | NodeData::Struct(_)
    | NodeData::Enum(_)
    | NodeData::Defer { .. }
    | NodeData::Asm(_)
    | NodeData::DirectiveScope { .. }
    | NodeData::DirectiveModify { .. } => false,
    // An `ifx` is an expression, so it needs the enclosing statement's `;` —
    // unless its last branch is a braced block (**L§5.13**).
    NodeData::If(node) => {
      node.if_flags.contains(crate::ast::IfFlags::IS_IFX)
        && match node.else_block.or(node.then_block) {
          Some(block) => !ast.flags(block).contains(NodeFlags::IS_PARENTHESIZED),
          None => true,
        }
    }
    NodeData::PushContext { block, .. } => block.is_none(),
    NodeData::DirectiveRun(run) => !run_ends_in_a_block(ast, run),
    NodeData::DirectiveInsert(insert) => needs_semicolon(ast, insert.expression),
    NodeData::Using(using) => needs_semicolon(ast, using.expression),
    NodeData::DirectiveCode {
      expression: Some(expression),
      ..
    } => needs_semicolon(ast, *expression),
    NodeData::DirectiveModuleParameters(parameters) => parameters.common_code.is_none(),
    // A here-string ends at its terminator line, which the standard modules
    // treat as the end of the declaration (**L§2.6**).
    NodeData::Literal(literal) => !prints_as_here_string(literal),
    _ => true,
  }
}

fn run_ends_in_a_block(ast: &Ast, run: &DirectiveRun) -> bool {
  if run.flags.contains(RunFlags::ASSERTION) {
    return false;
  }
  let NodeData::ProcedureHeader(header) = ast.data(run.procedure) else {
    return false;
  };
  let Some(body) = header.body_or_null else {
    return false;
  };
  let NodeData::ProcedureBody { block, .. } = ast.data(body) else {
    return false;
  };
  ast.flags(*block).contains(NodeFlags::IS_PARENTHESIZED)
}

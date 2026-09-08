use oj_diag::{SourceId, Span};
use oj_scope::{DeclId, DeclKind, ScopeId};
use oj_syntax::ast::{Argument, DeclarationFlags, NodeData, NodeId, OperatorType, ProcedureFlags};
use oj_types::TypeId;

use crate::checker::Checker;

/// What a `return` in the statement being checked has to match.
#[derive(Clone)]
struct Context {
  returns: Vec<TypeId>,
  scope: ScopeId,
  source: SourceId,
}

impl Checker<'_> {
  /// Typechecks the body of every procedure in the program: the conversions
  /// its declarations, assignments, returns and calls ask for (**L§5.10**).
  pub(crate) fn check_bodies(&mut self) {
    let tree = self.program().tree();
    let procedures: Vec<DeclId> = (0..tree.declaration_count() as u32)
      .map(DeclId)
      .filter(|id| tree.decl(*id).kind == DeclKind::Procedure)
      .collect();
    for procedure in procedures {
      self.check_procedure(procedure);
    }
  }

  /// Executes the `#run`s and `#assert`s written at file scope (**L§6.11**).
  /// A run inside a declaration has already been reached by whoever asked for
  /// that declaration's type; these belong to nobody, so the checker walks
  /// them itself, in the order they were written.
  pub(crate) fn check_file_runs(&mut self) {
    let units: Vec<(SourceId, ScopeId, NodeId)> = self
      .program()
      .units()
      .iter()
      .map(|unit| (unit.source, unit.scope, unit.parsed.root))
      .collect();
    for (source, scope, root) in units {
      self.check_runs_in(source, scope, root);
    }
  }

  fn check_runs_in(&mut self, source: SourceId, scope: ScopeId, node: NodeId) {
    if !self.enter() {
      return;
    }
    self.check_runs_in_inner(source, scope, node);
    self.leave();
  }

  fn check_runs_in_inner(&mut self, source: SourceId, scope: ScopeId, node: NodeId) {
    let Some(ast) = self.ast(source) else {
      return;
    };
    match ast.data(node) {
      NodeData::Block(block) => {
        for statement in block.statements.clone() {
          self.check_runs_in(source, scope, statement);
        }
      }
      NodeData::DirectiveRun(_) => {
        let scope = self.scope_at(source, node, scope);
        self.expression_type(scope, source, node);
      }
      // A `#if` at file scope contributes only the branch it decided
      // (**L§6.10**), so only that branch's runs happen. One nobody can decide
      // yet contributes none, the way the scope tree holds its diagnostics
      // back (`docs/spec.md` §10).
      NodeData::If(payload)
        if payload
          .if_flags
          .contains(oj_syntax::ast::IfFlags::IS_STATIC) =>
      {
        let payload = payload.clone();
        for branch in self
          .static_branch(scope, source, &payload)
          .unwrap_or_default()
        {
          self.check_runs_in(source, scope, branch);
        }
      }
      _ => {}
    }
  }

  /// The branches a static `#if` decided to keep (**L§6.10**). `None` means
  /// the condition did not fold, which leaves the decision to whoever asked.
  pub(crate) fn static_branch(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    payload: &oj_syntax::ast::IfNode,
  ) -> Option<Vec<NodeId>> {
    if !payload
      .if_flags
      .contains(oj_syntax::ast::IfFlags::IS_STATIC)
    {
      return None;
    }
    let condition_scope = self.scope_at(source, payload.condition, scope);
    let truth = self
      .expression_type(condition_scope, source, payload.condition)
      .constant
      .and_then(|value| value.value.truth())?;
    let branch = if truth {
      payload.then_block
    } else {
      payload.else_block
    };
    Some(branch.into_iter().collect())
  }

  fn check_procedure(&mut self, procedure: DeclId) {
    let decl = self.program().tree().decl(procedure).clone();
    let (Some(node), Some(source)) = (decl.node, decl.source) else {
      return;
    };
    let Some(ast) = self.ast(source) else {
      return;
    };
    let NodeData::Declaration(declaration) = ast.data(node) else {
      return;
    };
    let Some(expression) = declaration.expression else {
      return;
    };
    let NodeData::ProcedureHeader(header) = ast.data(expression) else {
      return;
    };
    // A macro's body is typechecked where it expands, not here (**L§7.13**),
    // and a polymorphic header has no concrete types to check against.
    if header
      .procedure_flags
      .intersects(ProcedureFlags::MACRO | ProcedureFlags::POLYMORPHIC)
    {
      return;
    }
    let Some(body) = header.body_or_null else {
      return;
    };
    let NodeData::ProcedureBody { block, .. } = ast.data(body) else {
      return;
    };

    let type_id = self.decl_type(procedure).value;
    let Some(signature) = self.types().procedure_of(type_id).cloned() else {
      return;
    };
    if signature
      .flags
      .contains(oj_types::ProcedureFlags::IS_POLYMORPHIC)
      || signature
        .arguments
        .iter()
        .chain(&signature.returns)
        .any(|type_id| self.is_polymorphic_type(*type_id))
    {
      return;
    }

    let context = Context {
      returns: signature.returns.clone(),
      scope: decl.scope,
      source,
    };
    self.check_statement(&context, *block);
  }

  fn check_statement(&mut self, context: &Context, node: NodeId) {
    if !self.enter() {
      return;
    }
    self.check_statement_inner(context, node);
    self.leave();
  }

  fn check_statement_inner(&mut self, context: &Context, node: NodeId) {
    let (scope, source) = (context.scope, context.source);
    let Some(ast) = self.ast(source) else {
      return;
    };
    match ast.data(node) {
      NodeData::Block(block) => {
        for statement in &block.statements {
          self.check_statement(context, *statement);
        }
      }
      NodeData::Declaration(declaration) => self.check_declaration(context, node, declaration),
      NodeData::Return { arguments, .. } => self.check_return(context, node, arguments),
      NodeData::While { condition, block } => {
        let (condition, block) = (*condition, *block);
        self.check_condition(context, condition);
        self.check_statement(context, block);
      }
      NodeData::If(payload) => {
        let payload = payload.clone();
        if payload
          .if_flags
          .contains(oj_syntax::ast::IfFlags::IS_SWITCH_STATEMENT)
        {
          self.expression_type(scope, source, payload.condition);
        } else {
          self.check_condition(context, payload.condition);
        }
        // The branch a `#if` rejected is never typechecked (**L§6.10**), which
        // is what lets a module write `#assert false` in the `else` of an
        // `OS ==` chain.
        let decided = self.static_branch(scope, source, &payload);
        let undecided = decided.is_none()
          && payload
            .if_flags
            .contains(oj_syntax::ast::IfFlags::IS_STATIC);
        let branches = decided.unwrap_or_else(|| {
          [payload.then_block, payload.else_block]
            .into_iter()
            .flatten()
            .collect()
        });
        if undecided {
          self.undecided_static_ifs += 1;
        }
        for branch in branches {
          self.check_statement(context, branch);
        }
        if undecided {
          self.undecided_static_ifs -= 1;
        }
      }
      NodeData::Case(case) => {
        let (condition, then_block) = (case.condition, case.then_block);
        if let Some(condition) = condition {
          self.expression_type(scope, source, condition);
        }
        self.check_statement(context, then_block);
      }
      NodeData::For(payload) => {
        let payload = payload.clone();
        self.expression_type(scope, source, payload.iteration_expression);
        if let Some(right) = payload.iteration_expression_right {
          self.expression_type(scope, source, right);
        }
        self.check_statement(context, payload.block);
      }
      NodeData::Defer { block, .. } => {
        let block = *block;
        self.check_statement(context, block);
      }
      NodeData::PushContext { to_push, block, .. } => {
        let (to_push, block) = (*to_push, *block);
        if let Some(to_push) = to_push {
          self.expression_type(scope, source, to_push);
        }
        if let Some(block) = block {
          self.check_statement(context, block);
        }
      }
      NodeData::BinaryOperator {
        operator,
        left,
        right,
        ..
      } if operator.is_assignment() => {
        let (operator, left, right) = (*operator, *left, *right);
        self.check_assignment(context, node, operator, left, right);
      }
      // Anything else is an expression statement: typing it is what reports
      // the calls inside it.
      _ => {
        self.expression_type(scope, source, node);
      }
    }
  }

  fn check_declaration(
    &mut self,
    context: &Context,
    node: NodeId,
    declaration: &oj_syntax::ast::Declaration,
  ) {
    let (scope, source) = (context.scope, context.source);
    // A local struct, enum or procedure is a declaration like any other; its
    // own type was worked out when the scope tree's declaration was typed.
    let Some(expression) = declaration.expression else {
      return;
    };
    if declaration
      .flags
      .contains(DeclarationFlags::IS_UNINITIALIZED)
    {
      return;
    }
    let Some(type_inst) = declaration.type_inst else {
      self.expression_type(scope, source, expression);
      return;
    };

    let target = self.type_from_node(scope, source, type_inst);
    let value = self.expression_type(scope, source, expression);
    if self.implicit_conversion(&value, target).is_some() {
      return;
    }
    let span = self
      .ast(source)
      .map_or(Span::at(0), |ast| ast.node(node).span);
    let name = declaration
      .name
      .and_then(|node| self.ident_name(source, node))
      .map(|name| self.symbol_text(name));
    self.report_mismatch(source, span, target, value.type_id);
    if let Some(name) = name {
      let (wanted, given) = (self.type_name(target), self.type_name(value.type_id));
      self.info(
        source,
        span,
        format!("... in declaration of '{name}' (declared type: {wanted}; rvalue type: {given})"),
      );
    }
  }

  fn check_assignment(
    &mut self,
    context: &Context,
    node: NodeId,
    operator: OperatorType,
    left: NodeId,
    right: NodeId,
  ) {
    let (scope, source) = (context.scope, context.source);
    let target = self.expression_type(scope, source, left);
    let value = self.expression_type(scope, source, right);
    // A compound assignment goes through the operator, whose result may be
    // wider than the target; only a plain `=` is a straight conversion.
    if operator != OperatorType::ASSIGN {
      return;
    }
    if self.implicit_conversion(&value, target.type_id).is_some() {
      return;
    }
    let span = self
      .ast(source)
      .map_or(Span::at(0), |ast| ast.node(node).span);
    self.report_mismatch(source, span, target.type_id, value.type_id);
  }

  fn check_return(&mut self, context: &Context, node: NodeId, arguments: &[Argument]) {
    let (scope, source) = (context.scope, context.source);
    for (index, argument) in arguments.iter().enumerate() {
      let value = self.expression_type(scope, source, argument.expression);
      // `return format, compressed = true;` names the return it fills, which
      // the procedure type does not record (**L§7.2**).
      if argument.name.is_some() {
        continue;
      }
      let Some(target) = context.returns.get(index).copied() else {
        continue;
      };
      if self.implicit_conversion(&value, target).is_some() {
        continue;
      }
      let span = self
        .ast(source)
        .map_or(Span::at(0), |ast| ast.node(node).span);
      self.report_mismatch(source, span, target, value.type_id);
    }
  }

  /// `if`, `while` and `!` accept any value with a truth value (**L§5.9**).
  fn check_condition(&mut self, context: &Context, node: NodeId) {
    let (scope, source) = (context.scope, context.source);
    let value = self.expression_type(scope, source, node);
    if self.has_truth_value(value.type_id) {
      return;
    }
    let span = self
      .ast(source)
      .map_or(Span::at(0), |ast| ast.node(node).span);
    let given = self.type_name(value.type_id);
    self.error(
      source,
      span,
      format!("Type {given} cannot implicitly coerce to bool."),
    );
  }

  fn has_truth_value(&mut self, type_id: TypeId) -> bool {
    use oj_types::{ArrayKind, TypeKind};
    let underlying = self.types().underlying(type_id);
    match self.types().kind(underlying) {
      TypeKind::Bool
      | TypeKind::Integer(_)
      | TypeKind::Float(_)
      | TypeKind::Enum(_)
      | TypeKind::Pointer(_)
      | TypeKind::Procedure(_)
      | TypeKind::String
      | TypeKind::Code => true,
      TypeKind::Array { kind, .. } => !matches!(kind, ArrayKind::Fixed(_)),
      // A struct has no truth value of its own (**L§5.9**), but `operator !`
      // may give it one, and operator overloading is M7.
      TypeKind::Struct(_) | TypeKind::Any => true,
      // A literal, a polymorph variable or a type the front end has not worked
      // out is not something to complain about.
      TypeKind::Unknown
      | TypeKind::Polymorph(_)
      | TypeKind::UntypedInt
      | TypeKind::UntypedFloat(_)
      | TypeKind::UntypedEnum
      | TypeKind::UntypedLiteral
      | TypeKind::OverloadSet => true,
      _ => false,
    }
  }

  pub(crate) fn report_mismatch(
    &mut self,
    source: SourceId,
    span: Span,
    wanted: TypeId,
    given: TypeId,
  ) {
    // An overload set is not a type: a name that stands for several procedures
    // is narrowed by the call site, not by an assignment (**L§7.5**).
    if wanted == TypeId::OVERLOAD_SET || given == TypeId::OVERLOAD_SET {
      return;
    }
    // Two integers that differ only in signedness get their own wording
    // (**C§12**).
    let signedness = match (
      self.types().integer_kind(wanted),
      self.types().integer_kind(given),
    ) {
      (Some(left), Some(right)) => left.is_signed() != right.is_signed(),
      _ => false,
    };
    let (wanted, given) = (self.type_name(wanted), self.type_name(given));
    let message = if signedness {
      format!("Number signedness mismatch. Type wanted: {wanted}; type given: {given}.")
    } else {
      format!("Type mismatch. Type wanted: {wanted}; type given: {given}.")
    };
    self.error(source, span, message);
  }
}

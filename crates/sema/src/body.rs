use oj_diag::{SourceId, Span};
use oj_scope::{DeclId, DeclKind, ScopeId};
use oj_syntax::ast::{
  Argument, AsmOperandKind, AsmSize, DeclarationFlags, NodeData, NodeId, OperatorType,
  ProcedureFlags,
};
use oj_types::TypeId;

use crate::checker::Checker;

/// What a `return` in the statement being checked has to match.
#[derive(Clone)]
struct Context {
  returns: Vec<TypeId>,
  /// Whether each return value was written with a default, which is what lets
  /// a `return x;` leave the rest of them out (**L§7.2**).
  return_defaults: Vec<bool>,
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
  ///
  /// A `#run,stallable` says it may be waiting for a declaration nothing has
  /// produced yet (**L§12.1**). orangejuice has no interpreter to suspend
  /// (`docs/spec.md` §6.5), so what the marker buys instead is *order*: every
  /// run that cannot wait goes first, and the ones that said they could go
  /// after, by which time whatever the others declared is there.
  pub(crate) fn check_file_runs(&mut self) {
    let units: Vec<(SourceId, ScopeId, NodeId)> = self
      .program()
      .units()
      .collect::<Vec<_>>()
      .iter()
      .map(|unit| (unit.source, unit.scope, unit.parsed.root))
      .collect();
    for stallable in [false, true] {
      self.stallable_runs = stallable;
      for (source, scope, root) in &units {
        self.check_runs_in(*source, *scope, *root);
      }
    }
    self.stallable_runs = false;
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
      NodeData::DirectiveRun(run) => {
        // This pass is either the one that runs what cannot wait or the one
        // that runs what can; the other pass takes the rest.
        let stallable = run.flags.contains(oj_syntax::ast::RunFlags::STALLABLE);
        if stallable != self.stallable_runs {
          return;
        }
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
    if payload
      .if_flags
      .contains(oj_syntax::ast::IfFlags::IS_SWITCH_STATEMENT)
    {
      return self.static_switch_branch(condition_scope, source, payload);
    }
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

  /// `#if x == { case a; … case; … }` (**L§6.10**): the first case whose value
  /// equals the subject is taken, `#through` continues into the next one, and
  /// a case with no value is the default. Only the cases taken are typechecked
  /// or lowered, which is what lets `Basic`'s `exit` write a `#asm` for Linux
  /// and a libc call for everything else.
  fn static_switch_branch(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    payload: &oj_syntax::ast::IfNode,
  ) -> Option<Vec<NodeId>> {
    let subject = self
      .expression_type(scope, source, payload.condition)
      .constant?;
    let block = payload.then_block?;
    let NodeData::Block(cases) = self.ast(source)?.data(block) else {
      return None;
    };
    let cases = cases.statements.clone();

    let mut taken = Vec::new();
    let mut matching = false;
    let mut default = None;
    for case in cases {
      let NodeData::Case(entry) = self.ast(source)?.data(case) else {
        continue;
      };
      let entry = entry.clone();
      if !matching {
        match entry.condition {
          Some(condition) => {
            let value = self.expression_type(scope, source, condition).constant?;
            if !self.same_constant(&subject, &value) {
              continue;
            }
            matching = true;
          }
          None => {
            default = Some(entry.then_block);
            continue;
          }
        }
      }
      taken.push(entry.then_block);
      if !entry.marked_as_fallthrough {
        return Some(taken);
      }
    }
    if matching {
      return Some(taken);
    }
    Some(default.into_iter().collect())
  }

  /// Whether a case's value is the subject's. A `.NAME` written in a case has
  /// no enum of its own until the subject supplies one (**L§5.12**).
  fn same_constant(
    &mut self,
    subject: &crate::constants::Const,
    value: &crate::constants::Const,
  ) -> bool {
    let normalize =
      |checker: &mut Self, value: &crate::constants::Const, target: TypeId| match &value.value {
        crate::constants::Value::EnumName(name) => {
          let name = *name;
          checker
            .types()
            .enum_of(checker.types().underlying(target))
            .and_then(|definition| checker.types().enum_info(definition).value_of(name))
            .map(|member| crate::constants::Value::Int(i128::from(member)))
        }
        other => Some(other.clone()),
      };
    let subject_value = normalize(self, subject, subject.type_id);
    let case_value = normalize(self, value, subject.type_id);
    match (subject_value, case_value) {
      (Some(left), Some(right)) => left == right,
      _ => false,
    }
  }

  fn check_procedure(&mut self, procedure: DeclId) {
    let decl = self.program().tree().decl(procedure);
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

    let defaults = self.return_defaults(source, &header.returns);
    self.check_procedure_body(
      source,
      decl.scope,
      *block,
      signature.returns.clone(),
      defaults,
    );
  }

  /// Whether each of a header's return values was written with a default
  /// (**L§7.2**).
  pub(crate) fn return_defaults(&self, source: SourceId, returns: &[NodeId]) -> Vec<bool> {
    let Some(ast) = self.ast(source) else {
      return vec![false; returns.len()];
    };
    returns
      .iter()
      .map(|node| match ast.data(*node) {
        NodeData::Declaration(declaration) => declaration.expression.is_some(),
        _ => false,
      })
      .collect()
  }

  /// The statements of one body, checked against what its `return`s have to
  /// match. An instantiation reaches this with its own specialized returns
  /// (**L§7.8**).
  pub(crate) fn check_procedure_body(
    &mut self,
    source: SourceId,
    scope: ScopeId,
    block: NodeId,
    returns: Vec<TypeId>,
    return_defaults: Vec<bool>,
  ) {
    let context = Context {
      returns,
      return_defaults,
      scope,
      source,
    };
    self.while_checking_body(|checker| checker.check_statement(&context, block));
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
      // An `#insert` splices a program into the block around it (**L§13.2**);
      // what it spliced is checked here, in the scope it was admitted to.
      NodeData::DirectiveInsert(_) => {
        self.check_insert(context, node);
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
      // The operands of an `#asm` block are expressions of the block around
      // it (**L§15**).
      NodeData::Asm(_) => self.check_asm(context, node),
      // Anything else is an expression statement: typing it is what reports
      // the calls inside it.
      _ => {
        self.expression_type(scope, source, node);
      }
    }
  }

  /// An `#asm` block's operands are ordinary expressions (**L§15**): typing
  /// them here is what tells the back end how wide a high-level operand is and
  /// what a `?T` size means, and what makes a name written in one report the
  /// way any other undeclared name would.
  fn check_asm(&mut self, context: &Context, node: NodeId) {
    let (scope, source) = (context.scope, context.source);
    let Some(ast) = self.ast(source) else {
      return;
    };
    let NodeData::Asm(block) = ast.data(node) else {
      return;
    };
    let block = block.clone();
    for instruction in &block.instructions {
      if let AsmSize::Of(expression) = instruction.size {
        self.expression_type(scope, source, expression);
      }
      for operand in &instruction.operands {
        let expressions = match &operand.kind {
          AsmOperandKind::Declaration(_) => Vec::new(),
          AsmOperandKind::Pin { name, .. } => vec![*name],
          AsmOperandKind::Expression(expression) => vec![*expression],
          AsmOperandKind::Memory(memory) => [
            Some(memory.base),
            memory.index,
            memory.scale,
            memory.displacement,
          ]
          .into_iter()
          .flatten()
          .collect(),
        };
        for expression in expressions
          .into_iter()
          .chain(operand.mask.iter().map(|mask| mask.register))
        {
          self.expression_type(scope, source, expression);
        }
      }
    }
  }

  /// Checks what an `#insert` spliced into the block around it. The statements
  /// are the caller's, so they are checked with the caller's return types and
  /// in the scope they were admitted to (**L§13.2**).
  fn check_insert(&mut self, context: &Context, node: NodeId) {
    // The loop-control replacements stand where the `#insert` was written, so
    // they are checked here rather than wherever the spliced body puts them
    // (**L§13.2**).
    if let Some(NodeData::DirectiveInsert(insert)) =
      self.ast(context.source).map(|ast| ast.data(node))
    {
      let replacements = [
        insert.break_replacement,
        insert.continue_replacement,
        insert.remove_replacement,
      ];
      for replacement in replacements.into_iter().flatten() {
        self.check_statement(context, replacement);
      }
    }
    let Some(expansion) = self.expand_insert(context.scope, context.source, node) else {
      return;
    };
    let Some(ast) = self.ast(expansion.source) else {
      return;
    };
    let NodeData::Block(block) = ast.data(expansion.root) else {
      return;
    };
    let statements = block.statements.clone();
    let inner = Context {
      returns: context.returns.clone(),
      return_defaults: context.return_defaults.clone(),
      scope: expansion.scope,
      source: expansion.source,
    };
    for statement in statements {
      self.check_statement(&inner, statement);
    }
  }

  /// A local whose initializer only means something once a macro has expanded
  /// could not be typed in the pass that runs before any body is checked — a
  /// `` `x `` the macro declares does not exist until then (**L§7.13**) — so
  /// what it turned out to be is recorded now. `state := context.simp` after a
  /// `CheckInit()` is the shape, which is how `Simp` reaches its own state.
  fn record_late_local(
    &mut self,
    node: NodeId,
    source: SourceId,
    declaration: &oj_syntax::ast::Declaration,
    value: &crate::checker::Expr,
  ) {
    if value.is_unknown() {
      return;
    }
    let Some(decl) = self.decl_at(source, node) else {
      return;
    };
    let existing = self.decl_type(decl).value;
    if !self.types().is_unknown(existing) {
      return;
    }
    let constant = declaration.flags.contains(DeclarationFlags::IS_CONSTANT);
    let resolved = match (constant, value.denoted) {
      (true, Some(denoted)) => crate::checker::DeclType::type_name(denoted),
      (true, None) => crate::checker::DeclType::value(value.type_id),
      (false, _) => crate::checker::DeclType::value(self.harden(value.type_id)),
    };
    self.publish(decl, resolved);
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
      let value = self.expression_type(scope, source, expression);
      self.record_late_local(node, source, declaration, &value);
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
    self.check_return_arity(context, node, arguments);
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

  /// A `return` has to fill every return value that was not written with a
  /// default (**L§7.2**). `return f();` hands on all of `f`'s values at once,
  /// so what counts is how many values were produced rather than how many
  /// expressions were written.
  fn check_return_arity(&mut self, context: &Context, node: NodeId, arguments: &[Argument]) {
    let wanted = context.returns.len();
    // A named return fills a slot of its own, which the count does not say
    // anything about; and a return type nothing has worked out yet is not
    // something to measure against.
    if arguments.iter().any(|argument| argument.name.is_some())
      || context
        .returns
        .iter()
        .any(|type_id| self.mentions_unknown(*type_id))
    {
      return;
    }
    let required = context
      .return_defaults
      .iter()
      .rposition(|has_default| !has_default)
      .map_or(0, |last| last + 1);
    if required == 0 {
      return;
    }
    let given = match arguments {
      [only] => match self.values_produced(context, only.expression) {
        Some(count) => count,
        None => return,
      },
      other => other.len(),
    };
    if given >= required {
      return;
    }
    let span = self
      .ast(context.source)
      .map_or(Span::at(0), |ast| ast.node(node).span);
    self.error(
      context.source,
      span,
      format!("Not enough return values: Wanted {wanted}, got {given}."),
    );
  }

  /// How many values one expression stands for: a call hands on every return
  /// it has (**L§7.2**), and everything else is one value. A call that did not
  /// resolve stands for nothing anyone can count, so it is `None`.
  fn values_produced(&mut self, context: &Context, expression: NodeId) -> Option<usize> {
    let (scope, source) = (context.scope, context.source);
    let ast = self.ast(source)?;
    match ast.data(expression) {
      NodeData::ProcedureCall(_) => match self.call_return_types(scope, source, expression).len() {
        0 => None,
        count => Some(count),
      },
      _ => Some(1),
    }
  }

  /// `if`, `while` and `!` accept any value with a truth value (**L§5.9**).
  fn check_condition(&mut self, context: &Context, node: NodeId) {
    let (scope, source) = (context.scope, context.source);
    // `while name := expression` declares the loop's name and tests the value
    // it takes (**L§6.4**).
    if let Some(NodeData::Declaration(_)) = self.ast(source).map(|ast| ast.data(node)) {
      self.check_statement(context, node);
      let declared = self
        .decl_at(source, node)
        .map(|decl| self.decl_type(decl).value)
        .unwrap_or(TypeId::UNKNOWN);
      if !self.has_truth_value(declared) && !self.mentions_unknown(declared) {
        let span = self
          .ast(source)
          .map_or(Span::at(0), |ast| ast.node(node).span);
        let given = self.type_name(declared);
        self.error(
          source,
          span,
          format!("Type {given} cannot implicitly coerce to bool."),
        );
      }
      return;
    }
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
    use oj_types::TypeKind;
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
      // A view and a resizable array are true when their count is; a fixed
      // array's count is a constant, so its truth is decided here (**L§5.9**)
      // — measured against the reference, which is what `Treemap`'s `if
      // node.border` over a `[4] float` reads.
      TypeKind::Array { .. } => true,
      // An `Any` is a pair, and the reference reads one as a condition.
      TypeKind::Any => true,
      // A struct never has a truth value, even with an operator of its own
      // (**L§5.9**); `!x` reaches `operator !` before this is asked.
      TypeKind::Struct(_) => false,
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
    // Two integers get their own wording: one that differs in signedness, and
    // one that would not fit (**C§12**).
    // A string that is not a literal has bytes the compiler did not lay down,
    // so there is nothing for a `*u8` to point at (**L§3.4**, **C§12**).
    if given == TypeId::STRING
      && self
        .types()
        .pointee(wanted)
        .is_some_and(|pointee| matches!(pointee, TypeId::U8 | TypeId::S8))
    {
      let wanted = self.type_name(wanted);
      self.error(
        source,
        span,
        format!("Dynamically-computed strings do not cast to {wanted}; only literals do."),
      );
      return;
    }
    // An untyped literal is reported as what it defaults to, so its width is
    // that type's (**L§5.10**).
    let (wanted_type, given_type) = (self.harden(wanted), self.harden(given));
    let kinds = (
      self.types().integer_kind(wanted_type),
      self.types().integer_kind(given_type),
    );
    let signedness = match kinds {
      (Some(left), Some(right)) => left.is_signed() != right.is_signed(),
      _ => false,
    };
    let narrowing = match kinds {
      (Some(left), Some(right)) => (!signedness).then(|| (right.size() * 8, left.size() * 8)),
      _ => None,
    }
    .filter(|(from, to)| from > to);
    let (wanted, given) = (self.type_name(wanted), self.type_name(given));
    let message = if signedness {
      format!("Number signedness mismatch. Type wanted: {wanted}; type given: {given}.")
    } else if let Some((from, to)) = narrowing {
      format!(
        "Loss of information (trying to fit {from} bits into {to} bits). Can't do this without \
         an explicit cast. Type wanted: {wanted}; type given: {given}."
      )
    } else {
      format!("Type mismatch. Type wanted: {wanted}; type given: {given}.")
    };
    self.error(source, span, message);
  }
}

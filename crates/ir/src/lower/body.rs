//! Procedures and statements: the shape of a body, and the control flow it
//! turns into.

use super::*;

use crate::ir::{Abi, AbiParameter, ParameterKind};

impl Lowering<'_, '_> {
  /// The machine-level parameter list of a procedure type, with every type it
  /// mentions laid out first.
  pub(super) fn abi_of(&mut self, type_id: TypeId, flags: ProcedureFlags) -> Abi {
    if let Some(signature) = self.checker.types().procedure_of(type_id).cloned() {
      for member in signature.arguments.iter().chain(&signature.returns) {
        self.checker.layout(*member);
      }
    }
    self.checker.layout(self.context_type);
    crate::ir::abi_of(self.checker.types(), type_id, flags, self.context_type)
  }

  /// Clears the per-procedure state, so that one lowering can walk one body
  /// after another.
  pub(super) fn start_procedure(&mut self) {
    self.blocks = vec![Block::new()];
    self.locals.clear();
    self.value_types.clear();
    self.current = BlockId(0);
    self.local_of_decl.clear();
    self.loops.clear();
    self.defers.clear();
    self.returns.clear();
    self.return_pointers.clear();
    self.context_value = None;
  }

  pub(super) fn lower_procedure(&mut self, id: ProcId, key: ProcKey) {
    self.start_procedure();
    let body = match key {
      ProcKey::Decl(decl) => self.checker.procedure_body(decl),
      ProcKey::Node(source, header) => {
        let scope = self.checker.scope_for(source, header, self.body_scope);
        self.checker.procedure_body_at(source, header, scope)
      }
    };
    let Some(body) = body else {
      return;
    };
    let Some(block) = body.block else {
      return;
    };
    let type_id = self.procedures[id.0 as usize].type_id;
    let flags = self.procedures[id.0 as usize].flags;

    if self.mentions_unknown(type_id) {
      let name = self.procedures[id.0 as usize].name.clone();
      self.error(
        body.source,
        body.header,
        format!(
          "Code generation for '{name}' needs a type the front end cannot work out yet \
           (polymorphs and macros are milestone M7, '#run' is milestone M6)."
        ),
      );
      return;
    }

    let abi = self.abi_of(type_id, flags);
    let signature = self
      .checker
      .types()
      .procedure_of(type_id)
      .cloned()
      .unwrap_or_else(|| oj_types::ProcedureType::new(Vec::new(), Vec::new()));
    self.returns = signature.returns.clone();
    self.body_source = body.source;
    self.body_scope = body.scope;

    // The machine parameters are the first values of the procedure, in order.
    let mut incoming = Vec::with_capacity(abi.parameters.len());
    for parameter in &abi.parameters {
      let type_id = match parameter.kind {
        ParameterKind::Value => parameter.type_id,
        _ => self.pointer_to(parameter.type_id),
      };
      incoming.push(self.value(type_id));
    }

    let mut declared = 0usize;
    for (index, parameter) in abi.parameters.iter().enumerate() {
      match parameter.kind {
        ParameterKind::Context => self.context_value = Some(incoming[index]),
        ParameterKind::ReturnPointer => self.return_pointers.push(incoming[index]),
        ParameterKind::Value | ParameterKind::Pointer => {
          let name = body
            .parameters
            .get(declared)
            .and_then(|id| *id)
            .map(|id| {
              let symbol = self.checker.program().tree().decl(id).name;
              self.text(symbol)
            })
            .unwrap_or_else(|| format!("arg{declared}"));
          let local = self.new_local(name, parameter.type_id);
          let address = self.local_address(local);
          let value = Val {
            id: incoming[index],
            type_id: parameter.type_id,
            indirect: parameter.kind == ParameterKind::Pointer,
          };
          self.store(address, value);
          if let Some(Some(id)) = body.parameters.get(declared) {
            self.local_of_decl.insert(*id, local);
          }
          declared += 1;
        }
      }
    }

    // A named return value is a local the body may assign before returning
    // (**L§7.2**).
    for (index, name) in body.returns.iter().enumerate() {
      let Some(id) = name else { continue };
      let Some(type_id) = signature.returns.get(index).copied() else {
        continue;
      };
      let symbol = self.checker.program().tree().decl(*id).name;
      let text = self.text(symbol);
      let local = self.new_local(text, type_id);
      let address = self.local_address(local);
      self.clear(address, type_id);
      self.local_of_decl.insert(*id, local);
    }

    self.defers.push(Vec::new());
    self.statement(block);
    let scope = self.defers.pop().unwrap_or_default();
    if !self.terminated() {
      self.run_defers(&scope);
      self.implicit_return(&body);
    }

    let procedure = &mut self.procedures[id.0 as usize];
    procedure.abi = abi;
    procedure.locals = std::mem::take(&mut self.locals);
    procedure.blocks = std::mem::take(&mut self.blocks);
    procedure.value_types = std::mem::take(&mut self.value_types);
    procedure.entry = BlockId(0);
  }

  /// Falling off the end of a body returns the named return values, or
  /// nothing (**L§7.2**).
  fn implicit_return(&mut self, body: &ProcedureBody) {
    let returns = self.returns.clone();
    if returns.is_empty() {
      self.terminate(Terminator::Return(Vec::new()));
      return;
    }
    let mut values = Vec::new();
    for (index, type_id) in returns.iter().enumerate() {
      let named = body.returns.get(index).and_then(|id| *id);
      let value = match named.and_then(|id| self.local_of_decl.get(&id).copied()) {
        Some(local) => {
          let address = self.local_address(local);
          Val {
            id: address,
            type_id: *type_id,
            indirect: true,
          }
        }
        None => {
          let local = self.new_local(String::from("result"), *type_id);
          let address = self.local_address(local);
          self.clear(address, *type_id);
          Val {
            id: address,
            type_id: *type_id,
            indirect: true,
          }
        }
      };
      values.push(value);
    }
    self.emit_return(values);
  }

  /// The first return travels in registers when it fits; everything else is
  /// written through the storage the caller passed (**L§7.2**).
  fn emit_return(&mut self, values: Vec<Val>) {
    let returns_directly = values
      .first()
      .map(|value| value.type_id)
      .is_some_and(|type_id| self.is_scalar(type_id));
    let mut direct = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
      if index == 0 && returns_directly {
        direct.push(self.scalar(value));
        continue;
      }
      let slot = if returns_directly { index - 1 } else { index };
      let Some(pointer) = self.return_pointers.get(slot).copied() else {
        continue;
      };
      self.store(pointer, value);
    }
    self.terminate(Terminator::Return(direct));
  }

  fn mentions_unknown(&mut self, type_id: TypeId) -> bool {
    match self.checker.types().kind(type_id).clone() {
      TypeKind::Unknown | TypeKind::Polymorph(_) | TypeKind::OverloadSet => true,
      TypeKind::Pointer(pointee) => self.mentions_unknown(pointee),
      TypeKind::Array { element, .. } => self.mentions_unknown(element),
      TypeKind::Procedure(signature) => signature
        .arguments
        .iter()
        .chain(&signature.returns)
        .any(|type_id| self.mentions_unknown(*type_id)),
      _ => false,
    }
  }

  // ------------------------------------------------------------ statements ---

  pub(super) fn statement(&mut self, node: NodeId) {
    if self.terminated() {
      return;
    }
    let source = self.body_source;
    let Some(ast) = self.checker.tree_of(source) else {
      return;
    };
    match ast.data(node) {
      NodeData::Block(block) => {
        let statements = block.statements.clone();
        self.defers.push(Vec::new());
        for statement in statements {
          self.statement(statement);
        }
        let scope = self.defers.pop().unwrap_or_default();
        if !self.terminated() {
          self.run_defers(&scope);
        }
      }
      NodeData::Declaration(_) => self.declaration_statement(node),
      NodeData::CompoundDeclaration(_) => self.compound_declaration(node),
      NodeData::Return { arguments, .. } => {
        let arguments = arguments.clone();
        self.return_statement(node, &arguments);
      }
      NodeData::If(payload) => {
        let payload = payload.clone();
        if payload.if_flags.contains(IfFlags::IS_SWITCH_STATEMENT) {
          self.switch_statement(node, &payload);
        } else {
          self.if_statement(&payload);
        }
      }
      NodeData::While { condition, block } => {
        let (condition, block) = (*condition, *block);
        self.while_statement(condition, block);
      }
      NodeData::For(payload) => {
        let payload = payload.clone();
        self.for_statement(node, &payload);
      }
      NodeData::LoopControl {
        control_type,
        target_ident,
      } => {
        let (control_type, target) = (*control_type, *target_ident);
        self.loop_control(node, control_type, target);
      }
      NodeData::Defer { block, .. } => {
        let block = *block;
        let scope = self.checker.scope_for(source, block, self.body_scope);
        if let Some(frame) = self.defers.last_mut() {
          frame.push((scope, source, block));
        }
      }
      NodeData::Using(_) | NodeData::Note { .. } | NodeData::DirectiveScope { .. } => {}
      NodeData::BinaryOperator {
        operator,
        left,
        right,
        ..
      } if operator.is_assignment() => {
        let (operator, left, right) = (*operator, *left, *right);
        self.assignment(node, operator, left, right);
      }
      NodeData::PushContext { .. } => {
        self.unsupported(source, node, "'push_context'", "M6");
      }
      // A `#run` statement already happened, when the front end typechecked
      // the body around it (**L§6.11**); the executable holds nothing for it.
      NodeData::DirectiveRun(_) => {}
      NodeData::DirectiveInsert(_) => {
        self.unsupported(source, node, "'#insert'", "M6");
      }
      _ => {
        let scope = self.checker.scope_for(source, node, self.body_scope);
        self.expression(scope, source, node, None);
      }
    }
  }

  fn run_defers(&mut self, frame: &[(ScopeId, SourceId, NodeId)]) {
    for (scope, source, node) in frame.iter().rev() {
      let (previous_scope, previous_source) = (self.body_scope, self.body_source);
      self.body_scope = *scope;
      self.body_source = *source;
      self.statement(*node);
      self.body_scope = previous_scope;
      self.body_source = previous_source;
    }
  }

  /// Everything a jump out of `depth` scopes has to run first (**L§6.6**).
  fn run_defers_to(&mut self, depth: usize) {
    let frames: Vec<Vec<(ScopeId, SourceId, NodeId)>> =
      self.defers[depth..].iter().rev().cloned().collect();
    for frame in frames {
      self.run_defers(&frame);
    }
  }

  fn declaration_statement(&mut self, node: NodeId) {
    let source = self.body_source;
    let Some(NodeData::Declaration(declaration)) =
      self.checker.tree_of(source).map(|ast| ast.data(node))
    else {
      return;
    };
    let declaration = declaration.clone();
    let Some(decl) = self.checker.decl_at(source, node) else {
      return;
    };
    let kind = self.checker.program().tree().decl(decl).kind;
    // A local constant, struct, enum or procedure declares no storage; the
    // scope tree gave it a declaration of its own already.
    if kind != DeclKind::Variable || declaration.flags.contains(DeclarationFlags::IS_CONSTANT) {
      return;
    }
    let type_id = self.checker.decl_type(decl).value;
    if self.mentions_unknown(type_id) {
      self.unsupported(source, node, "a declaration of unknown type", "M6/M7");
      return;
    }
    let name = {
      let symbol = self.checker.program().tree().decl(decl).name;
      self.text(symbol)
    };
    let local = self.new_local(name, type_id);
    self.local_of_decl.insert(decl, local);
    let address = self.local_address(local);

    if declaration
      .flags
      .contains(DeclarationFlags::IS_UNINITIALIZED)
    {
      return;
    }
    let Some(expression) = declaration.expression else {
      self.default_initialize(address, type_id);
      return;
    };
    let scope = self.checker.scope_for(source, expression, self.body_scope);
    if let Some(value) = self.expression(scope, source, expression, Some(type_id)) {
      self.store(address, value);
    }
  }

  /// `a, b := f();` distributes a call's returns across its names
  /// (**L§4.5**).
  fn compound_declaration(&mut self, node: NodeId) {
    let source = self.body_source;
    let Some(NodeData::CompoundDeclaration(compound)) =
      self.checker.tree_of(source).map(|ast| ast.data(node))
    else {
      return;
    };
    let compound = compound.clone();
    let Some(NodeData::Declaration(properties)) = self
      .checker
      .tree_of(source)
      .map(|ast| ast.data(compound.declaration_properties))
    else {
      return;
    };
    let properties = properties.clone();
    let Some(NodeData::CommaSeparatedArguments { arguments }) = self
      .checker
      .tree_of(source)
      .map(|ast| ast.data(compound.comma_separated_assignment))
    else {
      return;
    };
    let names: Vec<NodeId> = arguments.iter().map(|argument| argument.node).collect();

    let mut places = Vec::new();
    for name in &names {
      let Some(decl) = self.checker.decl_at(source, *name) else {
        places.push(None);
        continue;
      };
      let type_id = self.checker.decl_type(decl).value;
      let text = {
        let symbol = self.checker.program().tree().decl(decl).name;
        self.text(symbol)
      };
      let local = self.new_local(text, type_id);
      self.local_of_decl.insert(decl, local);
      let address = self.local_address(local);
      places.push(Some((address, type_id)));
    }

    let Some(expression) = properties.expression else {
      for place in places.into_iter().flatten() {
        self.default_initialize(place.0, place.1);
      }
      return;
    };
    let scope = self.checker.scope_for(source, expression, self.body_scope);
    let targets: Vec<Option<(ValueId, TypeId)>> = places;
    let is_call = matches!(
      self.checker.tree_of(source).map(|ast| ast.data(expression)),
      Some(NodeData::ProcedureCall(_))
    );
    if is_call {
      self.call_into(scope, source, expression, &targets);
      return;
    }
    // `a, b := 1, 2;` is not a call: each name takes its own value.
    if let Some(Some((address, type_id))) = targets.first().copied()
      && let Some(value) = self.expression(scope, source, expression, Some(type_id))
    {
      self.store(address, value);
    }
  }

  fn return_statement(&mut self, node: NodeId, arguments: &[ast::Argument]) {
    let source = self.body_source;
    let returns = self.returns.clone();
    let mut values = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
      let target = returns.get(index).copied();
      let scope = self
        .checker
        .scope_for(source, argument.expression, self.body_scope);
      match self.expression(scope, source, argument.expression, target) {
        Some(value) => values.push(value),
        None => return,
      }
    }
    // A `return` inside nested blocks runs every defer on the way out
    // (**L§6.6**).
    if values.len() < returns.len() {
      self.unsupported(source, node, "a return that names its values", "M7");
      return;
    }
    self.run_defers_to(0);
    self.emit_return(values);
  }

  fn if_statement(&mut self, payload: &ast::IfNode) {
    let source = self.body_source;
    let scope = self
      .checker
      .scope_for(source, payload.condition, self.body_scope);
    // A `#if` the front end decided contributes only the branch it kept, and
    // no branch at all (**L§6.10**); the rejected one was never typechecked,
    // so there is nothing here to lower.
    if let Some(branches) = self.checker.static_if_branches(scope, source, payload) {
      for branch in branches {
        self.statement(branch);
      }
      return;
    }
    let Some(condition) = self.condition(scope, source, payload.condition) else {
      return;
    };
    let then_block = self.new_block();
    let else_block = self.new_block();
    let join = self.new_block();
    self.terminate(Terminator::Branch {
      condition,
      then_block,
      else_block,
    });

    self.current = then_block;
    if let Some(block) = payload.then_block {
      self.statement(block);
    }
    self.terminate(Terminator::Jump(join));

    self.current = else_block;
    if let Some(block) = payload.else_block {
      self.statement(block);
    }
    self.terminate(Terminator::Jump(join));

    self.current = join;
  }

  /// `if value == { case a; … }` (**L§6.4**): a chain of comparisons against
  /// one value, where a case falls through only when it says so.
  fn switch_statement(&mut self, node: NodeId, payload: &ast::IfNode) {
    let source = self.body_source;
    let scope = self
      .checker
      .scope_for(source, payload.condition, self.body_scope);
    let Some(subject) = self.expression(scope, source, payload.condition, None) else {
      return;
    };
    let subject_type = subject.type_id;
    let subject = self.scalar(subject);
    let Some(block) = payload.then_block else {
      return;
    };
    let Some(NodeData::Block(body)) = self.checker.tree_of(source).map(|ast| ast.data(block))
    else {
      return;
    };
    let cases: Vec<NodeId> = body.statements.clone();
    let join = self.new_block();

    let mut bodies = Vec::new();
    for _ in &cases {
      bodies.push(self.new_block());
    }

    for (index, case) in cases.iter().enumerate() {
      let Some(NodeData::Case(payload)) = self.checker.tree_of(source).map(|ast| ast.data(*case))
      else {
        self.unsupported(source, *case, "a 'case' the parser did not produce", "M5");
        continue;
      };
      let payload = payload.clone();
      match payload.condition {
        Some(condition) => {
          let scope = self.checker.scope_for(source, condition, self.body_scope);
          let Some(value) = self.expression(scope, source, condition, Some(subject_type)) else {
            continue;
          };
          let value = self.scalar(value);
          let compared = self.value(TypeId::BOOL);
          self.emit(Inst::Binary {
            dest: compared,
            operator: crate::ir::BinaryOp::Equal,
            left: subject,
            right: value,
          });
          let next = self.new_block();
          self.terminate(Terminator::Branch {
            condition: compared,
            then_block: bodies[index],
            else_block: next,
          });
          self.current = next;
        }
        // `case;` with no value is the default arm.
        None => {
          self.terminate(Terminator::Jump(bodies[index]));
          self.current = self.new_block();
        }
      }
    }
    self.terminate(Terminator::Jump(join));

    let breaks = Loop {
      break_block: join,
      continue_block: join,
      depth: self.defers.len(),
    };
    self.loops.push(breaks);
    for (index, case) in cases.iter().enumerate() {
      let Some(NodeData::Case(payload)) = self.checker.tree_of(source).map(|ast| ast.data(*case))
      else {
        continue;
      };
      let payload = payload.clone();
      self.current = bodies[index];
      self.statement(payload.then_block);
      let next = match payload.marked_as_fallthrough {
        true => bodies.get(index + 1).copied().unwrap_or(join),
        false => join,
      };
      self.terminate(Terminator::Jump(next));
    }
    self.loops.pop();
    let _ = node;
    self.current = join;
  }

  fn while_statement(&mut self, condition: NodeId, block: NodeId) {
    let source = self.body_source;
    let head = self.new_block();
    let body = self.new_block();
    let exit = self.new_block();
    self.terminate(Terminator::Jump(head));

    self.current = head;
    let scope = self.checker.scope_for(source, condition, self.body_scope);
    match self.condition(scope, source, condition) {
      Some(value) => self.terminate(Terminator::Branch {
        condition: value,
        then_block: body,
        else_block: exit,
      }),
      None => self.terminate(Terminator::Jump(exit)),
    }

    self.current = body;
    self.loops.push(Loop {
      break_block: exit,
      continue_block: head,
      depth: self.defers.len(),
    });
    self.statement(block);
    self.loops.pop();
    self.terminate(Terminator::Jump(head));

    self.current = exit;
  }

  fn loop_control(&mut self, node: NodeId, control: LoopControlType, target: Option<NodeId>) {
    let source = self.body_source;
    if target.is_some() {
      self.unsupported(source, node, "a labelled 'break' or 'continue'", "M7");
      return;
    }
    let Some(entry) = self.loops.last() else {
      self.error(source, node, "'break' is not inside a loop.");
      return;
    };
    let (break_block, continue_block, depth) =
      (entry.break_block, entry.continue_block, entry.depth);
    match control {
      LoopControlType::Break => {
        self.run_defers_to(depth);
        self.terminate(Terminator::Jump(break_block));
      }
      LoopControlType::Continue => {
        self.run_defers_to(depth);
        self.terminate(Terminator::Jump(continue_block));
      }
      LoopControlType::Remove => {
        self.unsupported(source, node, "'remove'", "M7");
      }
    }
  }

  fn assignment(&mut self, node: NodeId, operator: OperatorType, left: NodeId, right: NodeId) {
    let source = self.body_source;
    let scope = self.checker.scope_for(source, left, self.body_scope);
    let Some(place) = self.place(scope, source, left) else {
      return;
    };
    if operator == OperatorType::ASSIGN {
      let scope = self.checker.scope_for(source, right, self.body_scope);
      if let Some(value) = self.expression(scope, source, right, Some(place.type_id)) {
        let address = place.id;
        self.store(address, value);
      }
      return;
    }

    let Some(binary) = compound_operator(operator) else {
      self.unsupported(source, node, "this assignment operator", "M7");
      return;
    };
    // `a &&= b` and `a ||= b` only assign when the operator would change the
    // value, which a plain read-modify-write already does (**L§5.2**).
    let scope = self.checker.scope_for(source, right, self.body_scope);
    let Some(value) = self.expression(scope, source, right, None) else {
      return;
    };
    let current = Val {
      id: place.id,
      type_id: place.type_id,
      indirect: true,
    };
    let Some(result) = self.binary_values(source, node, binary, current, value, place.type_id)
    else {
      return;
    };
    let address = place.id;
    self.store(address, result);
  }

  /// `for` over a range or over an array (**L§6.5**). A `for_expansion` and a
  /// reverse range are M7 and an error respectively.
  fn for_statement(&mut self, node: NodeId, payload: &ast::ForNode) {
    let source = self.body_source;
    if payload.want_replacement_for_expansion.is_some()
      || payload.want_pointer_expression.is_some()
      || payload.want_reverse_expression.is_some()
    {
      self.unsupported(source, node, "a 'for' with computed modifiers", "M7");
      return;
    }
    let reverse = payload.for_flags.contains(ForFlags::REVERSE);
    let by_pointer = payload.for_flags.contains(ForFlags::POINTER);

    let (it_decl, index_decl) = self.checker.loop_iterators(source, node);
    let index_local = self.new_local(String::from("it_index"), TypeId::S64);
    if let Some(id) = index_decl {
      self.local_of_decl.insert(id, index_local);
    }

    match payload.iteration_expression_right {
      Some(right) => self.range_loop(node, payload, right, reverse, index_local, it_decl),
      None => self.array_loop(node, payload, reverse, by_pointer, index_local, it_decl),
    }
  }

  fn range_loop(
    &mut self,
    node: NodeId,
    payload: &ast::ForNode,
    right: NodeId,
    reverse: bool,
    index_local: LocalId,
    it_decl: Option<DeclId>,
  ) {
    let source = self.body_source;
    if reverse {
      // The reference warns and iterates forwards; orangejuice reports it
      // (`docs/spec.md` §10).
      self.error(
        source,
        node,
        "A reverse 'for' over a range does not iterate backwards; write the range the other way.",
      );
      return;
    }
    let scope = self
      .checker
      .scope_for(source, payload.iteration_expression, self.body_scope);
    let Some(start) = self.expression(scope, source, payload.iteration_expression, None) else {
      return;
    };
    let it_type = it_decl
      .map(|id| self.checker.decl_type(id).value)
      .filter(|type_id| !self.checker.types().is_unknown(*type_id))
      .unwrap_or_else(|| self.checker.hardened(start.type_id));
    let Some(start) = self.convert(source, node, start, it_type) else {
      return;
    };
    let scope = self.checker.scope_for(source, right, self.body_scope);
    let Some(end) = self.expression(scope, source, right, Some(it_type)) else {
      return;
    };

    let it_local = self.new_local(String::from("it"), it_type);
    if let Some(id) = it_decl {
      self.local_of_decl.insert(id, it_local);
    }
    let it_address = self.local_address(it_local);
    self.store(it_address, start);
    let index_address = self.local_address(index_local);
    let zero = self.constant(Constant::Int(0), TypeId::S64);
    self.store(index_address, zero);
    let limit_local = self.new_local(String::from("limit"), it_type);
    let limit_address = self.local_address(limit_local);
    self.store(limit_address, end);

    let head = self.new_block();
    let body = self.new_block();
    let step = self.new_block();
    let exit = self.new_block();
    self.terminate(Terminator::Jump(head));

    self.current = head;
    let it_address = self.local_address(it_local);
    let limit_address = self.local_address(limit_local);
    let current = Val {
      id: it_address,
      type_id: it_type,
      indirect: true,
    };
    let limit = Val {
      id: limit_address,
      type_id: it_type,
      indirect: true,
    };
    match self.binary_values(
      source,
      node,
      crate::ir::BinaryOp::LessOrEqual,
      current,
      limit,
      TypeId::BOOL,
    ) {
      Some(condition) => {
        let condition = self.scalar(condition);
        self.terminate(Terminator::Branch {
          condition,
          then_block: body,
          else_block: exit,
        });
      }
      None => self.terminate(Terminator::Jump(exit)),
    }

    self.current = body;
    self.loops.push(Loop {
      break_block: exit,
      continue_block: step,
      depth: self.defers.len(),
    });
    self.statement(payload.block);
    self.loops.pop();
    self.terminate(Terminator::Jump(step));

    self.current = step;
    self.increment(it_local, it_type, 1);
    self.increment(index_local, TypeId::S64, 1);
    self.terminate(Terminator::Jump(head));

    self.current = exit;
  }

  fn array_loop(
    &mut self,
    node: NodeId,
    payload: &ast::ForNode,
    reverse: bool,
    by_pointer: bool,
    index_local: LocalId,
    it_decl: Option<DeclId>,
  ) {
    let source = self.body_source;
    let scope = self
      .checker
      .scope_for(source, payload.iteration_expression, self.body_scope);
    let Some(subject) = self.place(scope, source, payload.iteration_expression) else {
      return;
    };
    let Some((element, kind)) = self.checker.types().array_of(subject.type_id) else {
      self.unsupported(
        source,
        node,
        "a 'for' over this type ('for_expansion')",
        "M7",
      );
      return;
    };

    let count = match kind {
      ArrayKind::Fixed(length) => self.constant(Constant::Int(i128::from(length)), TypeId::S64),
      _ => {
        let slot = self.offset(subject.id, 0, TypeId::S64);
        Val {
          id: slot,
          type_id: TypeId::S64,
          indirect: true,
        }
      }
    };
    let pointer = self.pointer_to(element);
    let data = match kind {
      ArrayKind::Fixed(_) => Val {
        id: subject.id,
        type_id: pointer,
        indirect: false,
      },
      _ => {
        let slot = self.offset(subject.id, 8, pointer);
        Val {
          id: slot,
          type_id: pointer,
          indirect: true,
        }
      }
    };
    let data_local = self.new_local(String::from("data"), pointer);
    let data_address = self.local_address(data_local);
    self.store(data_address, data);
    let count_local = self.new_local(String::from("count"), TypeId::S64);
    let count_address = self.local_address(count_local);
    self.store(count_address, count);

    let index_address = self.local_address(index_local);
    let start = match reverse {
      false => self.constant(Constant::Int(0), TypeId::S64),
      true => {
        let count = Val {
          id: count_address,
          type_id: TypeId::S64,
          indirect: true,
        };
        let one = self.constant(Constant::Int(1), TypeId::S64);
        match self.binary_values(
          source,
          node,
          crate::ir::BinaryOp::Subtract,
          count,
          one,
          TypeId::S64,
        ) {
          Some(value) => value,
          None => return,
        }
      }
    };
    self.store(index_address, start);

    let it_type = if by_pointer { pointer } else { element };
    let it_local = self.new_local(String::from("it"), it_type);
    if let Some(id) = it_decl {
      self.local_of_decl.insert(id, it_local);
    }

    let head = self.new_block();
    let body = self.new_block();
    let step = self.new_block();
    let exit = self.new_block();
    self.terminate(Terminator::Jump(head));

    self.current = head;
    let index_address = self.local_address(index_local);
    let index = Val {
      id: index_address,
      type_id: TypeId::S64,
      indirect: true,
    };
    let bound = match reverse {
      false => {
        let count_address = self.local_address(count_local);
        Val {
          id: count_address,
          type_id: TypeId::S64,
          indirect: true,
        }
      }
      true => self.constant(Constant::Int(0), TypeId::S64),
    };
    let operator = match reverse {
      false => crate::ir::BinaryOp::Less,
      true => crate::ir::BinaryOp::GreaterOrEqual,
    };
    match self.binary_values(source, node, operator, index, bound, TypeId::BOOL) {
      Some(condition) => {
        let condition = self.scalar(condition);
        self.terminate(Terminator::Branch {
          condition,
          then_block: body,
          else_block: exit,
        });
      }
      None => self.terminate(Terminator::Jump(exit)),
    }

    self.current = body;
    let data_address = self.local_address(data_local);
    let base = self.value(pointer);
    self.emit(Inst::Load {
      dest: base,
      address: data_address,
    });
    let index_address = self.local_address(index_local);
    let index = self.value(TypeId::S64);
    self.emit(Inst::Load {
      dest: index,
      address: index_address,
    });
    let (stride, _) = self.size_align(element);
    let slot = self.value(pointer);
    self.emit(Inst::Index {
      dest: slot,
      base,
      index,
      stride,
    });
    let it_address = self.local_address(it_local);
    let source_value = Val {
      id: slot,
      type_id: if by_pointer { pointer } else { element },
      indirect: !by_pointer,
    };
    self.store(it_address, source_value);

    self.loops.push(Loop {
      break_block: exit,
      continue_block: step,
      depth: self.defers.len(),
    });
    self.statement(payload.block);
    self.loops.pop();
    self.terminate(Terminator::Jump(step));

    self.current = step;
    self.increment(index_local, TypeId::S64, if reverse { -1 } else { 1 });
    self.terminate(Terminator::Jump(head));

    self.current = exit;
  }

  fn increment(&mut self, local: LocalId, type_id: TypeId, by: i64) {
    let address = self.local_address(local);
    let current = self.value(type_id);
    self.emit(Inst::Load {
      dest: current,
      address,
    });
    let step = self.value(type_id);
    self.emit(Inst::Const {
      dest: step,
      value: Constant::Int(i128::from(by)),
    });
    let next = self.value(type_id);
    self.emit(Inst::Binary {
      dest: next,
      operator: crate::ir::BinaryOp::Add,
      left: current,
      right: step,
    });
    self.emit(Inst::Store {
      address,
      value: next,
    });
  }

  /// Storage that no value was written for starts out zeroed (**L§4.6**).
  pub(super) fn default_initialize(&mut self, address: ValueId, type_id: TypeId) {
    self.clear(address, type_id);
    self.apply_member_defaults(address, type_id);
  }

  fn apply_member_defaults(&mut self, address: ValueId, type_id: TypeId) {
    let underlying = self.checker.types().underlying(type_id);
    let Some(definition) = self.checker.types().struct_of(underlying) else {
      return;
    };
    let defaults = self.checker.member_defaults(definition);
    for (offset, member_type, source, node) in defaults {
      let member = self.offset(address, offset, member_type);
      let scope = self.checker.scope_for(source, node, self.body_scope);
      let (previous_source, previous_scope) = (self.body_source, self.body_scope);
      self.body_source = source;
      self.body_scope = scope;
      if let Some(value) = self.expression(scope, source, node, Some(member_type)) {
        self.store(member, value);
      }
      self.body_source = previous_source;
      self.body_scope = previous_scope;
    }
  }

  // ------------------------------------------------------- global prologue ---

  pub(super) fn emit_global_initializers(&mut self) {
    if self.deferred_initializers.is_empty() {
      return;
    }
    self.blocks = vec![Block::new()];
    self.locals.clear();
    self.value_types.clear();
    self.current = BlockId(0);
    self.local_of_decl.clear();
    self.loops.clear();
    self.defers.clear();
    self.returns.clear();
    self.return_pointers.clear();

    let context = self.pointer_to(self.context_type);
    let context_value = self.value(context);
    self.context_value = Some(context_value);

    // Assigning one global may reach another, so keep taking whatever the
    // last round added: they all belong in this one procedure.
    let mut pending = std::mem::take(&mut self.deferred_initializers);
    while !pending.is_empty() {
      for (global, decl) in pending {
        self.emit_one_global_initializer(global, decl);
      }
      pending = std::mem::take(&mut self.deferred_initializers);
    }
    self.terminate(Terminator::Return(Vec::new()));

    let signature = oj_types::ProcedureType::new(Vec::new(), Vec::new());
    let type_id = self.checker.types_table_mut().procedure(signature);
    let abi = Abi {
      parameters: vec![AbiParameter {
        type_id: self.context_type,
        kind: ParameterKind::Context,
      }],
      direct_return: None,
    };
    self.procedures.push(Procedure {
      symbol: String::from(GLOBAL_INIT_SYMBOL),
      name: String::from(GLOBAL_INIT_SYMBOL),
      type_id,
      parameters: Vec::new(),
      returns: Vec::new(),
      flags: ProcedureFlags::COMPILER_GENERATED,
      library: None,
      abi,
      locals: std::mem::take(&mut self.locals),
      blocks: std::mem::take(&mut self.blocks),
      value_types: std::mem::take(&mut self.value_types),
      entry: BlockId(0),
    });
  }

  fn emit_one_global_initializer(&mut self, global: GlobalId, decl: DeclId) {
    let info = self.checker.program().tree().decl(decl).clone();
    let (Some(source), Some(node)) = (info.source, info.node) else {
      return;
    };
    let Some(NodeData::Declaration(declaration)) =
      self.checker.tree_of(source).map(|ast| ast.data(node))
    else {
      return;
    };
    let Some(expression) = declaration.expression else {
      return;
    };
    let type_id = self.globals[global.0 as usize].type_id;
    self.body_source = source;
    self.body_scope = info.scope;
    let scope = self.checker.scope_for(source, expression, info.scope);
    let pointer = self.pointer_to(type_id);
    let address = self.value(pointer);
    self.emit(Inst::GlobalAddress {
      dest: address,
      global,
    });
    if let Some(value) = self.expression(scope, source, expression, Some(type_id)) {
      self.store(address, value);
    }
  }
}

/// The binary operator a compound assignment applies (**L§5.2**).
fn compound_operator(operator: OperatorType) -> Option<crate::ir::BinaryOp> {
  use crate::ir::BinaryOp;
  Some(match operator {
    OperatorType::PLUS_ASSIGN => BinaryOp::Add,
    OperatorType::MINUS_ASSIGN => BinaryOp::Subtract,
    OperatorType::TIMES_ASSIGN => BinaryOp::Multiply,
    OperatorType::DIV_ASSIGN => BinaryOp::Divide,
    OperatorType::MOD_ASSIGN => BinaryOp::Modulus,
    OperatorType::SHIFT_LEFT_ASSIGN => BinaryOp::ShiftLeft,
    OperatorType::SHIFT_RIGHT_ASSIGN => BinaryOp::ShiftRight,
    OperatorType::ROTATE_LEFT_ASSIGN => BinaryOp::RotateLeft,
    OperatorType::ROTATE_RIGHT_ASSIGN => BinaryOp::RotateRight,
    OperatorType::BITWISE_AND_ASSIGN | OperatorType::LOGICAL_AND_ASSIGN => BinaryOp::BitwiseAnd,
    OperatorType::BITWISE_OR_ASSIGN | OperatorType::LOGICAL_OR_ASSIGN => BinaryOp::BitwiseOr,
    OperatorType::BITWISE_XOR_ASSIGN => BinaryOp::BitwiseXor,
    _ => return None,
  })
}

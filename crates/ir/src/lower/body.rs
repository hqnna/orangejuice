//! Procedures and statements: the shape of a body, and the control flow it
//! turns into.

use super::*;

use crate::ir::{Abi, AbiParameter, ParameterKind};
use crate::lower::expr::{ADDRESS_SUBSCRIPT, SUBSCRIPT, SUBSCRIPT_ASSIGN};

/// Deep enough for the nesting the module tree has, shallow enough that a
/// pathological type fails rather than overflowing the stack.
const MAX_DEFAULT_DEPTH: u32 = 32;

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
    self.asm_registers.clear();
    self.asm_register_aliases.clear();
    self.loops.clear();
    self.expansions.clear();
    self.call_sites.clear();
    self.defers.clear();
    self.returns.clear();
    self.return_decls.clear();
    self.return_pointers.clear();
    self.context_value = None;
  }

  pub(super) fn lower_procedure(&mut self, id: ProcId, key: ProcKey) {
    self.start_procedure();
    // An instantiated body is walked with its own constants bound, so that the
    // questions the lowering asks get the specialization's answers, not the
    // header's (**L§7.8**).
    let entered = match key {
      ProcKey::Instance(instance) => Some(self.checker.enter_instance(Some(instance))),
      _ => None,
    };
    let body = match key {
      ProcKey::Decl(decl) => self.checker.procedure_body(decl),
      ProcKey::Node(source, header) => {
        let scope = self.checker.scope_for(source, header, self.body_scope);
        self.checker.procedure_body_at(source, header, scope)
      }
      ProcKey::Instance(instance) => self.checker.instance_body(instance),
      // A generated initializer has no body to read: it is the default
      // initialization every declaration of that type does (**L§4.6**).
      ProcKey::Initializer(type_id) => {
        self.lower_initializer(id, type_id);
        return;
      }
    };
    let leave = |lowering: &mut Self| {
      if let Some(previous) = entered {
        lowering.checker.enter_instance(previous);
      }
    };
    let Some(body) = body else {
      leave(self);
      return;
    };
    let Some(block) = body.block else {
      leave(self);
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
           (polymorphs and macros are milestone M7)."
        ),
      );
      leave(self);
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
    self.return_decls = body.returns.clone();
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
            {
              let key = self.local_key(*id);
              self.local_of_decl.insert(key, local);
            }
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
      {
        let key = self.local_key(*id);
        self.local_of_decl.insert(key, local);
      }
    }

    self.defers.push(Vec::new());
    self.statement(block);
    let scope = self.defers.pop().unwrap_or_default();
    let fell_through = (!self.terminated()).then_some(self.current);
    if let Some(end) = fell_through {
      self.run_defers(&scope);
      self.implicit_return(&body);
      // A body whose end control can reach, in a procedure that was declared
      // to return something (**C§12**). The end of a body is a block like any
      // other, and an `if` whose branches all return leaves it behind
      // unreachable, so it is the graph that says whether it can be got to.
      if !signature.returns.is_empty() && self.reaches(end) {
        let span = match key {
          ProcKey::Decl(decl) => self.checker.program().tree().decl(decl).span,
          _ => self.span_of(body.source, body.header),
        };
        self.warn(body.source, span, "Not all control paths return a value.");
      }
    }

    let procedure = &mut self.procedures[id.0 as usize];
    procedure.abi = abi;
    procedure.locals = std::mem::take(&mut self.locals);
    procedure.blocks = std::mem::take(&mut self.blocks);
    procedure.value_types = std::mem::take(&mut self.value_types);
    procedure.entry = BlockId(0);
    leave(self);
  }

  /// Whether control can get from the entry block to `target`. A block the
  /// lowering left behind — the join of an `if` whose branches all return —
  /// is in the list but nothing jumps to it.
  fn reaches(&self, target: BlockId) -> bool {
    let mut seen = vec![false; self.blocks.len()];
    let mut queue = vec![BlockId(0)];
    while let Some(block) = queue.pop() {
      let index = block.0 as usize;
      if index >= seen.len() || seen[index] {
        continue;
      }
      seen[index] = true;
      if block == target {
        return true;
      }
      match &self.blocks[index].terminator {
        Terminator::Jump(next) => queue.push(*next),
        Terminator::Branch {
          then_block,
          else_block,
          ..
        } => {
          queue.push(*then_block);
          queue.push(*else_block);
        }
        Terminator::Return(_) | Terminator::Unreachable => {}
      }
    }
    false
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
      let value = match named.and_then(|id| self.local_of_decl.get(&self.local_key(id)).copied()) {
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
      NodeData::Return { arguments, flags } => {
        let arguments = arguments.clone();
        let backticked = flags.contains(ast::ReturnFlags::IS_BACKTICKED);
        self.return_statement(node, &arguments, backticked);
      }
      NodeData::If(payload) => {
        let payload = payload.clone();
        // A `#if x == { … }` the front end decided contributes only the cases
        // it kept, the way a `#if` without cases does (**L§6.10**); one it
        // could not decide is lowered as an ordinary switch.
        if payload.if_flags.contains(IfFlags::IS_SWITCH_STATEMENT) {
          let source = self.body_source;
          let scope = self
            .checker
            .scope_for(source, payload.condition, self.body_scope);
          match self.checker.static_if_branches(scope, source, &payload) {
            Some(branches) => {
              for branch in branches {
                self.statement(branch);
              }
            }
            None => self.switch_statement(node, &payload),
          }
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
      // An `#import` or a `#load` in a body brings names into the block; it is
      // the scope tree's business, and the executable holds nothing for it
      // (**L§11.2**).
      NodeData::Using(_)
      | NodeData::Note { .. }
      | NodeData::DirectiveScope { .. }
      | NodeData::DirectiveImport(_)
      | NodeData::DirectiveLoad { .. } => {}
      NodeData::BinaryOperator {
        operator,
        left,
        right,
        ..
      } if operator.is_assignment() => {
        let (operator, left, right) = (*operator, *left, *right);
        self.assignment(node, operator, left, right);
      }
      // `push_context c { … }` runs the block with `c` as the context
      // (**L§6.9**). Every call carries the context pointer explicitly, so
      // making it current is a matter of which pointer the block hands on.
      NodeData::PushContext { to_push, block, .. } => {
        let (to_push, block) = (*to_push, *block);
        let saved = self.context_value;
        if let Some(to_push) = to_push {
          let scope = self.checker.scope_for(source, to_push, self.body_scope);
          let context = self.context_type;
          if let Some(value) = self.expression(scope, source, to_push, Some(context)) {
            self.context_value = Some(self.address_of(value));
          }
        }
        if let Some(block) = block {
          self.statement(block);
        }
        self.context_value = saved;
      }
      // A `#run` statement already happened, when the front end typechecked
      // the body around it (**L§6.11**); the executable holds nothing for it.
      NodeData::DirectiveRun(_) => {}
      // An `#insert` splices a piece of program into the block around it: the
      // statements a string parsed into, or the ones a `Code` value names, in
      // the scope the front end admitted them to (**L§13.2**).
      NodeData::DirectiveInsert(_) => {
        let scope = self.body_scope;
        match self.checker.insert_expansion(scope, source, node) {
          Some(expansion) => {
            let previous_source = std::mem::replace(&mut self.body_source, expansion.source);
            let previous_scope = std::mem::replace(&mut self.body_scope, expansion.scope);
            self.statement(expansion.root);
            self.body_source = previous_source;
            self.body_scope = previous_scope;
          }
          None => self.unsupported(source, node, "'#insert'", "M8"),
        }
      }
      // An `#asm` block places its own registers and hands the back end the
      // text to assemble (**L§15**).
      NodeData::Asm(_) => self.asm(source, node),
      _ => {
        let scope = self.checker.scope_for(source, node, self.body_scope);
        self.expression(scope, source, node, None);
      }
    }
  }

  pub(super) fn run_defers(&mut self, frame: &[(ScopeId, SourceId, NodeId)]) {
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
    // `x: T #elsewhere;` declares no storage of its own wherever it is
    // written: it names a symbol somebody else defines — the compiler, for
    // `__runtime_info` — so there is nothing here to lay out or clear
    // (**L§4.8**).
    if self
      .checker
      .program()
      .tree()
      .decl(decl)
      .flags
      .contains(DeclarationFlags::ELSEWHERE)
    {
      return;
    }
    let type_id = self.checker.decl_type(decl).value;
    if self.mentions_unknown(type_id) {
      self.unsupported(source, node, "a declaration of unknown type", "M7");
      return;
    }
    let name = {
      let symbol = self.checker.program().tree().decl(decl).name;
      self.text(symbol)
    };
    // A `` `it := … `` inside a `for_expansion` writes the loop's own `it`,
    // which the expansion set aside before the body was spliced in
    // (**L§7.14**).
    let key = self.local_key(decl);
    let local = match self.local_of_decl.get(&key).copied() {
      Some(existing) => existing,
      None => {
        let local = self.new_local(name, type_id);
        self.local_of_decl.insert(key, local);
        local
      }
    };
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

    // `a, b = f();` assigns to names that already exist; only `:=` and a
    // typed form declare them (**L§4.5**).
    let assigns = compound.operator_type.is_some();
    let mut places = Vec::new();
    for name in &names {
      if assigns {
        let scope = self.checker.scope_for(source, *name, self.body_scope);
        places.push(
          self
            .place(scope, source, *name)
            .map(|place| (place.id, place.type_id)),
        );
        continue;
      }
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
      {
        let key = self.local_key(decl);
        self.local_of_decl.insert(key, local);
      }
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
    // `a, b := 1, 2;` gives each name its own value; one value on the right is
    // every name's (**L§4.5**).
    let values = match self.checker.tree_of(source).map(|ast| ast.data(expression)) {
      Some(NodeData::CommaSeparatedArguments { arguments }) => arguments
        .iter()
        .map(|argument| argument.node)
        .collect::<Vec<NodeId>>(),
      _ => vec![expression],
    };
    let value_at = |index: usize| -> NodeId {
      match values.len() {
        1 => values[0],
        _ => values[index.min(values.len() - 1)],
      }
    };

    // `a, b += 1;` applies the operator to each name in turn (**L§5.2**).
    if let Some(operator) = compound.operator_type
      && operator != OperatorType::ASSIGN
    {
      let Some(binary) = compound_operator(operator) else {
        self.unsupported(source, node, "this assignment operator", "M10");
        return;
      };
      for (index, target) in targets.iter().enumerate() {
        let Some(place) = *target else {
          continue;
        };
        self.read_modify_write(node, binary, place, value_at(index));
      }
      return;
    }

    let is_call = matches!(
      self.checker.tree_of(source).map(|ast| ast.data(expression)),
      Some(NodeData::ProcedureCall(_))
    );
    if is_call {
      self.call_into(scope, source, expression, &targets);
      return;
    }
    // One value on the right is every name's, and is worked out once.
    if values.len() == 1 {
      let Some((_, type_id)) = targets.iter().flatten().next().copied() else {
        return;
      };
      let Some(value) = self.expression(scope, source, expression, Some(type_id)) else {
        return;
      };
      for (address, _) in targets.into_iter().flatten() {
        self.store(address, value);
      }
      return;
    }
    for (index, target) in targets.iter().enumerate() {
      let Some((address, type_id)) = *target else {
        continue;
      };
      let value_node = value_at(index);
      let scope = self.checker.scope_for(source, value_node, self.body_scope);
      if let Some(value) = self.expression(scope, source, value_node, Some(type_id)) {
        self.store(address, value);
      }
    }
  }

  /// `return` inside a macro returns from the macro; `` `return `` returns
  /// from the procedure it expanded into (**L§7.13**).
  fn return_statement(&mut self, node: NodeId, arguments: &[ast::Argument], backticked: bool) {
    if !backticked && !self.expansions.is_empty() {
      self.macro_return(node, arguments);
      return;
    }
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
    // A named return the `return` left out takes the default the header gave
    // it (**L§7.2**).
    while values.len() < returns.len() {
      let index = values.len();
      let Some(value) = self.default_return(index) else {
        self.unsupported(source, node, "a return that names its values", "M7");
        return;
      };
      values.push(value);
    }
    // A `return` inside nested blocks runs every defer on the way out
    // (**L§6.6**).
    self.run_defers_to(0);
    self.emit_return(values);
  }

  /// A `return` written in a macro's body: the values go into the storage the
  /// expansion set aside and control leaves the spliced-in block, not the
  /// procedure (**L§7.13**).
  fn macro_return(&mut self, node: NodeId, arguments: &[ast::Argument]) {
    let source = self.body_source;
    let Some(frame) = self.expansions.last() else {
      return;
    };
    let (exit, defers) = (frame.exit, frame.defers);
    let results = frame.results.clone();
    for (index, argument) in arguments.iter().enumerate() {
      let Some((address, type_id)) = results.get(index).copied() else {
        break;
      };
      let scope = self
        .checker
        .scope_for(source, argument.expression, self.body_scope);
      let Some(value) = self.expression(scope, source, argument.expression, Some(type_id)) else {
        continue;
      };
      let Some(value) = self.convert(source, argument.expression, value, type_id) else {
        continue;
      };
      self.store(address, value);
    }
    let _ = node;
    self.run_defers_to(defers);
    self.terminate(Terminator::Jump(exit));
  }

  /// The value a named return takes when the `return` left it out
  /// (**L§7.2**).
  fn default_return(&mut self, index: usize) -> Option<Val> {
    let decl = self.return_decls.get(index).copied().flatten()?;
    let type_id = self.returns.get(index).copied()?;
    let info = self.checker.program().tree().decl(decl);
    let (source, node) = (info.source?, info.node?);
    let NodeData::Declaration(declaration) = self.checker.tree_of(source)?.data(node) else {
      return None;
    };
    let expression = declaration.expression?;
    let scope = self.checker.scope_for(source, expression, info.scope);
    self.expression(scope, source, expression, Some(type_id))
  }

  /// Whether a condition is decided before the program runs. `#compile_time`
  /// is the one each back end answers for itself (**L§6.10**).
  fn folded_condition(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<bool> {
    if let Some(ast) = self.checker.tree_of(source)
      && matches!(ast.data(node), NodeData::DirectiveCompileTime)
    {
      return Some(self.mode == Mode::CompileTime);
    }
    match self.checker.expression(scope, source, node).constant?.value {
      oj_sema::Value::Bool(value) => Some(value),
      oj_sema::Value::Int(value) => Some(value != 0),
      _ => None,
    }
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
    // A condition the front end folded, and `#compile_time`, which each back
    // end folds for itself, leave only one branch to lower: the other is dead
    // code, and lowering it would make the executable name a procedure that
    // only exists at compile time (**L§11.6**).
    if let Some(taken) = self.folded_condition(scope, source, payload.condition) {
      let branch = match taken {
        true => payload.then_block,
        false => payload.else_block,
      };
      if let Some(branch) = branch {
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
    // The subject is compared once per case, so it is worked out once and kept
    // where the comparisons can reach it.
    let subject = Val {
      id: self.address_of(subject),
      type_id: subject_type,
      indirect: true,
    };
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
          // A case is `subject == value`, and it is compared the way that
          // expression would be — two strings by their bytes (**L§3.4**).
          let Some(compared) = self.binary_values(
            source,
            condition,
            crate::ir::BinaryOp::Equal,
            subject,
            value,
            TypeId::BOOL,
          ) else {
            continue;
          };
          let compared = self.scalar(compared);
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
    // `#complete` says the cases cover every value, so falling past the last
    // comparison is not something that happens (**L§6.4**) — which is what
    // lets a switch whose arms all return be the end of a body.
    match payload.if_flags.contains(IfFlags::MARKED_AS_COMPLETE) {
      true => self.terminate(Terminator::Unreachable),
      false => self.terminate(Terminator::Jump(join)),
    }

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
    // `a[i] = v` writes through `operator []=` when the subscript has one and
    // no `operator *[]` gave an address to write to (**L§7.7**).
    if let Some((base, index)) = self.subscript_operands(source, left)
      && self.subscript_assignment(node, operator, left, base, index, right)
    {
      return;
    }
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

    // `a op= b` takes the `operator op=` declared for it, and otherwise means
    // `a = a op b` through `operator op` (**L§7.7**). Either way the place is
    // evaluated once, so it is handed over rather than written again.
    if let Some(plan) =
      self
        .checker
        .compound_operator_plan(scope, source, node, operator.text(), left, right)
    {
      let pointer = self.pointer_to(place.type_id);
      let address = Val {
        id: place.id,
        type_id: pointer,
        indirect: false,
      };
      self.emit_planned_call(scope, source, node, plan, &[Some(address)]);
      return;
    }
    if let Some(base) = base_operator(operator)
      && let Some(plan) =
        self
          .checker
          .operator_plan(scope, source, node, base.text(), &[left, right])
    {
      let current = Val {
        id: place.id,
        type_id: place.type_id,
        indirect: true,
      };
      let mut results = match self.emit_planned_call(scope, source, node, plan, &[Some(current)]) {
        Some(results) => results,
        None => return,
      };
      if !results.is_empty() {
        let value = results.remove(0);
        self.store(place.id, value);
      }
      return;
    }

    let Some(binary) = compound_operator(operator) else {
      self.unsupported(source, node, "this assignment operator", "M7");
      return;
    };
    self.read_modify_write(node, binary, (place.id, place.type_id), right);
  }

  /// The base and index of `a[i]`, when that is what was written.
  fn subscript_operands(&self, source: SourceId, node: NodeId) -> Option<(NodeId, NodeId)> {
    match self.checker.tree_of(source)?.data(node) {
      NodeData::BinaryOperator {
        operator: OperatorType::ARRAY_SUBSCRIPT,
        left,
        right,
        ..
      } => Some((*left, *right)),
      _ => None,
    }
  }

  /// `a[i] = v` and `a[i] op= v` through `operator []=` (**L§7.7**). The
  /// compound forms are rewritten as `a[i] = a[i] op v`, reading through
  /// `operator []`, with the index evaluated once. Returns whether the
  /// assignment was handled here.
  fn subscript_assignment(
    &mut self,
    node: NodeId,
    operator: OperatorType,
    left: NodeId,
    base: NodeId,
    index: NodeId,
    right: NodeId,
  ) -> bool {
    let source = self.body_source;
    let scope = self.checker.scope_for(source, left, self.body_scope);
    // An `operator *[]` names storage, so the ordinary place path writes
    // through it and there is nothing to rewrite.
    if self
      .checker
      .operator_plan(scope, source, left, ADDRESS_SUBSCRIPT, &[base, index])
      .is_some()
    {
      return false;
    }
    let Some(plan) =
      self
        .checker
        .operator_plan(scope, source, node, SUBSCRIPT_ASSIGN, &[base, index, right])
    else {
      return false;
    };
    let Some(index_target) = plan.arguments.get(1).map(|argument| argument.target) else {
      return false;
    };
    let Some(element) = plan.arguments.get(2).map(|argument| argument.target) else {
      return false;
    };
    let index_scope = self.checker.scope_for(source, index, self.body_scope);
    let Some(index_value) = self.expression(index_scope, source, index, Some(index_target)) else {
      return true;
    };
    let index_value = Val {
      id: self.scalar(index_value),
      type_id: index_target,
      indirect: false,
    };

    let value = match operator {
      OperatorType::ASSIGN => None,
      _ => {
        let Some(binary) = compound_operator(operator) else {
          self.unsupported(source, node, "this assignment operator", "M7");
          return true;
        };
        let Some(current) = self.operator_call_with(
          scope,
          source,
          left,
          SUBSCRIPT,
          &[base, index],
          &[None, Some(index_value)],
        ) else {
          self.unsupported(source, node, "'operator []'", "M7");
          return true;
        };
        let right_scope = self.checker.scope_for(source, right, self.body_scope);
        let Some(operand) = self.expression(right_scope, source, right, Some(element)) else {
          return true;
        };
        match self.binary_values(source, node, binary, current, operand, element) {
          Some(value) => Some(value),
          None => return true,
        }
      }
    };
    let given = [None, Some(index_value), value];
    self.operator_call_with(
      scope,
      source,
      node,
      SUBSCRIPT_ASSIGN,
      &[base, index, right],
      &given,
    );
    true
  }

  /// `x op= e`: the storage is read, combined and written back (**L§5.2**).
  fn read_modify_write(
    &mut self,
    node: NodeId,
    binary: crate::ir::BinaryOp,
    place: (ValueId, TypeId),
    right: NodeId,
  ) {
    let source = self.body_source;
    let (address, type_id) = place;
    // `a &&= b` and `a ||= b` only assign when the operator would change the
    // value, which a plain read-modify-write already does (**L§5.2**).
    // `x &= ~.A` reads the right operand as one of `x`'s own type. A shift's
    // right operand is a count and a pointer's is an index of elements, so
    // neither takes it (**L§3.2**, **L§5.2**).
    let keeps_left = matches!(
      binary,
      crate::ir::BinaryOp::ShiftLeft
        | crate::ir::BinaryOp::ShiftRight
        | crate::ir::BinaryOp::RotateLeft
        | crate::ir::BinaryOp::RotateRight
    ) || self.checker.types().is_pointer(type_id);
    let scope = self.checker.scope_for(source, right, self.body_scope);
    let wanted = (!keeps_left).then_some(type_id);
    let Some(value) = self.expression(scope, source, right, wanted) else {
      return;
    };
    let current = Val {
      id: address,
      type_id,
      indirect: true,
    };
    let Some(result) = self.binary_values(source, node, binary, current, value, type_id) else {
      return;
    };
    self.store(address, result);
  }

  /// `for` over a range, over an array, or through the `for_expansion` the
  /// container's type declares (**L§6.6**, **L§7.14**).
  fn for_statement(&mut self, node: NodeId, payload: &ast::ForNode) {
    let source = self.body_source;
    let scope = self
      .checker
      .scope_for(source, payload.iteration_expression, self.body_scope);
    if let Some(expansion) = self.checker.loop_expansion(scope, source, node) {
      self.expand_for(node, payload, expansion);
      return;
    }
    if payload.want_pointer_expression.is_some() || payload.want_reverse_expression.is_some() {
      self.unsupported(source, node, "a 'for' with computed modifiers", "M7");
      return;
    }
    let reverse = payload.for_flags.contains(ForFlags::REVERSE);
    let by_pointer = payload.for_flags.contains(ForFlags::POINTER);

    let (it_decl, index_decl) = self.checker.loop_iterators(source, node);
    let index_local = self.new_local(String::from("it_index"), TypeId::S64);
    if let Some(id) = index_decl {
      {
        let key = self.local_key(id);
        self.local_of_decl.insert(key, index_local);
      }
    }

    match payload.iteration_expression_right {
      Some(right) => self.range_loop(node, payload, right, reverse, index_local, it_decl),
      None => self.array_loop(node, payload, reverse, by_pointer, index_local, it_decl),
    }
  }

  /// A `for` over a container: the `for_expansion` its type declares is
  /// spliced in with the loop's body as its `Code` argument, and the `` `it ``
  /// it exports *is* the loop's own (**L§7.14**).
  fn expand_for(&mut self, node: NodeId, payload: &ast::ForNode, expansion: LoopExpansion) {
    let source = self.body_source;
    let instance = expansion.instance;
    let previous_instance = self.checker.enter_instance(Some(instance));
    let expanded = self.expand_for_body(node, payload, expansion);
    self.checker.enter_instance(previous_instance);
    if expanded.is_none() {
      self.unsupported(source, node, "this 'for_expansion'", "M7");
    }
  }

  fn expand_for_body(
    &mut self,
    node: NodeId,
    payload: &ast::ForNode,
    expansion: LoopExpansion,
  ) -> Option<()> {
    let source = self.body_source;
    let instance = expansion.instance;
    let body = self.checker.instance_body(instance)?;
    let block = body.block?;
    let signature = self
      .checker
      .types()
      .procedure_of(self.checker.instance_info(instance).type_id)
      .cloned()?;

    // The loop's `it` and `it_index` are the macro's exports, so both names
    // stand for one local.
    let (loop_it, loop_index) = self.checker.loop_iterators(source, node);
    for (exported, written, is_index) in [
      (expansion.it, loop_it, false),
      (expansion.it_index, loop_index, true),
    ] {
      let Some(exported) = exported else { continue };
      let type_id = self.checker.expansion_iterator_type(expansion, is_index);
      if self.mentions_unknown(type_id) {
        return None;
      }
      let name = String::from(if is_index { "it_index" } else { "it" });
      let local = self.new_local(name, type_id);
      let key = self.local_key(exported);
      self.local_of_decl.insert(key, local);
      if let Some(written) = written {
        let previous = self.checker.enter_instance(None);
        let key = self.local_key(written);
        self.checker.enter_instance(previous);
        self.local_of_decl.insert(key, local);
      }
    }

    // The container is the only argument the expansion did not fold into a
    // constant.
    let mut container = Some(payload.iteration_expression);
    for (index, type_id) in signature.arguments.iter().copied().enumerate() {
      let Some(decl) = body.parameters.get(index).copied().flatten() else {
        continue;
      };
      if self.checker.instance_binds(decl) {
        continue;
      }
      let Some(written) = container.take() else {
        continue;
      };
      let scope = self.checker.scope_for(source, written, self.body_scope);
      // A `for_expansion` that asked for `*T` gets the container's address
      // (**L§7.14**).
      let value = match self.checker.types().pointee(type_id) {
        Some(pointee) if !self.iterates_a_pointer(scope, source, written) => {
          let place = self.place(scope, source, written)?;
          Val {
            id: place.id,
            type_id: self.pointer_to(pointee),
            indirect: false,
          }
        }
        _ => self.expression(scope, source, written, Some(type_id))?,
      };
      let name = {
        let symbol = self.checker.program().tree().decl(decl).name;
        self.text(symbol)
      };
      let local = self.new_local(name, type_id);
      let address = self.local_address(local);
      self.store(address, value);
      let key = self.local_key(decl);
      self.local_of_decl.insert(key, local);
    }

    let exit = self.new_block();
    let previous_source = std::mem::replace(&mut self.body_source, body.source);
    let previous_scope = std::mem::replace(&mut self.body_scope, body.scope);
    self.expansions.push(crate::lower::Expansion {
      exit,
      results: Vec::new(),
      defers: self.defers.len(),
    });
    self.defers.push(Vec::new());
    self.statement(block);
    let scope = self.defers.pop().unwrap_or_default();
    if !self.terminated() {
      self.run_defers(&scope);
      self.terminate(Terminator::Jump(exit));
    }
    self.expansions.pop();
    self.body_source = previous_source;
    self.body_scope = previous_scope;
    self.current = exit;
    Some(())
  }

  fn iterates_a_pointer(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> bool {
    let subject = self.checker.expression(scope, source, node).type_id;
    self.checker.types().is_pointer(subject)
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
      {
        let key = self.local_key(id);
        self.local_of_decl.insert(key, it_local);
      }
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
      {
        let key = self.local_key(id);
        self.local_of_decl.insert(key, it_local);
      }
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
    self.apply_member_defaults_at(address, type_id, 0);
  }

  /// A member with no value of its own still gets one when its own type has
  /// defaults: `#Context` is a `using` of `Context_Base`, whose allocator and
  /// logger are what a program starts with (**L§4.6**, **L§8.4**).
  fn apply_member_defaults_at(&mut self, address: ValueId, type_id: TypeId, depth: u32) {
    if depth > MAX_DEFAULT_DEPTH {
      return;
    }
    let underlying = self.checker.types().underlying(type_id);
    let Some(definition) = self.checker.types().struct_of(underlying) else {
      return;
    };
    // A baked struct's defaults may name its parameters, which are constants
    // of the instantiation (**L§8.5**).
    let entered = self
      .checker
      .struct_instance_of(definition)
      .map(|instance| self.checker.enter_instance(Some(instance)));
    let defaults = self.checker.member_defaults(definition);
    let nested: Vec<(u64, TypeId)> = self
      .checker
      .types()
      .struct_info(definition)
      .members
      .iter()
      .filter(|member| {
        member.imported_through.is_none()
          && !member
            .flags
            .intersects(oj_types::MemberFlags::CONSTANT | oj_types::MemberFlags::IMPORTED)
          && !defaults.iter().any(|(offset, ..)| *offset == member.offset)
      })
      .map(|member| (member.offset, member.type_id))
      .collect();
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
    for (offset, member_type) in nested {
      let member = self.offset(address, offset, member_type);
      self.apply_member_defaults_at(member, member_type, depth + 1);
    }
    // A `member.field = value;` default reaches past a member the loop above
    // has just filled in, so it is written last (**L§8.1**).
    for (offset, member_type, source, node) in self.checker.member_path_defaults(definition) {
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
    if let Some(previous) = entered {
      self.checker.enter_instance(previous);
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
    self.asm_registers.clear();
    self.asm_register_aliases.clear();
    self.loops.clear();
    self.expansions.clear();
    self.call_sites.clear();
    self.defers.clear();
    self.returns.clear();
    self.return_decls.clear();
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

        class: None,
      }],
      direct_return: None,
      return_class: None,
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

  /// `initializer_of(T)` is a procedure the compiler writes: it zeroes the
  /// storage it is handed and then applies the member defaults, which is what
  /// a declaration of that type does (**L§4.6**, **L§17**).
  fn lower_initializer(&mut self, id: ProcId, type_id: TypeId) {
    let pointer = self.pointer_to(type_id);
    let address = self.value(pointer);
    self.default_initialize(address, type_id);
    self.terminate(Terminator::Return(Vec::new()));
    let procedure = &mut self.procedures[id.0 as usize];
    procedure.locals = std::mem::take(&mut self.locals);
    procedure.blocks = std::mem::take(&mut self.blocks);
    procedure.value_types = std::mem::take(&mut self.value_types);
    procedure.entry = BlockId(0);
  }

  fn emit_one_global_initializer(&mut self, global: GlobalId, decl: DeclId) {
    let info = self.checker.program().tree().decl(decl);
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

/// The plain operator behind a compound assignment: `a += b` falls back to
/// `a = a + b` when no `operator +=` is declared (**L§7.7**).
fn base_operator(operator: OperatorType) -> Option<OperatorType> {
  Some(match operator {
    OperatorType::PLUS_ASSIGN => OperatorType::PLUS,
    OperatorType::MINUS_ASSIGN => OperatorType::MINUS,
    OperatorType::TIMES_ASSIGN => OperatorType::TIMES,
    OperatorType::DIV_ASSIGN => OperatorType::DIVIDE,
    OperatorType::MOD_ASSIGN => OperatorType::MODULUS,
    OperatorType::SHIFT_LEFT_ASSIGN => OperatorType::SHIFT_LEFT,
    OperatorType::SHIFT_RIGHT_ASSIGN => OperatorType::SHIFT_RIGHT,
    OperatorType::ROTATE_LEFT_ASSIGN => OperatorType::ROTATE_LEFT,
    OperatorType::ROTATE_RIGHT_ASSIGN => OperatorType::ROTATE_RIGHT,
    OperatorType::BITWISE_AND_ASSIGN => OperatorType::BITWISE_AND,
    OperatorType::BITWISE_OR_ASSIGN => OperatorType::BITWISE_OR,
    OperatorType::BITWISE_XOR_ASSIGN => OperatorType::BITWISE_XOR,
    _ => return None,
  })
}

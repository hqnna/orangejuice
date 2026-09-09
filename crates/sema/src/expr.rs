use oj_diag::{SourceId, Span};
use oj_lexer::Symbol;
use oj_scope::{DeclId, DeclKind, Resolution, ScopeId, ScopeKind};
use oj_syntax::ast::{
  ExpressionQueryKind, FileInfoKind, IdentFlags, LiteralFlags, LiteralValue, NodeData, NodeId,
  OperatorType, ProcedureFlags, TypeQueryKind,
};
use oj_types::{ArrayKind, FloatKind, TypeId, TypeKind};

use crate::checker::{Checker, Expr};
use crate::constants::{Const, RunBytes, Value};

impl Checker<'_> {
  /// The type of an expression, with its constant value when it has one
  /// (**L§5**). Anything whose answer needs a milestone the front end has not
  /// reached — `#run`, a macro expansion, a polymorph instantiation — comes
  /// back as [`Expr::UNKNOWN`] rather than as an error.
  pub(crate) fn expression_type(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Expr {
    if !self.enter() {
      return Expr::UNKNOWN;
    }
    let result = self.expression_type_inner(scope, source, node);
    self.leave();
    result
  }

  fn expression_type_inner(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Expr {
    let Some(ast) = self.ast(source) else {
      return Expr::UNKNOWN;
    };

    match ast.data(node) {
      NodeData::Literal(literal) => {
        self.literal_type(scope, source, node, &literal.value, literal.flags)
      }
      NodeData::Ident(ident) => self.ident_type(scope, source, node, ident.name, ident.flags),
      NodeData::UnaryOperator { operator, operand } => {
        self.unary_type(scope, source, node, *operator, *operand)
      }
      NodeData::BinaryOperator {
        operator,
        left,
        right,
        ..
      } => self.binary_type(scope, source, node, *operator, *left, *right),
      NodeData::ProcedureHeader(header) => {
        let type_id = self.procedure_type(source, node, scope);
        if header.procedure_flags.contains(ProcedureFlags::TYPE_ONLY) {
          Expr::type_expression(type_id)
        } else {
          Expr::value(type_id)
        }
      }
      NodeData::ProcedureBody { header, .. } => {
        Expr::value(self.procedure_type(source, *header, scope))
      }
      NodeData::Struct(_) => {
        Expr::type_expression(self.build_struct(None, None, source, node, scope))
      }
      NodeData::Enum(_) => Expr::type_expression(self.build_enum(None, None, source, node, scope)),
      NodeData::TypeInstantiation(_) => {
        Expr::type_expression(self.type_from_node(scope, source, node))
      }
      NodeData::Cast(cast) => self.cast_type(scope, source, node, cast),
      NodeData::ProcedureCall(_) => self.call_type(scope, source, node),
      NodeData::TypeQuery {
        query_kind,
        type_to_query,
      } => self.type_query(scope, source, *query_kind, *type_to_query),
      NodeData::ExpressionQuery {
        query_kind,
        expression_to_query,
      } => self.expression_query(scope, source, *query_kind, *expression_to_query),
      NodeData::If(payload) => self.ifx_type(scope, source, payload),
      NodeData::Context | NodeData::DirectiveContextType => {
        let context = self.context_type();
        if matches!(ast.data(node), NodeData::DirectiveContextType) {
          Expr::type_expression(context)
        } else {
          Expr::place(context)
        }
      }
      NodeData::DirectiveFileInfo { which } => match which {
        FileInfoKind::File | FileInfoKind::Filepath => {
          Expr::constant(Const::string(Box::from(&b""[..])))
        }
        FileInfoKind::Line => Expr::constant(Const::untyped_int(0)),
      },
      NodeData::DirectiveLocation(_) => {
        let location = self.source_code_location_type();
        Expr::value(location)
      }
      // `#caller_code` is the whole call the macro was expanded from, as
      // `Code` (**L§7.13**).
      NodeData::DirectiveCallerCode => match self.expansion_site().or(self.call_site) {
        Some(site) => Expr::constant(Const::new(
          TypeId::CODE,
          Value::Code {
            source: site.source,
            node: site.node,
            scope: site.caller_scope,
          },
        )),
        None => Expr::value(TypeId::CODE),
      },
      // `#this` is the procedure or the type that contains it, as a constant
      // (**L§5.11**).
      NodeData::DirectiveThis => {
        let scope = self
          .program()
          .directive_scope(source, node)
          .unwrap_or(scope);
        self.this_value(scope)
      }
      NodeData::DirectiveProcedureName { .. } => Expr::value(TypeId::STRING),
      NodeData::DirectiveExists(_) => Expr::constant(Const::bool(false)),
      // `#compile_time` is a `bool` but not a constant one (**L§5.14**).
      NodeData::DirectiveCompileTime => Expr::value(TypeId::BOOL),
      // An `#insert` where a value goes stands for the one expression its text
      // parses into (**L§13.2**).
      NodeData::DirectiveInsert(_) => match self.insert_expression(scope, source, node) {
        Some((source, scope, expression)) => self.expression_type(scope, source, expression),
        None => Expr::UNKNOWN,
      },
      // `#code,null` is the one `Code` that is a constant on its own: it names
      // no program, tests false, and inserts nothing (**L§13.1**).
      NodeData::DirectiveCode { flags, .. } if flags.contains(oj_syntax::ast::CodeFlags::NULL) => {
        Expr::constant(Const::new(TypeId::CODE, Value::Null))
      }
      // Every other `#code` is the piece of the program it was written around,
      // which is a constant: the node, and the scope its names resolve in
      // (**L§13.1**).
      NodeData::DirectiveCode {
        expression: Some(expression),
        ..
      } => {
        let expression = *expression;
        let scope = self
          .program()
          .directive_scope(source, node)
          .unwrap_or(scope);
        Expr::constant(Const::new(
          TypeId::CODE,
          Value::Code {
            source,
            node: expression,
            scope,
          },
        ))
      }
      NodeData::DirectiveCode { .. } => Expr::value(TypeId::CODE),
      NodeData::DirectiveRun(_) => self.run_type(scope, source, node),
      // A block in expression position is an `ifx` branch: its value is its
      // last expression statement (**L§5.13**).
      NodeData::Block(block) => match block.statements.last() {
        Some(last) => {
          let last = *last;
          self.expression_type(scope, source, last)
        }
        None => Expr::value(TypeId::VOID),
      },
      _ => Expr::UNKNOWN,
    }
  }

  fn literal_type(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    value: &LiteralValue,
    flags: LiteralFlags,
  ) -> Expr {
    match value {
      LiteralValue::Integer(number) => {
        let value = Value::Int(i128::from(*number));
        if flags.intersects(LiteralFlags::HEX | LiteralFlags::BINARY) {
          Expr::constant(Const::bit_pattern(TypeId::UNTYPED_INT, value))
        } else {
          Expr::constant(Const::new(TypeId::UNTYPED_INT, value))
        }
      }
      LiteralValue::Float(number) => {
        let kind =
          if flags.intersects(LiteralFlags::REQUIRES_FLOAT64 | LiteralFlags::DEFAULTS_TO_FLOAT64) {
            FloatKind::F64
          } else {
            FloatKind::F32
          };
        let type_id = match kind {
          FloatKind::F32 => TypeId::UNTYPED_FLOAT32,
          FloatKind::F64 => TypeId::UNTYPED_FLOAT64,
        };
        Expr::constant(Const::new(type_id, Value::Float(*number)))
      }
      LiteralValue::Text(text) => Expr::constant(Const::string(text.clone())),
      LiteralValue::Bool(value) => Expr::constant(Const::bool(*value)),
      LiteralValue::Null => Expr::constant(Const::new(TypeId::VOID_POINTER, Value::Null)),
      LiteralValue::Array(array) => {
        let count = array.members.len() as u64;
        let Some(element) = array.element_type else {
          for member in &array.members {
            self.expression_type(scope, source, *member);
          }
          // `.[…]` takes its element type from the context (**L§5.8**).
          return Expr::value(TypeId::UNTYPED_LITERAL);
        };
        let element = self.type_from_node(scope, source, element);
        self.check_array_literal(scope, source, element, &array.members);
        if element == TypeId::UNKNOWN {
          return Expr::UNKNOWN;
        }
        let type_id = self.types_mut().array(element, ArrayKind::Fixed(count));
        // An array literal whose members all fold is itself a constant, which
        // is what makes a lookup table data rather than code (**L§5.11**).
        let members = array.members.clone();
        match self.fold_array_literal(scope, source, element, &members) {
          Some(bytes) => Expr::constant(Const::new(type_id, Value::Bytes(bytes))),
          None => Expr::value(type_id),
        }
      }
      LiteralValue::Struct(literal) => {
        let Some(type_expression) = literal.type_expression else {
          for argument in &literal.arguments {
            self.expression_type(scope, source, argument.expression);
          }
          return Expr::value(TypeId::UNTYPED_LITERAL);
        };
        let type_id = self.type_from_node(scope, source, type_expression);
        let span = self
          .ast(source)
          .map_or(Span::at(0), |ast| ast.node(node).span);
        self.check_struct_literal(scope, source, node, span, type_id, &literal.arguments);
        // A struct literal whose members all fold is itself a constant
        // (**L§5.11**).
        let arguments = literal.arguments.clone();
        match self.fold_struct_literal(scope, source, type_id, &arguments) {
          Some(bytes) => Expr::constant(Const::new(type_id, Value::Bytes(RunBytes::plain(bytes)))),
          None => Expr::value(type_id),
        }
      }
    }
  }

  /// The constant value of an expression when a type is already known for it.
  /// An undesignated `.[…]` has no type of its own, so it can only become data
  /// once something says what it is (**L§5.8**).
  pub(crate) fn const_value_at(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    target: TypeId,
  ) -> Option<Const> {
    if let Some(value) = self.const_value(scope, source, node) {
      return Some(value);
    }
    let ast = self.ast(source)?;
    let NodeData::Literal(literal) = ast.data(node) else {
      return None;
    };
    let LiteralValue::Array(array) = &literal.value else {
      return None;
    };
    if array.element_type.is_some() {
      return None;
    }
    let members = array.members.clone();
    let (element, kind) = self.types().array_of(target)?;
    if kind != ArrayKind::Fixed(members.len() as u64) {
      return None;
    }
    let bytes = self.fold_array_literal(scope, source, element, &members)?;
    Some(Const::new(target, Value::Bytes(bytes)))
  }

  /// The storage of a struct literal all of whose members fold, starting from
  /// the type's own defaults (**L§5.11**, **L§8.2**). `None` as soon as one of
  /// them does not fold, or the literal designates a slot inside a member,
  /// which is a place rather than a member of its own.
  fn fold_struct_literal(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    type_id: TypeId,
    arguments: &[oj_syntax::ast::Argument],
  ) -> Option<Box<[u8]>> {
    let definition = self.types().struct_of(self.types().underlying(type_id))?;
    self.complete_struct(definition);
    let mut bytes = self.default_bytes(type_id)?;
    let settable: Vec<(Symbol, TypeId, u64)> = self
      .types()
      .struct_info(definition)
      .settable_members()
      .map(|member| (member.name, member.type_id, member.offset))
      .collect();
    let named: Vec<(Symbol, TypeId, u64)> = self
      .types()
      .struct_info(definition)
      .members
      .iter()
      .map(|member| (member.name, member.type_id, member.offset))
      .collect();

    let mut position = 0usize;
    for argument in arguments {
      // A designator names a slot inside a member; the bytes of that are the
      // back end's to lay down.
      if matches!(
        self.ast(source).map(|ast| ast.data(argument.expression)),
        Some(NodeData::BinaryOperator {
          operator: OperatorType::ASSIGN,
          ..
        })
      ) {
        return None;
      }
      let member = match argument.name.and_then(|node| self.ident_name(source, node)) {
        Some(name) => named.iter().find(|entry| entry.0 == name).copied()?,
        None => {
          let member = settable.get(position).copied()?;
          position += 1;
          member
        }
      };
      let (_, member_type, offset) = member;
      let size = self.layout_of(member_type)?.size as usize;
      let start = offset as usize;
      if size == 0 || start + size > bytes.len() {
        return None;
      }
      let value = self.const_value_at(scope, source, argument.expression, member_type)?;
      if !value.write_bytes(self.types(), member_type, &mut bytes[start..start + size]) {
        return None;
      }
    }
    Some(bytes.into_boxed_slice())
  }

  /// The storage of an array literal all of whose members fold. `None` as soon
  /// as one of them does not, or is of a kind that needs an address rather
  /// than bytes.
  fn fold_array_literal(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    element: TypeId,
    members: &[NodeId],
  ) -> Option<RunBytes> {
    let stride = self.layout_of(element)?.size as usize;
    if stride == 0 {
      return None;
    }
    let mut bytes = vec![0u8; stride * members.len()];
    let mut links = Vec::new();
    for (index, member) in members.iter().enumerate() {
      // A procedure name is a constant like any other declared with `::`, so a
      // literal made of them is data the back end fills the addresses into
      // (**L§5.11**).
      let written = self.expression_type(scope, source, *member);
      let named = self.named_procedure(&written);
      let value = written.constant.or(named)?;
      let start = index * stride;
      // A string is a `{count, data}` pair whose bytes go beside the array,
      // with the pointer linked to them (**L§3.4**, **L§12.1**).
      if let Value::String(text) = &value.value
        && self.types().underlying(element) == TypeId::STRING
      {
        let count = (text.len() as u64).to_le_bytes();
        bytes[start..start + 8].copy_from_slice(&count);
        links.push(crate::constants::RunLink {
          at: (start + 8) as u64,
          target: crate::constants::RunTarget::Data(text.clone()),
          offset: 0,
        });
        continue;
      }
      // A procedure is an address the back end supplies, so the slot is left
      // empty and linked to it (**L§5.11**).
      if let Value::Procedure(decl) = value.value {
        links.push(crate::constants::RunLink {
          at: start as u64,
          target: crate::constants::RunTarget::Procedure(decl),
          offset: 0,
        });
        continue;
      }
      if !value.write_bytes(self.types(), element, &mut bytes[start..start + stride]) {
        return None;
      }
    }
    Some(RunBytes {
      data: bytes.into_boxed_slice(),
      links: links.into_boxed_slice(),
    })
  }

  fn ident_type(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    name: Symbol,
    flags: IdentFlags,
  ) -> Expr {
    if flags.contains(IdentFlags::DEFINES_POLYMORPH_VARIABLE) {
      // Inside an instantiation the `$T` that declares the variable is the
      // type it was bound to, which is what turns the header into a concrete
      // signature (**L§7.8**).
      if let Some(value) = self
        .decl_at(source, node)
        .and_then(|decl| self.bound_constant(decl))
      {
        // A `$N` bound to a *value* stands for that value, not for a type.
        return match value.as_type() {
          Some(bound) => Expr::type_expression(bound),
          None => Expr::constant(value),
        };
      }
      return Expr::type_expression(self.polymorph_type(source, node, name));
    }
    // `` `x `` names the caller's scope, never the macro's own (**L§7.13**).
    let start = match flags.contains(IdentFlags::HAS_SCOPE_MODIFIER) {
      true => match self.caller_scope() {
        Some(caller) => caller,
        None => return Expr::UNKNOWN,
      },
      false => self.scope_at(source, node, scope),
    };
    // A backticked name the caller's scope cannot answer for may be one the
    // macro declared with a backtick of its own — `` `it: u8; `` in a
    // `for_expansion`, whose type is what the loop's own `it` takes, so asking
    // the loop for it while the macro is being typed goes in a circle
    // (**L§7.13**, **L§7.14**).
    if flags.contains(IdentFlags::HAS_SCOPE_MODIFIER) {
      let caller = self.lookup_from(start, name);
      if let Some(found) = caller.clone().filter(|found| !found.is_unknown()) {
        return found;
      }
      if let Some(declared) = self.backticked_declaration(name) {
        let found = self.declarations_type(&[declared]);
        if !found.is_unknown() {
          return found;
        }
      }
      if let Some(found) = caller {
        return found;
      }
    }
    if let Some(found) = self.lookup_from(start, name) {
      return found;
    }
    // A name no declaration holds may still be a member of something a
    // `using` brought into scope (**L§6.8**).
    if let Some(found) = self.used_name_type(start, name) {
      return found;
    }
    // A macro's body sees the caller's locals by name, for names its own
    // scopes do not hold (**L§7.13**).
    let caller = self.caller_scope().and_then(|caller| {
      self
        .lookup_from(caller, name)
        .or_else(|| self.used_name_type(caller, name))
    });
    if let Some(found) = caller.clone().filter(|found| !found.is_unknown()) {
      return found;
    }
    // A `` `x := … `` a macro wrote declares `x` in the block the macro was
    // expanded into, so what is written there sees it — including a `Code`
    // argument the macro splices back in, which is how `Hash_Table`'s
    // `Walk_Table` hands its `index` to the body it was given. The macro's own
    // body reads it by the plain name too, and has to: asking the loop what
    // its `it_index` is while the macro that exports it is being typed goes in
    // a circle (**L§7.13**, **L§7.14**).
    if let Some(declared) = self.backticked_declaration(name) {
      let found = self.declarations_type(&[declared]);
      if !found.is_unknown() {
        return found;
      }
    }
    caller.unwrap_or(Expr::UNKNOWN)
  }

  /// What `#this` names: the procedure, struct or enum whose body it was
  /// written in, as the compile-time constant that is (**L§5.11**). A macro is
  /// spliced into whoever expanded it, so one written inside a macro names the
  /// procedure the expansion landed in (**L§7.13**).
  fn this_value(&mut self, scope: ScopeId) -> Expr {
    let start = match self.program().is_in_macro(scope) {
      true => self.caller_scope().unwrap_or(scope),
      false => scope,
    };
    let mut current = Some(start);
    while let Some(id) = current {
      // A struct or enum body: the type it makes, which inside an
      // instantiation is that specialization (**L§8.5**).
      if let Some((source, node)) = self.aggregate_owner(id)
        && let Some(type_id) = self.aggregate_type_in(id, source, node)
      {
        return Expr::type_expression(type_id);
      }
      if let Some((source, header)) = self.program().procedure_owner(id) {
        let outer = self.program().tree().parent(id).unwrap_or(id);
        // One with a name of its own is that declaration, and so a constant a
        // `::` can hold; an anonymous procedure — a quick lambda calling
        // itself — is only the address the back end generates for it.
        let Some(decl) = self.procedure_declared_at(source, header, id) else {
          return Expr::value(self.procedure_type(source, header, outer));
        };
        let type_id = self.decl_type(decl).value;
        return Expr {
          overloads: vec![decl],
          overload_instance: None,
          constant: Some(Const::new(type_id, Value::Procedure(decl))),
          ..Expr::value(type_id)
        };
      }
      current = self.program().tree().parent(id);
    }
    Expr::UNKNOWN
  }

  /// The declaration a procedure header was written at, found from the scope
  /// around it: a header knows nothing about the name it was given.
  fn procedure_declared_at(
    &mut self,
    source: SourceId,
    header: NodeId,
    constants: ScopeId,
  ) -> Option<DeclId> {
    let mut scope = self.program().tree().parent(constants);
    while let Some(id) = scope {
      let found = self
        .program()
        .tree()
        .declarations(id)
        .into_iter()
        .find(|declared| {
          let decl = self.program().tree().decl(*declared);
          decl.source == Some(source)
            && decl.node.is_some_and(|node| {
              matches!(
                self.ast(source).map(|ast| ast.data(node)),
                Some(NodeData::Declaration(declaration))
                  if declaration.expression == Some(header)
              )
            })
        });
      if found.is_some() {
        return found;
      }
      scope = self.program().tree().parent(id);
    }
    None
  }

  /// Resolves `name` on the chain out of `scope`. `None` means it is not there
  /// at all, which is what lets a macro's body fall back to its caller.
  fn lookup_from(&mut self, scope: ScopeId, name: Symbol) -> Option<Expr> {
    let mut scope = scope;
    loop {
      let Resolution::Found(candidates) = self.program().tree().lookup(scope, name) else {
        return None;
      };
      // A local is not in scope in its own initializer, so `type, ok :=
      // get_type(t, type)` reads the outer `type` (**L§6.13**).
      let Some(shadowed) = self.shadowed_declaration(&candidates) else {
        return Some(self.declarations_type(&candidates));
      };
      let owner = self.program().tree().decl(shadowed).scope;
      scope = self.program().tree().parent(owner)?;
    }
  }

  /// Drops the candidates the scope tree admitted from a `#if` branch that the
  /// checker can now decide against (**L§6.10**). The tree folds what it can
  /// without types; a condition that needed one — a `size_of`, a `#run` — is
  /// only decidable here.
  pub(crate) fn live_candidates(&mut self, candidates: &[DeclId]) -> Vec<DeclId> {
    let candidates = &self.candidates_of_this_instantiation(candidates);
    if !candidates
      .iter()
      .any(|id| self.program().tree().decl(*id).branch.is_some())
    {
      return candidates.to_vec();
    }
    let mut live = Vec::with_capacity(candidates.len());
    for id in candidates {
      let decl = self.program().tree().decl(*id);
      let Some(branch) = decl.branch else {
        live.push(*id);
        continue;
      };
      match self.taken_branch(branch, decl.scope) {
        Some(taken) if taken != branch.block => {}
        _ => live.push(*id),
      }
    }
    // A `#if` nobody can decide leaves every branch standing, which is what
    // the scope tree already did.
    match live.is_empty() {
      true => candidates.to_vec(),
      false => live,
    }
  }

  /// Drops the candidates another instantiation declared. An `#insert` written
  /// inside a polymorphic body expands once per specialization, and every
  /// expansion declares its names into the one scope the body has, so a name a
  /// sibling instantiation made is not one this one can see (**L§13.2**).
  fn candidates_of_this_instantiation(&self, candidates: &[DeclId]) -> Vec<DeclId> {
    let inserted = |id: &DeclId| {
      let decl = self.program().tree().decl(*id);
      decl
        .source
        .map_or(0, |source| self.program().insert_variant(source))
    };
    if !candidates.iter().any(|id| inserted(id) != 0) {
      return candidates.to_vec();
    }
    let mine: Vec<DeclId> = candidates
      .iter()
      .copied()
      .filter(|id| {
        let variant = inserted(id);
        variant == 0
          || Some(variant)
            == self
              .instance_of_scope(self.program().tree().decl(*id).scope)
              .map(|instance| instance.0 + 1)
      })
      .collect();
    match mine.is_empty() {
      true => candidates.to_vec(),
      false => mine,
    }
  }

  /// Whether the checker can still believe a `#if` branch the scope tree could
  /// not decide: `false` once the condition folds the other way (**L§6.10**).
  pub(crate) fn branch_is_live(&mut self, branch: oj_scope::Branch, scope: ScopeId) -> bool {
    match self.taken_branch(branch, scope) {
      Some(taken) => taken == branch.block,
      None => true,
    }
  }

  /// Which of a static `#if`'s two blocks its condition selects, or `None`
  /// when the condition still does not fold.
  fn taken_branch(&mut self, branch: oj_scope::Branch, scope: ScopeId) -> Option<NodeId> {
    let NodeData::If(payload) = self.ast(branch.source)?.data(branch.node) else {
      return None;
    };
    let payload = payload.clone();
    let condition_scope = self.scope_at(branch.source, payload.condition, scope);
    let truth = self
      .expression_type(condition_scope, branch.source, payload.condition)
      .constant
      .and_then(|value| value.value.truth())?;
    match truth {
      true => payload.then_block,
      false => payload.else_block,
    }
  }

  /// The declaration to look past, when every candidate is a local whose own
  /// type is still being worked out.
  fn shadowed_declaration(&self, candidates: &[DeclId]) -> Option<DeclId> {
    let tree = self.program().tree();
    let all_pending = candidates.iter().all(|id| {
      self.is_resolving(*id) && tree.scope_kind(tree.decl(*id).scope) == ScopeKind::Imperative
    });
    (all_pending && !candidates.is_empty()).then(|| candidates[0])
  }

  /// The type a resolved name stands for. Several declarations mean an
  /// overload set, which is a type only a call site can narrow (**L§7.5**).
  pub(crate) fn declarations_type(&mut self, candidates: &[DeclId]) -> Expr {
    // The same declaration can be reached down two import paths, which is not
    // an overload set (**L§7.5**).
    let mut real: Vec<DeclId> = self
      .live_candidates(candidates)
      .into_iter()
      .filter(|id| self.program().tree().decl(*id).kind != DeclKind::Placeholder)
      .collect();
    real.sort_unstable();
    real.dedup();
    let [only] = real[..] else {
      if real.is_empty() {
        return Expr::UNKNOWN;
      }
      return Expr {
        overloads: real,
        overload_instance: None,
        ..Expr::value(TypeId::OVERLOAD_SET)
      };
    };

    // A `$x` parameter is a constant of the instantiation, whatever the
    // declaration it was written as says (**L§7.8**).
    if let Some(value) = self.bound_constant(only) {
      return Expr {
        overloads: vec![only],
        overload_instance: None,
        ..Expr::constant(value)
      };
    }
    let resolved = self.decl_type(only);
    if let Some(denoted) = resolved.denoted {
      return Expr::type_expression(denoted);
    }
    let decl = self.program().tree().decl(only);
    if decl
      .flags
      .contains(oj_syntax::ast::DeclarationFlags::IS_CONSTANT)
      || self.is_module_parameter(only)
    {
      if let Some(value) = self.decl_constant(only) {
        return Expr {
          overloads: vec![only],
          overload_instance: None,
          ..Expr::constant(value)
        };
      }
      // `assert :: Basic.assert` is a name for what it aliases rather than a
      // value of its own, which is what lets a call site resolve it the way it
      // would resolve the original — a polymorphic procedure and a macro have
      // no type until then (**L§7.5**, **L§7.8**).
      let aliased = self.aliased_declarations(only);
      if !aliased.is_empty() {
        let type_id = match aliased[..] {
          [target] => self.decl_type(target).value,
          _ => TypeId::OVERLOAD_SET,
        };
        return Expr {
          overloads: aliased,
          overload_instance: None,
          ..Expr::value(type_id)
        };
      }
      return Expr {
        overloads: vec![only],
        overload_instance: None,
        ..Expr::value(resolved.value)
      };
    }
    Expr {
      overloads: vec![only],
      overload_instance: None,
      ..Expr::place(resolved.value)
    }
  }

  /// What a constant that is written as a bare name or a module member
  /// stands for, when that name is one or more procedures (**L§7.5**). The
  /// alias is those declarations rather than a value, which is what lets a
  /// call site resolve it the way it would resolve the original.
  fn aliased_declarations(&mut self, decl: DeclId) -> Vec<DeclId> {
    let info = self.program().tree().decl(decl);
    let (Some(source), Some(node)) = (info.source, info.node) else {
      return Vec::new();
    };
    let Some(NodeData::Declaration(declaration)) = self.ast(source).map(|ast| ast.data(node))
    else {
      return Vec::new();
    };
    let Some(expression) = declaration.expression else {
      return Vec::new();
    };
    let names_a_name = matches!(
      self.ast(source).map(|ast| ast.data(expression)),
      Some(NodeData::Ident(_))
        | Some(NodeData::BinaryOperator {
          operator: oj_syntax::ast::OperatorType::DOT,
          ..
        })
    );
    if !names_a_name {
      return Vec::new();
    }
    let aliased = self
      .expression_type(info.scope, source, expression)
      .overloads;
    aliased
      .into_iter()
      .filter(|id| *id != decl && self.program().tree().decl(*id).kind == DeclKind::Procedure)
      .collect()
  }

  fn unary_type(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: OperatorType,
    operand: NodeId,
  ) -> Expr {
    // `.NAME` is a member of an enum the context decides (**L§5.12**).
    if operator == OperatorType::DOT {
      let name = self.ident_name(source, operand);
      return match name {
        Some(name) => Expr {
          type_id: TypeId::UNTYPED_ENUM,
          denoted: None,
          constant: Some(Const::new(TypeId::UNTYPED_ENUM, Value::EnumName(name))),
          lvalue: false,
          overloads: Vec::new(),
          overload_instance: None,
          explicitly_cast: false,
          autocast: false,
        },
        None => Expr::UNKNOWN,
      };
    }

    let inner = self.expression_type(scope, source, operand);
    match operator {
      // `*T` is a pointer type where `T` is one, and the address of an lvalue
      // otherwise (**L§3.2**).
      OperatorType::TIMES => match inner.denoted {
        Some(denoted) => Expr::type_expression(self.types_mut().pointer_to(denoted)),
        None if inner.is_unknown() => Expr::UNKNOWN,
        None => Expr::value(self.types_mut().pointer_to(inner.type_id)),
      },
      OperatorType::POINTER_DEREFERENCE | OperatorType::POSTFIX_DEREFERENCE => {
        match self.types().pointee(inner.type_id) {
          Some(pointee) => Expr::place(pointee),
          None => Expr::UNKNOWN,
        }
      }
      OperatorType::NOT => match inner
        .constant
        .as_ref()
        .and_then(|value| value.value.truth())
      {
        Some(truth) => Expr::constant(Const::bool(!truth)),
        None => Expr::value(TypeId::BOOL),
      },
      // `~` on a constant folds, which is what makes `MASK :: ~(SIZE - 1)` a
      // constant and `x & ~7` an ordinary integer expression (**L§5.11**).
      OperatorType::BITWISE_NOT
        if inner
          .constant
          .as_ref()
          .is_some_and(|value| value.value.as_int().is_some()) =>
      {
        let value = inner
          .constant
          .as_ref()
          .and_then(|value| value.value.as_int())
          .expect("the guard checked it");
        let width = self.types().integer_kind(inner.type_id);
        let complement = match width {
          Some(kind) => !value & ((1i128 << (kind.size() * 8)) - 1),
          None => !value,
        };
        let complement = match width {
          Some(kind) if kind.is_signed() => !value,
          _ => complement,
        };
        Expr::constant(Const::new(inner.type_id, Value::Int(complement)))
      }
      OperatorType::MINUS | OperatorType::PLUS | OperatorType::BITWISE_NOT => {
        // A struct's unary operators are overloads like its binary ones
        // (**L§7.7**).
        if let Some(result) =
          self.operator_overload(scope, source, node, operator, std::slice::from_ref(&inner))
        {
          return result;
        }
        // `+x` is `x` (**L§5.2**), constant and all.
        if operator == OperatorType::PLUS {
          return inner;
        }
        match inner
          .constant
          .as_ref()
          .filter(|_| operator == OperatorType::MINUS)
        {
          Some(value) => match negate(value) {
            Some(negated) => Expr::constant(negated),
            None => Expr::value(inner.type_id),
          },
          None => Expr::value(inner.type_id),
        }
      }
      _ => Expr::value(inner.type_id),
    }
  }

  fn binary_type(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: OperatorType,
    left: NodeId,
    right: NodeId,
  ) -> Expr {
    if operator == OperatorType::DOT {
      return self.member_access(scope, source, left, right);
    }
    if operator == OperatorType::ARRAY_SUBSCRIPT {
      let base = self.expression_type(scope, source, left);
      let index = self.expression_type(scope, source, right);
      if let Some((element, _)) = self.types().array_of(base.type_id) {
        return Expr::place(element);
      }
      // `operator []` reads, `operator *[]` gives the address of, an element
      // of a struct that behaves like an array (**L§7.7**).
      let operands = [base, index];
      for subscript in [SUBSCRIPT, ADDRESS_SUBSCRIPT] {
        if let Some(result) =
          self.operator_overload_named(scope, source, node, subscript, &operands)
        {
          return match self.types().pointee(result.type_id) {
            Some(pointee) if subscript == ADDRESS_SUBSCRIPT => Expr::place(pointee),
            _ => result,
          };
        }
      }
      // `p[i]` indexes a pointer as if it were an array (**L§5.4**), for a
      // pointee that has no subscript operator of its own to prefer.
      if let Some(pointee) = self.types().pointee(operands[0].type_id)
        && !self.has_operator(scope, source, node, ADDRESS_SUBSCRIPT)
        && !self.has_operator(scope, source, node, SUBSCRIPT)
      {
        return Expr::place(pointee);
      }
      return Expr::UNKNOWN;
    }

    let left_type = self.expression_type(scope, source, left);
    let right_type = self.expression_type(scope, source, right);
    if operator.is_assignment() {
      return Expr::value(TypeId::VOID);
    }
    if let Some(result) = self.operator_overload(
      scope,
      source,
      node,
      operator,
      &[left_type.clone(), right_type.clone()],
    ) {
      return result;
    }
    if matches!(
      operator,
      OperatorType::IS_EQUAL
        | OperatorType::IS_NOT_EQUAL
        | OperatorType::LESS
        | OperatorType::LESS_OR_EQUAL
        | OperatorType::GREATER
        | OperatorType::GREATER_OR_EQUAL
        | OperatorType::LOGICAL_AND
        | OperatorType::LOGICAL_OR
    ) {
      let folded = self.fold_binary(operator, &left_type, &right_type);
      return match folded {
        Some(value) => Expr::constant(value),
        None => Expr::value(TypeId::BOOL),
      };
    }

    // Subtracting two pointers gives the element difference as `s64`;
    // `p + n` and `p - n` advance by elements and stay pointers (**L§3.2**).
    if matches!(operator, OperatorType::PLUS | OperatorType::MINUS)
      && self.types().is_pointer(left_type.type_id)
    {
      if operator == OperatorType::MINUS && self.types().is_pointer(right_type.type_id) {
        return Expr::value(TypeId::S64);
      }
      if self.types().is_integer(self.harden(right_type.type_id)) {
        return Expr::value(left_type.type_id);
      }
    }

    // Bitwise operators take pointer operands without a cast, and give back a
    // pointer (**L§3.2**).
    if matches!(
      operator,
      OperatorType::BITWISE_AND | OperatorType::BITWISE_OR | OperatorType::BITWISE_XOR
    ) {
      for side in [&left_type, &right_type] {
        if self.types().is_pointer(side.type_id) {
          return Expr::value(side.type_id);
        }
      }
    }

    // A shift's result is the left operand's type; so is a bitwise operator's
    // when the left operand was explicitly cast, which is how
    // `cast,trunc(u32) a ^ b` stays a `u32` (**L§5.2**). Everything else
    // unifies (**L§5.10**).
    let keeps_left = matches!(
      operator,
      OperatorType::SHIFT_LEFT
        | OperatorType::SHIFT_RIGHT
        | OperatorType::ROTATE_LEFT
        | OperatorType::ROTATE_RIGHT
    ) || (left_type.explicitly_cast
      && matches!(
        operator,
        OperatorType::BITWISE_AND | OperatorType::BITWISE_OR | OperatorType::BITWISE_XOR
      ));
    let unified = if keeps_left {
      left_type.type_id
    } else {
      self.unify_operands(&left_type, &right_type)
    };
    match self.fold_binary(operator, &left_type, &right_type) {
      Some(value) => Expr::constant(Const::new(unified, value.value)),
      None => Expr::value(unified),
    }
  }

  /// `Match` for two flexible operands (**L§5.10**): a literal or a numeric
  /// constant takes the other side's type, and integers of different widths
  /// unify to the one that holds both.
  fn unify_operands(&mut self, left: &Expr, right: &Expr) -> TypeId {
    // A constant behaves like a literal of its value, so `size_of(u64) * n`
    // has `n`'s type rather than the constant's (**L§5.10** rule 2).
    for (constant, other) in [(left, right), (right, left)] {
      if let Some(crate::constants::Value::Int(number)) =
        constant.constant.as_ref().map(|value| &value.value)
        && self.types().integer_kind(constant.type_id).is_some()
        && other.constant.is_none()
        && let Some(kind) = self.types().integer_kind(other.type_id)
        && self.types().enum_of(other.type_id).is_none()
        && kind.holds(*number)
      {
        return other.type_id;
      }
    }
    self.unify(left.type_id, right.type_id)
  }

  pub(crate) fn unify(&mut self, left: TypeId, right: TypeId) -> TypeId {
    if left == right {
      return left;
    }
    if self.types().is_unknown(left) || self.types().is_unknown(right) {
      return TypeId::UNKNOWN;
    }
    if self.types().is_untyped(left) {
      return self.unify_untyped(left, right);
    }
    if self.types().is_untyped(right) {
      return self.unify_untyped(right, left);
    }
    // A float and an integer of the same "runtime-ness" do not mix without a
    // cast, and neither do two float widths (**L§5.10**).
    if let (Some(left_kind), Some(right_kind)) = (
      self.types().integer_kind(left),
      self.types().integer_kind(right),
    ) {
      if left_kind.contains_range_of(right_kind) {
        return left;
      }
      if right_kind.contains_range_of(left_kind) {
        return right;
      }
      return TypeId::UNKNOWN;
    }
    // Two float widths meet at the wider one, and an integer widens into a
    // float: `cast(float) j * j` and `f * u` are `float32`, measured with the
    // reference (**L§5.10**).
    if let (Some(left_kind), Some(right_kind)) = (
      self.types().float_kind(left),
      self.types().float_kind(right),
    ) {
      return match left_kind.size() >= right_kind.size() {
        true => left,
        false => right,
      };
    }
    if self.types().is_float(left) && self.types().is_integer(right) {
      return left;
    }
    if self.types().is_integer(left) && self.types().is_float(right) {
      return right;
    }
    // Pointer arithmetic and comparisons keep the pointer's type.
    if self.types().is_pointer(left) && self.types().is_integer(right) {
      return left;
    }
    if self.types().is_integer(left) && self.types().is_pointer(right) {
      return right;
    }
    TypeId::UNKNOWN
  }

  fn unify_untyped(&mut self, untyped: TypeId, other: TypeId) -> TypeId {
    match self.types().kind(untyped) {
      // An integer literal takes any numeric or enum type, and leaves another
      // literal as it is (**L§5.10**).
      TypeKind::UntypedInt => {
        let takes_it = self.types().is_numeric(other)
          || self.types().enum_of(other).is_some()
          || self.types().is_untyped(other);
        if takes_it { other } else { TypeId::UNKNOWN }
      }
      TypeKind::UntypedFloat(_) => {
        // An integer next to a float literal is converted to the literal's
        // float type (**L§5.10**).
        if self.types().is_float(other) {
          other
        } else if self.types().is_integer(other) {
          self.harden(untyped)
        } else {
          TypeId::UNKNOWN
        }
      }
      TypeKind::UntypedEnum => {
        if self.types().enum_of(other).is_some() {
          other
        } else {
          TypeId::UNTYPED_ENUM
        }
      }
      _ => TypeId::UNKNOWN,
    }
  }

  fn type_query(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    kind: TypeQueryKind,
    argument: NodeId,
  ) -> Expr {
    let type_id = self.type_from_node(scope, source, argument);
    match kind {
      TypeQueryKind::SizeOf => match self.layout_of(type_id).map(|layout| layout.size) {
        Some(size) => Expr::constant(Const::new(TypeId::S64, Value::Int(i128::from(size)))),
        // The size of a type the front end cannot lay out yet is not `s64`
        // with an unknown value: it is not known at all.
        None => Expr::UNKNOWN,
      },
      TypeQueryKind::TypeInfo => {
        let info = self.type_info_type(type_id);
        Expr::value(self.types_mut().pointer_to(info))
      }
      TypeQueryKind::InitializerOf => {
        let void_pointer = TypeId::VOID_POINTER;
        let mut signature = oj_types::ProcedureType::new(vec![void_pointer], Vec::new());
        signature.flags = oj_types::ProcedureFlags::HAS_NO_CONTEXT;
        Expr::value(self.types_mut().procedure(signature))
      }
    }
  }

  fn expression_query(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    kind: ExpressionQueryKind,
    argument: NodeId,
  ) -> Expr {
    match kind {
      ExpressionQueryKind::TypeOf => {
        let value = self.expression_type(scope, source, argument);
        let type_id = self.harden(value.type_id);
        Expr::type_expression(type_id)
      }
      ExpressionQueryKind::IsConstant => {
        let value = self.expression_type(scope, source, argument);
        Expr::constant(Const::bool(value.constant.is_some()))
      }
      ExpressionQueryKind::CodeOf => Expr::value(TypeId::CODE),
    }
  }

  fn ifx_type(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    payload: &oj_syntax::ast::IfNode,
  ) -> Expr {
    let condition = self.expression_type(scope, source, payload.condition);
    // `#ifx` picks one branch at compile time; only that one has a type, and
    // only that one is lowered (**L§6.10**).
    if payload
      .if_flags
      .contains(oj_syntax::ast::IfFlags::IS_STATIC)
      && let Some(truth) = condition.constant.and_then(|value| value.value.truth())
    {
      let taken = match truth {
        true => payload.then_block,
        false => payload.else_block,
      };
      return match taken {
        Some(taken) => {
          let value = self.expression_type(scope, source, taken);
          Expr::value(self.harden(value.type_id))
        }
        None => Expr::value(TypeId::VOID),
      };
    }
    // `ifx cond` with no `then` is worth the value the condition tested
    // (**L§5.13**).
    let then_node = payload
      .then_block
      .unwrap_or_else(|| self.ifx_then_value(source, payload.condition));
    let then_value = Some(self.expression_type(scope, source, then_node));
    let else_value = payload
      .else_block
      .map(|node| self.expression_type(scope, source, node));
    match (then_value, else_value) {
      // A branch written `xx e` takes the other one's type (**L§5.6**), and
      // so does one that is `null`, which names no type of its own
      // (**L§3.2**).
      (Some(left), Some(right)) if left.autocast || is_null(&left) => Expr::value(right.type_id),
      (Some(left), Some(right)) if right.autocast || is_null(&right) => Expr::value(left.type_id),
      (Some(left), Some(right)) => Expr::value(self.unify(left.type_id, right.type_id)),
      (Some(only), None) | (None, Some(only)) => Expr::value(self.harden(only.type_id)),
      (None, None) => Expr::value(TypeId::VOID),
    }
  }

  /// The value an `ifx` with no `then` is worth when its condition holds
  /// (**L§5.13**): the condition itself, or — one level of boolean operator
  /// down — the thing it tested. `ifx x > 3` is worth `x`, `ifx f(x)` is worth
  /// `x`, and `ifx a.b` is worth the `bool` `a.b`.
  pub fn ifx_then_value(&self, source: SourceId, condition: NodeId) -> NodeId {
    let Some(ast) = self.ast(source) else {
      return condition;
    };
    match ast.data(condition) {
      NodeData::UnaryOperator {
        operator: OperatorType::NOT,
        operand,
      } => self.ifx_then_value(source, *operand),
      NodeData::BinaryOperator { operator, left, .. } if is_boolean_operator(*operator) => *left,
      NodeData::ProcedureCall(call) => call
        .arguments
        .first()
        .map_or(condition, |argument| argument.expression),
      _ => condition,
    }
  }

  /// The `Type_Info_*` struct a `type_info(T)` is statically typed as
  /// (**L§5.14**).
  fn type_info_type(&mut self, type_id: TypeId) -> TypeId {
    let name: &[u8] = match self.types().kind(type_id) {
      TypeKind::Integer(_) | TypeKind::UntypedInt => b"Type_Info_Integer",
      TypeKind::Float(_) | TypeKind::UntypedFloat(_) => b"Type_Info_Float",
      TypeKind::String => b"Type_Info_String",
      TypeKind::Pointer(_) => b"Type_Info_Pointer",
      TypeKind::Procedure(_) => b"Type_Info_Procedure",
      TypeKind::Struct(_) => b"Type_Info_Struct",
      TypeKind::Array { .. } => b"Type_Info_Array",
      TypeKind::Enum(_) => b"Type_Info_Enum",
      TypeKind::Variant(_) => b"Type_Info_Variant",
      _ => b"Type_Info",
    };
    let name = self.interned().intern(name);
    self.preload_type(name)
  }

  fn source_code_location_type(&mut self) -> TypeId {
    let name = self.interned().intern(b"Source_Code_Location");
    self.preload_type(name)
  }
}

fn is_null(value: &Expr) -> bool {
  matches!(
    value.constant.as_ref().map(|constant| &constant.value),
    Some(Value::Null)
  )
}

fn negate(value: &Const) -> Option<Const> {
  match &value.value {
    Value::Int(number) => Some(Const::new(value.type_id, Value::Int(-number))),
    Value::Float(number) => Some(Const::new(value.type_id, Value::Float(-number))),
    _ => None,
  }
}

/// The subscript operators, which are named by their written text rather
/// than by an `Operator_Type` the reference exports (**L§7.7**): `operator []`
/// reads an element and `operator *[]` gives its address, which enables reads,
/// writes and compound assignment at once.
const SUBSCRIPT: &str = "[]";
const ADDRESS_SUBSCRIPT: &str = "*[]";

/// The operators an `ifx` with no `then` looks through to find the value its
/// condition tested (**L§5.13**). `.` is not one: `ifx a.b` is worth `a.b`.
fn is_boolean_operator(operator: OperatorType) -> bool {
  matches!(
    operator,
    OperatorType::IS_EQUAL
      | OperatorType::IS_NOT_EQUAL
      | OperatorType::LESS
      | OperatorType::LESS_OR_EQUAL
      | OperatorType::GREATER
      | OperatorType::GREATER_OR_EQUAL
      | OperatorType::LOGICAL_AND
      | OperatorType::LOGICAL_OR
  )
}

//! Expressions: values, places, conversions and calls.

use super::*;

use crate::ir::{BinaryOp, ParameterKind};

impl Lowering<'_, '_> {
  /// An expression, converted to `want` when the context asked for a type.
  pub(super) fn expression(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    want: Option<TypeId>,
  ) -> Option<Val> {
    let info = self.checker.expression(scope, source, node);
    let want = want.filter(|type_id| !self.checker.types().is_unknown(*type_id));

    // Anything the front end folded is emitted as data rather than walked
    // again (**L§5.11**).
    if let Some(constant) = info.constant.clone() {
      let target = want.unwrap_or_else(|| self.checker.hardened(info.type_id));
      // A constant that is being boxed into an `Any` becomes a value of its
      // own type first, since that is the type the `Any` records (**L§3.8**).
      let direct = match self.is_any(target) {
        true => self.checker.hardened(info.type_id),
        false => target,
      };
      if let Some(value) = self.constant_value(&constant, direct) {
        return self.convert(source, node, value, target);
      }
    }

    if self.checker.types().is_unknown(info.type_id) && info.overloads.is_empty() {
      self.unsupported(
        source,
        node,
        "an expression whose type the front end cannot work out yet",
        "M7",
      );
      return None;
    }

    let value = self.expression_inner(scope, source, node, &info, want)?;
    match want {
      Some(target) => self.convert(source, node, value, target),
      None => Some(value),
    }
  }

  /// A folded constant as a value of `target`, or `None` when it is of a kind
  /// the back end has no data form for.
  fn constant_value(&mut self, constant: &Const, target: TypeId) -> Option<Val> {
    if self.is_scalar(target)
      && let Some(value) = self.fold(constant, target)
    {
      return Some(self.constant(value, target));
    }
    if let Value::String(text) = &constant.value
      && self.is_string_like(target)
    {
      return Some(self.string_constant(text.clone()));
    }
    // An aggregate a `#run` produced is read-only data the program reads out
    // of, the same as any other constant (**L§12.1**).
    if let Value::Bytes(bytes) = &constant.value {
      return Some(self.bytes_constant(bytes.clone(), target));
    }
    // A `Type` is its `Type_Info`'s address at runtime, which is also what
    // makes two of them compare equal exactly when the types are the same
    // (**L§3.10**, **L§3.13**).
    if let Value::Type(queried) = constant.value {
      return self.type_info_value(queried, target);
    }
    None
  }

  fn is_any(&mut self, type_id: TypeId) -> bool {
    let underlying = self.checker.types().underlying(type_id);
    matches!(self.checker.types().kind(underlying), TypeKind::Any)
  }

  /// A string literal is read-only data: the value is the address of the
  /// `{count, data}` pair the back end laid down (**L§3.4**).
  fn string_constant(&mut self, text: Box<[u8]>) -> Val {
    let pointer = self.pointer_to(TypeId::STRING);
    let dest = self.value(pointer);
    self.emit(Inst::Const {
      dest,
      value: Constant::String(text),
    });
    Val {
      id: dest,
      type_id: TypeId::STRING,
      indirect: true,
    }
  }

  /// A conversion into a `[] T` (**L§3.3**). A fixed array has to be given a
  /// count and a pointer; a `string`, a resizable array and another view
  /// already start with exactly those two words, so only the recorded type
  /// changes.
  fn view_conversion(
    &mut self,
    value: Val,
    from: TypeId,
    to: TypeId,
    target: TypeId,
  ) -> Option<Val> {
    let types = self.checker.types();
    let (element, ArrayKind::View) = types.array_of(to)? else {
      return None;
    };
    let source_kind = match types.kind(from) {
      // `string` is `[] u8` with a name (**L§3.4**).
      TypeKind::String if element == TypeId::U8 => None,
      _ => match types.array_of(from) {
        Some((from_element, kind))
          if types.underlying(from_element) == types.underlying(element) =>
        {
          Some(kind)
        }
        _ => return None,
      },
    };

    match source_kind {
      Some(ArrayKind::Fixed(count)) => {
        let storage = self.address_of(value);
        let local = self.new_local(String::from("view"), target);
        let address = self.local_address(local);
        let count = self.constant(Constant::Int(i128::from(count)), TypeId::S64);
        self.store(address, count);
        let pointer = self.pointer_to(element);
        let slot = self.offset(address, VIEW_DATA, pointer);
        self.emit(Inst::Store {
          address: slot,
          value: storage,
        });
        Some(Val {
          id: address,
          type_id: target,
          indirect: true,
        })
      }
      _ => {
        let address = self.address_of(value);
        Some(Val {
          id: address,
          type_id: target,
          indirect: true,
        })
      }
    }
  }

  /// `#location(x)`: the `Source_Code_Location` of where `x` was written, with
  /// the 1-based line and character the reference reports (**L§5.14**).
  fn source_location(&mut self, source: SourceId, at: NodeId, type_id: TypeId) -> Option<Val> {
    let span = self.checker.tree_of(source)?.node(at).span;
    let file = self.checker.program().sources().file(source);
    let position = file.location(span.start);
    let path = file.path().to_string_lossy().into_owned();

    let definition = {
      let underlying = self.checker.types().underlying(type_id);
      self.checker.types().struct_of(underlying)?
    };
    let local = self.new_local(String::from("location"), type_id);
    let address = self.local_address(local);
    self.clear(address, type_id);

    let fields: [(&str, Val); 3] = [
      (
        "fully_pathed_filename",
        self.string_constant(path.into_bytes().into()),
      ),
      (
        "line_number",
        self.constant(Constant::Int(i128::from(position.line)), TypeId::S64),
      ),
      (
        "character_number",
        self.constant(Constant::Int(i128::from(position.column)), TypeId::S64),
      ),
    ];
    for (name, value) in fields {
      let symbol = self.checker.interner().intern(name.as_bytes());
      let member = self
        .checker
        .types()
        .struct_info(definition)
        .member(symbol)
        .cloned();
      let Some(member) = member else { continue };
      let slot = self.offset(address, member.offset, member.type_id);
      self.store(slot, value);
    }
    Some(Val {
      id: address,
      type_id,
      indirect: true,
    })
  }

  /// Boxes a value into an `Any`: `{type: *Type_Info, value_pointer: *void}`
  /// (**L§3.8**, **L§17**). The value has to be somewhere addressable, so one
  /// that only lived in a register is spilled first.
  fn box_any(&mut self, value: Val, target: TypeId) -> Option<Val> {
    let hardened = self.checker.hardened(value.type_id);
    let value = match hardened == value.type_id {
      true => value,
      // A literal has no type of its own to record, so it takes the one it
      // defaults to (**L§5.10**).
      false => Val {
        type_id: hardened,
        ..value
      },
    };
    let pointer = self.pointer_to(TypeId::VOID);
    let info_type = self.pointer_to(TypeId::VOID);
    let info = self.type_info_value(hardened, info_type)?;
    let stored = self.address_of(value);

    let local = self.new_local(String::from("any"), target);
    let address = self.local_address(local);
    self.emit(Inst::Store {
      address,
      value: info.id,
    });
    let slot = self.offset(address, ANY_VALUE_POINTER, pointer);
    self.emit(Inst::Store {
      address: slot,
      value: stored,
    });
    Some(Val {
      id: address,
      type_id: target,
      indirect: true,
    })
  }

  /// `type_info(T)`: the address of `T`'s record inside the type table
  /// (**L§17**). Laying the record out drags in every type it mentions, which
  /// is what makes the table hold exactly what the program can reach.
  pub(super) fn type_info_value(&mut self, queried: TypeId, pointer: TypeId) -> Option<Val> {
    // The table borrows the checker while it lays a record out, so it is taken
    // out of the lowering for the duration.
    let mut table = std::mem::take(&mut self.type_table);
    let offset = table.offset_of(self.checker, queried);
    self.type_table = table;

    let global = self.type_table_id();
    let base = self.value(pointer);
    self.emit(Inst::GlobalAddress { dest: base, global });
    if offset == 0 {
      return Some(Val {
        id: base,
        type_id: pointer,
        indirect: false,
      });
    }
    let dest = self.value(pointer);
    self.emit(Inst::Offset {
      dest,
      base,
      offset: offset as i64,
    });
    Some(Val {
      id: dest,
      type_id: pointer,
      indirect: false,
    })
  }

  /// The storage of an aggregate constant: the value is its address, the way
  /// every aggregate value is (**L§3.14**).
  fn bytes_constant(&mut self, bytes: Box<[u8]>, type_id: TypeId) -> Val {
    let pointer = self.pointer_to(type_id);
    let dest = self.value(pointer);
    self.emit(Inst::Const {
      dest,
      value: Constant::Bytes(bytes),
    });
    Val {
      id: dest,
      type_id,
      indirect: true,
    }
  }

  fn is_string_like(&mut self, type_id: TypeId) -> bool {
    let underlying = self.checker.types().underlying(type_id);
    matches!(self.checker.types().kind(underlying), TypeKind::String)
      || self
        .checker
        .types()
        .array_of(underlying)
        .is_some_and(|(element, kind)| element == TypeId::U8 && kind == ArrayKind::View)
  }

  fn expression_inner(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    info: &Expr,
    want: Option<TypeId>,
  ) -> Option<Val> {
    let data = self.checker.tree_of(source)?.data(node).clone();
    match data {
      NodeData::Ident(_) => self.name_value(source, node, info),
      NodeData::Literal(literal) => {
        self.literal_value(scope, source, node, &literal.value, info, want)
      }
      NodeData::UnaryOperator { operator, operand } => {
        self.unary_value(scope, source, node, operator, operand, info)
      }
      NodeData::BinaryOperator {
        operator,
        left,
        right,
        ..
      } => self.binary_value(scope, source, node, operator, left, right, info),
      NodeData::Cast(cast) => {
        let value = self.expression(scope, source, cast.expression, None)?;
        self.convert(source, node, value, info.type_id)
      }
      NodeData::ProcedureCall(_) => self.call_value(scope, source, node, info),
      NodeData::Context => {
        let context = self.context_pointer(source, node)?;
        Some(Val {
          id: context,
          type_id: self.context_type,
          indirect: true,
        })
      }
      NodeData::If(payload) => self.ifx_value(scope, source, node, &payload, info),
      NodeData::Block(block) => match block.statements.last() {
        Some(last) => {
          let last = *last;
          self.expression(scope, source, last, None)
        }
        None => None,
      },
      NodeData::ProcedureHeader(_) | NodeData::ProcedureBody { .. } => {
        self.unsupported(source, node, "an anonymous procedure", "M7");
        None
      }
      // `#compile_time` is a value each back end folds for itself: true in
      // compile-time code, false in the executable (**L§6.10**).
      NodeData::DirectiveCompileTime => {
        let running = self.mode == Mode::CompileTime;
        Some(self.constant(Constant::Bool(running), TypeId::BOOL))
      }
      NodeData::TypeQuery {
        query_kind: ast::TypeQueryKind::TypeInfo,
        type_to_query,
      } => {
        let scope = self
          .checker
          .scope_for(source, type_to_query, self.body_scope);
        let queried = self.checker.denoted_type(scope, source, type_to_query);
        self.type_info_value(queried, info.type_id)
      }
      NodeData::TypeQuery { .. } | NodeData::ExpressionQuery { .. } => {
        self.unsupported(source, node, "'initializer_of'", "M6");
        None
      }
      // `#caller_location` is the call site's, which a macro or a baked
      // default supplies (**L§7.13**); `#location` is this one's.
      NodeData::DirectiveLocation(location) if !location.is_caller_location => {
        let at = location.expression.unwrap_or(node);
        self.source_location(source, at, info.type_id)
      }
      NodeData::DirectiveLocation(_) => {
        self.unsupported(source, node, "'#caller_location'", "M7");
        None
      }
      _ => {
        self.unsupported(source, node, "this expression", "M7");
        None
      }
    }
  }

  /// A place: storage the program can read from and write to (**L§7.6**).
  pub(super) fn place(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<Val> {
    let value = self.expression(scope, source, node, None)?;
    if value.indirect {
      return Some(value);
    }
    let address = self.address_of(value);
    Some(Val {
      id: address,
      type_id: value.type_id,
      indirect: true,
    })
  }

  /// A condition: everything with a truth value becomes a `bool` the same way
  /// (**L§5.9**).
  pub(super) fn condition(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<ValueId> {
    let value = self.expression(scope, source, node, None)?;
    let boolean = self.truth(source, node, value)?;
    Some(self.scalar(boolean))
  }

  fn truth(&mut self, source: SourceId, node: NodeId, value: Val) -> Option<Val> {
    if self.checker.types().underlying(value.type_id) == TypeId::BOOL {
      return Some(value);
    }
    if !self.is_scalar(value.type_id) {
      self.unsupported(source, node, "a truth value for this type", "M7");
      return None;
    }
    let scalar = self.scalar(value);
    let zero = self.value(value.type_id);
    let is_float = self.checker.types().is_float(value.type_id);
    self.emit(Inst::Const {
      dest: zero,
      value: if is_float {
        Constant::Float(0.0)
      } else if self.checker.types().is_pointer(value.type_id) {
        Constant::Null
      } else {
        Constant::Int(0)
      },
    });
    let dest = self.value(TypeId::BOOL);
    self.emit(Inst::Binary {
      dest,
      operator: BinaryOp::NotEqual,
      left: scalar,
      right: zero,
    });
    Some(Val {
      id: dest,
      type_id: TypeId::BOOL,
      indirect: false,
    })
  }

  // ------------------------------------------------------------- names ------

  fn name_value(&mut self, source: SourceId, node: NodeId, info: &Expr) -> Option<Val> {
    let [only] = info.overloads[..] else {
      if info.overloads.len() > 1 {
        self.error(
          source,
          node,
          "This name stands for more than one procedure; only a call site can choose between them.",
        );
      } else {
        self.unsupported(source, node, "this name", "M7");
      }
      return None;
    };
    self.declaration_value(source, node, only, info.type_id)
  }

  fn declaration_value(
    &mut self,
    source: SourceId,
    node: NodeId,
    decl: DeclId,
    type_id: TypeId,
  ) -> Option<Val> {
    if let Some(local) = self.local_of_decl.get(&decl).copied() {
      let address = self.local_address(local);
      let local_type = self.locals[local.0 as usize].type_id;
      return Some(Val {
        id: address,
        type_id: local_type,
        indirect: true,
      });
    }
    let info = self.checker.program().tree().decl(decl).clone();
    match info.kind {
      DeclKind::Procedure => {
        let id = self.procedure_id(decl);
        let procedure_type = self.procedures[id.0 as usize].type_id;
        let dest = self.value(procedure_type);
        self.emit(Inst::ProcedureAddress {
          dest,
          procedure: id,
        });
        Some(Val {
          id: dest,
          type_id: procedure_type,
          indirect: false,
        })
      }
      DeclKind::Variable | DeclKind::Parameter | DeclKind::Iterator => {
        // A name that is not a local of this procedure is a global; a
        // parameter that is not is one a milestone the front end lacks would
        // have bound.
        if info.kind != DeclKind::Variable {
          self.unsupported(source, node, "this name", "M7");
          return None;
        }
        let id = self.global_id(decl);
        let global_type = self.globals[id.0 as usize].type_id;
        let pointer = self.pointer_to(global_type);
        let dest = self.value(pointer);
        self.emit(Inst::GlobalAddress { dest, global: id });
        Some(Val {
          id: dest,
          type_id: global_type,
          indirect: true,
        })
      }
      DeclKind::Constant => {
        // A constant that did not fold is one whose value is a procedure or a
        // `Code`, neither of which the front end represents yet.
        let _ = type_id;
        self.unsupported(source, node, "this constant", "M7");
        None
      }
      _ => {
        self.unsupported(source, node, "this name", "M7");
        None
      }
    }
  }

  // ---------------------------------------------------------- literals ------

  fn literal_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    literal: &LiteralValue,
    info: &Expr,
    want: Option<TypeId>,
  ) -> Option<Val> {
    // An undesignated `.[…]` or `.{…}` is built at the type the context asked
    // for, since it has none of its own (**L§5.7**, **L§5.8**).
    let type_id = match self.checker.types().kind(info.type_id) {
      TypeKind::UntypedLiteral => want.unwrap_or(info.type_id),
      _ => info.type_id,
    };
    match literal {
      LiteralValue::Array(array) => {
        let (element, _) = self.checker.types().array_of(type_id)?;
        let local = self.new_local(String::from("literal"), type_id);
        let address = self.local_address(local);
        self.clear(address, type_id);
        let (stride, _) = self.size_align(element);
        for (index, member) in array.members.clone().iter().enumerate() {
          let slot = self.offset(address, index as u64 * stride, element);
          if let Some(value) = self.expression(scope, source, *member, Some(element)) {
            self.store(slot, value);
          }
        }
        Some(Val {
          id: address,
          type_id,
          indirect: true,
        })
      }
      LiteralValue::Struct(literal) => {
        if self.checker.types().is_unknown(type_id) {
          self.unsupported(source, node, "an undesignated struct literal", "M7");
          return None;
        }
        let local = self.new_local(String::from("literal"), type_id);
        let address = self.local_address(local);
        self.default_initialize(address, type_id);
        let arguments = literal.arguments.clone();
        self.fill_struct_literal(scope, source, address, type_id, &arguments);
        Some(Val {
          id: address,
          type_id,
          indirect: true,
        })
      }
      _ => {
        self.unsupported(source, node, "this literal", "M6");
        None
      }
    }
  }

  fn fill_struct_literal(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    address: ValueId,
    type_id: TypeId,
    arguments: &[ast::Argument],
  ) {
    let underlying = self.checker.types().underlying(type_id);
    let Some(definition) = self.checker.types().struct_of(underlying) else {
      return;
    };
    let settable: Vec<(Symbol, TypeId, u64)> = self
      .checker
      .types()
      .struct_info(definition)
      .settable_members()
      .map(|member| (member.name, member.type_id, member.offset))
      .collect();
    let named: Vec<(Symbol, TypeId, u64)> = self
      .checker
      .types()
      .struct_info(definition)
      .members
      .iter()
      .map(|member| (member.name, member.type_id, member.offset))
      .collect();

    let mut position = 0usize;
    for argument in arguments {
      let member = match argument
        .name
        .and_then(|node| self.checker.name_at(source, node))
      {
        Some(name) => named.iter().find(|entry| entry.0 == name).copied(),
        None => {
          let member = settable.get(position).copied();
          position += 1;
          member
        }
      };
      let Some((_, member_type, offset)) = member else {
        continue;
      };
      let slot = self.offset(address, offset, member_type);
      if let Some(value) = self.expression(scope, source, argument.expression, Some(member_type)) {
        self.store(slot, value);
      }
    }
  }

  // ---------------------------------------------------------- operators -----

  fn unary_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: OperatorType,
    operand: NodeId,
    info: &Expr,
  ) -> Option<Val> {
    match operator {
      // `*x` on a value is its address (**L§3.2**).
      OperatorType::TIMES => {
        let place = self.place(scope, source, operand)?;
        let pointer = self.pointer_to(place.type_id);
        Some(Val {
          id: place.id,
          type_id: pointer,
          indirect: false,
        })
      }
      OperatorType::POINTER_DEREFERENCE | OperatorType::POSTFIX_DEREFERENCE => {
        let value = self.expression(scope, source, operand, None)?;
        let pointee = self.checker.types().pointee(value.type_id)?;
        let address = self.scalar(value);
        Some(Val {
          id: address,
          type_id: pointee,
          indirect: true,
        })
      }
      OperatorType::NOT => {
        let value = self.expression(scope, source, operand, None)?;
        let truth = self.truth(source, node, value)?;
        let operand = self.scalar(truth);
        let dest = self.value(TypeId::BOOL);
        self.emit(Inst::Unary {
          dest,
          operator: UnaryOp::LogicalNot,
          operand,
        });
        Some(Val {
          id: dest,
          type_id: TypeId::BOOL,
          indirect: false,
        })
      }
      OperatorType::MINUS | OperatorType::BITWISE_NOT => {
        let value = self.expression(scope, source, operand, Some(info.type_id))?;
        let operand = self.scalar(value);
        let dest = self.value(info.type_id);
        self.emit(Inst::Unary {
          dest,
          operator: match operator {
            OperatorType::MINUS => UnaryOp::Negate,
            _ => UnaryOp::BitwiseNot,
          },
          operand,
        });
        Some(Val {
          id: dest,
          type_id: info.type_id,
          indirect: false,
        })
      }
      OperatorType::PLUS => self.expression(scope, source, operand, Some(info.type_id)),
      _ => {
        self.unsupported(source, node, "this operator", "M7");
        None
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn binary_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: OperatorType,
    left: NodeId,
    right: NodeId,
    info: &Expr,
  ) -> Option<Val> {
    if operator == OperatorType::DOT {
      return self.member_value(scope, source, node, left, right, info);
    }
    if operator == OperatorType::ARRAY_SUBSCRIPT {
      return self.subscript_value(scope, source, node, left, right);
    }
    if matches!(
      operator,
      OperatorType::LOGICAL_AND | OperatorType::LOGICAL_OR
    ) {
      return self.short_circuit(scope, source, operator, left, right);
    }
    let Some(binary) = binary_operator(operator) else {
      self.unsupported(source, node, "this operator", "M7");
      return None;
    };

    // Comparison and arithmetic unify their operands; a shift and a pointer
    // offset keep the left one (**L§5.10**). The unified type is what both
    // sides are asked for, so that a literal — an integer, a `.NAME` — takes
    // it rather than being lowered on its own.
    let left_type = self.checker.expression(scope, source, left).type_id;
    let right_type = self.checker.expression(scope, source, right).type_id;
    let operand = if binary.is_comparison() {
      self.comparison_type(left_type, right_type)
    } else {
      info.type_id
    };
    let wanted = (!self.checker.types().is_unknown(operand)
      && !self.checker.types().is_untyped(operand))
    .then_some(operand);
    let left_value = self.expression(scope, source, left, wanted)?;
    let shifts = matches!(
      binary,
      BinaryOp::ShiftLeft | BinaryOp::ShiftRight | BinaryOp::RotateLeft | BinaryOp::RotateRight
    );
    let right_value = self.expression(scope, source, right, wanted.filter(|_| !shifts))?;
    self.binary_values(source, node, binary, left_value, right_value, info.type_id)
  }

  pub(super) fn binary_values(
    &mut self,
    source: SourceId,
    node: NodeId,
    operator: BinaryOp,
    left: Val,
    right: Val,
    result: TypeId,
  ) -> Option<Val> {
    // Pointer arithmetic counts in elements, not bytes (**L§3.2**).
    if matches!(operator, BinaryOp::Add | BinaryOp::Subtract)
      && self.checker.types().is_pointer(left.type_id)
      && self.checker.types().is_integer(right.type_id)
    {
      let pointee = self.checker.types().pointee(left.type_id)?;
      let (stride, _) = self.size_align(pointee);
      let base = self.scalar(left);
      let mut index = self.scalar(right);
      if operator == BinaryOp::Subtract {
        let negated = self.value(right.type_id);
        self.emit(Inst::Unary {
          dest: negated,
          operator: UnaryOp::Negate,
          operand: index,
        });
        index = negated;
      }
      let dest = self.value(left.type_id);
      self.emit(Inst::Index {
        dest,
        base,
        index,
        stride,
      });
      return Some(Val {
        id: dest,
        type_id: left.type_id,
        indirect: false,
      });
    }

    let operand_type = if operator.is_comparison() {
      self.comparison_type(left.type_id, right.type_id)
    } else {
      result
    };
    if !self.is_scalar(operand_type) {
      self.unsupported(source, node, "an operator on this type", "M7");
      return None;
    }
    let left = self.convert(source, node, left, operand_type)?;
    let right_type = match operator {
      BinaryOp::ShiftLeft | BinaryOp::ShiftRight | BinaryOp::RotateLeft | BinaryOp::RotateRight => {
        right.type_id
      }
      _ => operand_type,
    };
    let right = self.convert(source, node, right, right_type)?;
    let left = self.scalar(left);
    let right = self.scalar(right);
    let type_id = if operator.is_comparison() {
      TypeId::BOOL
    } else {
      result
    };
    let dest = self.value(type_id);
    self.emit(Inst::Binary {
      dest,
      operator,
      left,
      right,
    });
    Some(Val {
      id: dest,
      type_id,
      indirect: false,
    })
  }

  /// The type two operands of a comparison are brought to before it: the wider
  /// of the two, or the typed one when the other is a literal.
  fn comparison_type(&mut self, left: TypeId, right: TypeId) -> TypeId {
    if left == right {
      return left;
    }
    let types = self.checker.types();
    if types.is_untyped(left) {
      return right;
    }
    if types.is_untyped(right) {
      return left;
    }
    if let (Some(left_kind), Some(right_kind)) =
      (types.integer_kind(left), types.integer_kind(right))
    {
      if left_kind.contains_range_of(right_kind) {
        return left;
      }
      if right_kind.contains_range_of(left_kind) {
        return right;
      }
    }
    if types.is_float(left) {
      return left;
    }
    if types.is_float(right) {
      return right;
    }
    if types.is_pointer(left) {
      return left;
    }
    if types.is_pointer(right) {
      return right;
    }
    left
  }

  fn short_circuit(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    operator: OperatorType,
    left: NodeId,
    right: NodeId,
  ) -> Option<Val> {
    let result = self.new_local(String::from("logical"), TypeId::BOOL);
    let slot = self.local_address(result);
    let condition = self.condition(scope, source, left)?;
    self.emit(Inst::Store {
      address: slot,
      value: condition,
    });

    let evaluate = self.new_block();
    let join = self.new_block();
    let (then_block, else_block) = match operator {
      OperatorType::LOGICAL_AND => (evaluate, join),
      _ => (join, evaluate),
    };
    self.terminate(Terminator::Branch {
      condition,
      then_block,
      else_block,
    });

    self.current = evaluate;
    if let Some(value) = self.condition(scope, source, right) {
      let slot = self.local_address(result);
      self.emit(Inst::Store {
        address: slot,
        value,
      });
    }
    self.terminate(Terminator::Jump(join));

    self.current = join;
    let address = self.local_address(result);
    Some(Val {
      id: address,
      type_id: TypeId::BOOL,
      indirect: true,
    })
  }

  fn member_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    left: NodeId,
    right: NodeId,
    info: &Expr,
  ) -> Option<Val> {
    // A name reached through a module or a type is a declaration, not storage
    // (**L§5.3**).
    if let [only] = info.overloads[..] {
      return self.declaration_value(source, node, only, info.type_id);
    }
    if info.overloads.len() > 1 {
      self.error(
        source,
        node,
        "This name stands for more than one procedure; only a call site can choose between them.",
      );
      return None;
    }
    let name = self.checker.name_at(source, right)?;
    let base = self.expression(scope, source, left, None)?;
    let base = self.dereference(base);

    let underlying = self.checker.types().underlying(base.type_id);
    if let Some((element, kind)) = self.checker.types().array_of(underlying) {
      return self.array_field(source, node, base, name, element, kind);
    }
    // An `Any` is the pair Preload calls `Any_Struct`, so its two members are
    // found there (**L§3.8**, **L§17**).
    let underlying = match self.checker.types().kind(underlying) {
      TypeKind::Any => self.checker.any_struct_type(),
      _ => underlying,
    };
    let definition = self.checker.types().struct_of(underlying)?;
    let member = self
      .checker
      .types()
      .struct_info(definition)
      .member(name)
      .cloned()?;
    let address = self.address_of(base);
    let slot = self.offset(address, member.offset, member.type_id);
    Some(Val {
      id: slot,
      type_id: member.type_id,
      indirect: true,
    })
  }

  /// Member access through a pointer follows it one level (**L§3.2**).
  fn dereference(&mut self, value: Val) -> Val {
    let Some(pointee) = self.checker.types().pointee(value.type_id) else {
      return value;
    };
    let address = self.scalar(value);
    Val {
      id: address,
      type_id: pointee,
      indirect: true,
    }
  }

  /// `.count` and `.data`, which every array kind has and `string` shares
  /// (**L§3.3**, **L§3.4**).
  fn array_field(
    &mut self,
    source: SourceId,
    node: NodeId,
    base: Val,
    name: Symbol,
    element: TypeId,
    kind: ArrayKind,
  ) -> Option<Val> {
    let count = self.checker.interner().intern(b"count");
    let data = self.checker.interner().intern(b"data");
    let pointer = self.pointer_to(element);
    if name == count {
      return match kind {
        ArrayKind::Fixed(length) => {
          Some(self.constant(Constant::Int(i128::from(length)), TypeId::S64))
        }
        _ => {
          let address = self.address_of(base);
          let slot = self.offset(address, 0, TypeId::S64);
          Some(Val {
            id: slot,
            type_id: TypeId::S64,
            indirect: true,
          })
        }
      };
    }
    if name == data {
      return match kind {
        ArrayKind::Fixed(_) => {
          let address = self.address_of(base);
          Some(Val {
            id: address,
            type_id: pointer,
            indirect: false,
          })
        }
        _ => {
          let address = self.address_of(base);
          let slot = self.offset(address, 8, pointer);
          Some(Val {
            id: slot,
            type_id: pointer,
            indirect: true,
          })
        }
      };
    }
    self.unsupported(source, node, "this array field", "M7");
    None
  }

  fn subscript_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    left: NodeId,
    right: NodeId,
  ) -> Option<Val> {
    let base = self.expression(scope, source, left, None)?;
    let base = self.dereference(base);
    let index = self.expression(scope, source, right, Some(TypeId::S64))?;
    let Some((element, kind)) = self.checker.types().array_of(base.type_id) else {
      self.unsupported(source, node, "'operator []'", "M7");
      return None;
    };
    let data = match kind {
      ArrayKind::Fixed(_) => self.address_of(base),
      _ => {
        let address = self.address_of(base);
        let pointer = self.pointer_to(element);
        let slot = self.offset(address, 8, pointer);
        let dest = self.value(pointer);
        self.emit(Inst::Load {
          dest,
          address: slot,
        });
        dest
      }
    };
    let (stride, _) = self.size_align(element);
    let index = self.scalar(index);
    let pointer = self.pointer_to(element);
    let dest = self.value(pointer);
    self.emit(Inst::Index {
      dest,
      base: data,
      index,
      stride,
    });
    Some(Val {
      id: dest,
      type_id: element,
      indirect: true,
    })
  }

  fn ifx_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    payload: &ast::IfNode,
    info: &Expr,
  ) -> Option<Val> {
    let type_id = info.type_id;
    if self.checker.types().is_unknown(type_id) {
      self.unsupported(source, node, "this 'ifx'", "M7");
      return None;
    }
    let result = self.new_local(String::from("ifx"), type_id);
    let condition = self.condition(scope, source, payload.condition)?;
    let then_block = self.new_block();
    let else_block = self.new_block();
    let join = self.new_block();
    self.terminate(Terminator::Branch {
      condition,
      then_block,
      else_block,
    });

    for (block, branch) in [
      (then_block, payload.then_block),
      (else_block, payload.else_block),
    ] {
      self.current = block;
      if let Some(branch) = branch
        && let Some(value) = self.expression(scope, source, branch, Some(type_id))
      {
        let address = self.local_address(result);
        self.store(address, value);
      }
      self.terminate(Terminator::Jump(join));
    }

    self.current = join;
    let address = self.local_address(result);
    Some(Val {
      id: address,
      type_id,
      indirect: true,
    })
  }

  // ------------------------------------------------------------- calls ------

  fn context_pointer(&mut self, source: SourceId, node: NodeId) -> Option<ValueId> {
    match self.context_value {
      Some(value) => Some(value),
      None => {
        self.error(
          source,
          node,
          "'context' is not available here: this procedure was declared '#no_context'.",
        );
        None
      }
    }
  }

  fn call_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    info: &Expr,
  ) -> Option<Val> {
    let _ = info;
    let mut results = self.emit_call(scope, source, node, &[])?;
    if results.is_empty() {
      return Some(Val {
        id: self.value(TypeId::VOID),
        type_id: TypeId::VOID,
        indirect: false,
      });
    }
    Some(results.remove(0))
  }

  /// A call whose returns are written straight into places the caller already
  /// has, which is what `a, b := f();` needs (**L§4.5**).
  pub(super) fn call_into(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    targets: &[Option<(ValueId, TypeId)>],
  ) {
    let Some(results) = self.emit_call(scope, source, node, targets) else {
      return;
    };
    for (index, target) in targets.iter().enumerate() {
      let Some((address, type_id)) = target else {
        continue;
      };
      let Some(value) = results.get(index).copied() else {
        continue;
      };
      if let Some(converted) = self.convert(source, node, value, *type_id) {
        self.store(*address, converted);
      }
    }
  }

  fn emit_call(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    _targets: &[Option<(ValueId, TypeId)>],
  ) -> Option<Vec<Val>> {
    let Some(plan) = self.checker.call_plan(scope, source, node) else {
      self.unsupported(
        source,
        node,
        "a call the front end could not resolve to one procedure",
        "M7",
      );
      return None;
    };
    if plan.varargs.is_some_and(|(_, extra)| !extra.is_empty()) {
      self.unsupported(source, node, "a call with variable arguments", "M7");
      return None;
    }
    // A macro is expanded into its caller rather than called (**L§7.13**);
    // until it is, the call site has nothing to lower.
    if let Some(decl) = plan.callee
      && plan.instance.is_none()
      && let Some(body) = self.checker.procedure_body(decl)
      && body
        .flags
        .intersects(ast::ProcedureFlags::MACRO | ast::ProcedureFlags::POLYMORPHIC)
    {
      self.unsupported(source, node, "a macro or polymorphic call", "M7");
      return None;
    }

    let (callee, flags) = match (plan.instance, plan.callee) {
      // A polymorphic call names a specialization, not the header it was
      // written as (**L§7.8**).
      (Some(instance), _) => {
        let id = self.instance_id(instance);
        (Callee::Direct(id), self.procedures[id.0 as usize].flags)
      }
      (None, Some(decl)) => {
        let id = self.procedure_id(decl);
        (Callee::Direct(id), self.procedures[id.0 as usize].flags)
      }
      (None, None) => {
        let NodeData::ProcedureCall(call) = self.checker.tree_of(source)?.data(node).clone() else {
          return None;
        };
        let value = self.expression(scope, source, call.procedure_expression, None)?;
        let pointer = self.scalar(value);
        let mut flags = ProcedureFlags::empty();
        if let Some(signature) = self.checker.types().procedure_of(plan.type_id)
          && signature.flags.intersects(
            oj_types::ProcedureFlags::IS_C_CALL | oj_types::ProcedureFlags::HAS_NO_CONTEXT,
          )
        {
          flags |= ProcedureFlags::NO_CONTEXT;
        }
        (Callee::Indirect(pointer), flags)
      }
    };

    let abi = self.abi_of(plan.type_id, flags);
    let mut arguments = Vec::with_capacity(abi.parameters.len());
    let mut return_places: Vec<Val> = Vec::new();
    let mut given = 0usize;

    for parameter in &abi.parameters {
      match parameter.kind {
        ParameterKind::ReturnPointer => {
          let local = self.new_local(String::from("result"), parameter.type_id);
          let address = self.local_address(local);
          arguments.push(address);
          return_places.push(Val {
            id: address,
            type_id: parameter.type_id,
            indirect: true,
          });
        }
        ParameterKind::Context => {
          let context = self.context_pointer(source, node)?;
          arguments.push(context);
        }
        ParameterKind::Value | ParameterKind::Pointer => {
          let argument = plan.arguments.get(given)?;
          given += 1;
          let value = self.expression(
            argument.scope,
            argument.source,
            argument.node,
            Some(parameter.type_id),
          )?;
          match parameter.kind {
            ParameterKind::Value => arguments.push(self.scalar(value)),
            _ => arguments.push(self.address_of(value)),
          }
        }
      }
    }

    let dest = abi.direct_return.map(|type_id| self.value(type_id));
    self.emit(Inst::Call {
      dest,
      callee,
      signature: plan.type_id,
      arguments,
    });

    let mut results = Vec::new();
    if let (Some(dest), Some(type_id)) = (dest, abi.direct_return) {
      results.push(Val {
        id: dest,
        type_id,
        indirect: false,
      });
    }
    results.extend(return_places);
    Some(results)
  }

  // ------------------------------------------------------- conversions ------

  /// Rewrites a value into `target` the way the front end said it converts
  /// (**L§5.6**, **L§5.10**).
  pub(super) fn convert(
    &mut self,
    source: SourceId,
    node: NodeId,
    value: Val,
    target: TypeId,
  ) -> Option<Val> {
    if value.type_id == target || self.checker.types().is_unknown(target) {
      return Some(value);
    }
    let types = self.checker.types();
    // A `#type,distinct` and a `#type,isa` share their base's representation
    // (**L§3.11**).
    if types.underlying(value.type_id) == types.underlying(target) {
      return Some(Val {
        type_id: target,
        ..value
      });
    }

    let from = types.underlying(value.type_id);
    let to = types.underlying(target);
    let from_kind = types.kind(from).clone();
    let to_kind = types.kind(to).clone();

    // `[N] T` becomes a `[] T` by building the two words a view is
    // (**L§3.3**).
    if let (
      TypeKind::Array {
        element: from_element,
        kind: ArrayKind::Fixed(count),
      },
      TypeKind::Array {
        element: to_element,
        kind: ArrayKind::View,
      },
    ) = (&from_kind, &to_kind)
      && from_element == to_element
    {
      return Some(self.make_view(target, value, *count));
    }
    if matches!(from_kind, TypeKind::String)
      && matches!(&to_kind, TypeKind::Array { element, kind: ArrayKind::View } if *element == TypeId::U8)
    {
      return Some(Val {
        type_id: target,
        ..value
      });
    }
    if matches!(to_kind, TypeKind::String)
      && matches!(&from_kind, TypeKind::Array { element, kind: ArrayKind::View } if *element == TypeId::U8)
    {
      return Some(Val {
        type_id: target,
        ..value
      });
    }

    // A struct converts to the member it marked `#as` (**L§8.4**).
    if let TypeKind::Struct(definition) = from_kind {
      let member = self
        .checker
        .types()
        .struct_info(definition)
        .as_members()
        .find(|member| {
          let member_type = self.checker.types().underlying(member.type_id);
          member_type == to
        })
        .cloned();
      if let Some(member) = member {
        let address = self.address_of(value);
        let slot = self.offset(address, member.offset, member.type_id);
        let inner = Val {
          id: slot,
          type_id: member.type_id,
          indirect: true,
        };
        return self.convert(source, node, inner, target);
      }
    }

    // An `Any` is the pair `{type, value_pointer}`, so boxing is writing the
    // value's `Type_Info` beside its address (**L§3.8**).
    if matches!(to_kind, TypeKind::Any) {
      return self.box_any(value, target);
    }

    // A fixed array becomes a view over its own storage; everything else that
    // is already `{count, data}` only changes what the pointer is said to
    // point at (**L§3.3**, **L§3.4**).
    if let Some(view) = self.view_conversion(value, from, to, target) {
      return Some(view);
    }

    if !self.is_scalar(from) || !self.is_scalar(to) {
      self.unsupported(source, node, "this conversion", "M7");
      return None;
    }

    let types = self.checker.types();
    let kind = match (&from_kind, &to_kind) {
      _ if types.is_pointer(from) && types.is_pointer(to) => ConvertKind::Bitcast,
      _ if types.is_pointer(from) && types.is_integer(to) => ConvertKind::PointerToInteger,
      _ if types.is_integer(from) && types.is_pointer(to) => ConvertKind::IntegerToPointer,
      _ if matches!(from_kind, TypeKind::Procedure(_)) && types.is_pointer(to) => {
        ConvertKind::Bitcast
      }
      _ if types.is_pointer(from) && matches!(to_kind, TypeKind::Procedure(_)) => {
        ConvertKind::Bitcast
      }
      _ if matches!(from_kind, TypeKind::Procedure(_))
        && matches!(to_kind, TypeKind::Procedure(_)) =>
      {
        ConvertKind::Bitcast
      }
      // A `Type` is the address of its `Type_Info`, so it casts to and from a
      // pointer without changing anything (**L§3.10**).
      _ if matches!(from_kind, TypeKind::Type) && types.is_pointer(to) => ConvertKind::Bitcast,
      _ if types.is_pointer(from) && matches!(to_kind, TypeKind::Type) => ConvertKind::Bitcast,
      _ => {
        let from_int = types.integer_kind(from).or_else(|| {
          matches!(from_kind, TypeKind::Bool | TypeKind::UntypedInt).then_some(IntKind::U8)
        });
        let to_int = types.integer_kind(to).or_else(|| {
          matches!(to_kind, TypeKind::Bool | TypeKind::UntypedInt).then_some(IntKind::U8)
        });
        let from_float = types.float_kind(from);
        let to_float = types.float_kind(to);
        match (from_int, from_float, to_int, to_float) {
          // A `bool` is a byte; anything that reaches it is a truth test.
          (_, _, _, _) if matches!(to_kind, TypeKind::Bool) => {
            return self.truth(source, node, value);
          }
          (Some(from), None, Some(to), None) => match to.size().cmp(&from.size()) {
            std::cmp::Ordering::Greater if from.is_signed() => ConvertKind::IntegerSignExtend,
            std::cmp::Ordering::Greater => ConvertKind::IntegerZeroExtend,
            std::cmp::Ordering::Less => ConvertKind::IntegerTruncate,
            std::cmp::Ordering::Equal => ConvertKind::Bitcast,
          },
          (Some(from), None, None, Some(_)) => {
            if from.is_signed() {
              ConvertKind::SignedToFloat
            } else {
              ConvertKind::UnsignedToFloat
            }
          }
          (None, Some(_), Some(to), None) => {
            if to.is_signed() {
              ConvertKind::FloatToSigned
            } else {
              ConvertKind::FloatToUnsigned
            }
          }
          (None, Some(from), None, Some(to)) => match to.size().cmp(&from.size()) {
            std::cmp::Ordering::Greater => ConvertKind::FloatExtend,
            std::cmp::Ordering::Less => ConvertKind::FloatTruncate,
            std::cmp::Ordering::Equal => {
              return Some(Val {
                type_id: target,
                ..value
              });
            }
          },
          _ => {
            self.unsupported(source, node, "this conversion", "M7");
            return None;
          }
        }
      }
    };

    let operand = self.scalar(value);
    if kind == ConvertKind::Bitcast && self.checker.types().is_pointer(from) {
      // Pointers are opaque: only the recorded type changes.
      return Some(Val {
        id: operand,
        type_id: target,
        indirect: false,
      });
    }
    let dest = self.value(target);
    self.emit(Inst::Convert {
      dest,
      kind,
      operand,
    });
    Some(Val {
      id: dest,
      type_id: target,
      indirect: false,
    })
  }

  /// `{ count, data }` over a fixed array's storage (**L§3.3**).
  fn make_view(&mut self, target: TypeId, value: Val, count: u64) -> Val {
    let local = self.new_local(String::from("view"), target);
    let address = self.local_address(local);
    let count_slot = self.offset(address, 0, TypeId::S64);
    let length = self.value(TypeId::S64);
    self.emit(Inst::Const {
      dest: length,
      value: Constant::Int(i128::from(count)),
    });
    self.emit(Inst::Store {
      address: count_slot,
      value: length,
    });

    let element = self
      .checker
      .types()
      .array_of(target)
      .map(|(element, _)| element)
      .unwrap_or(TypeId::U8);
    let pointer = self.pointer_to(element);
    let data_slot = self.offset(address, 8, pointer);
    let data = self.address_of(value);
    self.emit(Inst::Store {
      address: data_slot,
      value: data,
    });
    Val {
      id: address,
      type_id: target,
      indirect: true,
    }
  }
}

fn binary_operator(operator: OperatorType) -> Option<BinaryOp> {
  Some(match operator {
    OperatorType::PLUS => BinaryOp::Add,
    OperatorType::MINUS => BinaryOp::Subtract,
    OperatorType::TIMES => BinaryOp::Multiply,
    OperatorType::DIVIDE => BinaryOp::Divide,
    OperatorType::MODULUS => BinaryOp::Modulus,
    OperatorType::BITWISE_AND => BinaryOp::BitwiseAnd,
    OperatorType::BITWISE_OR => BinaryOp::BitwiseOr,
    OperatorType::BITWISE_XOR => BinaryOp::BitwiseXor,
    OperatorType::SHIFT_LEFT => BinaryOp::ShiftLeft,
    OperatorType::SHIFT_RIGHT => BinaryOp::ShiftRight,
    OperatorType::ROTATE_LEFT => BinaryOp::RotateLeft,
    OperatorType::ROTATE_RIGHT => BinaryOp::RotateRight,
    OperatorType::IS_EQUAL => BinaryOp::Equal,
    OperatorType::IS_NOT_EQUAL => BinaryOp::NotEqual,
    OperatorType::LESS => BinaryOp::Less,
    OperatorType::LESS_OR_EQUAL => BinaryOp::LessOrEqual,
    OperatorType::GREATER => BinaryOp::Greater,
    OperatorType::GREATER_OR_EQUAL => BinaryOp::GreaterOrEqual,
    _ => return None,
  })
}

/// Where `Any_Struct.value_pointer` sits: after the `*Type_Info` (**L§17**).
const ANY_VALUE_POINTER: u64 = 8;

/// Where the `data` of a `string` or a `[] T` sits: after the count
/// (**L§3.3**, **L§3.4**).
const VIEW_DATA: u64 = 8;

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

    // A block in expression position is worth its last statement, but the
    // statements before it are work the program does; folding the block to
    // that last value would skip them (**L§5.13**).
    let is_block = matches!(
      self.checker.tree_of(source).map(|ast| ast.data(node)),
      Some(NodeData::Block(_))
    );

    // Anything else the front end folded is emitted as data rather than walked
    // again (**L§5.11**).
    if let Some(constant) = info.constant.clone().filter(|_| !is_block) {
      let target = want.unwrap_or_else(|| self.checker.hardened(info.type_id));
      // A constant that is being boxed into an `Any` becomes a value of its
      // own type first, since that is the type the `Any` records (**L§3.8**);
      // so does one that is being given a count, since the bytes it becomes are
      // the array's rather than the `{count, data}` pair (**L§3.3**).
      let own = self.checker.hardened(info.type_id);
      // A struct constant reaching another type does so through the member it
      // marked `#as`, which takes bytes out of it rather than reading the same
      // ones differently (**L§8.4**).
      let extracted = matches!(constant.value, Value::Bytes(_))
        && own != target
        && self
          .checker
          .types()
          .struct_of(self.checker.types().underlying(own))
          .is_some();
      let direct =
        match self.is_any(target) || (self.is_view(target) && !self.is_view(own)) || extracted {
          true => own,
          false => target,
        };
      if let Some(value) = self.constant_value(&constant, direct) {
        return self.convert(source, node, value, target);
      }
    }

    // A polymorphic procedure or a quick lambda takes the shape of the
    // concrete procedure it is given to (**L§7.8**, **L§7.9**).
    if let Some(target) = want
      && matches!(
        self.checker.tree_of(source).map(|ast| ast.data(node)),
        Some(NodeData::Ident(_) | NodeData::ProcedureHeader(_))
      )
      && let Some(instance) = self
        .checker
        .procedure_instance_for(scope, source, node, target)
    {
      let id = self.instance_id(instance);
      let procedure_type = self.procedures[id.0 as usize].type_id;
      let dest = self.value(procedure_type);
      self.emit(Inst::ProcedureAddress {
        dest,
        procedure: id,
      });
      return Some(Val {
        id: dest,
        type_id: procedure_type,
        indirect: false,
      });
    }
    // `xx e` has no type of its own: whatever asked for the value decides it
    // (**L§5.6**), so it is not the unknown the next check reports.
    if self.checker.types().is_unknown(info.type_id) && info.overloads.is_empty() && !info.autocast
    {
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
  pub(super) fn constant_value(&mut self, constant: &Const, target: TypeId) -> Option<Val> {
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
      let bytes = bytes.clone();
      return Some(self.bytes_constant(&bytes, target));
    }
    // A procedure constant is the address of the procedure it names, generated
    // like any other (**L§5.11**).
    if let Value::Procedure(decl) = constant.value {
      let id = self.procedure_id(decl);
      let procedure_type = self.procedures[id.0 as usize].type_id;
      let dest = self.value(procedure_type);
      self.emit(Inst::ProcedureAddress {
        dest,
        procedure: id,
      });
      return Some(Val {
        id: dest,
        type_id: procedure_type,
        indirect: false,
      });
    }
    // A `Type` is its `Type_Info`'s address at runtime, which is also what
    // makes two of them compare equal exactly when the types are the same
    // (**L§3.10**, **L§3.13**).
    if let Value::Type(queried) = constant.value {
      return self.type_info_value(queried, target);
    }
    // A `Code` is the address of the `Code_Node` the program was written at,
    // which is what a metaprogram is handed and what `compiler_get_nodes`
    // answers for (**L§13.1**, **C§5.3**). It means nothing outside compile
    // time, where the compiler that exported it is still alive.
    if let Value::Code {
      source,
      node,
      scope,
    } = constant.value
      && let Some(address) = self.checker.code_address(source, node, scope)
    {
      let dest = self.value(target);
      self.emit(Inst::Const {
        dest,
        value: Constant::Int(address as i128),
      });
      return Some(Val {
        id: dest,
        type_id: target,
        indirect: false,
      });
    }
    None
  }

  /// Whether a value of this type is the `{count, data}` pair a view is
  /// (**L§3.3**).
  fn is_view(&mut self, type_id: TypeId) -> bool {
    let underlying = self.checker.types().underlying(type_id);
    matches!(
      self.checker.types().kind(underlying),
      TypeKind::Array {
        kind: oj_types::ArrayKind::View,
        ..
      } | TypeKind::String
    )
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

  /// The header a procedure-valued expression names, when it names one: what
  /// `#location` of a procedure reports is where that procedure was written
  /// rather than where the name was (**L§5.14**).
  fn procedure_written_at(
    &mut self,
    source: SourceId,
    expression: NodeId,
  ) -> Option<(SourceId, NodeId)> {
    let scope = self.checker.scope_for(source, expression, self.body_scope);
    let oj_sema::Value::Procedure(decl) = self
      .checker
      .expression(scope, source, expression)
      .constant?
      .value
    else {
      return None;
    };
    let declared = self.checker.program().tree().decl(decl);
    let (declared_source, node) = (declared.source?, declared.node?);
    let ast::NodeData::Declaration(declaration) = self.checker.tree_of(declared_source)?.data(node)
    else {
      return None;
    };
    Some((declared_source, declaration.expression?))
  }

  /// `#location(x)`: the `Source_Code_Location` of where `x` was written, with
  /// the 1-based line and character the reference reports (**L§5.14**).
  fn source_location(&mut self, source: SourceId, at: NodeId, type_id: TypeId) -> Option<Val> {
    let span = self.checker.tree_of(source)?.node(at).span;
    let file = self.checker.program().sources().file(source);
    let position = file.location(span.start);
    // `Source_Code_Location.fully_pathed_filename` is what its name says: the
    // reference reports an absolute path however the file was named on the
    // command line (**L§5.14**).
    let path = std::path::absolute(file.path())
      .unwrap_or_else(|_| file.path().to_path_buf())
      .to_string_lossy()
      .into_owned();

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
  /// An aggregate a `#run` produced, as data the program can address
  /// (**L§12.1**). A pointer among those bytes named compile-time storage, so
  /// what it named is appended to them and the pointer points at that instead
  /// — which is how a `[] u8` from `add_global_data` reaches the executable
  /// (**C§3.3**).
  fn bytes_constant(&mut self, source: &oj_sema::RunBytes, type_id: TypeId) -> Val {
    /// A pointer's alignment on the one target orangejuice has.
    const POINTER_ALIGNMENT: usize = 8;

    let mut bytes = source.data.to_vec();
    let mut links = Vec::with_capacity(source.links.len());
    for link in &source.links {
      match &link.target {
        // A procedure is generated like any other and its address written in
        // by the module that has it (**L§5.11**).
        oj_sema::RunTarget::Procedure(decl) => {
          let id = self.procedure_id(*decl);
          links.push((link.at, crate::ir::ConstLink::Procedure(id)));
        }
        oj_sema::RunTarget::Data(data) => {
          // Every pointee starts on a pointer boundary, so that anything with
          // an alignment of its own still lands somewhere it can be read.
          let padding = bytes.len().next_multiple_of(POINTER_ALIGNMENT) - bytes.len();
          bytes.extend(std::iter::repeat_n(0u8, padding));
          let at = bytes.len() as u64;
          bytes.extend_from_slice(data);
          links.push((link.at, crate::ir::ConstLink::Offset(at + link.offset)));
        }
      }
    }
    let pointer = self.pointer_to(type_id);
    let dest = self.value(pointer);
    self.emit(Inst::Const {
      dest,
      value: Constant::Bytes {
        bytes: bytes.into_boxed_slice(),
        links: links.into_boxed_slice(),
      },
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
      NodeData::Ident(_) => self.name_value(scope, source, node, info, want),
      NodeData::Literal(literal) => {
        self.literal_value(scope, source, node, &literal.value, info, want)
      }
      NodeData::UnaryOperator { operator, operand } => {
        self.unary_value(scope, source, node, operator, operand, info, want)
      }
      NodeData::BinaryOperator {
        operator,
        left,
        right,
        ..
      } => self.binary_value(scope, source, node, operator, left, right, info, want),
      NodeData::Cast(cast) => {
        // `xx e` is a cast to whatever asked for the value (**L§5.6**).
        let target = match cast.target_type {
          Some(_) => Some(info.type_id),
          None => want,
        };
        // `cast(Stuff) .THIRD` casts something that has no type of its own:
        // the target is what says which enum the name belongs to (**L§5.12**).
        let operand = self
          .checker
          .expression(scope, source, cast.expression)
          .type_id;
        let hint = target.filter(|_| self.checker.types().is_untyped(operand));
        let value = self.expression(scope, source, cast.expression, hint)?;
        match target {
          Some(target) => self.convert(source, node, value, target),
          None => Some(value),
        }
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
      // A block in expression position is worth its last statement, and takes
      // the type whatever asked for the block wanted (**L§5.13**). The
      // statements before it are the work that produced it, so they run.
      NodeData::Block(block) => match block.statements.split_last() {
        Some((last, before)) => {
          let (last, before) = (*last, before.to_vec());
          for statement in before {
            self.statement(statement);
          }
          self.expression(scope, source, last, want)
        }
        None => None,
      },
      // A procedure written where a value goes is one the program has, named
      // by nothing: it is generated like any other and its address is the
      // value (**L§7.9**).
      NodeData::ProcedureBody { header, .. } => self.anonymous_procedure(scope, source, header),
      NodeData::ProcedureHeader(_) => self.anonymous_procedure(scope, source, node),
      // An `#insert` where a value goes is the one expression its text parsed
      // into, read in the scope the `#insert` was written in (**L§13.2**).
      NodeData::DirectiveInsert(_) => {
        let (inner_source, inner_scope, expression) =
          self.checker.insert_expression(scope, source, node)?;
        let previous_source = std::mem::replace(&mut self.body_source, inner_source);
        let previous_scope = std::mem::replace(&mut self.body_scope, inner_scope);
        let value = self.expression(inner_scope, inner_source, expression, want);
        self.body_source = previous_source;
        self.body_scope = previous_scope;
        value
      }
      // `#this` in a procedure with no name of its own is the address of the
      // one being generated, which is how a quick lambda calls itself
      // (**L§5.11**).
      NodeData::DirectiveThis => {
        let procedure = self.current_procedure?;
        let procedure_type = self.procedures[procedure.0 as usize].type_id;
        let dest = self.value(procedure_type);
        self.emit(Inst::ProcedureAddress { dest, procedure });
        Some(Val {
          id: dest,
          type_id: procedure_type,
          indirect: false,
        })
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
      // `initializer_of(T)` is a procedure the compiler writes out: it gives
      // storage the value a declaration of that type would start with
      // (**L§17**).
      NodeData::TypeQuery {
        query_kind: ast::TypeQueryKind::InitializerOf,
        type_to_query,
      } => {
        let scope = self
          .checker
          .scope_for(source, type_to_query, self.body_scope);
        let queried = self.checker.denoted_type(scope, source, type_to_query);
        let procedure = self.initializer_id(queried);
        let procedure_type = self.procedures[procedure.0 as usize].type_id;
        let dest = self.value(procedure_type);
        self.emit(Inst::ProcedureAddress { dest, procedure });
        Some(Val {
          id: dest,
          type_id: procedure_type,
          indirect: false,
        })
      }
      NodeData::TypeQuery { .. } | NodeData::ExpressionQuery { .. } => {
        self.unsupported(source, node, "this query", "M10");
        None
      }
      // `#caller_location` is the call site's, which a macro or a baked
      // default supplies (**L§7.13**); `#location` is this one's.
      NodeData::DirectiveLocation(location) if !location.is_caller_location => {
        let at = location.expression.unwrap_or(node);
        // `#location(x)` where `x` names a procedure is where that procedure
        // was written, which is what `#location(#this)` asks for (**L§5.14**).
        let declared = location
          .expression
          .and_then(|expression| self.procedure_written_at(source, expression));
        match declared {
          Some((declared_source, declared)) => {
            self.source_location(declared_source, declared, info.type_id)
          }
          None => self.source_location(source, at, info.type_id),
        }
      }
      // `#caller_location` is the site of the call whose arguments are being
      // evaluated, or of the macro being expanded (**L§7.13**).
      NodeData::DirectiveLocation(_) => match self.call_sites.last().copied() {
        Some((at_source, at)) => self.source_location(at_source, at, info.type_id),
        None => {
          self.unsupported(source, node, "'#caller_location'", "M7");
          None
        }
      },
      _ => {
        self.unsupported(source, node, "this expression", "M7");
        None
      }
    }
  }

  /// The address of a procedure written where a value goes (**L§7.9**), which
  /// is generated once per header the way a `#run` block's body is.
  fn anonymous_procedure(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    header: NodeId,
  ) -> Option<Val> {
    let scope = self.checker.scope_for(source, header, scope);
    let type_id = self.checker.procedure_type_at(source, header, scope);
    if self.mentions_unknown(type_id) {
      self.unsupported(source, header, "an anonymous procedure", "M7");
      return None;
    }
    let key = crate::lower::ProcKey::Node(source, header);
    let id = match self.procedure_ids.get(&key) {
      Some(id) => *id,
      None => self.declare_procedure(key, None, type_id, None),
    };
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

  pub(super) fn truth(&mut self, source: SourceId, node: NodeId, value: Val) -> Option<Val> {
    if self.checker.types().underlying(value.type_id) == TypeId::BOOL {
      return Some(value);
    }
    // A `string`, a view and a resizable array are true when they hold
    // something, which is their `count` (**L§5.9**).
    if let Some(count) = self.count_of(value) {
      return self.truth(source, node, count);
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

  /// Whether a type is a whole number as far as a cast is concerned: an
  /// integer, a `bool` or an enum (**L§3.2**, **L§5.6**).
  fn is_integral(&self, type_id: TypeId) -> bool {
    let types = self.checker.types();
    let underlying = types.underlying(type_id);
    types.is_integer(underlying)
      || matches!(
        types.kind(underlying),
        TypeKind::Bool | TypeKind::Enum(_) | TypeKind::UntypedInt
      )
  }

  /// The `count` word of anything that starts with one: a `string`, a `[] T`
  /// or a `[..] T` (**L§3.3**, **L§3.4**).
  fn count_of(&mut self, value: Val) -> Option<Val> {
    let underlying = self.checker.types().underlying(value.type_id);
    let counted = underlying == TypeId::STRING
      || matches!(
        self.checker.types().array_of(underlying),
        Some((_, ArrayKind::View | ArrayKind::Resizable))
      );
    if !counted {
      return None;
    }
    let address = self.address_of(value);
    let slot = self.offset(address, 0, TypeId::S64);
    Some(Val {
      id: slot,
      type_id: TypeId::S64,
      indirect: true,
    })
  }

  // ------------------------------------------------------------- names ------

  fn name_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    info: &Expr,
    want: Option<TypeId>,
  ) -> Option<Val> {
    // A bare name that a `using` of a value declared is that value's member
    // (**L§6.8**).
    if info.overloads.is_empty()
      && let Some(used) = self.checker.used_member(scope, source, node)
    {
      return self.used_member_value(source, node, &used);
    }
    // A name that stands for a whole overload set is narrowed by whatever
    // asked for the value: passing `to_upper` where a `(string) -> string` is
    // wanted means that one (**L§7.5**).
    if info.overloads.len() > 1
      && let Some(target) = want
      && let Some(chosen) = info
        .overloads
        .iter()
        .copied()
        .find(|decl| self.checker.decl_type(*decl).value == target)
    {
      return self.declaration_value(source, node, chosen, target);
    }
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

  /// The storage a `using`ed value's member names (**L§6.8**): the base is
  /// found the way any other name is, followed if it is a pointer, and the
  /// member read at its offset.
  fn used_member_value(
    &mut self,
    source: SourceId,
    node: NodeId,
    used: &oj_sema::UsedMember,
  ) -> Option<Val> {
    let base = match used.base {
      oj_sema::UsedBase::Declaration(decl) => {
        self.declaration_value(source, node, decl, used.base_type)?
      }
      // `using o.inner;` is an expression, evaluated where it was written
      // rather than where the bare name was (**L§6.8**).
      oj_sema::UsedBase::Expression {
        source,
        node,
        scope,
      } => self.expression(scope, source, node, None)?,
    };
    let address = match used.through_pointer {
      true => self.scalar(base),
      false => self.address_of(base),
    };
    let slot = self.offset(address, used.member.offset, used.member.type_id);
    Some(Val {
      id: slot,
      type_id: used.member.type_id,
      indirect: true,
    })
  }

  /// The storage a local declaration stands for, once the statement that
  /// declared it has run.
  pub(super) fn declaration_place(&mut self, decl: DeclId) -> Option<Val> {
    let local = self.local_of_decl.get(&self.local_key(decl)).copied()?;
    let address = self.local_address(local);
    Some(Val {
      id: address,
      type_id: self.locals[local.0 as usize].type_id,
      indirect: true,
    })
  }

  fn declaration_value(
    &mut self,
    source: SourceId,
    node: NodeId,
    decl: DeclId,
    type_id: TypeId,
  ) -> Option<Val> {
    if let Some(local) = self.local_of_decl.get(&self.local_key(decl)).copied() {
      let address = self.local_address(local);
      let local_type = self.locals[local.0 as usize].type_id;
      return Some(Val {
        id: address,
        type_id: local_type,
        indirect: true,
      });
    }
    let info = self.checker.program().tree().decl(decl);
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
        // A `#elsewhere` declaration reads a symbol somebody else laid out —
        // `__runtime_info` is the type table image — so the type the *name*
        // was declared with is what the storage holds, not whatever the
        // global's own type says (**L§4.8**).
        let global_type = match info.flags.contains(DeclarationFlags::ELSEWHERE) {
          true => type_id,
          false => self.globals[id.0 as usize].type_id,
        };
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
        // A constant the front end could not fold is one whose value has no
        // data form — `temp :: Allocator.{temporary_allocator_proc, null}`
        // holds a procedure's address (**L§5.11**). It has no storage of its
        // own, so its value is built where it is used.
        let (Some(declared), Some(at)) = (info.source, info.node) else {
          self.unsupported(source, node, "this constant", "M7");
          return None;
        };
        let Some(NodeData::Declaration(declaration)) =
          self.checker.tree_of(declared).map(|ast| ast.data(at))
        else {
          self.unsupported(source, node, "this constant", "M7");
          return None;
        };
        let Some(expression) = declaration.expression else {
          self.unsupported(source, node, "this constant", "M7");
          return None;
        };
        if !self.constants.insert(decl) {
          self.unsupported(source, node, "this constant", "M7");
          return None;
        }
        let scope = self.checker.scope_for(declared, expression, info.scope);
        let value = self.expression(scope, declared, expression, Some(type_id));
        self.constants.remove(&decl);
        value
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
        let (element, kind) = self.checker.types().array_of(type_id)?;
        let count = array.members.len() as u64;
        // `b: [] s32 = .[1, 2, 3]` needs somewhere for the elements to live:
        // the literal is laid out as a fixed array and the view points at it
        // (**L§5.8**, **L§3.3**).
        let storage = match kind {
          ArrayKind::Fixed(_) => type_id,
          _ => self
            .checker
            .types_table_mut()
            .array(element, ArrayKind::Fixed(count)),
        };
        // An empty literal names nothing at all: `.[]` as a view is a null
        // `data` and a zero `count` (**L§5.11**).
        if count == 0 && storage != type_id {
          let local = self.new_local(String::from("literal"), type_id);
          let address = self.local_address(local);
          self.clear(address, type_id);
          return Some(Val {
            id: address,
            type_id,
            indirect: true,
          });
        }
        // A literal whose members all fold is read-only data rather than a
        // frame's worth of stores, which is also what lets a `[] T` over one
        // outlive the procedure that returned it (**L§5.11**).
        if let Some(constant) = self.checker.constant_at(scope, source, node, storage)
          && let Some(value) = self.constant_value(&constant, storage)
        {
          return match storage == type_id {
            true => Some(value),
            false => Some(self.make_view(type_id, value, count)),
          };
        }
        let local = self.new_local(String::from("literal"), storage);
        let address = self.local_address(local);
        self.clear(address, storage);
        let (stride, _) = self.size_align(element);
        for (index, member) in array.members.clone().iter().enumerate() {
          let slot = self.offset(address, index as u64 * stride, element);
          if let Some(value) = self.expression(scope, source, *member, Some(element)) {
            self.store(slot, value);
          }
        }
        let value = Val {
          id: address,
          type_id: storage,
          indirect: true,
        };
        match storage == type_id {
          true => Some(value),
          false => Some(self.make_view(type_id, value, count)),
        }
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

  /// The storage a struct literal's designator names: `values[1]`, `a.b`, or
  /// `a.b[2]`, all rooted at a member of the literal's own type (**L§5.7**).
  /// The subscript has to be a constant, since it is a designator rather than
  /// an expression the program evaluates.
  fn designated_slot(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    address: ValueId,
    type_id: TypeId,
    designator: NodeId,
  ) -> Option<Val> {
    let data = self.checker.tree_of(source)?.data(designator).clone();
    match data {
      NodeData::Ident(_) => {
        let name = self.checker.name_at(source, designator)?;
        let underlying = self.checker.types().underlying(type_id);
        let definition = self.checker.types().struct_of(underlying)?;
        let member = self
          .checker
          .types()
          .struct_info(definition)
          .member(name)?
          .clone();
        let slot = self.offset(address, member.offset, member.type_id);
        Some(Val {
          id: slot,
          type_id: member.type_id,
          indirect: true,
        })
      }
      NodeData::BinaryOperator {
        operator: OperatorType::DOT,
        left,
        right,
        ..
      } => {
        let outer = self.designated_slot(scope, source, address, type_id, left)?;
        self.designated_slot(scope, source, outer.id, outer.type_id, right)
      }
      NodeData::BinaryOperator {
        operator: OperatorType::ARRAY_SUBSCRIPT,
        left,
        right,
        ..
      } => {
        let base = self.designated_slot(scope, source, address, type_id, left)?;
        let index = self
          .checker
          .expression(scope, source, right)
          .constant?
          .value
          .as_int()?;
        let (element, kind) = self.checker.types().array_of(base.type_id)?;
        if !matches!(kind, ArrayKind::Fixed(_)) {
          return None;
        }
        let (stride, _) = self.size_align(element);
        let slot = self.offset(base.id, index as u64 * stride, element);
        Some(Val {
          id: slot,
          type_id: element,
          indirect: true,
        })
      }
      _ => None,
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
      // `Body.{values[1] = 7}` designates a slot *inside* a member, which the
      // parser leaves as a whole assignment rather than as a name and a value
      // (**L§5.7**).
      if let Some(NodeData::BinaryOperator {
        operator: OperatorType::ASSIGN,
        left,
        right,
        ..
      }) = self
        .checker
        .tree_of(source)
        .map(|ast| ast.data(argument.expression))
      {
        let (left, right) = (*left, *right);
        if let Some(slot) = self.designated_slot(scope, source, address, type_id, left) {
          if let Some(value) = self.expression(scope, source, right, Some(slot.type_id)) {
            self.store(slot.id, value);
          }
          continue;
        }
      }
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

  #[allow(clippy::too_many_arguments)]
  fn unary_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: OperatorType,
    operand: NodeId,
    info: &Expr,
    want: Option<TypeId>,
  ) -> Option<Val> {
    if matches!(
      operator,
      OperatorType::MINUS | OperatorType::NOT | OperatorType::BITWISE_NOT
    ) && let Some(value) = self.unary_overload(scope, source, node, operator, operand)
    {
      return Some(value);
    }
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
        // `~.MEMBER` has no type of its own: what asked for the value decides
        // which enum it belongs to (**L§5.12**).
        let result = match self.checker.types().is_untyped(info.type_id) {
          true => want.unwrap_or_else(|| self.checker.hardened(info.type_id)),
          false => info.type_id,
        };
        let value = self.expression(scope, source, operand, Some(result))?;
        let operand = self.scalar(value);
        let dest = self.value(result);
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
          type_id: result,
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

  /// `operator *[]` on `a[i]`: the element's address, as a place the caller can
  /// read or write (**L§7.7**).
  pub(super) fn subscript_address(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    left: NodeId,
    right: NodeId,
  ) -> Option<Val> {
    let value = self.operator_call(scope, source, node, ADDRESS_SUBSCRIPT, &[left, right])?;
    let element = self.checker.types().pointee(value.type_id)?;
    let address = self.scalar(value);
    Some(Val {
      id: address,
      type_id: element,
      indirect: true,
    })
  }

  /// `-c` and `!c` on a struct are `operator -` and `operator !` (**L§7.7**).
  fn unary_overload(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: OperatorType,
    operand: NodeId,
  ) -> Option<Val> {
    self.operator_call(scope, source, node, operator.text(), &[operand])
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
    want: Option<TypeId>,
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
    // A struct operand may overload the operator (**L§7.7**); `a != b` with
    // only an `operator ==` in scope is that call, negated. A bare `.{…}`
    // operand takes the type the whole expression was asked for.
    let untyped = [left, right].into_iter().all(|operand| {
      let type_id = self.checker.expression(scope, source, operand).type_id;
      self.checker.types().is_untyped(type_id)
    });
    let hint = want.filter(|_| untyped);
    if let Some(value) =
      self.operator_call_hinted(scope, source, node, operator.text(), &[left, right], hint)
    {
      return Some(value);
    }
    if operator == OperatorType::IS_NOT_EQUAL
      && let Some(value) = self.operator_call(
        scope,
        source,
        node,
        OperatorType::IS_EQUAL.text(),
        &[left, right],
      )
    {
      let operand = self.scalar(value);
      let dest = self.value(TypeId::BOOL);
      self.emit(Inst::Unary {
        dest,
        operator: UnaryOp::LogicalNot,
        operand,
      });
      return Some(Val {
        id: dest,
        type_id: TypeId::BOOL,
        indirect: false,
      });
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
    // A shift keeps the left operand's type, which for a literal is what it
    // defaults to: `1 << n` is an `s64` (**L§5.2**, **L§5.10**). When neither
    // operand has a type of its own — `.WEST | .EAST` — what asked for the
    // value is what says which enum they belong to (**L§5.12**).
    let result = match self.checker.types().is_untyped(info.type_id) {
      true => want.unwrap_or_else(|| self.checker.hardened(info.type_id)),
      false => self.checker.hardened(info.type_id),
    };
    let operand = if binary.is_comparison() {
      self.comparison_type(left_type, right_type)
    } else {
      result
    };
    // Pointer arithmetic is not a unification: `p - q` is an `s64` but its
    // operands are pointers, and `p + n` keeps the pointer (**L§3.2**).
    let pointer_arithmetic = matches!(binary, BinaryOp::Add | BinaryOp::Subtract)
      && self.checker.types().is_pointer(left_type);
    let wanted = (!self.checker.types().is_unknown(operand)
      && !self.checker.types().is_untyped(operand)
      && !pointer_arithmetic)
      .then_some(operand);
    let left_value = self.expression(scope, source, left, wanted)?;
    // A shift's right operand is a count, and a pointer offset's is an index
    // of elements: neither takes the left operand's type (**L§3.2**,
    // **L§5.2**).
    let keeps_left = matches!(
      binary,
      BinaryOp::ShiftLeft | BinaryOp::ShiftRight | BinaryOp::RotateLeft | BinaryOp::RotateRight
    ) || self.checker.types().is_pointer(left_value.type_id);
    let right_value = self.expression(scope, source, right, wanted.filter(|_| !keeps_left))?;
    self.binary_values(source, node, binary, left_value, right_value, result)
  }

  /// `a == b` on strings: the same count and the same bytes (**L§3.4**). The
  /// bytes are compared with Preload's `memcmp`, and only when the counts
  /// agree, so neither side is read past its end.
  fn string_equality(
    &mut self,
    source: SourceId,
    node: NodeId,
    operator: BinaryOp,
    left: Val,
    right: Val,
  ) -> Option<Val> {
    let Some(memcmp) = self.checker.preload_procedure("memcmp") else {
      self.unsupported(source, node, "comparing two strings", "M7");
      return None;
    };
    let memcmp = self.procedure_id(memcmp);

    let result = self.new_local(String::from("equal"), TypeId::BOOL);
    let slot = self.local_address(result);
    let no = self.constant(Constant::Int(0), TypeId::BOOL);
    self.store(slot, no);

    let (left_count, left_data) = self.string_words(left);
    let (right_count, right_data) = self.string_words(right);
    let same_count = self.value(TypeId::BOOL);
    self.emit(Inst::Binary {
      dest: same_count,
      operator: BinaryOp::Equal,
      left: left_count,
      right: right_count,
    });
    let compare = self.new_block();
    let join = self.new_block();
    self.terminate(Terminator::Branch {
      condition: same_count,
      then_block: compare,
      else_block: join,
    });

    self.current = compare;
    let difference = self.value(TypeId::S16);
    self.emit(Inst::Call {
      dest: Some(difference),
      callee: Callee::Direct(memcmp),
      signature: self.procedures[memcmp.0 as usize].type_id,
      arguments: vec![left_data, right_data, left_count],
    });
    let zero = self.constant(Constant::Int(0), TypeId::S16);
    let zero = self.scalar(zero);
    let same_bytes = self.value(TypeId::BOOL);
    self.emit(Inst::Binary {
      dest: same_bytes,
      operator: BinaryOp::Equal,
      left: difference,
      right: zero,
    });
    let slot = self.local_address(result);
    self.emit(Inst::Store {
      address: slot,
      value: same_bytes,
    });
    self.terminate(Terminator::Jump(join));

    self.current = join;
    let slot = self.local_address(result);
    let equal = self.value(TypeId::BOOL);
    self.emit(Inst::Load {
      dest: equal,
      address: slot,
    });
    let id = match operator {
      BinaryOp::NotEqual => {
        let inverted = self.value(TypeId::BOOL);
        self.emit(Inst::Unary {
          dest: inverted,
          operator: UnaryOp::LogicalNot,
          operand: equal,
        });
        inverted
      }
      _ => equal,
    };
    Some(Val {
      id,
      type_id: TypeId::BOOL,
      indirect: false,
    })
  }

  /// The `{count, data}` a `string` is (**L§3.4**), as two loaded values.
  fn string_words(&mut self, value: Val) -> (ValueId, ValueId) {
    let address = self.address_of(value);
    let count_slot = self.offset(address, 0, TypeId::S64);
    let count = self.value(TypeId::S64);
    self.emit(Inst::Load {
      dest: count,
      address: count_slot,
    });
    let pointer = self.pointer_to(TypeId::U8);
    let data_slot = self.offset(address, 8, pointer);
    let data = self.value(pointer);
    self.emit(Inst::Load {
      dest: data,
      address: data_slot,
    });
    (count, data)
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
    // Two strings are equal when they hold the same bytes (**L§3.4**).
    if matches!(operator, BinaryOp::Equal | BinaryOp::NotEqual)
      && self.checker.types().underlying(left.type_id) == TypeId::STRING
      && self.checker.types().underlying(right.type_id) == TypeId::STRING
    {
      return self.string_equality(source, node, operator, left, right);
    }
    // A bitwise operator on a pointer works on the address (**L§3.2**).
    if matches!(
      operator,
      BinaryOp::BitwiseAnd | BinaryOp::BitwiseOr | BinaryOp::BitwiseXor
    ) && (self.checker.types().is_pointer(left.type_id)
      || self.checker.types().is_pointer(right.type_id))
    {
      let pointer = match self.checker.types().is_pointer(left.type_id) {
        true => left.type_id,
        false => right.type_id,
      };
      let left = self.convert(source, node, left, TypeId::U64)?;
      let right = self.convert(source, node, right, TypeId::U64)?;
      let bits = self.binary_values(source, node, operator, left, right, TypeId::U64)?;
      return self.convert(source, node, bits, pointer);
    }
    // One pointer minus another is how many elements apart they are
    // (**L§3.2**).
    if operator == BinaryOp::Subtract
      && self.checker.types().is_pointer(left.type_id)
      && self.checker.types().is_pointer(right.type_id)
    {
      let pointee = self.checker.types().pointee(left.type_id)?;
      let (stride, _) = self.size_align(pointee);
      let stride = stride.max(1);
      let left = self.scalar(left);
      let right = self.scalar(right);
      let bytes = self.value(TypeId::S64);
      self.emit(Inst::Binary {
        dest: bytes,
        operator: BinaryOp::Subtract,
        left,
        right,
      });
      if stride == 1 {
        return Some(Val {
          id: bytes,
          type_id: TypeId::S64,
          indirect: false,
        });
      }
      let size = self.constant(Constant::Int(i128::from(stride)), TypeId::S64);
      let size = self.scalar(size);
      let dest = self.value(TypeId::S64);
      self.emit(Inst::Binary {
        dest,
        operator: BinaryOp::Divide,
        left: bytes,
        right: size,
      });
      return Some(Val {
        id: dest,
        type_id: TypeId::S64,
        indirect: false,
      });
    }
    // Pointer arithmetic counts in elements, not bytes (**L§3.2**).
    if matches!(operator, BinaryOp::Add | BinaryOp::Subtract)
      && self.checker.types().is_pointer(left.type_id)
      && self.checker.types().is_integer(right.type_id)
    {
      let pointee = self.checker.types().pointee(left.type_id)?;
      // `*void` advances a byte at a time (**L§3.2**).
      let (stride, _) = self.size_align(pointee);
      let stride = stride.max(1);
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

  /// `.count` and `.data`, which every array kind has and `string` shares,
  /// plus the `.allocated` and `.allocator` a `[..]` array keeps after them
  /// (**L§3.3**, **L§3.4**, `Resizable_Array` in Preload).
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
        // An empty array literal's `data` is guaranteed null, so that whoever
        // frees it frees nothing (**L§5.11**).
        ArrayKind::Fixed(0) => Some(self.constant(Constant::Null, pointer)),
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
    if kind == ArrayKind::Resizable {
      let allocated = self.checker.interner().intern(b"allocated");
      let allocator = self.checker.interner().intern(b"allocator");
      if name == allocated {
        let address = self.address_of(base);
        let slot = self.offset(address, 16, TypeId::S64);
        return Some(Val {
          id: slot,
          type_id: TypeId::S64,
          indirect: true,
        });
      }
      if name == allocator {
        let type_id = self.checker.preload_named_type("Allocator");
        let address = self.address_of(base);
        let slot = self.offset(address, 24, type_id);
        return Some(Val {
          id: slot,
          type_id,
          indirect: true,
        });
      }
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
    // `operator []` reads an element; `operator *[]` gives its address, which
    // is a place. A read prefers `[]` and falls back to `*[]` (**L§7.7**).
    if let Some(value) = self.operator_call(scope, source, node, SUBSCRIPT, &[left, right]) {
      return Some(value);
    }
    if let Some(value) = self.subscript_address(scope, source, node, left, right) {
      return Some(value);
    }
    let base = self.expression(scope, source, left, None)?;
    let index = self.expression(scope, source, right, Some(TypeId::S64))?;
    // `p[i]` indexes a pointer as if it were an array (**L§5.4**), unless the
    // front end chose an `operator []` over that.
    let wanted = self.checker.expression(scope, source, node).type_id;
    if let Some(element) = self.checker.types().pointee(base.type_id)
      && element == wanted
    {
      let data = self.scalar(base);
      return Some(self.element_at(data, index, element));
    }
    let base = self.dereference(base);
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
    Some(self.element_at(data, index, element))
  }

  /// The storage of one element, `stride` bytes apart from the first
  /// (**L§3.3**).
  fn element_at(&mut self, data: ValueId, index: Val, element: TypeId) -> Val {
    let (stride, _) = self.size_align(element);
    let index = self.scalar(index);
    let pointer = self.pointer_to(element);
    let dest = self.value(pointer);
    self.emit(Inst::Index {
      dest,
      base: data,
      index,
      stride: stride.max(1),
    });
    Val {
      id: dest,
      type_id: element,
      indirect: true,
    }
  }

  fn ifx_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    payload: &ast::IfNode,
    info: &Expr,
  ) -> Option<Val> {
    // Both branches may be literals, in which case the `ifx` settles on what
    // they default to (**L§5.10**); whatever asked for the value converts it
    // afterwards.
    let type_id = self.checker.hardened(info.type_id);
    if self.checker.types().is_unknown(type_id) {
      self.unsupported(source, node, "this 'ifx'", "M7");
      return None;
    }
    // `#ifx` picks its branch at compile time, so only that one is lowered
    // (**L§6.10**).
    if payload.if_flags.contains(ast::IfFlags::IS_STATIC)
      && let Some(truth) = self
        .checker
        .expression(scope, source, payload.condition)
        .constant
        .and_then(|value| value.value.truth())
    {
      let taken = match truth {
        true => payload.then_block,
        false => payload.else_block,
      };
      return match taken {
        Some(taken) => self.expression(scope, source, taken, Some(type_id)),
        None => None,
      };
    }

    let result = self.new_local(String::from("ifx"), type_id);
    // A branch the `ifx` did not write is worth the *default value* of the
    // type the other one gave it, struct defaults and all (**L§5.13**).
    let address = self.local_address(result);
    match payload.then_block.is_none() || payload.else_block.is_none() {
      true => self.default_initialize(address, type_id),
      false => self.clear(address, type_id),
    }
    let condition = self.condition(scope, source, payload.condition)?;
    let then_block = self.new_block();
    let else_block = self.new_block();
    let join = self.new_block();
    self.terminate(Terminator::Branch {
      condition,
      then_block,
      else_block,
    });

    // `ifx cond` with no `then` is worth what the condition tested.
    let then_written = payload
      .then_block
      .or_else(|| Some(self.checker.ifx_then_value(source, payload.condition)));
    for (block, branch) in [(then_block, then_written), (else_block, payload.else_block)] {
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
    // `#procedure_of_call f(x)` is the procedure the call would reach rather
    // than the call itself: nothing is called, and the value is its address
    // (**L§7.10**).
    if matches!(
      self.checker.tree_of(source).map(|ast| ast.data(node)),
      Some(NodeData::ProcedureCall(call))
        if call.flags.contains(ast::CallFlags::RETURNS_PROCEDURE_POINTER_ONLY)
    ) {
      let plan = self.checker.call_plan(scope, source, node)?;
      let id = match (plan.instance, plan.callee) {
        (Some(instance), _) => self.instance_id(instance),
        (None, Some(decl)) => self.procedure_id(decl),
        (None, None) => return None,
      };
      let procedure_type = self.procedures[id.0 as usize].type_id;
      let dest = self.value(procedure_type);
      self.emit(Inst::ProcedureAddress {
        dest,
        procedure: id,
      });
      return Some(Val {
        id: dest,
        type_id: procedure_type,
        indirect: false,
      });
    }
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
    self.emit_planned_call(scope, source, node, plan, &[])
  }

  /// An `operator` procedure the operands of an operator node picked, called
  /// with those operands as its arguments (**L§7.7**).
  pub(super) fn operator_call(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: &str,
    operands: &[NodeId],
  ) -> Option<Val> {
    self.operator_call_with(scope, source, node, operator, operands, &[], None)
  }

  /// The same, telling the front end what type the whole expression was asked
  /// for, which is what says which struct a bare `.{…}` operand is a literal
  /// of (**L§7.7**).
  fn operator_call_hinted(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: &str,
    operands: &[NodeId],
    hint: Option<TypeId>,
  ) -> Option<Val> {
    self.operator_call_with(scope, source, node, operator, operands, &[], hint)
  }

  /// The same, with values the caller has already lowered for some of the
  /// operands — which is what keeps `w[i] += 1` from evaluating `i` twice.
  #[allow(clippy::too_many_arguments)]
  pub(super) fn operator_call_with(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: &str,
    operands: &[NodeId],
    given: &[Option<Val>],
    hint: Option<TypeId>,
  ) -> Option<Val> {
    let plan = self
      .checker
      .operator_plan_hinted(scope, source, node, operator, operands, hint)?;
    // A parameter that is a pointer to the operand's own type takes the
    // address of the place the operand names: `operator *[] :: (b: *Bucket…)`
    // is written `b[i]` (**L§7.7**).
    let mut overrides = Vec::with_capacity(plan.arguments.len());
    for (index, argument) in plan.arguments.iter().enumerate() {
      if let Some(value) = given.get(index).copied().flatten() {
        overrides.push(Some(value));
        continue;
      }
      let written = self
        .checker
        .expression(argument.scope, argument.source, argument.node)
        .type_id;
      if self.checker.types().pointee(argument.target) == Some(written) {
        let place = self.place(argument.scope, argument.source, argument.node)?;
        let pointer = self.pointer_to(place.type_id);
        overrides.push(Some(Val {
          id: place.id,
          type_id: pointer,
          indirect: false,
        }));
      } else {
        overrides.push(None);
      }
    }
    let mut results = self.emit_planned_call(scope, source, node, plan, &overrides)?;
    match results.is_empty() {
      true => Some(Val {
        id: self.value(TypeId::VOID),
        type_id: TypeId::VOID,
        indirect: false,
      }),
      false => Some(results.remove(0)),
    }
  }

  /// `overrides` gives values for declared parameters the caller has already
  /// lowered, by parameter index: `a += b` hands over the place `a` that way,
  /// since evaluating it a second time is not what the operator means.
  pub(super) fn emit_planned_call(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    plan: CallPlan,
    overrides: &[Option<Val>],
  ) -> Option<Vec<Val>> {
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

    // Preload's `compare_and_swap` is an operation, not a procedure
    // (**L§17**).
    if let Some(decl) = plan.callee
      && self.is_intrinsic_named(decl, "compare_and_swap")
    {
      return self.compare_and_swap(&plan);
    }
    // A macro is spliced into the block it was called from rather than called
    // (**L§7.13**).
    if let Some(instance) = plan.instance
      && self.checker.instance_info(instance).expansion.is_some()
    {
      return self.expand_macro(&plan, instance);
    }

    let (callee, flags) = match (plan.instance, plan.callee) {
      // A polymorphic call names a specialization, not the header it was
      // written as (**L§7.8**).
      (Some(instance), _) => {
        let id = self.instance_id(instance);
        (Callee::Direct(id), self.procedures[id.0 as usize].flags)
      }
      // A declaration that *is* a procedure is called by name; a parameter or
      // a constant that merely holds one is called through its value
      // (**L§7.5**).
      (None, Some(decl)) if self.checker.procedure_body(decl).is_some() => {
        let id = self.procedure_id(decl);
        (Callee::Direct(id), self.procedures[id.0 as usize].flags)
      }
      (None, Some(decl)) => {
        let value = self.declaration_value(source, node, decl, plan.type_id)?;
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
    let mut declared = 0usize;
    // `f(a,, allocator = temp)` runs the call with a *copy* of the context,
    // which is what keeps `Pool`'s own `alloc(…,, allocator=block_allocator)`
    // out of the pool it is refilling (**L§10.1**).
    let pushed = self.context_modification(scope, source, node);
    // A default written `#caller_location` is this call's site (**L§7.13**).
    self.call_sites.push((source, node));

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
          let context = match pushed {
            Some(pushed) => pushed,
            None => self.context_pointer(source, node)?,
          };
          arguments.push(context);
        }
        ParameterKind::Value | ParameterKind::Pointer => {
          // The last declared parameter of a varargs procedure is a `[] T`
          // the call site builds out of whatever was written past the fixed
          // ones (**L§7.3**).
          let value = match &plan.varargs {
            Some((slot, view, extra)) if *slot == declared => {
              let extra = extra.clone();
              self.gather_varargs(*view, &extra, plan.varargs_spread)?
            }
            _ => match overrides.get(declared).copied().flatten() {
              Some(value) => {
                given += 1;
                value
              }
              None => {
                let argument = *plan.arguments.get(given)?;
                given += 1;
                self.expression(
                  argument.scope,
                  argument.source,
                  argument.node,
                  Some(parameter.type_id),
                )?
              }
            },
          };
          declared += 1;
          match parameter.kind {
            ParameterKind::Value => arguments.push(self.scalar(value)),
            _ => arguments.push(self.address_of(value)),
          }
        }
      }
    }

    self.call_sites.pop();

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

  /// `f(a,, allocator = temp)`: a copy of the context with the named members
  /// set, which the callee gets instead of this procedure's own (**L§10.1**).
  /// An unnamed entry means `allocator = expression`.
  fn context_modification(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<ValueId> {
    let NodeData::ProcedureCall(call) = self.checker.tree_of(source)?.data(node).clone() else {
      return None;
    };
    let entries = call
      .context_modification
      .filter(|entries| !entries.is_empty())?;

    let context_type = self.context_type;
    let definition = self
      .checker
      .types()
      .struct_of(self.checker.types().underlying(context_type))?;
    let local = self.new_local(String::from("pushed_context"), context_type);
    let address = self.local_address(local);
    let current = self.context_pointer(source, node)?;
    let copy = Val {
      id: current,
      type_id: context_type,
      indirect: true,
    };
    self.store(address, copy);

    for entry in entries {
      let (written, value_node) = match self.checker.tree_of(source)?.data(entry).clone() {
        NodeData::BinaryOperator {
          operator: OperatorType::ASSIGN,
          left,
          right,
          ..
        } => (Some(left), right),
        _ => (None, entry),
      };
      let name = match written {
        Some(left) => self.checker.name_at(source, left)?,
        None => self.checker.interner().intern(b"allocator"),
      };
      let member = self
        .checker
        .types()
        .struct_info(definition)
        .member(name)?
        .clone();
      let slot = self.offset(address, member.offset, member.type_id);
      let written_scope = self.checker.scope_for(source, value_node, scope);
      if let Some(value) = self.expression(written_scope, source, value_node, Some(member.type_id))
      {
        self.store(slot, value);
      }
    }
    Some(address)
  }

  /// Whether a declaration is the `#intrinsic` of that name: an operation the
  /// back end provides rather than a procedure it calls (**L§17**).
  fn is_intrinsic_named(&mut self, decl: DeclId, name: &str) -> bool {
    let symbol = self.checker.program().tree().decl(decl).name;
    if self.checker.interner().resolve_lossy(symbol) != name {
      return false;
    }
    self
      .checker
      .procedure_body(decl)
      .is_some_and(|body| body.flags.contains(ast::ProcedureFlags::INTRINSIC))
  }

  /// Preload's `compare_and_swap(pointer, old, new)` (**L§17**): an atomic
  /// compare-exchange giving `(success, old_value)`.
  fn compare_and_swap(&mut self, plan: &CallPlan) -> Option<Vec<Val>> {
    let [pointer, old, new] = plan.arguments[..] else {
      return None;
    };
    let address = self.expression(pointer.scope, pointer.source, pointer.node, None)?;
    let element = self.checker.types().pointee(address.type_id)?;
    let address = self.scalar(address);
    let expected = self.expression(old.scope, old.source, old.node, Some(element))?;
    let desired = self.expression(new.scope, new.source, new.node, Some(element))?;
    let expected = self.scalar(expected);
    let desired = self.scalar(desired);
    let success = self.value(TypeId::BOOL);
    let previous = self.value(element);
    self.emit(Inst::AtomicCompareExchange {
      success,
      previous,
      address,
      expected,
      desired,
    });
    Some(vec![
      Val {
        id: success,
        type_id: TypeId::BOOL,
        indirect: false,
      },
      Val {
        id: previous,
        type_id: element,
        indirect: false,
      },
    ])
  }

  /// Splices a macro's body into the procedure being lowered (**L§7.13**). Its
  /// parameters become locals holding what the call site passed, its `return`s
  /// jump to a block after the splice, and its value is whatever it left in
  /// the storage set aside for it.
  /// `__reg`, the type Preload gives a register a macro can take (**L§15**).
  fn is_register_type(&mut self, type_id: TypeId) -> bool {
    let register = self.checker.preload_named_type("__reg");
    !self.checker.types().is_unknown(register) && type_id == register
  }

  /// The `#asm` register an argument names, when it names one.
  fn asm_register_named(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<DeclId> {
    let info = self.checker.expression(scope, source, node);
    let [only] = info.overloads[..] else {
      return None;
    };
    (self.checker.program().tree().decl(only).kind == DeclKind::AsmRegister).then_some(only)
  }

  fn expand_macro(&mut self, plan: &CallPlan, instance: InstanceId) -> Option<Vec<Val>> {
    let previous_instance = self.checker.enter_instance(Some(instance));
    // The whole expansion — its arguments and its body — is written at the
    // call site as far as `#caller_location` is concerned (**L§7.13**).
    let site = self.checker.instance_info(instance).expansion;
    if let Some(site) = site {
      self.call_sites.push(site);
    }
    let expanded = self.expand_macro_body(plan, instance);
    if site.is_some() {
      self.call_sites.pop();
    }
    self.checker.enter_instance(previous_instance);
    expanded
  }

  fn expand_macro_body(&mut self, plan: &CallPlan, instance: InstanceId) -> Option<Vec<Val>> {
    let body = self.checker.instance_body(instance)?;
    let block = body.block?;
    let signature = self
      .checker
      .types()
      .procedure_of(plan.type_id)
      .cloned()
      .unwrap_or_else(|| oj_types::ProcedureType::new(Vec::new(), Vec::new()));

    // The arguments are the caller's expressions, evaluated where they were
    // written; each parameter becomes a local of the expansion.
    let mut given = 0usize;
    for (index, type_id) in signature.arguments.iter().copied().enumerate() {
      let declared = body.parameters.get(index).copied().flatten();
      let spread = matches!(&plan.varargs, Some((slot, _, _)) if *slot == index);
      let written = match spread {
        true => None,
        false => {
          let argument = *plan.arguments.get(given)?;
          given += 1;
          Some(argument)
        }
      };
      // A parameter the expansion folded into a constant has no storage, and
      // a `Code` one is the argument itself, which is spliced in wherever the
      // body says rather than evaluated here (**L§7.13**).
      if declared.is_some_and(|decl| self.checker.instance_binds(decl)) {
        continue;
      }
      // A macro may take a register (**L§15**): the name is bound to the
      // caller's, and nothing is generated for the binding.
      if let (Some(decl), Some(argument)) = (declared, written)
        && self.is_register_type(type_id)
        && let Some(target) =
          self.asm_register_named(argument.scope, argument.source, argument.node)
      {
        let key = self.local_key(decl);
        self.asm_register_aliases.insert(key, target);
        continue;
      }
      let value = match (written, &plan.varargs) {
        (Some(argument), _) => self.expression(
          argument.scope,
          argument.source,
          argument.node,
          Some(type_id),
        )?,
        (None, Some((_, view, extra))) => {
          let extra = extra.clone();
          self.gather_varargs(*view, &extra, plan.varargs_spread)?
        }
        (None, None) => continue,
      };
      let Some(decl) = declared else {
        continue;
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
    let mut results: Vec<(ValueId, TypeId)> = Vec::new();
    for (index, type_id) in signature.returns.iter().copied().enumerate() {
      let local = self.new_local(String::from("expanded"), type_id);
      let address = self.local_address(local);
      self.clear(address, type_id);
      results.push((address, type_id));
      // A named return value is a local the macro's body may assign
      // (**L§7.2**).
      if let Some(Some(id)) = body.returns.get(index) {
        let key = self.local_key(*id);
        self.local_of_decl.insert(key, local);
      }
    }

    let previous_source = std::mem::replace(&mut self.body_source, body.source);
    let previous_scope = std::mem::replace(&mut self.body_scope, body.scope);
    self.expansions.push(crate::lower::Expansion {
      exit,
      results: results.clone(),
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

    Some(
      results
        .into_iter()
        .map(|(id, type_id)| Val {
          id,
          type_id,
          indirect: true,
        })
        .collect(),
    )
  }

  /// Builds the `[] T` a `..T` parameter receives: the extra arguments are
  /// stored into a fixed array of the call's own and handed over as a view
  /// (**L§7.3**). `..xs` skips that and passes the array it names.
  fn gather_varargs(
    &mut self,
    view: TypeId,
    extra: &[oj_sema::PlannedArgument],
    spread: bool,
  ) -> Option<Val> {
    if spread {
      let argument = extra.first()?;
      let value = self.expression(argument.scope, argument.source, argument.node, Some(view))?;
      return self.convert(argument.source, argument.node, value, view);
    }
    let element = self
      .checker
      .types()
      .array_of(view)
      .map(|(element, _)| element)?;
    let count = extra.len() as u64;
    let storage = self
      .checker
      .types_table_mut()
      .array(element, ArrayKind::Fixed(count));
    let local = self.new_local(String::from("varargs"), storage);
    let base = self.local_address(local);
    let (stride, _) = self.size_align(element);
    for (index, argument) in extra.iter().enumerate() {
      let value = self.expression(
        argument.scope,
        argument.source,
        argument.node,
        Some(element),
      )?;
      let value = self.convert(argument.source, argument.node, value, element)?;
      let slot = self.offset(base, index as u64 * stride, element);
      self.store(slot, value);
    }
    let storage = Val {
      id: base,
      type_id: storage,
      indirect: true,
    };
    Some(self.make_view(view, storage, count))
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

    // A string literal casts to the bytes the compiler laid down and to the
    // count it wrote beside them; only a literal reaches here, since the
    // checker rejects a string a program computed (**L§3.4**, **L§5.6**).
    if from == TypeId::STRING && (matches!(to_kind, TypeKind::Pointer(_)) || types.is_numeric(to)) {
      let (count, data) = self.string_words(value);
      let word = match matches!(to_kind, TypeKind::Pointer(_)) {
        true => data,
        false => count,
      };
      let word = Val {
        id: word,
        type_id: match matches!(to_kind, TypeKind::Pointer(_)) {
          true => self.pointer_to(TypeId::U8),
          false => TypeId::S64,
        },
        indirect: false,
      };
      return self.convert(source, node, word, target);
    }

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

    // Passing a `*S` where an `S` is wanted follows the pointer (**L§7.6**),
    // which is also how a `*S` reaches the member `S` marked `#as`.
    if let Some(pointee) = self.checker.types().pointee(from)
      && self
        .checker
        .types()
        .struct_of(self.checker.types().underlying(pointee))
        .is_some()
      && self
        .checker
        .types()
        .struct_of(self.checker.types().underlying(to))
        .is_some()
    {
      let address = self.scalar(value);
      let inner = Val {
        id: address,
        type_id: pointee,
        indirect: true,
      };
      return self.convert(source, node, inner, target);
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

    // `cast(bool) x` is a truth test whatever `x` is, and a `string` or a view
    // is true when its count is (**L§5.9**).
    if matches!(to_kind, TypeKind::Bool) {
      return self.truth(source, node, value);
    }

    if !self.is_scalar(from) || !self.is_scalar(to) {
      self.unsupported(source, node, "this conversion", "M7");
      return None;
    }

    // A `bool` and an enum are integers as far as a cast to a pointer is
    // concerned (**L§3.2**).
    let (from_integral, to_integral) = (self.is_integral(from), self.is_integral(to));
    let types = self.checker.types();
    let kind = match (&from_kind, &to_kind) {
      _ if types.is_pointer(from) && types.is_pointer(to) => ConvertKind::Bitcast,
      _ if types.is_pointer(from) && to_integral => ConvertKind::PointerToInteger,
      _ if from_integral && types.is_pointer(to) => ConvertKind::IntegerToPointer,
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
    // An empty array's `data` is guaranteed null (**L§5.11**), so that whoever
    // frees a view over one frees nothing.
    let data = match count {
      0 => {
        let null = self.value(pointer);
        self.emit(Inst::Const {
          dest: null,
          value: Constant::Null,
        });
        null
      }
      _ => self.address_of(value),
    };
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

/// The subscript operators, which are named by their written text (**L§7.7**):
/// `operator []` reads an element and `operator *[]` gives its address.
pub(super) const SUBSCRIPT: &str = "[]";
pub(super) const ADDRESS_SUBSCRIPT: &str = "*[]";
/// `operator []=` writes one, which is what `a[i] = v` prefers.
pub(super) const SUBSCRIPT_ASSIGN: &str = "[]=";

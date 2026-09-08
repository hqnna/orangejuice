use oj_diag::{SourceId, Span};
use oj_lexer::Symbol;
use oj_scope::{DeclId, DeclKind, Resolution, ScopeId, ScopeKind};
use oj_syntax::ast::{
  ExpressionQueryKind, FileInfoKind, IdentFlags, LiteralFlags, LiteralValue, NodeData, NodeId,
  OperatorType, ProcedureFlags, TypeQueryKind,
};
use oj_types::{ArrayKind, FloatKind, TypeId, TypeKind};

use crate::checker::{Checker, Expr};
use crate::constants::{Const, Value};

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
      NodeData::DirectiveProcedureName { .. } => Expr::value(TypeId::STRING),
      NodeData::DirectiveExists(_) => Expr::constant(Const::bool(false)),
      // `#compile_time` is a `bool` but not a constant one (**L§5.14**).
      NodeData::DirectiveCompileTime => Expr::value(TypeId::BOOL),
      NodeData::DirectiveCode { .. } | NodeData::DirectiveCallerCode => Expr::value(TypeId::CODE),
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
        Expr::value(self.types_mut().array(element, ArrayKind::Fixed(count)))
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
        Expr::value(type_id)
      }
    }
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
      return Expr::type_expression(self.polymorph_type(source, node, name));
    }
    let mut scope = self.scope_at(source, node, scope);
    loop {
      let Resolution::Found(candidates) = self.program().tree().lookup(scope, name) else {
        return Expr::UNKNOWN;
      };
      // A local is not in scope in its own initializer, so `type, ok :=
      // get_type(t, type)` reads the outer `type` (**L§6.13**).
      let Some(shadowed) = self.shadowed_declaration(&candidates) else {
        return self.declarations_type(&candidates);
      };
      let owner = self.program().tree().decl(shadowed).scope;
      match self.program().tree().scope(owner).parent {
        Some(parent) => scope = parent,
        None => return Expr::UNKNOWN,
      }
    }
  }

  /// The declaration to look past, when every candidate is a local whose own
  /// type is still being worked out.
  fn shadowed_declaration(&self, candidates: &[DeclId]) -> Option<DeclId> {
    let tree = self.program().tree();
    let all_pending = candidates.iter().all(|id| {
      self.is_resolving(*id) && tree.scope(tree.decl(*id).scope).kind == ScopeKind::Imperative
    });
    (all_pending && !candidates.is_empty()).then(|| candidates[0])
  }

  /// The type a resolved name stands for. Several declarations mean an
  /// overload set, which is a type only a call site can narrow (**L§7.5**).
  pub(crate) fn declarations_type(&mut self, candidates: &[DeclId]) -> Expr {
    // The same declaration can be reached down two import paths, which is not
    // an overload set (**L§7.5**).
    let mut real: Vec<DeclId> = candidates
      .iter()
      .copied()
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
        ..Expr::value(TypeId::OVERLOAD_SET)
      };
    };

    let resolved = self.decl_type(only);
    if let Some(denoted) = resolved.denoted {
      return Expr::type_expression(denoted);
    }
    let decl = self.program().tree().decl(only).clone();
    if decl
      .flags
      .contains(oj_syntax::ast::DeclarationFlags::IS_CONSTANT)
      || self.is_module_parameter(only)
    {
      if let Some(value) = self.decl_constant(only) {
        return Expr {
          overloads: vec![only],
          ..Expr::constant(value)
        };
      }
      return Expr {
        overloads: vec![only],
        ..Expr::value(resolved.value)
      };
    }
    Expr {
      overloads: vec![only],
      ..Expr::place(resolved.value)
    }
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
          explicitly_cast: false,
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
      OperatorType::NOT => Expr::value(TypeId::BOOL),
      OperatorType::MINUS | OperatorType::PLUS | OperatorType::BITWISE_NOT => {
        // A struct's unary operators are overloads like its binary ones
        // (**L§7.7**).
        if let Some(result) =
          self.operator_overload(scope, source, node, operator, std::slice::from_ref(&inner))
        {
          return result;
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
      for subscript in [OperatorType::ARRAY_SUBSCRIPT, ADDRESS_SUBSCRIPT] {
        if let Some(result) = self.operator_overload(scope, source, node, subscript, &operands) {
          return match self.types().pointee(result.type_id) {
            Some(pointee) if subscript == ADDRESS_SUBSCRIPT => Expr::place(pointee),
            _ => result,
          };
        }
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

    // Subtracting two pointers gives the element difference as `s64`
    // (**L§3.2**).
    if operator == OperatorType::MINUS
      && self.types().is_pointer(left_type.type_id)
      && self.types().is_pointer(right_type.type_id)
    {
      return Expr::value(TypeId::S64);
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
    if self.types().is_float(left) && self.types().is_float(right) {
      return TypeId::UNKNOWN;
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
    self.expression_type(scope, source, payload.condition);
    let then_type = payload
      .then_block
      .map(|node| self.expression_type(scope, source, node).type_id);
    let else_type = payload
      .else_block
      .map(|node| self.expression_type(scope, source, node).type_id);
    match (then_type, else_type) {
      (Some(left), Some(right)) => Expr::value(self.unify(left, right)),
      (Some(only), None) | (None, Some(only)) => Expr::value(only),
      (None, None) => Expr::value(TypeId::VOID),
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

fn negate(value: &Const) -> Option<Const> {
  match &value.value {
    Value::Int(number) => Some(Const::new(value.type_id, Value::Int(-number))),
    Value::Float(number) => Some(Const::new(value.type_id, Value::Float(-number))),
    _ => None,
  }
}

/// `operator *[]` — the address-of-element subscript, which enables reads,
/// writes and compound assignment at once (**L§7.7**). It is not a
/// `Operator_Type` the reference exports, so the number is orangejuice's.
const ADDRESS_SUBSCRIPT: OperatorType = OperatorType(501);

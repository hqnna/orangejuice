use oj_diag::SourceId;
use oj_scope::{Resolution, ScopeId};
use oj_syntax::ast::{NodeData, NodeId, OperatorType};
use oj_types::{TypeId, TypeKind};

use crate::checker::{Checker, Expr};
use crate::overload::Resolved;

impl Checker<'_> {
  /// The result of an operator the operands overload (**L§7.7**). Operators are
  /// constants named by their own text, so `a + b` on a struct resolves the
  /// overload set of `"+"` the way a call resolves a procedure's.
  pub(crate) fn operator_overload(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: OperatorType,
    operands: &[Expr],
  ) -> Option<Expr> {
    // At least one operand must be a struct: the built-in types' operators
    // cannot be overloaded (**L§7.7**).
    if !operands
      .iter()
      .any(|operand| self.is_overloadable_operand(operand.type_id))
    {
      return None;
    }
    let Some((signature, _)) = self.resolve_operator(scope, source, node, operator, operands)
    else {
      // A comparison whose `operator ==` we could not pick is still a `bool`.
      return is_comparison(operator).then(|| Expr::value(TypeId::BOOL));
    };
    let result = signature.returns.first().copied().unwrap_or(TypeId::VOID);
    Some(Expr::value(self.upcast_back(operands, result)))
  }

  /// The same by written text, for `operator []=` and `operator *[]`.
  pub(crate) fn operator_overload_named(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    text: &str,
    operands: &[Expr],
  ) -> Option<Expr> {
    let (signature, _) = self.resolve_operator_named(scope, source, node, text, operands)?;
    let result = signature.returns.first().copied().unwrap_or(TypeId::VOID);
    Some(Expr::value(self.upcast_back(operands, result)))
  }

  /// The overload an operator's operands pick, and whether it was matched with
  /// its arguments the other way round — which `#symmetric` allows for a
  /// two-parameter operator (**L§7.7**).
  pub(crate) fn resolve_operator(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    operator: OperatorType,
    operands: &[Expr],
  ) -> Option<(crate::overload::Signature, bool)> {
    self.resolve_operator_named(scope, source, node, operator.text(), operands)
  }

  /// The same by the operator's written text, for the ones that have no
  /// `Operator_Type` of their own: `operator []=` and `operator *[]`
  /// (**L§7.7**).
  pub(crate) fn resolve_operator_named(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    text: &str,
    operands: &[Expr],
  ) -> Option<(crate::overload::Signature, bool)> {
    if !operands
      .iter()
      .any(|operand| self.is_overloadable_operand(operand.type_id))
    {
      return None;
    }
    let name = self.interned().intern(text.as_bytes());
    let scope = self.nearest_scope(source, node, scope);
    let Resolution::Found(candidates) = self.program().tree().lookup(scope, name) else {
      return None;
    };

    let arguments: Vec<crate::overload::CallArgument> = operands
      .iter()
      .map(|operand| crate::overload::CallArgument::positional(operand.clone()))
      .collect();
    if let Resolved::One(signature) = self.resolve_overload(&candidates, &arguments) {
      return Some((signature, false));
    }
    if arguments.len() == 2 {
      let swapped = vec![arguments[1].clone(), arguments[0].clone()];
      if let Resolved::One(signature) = self.resolve_overload(&candidates, &swapped) {
        return Some((signature, true));
      }
    }
    // `operator *[]` and `operator []=` take a *pointer* to the thing being
    // subscripted, and the compiler takes the address of the place rather than
    // making the caller write it (**L§7.7**).
    let first = operands.first()?;
    if !first.lvalue {
      return None;
    }
    let pointer = self.types_mut().pointer_to(first.type_id);
    let mut through_address = arguments;
    through_address[0] = crate::overload::CallArgument::positional(Expr::value(pointer));
    match self.resolve_overload(&candidates, &through_address) {
      Resolved::One(signature) => Some((signature, false)),
      _ => None,
    }
  }

  /// Whether an operator of that name is declared anywhere in scope
  /// (**L§7.7**). A subscript falls back to plain pointer indexing only when
  /// nothing would rather handle it.
  pub(crate) fn has_operator(
    &self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    text: &str,
  ) -> bool {
    let name = self.interned().intern(text.as_bytes());
    let scope = self.nearest_scope(source, node, scope);
    matches!(
      self.program().tree().lookup(scope, name),
      Resolution::Found(_)
    )
  }

  /// A call that downcast an `isa` operand to its base and returned the base
  /// gives the variant back (**L§3.11**).
  fn upcast_back(&self, operands: &[Expr], result: TypeId) -> TypeId {
    for operand in operands {
      if let TypeKind::Variant(definition) = *self.types().kind(operand.type_id) {
        let info = self.types().variant_info(definition);
        if info.is_isa() && info.base == result {
          return operand.type_id;
        }
      }
    }
    result
  }

  /// Whether an operand can carry an operator overload: a struct, a variant of
  /// one, or a pointer to one (**L§7.7**).
  fn is_overloadable_operand(&self, type_id: TypeId) -> bool {
    let value = self.types().pointee(type_id).unwrap_or(type_id);
    matches!(
      self.types().kind(self.types().underlying(value)),
      TypeKind::Struct(_)
    )
  }

  /// The scope an expression was written in, found through the recorded scope
  /// of an identifier under it. Only identifiers have one of their own, and an
  /// operator's overload set is looked up like any other name. The leftmost
  /// spine answers for almost everything; the right one is what `.{3, 2, 1} +
  /// w` needs, since a bare `.{…}` names nothing at all.
  pub(crate) fn nearest_scope(&self, source: SourceId, node: NodeId, fallback: ScopeId) -> ScopeId {
    self
      .spine_scope(source, node, true)
      .or_else(|| self.spine_scope(source, node, false))
      .unwrap_or(fallback)
  }

  fn spine_scope(&self, source: SourceId, node: NodeId, leftmost: bool) -> Option<ScopeId> {
    let mut current = node;
    for _ in 0..MAX_SPINE {
      if let Some(scope) = self.scope_of(source, current) {
        return Some(scope);
      }
      let ast = self.ast(source)?;
      current = match ast.data(current) {
        NodeData::BinaryOperator { left, right, .. } => match leftmost {
          true => *left,
          false => *right,
        },
        NodeData::UnaryOperator { operand, .. } => *operand,
        NodeData::Cast(cast) => cast.expression,
        NodeData::ProcedureCall(call) => call.procedure_expression,
        NodeData::TypeInstantiation(inst) => inst.type_valued_expression.or(inst.pointer_to)?,
        // A designated literal's type is written where the literal is.
        NodeData::Literal(literal) => match &literal.value {
          oj_syntax::ast::LiteralValue::Struct(structure) => structure.type_expression?,
          oj_syntax::ast::LiteralValue::Array(array) => array.element_type?,
          _ => return None,
        },
        _ => return None,
      };
    }
    None
  }
}

/// How far down an expression's leftmost spine to look for a recorded scope.
const MAX_SPINE: u32 = 16;

fn is_comparison(operator: OperatorType) -> bool {
  matches!(
    operator,
    OperatorType::IS_EQUAL
      | OperatorType::IS_NOT_EQUAL
      | OperatorType::LESS
      | OperatorType::LESS_OR_EQUAL
      | OperatorType::GREATER
      | OperatorType::GREATER_OR_EQUAL
  )
}

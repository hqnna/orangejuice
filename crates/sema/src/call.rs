use oj_diag::SourceId;
use oj_scope::ScopeId;
use oj_syntax::ast::{NodeData, NodeId};
use oj_types::TypeId;

use crate::checker::{Checker, Expr};
use crate::overload::Resolved;

impl Checker<'_> {
  /// A call's type is its first return type, or `void` when it returns nothing
  /// (**L§5.5**). Which procedure it calls is decided by overload resolution
  /// (**L§7.5**).
  pub(crate) fn call_type(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Expr {
    match self.call_returns(scope, source, node) {
      Some(returns) => match returns.first() {
        Some(first) => Expr::value(*first),
        None => Expr::value(TypeId::VOID),
      },
      None => Expr::UNKNOWN,
    }
  }

  /// Every return type of a call, which a compound declaration distributes
  /// across its names (**L§4.5**).
  pub(crate) fn call_return_types(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Vec<TypeId> {
    self.call_returns(scope, source, node).unwrap_or_default()
  }

  fn call_returns(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<Vec<TypeId>> {
    let ast = self.ast(source)?;
    let NodeData::ProcedureCall(call) = ast.data(node) else {
      return None;
    };

    let arguments = self.call_arguments(scope, source, &call.arguments);
    for entry in call.context_modification.iter().flatten() {
      self.expression_type(scope, source, *entry);
    }

    let callee = self.expression_type(scope, source, call.procedure_expression);
    // `Holder(float, 5)` instantiates a polymorphic struct, which is M7.
    if callee.denoted.is_some() {
      return None;
    }

    // Even a single candidate is scored: a call whose arguments it does not
    // accept is one whose answer we do not have, not one whose return type is
    // that candidate's (**L§7.5**).
    let resolved = if callee.overloads.is_empty() {
      let signature = self.signature_of_type(callee.type_id)?;
      match self.accepts(&signature, &arguments) {
        true => Resolved::One(signature),
        false => Resolved::None,
      }
    } else {
      self.resolve_overload(&callee.overloads.clone(), &arguments)
    };

    match resolved {
      Resolved::One(signature) => Some(signature.returns),
      Resolved::Ambiguous | Resolved::None => None,
    }
  }
}

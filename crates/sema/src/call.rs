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
    let Some(signature) = self.resolve_call(scope, source, node) else {
      return Expr::UNKNOWN;
    };
    // `#procedure_of_call f(x)` is the procedure the call would reach, not
    // what the call would produce (**L§7.10**).
    if self.returns_procedure_pointer_only(source, node) {
      return Expr::value(signature.type_id);
    }
    match signature.returns.first() {
      Some(first) => Expr::value(*first),
      None => Expr::value(TypeId::VOID),
    }
  }

  fn returns_procedure_pointer_only(&self, source: SourceId, node: NodeId) -> bool {
    let Some(ast) = self.ast(source) else {
      return false;
    };
    match ast.data(node) {
      NodeData::ProcedureCall(call) => call
        .flags
        .contains(oj_syntax::ast::CallFlags::RETURNS_PROCEDURE_POINTER_ONLY),
      _ => false,
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
    self
      .resolve_call(scope, source, node)
      .map(|signature| signature.returns)
      .unwrap_or_default()
  }

  /// The candidate a call site resolves to, with its polymorphic header
  /// already instantiated (**L§7.5**, **L§7.8**).
  fn resolve_call(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<crate::overload::Signature> {
    self.at_call_site(scope, source, node, |checker| {
      checker.resolve_call_inner(scope, source, node)
    })
  }

  fn resolve_call_inner(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<crate::overload::Signature> {
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
      Resolved::One(signature) => {
        if let Some(instance) = signature.instance {
          self.use_instance(instance);
        }
        Some(signature)
      }
      Resolved::Ambiguous | Resolved::None => None,
    }
  }
}

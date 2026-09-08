use oj_diag::SourceId;
use oj_scope::ScopeId;
use oj_syntax::ast::{NodeData, NodeId};
use oj_types::TypeId;

use crate::checker::{Checker, Expr};

impl Checker<'_> {
  /// A call's type is its first return type, or `void` when it returns nothing
  /// (**L§5.5**).
  pub(crate) fn call_type(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Expr {
    let Some(ast) = self.ast(source) else {
      return Expr::UNKNOWN;
    };
    let NodeData::ProcedureCall(call) = ast.data(node) else {
      return Expr::UNKNOWN;
    };

    for argument in &call.arguments {
      self.expression_type(scope, source, argument.expression);
    }
    for entry in call.context_modification.iter().flatten() {
      self.expression_type(scope, source, *entry);
    }

    let callee = self.expression_type(scope, source, call.procedure_expression);
    // `Holder(float, 5)` instantiates a polymorphic struct, which is M7.
    if callee.denoted.is_some() {
      return Expr::UNKNOWN;
    }
    let Some(signature) = self.types().procedure_of(callee.type_id) else {
      return Expr::UNKNOWN;
    };
    match signature.returns.first() {
      Some(first) => Expr::value(*first),
      None => Expr::value(TypeId::VOID),
    }
  }
}

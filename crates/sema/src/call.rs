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
    // `Holder(float, 5)` bakes a polymorphic struct rather than calling
    // anything (**L§8.5**).
    if let Some(baked) = self.instantiate_struct(scope, source, node) {
      return Expr::type_expression(baked);
    }
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
    if callee.denoted.is_some() {
      return None;
    }

    // A name that resolves to something with no procedure type at all is a call
    // that cannot be made: overload resolution has nothing to score, and would
    // otherwise leave the answer open the way an unresolved candidate does.
    if !callee.overloads.is_empty() {
      let candidates = callee.overloads.clone();
      let types: Vec<TypeId> = candidates
        .iter()
        .map(|id| self.decl_type(*id).value)
        .collect();
      if types.iter().all(|type_id| {
        // A pointer is left alone: what it points at may still turn out to be
        // a procedure the checker has not settled.
        !self.undecided_type(*type_id)
          && !self.types().is_pointer(*type_id)
          && self.types().procedure_of(*type_id).is_none()
      }) && let Some(first) = types.first().copied()
      {
        return self.report_not_callable(source, node, call.procedure_expression, first);
      }
    }

    // Even a single candidate is scored: a call whose arguments it does not
    // accept is one whose answer we do not have, not one whose return type is
    // that candidate's (**L§7.5**).
    let resolved = if callee.overloads.is_empty() {
      let Some(signature) = self.signature_of_annotated(scope, source, call.procedure_expression)
      else {
        return self.report_not_callable(source, node, call.procedure_expression, callee.type_id);
      };
      match self.accepts(&signature, &arguments) {
        true => Resolved::One(signature),
        false => Resolved::None,
      }
    } else {
      // A name reached through an instantiated struct is resolved under that
      // instantiation, since what its body says depends on it (**L§8.5**).
      let candidates = callee.overloads.clone();
      let previous = self.enter_instance(callee.overload_instance.or(self.current_instance));
      let resolved = self.resolve_overload(&candidates, &arguments);
      self.enter_instance(previous);
      resolved
    };

    match resolved {
      Resolved::One(signature) => {
        if let Some(instance) = signature.instance {
          self.use_instance(instance);
        }
        Some(signature)
      }
      // Every candidate was scored and none accepted these arguments, which is
      // the call site's error rather than something a later stage discovers
      // (**L§7.5**).
      Resolved::None => {
        self.report_no_overload(source, node, &arguments);
        None
      }
      // A candidate whose own type is not worked out yet leaves the answer
      // open: there is nothing to complain about until it is.
      Resolved::Ambiguous => None,
    }
  }

  /// A call whose callee is not a procedure at all (**L§7.1**). Nothing about
  /// the arguments can be judged, so what is named is the thing that was
  /// called.
  fn report_not_callable(
    &mut self,
    source: SourceId,
    node: NodeId,
    callee: NodeId,
    callee_type: TypeId,
  ) -> Option<crate::overload::Signature> {
    if self.undecided_type(callee_type) {
      return None;
    }
    let span = self.span_of(source, node)?;
    let name = self.written_name(source, callee);
    let printed = self.type_name(callee_type);
    self.report_once(
      source,
      span,
      format!("'{name}' is not a procedure: it is a '{printed}'."),
    );
    None
  }

  /// What a call no candidate accepts reports (**L§7.5**). The argument types
  /// are printed the way the reference prints them; a call one of whose
  /// arguments has no type yet is left alone, since the complaint would be
  /// about the other error rather than about this call.
  fn report_no_overload(
    &mut self,
    source: SourceId,
    node: NodeId,
    arguments: &[crate::overload::CallArgument],
  ) {
    if !self.checking_a_body() {
      return;
    }
    if arguments
      .iter()
      .any(|argument| self.unresolved_argument(argument.value.type_id))
    {
      return;
    }
    let Some(ast) = self.ast(source) else {
      return;
    };
    let NodeData::ProcedureCall(call) = ast.data(node) else {
      return;
    };
    let callee = call.procedure_expression;
    let span = ast.node(node).span;
    let name = self.written_name(source, callee);
    let given: Vec<String> = arguments
      .iter()
      .map(|argument| {
        let printed = self.type_name(self.harden(argument.value.type_id));
        match argument
          .name
          .map(|name| self.interner().resolve_lossy(name))
        {
          Some(written) => format!("{written} = {printed}"),
          None => printed,
        }
      })
      .collect();
    self.error(
      source,
      span,
      format!(
        "The arguments given to '{name}' did not match any of its overloads. \
         The arguments were: ({}).",
        given.join(", ")
      ),
    );
  }

  /// Whether an argument's type is one the front end has not worked out, in
  /// which case nothing can be concluded about the call it was written in.
  fn unresolved_argument(&mut self, type_id: TypeId) -> bool {
    self.mentions_unknown(type_id) || self.mentions_polymorph(type_id)
  }

  /// The text a callee was written as, for a diagnostic that has to name it.
  fn written_name(&mut self, source: SourceId, node: NodeId) -> String {
    let Some(ast) = self.ast(source) else {
      return String::from("this procedure");
    };
    match ast.data(node) {
      NodeData::Ident(ident) => {
        let name = ident.name;
        self.interner().resolve_lossy(name).into_owned()
      }
      // `Simp.set_scissor(…)` is named by what follows the dot (**L§5.4**).
      NodeData::BinaryOperator {
        operator: oj_syntax::ast::OperatorType::DOT,
        right,
        ..
      } => {
        let right = *right;
        self.written_name(source, right)
      }
      _ => String::from("this procedure"),
    }
  }
}

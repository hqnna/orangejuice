//! `#bake_arguments`: a procedure with some of its arguments already given
//! (**L§7.10**).
//!
//! `only_set :: #bake_arguments only_set_or_unset(target_value = true);` is a
//! new constant procedure that takes everything the original did *except* the
//! parameters the bake named. Those keep their values wherever the baked name
//! is called, so the call site fills the rest and the back end calls the
//! original.

use oj_diag::SourceId;
use oj_scope::{DeclId, ScopeId};
use oj_syntax::ast::{BakeType, NodeData, NodeId};
use oj_types::{ProcedureType, TypeId};

use crate::checker::Checker;
use crate::overload::Signature;

impl Checker<'_> {
  /// The candidate a `#bake_arguments` constant stands for: the procedure it
  /// names, with the baked parameters filled in and hidden from the call site
  /// (**L§7.10**).
  pub(crate) fn baked_signature(&mut self, candidate: DeclId) -> Option<Signature> {
    let (source, bake, scope) = self.bake_of(candidate)?;
    let NodeData::DirectiveBake {
      procedure_call,
      bake_type,
    } = self.ast(source)?.data(bake)
    else {
      return None;
    };
    let bake_type = *bake_type;
    let call = *procedure_call;
    if bake_type == BakeType::Constants {
      return self.baked_constants(source, call, scope);
    }
    if bake_type != BakeType::ParameterValue {
      return None;
    }
    let NodeData::ProcedureCall(payload) = self.ast(source)?.data(call) else {
      return None;
    };
    let payload = payload.clone();

    let scope = self.scope_at(source, payload.procedure_expression, scope);
    let target = self.expression_type(scope, source, payload.procedure_expression);
    let [only] = target.overloads[..] else {
      return None;
    };
    let mut signature = self.signature_of(only)?;

    let mut next = 0usize;
    for argument in &payload.arguments {
      let index = match argument.name.and_then(|node| self.ident_name(source, node)) {
        Some(name) => signature
          .parameters
          .iter()
          .position(|parameter| parameter.name == Some(name))?,
        None => {
          next += 1;
          next - 1
        }
      };
      let parameter = signature.parameters.get_mut(index)?;
      parameter.has_default = true;
      parameter.default = Some(argument.expression);
      parameter.default_source = Some(source);
      signature.hidden.push(index);
    }
    Some(signature)
  }

  /// `#bake_constants f(T = int)`: the instantiation those constants name
  /// (**L§7.10**).
  fn baked_constants(
    &mut self,
    source: SourceId,
    call: NodeId,
    scope: ScopeId,
  ) -> Option<Signature> {
    let NodeData::ProcedureCall(payload) = self.ast(source)?.data(call) else {
      return None;
    };
    let payload = payload.clone();
    let scope = self.scope_at(source, payload.procedure_expression, scope);
    let target = self.expression_type(scope, source, payload.procedure_expression);
    let [only] = target.overloads[..] else {
      return None;
    };
    let signature = self.signature_of(only)?;
    let (header_source, header) = signature.header?;
    let scopes = self.program().procedure_scopes(header_source, header)?;
    let outer_scope = self.program().tree().decl(only).scope;

    // The arguments name the header's own constants, in any order.
    let variables = self.program().tree().declarations(scopes.constants);
    let mut bindings = Vec::with_capacity(payload.arguments.len());
    let mut next = 0usize;
    for argument in &payload.arguments {
      let index = match argument.name.and_then(|node| self.ident_name(source, node)) {
        Some(name) => variables
          .iter()
          .position(|id| self.program().tree().decl(*id).name == name)?,
        None => {
          next += 1;
          next - 1
        }
      };
      let value = self
        .expression_type(scope, source, argument.expression)
        .constant?;
      bindings.push((*variables.get(index)?, value));
    }
    bindings.sort_by_key(|(id, _)| *id);

    let solution = crate::poly::Solution {
      bindings,
      ..Default::default()
    };
    let instance = self.finish_instantiation(
      &signature,
      header_source,
      header,
      scopes,
      outer_scope,
      solution,
    )?;
    self.use_instance(instance);
    let type_id = self.instance_type(instance);
    let procedure = self.types().procedure_of(type_id)?.clone();
    let parameters = self.with_instance(Some(instance), |checker| {
      checker.header_parameters(header_source, header, &procedure.arguments)
    });
    Some(Signature {
      parameters,
      returns: procedure.returns.clone(),
      vararg_slot: procedure.vararg_index.map(|index| index as usize),
      hidden: Vec::new(),
      polymorphic: false,
      is_macro: signature.is_macro,
      decl: signature.decl,
      header: signature.header,
      type_id,
      instance: Some(instance),
    })
  }

  /// The type a `#bake_arguments` constant has: the original's, without the
  /// parameters the bake gave values to (**L§7.10**).
  pub(crate) fn baked_type(&mut self, candidate: DeclId) -> TypeId {
    let Some(signature) = self.baked_signature(candidate) else {
      return TypeId::UNKNOWN;
    };
    // A constants bake hides nothing: the specialization is the type.
    if signature.hidden.is_empty() {
      return signature.type_id;
    }
    let Some(original) = self.types().procedure_of(signature.type_id).cloned() else {
      return TypeId::UNKNOWN;
    };
    let arguments = original
      .arguments
      .iter()
      .enumerate()
      .filter(|(index, _)| !signature.hidden.contains(index))
      .map(|(_, type_id)| *type_id)
      .collect();
    let mut reduced = ProcedureType::new(arguments, original.returns.clone());
    reduced.flags = original.flags;
    self.types_mut().procedure(reduced)
  }

  /// The `#bake_arguments` a declaration's value is, and the scope it was
  /// written in.
  fn bake_of(&mut self, candidate: DeclId) -> Option<(SourceId, NodeId, ScopeId)> {
    let decl = self.program().tree().decl(candidate);
    let (node, source) = (decl.node?, decl.source?);
    let NodeData::Declaration(declaration) = self.ast(source)?.data(node) else {
      return None;
    };
    let expression = declaration.expression?;
    matches!(
      self.ast(source)?.data(expression),
      NodeData::DirectiveBake { .. }
    )
    .then_some((source, expression, decl.scope))
  }
}

//! Instantiating a polymorphic procedure (**L§7.8**).
//!
//! A header with `$` markers is a family rather than a procedure. A call site
//! *solves* it: the argument types are matched against the parameter types, the
//! `$T`s that turn up bind to concrete types and the `$x`s to constant values,
//! and the header is then re-read with those bindings in place, which gives the
//! specialization its own procedure type. Identical constant sets share one
//! instantiation, program-wide.

use std::collections::HashMap;

use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclId, ProcedureScopes, ScopeId};
use oj_syntax::ast::{DeclarationFlags, NodeData, NodeId};
use oj_types::{ArrayKind, PolymorphId, TypeId, TypeKind};

use crate::checker::{Checker, Expr};
use crate::constants::{Const, Value};
use crate::overload::CallArgument;

/// One instantiation of a polymorphic procedure (**L§7.8**).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceId(pub u32);

/// What makes two instantiations the same one: the same header, bound to the
/// same constants (**L§7.8**).
pub(crate) type InstanceKey = (SourceId, NodeId, Vec<(DeclId, TypeId, ConstKey)>);

/// A [`Const`] projected onto something hashable, so that instantiations can be
/// looked up by the values they bind.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ConstKey {
  Int(i128),
  Float(u64),
  Bool(bool),
  Bytes(Box<[u8]>),
  Null,
  Type(TypeId),
  Name(Symbol),
}

pub(crate) fn const_key(value: &Const) -> ConstKey {
  match &value.value {
    Value::Int(value) => ConstKey::Int(*value),
    Value::Float(value) => ConstKey::Float(value.to_bits()),
    Value::Bool(value) => ConstKey::Bool(*value),
    Value::String(text) | Value::Bytes(text) => ConstKey::Bytes(text.clone()),
    Value::Null => ConstKey::Null,
    Value::Type(id) => ConstKey::Type(*id),
    Value::EnumName(name) => ConstKey::Name(*name),
  }
}

#[derive(Clone, Debug)]
pub(crate) struct Instance {
  /// The declaration the header was written at, when it has one.
  pub decl: Option<DeclId>,
  pub source: SourceId,
  pub header: NodeId,
  pub scopes: ProcedureScopes,
  /// The scope the header was written in, which is where its own names resolve.
  pub outer_scope: ScopeId,
  /// The constants this specialization binds, by the declaration each names.
  pub bindings: Vec<(DeclId, Const)>,
  /// The procedure type the bindings produce: the header with every `$T`
  /// replaced.
  pub type_id: TypeId,
  /// The instantiation this one was created inside, when the header is nested
  /// in another polymorphic body.
  pub parent: Option<InstanceId>,
}

impl Instance {
  /// The scope everything belonging to the instantiation hangs under.
  pub fn body_root(&self) -> ScopeId {
    self.scopes.constants
  }
}

/// The bindings a call site solved, before they become an instantiation.
#[derive(Clone, Debug, Default)]
pub(crate) struct Solution {
  pub bindings: Vec<(DeclId, Const)>,
}

impl Checker<'_> {
  /// The value the active instantiation gave a declaration, if it bound one
  /// (**L§7.8**).
  pub(crate) fn bound_constant(&self, id: DeclId) -> Option<Const> {
    let mut current = self.current_instance;
    while let Some(instance) = current {
      let instance = self.instance(instance);
      if let Some((_, value)) = instance.bindings.iter().find(|(bound, _)| *bound == id) {
        return Some(value.clone());
      }
      current = instance.parent;
    }
    None
  }

  /// Runs `body` with `instance` as the active instantiation.
  pub(crate) fn with_instance<T>(
    &mut self,
    instance: Option<InstanceId>,
    body: impl FnOnce(&mut Self) -> T,
  ) -> T {
    let previous = std::mem::replace(&mut self.current_instance, instance);
    let result = body(self);
    self.current_instance = previous;
    result
  }

  pub(crate) fn instance_type(&self, id: InstanceId) -> TypeId {
    self.instance(id).type_id
  }

  /// The candidate a polymorphic header becomes for one call site's arguments:
  /// the instantiation they solve, read back as an ordinary signature
  /// (**L§7.8**).
  pub(crate) fn specialize(
    &mut self,
    signature: &crate::overload::Signature,
    arguments: &[CallArgument],
  ) -> Option<crate::overload::Signature> {
    let instance = self.instantiate(signature, arguments)?;
    let (source, header) = signature.header?;
    let type_id = self.instance_type(instance);
    let procedure = self.types().procedure_of(type_id)?.clone();
    let parameters = self.with_instance(Some(instance), |checker| {
      checker.header_parameters(source, header, &procedure.arguments)
    });
    Some(crate::overload::Signature {
      parameters,
      returns: procedure.returns.clone(),
      varargs: procedure.varargs,
      polymorphic: false,
      decl: signature.decl,
      header: signature.header,
      type_id,
      instance: Some(instance),
    })
  }

  /// Records that a call site really does call an instantiation, so that its
  /// body is typechecked. Solving happens for every candidate an overload set
  /// offers; only the winner is real.
  pub(crate) fn use_instance(&mut self, id: InstanceId) {
    if !self.pending_instances.contains(&id) && !self.checked_instances.contains(&id) {
      self.pending_instances.push(id);
    }
  }

  /// Typechecks the body of every instantiation a call site chose, and of the
  /// ones those bodies go on to choose (**L§7.8**).
  pub(crate) fn check_instances(&mut self) {
    while let Some(id) = self.pending_instances.pop() {
      if !self.checked_instances.insert(id) {
        continue;
      }
      self.check_instance(id);
    }
  }

  fn check_instance(&mut self, id: InstanceId) {
    let instance = self.instance(id).clone();
    let source = instance.source;
    let Some(ast) = self.ast(source) else { return };
    let NodeData::ProcedureHeader(payload) = ast.data(instance.header) else {
      return;
    };
    let Some(body) = payload.body_or_null else {
      return;
    };
    let NodeData::ProcedureBody { block, .. } = ast.data(body) else {
      return;
    };
    let block = *block;
    let Some(returns) = self
      .types()
      .procedure_of(instance.type_id)
      .map(|procedure| procedure.returns.clone())
    else {
      return;
    };
    self.with_instance(Some(id), |checker| {
      checker.check_procedure_body(source, instance.outer_scope, block, returns);
    });
  }

  /// Binds the polymorph variables a header declares from the types and values
  /// a call site passes, and returns the instantiation they name (**L§7.8**).
  /// `None` when something is left unsolved, which is a candidate the call site
  /// does not match rather than an error.
  pub(crate) fn instantiate(
    &mut self,
    signature: &crate::overload::Signature,
    arguments: &[CallArgument],
  ) -> Option<InstanceId> {
    let (source, header) = signature.header?;
    let scopes = self.program().procedure_scopes(source, header)?;
    let outer_scope = signature
      .decl
      .map(|decl| self.program().tree().decl(decl).scope)
      .or_else(|| self.program().tree().scope(scopes.constants).parent)?;

    let solution = self.solve(signature, arguments, source, header, scopes)?;

    let key: InstanceKey = (
      source,
      header,
      solution
        .bindings
        .iter()
        .map(|(id, value)| (*id, value.type_id, const_key(value)))
        .collect(),
    );
    if let Some(existing) = self.instance_cache.get(&key) {
      return Some(*existing);
    }

    let id = InstanceId(self.instances.len() as u32);
    self.instances.push(Instance {
      decl: signature.decl,
      source,
      header,
      scopes,
      outer_scope,
      bindings: solution.bindings,
      type_id: TypeId::UNKNOWN,
      parent: self.current_instance,
    });
    self.instance_cache.insert(key, id);

    // The specialization's own type is the header read again with the
    // bindings in place, which is also what turns `$T` into a real type in
    // the parameter and return slots.
    let type_id = self.with_instance(Some(id), |checker| {
      checker.procedure_type(source, header, outer_scope)
    });
    self.instances[id.0 as usize].type_id = type_id;
    if self.types().procedure_of(type_id).is_some_and(|signature| {
      signature
        .arguments
        .iter()
        .chain(&signature.returns)
        .any(|argument| self.is_polymorphic_type(*argument))
    }) {
      return None;
    }
    Some(id)
  }

  /// Works out what each polymorph variable of a header has to be for a call
  /// site's arguments to fit (**L§7.8**).
  fn solve(
    &mut self,
    signature: &crate::overload::Signature,
    arguments: &[CallArgument],
    source: SourceId,
    header: NodeId,
    scopes: ProcedureScopes,
  ) -> Option<Solution> {
    let mut substitution: HashMap<PolymorphId, TypeId> = HashMap::new();
    let mut solution = Solution::default();

    let count = signature.parameters.len();
    let vararg_slot = signature.varargs.then(|| count.saturating_sub(1));
    let mut next = 0usize;
    for argument in arguments {
      let index = match argument.name {
        Some(name) => signature
          .parameters
          .iter()
          .position(|parameter| parameter.name == Some(name))?,
        None => {
          let index = next;
          next += 1;
          index
        }
      };
      let index = match vararg_slot {
        Some(slot) if index >= slot => slot,
        _ => index,
      };
      let parameter = signature.parameters.get(index)?;
      // An argument in the varargs slot matches the `[] T`'s element
      // (**L§7.8**): `values: ..$T` takes `T` from the first one. `..xs`
      // hands over the whole array instead.
      let target = match vararg_slot == Some(index) && !argument.spread {
        true => self
          .types()
          .array_of(parameter.type_id)
          .map_or(parameter.type_id, |(element, _)| element),
        false => parameter.type_id,
      };
      if !self.unify_polymorph(target, &argument.value, &mut substitution) {
        return None;
      }
      // A `$x` parameter is a constant of the instantiation, so the argument
      // has to be one (**L§7.8**).
      if let Some(decl) = self.baked_parameter_decl(source, signature, index) {
        let value = argument.value.constant.clone()?;
        solution.bindings.push((decl, value));
      }
    }

    // Every `$T` the header declares has to have come out of that.
    for id in self
      .program()
      .tree()
      .scope(scopes.constants)
      .declarations
      .clone()
    {
      if solution.bindings.iter().any(|(bound, _)| *bound == id) {
        continue;
      }
      let decl = self.program().tree().decl(id).clone();
      let node = decl.node?;
      let variable = self.aggregate_type(decl.source.unwrap_or(source), node)?;
      let TypeKind::Polymorph(definition) = *self.types().kind(variable) else {
        continue;
      };
      let bound = substitution.get(&definition).copied()?;
      solution
        .bindings
        .push((id, Const::new(TypeId::TYPE, Value::Type(bound))));
    }

    let _ = header;
    solution.bindings.sort_by_key(|(id, _)| *id);
    Some(solution)
  }

  /// The declaration of a `$x` parameter, whose *value* the instantiation
  /// bakes rather than its type (**L§7.8**).
  fn baked_parameter_decl(
    &mut self,
    source: SourceId,
    signature: &crate::overload::Signature,
    index: usize,
  ) -> Option<DeclId> {
    let (_, header) = signature.header?;
    let NodeData::ProcedureHeader(payload) = self.ast(source)?.data(header) else {
      return None;
    };
    let parameter = *payload.arguments.get(index)?;
    let NodeData::Declaration(declaration) = self.ast(source)?.data(parameter) else {
      return None;
    };
    if !declaration
      .flags
      .intersects(DeclarationFlags::AUTO_VALUE_BAKE | DeclarationFlags::AUTO_VALUE_BAKE_IS_REQUIRED)
    {
      return None;
    }
    self.decl_at(source, parameter)
  }

  /// Matches a parameter type against the type of what a call site passes,
  /// binding the polymorph variables it meets on the way (**L§7.8**).
  fn unify_polymorph(
    &mut self,
    pattern: TypeId,
    value: &Expr,
    substitution: &mut HashMap<PolymorphId, TypeId>,
  ) -> bool {
    if !self.is_polymorphic_type(pattern) {
      return true;
    }
    // `null` names no type, so it never decides a `$T` (**L§7.8**).
    if matches!(
      value.constant.as_ref().map(|value| &value.value),
      Some(Value::Null)
    ) {
      return true;
    }
    let actual = self.harden(value.type_id);
    self.unify_types(pattern, actual, substitution)
  }

  fn unify_types(
    &mut self,
    pattern: TypeId,
    actual: TypeId,
    substitution: &mut HashMap<PolymorphId, TypeId>,
  ) -> bool {
    if self.types().is_unknown(actual) {
      return false;
    }
    let kinds = (
      self.types().kind(pattern).clone(),
      self.types().kind(actual).clone(),
    );
    match kinds {
      (TypeKind::Polymorph(definition), _) => match substitution.get(&definition) {
        Some(bound) => *bound == actual,
        None => {
          substitution.insert(definition, actual);
          true
        }
      },
      (TypeKind::Pointer(pattern), TypeKind::Pointer(actual)) => {
        self.unify_types(pattern, actual, substitution)
      }
      (
        TypeKind::Array {
          element: pattern,
          kind: want,
        },
        TypeKind::Array {
          element: actual,
          kind: got,
        },
      ) => {
        // A fixed array converts to a view, so `[] $T` matches one; `[$N] T`
        // only matches a fixed array of the same length (**L§7.8**).
        let compatible = match (want, got) {
          (ArrayKind::View, _) => true,
          (want, got) => want == got,
        };
        compatible && self.unify_types(pattern, actual, substitution)
      }
      (TypeKind::Procedure(pattern), TypeKind::Procedure(actual)) => {
        if pattern.arguments.len() != actual.arguments.len()
          || pattern.returns.len() != actual.returns.len()
        {
          return false;
        }
        pattern
          .arguments
          .iter()
          .zip(actual.arguments.iter())
          .chain(pattern.returns.iter().zip(actual.returns.iter()))
          .collect::<Vec<_>>()
          .into_iter()
          .all(|(pattern, actual)| self.unify_types(*pattern, *actual, substitution))
      }
      _ => false,
    }
  }
}

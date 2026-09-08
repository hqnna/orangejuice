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

/// What makes two instantiations the same one: the same header bound to the
/// same constants (**L§7.8**). A macro is not shared at all — it expands once
/// per call site, into that site's block (**L§7.13**) — so its key carries the
/// site and the instantiation the site was written in.
pub(crate) type InstanceKey = (
  SourceId,
  NodeId,
  Option<(SourceId, NodeId, Option<InstanceId>)>,
  Vec<(DeclId, TypeId, ConstKey)>,
);

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
  Code(SourceId, NodeId),
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
    Value::Code { source, node, .. } => ConstKey::Code(*source, *node),
  }
}

#[derive(Clone, Debug)]
pub(crate) struct Instance {
  /// The declaration the header was written at, when it has one.
  pub decl: Option<DeclId>,
  pub source: SourceId,
  /// The header, or the `struct` node for an instantiated polymorphic struct.
  pub header: NodeId,
  /// The scope everything belonging to the instantiation hangs under: a
  /// procedure's constants block, or a struct's argument list (**L§8.5**).
  pub root: ScopeId,
  /// The scope the header was written in, which is where its own names resolve.
  pub outer_scope: ScopeId,
  /// The constants this specialization binds, by the declaration each names.
  pub bindings: Vec<(DeclId, Const)>,
  /// The type a parameter has in this specialization when the header could not
  /// say it: `(holder: Holder)` over a polymorphic struct means whichever
  /// instantiation the call passed (**L§7.8**).
  pub overrides: Vec<(DeclId, TypeId)>,
  /// The procedure type the bindings produce: the header with every `$T`
  /// replaced.
  pub type_id: TypeId,
  /// The instantiation this one was created inside, when the header is nested
  /// in another polymorphic body.
  pub parent: Option<InstanceId>,
  /// Where a macro expands, when the header is one: the call site and the
  /// scope it was written in, which is the scope the macro's body can also see
  /// names in (**L§7.13**).
  pub expansion: Option<Expansion>,
}

/// Where a macro was invoked (**L§7.13**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expansion {
  pub source: SourceId,
  pub node: NodeId,
  /// The scope the call was written in: a name the macro's own scopes do not
  /// hold is looked up here, and a backticked one always is.
  pub caller_scope: ScopeId,
}

impl Instance {
  pub fn body_root(&self) -> ScopeId {
    self.root
  }
}

/// The bindings a call site solved, before they become an instantiation.
#[derive(Clone, Debug, Default)]
pub(crate) struct Solution {
  pub bindings: Vec<(DeclId, Const)>,
  pub overrides: Vec<(DeclId, TypeId)>,
}

impl Checker<'_> {
  /// Where the innermost active macro was invoked (**L§7.13**), which is what
  /// `#caller_code` names.
  pub(crate) fn expansion_site(&self) -> Option<Expansion> {
    let mut current = self.current_instance;
    while let Some(id) = current {
      let instance = self.instance(id);
      if let Some(expansion) = instance.expansion {
        return Some(expansion);
      }
      current = instance.parent;
    }
    None
  }

  /// The scope a name that the macro's own scopes do not hold falls back to:
  /// the block the innermost active macro expanded into (**L§7.13**).
  pub(crate) fn caller_scope(&self) -> Option<ScopeId> {
    let mut current = self.current_instance;
    while let Some(id) = current {
      let instance = self.instance(id);
      if let Some(expansion) = instance.expansion {
        return Some(expansion.caller_scope);
      }
      current = instance.parent;
    }
    None
  }

  /// The type the active instantiation gave a parameter the header could not
  /// name on its own (**L§7.8**).
  pub(crate) fn bound_parameter_type(&self, id: DeclId) -> Option<TypeId> {
    let mut current = self.current_instance;
    while let Some(instance) = current {
      let instance = self.instance(instance);
      if let Some((_, type_id)) = instance.overrides.iter().find(|(bound, _)| *bound == id) {
        return Some(*type_id);
      }
      current = instance.parent;
    }
    None
  }

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

  /// Runs `body` with `site` as the call being resolved, which is where a
  /// macro reached from it expands (**L§7.13**).
  pub(crate) fn at_call_site<T>(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    body: impl FnOnce(&mut Self) -> T,
  ) -> T {
    let previous = self.call_site.replace(Expansion {
      source,
      node,
      // A macro's body sees the block the call was written in, which is not
      // always the scope the walk was handed (**L§7.13**).
      caller_scope: self.nearest_scope(source, node, scope),
    });
    let result = body(self);
    self.call_site = previous;
    result
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

  /// `Holder(float, 5)` bakes a polymorphic struct down to a type of its own
  /// (**L§8.5**). `None` when the call names something else.
  pub(crate) fn instantiate_struct(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<TypeId> {
    let NodeData::ProcedureCall(call) = self.ast(source)?.data(node) else {
      return None;
    };
    let call = call.clone();
    let family = self
      .expression_type(scope, source, call.procedure_expression)
      .denoted?;
    let definition = self.types().struct_of(family)?;
    if !self
      .types()
      .struct_info(definition)
      .nontextual_flags
      .contains(oj_types::StructNontextualFlags::POLYMORPHIC)
    {
      return None;
    }
    let members = self.struct_scope(definition)?;
    let (body_source, body) = self.aggregate_owner(members)?;
    let arguments_scope = self.program().tree().scope(members).parent?;
    let outer_scope = self.program().tree().scope(arguments_scope).parent?;

    // Every argument has to be a constant, since the members are laid out
    // against them (**L§8.5**).
    let parameters = self
      .program()
      .tree()
      .scope(arguments_scope)
      .declarations
      .clone();
    // Struct arguments may be given by name, in any order, and two
    // instantiations that agree on them are one type (**L§8.5**).
    let mut given: Vec<Option<NodeId>> = vec![None; parameters.len()];
    let mut next = 0usize;
    for argument in &call.arguments {
      let index = match argument.name.and_then(|node| self.ident_name(source, node)) {
        Some(name) => parameters
          .iter()
          .position(|id| self.program().tree().decl(*id).name == name)?,
        None => {
          next += 1;
          next - 1
        }
      };
      *given.get_mut(index)? = Some(argument.expression);
    }
    let mut bindings = Vec::with_capacity(parameters.len());
    for (parameter, expression) in parameters.iter().zip(given) {
      let declared = self.decl_type(*parameter).value;
      let value = match expression {
        Some(expression) => self.expression_type(scope, source, expression).constant?,
        // A parameter the instantiation left out takes its default.
        None => self.decl_constant(*parameter)?,
      };
      let value = match self.types().is_unknown(declared) {
        true => value,
        false => value.convert(self.types(), declared).unwrap_or(value),
      };
      bindings.push((*parameter, value));
    }

    let key: InstanceKey = (
      body_source,
      body,
      None,
      bindings
        .iter()
        .map(|(id, value)| (*id, value.type_id, const_key(value)))
        .collect(),
    );
    let instance = match self.instance_cache.get(&key) {
      Some(existing) => *existing,
      None => {
        let id = InstanceId(self.instances.len() as u32);
        self.instances.push(Instance {
          decl: None,
          source: body_source,
          header: body,
          root: arguments_scope,
          outer_scope,
          bindings,
          overrides: Vec::new(),
          type_id: TypeId::UNKNOWN,
          parent: self.current_instance,
          expansion: None,
        });
        self.instance_cache.insert(key, id);
        id
      }
    };

    // The specialization is printed the way it was written, `Holder(float, 5)`
    // (**L§8.5**), whichever order the arguments were given in.
    let name = self.instantiation_name(family, instance);
    let baked = self.with_instance(Some(instance), |checker| {
      checker.build_struct(None, name, body_source, body, outer_scope)
    });
    self.instances[instance.0 as usize].type_id = baked;
    if let Some(baked) = self.types().struct_of(baked) {
      self.types_mut().struct_info_mut(baked).polymorph_source = Some(definition);
    }
    Some(baked)
  }

  /// `Holder(float, 5)`: the family's name with the arguments it was baked
  /// with (**L§8.5**).
  fn instantiation_name(&mut self, family: TypeId, instance: InstanceId) -> Option<Symbol> {
    let base = self.types().declared_name(family)?;
    let mut text = self.symbol_text(base);
    text.push('(');
    let bindings = self.instance(instance).bindings.clone();
    for (index, (_, value)) in bindings.iter().enumerate() {
      if index > 0 {
        text.push_str(", ");
      }
      match value.as_type() {
        Some(type_id) => text.push_str(&self.type_name(type_id)),
        None => match &value.value {
          Value::Int(number) => text.push_str(&number.to_string()),
          Value::Float(number) => text.push_str(&number.to_string()),
          Value::Bool(value) => text.push_str(if *value { "true" } else { "false" }),
          Value::String(bytes) => {
            text.push('"');
            text.push_str(&String::from_utf8_lossy(bytes));
            text.push('"');
          }
          _ => text.push('?'),
        },
      }
    }
    text.push(')');
    Some(self.interned().intern(text.as_bytes()))
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
      vararg_slot: signature.vararg_slot,
      hidden: signature.hidden.clone(),
      polymorphic: false,
      is_macro: signature.is_macro,
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
    // `#modify` decides the candidate and may change what its variables are,
    // and deduplication happens after it (**L§7.8**).
    let bindings = self.run_modify(
      source,
      header,
      scopes,
      outer_scope,
      solution.bindings.clone(),
    )?;
    let solution = Solution {
      bindings,
      ..solution
    };

    // A macro is expanded into the block it was called from rather than
    // called, so two sites never share one (**L§7.13**).
    let expansion = signature.is_macro.then_some(self.call_site).flatten();
    let key: InstanceKey = (
      source,
      header,
      expansion.map(|site| (site.source, site.node, self.current_instance)),
      solution
        .bindings
        .iter()
        .map(|(id, value)| (*id, value.type_id, const_key(value)))
        .collect(),
    );
    if let Some(existing) = self.instance_cache.get(&key) {
      return Some(*existing);
    }
    if signature.is_macro && expansion.is_none() {
      return None;
    }

    let id = InstanceId(self.instances.len() as u32);
    self.instances.push(Instance {
      decl: signature.decl,
      source,
      header,
      root: scopes.constants,
      outer_scope,
      bindings: solution.bindings,
      overrides: solution.overrides,
      type_id: TypeId::UNKNOWN,
      parent: self.current_instance,
      expansion,
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

    let vararg_slot = signature.vararg_slot;
    let slots = self.argument_slots(signature, arguments)?;
    // A literal argument is whatever the header wants, so it only decides a
    // `$T` that nothing typed did: `compare_and_swap(*lock, 0, 1)` takes `T`
    // from the pointer and converts the two literals to it (**L§7.8**).
    let typed_first = slots
      .iter()
      .copied()
      .zip(arguments)
      .filter(|(_, argument)| !self.types().is_untyped(argument.value.type_id))
      .chain(
        slots
          .iter()
          .copied()
          .zip(arguments)
          .filter(|(_, argument)| self.types().is_untyped(argument.value.type_id)),
      )
      .map(|(index, argument)| (index, argument.clone()))
      .collect::<Vec<_>>();
    for (index, argument) in typed_first {
      let argument = &argument;
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
      // A parameter written as a bare polymorphic struct takes whichever
      // instantiation the call passed (**L§7.8**).
      if let Some(concrete) = self.instantiation_passed(target, argument.value.type_id)
        && let Some(decl) = self.header_parameter_decl(source, signature, index)
      {
        solution.overrides.push((decl, concrete));
      } else if !self.unify_polymorph(target, &argument.value, &mut substitution) {
        return None;
      }
      // A `$x` parameter is a constant of the instantiation, so the argument
      // has to be one (**L§7.8**).
      if let Some(decl) = self.baked_parameter_decl(source, signature, index) {
        let value = argument.value.constant.clone()?;
        solution.bindings.push((decl, value));
        continue;
      }
      // A macro's `Code` parameter is the argument itself, unevaluated
      // (**L§13.1**); anything else it was handed a constant for is a constant
      // of the expansion, which is what lets `#assert` and `#if` read it.
      if signature.is_macro
        && let Some(decl) = self.header_parameter_decl(source, signature, index)
      {
        if parameter.type_id == TypeId::CODE
          && let Some((written_source, node, written_scope)) = argument.written
        {
          solution.bindings.push((
            decl,
            Const::new(
              TypeId::CODE,
              Value::Code {
                source: written_source,
                node,
                scope: written_scope,
              },
            ),
          ));
          continue;
        }
        if let Some(value) = argument.value.constant.clone() {
          solution.bindings.push((decl, value));
        }
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

    // A macro's `Code` parameter the call site left out takes its default,
    // which is how `call := #caller_code` reaches the body (**L§7.13**).
    if signature.is_macro {
      for (index, parameter) in signature.parameters.iter().enumerate() {
        if parameter.type_id != TypeId::CODE || slots.contains(&index) {
          continue;
        }
        let (Some(default), Some(decl)) = (
          parameter.default,
          self.header_parameter_decl(source, signature, index),
        ) else {
          continue;
        };
        let scope = self.scope_at(source, default, scopes.arguments);
        if let Some(value) = self.expression_type(scope, source, default).constant {
          solution.bindings.push((decl, value));
        }
      }
    }

    // A parameter whose type mentions a variable that no constants block
    // declares — a quick lambda's, which are all `$`-inferred (**L§7.9**) —
    // takes the solved type directly.
    for (index, parameter) in signature.parameters.iter().enumerate() {
      let solved = self.substitute(parameter.type_id, &substitution);
      if solved == parameter.type_id {
        continue;
      }
      if let Some(decl) = self.header_parameter_decl(source, signature, index)
        && !solution.overrides.iter().any(|(bound, _)| *bound == decl)
      {
        solution.overrides.push((decl, solved));
      }
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

  /// Whether `actual` is an instantiation of the polymorphic struct `pattern`
  /// names, in which case the parameter is of *that* type (**L§7.8**). A
  /// pointer to one counts, at the same level of indirection.
  fn instantiation_passed(&self, pattern: TypeId, actual: TypeId) -> Option<TypeId> {
    let (family, concrete, pointer) = match self.types().pointee(pattern) {
      Some(family) => (family, self.types().pointee(actual)?, true),
      None => (pattern, actual, false),
    };
    let family = self.types().struct_of(self.types().underlying(family))?;
    if !self
      .types()
      .struct_info(family)
      .nontextual_flags
      .contains(oj_types::StructNontextualFlags::POLYMORPHIC)
    {
      return None;
    }
    let baked = self.types().struct_of(self.types().underlying(concrete))?;
    if self.types().struct_info(baked).polymorph_source != Some(family) {
      return None;
    }
    Some(match pointer {
      true => actual,
      false => concrete,
    })
  }

  /// The declaration one parameter of a header introduced.
  fn header_parameter_decl(
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
    let parameter = match self.ast(source)?.data(parameter) {
      NodeData::Using(using) => using.expression,
      _ => parameter,
    };
    self.decl_at(source, parameter)
  }

  /// The type with every polymorph variable the solution decided replaced by
  /// what it decided (**L§7.8**).
  fn substitute(&mut self, type_id: TypeId, substitution: &HashMap<PolymorphId, TypeId>) -> TypeId {
    match self.types().kind(type_id).clone() {
      TypeKind::Polymorph(definition) => substitution.get(&definition).copied().unwrap_or(type_id),
      TypeKind::Pointer(pointee) => {
        let pointee = self.substitute(pointee, substitution);
        self.types_mut().pointer_to(pointee)
      }
      TypeKind::Array { element, kind } => {
        let element = self.substitute(element, substitution);
        self.types_mut().array(element, kind)
      }
      _ => type_id,
    }
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
      // A variable something else already decided is not decided again: the
      // argument only has to convert to it, which scoring checks
      // (**L§7.8**).
      (TypeKind::Polymorph(definition), _) => {
        substitution.entry(definition).or_insert(actual);
        true
      }
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

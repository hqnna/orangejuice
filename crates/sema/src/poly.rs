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
  // The last is the types the call gave parameters the header could not name:
  // a header whose only variable is a parameter typed by a polymorphic struct
  // *family* binds no constants at all, and two tables are still two
  // specializations (**L§7.8**, **L§8.5**).
  Vec<(DeclId, TypeId)>,
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
  Procedure(DeclId),
}

pub(crate) fn const_key(value: &Const) -> ConstKey {
  match &value.value {
    Value::Int(value) => ConstKey::Int(*value),
    Value::Float(value) => ConstKey::Float(value.to_bits()),
    Value::Bool(value) => ConstKey::Bool(*value),
    Value::String(text) => ConstKey::Bytes(text.clone()),
    Value::Bytes(bytes) => ConstKey::Bytes(bytes.data.clone()),
    Value::Null => ConstKey::Null,
    Value::Type(id) => ConstKey::Type(*id),
    Value::EnumName(name) => ConstKey::Name(*name),
    Value::Code { source, node, .. } => ConstKey::Code(*source, *node),
    Value::Procedure(decl) => ConstKey::Procedure(*decl),
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
  /// What unification decided each variable is. A parameter whose type is a
  /// variable no constants block declares — a quick lambda's, which are all
  /// `$`-inferred (**L§7.9**) — takes its type from here, once `#modify` has
  /// had its say about the ones that are declared.
  pub substitution: HashMap<PolymorphId, TypeId>,
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
    let arguments_scope = self.program().tree().parent(members)?;
    let parameters = self.struct_parameters(arguments_scope);
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
    self.bake_struct(family, definition, &given, scope, source)
  }

  /// A polymorphic struct named without an argument list at all: it bakes with
  /// every parameter's default, which is what `a: A_Struct;` means for a family
  /// whose parameters all have one (**L§8.5**). A parameter written as a bare
  /// family is a *pattern* rather than a type, so it never comes through here.
  pub(crate) fn bake_family_defaults(&mut self, family: TypeId) -> Option<TypeId> {
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
    let arguments_scope = self.program().tree().parent(members)?;
    let count = self.program().tree().declarations(arguments_scope).len();
    let (source, _) = self.aggregate_owner(members)?;
    self.bake_struct(
      family,
      definition,
      &vec![None; count],
      arguments_scope,
      source,
    )
  }

  /// Bakes one instantiation of a polymorphic struct: `given` holds the
  /// argument each parameter was written with, in declaration order, and the
  /// ones left out take their defaults (**L§8.5**).
  fn bake_struct(
    &mut self,
    family: TypeId,
    definition: oj_types::StructId,
    given: &[Option<NodeId>],
    scope: ScopeId,
    source: SourceId,
  ) -> Option<TypeId> {
    let members = self.struct_scope(definition)?;
    let (body_source, body) = self.aggregate_owner(members)?;
    let arguments_scope = self.program().tree().parent(members)?;
    let outer_scope = self.program().tree().parent(arguments_scope)?;

    // Every argument has to be a constant, since the members are laid out
    // against them (**L§8.5**).
    let parameters = self.struct_parameters(arguments_scope);
    let given = given.iter().copied();
    let mut bindings = Vec::with_capacity(parameters.len());
    for (parameter, expression) in parameters.iter().zip(given) {
      let declared = self.decl_type(*parameter).value;
      let value = match expression {
        Some(expression) => {
          // A procedure name is a constant like any other declared with `::`
          // (**L§5.11**), so `Thing(main)` bakes one.
          let written = self.expression_type(scope, source, expression);
          let named = self.named_procedure(&written);
          written.constant.or(named)?
        }
        // A parameter the instantiation left out takes its default.
        None => self.parameter_default(*parameter)?,
      };
      let value = match self.types().is_unknown(declared) {
        true => value,
        false => value.convert(self.types(), declared).unwrap_or(value),
      };
      bindings.push((*parameter, value));
    }
    // `Thing :: struct (x: $T)` declares `T` beside `x`; the argument's own
    // type is what it bakes to (**L§8.5**).
    let mut variables = Vec::new();
    for (parameter, value) in &bindings {
      let declared = self.decl_type(*parameter).value;
      if !matches!(self.types().kind(declared), TypeKind::Polymorph(_)) {
        continue;
      }
      let Some(decl) = self.arguments_variable(arguments_scope, source, declared) else {
        continue;
      };
      if bindings
        .iter()
        .chain(variables.iter())
        .any(|(bound, _)| *bound == decl)
      {
        continue;
      }
      let bound = self.harden(value.type_id);
      variables.push((decl, Const::new(TypeId::TYPE, Value::Type(bound))));
    }
    // A parameter written `[$N] $T` binds both from the array the call passed
    // (**L§8.5**), the way a procedure's does (**L§7.8**).
    for (parameter, value) in &bindings {
      let declared = self.decl_type(*parameter).value;
      if !self.types().is_unknown(declared) {
        continue;
      }
      let Some((element, ArrayKind::Fixed(count))) = self.types().array_of(value.type_id) else {
        continue;
      };
      let Some((dimension, written_element)) = self.array_slot_variables(*parameter) else {
        continue;
      };
      for (decl, bound) in [
        dimension.map(|decl| (decl, Const::new(TypeId::S64, Value::Int(i128::from(count))))),
        written_element.map(|decl| (decl, Const::new(TypeId::TYPE, Value::Type(element)))),
      ]
      .into_iter()
      .flatten()
      {
        if !bindings
          .iter()
          .chain(variables.iter())
          .any(|(bound, _)| *bound == decl)
        {
          variables.push((decl, bound));
        }
      }
    }
    bindings.extend(variables);
    // The reference names an instantiation in declaration order, which puts a
    // variable before the parameter whose type slot declared it:
    // `Thing(T=string, x="Hello")` (**L§8.5**).
    let order = self.program().tree().declarations(arguments_scope);
    bindings.sort_by_key(|(id, _)| order.iter().position(|declared| declared == id));
    // A struct's `#modify` may change what its arguments are, and
    // deduplication happens after it, the way a procedure's does (**L§8.5**).
    let bindings = self.run_modify(
      body_source,
      body,
      arguments_scope,
      arguments_scope,
      outer_scope,
      bindings,
    )?;

    let key: InstanceKey = (
      body_source,
      body,
      None,
      bindings
        .iter()
        .map(|(id, value)| (*id, value.type_id, const_key(value)))
        .collect(),
      Vec::new(),
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
      self.record_struct_instance(baked, instance);
    }
    Some(baked)
  }

  /// Whether an actual type satisfies a `$T/Base` restriction (**L§7.8**): it
  /// is the base, a variant of it, or reaches it through a `using` or `#as`
  /// member.
  fn satisfies_restriction(
    &mut self,
    actual: TypeId,
    restriction: TypeId,
    interface: bool,
  ) -> bool {
    if self.types().is_unknown(restriction) {
      return true;
    }
    // `$T/interface R` asks for R's *members* rather than for R itself: any
    // struct that declares all of them, of the same types, in any order
    // (**L§7.8**).
    if interface {
      return self.implements_interface(actual, restriction);
    }
    if actual == restriction || self.types().underlying(actual) == restriction {
      return true;
    }
    // `$T/Blentity` on a polymorphic struct means any instantiation of it.
    if self.instantiation_passed(restriction, actual).is_some() {
      return true;
    }
    self.as_conversion(actual, restriction, 0).is_some()
  }

  /// Whether `actual` declares every member the interface struct does, with
  /// the same name and type (**L§7.8**). Position and the members it has
  /// besides do not matter.
  fn implements_interface(&mut self, actual: TypeId, interface: TypeId) -> bool {
    let underlying = self.types().underlying(actual);
    self.complete_type(underlying);
    self.complete_type(interface);
    let (Some(wanted), Some(given)) = (
      self.types().struct_of(interface),
      self.types().struct_of(underlying),
    ) else {
      return false;
    };
    let required: Vec<(Symbol, TypeId)> = self
      .types()
      .struct_info(wanted)
      .members
      .iter()
      .filter(|member| !member.is_constant())
      .map(|member| (member.name, member.type_id))
      .collect();
    required.into_iter().all(|(name, type_id)| {
      self
        .types()
        .struct_info(given)
        .member(name)
        .is_some_and(|member| member.type_id == type_id)
    })
  }

  /// The value a struct parameter takes when the instantiation left it out
  /// (**L§8.5**). A struct's parameters are constants even though they are
  /// written `N: int = 10`, so the default is a constant too — which is what
  /// `Table(string, string)` leans on for the four it does not supply.
  fn parameter_default(&mut self, id: DeclId) -> Option<Const> {
    if let Some(value) = self.decl_constant(id) {
      return Some(value);
    }
    let decl = self.program().tree().decl(id);
    let (source, node, scope) = (decl.source?, decl.node?, decl.scope);
    let NodeData::Declaration(declaration) = self.ast(source)?.data(node) else {
      return None;
    };
    let expression = declaration.expression?;
    let declared = self.decl_type(id).value;
    match self.types().is_unknown(declared) {
      true => self.const_value(scope, source, expression),
      false => self.const_value_at(scope, source, expression, declared),
    }
  }

  /// `Holder(float, 5)`: the family's name with the arguments it was baked
  /// with (**L§8.5**).
  fn instantiation_name(&mut self, family: TypeId, instance: InstanceId) -> Option<Symbol> {
    let base = self.types().declared_name(family)?;
    let mut text = self.symbol_text(base);
    text.push('(');
    let bindings = self.instance(instance).bindings.clone();
    for (index, (parameter, value)) in bindings.iter().enumerate() {
      if index > 0 {
        text.push_str(", ");
      }
      // The reference names each argument: `Holder(T=float32, N=5)`
      // (**L§8.5**).
      let name = self.program().tree().decl(*parameter).name;
      text.push_str(&self.symbol_text(name));
      text.push('=');
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
      .or_else(|| self.program().tree().parent(scopes.constants))?;

    let solution = self.solve(signature, arguments, source, header, scopes)?;
    self.finish_instantiation(signature, source, header, scopes, outer_scope, solution)
  }

  /// Turns a solved header into an instantiation: `#modify` decides it, the
  /// constants key it, and the specialization's own type comes out of reading
  /// the header again (**L§7.8**).
  pub(crate) fn finish_instantiation(
    &mut self,
    signature: &crate::overload::Signature,
    source: SourceId,
    header: NodeId,
    scopes: ProcedureScopes,
    outer_scope: ScopeId,
    solution: Solution,
  ) -> Option<InstanceId> {
    // `#modify` decides the candidate and may change what its variables are,
    // and deduplication happens after it (**L§7.8**).
    let bindings = self.run_modify(
      source,
      header,
      scopes.constants,
      scopes.arguments,
      outer_scope,
      solution.bindings.clone(),
    )?;
    let mut solution = Solution {
      bindings,
      ..solution
    };
    // A parameter whose type mentions a variable is that variable's type,
    // read after `#modify` has said what the variable is: `proc :: (a: $T)
    // #modify { T = s64; }` takes an `s64` however it was called (**L§7.8**).
    for (id, value) in &solution.bindings {
      let decl = self.program().tree().decl(*id);
      let (Some(node), Some(bound)) = (decl.node, value.as_type()) else {
        continue;
      };
      let Some(variable) = self.aggregate_type(decl.source.unwrap_or(source), node) else {
        continue;
      };
      if let TypeKind::Polymorph(definition) = *self.types().kind(variable) {
        solution.substitution.insert(definition, bound);
      }
    }
    let substitution = solution.substitution.clone();
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
      solution.overrides.clone(),
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
    // A header with a `#modify` may decide a variable the call site said
    // nothing about — one declared in the return list, or one whose argument
    // was written `xx` and so has no type of its own (**L§7.8**).
    // Until it runs, the variable is whatever the block will say: `void` while
    // there is an engine to run it, and the unknown that is never reported
    // when there is none, since nothing will ever fill it in.
    let modified = match self.modify_blocks(source, header).is_empty() {
      true => None,
      false => Some(match self.compile_time.is_some() {
        true => TypeId::VOID,
        false => TypeId::UNKNOWN,
      }),
    };

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
      // `[$N] T` binds `N` to how long the argument is. The dimension is a
      // value rather than a type, so unification never reaches it, and the
      // header cannot say what the parameter's type is until it is bound
      // (**L§7.8**).
      if let Some((dimension, count, array)) =
        self.array_dimension_binding(source, signature, index, argument)
      {
        if !solution
          .bindings
          .iter()
          .any(|(bound, _)| *bound == dimension)
        {
          solution
            .bindings
            .push((dimension, Const::new(TypeId::S64, Value::Int(count))));
        }
        if let Some(decl) = self.header_parameter_decl(source, signature, index)
          && !solution.overrides.iter().any(|(bound, _)| *bound == decl)
        {
          solution.overrides.push((decl, array));
        }
      }
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
      // A parameter written as an *instantiation over variables* —
      // `holder: Holder($T, $N)` — matches the arguments of whichever
      // instantiation the call passed, one by one (**L§7.8**, **L§8.5**).
      let matched = self.instantiation_arguments(
        target,
        argument.value.type_id,
        scopes,
        source,
        &mut substitution,
        &mut solution,
      );
      if let Some(matched) = matched {
        if !matched {
          return None;
        }
      }
      // A parameter written as a bare polymorphic struct takes whichever
      // instantiation the call passed (**L§7.8**).
      else if let Some(concrete) = self.instantiation_passed(target, argument.value.type_id)
        && let Some(decl) = self.header_parameter_decl(source, signature, index)
      {
        solution.overrides.push((decl, concrete));
      }
      // A polymorphic procedure handed to a procedure pattern takes the shape
      // the pattern asks for before it decides anything (**L§7.9**).
      else if let Some(matched) =
        self.unify_polymorphic_procedure(target, argument, &mut substitution)
      {
        if !matched {
          return None;
        }
      } else if !self.unify_polymorph(target, &argument.value, &mut substitution)
        && !(modified.is_some() && argument.value.autocast)
      {
        return None;
      }
      // A `$x` parameter is a constant of the instantiation, so the argument
      // has to be one; a `$$x` takes one when the call site has one and stays
      // an ordinary parameter otherwise (**L§7.8**).
      if let Some((decl, required)) = self.baked_parameter_decl(source, signature, index) {
        // A procedure name is a constant like any other declared with `::`
        // (**L§5.11**), which is what lets `$$x` bake one and the body ask
        // `is_constant(x)` about it. It has no value of its own before that,
        // since a call site resolves the name rather than reading it.
        let named = self.named_procedure(&argument.value);
        // A `.[…]` has no type of its own, so it only becomes data once the
        // parameter says which array it is (**L§5.8**, **L§5.11**).
        let folded = argument.value.constant.clone().or(named).or_else(|| {
          let target = solution
            .overrides
            .iter()
            .find(|(bound, _)| *bound == decl)
            .map(|(_, type_id)| *type_id)
            .unwrap_or(parameter.type_id);
          let (written_source, written, written_scope) = argument.written?;
          self.const_value_at(written_scope, written_source, written, target)
        });
        match folded {
          Some(value) => {
            solution.bindings.push((decl, value));
            continue;
          }
          None if required => return None,
          None => continue,
        }
      }
      // A macro's `Code` parameter is the argument itself, unevaluated
      // (**L§13.1**); anything else it was handed a constant for is a constant
      // of the expansion, which is what lets `#assert` and `#if` read it.
      // The `..T` slot is the array the call site builds, not any one of the
      // arguments that went into it, so it binds nothing (**L§7.3**).
      if signature.is_macro
        && vararg_slot != Some(index)
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
    for id in self.program().tree().declarations(scopes.constants) {
      if solution.bindings.iter().any(|(bound, _)| *bound == id) {
        continue;
      }
      let decl = self.program().tree().decl(id);
      let node = decl.node?;
      let variable = self.aggregate_type(decl.source.unwrap_or(source), node)?;
      let TypeKind::Polymorph(definition) = *self.types().kind(variable) else {
        continue;
      };
      let bound = match substitution.get(&definition).copied() {
        Some(bound) => bound,
        // A `#modify` fills what the call site could not: `sum :: (a: [] $T)
        // -> $R` declares `R` in its return list, where nothing an argument
        // says can reach it (**L§7.8**).
        None if modified.is_some() => modified?,
        None => return None,
      };
      solution
        .bindings
        .push((id, Const::new(TypeId::TYPE, Value::Type(bound))));
    }

    // A `$T := int` the call site said nothing about takes the default it was
    // declared with, which is what makes `string_to_int(s)` mean
    // `string_to_int(s, 10, int)` (**L§7.8**). Without a default there is
    // nothing to bake and the candidate does not apply.
    for (index, parameter) in signature.parameters.iter().enumerate() {
      if slots.contains(&index) {
        continue;
      }
      let Some((decl, _)) = self.baked_parameter_decl(source, signature, index) else {
        continue;
      };
      if solution.bindings.iter().any(|(bound, _)| *bound == decl) {
        continue;
      }
      let default = parameter.default?;
      // A `#bake_arguments` writes the default at the *bake*, not in the
      // header, so the node belongs to that file rather than to this one
      // (**L§7.10**). Reading it out of the header's own AST would be reading
      // whichever node happens to sit at that index.
      let default_source = parameter.default_source.unwrap_or(source);
      let scope = self.scope_at(default_source, default, scopes.arguments);
      // A default that is not a constant leaves the parameter unbaked rather
      // than rejecting the candidate: `$x := #caller_location` is filled at the
      // call site like any other default (**L§7.13**).
      let Some(value) = self
        .expression_type(scope, default_source, default)
        .constant
      else {
        continue;
      };
      solution.bindings.push((decl, value));
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
        let default_source = parameter.default_source.unwrap_or(source);
        let scope = self.scope_at(default_source, default, scopes.arguments);
        if let Some(value) = self
          .expression_type(scope, default_source, default)
          .constant
        {
          solution.bindings.push((decl, value));
        }
      }
    }

    let _ = header;
    solution.substitution = substitution;
    solution.bindings.sort_by_key(|(id, _)| *id);
    Some(solution)
  }

  /// The one concrete procedure an expression names, as the constant it is
  /// (**L§5.11**). A polymorphic one has no value until a call site
  /// instantiates it, and a name that stands for a whole overload set has none
  /// until one is chosen (**L§7.5**, **L§7.8**).
  pub(crate) fn named_procedure(&mut self, value: &Expr) -> Option<Const> {
    let [only] = value.overloads[..] else {
      return None;
    };
    let type_id = self.decl_type(only).value;
    let concrete = self.types().procedure_of(type_id).is_some_and(|signature| {
      !signature
        .flags
        .contains(oj_types::ProcedureFlags::IS_POLYMORPHIC)
    });
    concrete.then(|| Const::new(type_id, Value::Procedure(only)))
  }

  /// The variables a parameter written `[$N] $T` declares in its type slot: the
  /// dimension and the element, either of which may be written concretely
  /// (**L§7.8**, **L§8.5**).
  fn array_slot_variables(
    &mut self,
    parameter: DeclId,
  ) -> Option<(Option<DeclId>, Option<DeclId>)> {
    let info = self.program().tree().decl(parameter);
    let (source, node) = (info.source?, info.node?);
    let NodeData::Declaration(declaration) = self.ast(source)?.data(node) else {
      return None;
    };
    let NodeData::TypeInstantiation(inst) = self.ast(source)?.data(declaration.type_inst?) else {
      return None;
    };
    let (dimension, element) = (inst.array_dimension, inst.array_element_type);
    let variable = |checker: &mut Self, node: Option<NodeId>| {
      let node = node?;
      let node = match checker.ast(source)?.data(node) {
        NodeData::TypeInstantiation(inner) => inner.type_valued_expression?,
        _ => node,
      };
      let NodeData::Ident(ident) = checker.ast(source)?.data(node) else {
        return None;
      };
      ident
        .flags
        .contains(oj_syntax::ast::IdentFlags::DEFINES_POLYMORPH_VARIABLE)
        .then(|| checker.decl_at(source, node))
        .flatten()
    };
    let dimension = variable(self, dimension);
    let element = variable(self, element);
    (dimension.is_some() || element.is_some()).then_some((dimension, element))
  }

  /// The `$N` a parameter written `[$N] T` declares, and the length the call
  /// site gives it (**L§7.8**). `None` when the parameter is not written that
  /// way, or when what was passed has no length of its own.
  fn array_dimension_binding(
    &mut self,
    source: SourceId,
    signature: &crate::overload::Signature,
    index: usize,
    argument: &CallArgument,
  ) -> Option<(DeclId, i128, TypeId)> {
    let parameter = self.header_parameter_decl(source, signature, index)?;
    let node = self.program().tree().decl(parameter).node?;
    let NodeData::Declaration(declaration) = self.ast(source)?.data(node) else {
      return None;
    };
    let NodeData::TypeInstantiation(inst) = self.ast(source)?.data(declaration.type_inst?) else {
      return None;
    };
    let dimension = inst.array_dimension?;
    let NodeData::Ident(ident) = self.ast(source)?.data(dimension) else {
      return None;
    };
    if !ident
      .flags
      .contains(oj_syntax::ast::IdentFlags::DEFINES_POLYMORPH_VARIABLE)
    {
      return None;
    }
    let declared = self.decl_at(source, dimension)?;
    let count = match self.types().array_of(argument.value.type_id) {
      Some((_, ArrayKind::Fixed(count))) => i128::from(count),
      // A `.[…]` has no type of its own, so its length is what was written
      // (**L§5.8**).
      _ => {
        let (written_source, written, _) = argument.written?;
        let NodeData::Literal(literal) = self.ast(written_source)?.data(written) else {
          return None;
        };
        let oj_syntax::ast::LiteralValue::Array(array) = &literal.value else {
          return None;
        };
        i128::try_from(array.members.len()).ok()?
      }
    };
    // With the length known the parameter is a real array type, which is what
    // a `$a: [$N] float` has to fold its argument against.
    let scopes = self
      .program()
      .procedure_scopes(source, signature.header?.1)?;
    let element = self.type_from_node(scopes.arguments, source, inst.array_element_type?);
    let length = u64::try_from(count).ok()?;
    let array = self.types_mut().array(element, ArrayKind::Fixed(length));
    Some((declared, count, array))
  }

  /// The declaration of a `$x` or `$$x` parameter, whose *value* the
  /// instantiation bakes rather than its type, and whether the bake is
  /// *required*: `$x` only accepts a constant, while `$$x` takes one when the
  /// call site has one and stays an ordinary parameter otherwise (**L§7.8**).
  fn baked_parameter_decl(
    &mut self,
    source: SourceId,
    signature: &crate::overload::Signature,
    index: usize,
  ) -> Option<(DeclId, bool)> {
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
    let required = declaration
      .flags
      .contains(DeclarationFlags::AUTO_VALUE_BAKE_IS_REQUIRED);
    Some((self.decl_at(source, parameter)?, required))
  }

  /// A parameter written `Holder($T, $N)` matches an argument that is an
  /// instantiation of the same family, and binds each variable to what that
  /// instantiation bound in the same slot (**L§8.5**). `None` when the
  /// parameter is not an instantiation over variables at all.
  #[allow(clippy::too_many_arguments)]
  fn instantiation_arguments(
    &mut self,
    pattern: TypeId,
    actual: TypeId,
    scopes: ProcedureScopes,
    source: SourceId,
    substitution: &mut HashMap<PolymorphId, TypeId>,
    solution: &mut Solution,
  ) -> Option<bool> {
    let (pattern, actual) = match self.types().pointee(pattern) {
      Some(inner) => (inner, self.types().pointee(actual)?),
      None => (pattern, actual),
    };
    let wanted = self.types().struct_of(self.types().underlying(pattern))?;
    let family = self.types().struct_info(wanted).polymorph_source?;
    let bindings = self
      .struct_instance(wanted)
      .map(|instance| self.instance(instance).bindings.clone())?;
    // Only a pattern that mentions a variable is one; `Holder(float, 5)` in a
    // header is an ordinary type.
    if !bindings.iter().any(|(_, value)| {
      value
        .as_type()
        .is_some_and(|type_id| matches!(self.types().kind(type_id), TypeKind::Polymorph(_)))
    }) {
      return None;
    }

    let Some(given) = self.types().struct_of(self.types().underlying(actual)) else {
      return Some(false);
    };
    if self.types().struct_info(given).polymorph_source != Some(family) {
      return Some(false);
    }
    let Some(passed) = self
      .struct_instance(given)
      .map(|instance| self.instance(instance).bindings.clone())
    else {
      return Some(false);
    };
    if bindings.len() != passed.len() {
      return Some(false);
    }

    for ((_, want), (_, got)) in bindings.iter().zip(&passed) {
      let Some(variable) = want.as_type() else {
        if const_key(want) != const_key(got) {
          return Some(false);
        }
        continue;
      };
      let TypeKind::Polymorph(definition) = *self.types().kind(variable) else {
        match got.as_type() {
          Some(given) if given == variable => continue,
          _ => return Some(false),
        }
      };
      match got.as_type() {
        // `$T` takes the type that slot was baked with.
        Some(given) => {
          substitution.entry(definition).or_insert(given);
        }
        // `$N` takes the *value*, which is a constant of the specialization
        // rather than one of its types.
        None => {
          let decl = self.constants_variable(scopes, source, variable)?;
          if !solution.bindings.iter().any(|(bound, _)| *bound == decl) {
            solution.bindings.push((decl, got.clone()));
          }
        }
      }
    }
    Some(true)
  }

  /// The written parameters of a polymorphic struct. A `$T` in a parameter's
  /// type slot is declared beside them, so the arguments scope holds more
  /// names than the argument list does (**L§8.5**).
  fn struct_parameters(&self, arguments: ScopeId) -> Vec<DeclId> {
    self
      .program()
      .tree()
      .declarations(arguments)
      .iter()
      .copied()
      .filter(|id| self.program().tree().decl(*id).kind == oj_scope::DeclKind::Parameter)
      .collect()
  }

  /// The declaration in a struct's argument list that declared the polymorph
  /// variable `variable` in a parameter's type slot.
  fn arguments_variable(
    &mut self,
    arguments: ScopeId,
    source: SourceId,
    variable: TypeId,
  ) -> Option<DeclId> {
    for id in self.program().tree().declarations(arguments) {
      let decl = self.program().tree().decl(id);
      let Some(node) = decl.node else { continue };
      if self.aggregate_type(decl.source.unwrap_or(source), node) == Some(variable) {
        return Some(id);
      }
    }
    None
  }

  /// The declaration in a header's constants block that declared the polymorph
  /// variable `variable`.
  fn constants_variable(
    &mut self,
    scopes: ProcedureScopes,
    source: SourceId,
    variable: TypeId,
  ) -> Option<DeclId> {
    for id in self.program().tree().declarations(scopes.constants) {
      let decl = self.program().tree().decl(id);
      let Some(node) = decl.node else { continue };
      if self.aggregate_type(decl.source.unwrap_or(source), node) == Some(variable) {
        return Some(id);
      }
    }
    None
  }

  /// Whether `actual` is an instantiation of the polymorphic struct `pattern`
  /// names, in which case the parameter is of *that* type (**L§7.8**). A
  /// pointer to one counts, at the same level of indirection.
  fn instantiation_passed(&self, pattern: TypeId, actual: TypeId) -> Option<TypeId> {
    (self.mentions_family(pattern) && self.family_matches(pattern, actual)).then_some(actual)
  }

  /// Whether a type slot names a polymorphic struct family anywhere inside it:
  /// `Holder`, `*Holder`, `*[..] *Holder` (**L§8.5**).
  fn mentions_family(&self, pattern: TypeId) -> bool {
    match self.types().kind(pattern) {
      TypeKind::Pointer(pointee) => self.mentions_family(*pointee),
      TypeKind::Array { element, .. } => self.mentions_family(*element),
      _ => self.is_polymorph_family(pattern),
    }
  }

  /// Whether `actual` has the same shape as `pattern` with every family in it
  /// replaced by one of its instantiations (**L§8.5**).
  fn family_matches(&self, pattern: TypeId, actual: TypeId) -> bool {
    if pattern == actual {
      return true;
    }
    match (
      self.types().kind(pattern).clone(),
      self.types().kind(actual).clone(),
    ) {
      (TypeKind::Pointer(pattern), TypeKind::Pointer(actual)) => {
        self.family_matches(pattern, actual)
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
      ) => (want == got || want == ArrayKind::View) && self.family_matches(pattern, actual),
      _ => {
        let Some(family) = self.types().struct_of(self.types().underlying(pattern)) else {
          return false;
        };
        if !self.is_polymorph_family(pattern) {
          return false;
        }
        self
          .types()
          .struct_of(self.types().underlying(actual))
          .is_some_and(|baked| self.types().struct_info(baked).polymorph_source == Some(family))
      }
    }
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

  /// A polymorphic procedure written where a procedure pattern goes — a quick
  /// lambda, or a `$`-marked header passed by name — is instantiated to the
  /// shape the pattern asks for, and what that instantiation returns is what
  /// decides the rest of the pattern: `map(strings, x => x.count)` solves `$S`
  /// from the lambda once `$T` has made the lambda's own parameter a `string`
  /// (**L§7.8**, **L§7.9**).
  ///
  /// `None` when this is not that case at all, so the ordinary unification
  /// still gets its turn.
  fn unify_polymorphic_procedure(
    &mut self,
    pattern: TypeId,
    argument: &CallArgument,
    substitution: &mut HashMap<PolymorphId, TypeId>,
  ) -> Option<bool> {
    let wanted = self.types().procedure_of(pattern)?.clone();
    let signature = match self.polymorphic_procedure(&argument.value) {
      Some(decl) => self.signature_of(decl),
      None => match argument.written {
        Some((source, node, scope))
          if matches!(
            self.ast(source).map(|ast| ast.data(node)),
            Some(NodeData::ProcedureHeader(_))
          ) =>
        {
          self.signature_of_header(source, node, scope)
        }
        _ => None,
      },
    };
    let signature = signature.filter(|signature| signature.polymorphic)?;
    // Only what the pattern already knows can shape it; a parameter still
    // waiting on a variable of its own is not something to instantiate with.
    let arguments: Vec<TypeId> = wanted
      .arguments
      .iter()
      .map(|type_id| self.substitute(*type_id, substitution))
      .collect();
    if arguments
      .iter()
      .any(|type_id| self.is_polymorphic_type(*type_id))
    {
      return None;
    }
    let arguments: Vec<CallArgument> = arguments
      .into_iter()
      .map(|type_id| CallArgument::positional(Expr::value(type_id)))
      .collect();
    let specialized = self.specialize(&signature, &arguments)?;
    Some(self.unify_types(pattern, specialized.type_id, substitution))
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
    // A name that stands for a whole overload set decides a variable through
    // the one member whose shape the pattern already admits (**L§7.5**).
    if !value.overloads.is_empty() && self.types().procedure_of(pattern).is_some() {
      for candidate in self.narrowed_overloads(value) {
        if self.pattern_admits(pattern, candidate.type_id, substitution) {
          return self.unify_types(pattern, candidate.type_id, substitution);
        }
      }
      return false;
    }
    let actual = self.harden(value.type_id);
    self.unify_types(pattern, actual, substitution)
  }

  /// Whether a type could be what a pattern is solved to, given what is
  /// already substituted: a variable nothing has decided admits anything, and
  /// one that is already bound admits only what it was bound to (**L§7.8**).
  fn pattern_admits(
    &mut self,
    pattern: TypeId,
    actual: TypeId,
    substitution: &HashMap<PolymorphId, TypeId>,
  ) -> bool {
    if pattern == actual || !self.is_polymorphic_type(pattern) {
      return true;
    }
    match (
      self.types().kind(pattern).clone(),
      self.types().kind(actual).clone(),
    ) {
      (TypeKind::Polymorph(definition), _) => substitution.get(&definition).is_none_or(|bound| {
        *bound == actual || self.types().underlying(*bound) == self.types().underlying(actual)
      }),
      (TypeKind::Unknown, _) => true,
      (TypeKind::Pointer(pattern), TypeKind::Pointer(actual)) => {
        self.pattern_admits(pattern, actual, substitution)
      }
      (
        TypeKind::Array {
          element: pattern, ..
        },
        TypeKind::Array {
          element: actual, ..
        },
      ) => self.pattern_admits(pattern, actual, substitution),
      (TypeKind::Procedure(pattern), TypeKind::Procedure(actual)) => {
        pattern.arguments.len() == actual.arguments.len()
          && pattern.returns.len() == actual.returns.len()
          && pattern
            .arguments
            .iter()
            .zip(actual.arguments.iter())
            .chain(pattern.returns.iter().zip(actual.returns.iter()))
            .collect::<Vec<_>>()
            .into_iter()
            .all(|(pattern, actual)| self.pattern_admits(*pattern, *actual, substitution))
      }
      _ => false,
    }
  }

  fn unify_types(
    &mut self,
    pattern: TypeId,
    actual: TypeId,
    substitution: &mut HashMap<PolymorphId, TypeId>,
  ) -> bool {
    // A part of the pattern with no variable in it decides nothing; whether
    // the argument fits there is what scoring answers (**L§7.8**). This is
    // what lets `(int) -> $S` unify with `(int) -> string`.
    if !self.is_polymorphic_type(pattern) {
      return true;
    }
    // A pattern the header could not work out on its own decides nothing and
    // rejects nothing, whatever was passed: `a: [$N] float` has no type until
    // `N` is bound, and the argument may be an untyped `.[…]` (**L§7.8**).
    if matches!(self.types().kind(pattern), TypeKind::Unknown) {
      return true;
    }
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
      // A parameter whose type the header could not work out on its own —
      // `value: holder.T`, which needs the instantiation `holder` was passed —
      // decides nothing here and rejects nothing: re-reading the header with
      // the bindings in place is what gives it a type (**L§7.8**).
      (TypeKind::Unknown, _) => true,
      (TypeKind::Polymorph(definition), _) => {
        // An argument that is itself polymorphic — `square :: (x: $T) -> T`
        // passed where `(x: X) -> X` is wanted — decides nothing; it takes the
        // shape of whatever else does (**L§7.8**).
        if self.is_polymorphic_type(actual) {
          return true;
        }
        // `$T/Entity` only takes an `Entity` or something that reaches one
        // through `using`/`#as` (**L§7.8**).
        let info = self.types().polymorph_info(definition);
        let (restriction, interface) = (info.restriction, info.interface);
        if let Some(restriction) = restriction
          && !self.satisfies_restriction(actual, restriction, interface)
        {
          return false;
        }
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

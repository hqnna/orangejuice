//! What a back end has to ask the typechecker.
//!
//! [`Checker`] answers on demand and memoizes; nothing about a body is
//! recorded until someone asks for it. IR generation walks the same trees the
//! checker did and asks these questions as it goes, so the two never disagree
//! about which procedure a call names or which type an expression has.

use std::collections::HashMap;

use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclId, ScopeId};
use oj_syntax::ast::{Argument, NodeData, NodeId, ProcedureFlags};
use oj_types::{Layout, TypeId};

use crate::checker::{Checker, Expr};
use crate::constants::{Const, Value};
use crate::overload::Resolved;
use crate::poly::InstanceId;

/// One argument of a resolved call, in parameter order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlannedArgument {
  pub source: SourceId,
  /// The scope the expression was written in — the call site's for an argument
  /// the caller wrote, the header's for a default value (**L§7.4**).
  pub scope: ScopeId,
  pub node: NodeId,
  /// The parameter type the argument is converted to.
  pub target: TypeId,
}

/// A call site with its callee decided and its arguments put in parameter
/// order, defaults filled in and extra arguments collected into the varargs
/// slot (**L§7.3**, **L§7.5**).
#[derive(Clone, Debug)]
pub struct CallPlan {
  /// The procedure the call names, or `None` when it goes through a value of
  /// procedure type.
  pub callee: Option<DeclId>,
  /// The instantiation the call names, when the procedure it resolved to was
  /// polymorphic (**L§7.8**). It, not the declaration, is what the back end
  /// generates code for.
  pub instance: Option<InstanceId>,
  /// The procedure type being called.
  pub type_id: TypeId,
  pub returns: Vec<TypeId>,
  pub arguments: Vec<PlannedArgument>,
  /// Which parameter the `..T` one is, the `[] T` it takes, and the arguments
  /// that landed in it, which the call site gathers into one (**L§7.3**). An
  /// argument written `..xs` is the whole `[] T` on its own, and is the only
  /// one there. The other parameters are in `arguments`, in declaration order
  /// with this slot left out.
  pub varargs: Option<(usize, TypeId, Vec<PlannedArgument>)>,
  /// Whether the varargs slot was filled by a single `..xs`.
  pub varargs_spread: bool,
}

/// One instantiation, as a back end sees it (**L§7.8**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstanceInfo {
  /// The declaration the polymorphic header was written at, when it has one.
  pub decl: Option<DeclId>,
  pub source: SourceId,
  pub header: NodeId,
  /// The scope the header was written in.
  pub scope: ScopeId,
  /// The specialized procedure type.
  pub type_id: TypeId,
  /// Where a macro expands, when the header is one: the call site whose block
  /// its body is spliced into (**L§7.13**).
  pub expansion: Option<(SourceId, NodeId)>,
}

/// The pieces of a procedure declaration a back end needs: where its header
/// and body were written, and the declarations of its parameters.
#[derive(Clone, Debug)]
pub struct ProcedureBody {
  pub source: SourceId,
  pub header: NodeId,
  /// The outermost block, or `None` for a `#foreign` or `#elsewhere` header.
  pub block: Option<NodeId>,
  /// The declaring scope, which is the fallback for expressions no reference
  /// recorded a scope for.
  pub scope: ScopeId,
  pub parameters: Vec<Option<DeclId>>,
  pub returns: Vec<Option<DeclId>>,
  pub flags: ProcedureFlags,
  pub foreign_name: Option<Box<[u8]>>,
  /// The name a `#compiler "name"` gave the compiler procedure it declares,
  /// which is the symbol the compiler binds it under (**C§3.3**).
  pub intrinsic_name: Option<Box<[u8]>>,
  pub library: Option<Symbol>,
  pub export_name: Option<Box<[u8]>>,
}

impl Checker<'_> {
  /// The type and constant value of an expression, as the checker sees it.
  pub fn expression(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Expr {
    self.expression_type(scope, source, node)
  }

  /// The type a type expression denotes.
  pub fn denoted_type(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> TypeId {
    self.type_from_node(scope, source, node)
  }

  /// The storage of a type, resolving whatever it depends on first.
  pub fn layout(&mut self, type_id: TypeId) -> Option<Layout> {
    self.layout_of(type_id)
  }

  /// The type table, so that a back end can intern the pointer and view types
  /// its own lowering needs.
  pub fn types_table_mut(&mut self) -> &mut oj_types::Types {
    self.types_mut()
  }

  pub fn tree_of(&self, source: SourceId) -> Option<&oj_syntax::ast::Ast> {
    self.ast(source)
  }

  /// The `it` and `it_index` a `for` loop declares, named or not (**L§6.5**).
  pub fn loop_iterators(
    &self,
    source: SourceId,
    loop_node: NodeId,
  ) -> (Option<DeclId>, Option<DeclId>) {
    let Some(scope) = self.program().loop_scope(source, loop_node) else {
      return (None, None);
    };
    let declarations = &self.program().tree().declarations(scope);
    (declarations.first().copied(), declarations.get(1).copied())
  }

  /// The type an untyped literal settles on when nothing asked for one:
  /// `s64` for an integer, `float32` or `float64` for a float (**L§5.10**).
  pub fn hardened(&self, type_id: TypeId) -> TypeId {
    self.harden(type_id)
  }

  /// The default value of every member of a struct that has one, as
  /// `(offset, type, where it was written)` (**L§8.1**). The struct is laid
  /// out first, so the offsets are real.
  pub fn member_defaults(
    &mut self,
    definition: oj_types::StructId,
  ) -> Vec<(u64, TypeId, SourceId, NodeId)> {
    self.complete_struct(definition);
    let defaults = self.member_defaults_of(definition).to_vec();
    defaults
      .into_iter()
      .filter_map(|(index, source, node)| {
        let member = self.types().struct_info(definition).members.get(index)?;
        Some((member.offset, member.type_id, source, node))
      })
      .collect()
  }

  /// The scope an expression was written in.
  pub fn scope_for(&self, source: SourceId, node: NodeId, fallback: ScopeId) -> ScopeId {
    self.scope_at(source, node, fallback)
  }

  pub fn name_at(&self, source: SourceId, node: NodeId) -> Option<Symbol> {
    self.ident_name(source, node)
  }

  /// Whether a value converts to a target implicitly (**L§5.10**).
  pub fn converts_to(&mut self, value: &Expr, target: TypeId) -> bool {
    self.implicit_conversion(value, target).is_some()
  }

  /// The declaration written at a node, which is how a back end walking a body
  /// finds the local a `x := 1;` introduced.
  pub fn decl_at(&mut self, source: SourceId, node: NodeId) -> Option<DeclId> {
    self.decl_index().get(&(source, node)).copied()
  }

  fn decl_index(&mut self) -> &HashMap<(SourceId, NodeId), DeclId> {
    let count = self.program().tree().declaration_count() as u32;
    if self.decl_nodes.is_none() || self.decl_nodes_indexed < count {
      let tree = self.program().tree();
      let mut index = self.decl_nodes.take().unwrap_or_default();
      for id in (self.decl_nodes_indexed..count).map(DeclId) {
        let decl = tree.decl(id);
        if let (Some(source), Some(node)) = (decl.source, decl.node) {
          index.entry((source, node)).or_insert(id);
        }
      }
      self.decl_nodes = Some(index);
      self.decl_nodes_indexed = count;
    }
    self
      .decl_nodes
      .as_ref()
      .expect("the index was just filled in")
  }

  /// Where a procedure was written, and what its parameters are declared as.
  pub fn procedure_body(&mut self, procedure: DeclId) -> Option<ProcedureBody> {
    let decl = self.program().tree().decl(procedure);
    let (node, source) = (decl.node?, decl.source?);
    let NodeData::Declaration(declaration) = self.ast(source)?.data(node) else {
      return None;
    };
    let export_name = declaration.program_export_name.clone();
    let header = declaration.expression?;
    self.procedure_body_from(source, header, decl.scope, export_name)
  }

  /// What a back end needs to know about one instantiation: where the header
  /// was written, the specialized procedure type, and the declaration it came
  /// from, which is what its symbol is named after (**L§7.8**).
  pub fn instance_info(&self, id: InstanceId) -> InstanceInfo {
    let instance = self.instance(id);
    InstanceInfo {
      decl: instance.decl,
      source: instance.source,
      header: instance.header,
      scope: instance.outer_scope,
      type_id: instance.type_id,
      expansion: instance
        .expansion
        .map(|expansion| (expansion.source, expansion.node)),
    }
  }

  /// A polymorphic procedure written where a concrete one is wanted is
  /// instantiated to it (**L§7.8**), which is also how a quick lambda passed
  /// as an argument becomes a real procedure (**L§7.9**).
  pub fn procedure_instance_for(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    target: TypeId,
  ) -> Option<InstanceId> {
    let key = (self.current_instance, source, node);
    if let Some(found) = self.argument_instances.get(&key) {
      return Some(*found);
    }
    let wanted = self.types().procedure_of(target).cloned()?;
    let value = self.expression_type(scope, source, node);
    let signature = match self.polymorphic_procedure(&value) {
      Some(decl) => self.signature_of(decl),
      None => match self.ast(source).map(|ast| ast.data(node)) {
        Some(NodeData::ProcedureHeader(_)) => self.signature_of_header(source, node, scope),
        _ => None,
      },
    };
    let signature = signature.filter(|signature| signature.polymorphic)?;
    let arguments: Vec<crate::overload::CallArgument> = wanted
      .arguments
      .iter()
      .map(|type_id| crate::overload::CallArgument::positional(Expr::value(*type_id)))
      .collect();
    let specialized = self.specialize(&signature, &arguments)?;
    let instance = specialized.instance?;
    self.use_instance(instance);
    self.argument_instances.insert(key, instance);
    Some(instance)
  }

  /// Which instantiation a declaration belongs to, as the active one sees it:
  /// a local written inside a macro's body is a different local in every
  /// expansion (**L§7.8**, **L§7.13**).
  pub fn decl_instance(&self, decl: DeclId) -> Option<InstanceId> {
    self.decl_key(decl).0
  }

  /// Whether the active instantiation gave a declaration a constant value, so
  /// that a back end knows it needs no storage (**L§7.8**).
  pub fn instance_binds(&self, decl: DeclId) -> bool {
    self.bound_constant(decl).is_some()
  }

  /// How many instantiations the program produced (**L§7.8**). Two call sites
  /// that solve the same constants share one.
  pub fn instance_count(&self) -> usize {
    self.instances.len()
  }

  /// Makes `instance` the active instantiation and returns the one it
  /// replaced, so that a back end walking an instantiated body asks its
  /// questions with the bindings the checker used (**L§7.8**).
  pub fn enter_instance(&mut self, instance: Option<InstanceId>) -> Option<InstanceId> {
    std::mem::replace(&mut self.current_instance, instance)
  }

  /// Where an instantiation's body was written, and what its parameters are.
  pub fn instance_body(&mut self, id: InstanceId) -> Option<ProcedureBody> {
    let info = self.instance_info(id);
    self.procedure_body_from(info.source, info.header, info.scope, None)
  }

  /// A procedure Preload declares, by name. `memcmp` is the one the back end
  /// needs of its own accord, to compare two strings (**L§17**).
  pub fn preload_procedure(&mut self, name: &str) -> Option<DeclId> {
    let symbol = self.interner().intern(name.as_bytes());
    let preload = self.program().preload_scope();
    let oj_scope::Resolution::Found(candidates) = self.program().tree().lookup(preload, symbol)
    else {
      return None;
    };
    candidates
      .into_iter()
      .find(|id| self.program().tree().decl(*id).kind == oj_scope::DeclKind::Procedure)
  }

  /// A procedure the program declares anywhere, by name. The entry point
  /// generated for the C runtime finds `__jai_runtime_init` this way, since it
  /// belongs to Runtime_Support rather than to Preload (**C§13**).
  pub fn procedure_named(&mut self, name: &str) -> Option<DeclId> {
    let symbol = self.interner().intern(name.as_bytes());
    let tree = self.program().tree();
    (0..tree.declaration_count() as u32).map(DeclId).find(|id| {
      let decl = tree.decl(*id);
      decl.name == symbol && decl.kind == oj_scope::DeclKind::Procedure
    })
  }

  /// A type Preload declares, by name — the `Type_Info*` structs the type
  /// table is laid out against (**L§17**). `unknown` when Preload is not
  /// loaded, which is the case when one file is checked on its own.
  pub fn preload_named_type(&mut self, name: &str) -> TypeId {
    let symbol = self.interner().intern(name.as_bytes());
    self.preload_type(symbol)
  }

  /// The branches a static `#if` decided on, or `None` when its condition did
  /// not fold (**L§6.10**). A back end lowers only what the front end kept.
  pub fn static_if_branches(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    payload: &oj_syntax::ast::IfNode,
  ) -> Option<Vec<NodeId>> {
    self.static_branch(scope, source, payload)
  }

  /// The procedure type a header spells out, for a header nobody declared.
  pub fn procedure_type_at(&mut self, source: SourceId, header: NodeId, scope: ScopeId) -> TypeId {
    self.procedure_type(source, header, scope)
  }

  /// The same, for a header nobody declared: the one the parser wraps a `#run`
  /// block in (**L§12.1**).
  pub fn procedure_body_at(
    &mut self,
    source: SourceId,
    header: NodeId,
    scope: ScopeId,
  ) -> Option<ProcedureBody> {
    self.procedure_body_from(source, header, scope, None)
  }

  /// The declaration one parameter introduced. `using p: *Player` wraps the
  /// declaration in the `using`, which is what names it (**L§6.8**).
  fn parameter_decl(&mut self, source: SourceId, parameter: NodeId) -> Option<DeclId> {
    let node = match self.ast(source)?.data(parameter) {
      NodeData::Using(using) => using.expression,
      _ => parameter,
    };
    self.decl_at(source, node)
  }

  fn procedure_body_from(
    &mut self,
    source: SourceId,
    header: NodeId,
    scope: ScopeId,
    export_name: Option<Box<[u8]>>,
  ) -> Option<ProcedureBody> {
    let NodeData::ProcedureHeader(payload) = self.ast(source)?.data(header) else {
      return None;
    };
    let payload = payload.clone();
    let block =
      payload
        .body_or_null
        .and_then(|body| match self.ast(source).map(|ast| ast.data(body)) {
          Some(NodeData::ProcedureBody { block, .. }) => Some(*block),
          _ => None,
        });
    let library = payload
      .library_identifier
      .and_then(|node| self.ident_name(source, node));

    let parameters = payload
      .arguments
      .iter()
      .map(|parameter| self.parameter_decl(source, *parameter))
      .collect();
    let returns = payload
      .returns
      .iter()
      .map(|parameter| self.parameter_decl(source, *parameter))
      .collect();

    Some(ProcedureBody {
      source,
      header,
      block,
      scope,
      parameters,
      returns,
      flags: payload.procedure_flags,
      foreign_name: payload.foreign_function_name.clone(),
      intrinsic_name: payload.intrinsic_name.clone(),
      library,
      export_name,
    })
  }

  /// Which procedure a call names, with its arguments in parameter order
  /// (**L§7.5**). `None` when the front end cannot decide — a polymorphic
  /// candidate, an argument of unknown type — which is what a back end reports
  /// as work a later milestone has to do.
  pub fn call_plan(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<CallPlan> {
    self.at_call_site(scope, source, node, |checker| {
      checker.call_plan_inner(scope, source, node)
    })
  }

  fn call_plan_inner(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<CallPlan> {
    let ast = self.ast(source)?;
    let NodeData::ProcedureCall(call) = ast.data(node) else {
      return None;
    };
    let call = call.clone();

    let arguments = self.call_arguments(scope, source, &call.arguments);
    let callee = self.expression_type(scope, source, call.procedure_expression);
    if callee.denoted.is_some() {
      return None;
    }

    let signature = if callee.overloads.is_empty() {
      let signature = self.signature_of_type(callee.type_id)?;
      match self.accepts(&signature, &arguments) {
        true => signature,
        false => return None,
      }
    } else {
      match self.resolve_overload(&callee.overloads.clone(), &arguments) {
        Resolved::One(signature) => signature,
        Resolved::Ambiguous | Resolved::None => return None,
      }
    };
    if signature.polymorphic {
      return None;
    }
    if let Some(instance) = signature.instance {
      self.use_instance(instance);
    }

    self.plan_arguments(scope, source, &call.arguments, signature)
  }

  fn plan_arguments(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    written: &[Argument],
    signature: crate::overload::Signature,
  ) -> Option<CallPlan> {
    let count = signature.parameters.len();
    let vararg_slot = signature.vararg_slot;
    let typed = self.call_arguments(scope, source, written);
    let mapping = self.argument_slots(&signature, &typed)?;

    let mut slots: Vec<Option<PlannedArgument>> = vec![None; count];
    let mut extra: Vec<PlannedArgument> = Vec::new();

    for (argument, index) in written.iter().zip(mapping) {
      let slot = signature.parameters.get(index)?;
      // An argument in the varargs slot matches the `[] T`'s element rather
      // than the `[] T`, unless it was written `..xs` (**L§7.3**).
      let spread = self.is_spread(source, argument.expression);
      let target = match Some(index) == vararg_slot && !spread {
        true => self
          .types()
          .array_of(slot.type_id)
          .map_or(slot.type_id, |(element, _)| element),
        false => slot.type_id,
      };
      let written = self.scope_at(source, argument.expression, scope);
      self.procedure_instance_for(written, source, argument.expression, target);
      let planned = PlannedArgument {
        source,
        scope: written,
        node: argument.expression,
        target,
      };
      match Some(index) == vararg_slot {
        true => extra.push(planned),
        false => slots[index] = Some(planned),
      }
    }

    // A parameter the call left out takes its default, evaluated where the
    // header wrote it (**L§7.4**).
    let mut planned = Vec::with_capacity(count);
    for (index, slot) in slots.into_iter().enumerate() {
      if Some(index) == vararg_slot {
        continue;
      }
      match slot {
        Some(argument) => planned.push(argument),
        None => {
          let parameter = signature.parameters.get(index)?;
          let default = parameter.default?;
          let default_source = match parameter.default_source {
            Some(source) => source,
            None => signature.header?.0,
          };
          planned.push(PlannedArgument {
            source: default_source,
            scope: self.scope_at(default_source, default, scope),
            node: default,
            target: parameter.type_id,
          });
        }
      }
    }

    let varargs_spread = extra.len() == 1
      && extra
        .first()
        .is_some_and(|argument| self.is_spread(argument.source, argument.node));
    let varargs = vararg_slot.map(|slot| {
      let element = signature.parameters[slot].type_id;
      (slot, element, extra)
    });

    Some(CallPlan {
      callee: signature.decl,
      instance: signature.instance,
      type_id: signature.type_id,
      returns: signature.returns,
      arguments: planned,
      varargs,
      varargs_spread,
    })
  }

  /// The program an `#insert` splices in, expanding it if nobody has yet
  /// (**L§13.2**). The block it names is lowered where the `#insert` stands,
  /// in the scope the expansion recorded.
  pub fn insert_expansion(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<oj_scope::Expansion> {
    self.expand_insert(scope, source, node)
  }

  /// The bytes a variable of `type_id` starts life with: zero, then whatever
  /// default each member carries (**L§8.2**). This is what a metaprogram sees
  /// when it asks the compiler for a struct it has never filled in — a fresh
  /// `Build_Options`, say (**C§3.1**).
  ///
  /// Only what folds is written. A default that needs a `#run`, an address or
  /// an allocation leaves its member zeroed, which is what the reference does
  /// for one it cannot put in the executable either.
  pub fn default_bytes(&mut self, type_id: TypeId) -> Option<Vec<u8>> {
    let layout = self.layout(type_id)?;
    let mut bytes = vec![0u8; layout.size as usize];
    self.write_defaults(type_id, 0, &mut bytes);
    Some(bytes)
  }

  fn write_defaults(&mut self, type_id: TypeId, at: usize, bytes: &mut [u8]) {
    let Some(definition) = self.types().struct_of(type_id) else {
      return;
    };
    let members: Vec<(usize, TypeId, u64, bool)> = self
      .types()
      .struct_info(definition)
      .members
      .iter()
      .enumerate()
      .map(|(index, member)| {
        (
          index,
          member.type_id,
          member.offset,
          member.is_constant() || member.imported_through.is_some(),
        )
      })
      .collect();
    let defaults: Vec<(usize, SourceId, NodeId)> = self.member_defaults_of(definition).to_vec();
    let scope = self.struct_scope(definition);

    for (index, member_type, offset, skip) in members {
      if skip {
        continue;
      }
      let at = at + offset as usize;
      match defaults.iter().find(|(slot, _, _)| *slot == index) {
        Some((_, source, node)) => {
          let (source, node) = (*source, *node);
          let scope = match scope {
            Some(scope) => self.scope_at(source, node, scope),
            None => continue,
          };
          if let Some(constant) = self.expression(scope, source, node).constant {
            self.write_constant(&constant, member_type, at, bytes);
          }
        }
        // A member with no default of its own may still be a struct whose
        // members have theirs.
        None => self.write_defaults(member_type, at, bytes),
      }
    }
  }

  /// Writes one folded constant into a struct's bytes, as far as a value that
  /// has to survive without an address can go.
  fn write_constant(&mut self, constant: &Const, target: TypeId, at: usize, bytes: &mut [u8]) {
    let Some(layout) = self.layout(target) else {
      return;
    };
    let size = layout.size as usize;
    if size == 0 || at + size > bytes.len() {
      return;
    }
    // `= .EXECUTABLE` is a name until something says which enum it belongs to,
    // and the member is what says (**L§5.12**).
    let resolved = match &constant.value {
      Value::EnumName(name) => self
        .types()
        .enum_of(self.types().underlying(target))
        .and_then(|definition| self.types().enum_info(definition).value_of(*name))
        .map(|value| Value::Int(i128::from(value))),
      _ => None,
    };
    let value = resolved.as_ref().unwrap_or(&constant.value);
    let slot = &mut bytes[at..at + size];
    match value {
      Value::Bool(value) => slot[0] = u8::from(*value),
      Value::Int(value) => {
        let value = *value as u128;
        for (index, byte) in slot.iter_mut().enumerate() {
          *byte = (value >> (index * 8)) as u8;
        }
      }
      Value::Float(value) => match size {
        4 => slot.copy_from_slice(&(*value as f32).to_ne_bytes()),
        8 => slot.copy_from_slice(&value.to_ne_bytes()),
        _ => {}
      },
      Value::Bytes(source) if source.len() == size => slot.copy_from_slice(&source.data),
      _ => {}
    }
  }

  /// Records a `compiler_set_type_info_flags` (**C§3.3**). The flags are only
  /// ever or-ed in: the reference says whatever is set here joins whatever the
  /// struct was declared with, and nothing clears them.
  pub fn add_type_info_flags(&mut self, type_id: TypeId, flags: u32) {
    *self.type_info_flags.entry(type_id).or_default() |= flags;
  }

  /// What a metaprogram asked to be left out of a type's record, as
  /// `Type_Info_Flags` numbers it (**C§3.3**).
  pub fn type_info_flags_of(&self, type_id: TypeId) -> u32 {
    self.type_info_flags.get(&type_id).copied().unwrap_or(0)
  }

  /// Where a struct was written: the file, the line and the character
  /// (**C§3.3**). This is what `compiler_get_struct_location` answers, and it
  /// is the declaration's own place rather than any use of it.
  pub fn struct_location(&mut self, type_id: TypeId) -> Option<(String, i64, i64)> {
    let underlying = self.types().underlying(type_id);
    let definition = self.types().struct_of(underlying)?;
    let scope = self.struct_scope(definition)?;
    let (source, node) = self.aggregate_owner(scope)?;
    let span = self.ast(source)?.node(node).span;
    let file = self.program().sources().file(source);
    let at = file.location(span.start);
    Some((
      file.path().display().to_string(),
      i64::from(at.line),
      i64::from(at.column),
    ))
  }

  /// The type a name denotes anywhere in the program. A metaprogram's own
  /// structs — `Build_Options`, `Message` — belong to the distribution rather
  /// than to the compiler, so this is how the compiler finds one without
  /// writing its scope down (**C§3.2**).
  pub fn type_named(&mut self, name: &str) -> Option<TypeId> {
    let symbol = self.interner().intern(name.as_bytes());
    let tree = self.program().tree();
    let candidates: Vec<DeclId> = (0..tree.declaration_count() as u32)
      .map(DeclId)
      .filter(|id| {
        let decl = tree.decl(*id);
        decl.name == symbol && matches!(decl.kind, oj_scope::DeclKind::Struct)
      })
      .collect();
    candidates
      .into_iter()
      .find_map(|id| self.decl_type(id).denoted)
  }
}

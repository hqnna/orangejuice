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
  /// The `[] T` a `..T` parameter is, and the arguments that landed in it,
  /// which the call site gathers into one (**L§7.3**). An argument written
  /// `..xs` is the whole `[] T` on its own, and is the only one there.
  pub varargs: Option<(TypeId, Vec<PlannedArgument>)>,
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
    let declarations = &self.program().tree().scope(scope).declarations;
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
    if self.decl_nodes.is_none() {
      let tree = self.program().tree();
      let mut index = HashMap::new();
      for id in (0..tree.declaration_count() as u32).map(DeclId) {
        let decl = tree.decl(id);
        if let (Some(source), Some(node)) = (decl.source, decl.node) {
          index.entry((source, node)).or_insert(id);
        }
      }
      self.decl_nodes = Some(index);
    }
    self
      .decl_nodes
      .as_ref()
      .expect("the index was just filled in")
  }

  /// Where a procedure was written, and what its parameters are declared as.
  pub fn procedure_body(&mut self, procedure: DeclId) -> Option<ProcedureBody> {
    let decl = self.program().tree().decl(procedure).clone();
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
    }
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
      .map(|parameter| self.decl_at(source, *parameter))
      .collect();
    let returns = payload
      .returns
      .iter()
      .map(|parameter| self.decl_at(source, *parameter))
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
      library,
      export_name,
    })
  }

  /// Which procedure a call names, with its arguments in parameter order
  /// (**L§7.5**). `None` when the front end cannot decide — a polymorphic
  /// candidate, an argument of unknown type — which is what a back end reports
  /// as work a later milestone has to do.
  pub fn call_plan(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<CallPlan> {
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
    let vararg_slot = signature.varargs.then(|| count.saturating_sub(1));
    let fixed = vararg_slot.unwrap_or(count);

    let mut slots: Vec<Option<PlannedArgument>> = vec![None; fixed];
    let mut extra: Vec<PlannedArgument> = Vec::new();
    let mut next = 0usize;

    for argument in written {
      let name = argument.name.and_then(|node| self.ident_name(source, node));
      let index = match name {
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
      // An argument past the last declared parameter, or one that lands in the
      // varargs slot, matches the `[] T`'s element rather than the `[] T`
      // (**L§7.9**).
      let (slot, spreads) = match signature.parameters.get(index) {
        Some(parameter) if Some(index) != vararg_slot => (parameter, false),
        Some(parameter) => (parameter, true),
        None => (signature.parameters.get(vararg_slot?)?, true),
      };
      // `..xs` fills the whole slot; anything else fills one element of it.
      let spread = self.is_spread(source, argument.expression);
      let target = match spreads && !spread {
        true => self
          .types()
          .array_of(slot.type_id)
          .map_or(slot.type_id, |(element, _)| element),
        false => slot.type_id,
      };
      let planned = PlannedArgument {
        source,
        scope: self.scope_at(source, argument.expression, scope),
        node: argument.expression,
        target,
      };
      match index < fixed {
        true => slots[index] = Some(planned),
        false => extra.push(planned),
      }
    }

    // A parameter the call left out takes its default, evaluated where the
    // header wrote it (**L§7.4**).
    let mut planned = Vec::with_capacity(fixed);
    for (index, slot) in slots.into_iter().enumerate() {
      match slot {
        Some(argument) => planned.push(argument),
        None => {
          let parameter = signature.parameters.get(index)?;
          let default = parameter.default?;
          let (header_source, _) = signature.header?;
          planned.push(PlannedArgument {
            source: header_source,
            scope: self.scope_at(header_source, default, scope),
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
      (element, extra)
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
}

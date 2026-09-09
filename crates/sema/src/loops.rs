//! Iterating over something the language does not know how to iterate over
//! (**L§7.14**).
//!
//! A `for` over a value of struct type expands the `for_expansion` macro found
//! in that type's scope, handing it the container, the loop's body as `Code`
//! and the modifiers the loop was written with. The macro exports `` `it `` and
//! `` `it_index ``, and those are what the loop's own iterators are.

use std::collections::HashMap;

use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclId, ScopeId};
use oj_syntax::ast::{DeclarationFlags, ForFlags, ForNode, NodeData, NodeId};
use oj_types::TypeId;

use crate::checker::{Checker, Expr};
use crate::constants::{Const, Value};
use crate::overload::{CallArgument, Resolved};
use crate::poly::InstanceId;

/// What a `for` over a container found (**L§7.14**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopExpansion {
  pub instance: InstanceId,
  /// The macro's `` `it ``, whose type is the loop's `it`.
  pub it: Option<DeclId>,
  pub it_index: Option<DeclId>,
}

impl Checker<'_> {
  /// The `for_expansion` a `for` over a container reaches, expanded for this
  /// loop (**L§7.14**). `None` when the loop iterates something built in.
  pub fn loop_expansion(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    loop_node: NodeId,
  ) -> Option<LoopExpansion> {
    let instance = self.current_instance;
    if let Some(found) = self.loop_expansions.get(&(instance, source, loop_node)) {
      return *found;
    }
    // A loop that is being worked out is not asked about again: its `it` is
    // what the expansion is for.
    self
      .loop_expansions
      .insert((instance, source, loop_node), None);
    let found = self.build_loop_expansion(scope, source, loop_node);
    self
      .loop_expansions
      .insert((instance, source, loop_node), found);
    found
  }

  fn build_loop_expansion(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    loop_node: NodeId,
  ) -> Option<LoopExpansion> {
    let NodeData::For(payload) = self.ast(source)?.data(loop_node) else {
      return None;
    };
    let payload = payload.clone();
    // A range and the three array kinds iterate on their own (**L§6.6**).
    if payload.iteration_expression_right.is_some() {
      return None;
    }
    let scope = self.scope_at(source, payload.iteration_expression, scope);
    let subject = self.expression_type(scope, source, payload.iteration_expression);

    // `for :utf8_iter s` names the macro itself, which is an ordinary name in
    // the loop's own scope and applies whatever the container is (**L§7.14**);
    // a bare `for` over a range or an array iterates on its own (**L§6.6**).
    let candidates = match payload.want_replacement_for_expansion {
      Some(node) => {
        let name = self.ident_name(source, node)?;
        let where_written = self.scope_at(source, node, scope);
        match self.program().tree().lookup(where_written, name) {
          oj_scope::Resolution::Found(candidates) => candidates,
          _ => return None,
        }
      }
      None => {
        if self.types().array_of(subject.type_id).is_some()
          || self.types().underlying(subject.type_id) == TypeId::STRING
        {
          return None;
        }
        // A `for` over a pointer to the container works too (**L§7.14**).
        let container = self
          .types()
          .pointee(subject.type_id)
          .unwrap_or(subject.type_id);
        let definition = self.types().struct_of(self.types().underlying(container))?;
        let members = self.struct_scope(definition)?;
        let name = self.interned().intern(b"for_expansion");
        match self.program().tree().lookup(members, name) {
          oj_scope::Resolution::Found(candidates) => candidates,
          _ => return None,
        }
      }
    };

    let header = candidates
      .first()
      .and_then(|candidate| self.signature_of(*candidate));
    // The flags are read as the enum the macro declared them, which is
    // Preload's `For_Flags` (**L§7.14**) but is whatever the header wrote.
    let flag_type = header
      .as_ref()
      .and_then(|signature| signature.parameters.get(2).map(|slot| slot.type_id))
      .filter(|type_id| self.types().enum_of(*type_id).is_some())
      .unwrap_or_else(|| self.preload_type(self.interned().intern(b"For_Flags")));
    // The container is handed over by pointer when the macro asked for one,
    // whatever the loop was written over (**L§7.14**).
    let wants_pointer = header
      .as_ref()
      .and_then(|signature| signature.parameters.first().map(|slot| slot.type_id))
      .is_some_and(|slot| self.types().is_pointer(slot))
      && !self.types().is_pointer(subject.type_id);
    let subject = match wants_pointer {
      true => Expr::value(self.types_mut().pointer_to(subject.type_id)),
      false => subject,
    };
    let arguments = self.expansion_arguments(scope, source, &payload, &subject, flag_type);
    let instance = self.at_call_site(scope, source, loop_node, |checker| {
      match checker.resolve_overload(&candidates, &arguments) {
        Resolved::One(signature) => signature.instance,
        Resolved::Ambiguous | Resolved::None => None,
      }
    })?;
    self.use_instance(instance);

    let it = self.exported_declaration(instance, b"it");
    let it_index = self.exported_declaration(instance, b"it_index");
    Some(LoopExpansion {
      instance,
      it,
      it_index,
    })
  }

  /// Whether a `for` iterates by pointer, and whether it runs backwards
  /// (**L§6.6**). `for *= cond, <= cond xs` decides both at compile time,
  /// which is what a `for_expansion` handed its caller's `For_Flags` does;
  /// `None` is a modifier that only a running program could answer, which
  /// nothing here can lower.
  pub fn loop_modifiers(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<(bool, bool)> {
    let NodeData::For(payload) = self.ast(source)?.data(node) else {
      return None;
    };
    let payload = payload.clone();
    let mut answers = [false, false];
    for (index, (written, flag)) in [
      (payload.want_pointer_expression, ForFlags::POINTER),
      (payload.want_reverse_expression, ForFlags::REVERSE),
    ]
    .into_iter()
    .enumerate()
    {
      answers[index] = match written {
        None => payload.for_flags.contains(flag),
        Some(expression) => {
          let scope = self.scope_at(source, expression, scope);
          self
            .expression(scope, source, expression)
            .constant?
            .value
            .truth()?
        }
      };
    }
    Some((answers[0], answers[1]))
  }

  /// What the compiler hands a `for_expansion`: the container, the loop's body
  /// as `Code`, and the modifiers the loop was written with (**L§7.14**).
  fn expansion_arguments(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    payload: &ForNode,
    subject: &Expr,
    flag_type: TypeId,
  ) -> Vec<CallArgument> {
    let mut flags = 0i128;
    for (written, bit) in [
      (ForFlags::POINTER, 1),
      (ForFlags::REVERSE, 2),
      (ForFlags::TEMPORARY_V2, 4),
    ] {
      if payload.for_flags.contains(written) {
        flags |= bit;
      }
    }
    let body_scope = self.scope_at(source, payload.block, scope);
    vec![
      CallArgument {
        name: None,
        value: subject.clone(),
        spread: false,
        written: Some((source, payload.iteration_expression, scope)),
      },
      CallArgument {
        name: None,
        value: Expr::constant(Const::new(
          TypeId::CODE,
          Value::Code {
            source,
            node: payload.block,
            scope: body_scope,
          },
        )),
        spread: false,
        written: Some((source, payload.block, body_scope)),
      },
      CallArgument {
        name: None,
        value: Expr::constant(Const::new(flag_type, Value::Int(flags))),
        spread: false,
        written: None,
      },
    ]
  }

  /// A name the *active* macro declared with a backtick of its own. Nothing
  /// records a scope for a backticked identifier — it is meant to resolve in
  /// the caller — so the instantiation's own body is searched by name
  /// (**L§7.13**).
  pub(crate) fn backticked_declaration(&self, name: Symbol) -> Option<DeclId> {
    let mut current = self.current_instance;
    while let Some(id) = current {
      let root = self.instance(id).body_root();
      let mut pending = vec![root];
      while let Some(scope) = pending.pop() {
        let tree = self.program().tree();
        for declared in &tree.declarations(scope) {
          let decl = tree.decl(*declared);
          if decl.name == name && decl.flags.contains(DeclarationFlags::HAS_SCOPE_MODIFIER) {
            return Some(*declared);
          }
        }
        pending.extend(tree.children(scope).iter().copied());
      }
      current = self.instance(id).parent;
    }
    None
  }

  fn exported_declaration(&self, instance: InstanceId, name: &[u8]) -> Option<DeclId> {
    let name = self.interned().intern(name);
    let root = self.instance(instance).body_root();
    let mut pending = vec![root];
    while let Some(scope) = pending.pop() {
      let tree = self.program().tree();
      for id in &tree.declarations(scope) {
        let decl = tree.decl(*id);
        if decl.name == name && decl.flags.contains(DeclarationFlags::HAS_SCOPE_MODIFIER) {
          return Some(*id);
        }
      }
      pending.extend(tree.children(scope).iter().copied());
    }
    None
  }

  /// The type the loop's `it` or `it_index` takes from the expansion
  /// (**L§7.14**).
  pub fn expansion_iterator_type(&mut self, expansion: LoopExpansion, is_index: bool) -> TypeId {
    let declaration = match is_index {
      true => expansion.it_index,
      false => expansion.it,
    };
    let Some(declaration) = declaration else {
      return TypeId::UNKNOWN;
    };
    self.with_instance(Some(expansion.instance), |checker| {
      checker.decl_type(declaration).value
    })
  }
}

/// The loops whose expansion has been worked out, by the instantiation they
/// were reached in.
pub(crate) type LoopExpansions =
  HashMap<(Option<InstanceId>, SourceId, NodeId), Option<LoopExpansion>>;

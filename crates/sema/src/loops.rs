//! Iterating over something the language does not know how to iterate over
//! (**L§7.14**).
//!
//! A `for` over a value of struct type expands the `for_expansion` macro found
//! in that type's scope, handing it the container, the loop's body as `Code`
//! and the modifiers the loop was written with. The macro exports `` `it `` and
//! `` `it_index ``, and those are what the loop's own iterators are.

use std::collections::HashMap;

use oj_diag::SourceId;
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

    let name = match payload.want_replacement_for_expansion {
      Some(node) => self.ident_name(source, node)?,
      None => self.interned().intern(b"for_expansion"),
    };
    let oj_scope::Resolution::Found(candidates) = self.program().tree().lookup(members, name)
    else {
      return None;
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

  /// A name the macro exported with a backtick (**L§7.13**), found anywhere in
  /// its body.
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

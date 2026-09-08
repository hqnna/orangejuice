//! `#insert`: splicing a piece of program into the scope the directive was
//! written in (**L§13.2**).
//!
//! The scope tree already expanded every `#insert` whose text it could fold on
//! its own — a literal, or a constant string declared somewhere. What is left
//! is the ones that needed a type or a `#run` to say what their text is, and
//! those are this module's: the checker asks the operand for its value, and
//! whatever comes back is admitted to the program the same way, so an
//! `#insert` expands exactly once however it was reached.

use oj_diag::SourceId;
use oj_scope::{Expansion, InsertKind, ScopeId, ScopeKind, Visibility};
use oj_syntax::ast::{NodeData, NodeId};

use crate::checker::Checker;
use crate::constants::Value;

impl Checker<'_> {
  /// Expands every `#insert` the scope tree left behind, before any
  /// declaration is typed: what one of them declares has to be in the tree
  /// before a name that reads it is resolved (**L§11.7**). Expanding may add
  /// `#insert`s of its own, so the list is re-read as it grows.
  ///
  /// An `#insert` written inside a polymorphic body or a macro is left alone:
  /// its text is whatever the instantiation makes it, so it belongs to the
  /// instantiation rather than to the program (**L§7.8**).
  pub(crate) fn expand_inserts(&mut self) {
    let mut index = 0;
    loop {
      let pending = self.program().pending_inserts();
      let Some(insert) = pending.get(index).copied() else {
        return;
      };
      index += 1;
      if self.program().is_uninstantiated(insert.scope) || self.program().is_in_macro(insert.scope)
      {
        continue;
      }
      self.expand_insert(insert.scope, insert.source, insert.node);
    }
  }

  /// The program `node` — an `#insert` — stands for, expanding it if that has
  /// not happened yet.
  pub(crate) fn expand_insert(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<Expansion> {
    self.expand_insert_as(scope, source, node, None)
  }

  /// The one expression an `#insert` in expression position stands for
  /// (**L§13.2**), with the source and scope to read it in.
  pub fn insert_expression(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<(SourceId, ScopeId, NodeId)> {
    let expansion = self.expand_insert_as(scope, source, node, Some(InsertKind::Expression))?;
    let ast = self.ast(expansion.source)?;
    let expression = match ast.data(expansion.root) {
      NodeData::Block(block) => *block.statements.first()?,
      _ => expansion.root,
    };
    Some((expansion.source, expansion.scope, expression))
  }

  fn expand_insert_as(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    kind: Option<InsertKind>,
  ) -> Option<Expansion> {
    if let Some(existing) = self.program().expansion_of(source, node) {
      return Some(existing);
    }
    let ast = self.ast(source)?;
    let NodeData::DirectiveInsert(insert) = ast.data(node) else {
      return None;
    };
    let expression = insert.expression;
    let span = ast.node(node).span;

    // The text is worked out where the operand was written; the program it
    // stands for joins the scope the `#insert` itself is in.
    let target = self.scope_at(source, node, scope);
    let operand_scope = self.scope_at(source, expression, target);
    let value = self
      .expression(operand_scope, source, expression)
      .constant?
      .value;
    match value {
      Value::String(text) => {
        let kind = kind.unwrap_or_else(|| insert_kind(self, target));
        self
          .program()
          .insert_source(target, kind, (source, node), &text)
      }
      // `#insert code` names a piece of program that was parsed where it was
      // written, so nothing has to be parsed here: it is spliced as it stands.
      Value::Code {
        source: code_source,
        node: code,
        scope: code_scope,
      } => Some(Expansion {
        source: code_source,
        root: code,
        scope: code_scope,
      }),
      _ => {
        self.error(
          source,
          span,
          "The operand of an #insert must be a string or a Code.",
        );
        None
      }
    }
  }
}

/// Where the declarations an `#insert` makes land, which is decided by the kind
/// of scope it was written in (**L§13.2**).
fn insert_kind(checker: &Checker<'_>, scope: ScopeId) -> InsertKind {
  match checker.program().tree().scope_kind(scope) {
    ScopeKind::StructMembers | ScopeKind::Enum => InsertKind::Members,
    kind if kind.is_program_scope() => InsertKind::Data(Visibility::Export),
    _ => InsertKind::Imperative,
  }
}

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
    let ast = self.ast(source)?;
    let NodeData::DirectiveInsert(insert) = ast.data(node) else {
      return None;
    };
    let expression = insert.expression;
    let span = ast.node(node).span;

    // The text is worked out where the operand was written; the program it
    // stands for joins the scope the `#insert` itself is in — the one the
    // walker recorded, since a caller holding only a body-wide fallback cannot
    // work out that an `#insert` in a procedure body belongs to that block. A
    // macro is the exception: its body is spliced into whoever expanded it, so
    // an `#insert` inside one belongs to the scope that expansion is going to.
    let written = self
      .program()
      .insert_scope(source, node)
      .filter(|scope| !self.program().is_in_macro(*scope));
    let target = written.unwrap_or_else(|| self.scope_at(source, node, scope));
    let operand_scope = self.scope_at(source, expression, target);
    // One written in a polymorphic body expands once per instantiation: the
    // text is whatever the constants make it (**L§13.2**).
    let variant = self
      .instance_of_scope(operand_scope)
      .map_or(0, |instance| instance.0 + 1);
    if let Some(existing) = self.program().expansion_of(source, node, variant) {
      return Some(existing);
    }
    // A `#placeholder` nothing has filled stands for no program at all. The
    // reference stalls the `#insert` until a metaprogram declares the name;
    // orangejuice compiles a `#run` before it executes, so the fill can never
    // arrive in time and the insert expands to nothing (`docs/spec.md` §10).
    if self.names_unfilled_placeholder(operand_scope, source, expression) {
      let kind = kind.unwrap_or_else(|| insert_kind(self, target));
      return self
        .program()
        .insert_source(target, kind, (source, node, variant), b"");
    }
    let value = self
      .expression(operand_scope, source, expression)
      .constant?
      .value;
    match value {
      Value::String(text) => {
        let kind = kind.unwrap_or_else(|| insert_kind(self, target));
        self
          .program()
          .insert_source(target, kind, (source, node, variant), &text)
      }
      // `#insert code` names a piece of program that was parsed where it was
      // written, so nothing has to be parsed here: it is spliced as it stands.
      //
      // Code a `#run` made — `#insert -> Code { … }` is the short way of
      // writing one — was built somewhere that is not part of the program, so
      // its names mean nothing there and everything at the `#insert`: that is
      // the whole point of handing a piece of program back (**L§13.1**).
      Value::Code {
        source: code_source,
        node: code,
        scope: code_scope,
      } => {
        // A value that names the `#code` itself stands for the program under
        // it: the directive is how the program was quoted, not part of it.
        let root = match self.ast(code_source).map(|ast| ast.data(code)) {
          Some(NodeData::DirectiveCode {
            expression: Some(inner),
            ..
          }) => *inner,
          _ => code,
        };
        Some(Expansion {
          source: code_source,
          root,
          scope: match matches!(ast.data(expression), NodeData::DirectiveRun(_)) {
            true => target,
            false => self
              .program()
              .directive_scope(code_source, code)
              .unwrap_or(code_scope),
          },
        })
      }
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

impl Checker<'_> {
  /// Whether the operand of an `#insert` is a `#placeholder` and nothing else:
  /// a name a metaprogram was meant to declare and did not (**L§11.8**).
  fn names_unfilled_placeholder(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> bool {
    let Some(name) = self.name_at(source, node) else {
      return false;
    };
    match self.program().tree().lookup(scope, name) {
      oj_scope::Resolution::Found(candidates) => {
        !candidates.is_empty()
          && candidates
            .iter()
            .all(|id| self.program().tree().decl(*id).kind == oj_scope::DeclKind::Placeholder)
      }
      _ => false,
    }
  }
}

use oj_diag::{SourceId, Span};
use oj_scope::ScopeId;
use oj_syntax::ast::{Argument, LiteralValue, NodeData, NodeId, OperatorType};
use oj_types::{ArrayKind, MemberFlags, StructMember, TypeId};

use crate::checker::Checker;

impl Checker<'_> {
  /// Whether a struct literal's argument fills a positional slot. `values[1] =
  /// 7` and `e.z = 9` name a member too, but their left side is a path rather
  /// than a name, so the parser leaves them as assignments (**L§5.7**).
  fn is_positional_member(&self, source: SourceId, argument: &Argument) -> bool {
    argument.name.is_none()
      && !self.ast(source).is_some_and(|ast| {
        matches!(
          ast.data(argument.expression),
          NodeData::BinaryOperator {
            operator: OperatorType::ASSIGN,
            ..
          }
        )
      })
  }

  /// `T.{…}` (**L§5.7**): a positional literal fills the settable members in
  /// order, a named one fills any subset, and every value has to convert to
  /// the member it fills.
  pub(crate) fn check_struct_literal(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    span: Span,
    type_id: TypeId,
    arguments: &[Argument],
  ) {
    let scope = self.nearest_scope(source, node, scope);
    self.complete_type(type_id);
    let Some(definition) = self.types().struct_of(type_id) else {
      return;
    };
    if self.mentions_unknown(type_id) {
      return;
    }
    let settable: Vec<StructMember> = self
      .types()
      .struct_info(definition)
      .settable_members()
      .cloned()
      .collect();
    let all_members: Vec<StructMember> = self
      .types()
      .struct_info(definition)
      .members
      .iter()
      .filter(|member| !member.flags.contains(MemberFlags::CONSTANT))
      .cloned()
      .collect();

    let positional = arguments
      .iter()
      .filter(|argument| self.is_positional_member(source, argument))
      .count();
    let named = arguments.len() - positional;
    if named == 0 && positional > 0 && positional != settable.len() {
      let message = if positional > settable.len() {
        "Too many values provided in this struct literal."
      } else {
        "Not enough values provided in this struct literal."
      };
      self.error(source, span, message);
      return;
    }

    for (index, argument) in arguments.iter().enumerate() {
      let member = match argument.name {
        Some(node) => {
          let Some(name) = self.ident_name(source, node) else {
            continue;
          };
          match all_members.iter().find(|member| member.name == name) {
            Some(member) => member.clone(),
            None => {
              // A scope that can still gain names may yet give this literal a
              // different type, so a member it does not have is not one to
              // complain about (**L§4.3**).
              if self.scope_may_gain_names(scope) {
                continue;
              }
              let (name, owner) = (self.symbol_text(name), self.type_name(type_id));
              let span = self.ast(source).map_or(span, |ast| ast.node(node).span);
              self.error(
                source,
                span,
                format!("'{name}' is not a member of {owner}."),
              );
              continue;
            }
          }
        }
        None if !self.is_positional_member(source, argument) => {
          self.expression_type(scope, source, argument.expression);
          continue;
        }
        None => match settable.get(index) {
          Some(member) => member.clone(),
          None => continue,
        },
      };

      let value = self.expression_type(scope, source, argument.expression);
      if self.implicit_conversion(&value, member.type_id).is_none() {
        let span = self
          .ast(source)
          .map_or(span, |ast| ast.node(argument.expression).span);
        self.report_mismatch(source, span, member.type_id, value.type_id);
      }
    }
  }

  /// Whether a `.{…}` or `.[…]` written at `node` could be a `target`
  /// (**L§5.7**, **L§5.8**). A literal has no type of its own, so this is what
  /// tells two overloads apart when one is passed to both: `.{x, y}` is the
  /// `put_vertex` taking a `Vector2`, not the one taking a `Vector3`. Only a
  /// mismatch the literal itself shows — the wrong number of positional
  /// values, or a name the struct does not have — rules a target out; anything
  /// the checker cannot decide yet admits it.
  pub(crate) fn untyped_literal_admits(
    &mut self,
    source: SourceId,
    node: NodeId,
    target: TypeId,
  ) -> bool {
    let Some(ast) = self.ast(source) else {
      return true;
    };
    let NodeData::Literal(literal) = ast.data(node) else {
      return true;
    };
    match literal.value.clone() {
      LiteralValue::Struct(payload) if payload.type_expression.is_none() => {
        if !self.types().is_complete(target) {
          return true;
        }
        if self.mentions_unknown(target) {
          return true;
        }
        let Some(definition) = self.types().struct_of(target) else {
          return false;
        };
        let settable = self
          .types()
          .struct_info(definition)
          .settable_members()
          .count();
        let names: Vec<oj_lexer::Symbol> = self
          .types()
          .struct_info(definition)
          .members
          .iter()
          .filter(|member| !member.flags.contains(MemberFlags::CONSTANT))
          .map(|member| member.name)
          .collect();
        let positional = payload
          .arguments
          .iter()
          .filter(|argument| self.is_positional_member(source, argument))
          .count();
        let named = payload.arguments.len() - positional;
        if named == 0 && positional > 0 && positional != settable {
          return false;
        }
        payload.arguments.iter().all(|argument| {
          let Some(name) = argument.name.and_then(|node| self.ident_name(source, node)) else {
            return true;
          };
          names.contains(&name)
        })
      }
      LiteralValue::Array(array) if array.element_type.is_none() => {
        match self.types().array_of(target) {
          Some((_, kind)) => {
            !matches!(kind, ArrayKind::Fixed(count) if count != array.members.len() as u64)
          }
          None => self.types().underlying(target) == TypeId::STRING,
        }
      }
      _ => true,
    }
  }

  /// `T.[…]` (**L§5.8**): every element has to convert to the element type.
  pub(crate) fn check_array_literal(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    element: TypeId,
    members: &[NodeId],
  ) {
    if self.mentions_unknown(element) {
      return;
    }
    for member in members {
      let value = self.expression_type(scope, source, *member);
      if self.implicit_conversion(&value, element).is_none() {
        let Some(span) = self.ast(source).map(|ast| ast.node(*member).span) else {
          continue;
        };
        self.report_mismatch(source, span, element, value.type_id);
      }
    }
  }
}

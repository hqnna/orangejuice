use oj_diag::SourceId;
use oj_scope::ScopeId;
use oj_syntax::ast::{CastFlags, NodeId};
use oj_types::{ArrayKind, TypeId, TypeKind};

use crate::checker::{Checker, Expr};

impl Checker<'_> {
  /// Types an explicit cast and reports the ones the reference rejects
  /// (**L§5.6**). `xx` has no target of its own: it takes the type the context
  /// asks for, which is decided where the value is used.
  pub(crate) fn cast_type(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    cast: &oj_syntax::ast::Cast,
  ) -> Expr {
    let value = self.expression_type(scope, source, cast.expression);
    let span = self.ast(source).map(|ast| ast.node(node).span);

    // `trunc` truncates without checking, so asking for `no_check` too says
    // two different things.
    if cast
      .cast_flags
      .contains(CastFlags::TRUNCATE | CastFlags::NO_BOUNDS_CHECK)
      && let Some(span) = span
    {
      self.error(
        source,
        span,
        "This cast has inconsistent modifiers. (It is flagged both 'trunc' and 'no_check').",
      );
    }

    let Some(target) = cast.target_type else {
      return Expr::UNKNOWN;
    };
    let target = self.type_from_node(scope, source, target);
    if let Some(span) = span {
      self.check_cast(source, span, &value, target, cast.cast_flags);
    }
    Expr {
      explicitly_cast: true,
      ..Expr::value(target)
    }
  }

  fn check_cast(
    &mut self,
    source: SourceId,
    span: oj_diag::Span,
    value: &Expr,
    target: TypeId,
    flags: CastFlags,
  ) {
    let from = value.type_id;
    if self.mentions_unknown(from) || self.mentions_unknown(target) || from == target {
      return;
    }
    if matches!(self.types().kind(from), TypeKind::Polymorph(_))
      || matches!(self.types().kind(target), TypeKind::Polymorph(_))
    {
      return;
    }

    if target == TypeId::VOID {
      let from = self.harden(from);
      if self.layout_of(from).is_some_and(|layout| layout.size > 0) {
        self.error(
          source,
          span,
          "Casting a non-zero-sized value to void is invalid.",
        );
      }
      return;
    }

    // A pointer is one address; a view is a count and an address (**L§3.3**).
    if self.types().is_pointer(from)
      && matches!(
        self.types().array_of(target),
        Some((_, ArrayKind::View | ArrayKind::Resizable))
      )
    {
      self.error(
        source,
        span,
        "Cannot cast from a pointer to a non-fixed array type.",
      );
      return;
    }

    if from == TypeId::STRING && self.types().is_numeric(target) {
      let target = self.type_name(target);
      self.error(
        source,
        span,
        format!("String cannot cast to this type (the target type is {target}.)"),
      );
      return;
    }

    // Reinterpreting one struct as another needs the intent spelled out; a
    // conversion the compiler would have made anyway does not (**L§5.6**).
    if flags.intersects(CastFlags::FORCE | CastFlags::VERY_FORCE) {
      return;
    }
    if self.types().struct_of(from).is_some()
      && self.types().struct_of(target).is_some()
      && self.types().underlying(from) != self.types().underlying(target)
      && self.implicit_conversion(value, target).is_none()
    {
      let (wanted, given) = (self.type_name(target), self.type_name(from));
      let (wanted_size, given_size) = (
        self.layout_of(target).map_or(0, |layout| layout.size),
        self.layout_of(from).map_or(0, |layout| layout.size),
      );
      self.error(
        source,
        span,
        format!(
          "Cannot cast from one struct to another without force modifiers. \
           Type wanted: {wanted} ({wanted_size} bytes); type given: {given} ({given_size} bytes)"
        ),
      );
    }
  }
}

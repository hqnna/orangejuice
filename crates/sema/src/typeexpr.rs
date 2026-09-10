use oj_diag::SourceId;
use oj_scope::ScopeId;
use oj_syntax::ast::{InstFlags, NodeData, NodeId};
use oj_types::{ArrayKind, FloatKind, PolymorphInfo, TypeId, TypeKind, VariantFlags, VariantInfo};

use crate::checker::Checker;

impl Checker<'_> {
  /// The type a type slot stands for. Every type slot is a
  /// `TYPE_INSTANTIATION` (**C§5.3**), whose shape says whether it is a
  /// pointer, an array, a `#type` directive or an arbitrary type-valued
  /// expression.
  pub(crate) fn type_from_node(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> TypeId {
    if !self.enter() {
      return TypeId::UNKNOWN;
    }
    let result = self.type_from_node_inner(scope, source, node);
    self.leave();
    result
  }

  fn type_from_node_inner(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> TypeId {
    let Some(ast) = self.ast(source) else {
      return TypeId::UNKNOWN;
    };
    // A procedure type in a type slot needs no `#type` (**L§3.7**):
    // `proc: (x: int) -> int;` and `initializer: (*void) #no_context;` are the
    // header itself.
    if matches!(ast.data(node), NodeData::ProcedureHeader(_)) {
      return self.procedure_type(source, node, scope);
    }
    let NodeData::TypeInstantiation(inst) = ast.data(node) else {
      // A type written where an expression was expected: a struct literal's
      // type slot lands here.
      let value = self.expression_type(scope, source, node);
      if let Some(denoted) = value.denoted {
        return denoted;
      }
      // A value that *is* a type names that type: `b.T` reads the argument a
      // polymorphic struct was baked with off a variable of it (**L§8.5**).
      return value
        .constant
        .and_then(|value| value.as_type())
        .unwrap_or(TypeId::UNKNOWN);
    };

    if let Some(pointee) = inst.pointer_to {
      let pointee = self.type_from_node(scope, source, pointee);
      return self.types_mut().pointer_to(pointee);
    }

    if let Some(element) = inst.array_element_type {
      let element = self.type_from_node(scope, source, element);
      // `..T` in a parameter builds the callee's `[] T` (**L§7.4**).
      let kind = if inst.inst_flags.contains(InstFlags::VARARGS)
        || inst.inst_flags.contains(InstFlags::ARRAY_VIEW)
      {
        ArrayKind::View
      } else if inst.inst_flags.contains(InstFlags::RESIZABLE) {
        ArrayKind::Resizable
      } else {
        match inst
          .array_dimension
          .and_then(|dimension| self.const_int(scope, source, dimension))
        {
          Some(count) if count >= 0 => ArrayKind::Fixed(count as u64),
          // `[$N] T` and dimensions that need the interpreter are not decided
          // here; the array stays unknown rather than wrong.
          _ => return TypeId::UNKNOWN,
        }
      };
      return self.types_mut().array(element, kind);
    }

    if let Some(target) = inst.type_directive_target {
      let base = self.type_from_node(scope, source, target);
      let flags = if inst.inst_flags.contains(InstFlags::TYPE_DIRECTIVE_DISTINCT) {
        VariantFlags::DISTINCT
      } else if inst.inst_flags.contains(InstFlags::TYPE_DIRECTIVE_ISA) {
        VariantFlags::ISA
      } else {
        // A bare `#type expr` only forces the parse; it makes no new type
        // (**L§3.7**).
        return base;
      };
      if base == TypeId::UNKNOWN {
        return TypeId::UNKNOWN;
      }
      if let Some(existing) = self.aggregate_type_in(scope, source, node) {
        return existing;
      }
      let (_, type_id) = self.types_mut().new_variant(VariantInfo {
        name: None,
        base,
        flags,
      });
      self.record_aggregate_type_in(scope, source, node, type_id);
      return type_id;
    }

    let Some(expression) = inst.type_valued_expression else {
      return TypeId::UNKNOWN;
    };
    let must_implement = inst.must_implement;
    let interface = inst.inst_flags.contains(InstFlags::INTERFACE);

    let type_id = self.type_from_node(scope, source, expression);
    // `$T/Entity` restricts what the variable may be solved as: the argument's
    // type has to be `Entity` or something that reaches it through `using`/`#as`
    // (**L§7.8**).
    if let Some(restriction) = must_implement
      && matches!(self.types().kind(type_id), TypeKind::Polymorph(_))
    {
      // A restriction written as a constant array of types allows any one of
      // them: `$T/MyVectors` over `MyVectors :: Type.[Vector2, Vector3]`
      // (**L§7.8**), which is what `Math.normalize` tells its two overloads
      // apart with.
      let alternatives = self.restriction_alternatives(scope, source, restriction);
      let restriction = self.type_from_node(scope, source, restriction);
      if let TypeKind::Polymorph(definition) = *self.types().kind(type_id) {
        let info = self.types_mut().polymorph_info_mut(definition);
        info.restriction = Some(restriction);
        info.interface = interface;
        info.alternatives = alternatives;
      }
    }
    type_id
  }

  /// The types a `$T/Name` restriction allows when `Name` is a constant array
  /// of them rather than one type (**L§7.8**). Empty when it names a type.
  fn restriction_alternatives(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    restriction: NodeId,
  ) -> Vec<TypeId> {
    let mut written = self.constant_array_written_at(scope, source, restriction);
    if written.is_none() && self.is_array_literal(source, restriction) {
      written = Some((source, restriction, scope));
    }
    let Some((written_source, written_node, written_scope)) = written else {
      return Vec::new();
    };
    let Some(NodeData::Literal(literal)) =
      self.ast(written_source).map(|ast| ast.data(written_node))
    else {
      return Vec::new();
    };
    let oj_syntax::ast::LiteralValue::Array(array) = &literal.value else {
      return Vec::new();
    };
    let members = array.members.clone();
    let mut allowed = Vec::with_capacity(members.len());
    for member in members {
      let type_id = self.type_from_node(written_scope, written_source, member);
      if !self.types().is_unknown(type_id) {
        allowed.push(type_id);
      }
    }
    allowed
  }

  fn is_array_literal(&self, source: SourceId, node: NodeId) -> bool {
    matches!(
      self.ast(source).map(|ast| ast.data(node)),
      Some(NodeData::Literal(literal))
        if matches!(literal.value, oj_syntax::ast::LiteralValue::Array(_))
    )
  }

  /// Declares the polymorph variable `name` written at `node`, once per site.
  pub(crate) fn polymorph_type(
    &mut self,
    source: SourceId,
    node: NodeId,
    name: oj_lexer::Symbol,
  ) -> TypeId {
    if let Some(existing) = self.aggregate_type(source, node) {
      return existing;
    }
    let (_, type_id) = self.types_mut().new_polymorph(PolymorphInfo {
      name,
      restriction: None,
      interface: false,
      alternatives: Vec::new(),
    });
    self.record_aggregate_type(source, node, type_id);
    type_id
  }

  /// Gives an untyped literal the type it defaults to when nothing else asks
  /// for one: `s64` for integers, its own width for floats (**L§5.10**).
  pub(crate) fn harden(&self, type_id: TypeId) -> TypeId {
    match self.types().kind(type_id) {
      TypeKind::UntypedInt => TypeId::S64,
      TypeKind::UntypedFloat(FloatKind::F32) => TypeId::FLOAT32,
      TypeKind::UntypedFloat(FloatKind::F64) => TypeId::FLOAT64,
      TypeKind::UntypedEnum | TypeKind::UntypedLiteral | TypeKind::OverloadSet => TypeId::UNKNOWN,
      _ => type_id,
    }
  }
}

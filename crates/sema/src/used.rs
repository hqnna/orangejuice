//! Names a `using` of a value brings into a scope (**L§6.8**).
//!
//! `using p: *Player;` makes `health` mean `p.health`. The scope tree cannot
//! know that: the members of `p` are only names once `p` has a type, so it
//! records the value and leaves the lookup here. The answer is a member of
//! something the caller can address, which is what lets a back end lower the
//! bare name the way it lowers the dot.

use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclId, ScopeId};
use oj_syntax::ast::{NodeData, NodeId};
use oj_types::{StructMember, TypeId};

use crate::checker::{Checker, Expr};

/// Where the bytes a `using`ed value's member sits in come from (**L§6.8**).
/// `using p;` names a declaration; `using o.inner;` names an expression, which
/// the back end evaluates the way it evaluates any other place.
#[derive(Clone, Copy, Debug)]
pub enum UsedBase {
  Declaration(DeclId),
  Expression {
    source: SourceId,
    node: NodeId,
    scope: ScopeId,
  },
}

/// What a bare name written next to a `using` of a value stands for
/// (**L§6.8**).
#[derive(Clone, Debug)]
pub struct UsedMember {
  /// Where the bytes are: the declaration the `using` named, or the expression
  /// it was written as.
  pub base: UsedBase,
  /// The type of that declaration, before any dereference.
  pub base_type: TypeId,
  /// Whether the base is a pointer that has to be followed first
  /// (**L§6.8**): `using p: *Player` reads through `p`.
  pub through_pointer: bool,
  pub member: StructMember,
  /// The struct the member belongs to, so that a nested constant can be
  /// looked up where it was declared rather than read out of storage.
  pub definition: oj_types::StructId,
}

impl Checker<'_> {
  /// The member a bare name stands for, when a `using` of a value in scope
  /// declares it (**L§6.8**).
  pub fn used_member(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<UsedMember> {
    let name = self.ident_name(source, node)?;
    let start = self.scope_at(source, node, scope);
    // A nested constant has no storage for a back end to address.
    self
      .resolve_used_member(start, name)
      .filter(|found| !found.member.is_constant())
  }

  pub(crate) fn resolve_used_member(&mut self, scope: ScopeId, name: Symbol) -> Option<UsedMember> {
    self.walk_used_values(scope, name, |checker, from, owner, value, name| {
      checker.member_of_used_value(from, owner, value, name)
    })
  }

  /// Runs `answer` against every `using`ed value on the chain out of `scope`,
  /// innermost first, and returns the first answer it gives.
  fn walk_used_values<T>(
    &mut self,
    scope: ScopeId,
    name: Symbol,
    answer: impl Fn(&mut Self, ScopeId, ScopeId, &oj_scope::UsedValue, Symbol) -> Option<T>,
  ) -> Option<T> {
    if !self.enter() {
      return None;
    }
    let mut found = None;
    for id in self.scopes_with_usings(scope) {
      let used: Vec<oj_scope::UsedValue> = self.program().tree().used_values(id).to_vec();
      for value in used {
        if !value.admits(name) {
          continue;
        }
        found = answer(self, scope, id, &value, name);
        if found.is_some() {
          break;
        }
      }
      if found.is_some() {
        break;
      }
    }
    self.leave();
    found
  }

  /// The scopes whose `using` of a value widens what `scope` can see: the ones
  /// it is written inside, and the modules it imports — a module that writes
  /// `using gl;` hands its importers the members of `gl` the way it hands them
  /// any other name (**L§6.8**, **L§11.2**), which is how `#import "GL"` makes
  /// `glClearColor` a name.
  fn scopes_with_usings(&self, scope: ScopeId) -> Vec<ScopeId> {
    let tree = self.program().tree();
    let mut order = Vec::new();
    let mut current = Some(scope);
    while let Some(id) = current {
      if !order.contains(&id) {
        order.push(id);
      }
      current = tree.parent(id);
    }
    let mut index = 0;
    while index < order.len() {
      let id = order[index];
      index += 1;
      for edge in tree.imports(id) {
        if !order.contains(&edge.target) {
          order.push(edge.target);
        }
      }
    }
    order
  }

  fn member_of_used_value(
    &mut self,
    from: ScopeId,
    owner: ScopeId,
    value: &oj_scope::UsedValue,
    name: Symbol,
  ) -> Option<UsedMember> {
    let (base, base_type) = match self.used_base(value) {
      Some(decl) => {
        // Asking a `using`ed value for its own type while that is what we are
        // working out is the lookup that started this, not a member. A member
        // of a struct that is being laid out is the same case from the other
        // side: asking for its type is what the layout is doing (**L§8.3**).
        let declared_in = self.program().tree().decl(decl).scope;
        if self.is_resolving(decl) || self.scope_is_completing(declared_in) {
          return None;
        }
        (UsedBase::Declaration(decl), self.decl_type(decl).value)
      }
      // `using o.inner;` names no declaration of its own, so the base is the
      // expression itself (**L§6.8**).
      None => self.used_expression_base(value, owner)?,
    };
    let (pointee, through_pointer) = match self.types().pointee(base_type) {
      Some(pointee) => (pointee, true),
      None => (base_type, false),
    };
    let definition = self.types().struct_of(pointee)?;
    // A procedure written inside a struct's body may `using` a pointer to that
    // struct (`act :: (using self: *Thing)`). Building the struct is what
    // reaches the procedure, so asking for its members from in there would
    // loop; the name stays unresolved until the struct stands on its own
    // (**L§8.3**).
    if !self.types().is_complete(pointee)
      && (self.is_completing(definition) || self.encloses_lookup(definition, from))
    {
      return None;
    }
    self.complete_struct(definition);
    let member = self.types().struct_info(definition).member(name)?.clone();
    Some(UsedMember {
      base,
      base_type,
      through_pointer,
      member,
      definition,
    })
  }

  /// The struct a `using`ed value is, following one pointer.
  fn struct_of_used_value(
    &mut self,
    owner: ScopeId,
    value: &oj_scope::UsedValue,
  ) -> Option<oj_types::StructId> {
    let base_type = match self.used_base(value) {
      Some(decl) => {
        let declared_in = self.program().tree().decl(decl).scope;
        if self.is_resolving(decl) || self.scope_is_completing(declared_in) {
          return None;
        }
        self.decl_type(decl).value
      }
      None => self.used_expression_base(value, owner)?.1,
    };
    let pointee = self.types().pointee(base_type).unwrap_or(base_type);
    self.types().struct_of(pointee)
  }

  /// What a `using`ed polymorphic struct's own argument list contributes: the
  /// constants its specialization was baked with, which live in the arguments
  /// scope above the members (**L§8.5**).
  fn parameter_of_used_value(
    &mut self,
    owner: ScopeId,
    value: &oj_scope::UsedValue,
    name: Symbol,
  ) -> Option<Expr> {
    let definition = self.struct_of_used_value(owner, value)?;
    let instance = self.struct_instance(definition)?;
    let members = self.struct_scope(definition)?;
    let arguments = self.program().tree().parent(members)?;
    let found = self.with_instance(Some(instance), |checker| {
      checker.member_in_scope(arguments, name)
    });
    match found.is_unknown() {
      true => None,
      false => Some(found),
    }
  }

  /// Whether the lookup was written inside the struct's own body.
  fn encloses_lookup(&self, definition: oj_types::StructId, from: ScopeId) -> bool {
    match self.struct_scope(definition) {
      Some(members) => self.scope_encloses(members, from),
      None => false,
    }
  }

  /// The declaration a `using` of an already-declared value names.
  fn declaration_named(&mut self, source: SourceId, expression: NodeId) -> Option<DeclId> {
    let node = match self.ast(source)?.data(expression) {
      NodeData::Ident(_) => expression,
      _ => return None,
    };
    let name = self.ident_name(source, node)?;
    let scope = self.scope_of(source, node)?;
    match self.program().tree().lookup(scope, name) {
      oj_scope::Resolution::Found(candidates) => candidates.first().copied(),
      _ => None,
    }
  }

  /// What a bare name stands for when a `using` in scope brings it in: a
  /// member of a used value, or a name of a used *type* — `using
  /// Type_Info_Tag;` makes `STRING` one of its members (**L§6.8**).
  pub(crate) fn used_name_type(&mut self, scope: ScopeId, name: Symbol) -> Option<Expr> {
    self.walk_used_values(scope, name, |checker, from, owner, value, name| {
      if let Some(found) = checker.name_of_used_type(value, owner, name) {
        return Some(found);
      }
      // A `using` of a polymorphic struct brings in its *parameters* as well
      // as its members: `using array: Bucket_Array` is what makes
      // `items_per_bucket` a name inside `operator *[]` (**L§6.8**,
      // **L§8.5**).
      if let Some(found) = checker.parameter_of_used_value(owner, value, name) {
        return Some(found);
      }
      let member = checker.member_of_used_value(from, owner, value, name)?;
      // A nested constant — `Buffer :: struct { … }` inside `String_Builder`
      // — has no storage; it is what its declaration says (**L§8.3**).
      if member.member.is_constant() {
        return checker.nested_constant(member.definition, name);
      }
      Some(Expr::place(member.member.type_id))
    })
  }

  /// What a bare name stands for when a `using` of a value written inside a
  /// body brings it in, and nothing on the way out to that body declares the
  /// name itself (**L§6.8**). A `using` at file or module level is not one of
  /// these: what it widens is a program scope, and a name declared there
  /// competes with it the way any two declarations do.
  pub(crate) fn locally_used_name_type(&mut self, scope: ScopeId, name: Symbol) -> Option<Expr> {
    let tree = self.program().tree();
    let mut chain = Vec::new();
    let mut current = Some(scope);
    while let Some(id) = current {
      if tree.scope_kind(id).is_program_scope() {
        break;
      }
      chain.push(id);
      current = tree.parent(id);
    }
    // Nothing written inside the body can shadow anything.
    if chain
      .iter()
      .all(|id| self.program().tree().used_values(*id).is_empty())
    {
      return None;
    }
    for id in chain {
      // A declaration of the name on the way out is what the name means.
      if self
        .program()
        .tree()
        .declarations(id)
        .iter()
        .any(|decl| self.program().tree().decl(*decl).name == name)
      {
        return None;
      }
      let used: Vec<oj_scope::UsedValue> = self.program().tree().used_values(id).to_vec();
      for value in used {
        if !value.admits(name) {
          continue;
        }
        // Only a `using` whose value already has a type. Working one out here
        // would be a lookup of its own, and this runs for every name a body
        // mentions: a `using` still being resolved answers nothing, and the
        // ordinary walk out is what the name means until it does.
        if !self
          .used_base(&value)
          .is_some_and(|base| self.resolved(base).is_some())
        {
          continue;
        }
        if let Some(member) = self.member_of_used_value(scope, id, &value, name) {
          if member.member.is_constant() {
            return self.nested_constant(member.definition, name);
          }
          return Some(Expr::place(member.member.type_id));
        }
      }
    }
    None
  }

  /// A constant declared inside a struct's body, resolved where it was
  /// written (**L§8.3**).
  fn nested_constant(&mut self, definition: oj_types::StructId, name: Symbol) -> Option<Expr> {
    let scope = self.struct_scope(definition)?;
    let oj_scope::Resolution::Found(candidates) = self.program().tree().lookup(scope, name) else {
      return None;
    };
    // A baked polymorphic struct's nested declarations belong to its
    // instantiation, not to whichever one is being checked (**L§8.5**).
    Some(match self.struct_instance(definition) {
      Some(instance) => self.with_instance(Some(instance), |checker| {
        checker.declarations_type(&candidates)
      }),
      None => self.declarations_type(&candidates),
    })
  }

  /// A name a `using` of a *type* brings in: an enum's members, or a struct's
  /// nested constants (**L§6.8**).
  fn name_of_used_type(
    &mut self,
    value: &oj_scope::UsedValue,
    owner: ScopeId,
    name: Symbol,
  ) -> Option<Expr> {
    let denoted = match self.used_base(value) {
      Some(base) => {
        if self.is_resolving(base) {
          return None;
        }
        self.decl_type(base).denoted?
      }
      // `using Code_Node.Kind;` names a type without naming a declaration of
      // any scope of ours, so the expression itself is what says which type
      // (**L§6.8**).
      None => {
        let scope = self.scope_at(value.source, value.expression, owner);
        self
          .expression_type(scope, value.source, value.expression)
          .denoted?
      }
    };
    let scope = match *self.types().kind(denoted) {
      oj_types::TypeKind::Enum(definition) => self.enum_scope(definition),
      oj_types::TypeKind::Struct(definition) => self.struct_scope(definition),
      _ => None,
    }?;
    match self.program().tree().lookup(scope, name) {
      oj_scope::Resolution::Found(candidates) => Some(self.declarations_type(&candidates)),
      _ => None,
    }
  }

  /// The declaration a `using` names, whether it declared it itself or refers
  /// to one made elsewhere.
  fn used_base(&mut self, value: &oj_scope::UsedValue) -> Option<DeclId> {
    match value.decl {
      Some(decl) => Some(decl),
      None => self.declaration_named(value.source, value.expression),
    }
  }

  /// The base of a `using` that names no declaration — `using o.inner;`, or a
  /// `using` of a call's result — and the type that expression has
  /// (**L§6.8**).
  fn used_expression_base(
    &mut self,
    value: &oj_scope::UsedValue,
    owner: ScopeId,
  ) -> Option<(UsedBase, TypeId)> {
    let source = value.source;
    let node = value.expression;
    let scope = self.scope_at(source, node, owner);
    let expression = self.expression_type(scope, source, node);
    if expression.is_unknown() {
      return None;
    }
    Some((
      UsedBase::Expression {
        source,
        node,
        scope,
      },
      expression.type_id,
    ))
  }
}

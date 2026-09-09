use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclKind, Resolution, ScopeId};
use oj_syntax::ast::{NodeData, NodeId};
use oj_types::{ArrayKind, MemberFlags, TypeId};

use crate::checker::{Checker, Expr};
use crate::constants::{Const, Value};

impl Checker<'_> {
  /// `a.b` (**L§5.3**): a name in a module, a constant or nested type reached
  /// through a type, a member of a struct value (through at most one pointer),
  /// an enum member, or the built-in fields of a string or array.
  pub(crate) fn member_access(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    left: NodeId,
    right: NodeId,
  ) -> Expr {
    let Some(name) = self.ident_name(source, right) else {
      return Expr::UNKNOWN;
    };

    if let Some(module) = self.module_scope(scope, source, left) {
      let candidates = self.program().tree().lookup_in_module(module, name);
      return self.declarations_type(&candidates);
    }

    let base = self.expression_type(scope, source, left);
    if let Some(denoted) = base.denoted {
      return self.member_of_type(denoted, name);
    }
    if base.is_unknown() {
      return Expr::UNKNOWN;
    }

    // Member access through a pointer dereferences one level (**L§3.2**).
    let value_type = self.types().pointee(base.type_id).unwrap_or(base.type_id);
    let lvalue = base.lvalue || self.types().is_pointer(base.type_id);
    self.complete_type(value_type);

    if let Some((element, kind)) = self.types().array_of(value_type)
      && let Some(built_in) = self.array_field(name, element, kind, lvalue)
    {
      return built_in;
    }

    // An `Any` is a pair, and Preload writes down what its two members are
    // called (**L§3.8**).
    let value_type = match self.types().kind(value_type) {
      oj_types::TypeKind::Any => self.any_struct_type(),
      _ => value_type,
    };

    if let Some(definition) = self.types().struct_of(value_type) {
      let member = self.types().struct_info(definition).member(name).cloned();
      if let Some(member) = member {
        if member.flags.contains(MemberFlags::CONSTANT) {
          return self.member_of_type(value_type, name);
        }
        return Expr {
          type_id: member.type_id,
          denoted: None,
          constant: None,
          lvalue,
          overloads: Vec::new(),
          explicitly_cast: false,
          autocast: false,
        };
      }
      // A nested type or procedure reached through a value (**L§8.3**).
      return self.member_of_type(value_type, name);
    }

    if self.types().enum_of(value_type).is_some() {
      return self.member_of_type(value_type, name);
    }

    Expr::UNKNOWN
  }

  /// `Any_Struct`, which is what an `Any` looks like (**L§17**).
  pub fn any_struct_type(&mut self) -> TypeId {
    let name = self.interned().intern(b"Any_Struct");
    let type_id = self.preload_type(name);
    self.complete_type(type_id);
    type_id
  }

  /// `T.NAME`: an enum member, or a constant, nested type or procedure
  /// declared in the type's body.
  fn member_of_type(&mut self, type_id: TypeId, name: Symbol) -> Expr {
    let underlying = self.types().underlying(type_id);
    if let Some(definition) = self.types().enum_of(underlying) {
      if let Some(value) = self.types().enum_info(definition).value_of(name) {
        return Expr::constant(Const::new(type_id, Value::Int(i128::from(value))));
      }
      // `E.loose` is `E` widened to accept any value of its base type
      // (**L§9**); it has the same representation, so it stands for `E` here.
      if name == self.loose_name() {
        return Expr::type_expression(type_id);
      }
      if let Some(scope) = self.enum_scope(definition) {
        return self.member_in_scope(scope, name);
      }
      return Expr::UNKNOWN;
    }
    if let Some(definition) = self.types().struct_of(underlying) {
      // The body's own scope answers first, and without laying the struct out:
      // `STRING_BUILDER_BUFFER_SIZE :: 4096 - size_of(String_Builder.Buffer);`
      // asks a struct for a nested type while that struct's own size is still
      // waiting on the answer (**L§8.3**).
      if let Some(scope) = self.struct_scope(definition) {
        // A nested declaration of a baked polymorphic struct is resolved under
        // the instantiation it belongs to, so that `Table(string, string).Entry`
        // is one type however it was reached (**L§8.5**).
        let instance = self.struct_instance(definition);
        let found = match instance {
          Some(instance) => self.with_instance(Some(instance), |checker| {
            checker.member_in_scope(scope, name)
          }),
          None => self.member_in_scope(scope, name),
        };
        if !found.is_unknown() {
          return found;
        }
      }
      // A constant a `using` member imported is only in the flattened list.
      self.complete_type(underlying);
      let member = self
        .types()
        .struct_info(definition)
        .member(name)
        .cloned()
        .filter(|member| member.flags.contains(MemberFlags::CONSTANT));
      if let Some(member) = member {
        return Expr::value(member.type_id);
      }
    }
    Expr::UNKNOWN
  }

  fn member_in_scope(&mut self, scope: ScopeId, name: Symbol) -> Expr {
    let candidates = self
      .program()
      .tree()
      .names_in(scope, name)
      .unwrap_or_default();
    if candidates.is_empty() {
      return Expr::UNKNOWN;
    }
    self.declarations_type(&candidates)
  }

  /// `.count`, `.data`, and the two extra fields of a resizable array
  /// (**L§3.3**, **L§3.4**).
  fn array_field(
    &mut self,
    name: Symbol,
    element: TypeId,
    kind: ArrayKind,
    lvalue: bool,
  ) -> Option<Expr> {
    if name == self.count_name() {
      return Some(match kind {
        // A fixed array's count is a constant and is not assignable.
        ArrayKind::Fixed(count) => {
          Expr::constant(Const::new(TypeId::S64, Value::Int(i128::from(count))))
        }
        _ => Expr {
          type_id: TypeId::S64,
          denoted: None,
          constant: None,
          lvalue,
          overloads: Vec::new(),
          explicitly_cast: false,
          autocast: false,
        },
      });
    }
    if name == self.data_name() {
      let pointer = self.types_mut().pointer_to(element);
      // A fixed array's `.data` is its storage and is not assignable
      // (**L§3.3**).
      return Some(Expr {
        type_id: pointer,
        denoted: None,
        constant: None,
        lvalue: lvalue && !matches!(kind, ArrayKind::Fixed(_)),
        overloads: Vec::new(),
        explicitly_cast: false,
        autocast: false,
      });
    }
    if kind == ArrayKind::Resizable {
      if name == self.allocated_name() {
        return Some(Expr::place(TypeId::S64));
      }
      if name == self.allocator_name() {
        let allocator = self.interned().intern(b"Allocator");
        let type_id = self.preload_type(allocator);
        return Some(Expr::place(type_id));
      }
    }
    None
  }

  /// The scope a named `#import` binding stands for, when `left` is one
  /// (**L§11.2**).
  fn module_scope(&mut self, scope: ScopeId, source: SourceId, left: NodeId) -> Option<ScopeId> {
    let ast = self.ast(source)?;
    let NodeData::Ident(ident) = ast.data(left) else {
      return None;
    };
    let scope = self.scope_at(source, left, scope);
    let Resolution::Found(candidates) = self.program().tree().lookup(scope, ident.name) else {
      return None;
    };
    candidates
      .into_iter()
      .find_map(|id| match self.program().tree().decl(id).kind {
        DeclKind::Module(target) => Some(target),
        _ => None,
      })
  }
}

use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclKind, Resolution, ScopeId};
use oj_syntax::ast::{NodeData, NodeId};
use oj_types::{ArrayKind, MemberFlags, TypeId};

use crate::checker::{Checker, Expr};
use crate::constants::{Address, AddressOf, Const, Value};

/// The text a constant string is (**L§5.11**). A `Value::Bytes` is not one:
/// for a view those bytes are the `{count, data}` pair rather than what it
/// points at, so nothing here can read the elements out of them.
fn constant_text(value: &Expr) -> Option<Box<[u8]>> {
  match &value.constant.as_ref()?.value {
    Value::String(text) => Some(text.clone()),
    _ => None,
  }
}

/// The bytes a constant's storage *is*, for an array whose elements are laid
/// out in it: a string's text, or a fixed array literal's own storage.
fn constant_elements(value: &Expr, kind: ArrayKind) -> Option<Box<[u8]>> {
  match &value.constant.as_ref()?.value {
    Value::String(text) => Some(text.clone()),
    Value::Bytes(bytes) if matches!(kind, ArrayKind::Fixed(_)) => Some(bytes.data.clone()),
    _ => None,
  }
}

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
      let found = self.member_of_type(denoted, name);
      if !found.is_unknown() {
        return found;
      }
      return self.unknown_member(source, right, denoted, name);
    }
    if base.is_unknown() {
      return Expr::UNKNOWN;
    }

    // A constant `Code`'s `type` is the type of the expression it quotes,
    // worked out where that was written (**L§13.1**, **C§5.3**): `f :: ($c:
    // Code) { T :: c.type; }` is how a body asks what it was handed.
    if base.type_id == TypeId::CODE
      && name == self.interned().intern(b"type")
      && let Some(Value::Code {
        source: code_source,
        node,
        scope: code_scope,
      }) = base.constant.as_ref().map(|value| value.value.clone())
    {
      let quoted = self.expression_type(code_scope, code_source, node);
      if !quoted.is_unknown() {
        return Expr::type_expression(self.harden(quoted.type_id));
      }
    }

    // Member access through a pointer dereferences one level (**L§3.2**).
    let value_type = self.types().pointee(base.type_id).unwrap_or(base.type_id);
    let lvalue = base.lvalue || self.types().is_pointer(base.type_id);
    self.complete_type(value_type);

    if let Some((element, kind)) = self.types().array_of(value_type)
      && let Some(built_in) = self.array_field(name, element, kind, lvalue, &base)
    {
      return built_in;
    }

    // `type_info(T).type` and `.runtime_size` are constants: the compiler put
    // them there (**L§5.11**, **L§17**).
    if let Some(Value::Address(address)) = base.constant.as_ref().map(|value| &value.value)
      && let AddressOf::TypeInfo(queried) = address.at
      && let Some(known) = self.type_info_field(queried, value_type, name)
    {
      return Expr::constant(known);
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
          overload_instance: None,
          explicitly_cast: false,
          autocast: false,
        };
      }
      // A nested type or procedure reached through a value (**L§8.3**).
      let found = self.member_of_type(value_type, name);
      if !found.is_unknown() {
        return found;
      }
      return self.unknown_member(source, right, value_type, name);
    }

    if self.types().enum_of(value_type).is_some() {
      let found = self.member_of_type(value_type, name);
      if !found.is_unknown() {
        return found;
      }
      return self.unknown_member(source, right, value_type, name);
    }

    self.unknown_member(source, right, value_type, name)
  }

  /// A member access that named nothing, where the type it was written against
  /// *is* known: `p.z` on a struct with no `z` (**L§5.3**). Nothing further out
  /// can supply the name — a struct's members are not searched outward — so the
  /// front end reports it here rather than leaving an `unknown` for the back
  /// end to fail on.
  ///
  /// A type the checker has not worked out yet says nothing, the same rule an
  /// argument of unknown type follows at a call site: one error should not turn
  /// into a second one about the type it left behind.
  fn unknown_member(
    &mut self,
    source: SourceId,
    right: NodeId,
    base: TypeId,
    name: Symbol,
  ) -> Expr {
    if !self.checking_a_body() || self.undecided_type(base) {
      return Expr::UNKNOWN;
    }
    // `#Context` is open: any module of the program may `#add_context` a member
    // to it (**L§10.2**), so a name that misses there is one the rest of the
    // program may still declare rather than one nothing has.
    if base == self.context_type() {
      return Expr::UNKNOWN;
    }
    let Some(span) = self.span_of(source, right) else {
      return Expr::UNKNOWN;
    };
    let written = self.symbol_text(name);
    let printed = self.type_name(base);
    self.report_once(
      source,
      span,
      format!("'{written}' is not a member of '{printed}'."),
    );
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
          // Whatever a call site does with the overloads it found has to be
          // done under the same instantiation: a `#bake_constants` written in
          // the body reads `#this` as that specialization (**L§8.5**).
          return Expr {
            overload_instance: instance.or(found.overload_instance),
            ..found
          };
        }
        // A baked struct's *parameters* are reachable through it too:
        // `floats.N` reads the `N` of `Holder(float, 5)`, which lives in the
        // arguments scope above the members (**L§8.5**). Nothing above that is,
        // which is what stops `a.OUTER_VALUE` (**L§8.3**).
        if let Some(instance) = instance
          && let Some(arguments) = self.program().tree().parent(scope)
        {
          let found = self.with_instance(Some(instance), |checker| {
            checker.member_in_scope(arguments, name)
          });
          if !found.is_unknown() {
            return found;
          }
        }
      }
      // A constant a `using` member imported is only in the flattened list; its
      // *value* lives in the struct it came from, which is where
      // `context.default_allocator` is (**L§8.4**).
      self.complete_type(underlying);
      let member = self
        .types()
        .struct_info(definition)
        .member(name)
        .cloned()
        .filter(|member| member.flags.contains(MemberFlags::CONSTANT));
      if let Some(member) = member {
        if let Some(through) = member.imported_through
          && let Some(base) = self
            .types()
            .struct_info(definition)
            .members
            .get(through)
            .map(|member| member.type_id)
        {
          let base = self.types().pointee(base).unwrap_or(base);
          let found = self.member_of_type(base, name);
          if !found.is_unknown() {
            return found;
          }
        }
        return Expr::value(member.type_id);
      }
      // A member a `using` brought in is not in the struct's own scope, only in
      // the flattened list (**L§8.4**), so `type_of(Header.magic)` over a
      // `using data: struct { magic: … }` is answered from there.
      if let Some(member) = self.types().struct_info(definition).member(name).cloned() {
        return Expr::value(member.type_id);
      }
    }
    Expr::UNKNOWN
  }

  pub(crate) fn member_in_scope(&mut self, scope: ScopeId, name: Symbol) -> Expr {
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
    base: &Expr,
  ) -> Option<Expr> {
    if name == self.count_name() {
      return Some(match kind {
        // A fixed array's count is a constant and is not assignable.
        ArrayKind::Fixed(count) => {
          Expr::constant(Const::new(TypeId::S64, Value::Int(i128::from(count))))
        }
        // A constant string's count is a constant too, though its type is a
        // view (**L§5.11**).
        _ => match constant_text(base) {
          Some(text) => Expr::constant(Const::new(TypeId::S64, Value::Int(text.len() as i128))),
          None => Expr {
            type_id: TypeId::S64,
            denoted: None,
            constant: None,
            lvalue,
            overloads: Vec::new(),
            overload_instance: None,
            explicitly_cast: false,
            autocast: false,
          },
        },
      });
    }
    if name == self.data_name() {
      let pointer = self.types_mut().pointer_to(element);
      // `.data` of a constant string or array literal, and of a *global* fixed
      // array, is an address the linker settles — constant, though nothing
      // here knows the number (**L§5.11**). A local's storage is not.
      // `.data` of an *empty* array literal is null rather than an address, and
      // that null is the constant (**L§5.11**).
      // An empty array has no storage, so its `.data` is the null the reference
      // reports rather than an address (**L§5.11**).
      let empty = kind == ArrayKind::Fixed(0)
        || constant_elements(base, kind).is_some_and(|bytes| bytes.is_empty());
      let constant = match constant_elements(base, kind).filter(|bytes| !bytes.is_empty()) {
        Some(bytes) => Some(Address {
          at: AddressOf::Data(bytes),
          offset: 0,
        }),
        None => match matches!(kind, ArrayKind::Fixed(_)) {
          true => self.global_named_by(base).map(|decl| Address {
            at: AddressOf::Global(decl),
            offset: 0,
          }),
          false => None,
        },
      };
      // A fixed array's `.data` is its storage and is not assignable
      // (**L§3.3**).
      return Some(Expr {
        type_id: pointer,
        denoted: None,
        constant: match empty {
          true => Some(Const::new(pointer, Value::Null)),
          false => constant.map(|address| Const::new(pointer, Value::Address(address))),
        },
        lvalue: lvalue && !matches!(kind, ArrayKind::Fixed(_)),
        overloads: Vec::new(),
        overload_instance: None,
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

  /// The two members of a `Type_Info` the compiler settles rather than the
  /// program: the tag it carries and the size of the type it describes
  /// (**L§17**).
  fn type_info_field(&mut self, queried: TypeId, record: TypeId, name: Symbol) -> Option<Const> {
    let text = self.interned().resolve_lossy(name).into_owned();
    let definition = self.types().struct_of(self.types().underlying(record))?;
    let member = self
      .types()
      .struct_info(definition)
      .member(name)
      .map(|member| member.type_id)?;
    match text.as_str() {
      "type" => {
        let tag = self.type_info_tag(queried)?;
        Some(Const::new(member, Value::Int(i128::from(tag))))
      }
      "runtime_size" => {
        let size = self.layout(queried).map(|layout| layout.size)?;
        Some(Const::new(member, Value::Int(i128::from(size))))
      }
      _ => None,
    }
  }

  /// The `Type_Info_Tag` a type carries (**L§17**), read out of the enum
  /// Preload declares rather than written down here.
  fn type_info_tag(&mut self, queried: TypeId) -> Option<i64> {
    let name = tag_name(self.types().kind(self.types().underlying(queried)));
    let tag_type = self.preload_named_type("Type_Info_Tag");
    let definition = self.types().enum_of(self.types().underlying(tag_type))?;
    let symbol = self.interned().intern(name.as_bytes());
    self.types().enum_info(definition).value_of(symbol)
  }

  /// The global declaration an expression names, when it names one. A local's
  /// storage moves with the frame, so only a global's address is a constant
  /// (**L§5.11**).
  pub(crate) fn global_named_by(&mut self, value: &Expr) -> Option<oj_scope::DeclId> {
    let [only] = value.overloads[..] else {
      return None;
    };
    let tree = self.program().tree();
    let decl = tree.decl(only);
    let global = decl.kind == DeclKind::Variable
      && !matches!(
        tree.scope_kind(decl.scope),
        oj_scope::ScopeKind::Imperative
          | oj_scope::ScopeKind::ProcedureArguments
          | oj_scope::ScopeKind::ProcedureReturns
      );
    global.then_some(only)
  }

  /// The scope a named `#import` binding stands for, when `left` is one
  /// (**L§11.2**).
  fn module_scope(&mut self, scope: ScopeId, source: SourceId, left: NodeId) -> Option<ScopeId> {
    let (mut source, mut node, mut scope) = (source, left, scope);
    // `P :: Posix;` names whatever `Posix` names, so an alias is followed to
    // the module it stands for (**L§11.2**).
    for _ in 0..16 {
      let ast = self.ast(source)?;
      let NodeData::Ident(ident) = ast.data(node) else {
        return None;
      };
      let name = ident.name;
      let at = self.scope_at(source, node, scope);
      let Resolution::Found(candidates) = self.program().tree().lookup(at, name) else {
        return None;
      };
      let found = candidates
        .iter()
        .find_map(|id| match self.program().tree().decl(*id).kind {
          DeclKind::Module(target) => Some(target),
          _ => None,
        });
      if found.is_some() {
        return found;
      }
      let alias = candidates.iter().copied().find_map(|id| {
        let info = self.program().tree().decl(id);
        if !info
          .flags
          .contains(oj_syntax::ast::DeclarationFlags::IS_CONSTANT)
        {
          return None;
        }
        let (alias_source, alias_node) = (info.source?, info.node?);
        let NodeData::Declaration(declaration) = self.ast(alias_source)?.data(alias_node) else {
          return None;
        };
        let expression = declaration.expression?;
        matches!(self.ast(alias_source)?.data(expression), NodeData::Ident(_)).then_some((
          alias_source,
          expression,
          info.scope,
        ))
      })?;
      (source, node, scope) = alias;
    }
    None
  }
}

/// The `Type_Info_Tag` member a type carries (**L§17**). The same table the
/// type table image is built from, kept here because the checker answers
/// `type_info(T).type` before anything is laid out.
fn tag_name(kind: &oj_types::TypeKind) -> &'static str {
  use oj_types::TypeKind;
  match kind {
    TypeKind::Integer(_) | TypeKind::UntypedInt => "INTEGER",
    TypeKind::Float(_) | TypeKind::UntypedFloat(_) => "FLOAT",
    TypeKind::Bool => "BOOL",
    TypeKind::String => "STRING",
    TypeKind::Pointer(_) => "POINTER",
    TypeKind::Procedure(_) => "PROCEDURE",
    TypeKind::Void => "VOID",
    TypeKind::Struct(_) => "STRUCT",
    TypeKind::Array { .. } => "ARRAY",
    TypeKind::OverloadSet => "OVERLOAD_SET",
    TypeKind::Any => "ANY",
    TypeKind::Enum(_) => "ENUM",
    TypeKind::Polymorph(_) => "POLYMORPHIC_VARIABLE",
    TypeKind::Type => "TYPE",
    TypeKind::Code => "CODE",
    TypeKind::UntypedLiteral => "UNTYPED_LITERAL",
    TypeKind::UntypedEnum => "UNTYPED_ENUM",
    TypeKind::Variant(_) => "VARIANT",
    _ => "VOID",
  }
}

use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclId, ScopeId, ScopeKind};
use oj_syntax::ast::{Declaration, DeclarationFlags, NodeData, NodeId, ProcedureFlags};
use oj_types::{ProcedureType, TypeId};

use crate::checker::{Checker, DeclType};

impl Checker<'_> {
  /// The type of one `Code_Declaration`: its type slot when it has one, else
  /// the type of the value it was given (**L§4.1**).
  pub(crate) fn declaration_type(
    &mut self,
    id: DeclId,
    decl_scope: ScopeId,
    source: SourceId,
    declaration: &Declaration,
  ) -> DeclType {
    // An enum member's value is decided by the enum, not by its own
    // expression, so asking for one builds the whole enum (**L§9**).
    if self.program().tree().scope(decl_scope).kind == ScopeKind::Enum
      && let Some((owner_source, owner_node)) = self.aggregate_owner(decl_scope)
    {
      let outer = self
        .program()
        .tree()
        .scope(decl_scope)
        .parent
        .unwrap_or(decl_scope);
      self.build_enum(None, None, owner_source, owner_node, outer);
      if let Some(resolved) = self.resolved(id) {
        return resolved;
      }
    }

    let name = declaration
      .name
      .and_then(|node| self.ident_name(source, node));

    if let Some(expression) = declaration.expression
      && let Some(ast) = self.ast(source)
    {
      match ast.data(expression) {
        NodeData::Struct(_) => {
          let type_id = self.build_struct(Some(id), name, source, expression, decl_scope);
          return DeclType::type_name(type_id);
        }
        NodeData::Enum(_) => {
          let type_id = self.build_enum(Some(id), name, source, expression, decl_scope);
          return DeclType::type_name(type_id);
        }
        NodeData::ProcedureHeader(_) => {
          let type_id = self.procedure_type(source, expression, decl_scope);
          return DeclType::value(type_id);
        }
        NodeData::DirectiveImport(_) | NodeData::Placeholder => return DeclType::UNKNOWN,
        _ => {}
      }
    }

    if let Some(type_inst) = declaration.type_inst {
      let declared = self.type_from_node(decl_scope, source, type_inst);
      // `T : Type : s32;` declares a type constant with its type spelled out.
      if declared == TypeId::TYPE
        && declaration.flags.contains(DeclarationFlags::IS_CONSTANT)
        && let Some(expression) = declaration.expression
      {
        let value = self.expression_type(decl_scope, source, expression);
        if let Some(denoted) = value.denoted {
          return DeclType::type_name(denoted);
        }
      }
      return DeclType::value(declared);
    }

    let Some(expression) = declaration.expression else {
      return DeclType::UNKNOWN;
    };
    let value = self.expression_type(decl_scope, source, expression);
    if declaration.flags.contains(DeclarationFlags::IS_CONSTANT)
      && let Some(denoted) = value.denoted
    {
      // `Handle :: #type,distinct u32;` names an otherwise anonymous variant.
      // An alias of an already-named type keeps the original's name, which is
      // what the reference prints (**L§3.13**).
      if let Some(name) = name
        && self.types().declared_name(denoted).is_none()
      {
        self.types_mut().set_declared_name(denoted, name);
      }
      return DeclType::type_name(denoted);
    }
    // A variable takes the *hardened* type of its initializer: an integer
    // literal becomes `s64` and a float literal its default width
    // (**L§5.10**).
    DeclType::value(self.harden(value.type_id))
  }

  pub(crate) fn ident_name(&self, source: SourceId, node: NodeId) -> Option<Symbol> {
    match self.ast(source)?.data(node) {
      NodeData::Ident(ident) => Some(ident.name),
      _ => None,
    }
  }

  /// The type of a procedure header (**L§3.7**): parameter and return types,
  /// the calling convention and the flags that change it. Names and defaults
  /// are informational and stay out.
  pub(crate) fn procedure_type(
    &mut self,
    source: SourceId,
    header: NodeId,
    outer: ScopeId,
  ) -> TypeId {
    if !self.enter() {
      return TypeId::UNKNOWN;
    }
    let result = self.procedure_type_inner(source, header, outer);
    self.leave();
    result
  }

  fn procedure_type_inner(&mut self, source: SourceId, header: NodeId, outer: ScopeId) -> TypeId {
    let Some(ast) = self.ast(source) else {
      return TypeId::UNKNOWN;
    };
    let NodeData::ProcedureHeader(payload) = ast.data(header) else {
      return TypeId::UNKNOWN;
    };

    let mut signature = ProcedureType::new(Vec::new(), Vec::new());
    let flags = payload.procedure_flags;
    signature.flags = procedure_flags(flags);

    for (index, parameter) in payload.arguments.iter().enumerate() {
      let (type_id, varargs) = self.parameter_type(source, *parameter, outer);
      if varargs && index + 1 == payload.arguments.len() {
        signature.varargs = true;
      }
      signature.arguments.push(type_id);
    }
    for parameter in &payload.returns {
      let (type_id, _) = self.parameter_type(source, *parameter, outer);
      signature.returns.push(type_id);
    }

    self.types_mut().procedure(signature)
  }

  /// One parameter or named return. `using p: Player` and `$T` parameters look
  /// like ordinary declarations from the type's point of view.
  fn parameter_type(
    &mut self,
    source: SourceId,
    parameter: NodeId,
    outer: ScopeId,
  ) -> (TypeId, bool) {
    let Some(ast) = self.ast(source) else {
      return (TypeId::UNKNOWN, false);
    };
    match ast.data(parameter) {
      NodeData::Declaration(declaration) => {
        let Some(type_inst) = declaration.type_inst else {
          // `name := "Hello"` is a parameter typed by its default (**L§5.10**).
          let Some(expression) = declaration.expression else {
            return (TypeId::UNKNOWN, false);
          };
          let value = self.expression_type(outer, source, expression);
          return (self.harden(value.type_id), false);
        };
        let varargs = self.is_varargs(source, type_inst);
        (self.type_from_node(outer, source, type_inst), varargs)
      }
      NodeData::Using(using) => {
        let expression = using.expression;
        self.parameter_type(source, expression, outer)
      }
      _ => (TypeId::UNKNOWN, false),
    }
  }

  fn is_varargs(&self, source: SourceId, type_inst: NodeId) -> bool {
    let Some(ast) = self.ast(source) else {
      return false;
    };
    match ast.data(type_inst) {
      NodeData::TypeInstantiation(inst) => {
        inst.inst_flags.contains(oj_syntax::ast::InstFlags::VARARGS)
      }
      _ => false,
    }
  }
}

/// The header flags that are part of the procedure's type (**L§3.7**).
fn procedure_flags(flags: ProcedureFlags) -> oj_types::ProcedureFlags {
  let mut result = oj_types::ProcedureFlags::empty();
  for (from, to) in [
    (
      ProcedureFlags::ELSEWHERE,
      oj_types::ProcedureFlags::IS_ELSEWHERE,
    ),
    (
      ProcedureFlags::COMPILE_TIME_ONLY,
      oj_types::ProcedureFlags::IS_COMPILE_TIME_ONLY,
    ),
    (
      ProcedureFlags::POLYMORPHIC,
      oj_types::ProcedureFlags::IS_POLYMORPHIC,
    ),
    (
      ProcedureFlags::SYNTACTICALLY_MARKED_AS_NO_CONTEXT,
      oj_types::ProcedureFlags::HAS_NO_CONTEXT,
    ),
    (ProcedureFlags::C_CALL, oj_types::ProcedureFlags::IS_C_CALL),
    (
      ProcedureFlags::INTRINSIC,
      oj_types::ProcedureFlags::IS_INTRINSIC,
    ),
    (
      ProcedureFlags::SYMMETRIC,
      oj_types::ProcedureFlags::IS_SYMMETRIC,
    ),
    (
      ProcedureFlags::CPP_METHOD,
      oj_types::ProcedureFlags::IS_CPP_METHOD,
    ),
    (
      ProcedureFlags::CPP_RETURN_TYPE_IS_NON_POD,
      oj_types::ProcedureFlags::HAS_CPP_NON_POD_RETURN_TYPE,
    ),
  ] {
    if flags.contains(from) {
      result |= to;
    }
  }
  result
}

use std::collections::HashSet;

use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclId, ScopeId};
use oj_syntax::ast::{DeclarationFlags, NodeData, NodeId, StructFlags};
use oj_types::{
  EnumInfo, EnumMember, EnumTypeFlags, IntKind, Layout, LayoutBuilder, MemberFlags, StructId,
  StructInfo, StructMember, StructNontextualFlags, StructTextualFlags, TypeId,
};

use crate::checker::{Checker, DeclType};
use crate::constants::{Const, Value};

/// `Build_Options.context_size_max`'s default, which is what `size_of(#Context)`
/// reports whatever the program imports (**L§10.1**, `docs/spec.md` §5.1).
const CONTEXT_SIZE_MAX: u64 = 4096;

/// The members and layout of one struct body, accumulated while its statements
/// are walked.
struct Members {
  builder: LayoutBuilder,
  members: Vec<StructMember>,
  /// Where the first `#place` started overlaying earlier members (**L§8.6**).
  overlay_from: Option<usize>,
  /// The declarations the scope tree actually admitted, so that the branch a
  /// `#if` discarded contributes nothing.
  live: HashSet<NodeId>,
  /// The default value each member was declared with, by member index
  /// (**L§8.1**): a variable of the struct starts out zeroed and then takes
  /// these (**L§4.6**).
  defaults: Vec<(usize, SourceId, NodeId)>,
  scope: ScopeId,
  source: SourceId,
}

impl Members {
  fn offset_of(&self, name: Symbol) -> Option<u64> {
    self
      .members
      .iter()
      .find(|member| member.name == name)
      .map(|member| member.offset)
  }
}

impl Checker<'_> {
  /// The type of the struct or union written at `node`, without its members
  /// (**L§8**). A pointer, a view or a procedure signature only ever needs this
  /// much, which is what lets `Thread_Proc :: #type (*Thread) -> s64;` and
  /// `Thread`, whose members include a `Thread_Proc`, both resolve.
  pub(crate) fn build_struct(
    &mut self,
    owner: Option<DeclId>,
    name: Option<Symbol>,
    source: SourceId,
    node: NodeId,
    outer: ScopeId,
  ) -> TypeId {
    if let Some(existing) = self.aggregate_type_in(outer, source, node) {
      if let Some(owner) = owner {
        self.publish(owner, DeclType::type_name(existing));
      }
      return existing;
    }
    let Some(ast) = self.ast(source) else {
      return TypeId::UNKNOWN;
    };
    let NodeData::Struct(payload) = ast.data(node) else {
      return TypeId::UNKNOWN;
    };

    let textual = StructTextualFlags::from_bits_truncate(payload.textual_flags.bits());
    let (definition, type_id) = self.types_mut().new_struct(StructInfo::new(name, textual));
    self.record_aggregate_type_in(outer, source, node, type_id);
    if let Some(owner) = owner {
      self.publish(owner, DeclType::type_name(type_id));
    }

    let scope = self
      .program()
      .aggregate_scope(source, node)
      .unwrap_or(outer);
    self.record_struct_scope(definition, scope);

    // A polymorphic struct is a family, not a type: its members need the
    // arguments of an instantiation (**L§8.5**, M7).
    if payload.has_argument_list && !payload.arguments.is_empty() {
      self
        .types_mut()
        .struct_info_mut(definition)
        .nontextual_flags |= StructNontextualFlags::POLYMORPHIC;
      self.types_mut().finish_struct(definition, 0, 1);
      return type_id;
    }

    self.record_pending_body(definition, source, node, scope);
    type_id
  }

  /// Fills a struct's members and layout in. Re-entering one is the genuine
  /// circular dependency: a struct that contains itself by value.
  pub(crate) fn complete_struct(&mut self, definition: StructId) {
    let Some((source, node, scope)) = self.take_pending_body(definition) else {
      return;
    };
    let Some(ast) = self.ast(source) else {
      return;
    };
    let NodeData::Struct(payload) = ast.data(node) else {
      return;
    };

    let mut state = Members {
      builder: LayoutBuilder::new(
        payload.textual_flags.contains(StructFlags::UNION),
        payload.textual_flags.contains(StructFlags::NO_PADDING),
      ),
      members: Vec::new(),
      overlay_from: None,
      live: self.live_declarations(scope),
      defaults: Vec::new(),
      scope,
      source,
    };
    if let Some(block) = payload.block {
      self.struct_statements(&mut state, block);
    }

    let layout = state.builder.finish();
    let overlay_from = state.overlay_from;
    let defaults = std::mem::take(&mut state.defaults);
    self.record_member_defaults(definition, defaults);
    let info = self.types_mut().struct_info_mut(definition);
    info.members = state.members;
    info.overlay_from = overlay_from;
    self
      .types_mut()
      .finish_struct(definition, layout.size, layout.alignment);
    self.finish_pending_body(definition);
  }

  /// The declaration nodes a scope holds. A `#if` the scope tree decided
  /// contributed only its taken branch, so walking both branches and keeping
  /// what the scope admits reproduces that decision (**L§6.10**). A `#if`
  /// nobody could decide contributed *every* branch, and those members are laid
  /// out too: leaving them out would give a struct in a conditionally loaded
  /// file no members at all, which is further from the truth than laying a
  /// discarded branch out beside the taken one.
  fn live_declarations(&self, scope: ScopeId) -> HashSet<NodeId> {
    let tree = self.program().tree();
    tree
      .scope(scope)
      .declarations
      .iter()
      .filter_map(|id| tree.decl(*id).node)
      .collect()
  }

  fn struct_statements(&mut self, state: &mut Members, block: NodeId) {
    let Some(ast) = self.ast(state.source) else {
      return;
    };
    let statements: &[NodeId] = match ast.data(block) {
      NodeData::Block(body) => &body.statements,
      _ => std::slice::from_ref(&block),
    };
    for statement in statements {
      self.struct_statement(state, *statement);
    }
  }

  fn struct_statement(&mut self, state: &mut Members, statement: NodeId) {
    let Some(ast) = self.ast(state.source) else {
      return;
    };
    match ast.data(statement) {
      NodeData::Declaration(declaration) => {
        if !state.live.contains(&statement) {
          return;
        }
        let Some(name) = declaration
          .name
          .and_then(|node| self.ident_name(state.source, node))
        else {
          return;
        };
        self.struct_member(state, statement, name, declaration, MemberFlags::empty());
      }
      // `a, b: float;` declares two members off one type slot (**L§4.5**).
      NodeData::CompoundDeclaration(compound) => {
        let properties = compound.declaration_properties;
        let NodeData::CommaSeparatedArguments { arguments } =
          ast.data(compound.comma_separated_assignment)
        else {
          return;
        };
        let NodeData::Declaration(declaration) = ast.data(properties) else {
          return;
        };
        for argument in arguments {
          if !state.live.contains(&argument.node) {
            continue;
          }
          let Some(name) = self.ident_name(state.source, argument.node) else {
            continue;
          };
          self.struct_member(
            state,
            argument.node,
            name,
            declaration,
            MemberFlags::empty(),
          );
        }
      }
      NodeData::Using(using) => {
        let expression = using.expression;
        let Some(NodeData::Declaration(declaration)) =
          self.ast(state.source).map(|ast| ast.data(expression))
        else {
          return;
        };
        if !state.live.contains(&expression) {
          return;
        }
        let Some(name) = declaration
          .name
          .and_then(|node| self.ident_name(state.source, node))
        else {
          return;
        };
        self.struct_member(state, expression, name, declaration, MemberFlags::USING);
      }
      // An anonymous `struct { … }` or `union { … }` contributes its members
      // to the struct around it (**L§8.1**).
      NodeData::Struct(_) => {
        let scope = state.scope;
        let source = state.source;
        let inner = self.build_struct(None, None, source, statement, scope);
        self.place_anonymous(state, inner);
      }
      NodeData::DirectivePlace { ident } => {
        let name = self.ident_name(state.source, *ident);
        if let Some(offset) = name.and_then(|name| state.offset_of(name)) {
          state.builder.rewind_to(offset);
          state.overlay_from.get_or_insert(state.members.len());
        }
      }
      NodeData::If(payload) => {
        let branches = [payload.then_block, payload.else_block];
        for branch in branches.into_iter().flatten() {
          self.struct_statements(state, branch);
        }
      }
      NodeData::Block(_) => self.struct_statements(state, statement),
      _ => {}
    }
  }

  fn struct_member(
    &mut self,
    state: &mut Members,
    node: NodeId,
    name: Symbol,
    declaration: &oj_syntax::ast::Declaration,
    extra: MemberFlags,
  ) {
    let mut flags = extra;
    if declaration
      .flags
      .contains(DeclarationFlags::IS_MARKED_AS_AS)
    {
      flags |= MemberFlags::AS;
    }

    // Constants and nested types have no storage; they are listed so that
    // `Type.Name` and `value.Name` find them (**L§8.3**).
    if declaration.flags.contains(DeclarationFlags::IS_CONSTANT) {
      let type_id = self
        .decl_in_scope(state.scope, node)
        .map(|id| self.decl_type(id).value)
        .unwrap_or(TypeId::UNKNOWN);
      state.members.push(StructMember {
        name,
        type_id,
        offset: 0,
        flags: flags | MemberFlags::CONSTANT,
        imported_through: None,
      });
      return;
    }

    let scope = state.scope;
    let source = state.source;
    let type_id = match declaration.type_inst {
      Some(type_inst) => self.type_from_node(scope, source, type_inst),
      None => match declaration.expression {
        Some(expression) => {
          let value = self.expression_type(scope, source, expression);
          self.harden(value.type_id)
        }
        None => TypeId::UNKNOWN,
      },
    };
    let alignment = declaration
      .alignment_expression
      .and_then(|node| self.const_int(scope, source, node))
      .filter(|value| *value > 0)
      .map(|value| value as u64);

    let Some(layout) = self.layout_of(type_id) else {
      // A member whose type is not knowable yet still takes a slot, so the
      // members after it keep their names even though the offsets are not
      // meaningful.
      state.members.push(StructMember {
        name,
        type_id,
        offset: state.builder.cursor(),
        flags,
        imported_through: None,
      });
      return;
    };

    let offset = state.builder.place(layout, alignment);
    let index = state.members.len();
    state.members.push(StructMember {
      name,
      type_id,
      offset,
      flags,
      imported_through: None,
    });
    if !declaration
      .flags
      .contains(DeclarationFlags::IS_UNINITIALIZED)
      && let Some(expression) = declaration.expression
    {
      state.defaults.push((index, source, expression));
    }

    if flags.contains(MemberFlags::USING) {
      self.import_members(state, type_id, offset, index);
    }
  }

  /// `using m: T;` makes `T`'s members part of this struct's namespace at
  /// their real offsets (**L§8.4**).
  fn import_members(&mut self, state: &mut Members, type_id: TypeId, offset: u64, through: usize) {
    // A `using` of a pointer member auto-dereferences.
    let target = self.types().pointee(type_id).unwrap_or(type_id);
    self.complete_type(target);
    let Some(definition) = self.types().struct_of(target) else {
      return;
    };
    let imported: Vec<StructMember> = self
      .types()
      .struct_info(definition)
      .members
      .iter()
      .map(|member| StructMember {
        name: member.name,
        type_id: member.type_id,
        offset: offset + member.offset,
        flags: member.flags | MemberFlags::IMPORTED,
        imported_through: Some(through),
      })
      .collect();
    state.members.extend(imported);
  }

  /// An anonymous struct or union member: its own members become the enclosing
  /// struct's, shifted to where it was placed.
  fn place_anonymous(&mut self, state: &mut Members, inner: TypeId) {
    let Some(layout) = self.layout_of(inner) else {
      return;
    };
    let offset = state.builder.place(layout, None);
    let Some(definition) = self.types().struct_of(inner) else {
      return;
    };
    let imported: Vec<StructMember> = self
      .types()
      .struct_info(definition)
      .members
      .iter()
      .map(|member| StructMember {
        name: member.name,
        type_id: member.type_id,
        offset: offset + member.offset,
        flags: member.flags,
        imported_through: None,
      })
      .collect();
    state.members.extend(imported);
  }

  pub(crate) fn decl_in_scope(&self, scope: ScopeId, node: NodeId) -> Option<DeclId> {
    let tree = self.program().tree();
    tree
      .scope(scope)
      .declarations
      .iter()
      .copied()
      .find(|id| tree.decl(*id).node == Some(node))
  }

  // ------------------------------------------------------------------ enums ---

  /// Builds the enum written at `node`, assigning member values in declaration
  /// order (**L§9**).
  pub(crate) fn build_enum(
    &mut self,
    owner: Option<DeclId>,
    name: Option<Symbol>,
    source: SourceId,
    node: NodeId,
    outer: ScopeId,
  ) -> TypeId {
    if let Some(existing) = self.aggregate_type_in(outer, source, node) {
      if let Some(owner) = owner {
        self.publish(owner, DeclType::type_name(existing));
      }
      return existing;
    }
    let Some(ast) = self.ast(source) else {
      return TypeId::UNKNOWN;
    };
    let NodeData::Enum(payload) = ast.data(node) else {
      return TypeId::UNKNOWN;
    };

    let base = match payload.internal_type_inst {
      Some(inst) => self.type_from_node(outer, source, inst),
      None => TypeId::S64,
    };
    let base = if self.types().integer_kind(base).is_some() {
      base
    } else {
      TypeId::S64
    };

    let mut flags = EnumTypeFlags::empty();
    flags.set(EnumTypeFlags::FLAGS, payload.is_flags);
    flags.set(EnumTypeFlags::COMPLETE, payload.marked_as_complete);
    flags.set(EnumTypeFlags::SPECIFIED, payload.marked_as_specified);

    let (definition, type_id) = self.types_mut().new_enum(EnumInfo::new(name, base, flags));
    self.record_aggregate_type_in(outer, source, node, type_id);
    if let Some(owner) = owner {
      self.publish(owner, DeclType::type_name(type_id));
    }

    let scope = self
      .program()
      .aggregate_scope(source, node)
      .unwrap_or(outer);
    self.record_enum_scope(definition, scope);

    let live = self.live_declarations(scope);
    if let Some(block) = payload.block {
      self.enum_statements(definition, type_id, scope, source, &live, block);
    }
    self.types_mut().enum_info_mut(definition).complete = true;
    type_id
  }

  fn enum_statements(
    &mut self,
    definition: oj_types::EnumId,
    type_id: TypeId,
    scope: ScopeId,
    source: SourceId,
    live: &HashSet<NodeId>,
    block: NodeId,
  ) {
    let Some(ast) = self.ast(source) else {
      return;
    };
    let statements: &[NodeId] = match ast.data(block) {
      NodeData::Block(body) => &body.statements,
      _ => std::slice::from_ref(&block),
    };
    for statement in statements {
      match ast.data(*statement) {
        NodeData::Declaration(declaration) => {
          if !live.contains(statement) {
            continue;
          }
          self.enum_member(definition, type_id, scope, source, *statement, declaration);
        }
        NodeData::If(payload) => {
          for branch in [payload.then_block, payload.else_block]
            .into_iter()
            .flatten()
          {
            self.enum_statements(definition, type_id, scope, source, live, branch);
          }
        }
        NodeData::Block(_) => {
          self.enum_statements(definition, type_id, scope, source, live, *statement)
        }
        _ => {}
      }
    }
  }

  fn enum_member(
    &mut self,
    definition: oj_types::EnumId,
    type_id: TypeId,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    declaration: &oj_syntax::ast::Declaration,
  ) {
    let Some(name) = declaration.name.and_then(|n| self.ident_name(source, n)) else {
      return;
    };
    // The member has to exist before its own expression is folded, so that a
    // member naming an earlier one resolves.
    if let Some(id) = self.decl_in_scope(scope, node) {
      self.publish(id, DeclType::value(type_id));
    }

    let value = match declaration.expression {
      Some(expression) => self.enum_expression_value(definition, scope, source, expression),
      None => None,
    };
    let value = value.unwrap_or_else(|| self.next_enum_value(definition));
    let base = self.types().enum_info(definition).base;
    let kind = self.types().integer_kind(base).unwrap_or(IntKind::S64);
    let value = crate::constants::wrap(value, kind) as i64;

    self
      .types_mut()
      .enum_info_mut(definition)
      .members
      .push(EnumMember { name, value });
    if let Some(id) = self.decl_in_scope(scope, node) {
      self.record_constant(id, Const::new(type_id, Value::Int(i128::from(value))));
    }
  }

  fn enum_expression_value(
    &mut self,
    definition: oj_types::EnumId,
    scope: ScopeId,
    source: SourceId,
    expression: NodeId,
  ) -> Option<i128> {
    let value = self.expression_type(scope, source, expression);
    match value.constant {
      Some(Const {
        value: Value::EnumName(name),
        ..
      }) => self
        .types()
        .enum_info(definition)
        .value_of(name)
        .map(i128::from),
      Some(other) => other.as_int(),
      None => None,
    }
  }

  /// An unspecified value is the previous one plus 1, or — for `enum_flags` —
  /// the next power of two after it (**L§9**).
  fn next_enum_value(&self, definition: oj_types::EnumId) -> i128 {
    let info = self.types().enum_info(definition);
    let Some(previous) = info.members.last() else {
      return i128::from(u8::from(info.is_flags()));
    };
    let previous = i128::from(previous.value);
    if !info.is_flags() {
      return previous + 1;
    }
    if previous <= 0 {
      return 1;
    }
    let mut next = 1i128;
    while next <= previous {
      next <<= 1;
    }
    next
  }

  // ---------------------------------------------------------------- context ---

  /// `#Context` is not declared anywhere: the compiler builds it from
  /// `Context_Base` plus every `#add_context` in the program (**L§10.2**).
  pub(crate) fn build_context_type(&mut self) -> TypeId {
    let base_name = self.interned().intern(b"Context_Base");
    let base = self.preload_type(base_name);
    let name = self.interned().intern(b"Context");
    let (definition, type_id) = self
      .types_mut()
      .new_struct(StructInfo::new(Some(name), StructTextualFlags::empty()));

    let mut builder = LayoutBuilder::new(false, false);
    let mut members = Vec::new();
    if let Some(layout) = self.layout_of(base) {
      let offset = builder.place(layout, None);
      let base_member = self.interned().intern(b"base");
      members.push(StructMember {
        name: base_member,
        type_id: base,
        offset,
        flags: MemberFlags::USING | MemberFlags::AS,
        imported_through: None,
      });
      let index = members.len() - 1;
      if let Some(inner) = self.types().struct_of(base) {
        let imported: Vec<StructMember> = self
          .types()
          .struct_info(inner)
          .members
          .iter()
          .map(|member| StructMember {
            name: member.name,
            type_id: member.type_id,
            offset: offset + member.offset,
            flags: member.flags | MemberFlags::IMPORTED,
            imported_through: Some(index),
          })
          .collect();
        members.extend(imported);
      }
    }

    // Added members are laid out alphabetically, so the context has the same
    // shape in every binary of a program (**L§10.2**).
    let mut added: Vec<(String, SourceId, NodeId, ScopeId)> = Vec::new();
    for (source, node, scope) in self.added_context_declarations() {
      let Some(NodeData::Declaration(declaration)) = self.ast(source).map(|ast| ast.data(node))
      else {
        continue;
      };
      let Some(name) = declaration.name.and_then(|n| self.ident_name(source, n)) else {
        continue;
      };
      added.push((self.symbol_text(name), source, node, scope));
    }
    added.sort_by(|left, right| left.0.cmp(&right.0));

    for (_, source, node, scope) in added {
      let Some(NodeData::Declaration(declaration)) = self.ast(source).map(|ast| ast.data(node))
      else {
        continue;
      };
      let Some(member_name) = declaration.name.and_then(|n| self.ident_name(source, n)) else {
        continue;
      };
      let member_type = match declaration.type_inst {
        Some(type_inst) => self.type_from_node(scope, source, type_inst),
        None => match declaration.expression {
          Some(expression) => {
            let value = self.expression_type(scope, source, expression);
            self.harden(value.type_id)
          }
          None => TypeId::UNKNOWN,
        },
      };
      let layout = self
        .types()
        .layout(member_type)
        .unwrap_or(Layout::new(0, 1));
      let offset = builder.place(layout, None);
      members.push(StructMember {
        name: member_name,
        type_id: member_type,
        offset,
        flags: MemberFlags::empty(),
        imported_through: None,
      });
    }

    // The context is padded out to `context_size_max` so that its size does not
    // depend on which modules a program imports (**L§10.1**).
    let layout = builder.finish();
    self.types_mut().struct_info_mut(definition).members = members;
    self.types_mut().finish_struct(
      definition,
      layout.size.max(CONTEXT_SIZE_MAX),
      layout.alignment,
    );
    type_id
  }

  /// Every `#add_context` declaration in the program, in file order.
  fn added_context_declarations(&self) -> Vec<(SourceId, NodeId, ScopeId)> {
    let mut found = Vec::new();
    for unit in self.program().units() {
      let ast = &unit.parsed.ast;
      for index in 0..ast.len() {
        let id = NodeId(index as u32);
        if let NodeData::DirectiveAddContext { expression } = ast.data(id) {
          found.push((unit.source, *expression, unit.scope));
        }
      }
    }
    found
  }
}

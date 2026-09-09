//! The `Code_*` export (**C§5.3**).
//!
//! `oj-syntax`'s nodes already carry the reference's kind numbers and flag
//! bits, so handing a metaprogram a tree is a projection: this walks an [`Ast`]
//! and writes the mirrors `oj-meta` declares into storage that lives as long as
//! the compilation.
//!
//! One AST node exports to one address for the whole compilation, so the
//! `Code_Declaration` a metaprogram was given in a `TYPECHECKED` message and
//! the one a later `Code_Ident.resolved_declaration` points at are the same
//! pointer. A name resolving to a declaration that has *not* been exported
//! leaves the pointer null rather than dragging that declaration's whole
//! subtree in behind it (`docs/spec.md` §10).

use std::collections::HashSet;

use oj_diag::SourceId;
use oj_meta::{
  CodeArgument, CodeArrayLiteralInfo, CodeAsm, CodeBinaryOperator, CodeBlock, CodeCase, CodeCast,
  CodeCommaSeparatedArgument, CodeCommaSeparatedArguments, CodeCompoundDeclaration,
  CodeDeclaration, CodeDefer, CodeDirectiveAddContext, CodeDirectiveBake, CodeDirectiveBytes,
  CodeDirectiveCode, CodeDirectiveExists, CodeDirectiveImport, CodeDirectiveInsert,
  CodeDirectiveLibrary, CodeDirectiveLoad, CodeDirectiveLocation, CodeDirectiveModify,
  CodeDirectiveModuleParameters, CodeDirectivePlace, CodeDirectivePokeName,
  CodeDirectiveProcedureName, CodeDirectiveRun, CodeDirectiveScope, CodeDirectiveWildcard,
  CodeEnum, CodeExpressionQuery, CodeFor, CodeIdent, CodeIf, CodeLiteral, CodeLiteralValues,
  CodeLoopControl, CodeNode, CodeNote, CodePointerLiteralInfo, CodeProcedureBody,
  CodeProcedureCall, CodeProcedureHeader, CodePushContext, CodeReturn, CodeStruct,
  CodeStructLiteralInfo, CodeTypeInstantiation, CodeTypeQuery, CodeUnaryOperator, CodeUsing,
  CodeWhile, ContextModification, Location, Nodes, Slice, Str, Tree, literal_type,
};
use oj_syntax::ast::{
  Argument, BakeType, BlockType, CommaModifier, ExpressionQueryKind, FilterType, ImportType,
  LiteralValue, LoopControlType, NodeData, NodeId, ScopeType, TypeQueryKind,
};

use crate::checker::Checker;

/// Walks one compilation's trees into the storage a metaprogram reads.
pub struct Exporter<'c, 'p> {
  checker: &'c mut Checker<'p>,
  nodes: &'c mut Nodes,
  /// The nodes this export touched, in the order it reached them, which is
  /// what `compiler_get_nodes` hands back as `expressions`.
  touched: Vec<*const CodeNode>,
  reached: HashSet<usize>,
  /// The compilation this export belongs to, which keys everything it places:
  /// one arena outlives many compilations, and each numbers its sources from
  /// zero (**C§3.2**).
  generation: u32,
  parent_block: *const CodeBlock,
  owning_statement: *const CodeNode,
}

impl<'c, 'p> Exporter<'c, 'p> {
  pub fn new(checker: &'c mut Checker<'p>, nodes: &'c mut Nodes, generation: u32) -> Self {
    Self {
      checker,
      nodes,
      touched: Vec::new(),
      reached: HashSet::new(),
      generation,
      parent_block: std::ptr::null(),
      owning_statement: std::ptr::null(),
    }
  }

  /// Exports one tree and hands back what `compiler_get_nodes` answers for it.
  pub fn tree(&mut self, source: SourceId, node: NodeId) -> Tree {
    self.touched.clear();
    self.reached.clear();
    let root = self.node(source, node);
    self.nodes.tree(root).unwrap_or(Tree {
      root,
      expressions: Slice::EMPTY,
    })
  }

  /// Exports one node, reusing the address it already has when this
  /// compilation has met it before.
  ///
  /// Every node exported gets a tree of its own recorded, since a `Code` a
  /// metaprogram holds may name any node of the program, not only one an
  /// export started at.
  pub fn node(&mut self, source: SourceId, id: NodeId) -> *const CodeNode {
    let key = (self.generation, source.0, id.0);
    if let Some(existing) = self.nodes.placed(key) {
      self.reach(existing);
      return existing;
    }
    let Some(ast) = self.checker.tree_of(source) else {
      return std::ptr::null();
    };
    let node = ast.node(id).clone();
    let start = self.touched.len();
    let address = self.place(source, id, &node);
    self.reach(address);
    // The nodes under this one come first and the root last, which is the
    // dependency order the reference flattens `subexpressions` into
    // (**C§3.2**).
    let expressions = self.nodes.arena.alloc_slice(&self.touched[start..]);
    self.nodes.record(Tree {
      root: address,
      expressions,
    });
    address
  }

  fn reach(&mut self, address: *const CodeNode) {
    if self.reached.insert(address as usize) {
      self.touched.push(address);
    }
  }

  fn optional(&mut self, source: SourceId, id: Option<NodeId>) -> *const CodeNode {
    match id {
      Some(id) => self.node(source, id),
      None => std::ptr::null(),
    }
  }

  fn list(&mut self, source: SourceId, ids: &[NodeId]) -> Slice {
    let items: Vec<*const CodeNode> = ids.iter().map(|id| self.node(source, *id)).collect();
    self.nodes.arena.alloc_slice(&items)
  }

  fn arguments(&mut self, source: SourceId, arguments: &[Argument]) -> Slice {
    let items: Vec<CodeArgument> = arguments
      .iter()
      .map(|argument| CodeArgument {
        expression: self.node(source, argument.expression),
        name: self.optional(source, argument.name).cast(),
      })
      .collect();
    self.nodes.arena.alloc_slice(&items)
  }

  fn text(&mut self, bytes: &[u8]) -> Str {
    self.nodes.arena.alloc_str(bytes)
  }

  fn symbol(&mut self, symbol: oj_lexer::Symbol) -> Str {
    let text = self.checker.interner().resolve_lossy(symbol).into_owned();
    self.text(text.as_bytes())
  }

  /// The head every node carries: its kind, the flags the parser put on it,
  /// and where it was written (**C§5.3**).
  fn head(&mut self, source: SourceId, id: NodeId, node: &oj_syntax::ast::Node) -> CodeNode {
    let kind = node.kind();
    let file = self.checker.program().sources().file(source);
    let start = file.location(node.span.start);
    let end = file.location(node.span.end);
    self
      .nodes
      .record_path((self.generation, source.0), file.path().to_path_buf());
    self.nodes.record_span(
      (self.generation, source.0, id.0),
      (node.span.start, node.span.end),
      file.bytes(),
    );
    CodeNode {
      // A kind of orangejuice's own is not one the reference names, so it
      // travels as `UNINITIALIZED` rather than as a number a metaprogram would
      // read as something else (**C§5.3**).
      kind: if kind.is_internal() {
        0
      } else {
        kind as u16 as u8
      },
      node_flags: node.flags.bits(),
      type_info: std::ptr::null(),
      location: Location {
        enclosing_load: std::ptr::null(),
        l0: start.line as i32,
        c0: start.column as i32,
        l1: end.line as i32,
        c1: end.column as i32,
      },
      serial: self.nodes.next_serial(),
    }
  }
}

impl Exporter<'_, '_> {
  /// Writes one node into the arena. The address is recorded before the
  /// members are, so a tree that points back at itself terminates.
  fn place(
    &mut self,
    source: SourceId,
    id: NodeId,
    node: &oj_syntax::ast::Node,
  ) -> *const CodeNode {
    let key = (self.generation, source.0, id.0);
    let owning_statement = self.owning_statement;

    macro_rules! start {
      ($mirror:ty) => {{
        let address = self.nodes.place::<$mirror>(key);
        let head = self.head(source, id, node);
        unsafe { (*address).base = head };
        self.owning_statement = address.cast();
        address
      }};
    }

    macro_rules! plain {
      () => {{
        let address = self.nodes.place::<CodeNode>(key);
        let head = self.head(source, id, node);
        unsafe { *address = head };
        self.owning_statement = address;
        address
      }};
    }

    let exported: *const CodeNode = match &node.data {
      NodeData::Block(block) => {
        let address = self.nodes.place::<CodeBlock>(key);
        let head = self.head(source, id, node);
        unsafe {
          (*address).base = head;
          (*address).parent = self.parent_block;
          (*address).owning_statement = owning_statement;
          (*address).block_type = block_type_of(block.block_type);
          (*address).block_flags = block.block_flags.bits();
        }
        let outer = std::mem::replace(&mut self.parent_block, address);
        self.owning_statement = std::ptr::null();
        let statements = self.list(source, &block.statements);
        self.parent_block = outer;
        // `members` are the block's scope entries, which among its statements
        // are the declarations (**C§5.3**).
        let declared: Vec<NodeId> = match self.checker.tree_of(source) {
          Some(ast) => block
            .statements
            .iter()
            .copied()
            .filter(|statement| ast.kind(*statement) == oj_syntax::ast::NodeKind::Declaration)
            .collect(),
          None => Vec::new(),
        };
        let entries: Vec<*const CodeNode> = declared
          .iter()
          .map(|statement| self.node(source, *statement))
          .collect();
        let members = self.nodes.arena.alloc_slice(&entries);
        unsafe {
          (*address).statements = statements;
          (*address).members = members;
        }
        address.cast()
      }

      NodeData::Literal(literal) => {
        let address = start!(CodeLiteral);
        let (value_type, values) = self.literal(source, &literal.value);
        unsafe {
          (*address).value_type = value_type;
          (*address).values = values;
          (*address).value_flags = literal.flags.bits();
        }
        address.cast()
      }

      NodeData::Ident(ident) => {
        let address = start!(CodeIdent);
        let name = self.symbol(ident.name);
        let resolved = self.resolved_declaration(source, id);
        unsafe {
          (*address).name = name;
          (*address).resolved_declaration = resolved;
          (*address).flags = ident.flags.bits();
        }
        address.cast()
      }

      NodeData::UnaryOperator { operator, operand } => {
        let address = start!(CodeUnaryOperator);
        let operand = self.node(source, *operand);
        unsafe {
          (*address).operator_type = operator.0;
          (*address).subexpression = operand;
        }
        address.cast()
      }

      NodeData::BinaryOperator {
        operator,
        flags,
        left,
        right,
      } => {
        let address = start!(CodeBinaryOperator);
        let left = self.node(source, *left);
        let right = self.node(source, *right);
        unsafe {
          (*address).operator_type = operator.0;
          (*address).flags = flags.bits() as u16;
          (*address).left = left;
          (*address).right = right;
        }
        address.cast()
      }

      NodeData::ProcedureBody { header, block } => {
        let address = start!(CodeProcedureBody);
        let block = self.node(source, *block);
        let header = self.node(source, *header);
        unsafe {
          (*address).block = block.cast();
          (*address).header = header.cast();
        }
        address.cast()
      }

      NodeData::ProcedureCall(call) => {
        let address = start!(CodeProcedureCall);
        let procedure = self.node(source, call.procedure_expression);
        let unsorted = self.arguments(source, &call.arguments);
        let modification = match &call.context_modification {
          Some(expressions) => {
            let expressions = self.list(source, expressions);
            self.nodes.arena.alloc(ContextModification {
              modification_expressions: expressions,
            })
          }
          None => std::ptr::null_mut(),
        };
        unsafe {
          (*address).procedure_expression = procedure;
          (*address).resolved_procedure_expression =
            self.already_exported(source, call.procedure_expression);
          (*address).arguments_unsorted = unsorted;
          (*address).context_modification = modification;
          (*address).flags = call.flags.bits();
        }
        address.cast()
      }

      NodeData::Context => plain!(),

      NodeData::While { condition, block } => {
        let address = start!(CodeWhile);
        let condition = self.node(source, *condition);
        let block = self.node(source, *block);
        unsafe {
          (*address).condition = condition;
          (*address).block = block.cast();
        }
        address.cast()
      }

      NodeData::If(node) => {
        let address = start!(CodeIf);
        let condition = self.node(source, node.condition);
        let then_block = self.optional(source, node.then_block);
        let else_block = self.optional(source, node.else_block);
        unsafe {
          (*address).condition = condition;
          (*address).then_block = then_block.cast();
          (*address).else_block = else_block.cast();
          (*address).if_flags = node.if_flags.bits() as u16;
        }
        address.cast()
      }

      NodeData::LoopControl {
        control_type,
        target_ident,
      } => {
        let address = start!(CodeLoopControl);
        let target = self.optional(source, *target_ident);
        unsafe {
          (*address).control_type = match control_type {
            LoopControlType::Break => 0,
            LoopControlType::Continue => 1,
            LoopControlType::Remove => 2,
          };
          (*address).target_ident = target.cast();
        }
        address.cast()
      }

      NodeData::Case(case) => {
        let address = start!(CodeCase);
        let condition = self.optional(source, case.condition);
        let then_block = self.node(source, case.then_block);
        unsafe {
          (*address).condition = condition;
          (*address).then_block = then_block.cast();
          (*address).marked_as_fallthrough = case.marked_as_fallthrough;
        }
        address.cast()
      }

      NodeData::Return { arguments, flags } => {
        let address = start!(CodeReturn);
        let unsorted = self.arguments(source, arguments);
        unsafe {
          (*address).arguments_unsorted = unsorted;
          (*address).return_flags = flags.bits();
        }
        address.cast()
      }

      NodeData::For(node) => {
        let address = start!(CodeFor);
        let iteration = self.node(source, node.iteration_expression);
        let right = self.optional(source, node.iteration_expression_right);
        let block = self.node(source, node.block);
        let it = self.optional(source, node.ident_it);
        let it_index = self.optional(source, node.ident_it_index);
        let replacement = self.optional(source, node.want_replacement_for_expansion);
        let pointer = self.optional(source, node.want_pointer_expression);
        let reverse = self.optional(source, node.want_reverse_expression);
        unsafe {
          (*address).iteration_expression = iteration;
          (*address).iteration_expression_right = right;
          (*address).block = block.cast();
          (*address).ident_it = it.cast();
          (*address).ident_it_index = it_index.cast();
          (*address).want_replacement_for_expansion = replacement;
          (*address).want_pointer_expression = pointer;
          (*address).want_reverse_expression = reverse;
          (*address).for_flags = node.for_flags.bits() as u8;
        }
        address.cast()
      }

      NodeData::TypeInstantiation(inst) => {
        let address = start!(CodeTypeInstantiation);
        let valued = self.optional(source, inst.type_valued_expression);
        let must_implement = self.optional(source, inst.must_implement);
        let pointer_to = self.optional(source, inst.pointer_to);
        let directive = self.optional(source, inst.type_directive_target);
        let element = self.optional(source, inst.array_element_type);
        let dimension = self.optional(source, inst.array_dimension);
        unsafe {
          (*address).type_valued_expression = valued;
          (*address).must_implement = must_implement;
          (*address).pointer_to = pointer_to.cast();
          (*address).type_directive_target = directive.cast();
          (*address).array_element_type = element.cast();
          (*address).array_dimension = dimension;
          (*address).inst_flags = inst.inst_flags.bits();
        }
        address.cast()
      }

      NodeData::Enum(node) => {
        let address = start!(CodeEnum);
        let internal = self.optional(source, node.internal_type_inst);
        let block = self.optional(source, node.block);
        let notes = self.list(source, &node.notes);
        unsafe {
          (*address).internal_type_inst = internal.cast();
          (*address).block = block.cast();
          (*address).notes = notes;
          (*address).marked_as_complete = node.marked_as_complete;
          (*address).marked_as_specified = node.marked_as_specified;
          (*address).is_flags = node.is_flags;
        }
        address.cast()
      }

      NodeData::ProcedureHeader(header) => {
        let address = start!(CodeProcedureHeader);
        let arguments = self.list(source, &header.arguments);
        let returns = self.list(source, &header.returns);
        let name = match header.name {
          Some(name) => self.symbol(name),
          None => Str::EMPTY,
        };
        let foreign = match &header.foreign_function_name {
          Some(text) => self.text(text),
          None => Str::EMPTY,
        };
        let library = self.optional(source, header.library_identifier);
        let deprecation = match &header.deprecation_string {
          Some(text) => self.text(text),
          None => Str::EMPTY,
        };
        let modify = self.list(source, &header.modify_directives);
        let notes = self.list(source, &header.notes);
        let body = self.optional(source, header.body_or_null);
        unsafe {
          (*address).arguments = arguments;
          (*address).returns = returns;
          (*address).name = name;
          (*address).foreign_function_name = foreign;
          (*address).library_identifier = library.cast();
          (*address).deprecation_string = deprecation;
          (*address).modify_directives = modify;
          (*address).notes = notes;
          (*address).body_or_null = body.cast();
          (*address).procedure_flags = header.procedure_flags.bits();
        }
        address.cast()
      }

      NodeData::Struct(node) => {
        let address = start!(CodeStruct);
        let modify = self.list(source, &node.modify_directives);
        let block = self.optional(source, node.block);
        let notes = self.list(source, &node.notes);
        unsafe {
          (*address).modify_directives = modify;
          (*address).block = block.cast();
          (*address).notes = notes;
          (*address).textual_flags = node.textual_flags.bits();
        }
        address.cast()
      }

      NodeData::CommaSeparatedArguments { arguments } => {
        let address = start!(CodeCommaSeparatedArguments);
        let items: Vec<CodeCommaSeparatedArgument> = arguments
          .iter()
          .map(|argument| CodeCommaSeparatedArgument {
            node: self.node(source, argument.node),
            modifier: match argument.modifier {
              CommaModifier::None => 0,
              CommaModifier::Declare => 1,
              CommaModifier::Assign => 2,
            },
          })
          .collect();
        let arguments = self.nodes.arena.alloc_slice(&items);
        unsafe { (*address).arguments = arguments };
        address.cast()
      }

      NodeData::DirectiveBytes { expression } => {
        let address = start!(CodeDirectiveBytes);
        let expression = self.node(source, *expression);
        unsafe { (*address).expression = expression };
        address.cast()
      }

      NodeData::Declaration(declaration) => {
        let address = self.nodes.place::<CodeDeclaration>(key);
        let head = self.head(source, id, node);
        unsafe { (*address).entry.base = head };
        self.owning_statement = address.cast();
        let name = match declaration.name {
          Some(name) => match self.checker.name_at(source, name) {
            Some(symbol) => self.symbol(symbol),
            None => Str::EMPTY,
          },
          None => Str::EMPTY,
        };
        let type_inst = self.optional(source, declaration.type_inst);
        let expression = self.optional(source, declaration.expression);
        let alignment = self.optional(source, declaration.alignment_expression);
        let notes = self.list(source, &declaration.notes);
        let export_name = match &declaration.program_export_name {
          Some(text) => self.text(text),
          None => Str::EMPTY,
        };
        // The reference puts a procedure's name on its *header*, which is how
        // a metaprogram reading a `TYPECHECKED` message knows which procedure
        // it has (**C§5.3**); orangejuice's parser keeps it on the
        // declaration, so the header takes it from there.
        if !expression.is_null()
          && unsafe { (*expression).kind } == oj_syntax::ast::NodeKind::ProcedureHeader as u16 as u8
        {
          let header = expression.cast::<CodeProcedureHeader>().cast_mut();
          if unsafe { (*header).name.count } == 0 {
            unsafe { (*header).name = name };
          }
        }
        unsafe {
          (*address).entry.name = name;
          (*address).type_inst = type_inst.cast();
          (*address).expression = expression;
          (*address).alignment_expression = alignment;
          (*address).notes = notes;
          (*address).program_export_name = export_name;
          (*address).flags = declaration.flags.bits();
        }
        address.cast()
      }

      NodeData::Cast(cast) => {
        let address = start!(CodeCast);
        let target = self.optional(source, cast.target_type);
        let expression = self.node(source, cast.expression);
        unsafe {
          (*address).target_type = target.cast();
          (*address).expression = expression;
          (*address).cast_flags = cast.cast_flags.bits();
        }
        address.cast()
      }

      NodeData::DirectiveImport(import) => {
        let address = start!(CodeDirectiveImport);
        let name = self.text(&import.name);
        unsafe {
          (*address).name = name;
          (*address).flags = import.flags.bits();
          (*address).import_type = match import.import_type {
            ImportType::ShortName => 0,
            ImportType::PathToFile => 1,
            ImportType::PathToDirectory => 2,
            ImportType::FullText => 3,
          };
        }
        address.cast()
      }

      NodeData::DirectiveThis | NodeData::DirectiveThrough | NodeData::DirectiveCompileTime => {
        plain!()
      }

      NodeData::DirectiveLoad { name } => {
        let address = start!(CodeDirectiveLoad);
        let short_name = self.text(name);
        unsafe { (*address).short_name = short_name };
        address.cast()
      }

      NodeData::DirectiveRun(run) => {
        let address = start!(CodeDirectiveRun);
        let procedure = self.node(source, run.procedure);
        let assertion = self.optional(source, run.assertion_string);
        let _ = assertion;
        unsafe {
          (*address).procedure = procedure.cast();
          (*address).flags = run.flags.bits();
        }
        address.cast()
      }

      NodeData::DirectiveCode { expression, flags } => {
        let address = start!(CodeDirectiveCode);
        let expression = self.optional(source, *expression);
        unsafe {
          (*address).expression = expression;
          (*address).code_flags = flags.bits();
        }
        address.cast()
      }

      NodeData::DirectivePokeName { module, name } => {
        let address = start!(CodeDirectivePokeName);
        let module = self.node(source, *module);
        let name = match self.checker.name_at(source, *name) {
          Some(symbol) => self.symbol(symbol),
          None => Str::EMPTY,
        };
        unsafe {
          (*address).module_struct = module.cast();
          (*address).name = name;
        }
        address.cast()
      }

      // `#asm` is opaque to a metaprogram in the reference too: the fields are
      // there so that the struct is the right size (**C§5.3**, **L§15**).
      NodeData::Asm(_) => start!(CodeAsm).cast(),

      NodeData::DirectiveBake {
        procedure_call,
        bake_type,
      } => {
        let address = start!(CodeDirectiveBake);
        let call = self.node(source, *procedure_call);
        unsafe {
          (*address).procedure_call = call.cast();
          (*address).bake_type = match bake_type {
            BakeType::Constants => 0,
            BakeType::ParameterValue => 1,
            BakeType::DynamicSpecialize => 2,
          };
        }
        address.cast()
      }

      NodeData::DirectiveModify { block } => {
        let address = start!(CodeDirectiveModify);
        let block = self.node(source, *block);
        unsafe { (*address).block = block.cast() };
        address.cast()
      }

      NodeData::DirectiveLibrary {
        name,
        library_flags,
      } => {
        let address = start!(CodeDirectiveLibrary);
        let name = self.text(name);
        unsafe {
          (*address).name = name;
          (*address).library_flags = library_flags.bits();
        }
        address.cast()
      }

      NodeData::ExpressionQuery {
        query_kind,
        expression_to_query,
      } => {
        let address = start!(CodeExpressionQuery);
        let expression = self.node(source, *expression_to_query);
        unsafe {
          (*address).query_kind = match query_kind {
            ExpressionQueryKind::TypeOf => 0,
            ExpressionQueryKind::IsConstant => 1,
            ExpressionQueryKind::CodeOf => 2,
          };
          (*address).expression_to_query = expression;
        }
        address.cast()
      }

      NodeData::PushContext {
        to_push,
        block,
        flags,
      } => {
        let address = start!(CodePushContext);
        let to_push = self.optional(source, *to_push);
        let block = self.optional(source, *block);
        unsafe {
          (*address).to_push = to_push;
          (*address).block = block.cast();
          (*address).push_context_flags = flags.bits();
        }
        address.cast()
      }

      NodeData::Note { text } => {
        let address = start!(CodeNote);
        let text = self.symbol(*text);
        unsafe { (*address).text = text };
        address.cast()
      }

      NodeData::DirectivePlace { ident } => {
        let address = start!(CodeDirectivePlace);
        let ident = self.node(source, *ident);
        unsafe { (*address).ident = ident.cast() };
        address.cast()
      }

      NodeData::DirectiveScope { scope_type } => {
        let address = start!(CodeDirectiveScope);
        unsafe {
          (*address).scope_type = match scope_type {
            ScopeType::Export => 0,
            ScopeType::File => 1,
            ScopeType::Internal => 2,
          };
        }
        address.cast()
      }

      NodeData::TypeQuery {
        query_kind,
        type_to_query,
      } => {
        let address = start!(CodeTypeQuery);
        let queried = self.node(source, *type_to_query);
        unsafe {
          (*address).query_kind = match query_kind {
            TypeQueryKind::SizeOf => 0,
            TypeQueryKind::TypeInfo => 1,
            TypeQueryKind::InitializerOf => 2,
          };
          (*address).type_to_query = queried.cast();
        }
        address.cast()
      }

      NodeData::DirectiveLocation(location) => {
        let address = start!(CodeDirectiveLocation);
        let expression = self.optional(source, location.expression);
        unsafe {
          (*address).expression = expression;
          (*address).is_caller_location = location.is_caller_location;
        }
        address.cast()
      }

      NodeData::DirectiveModuleParameters(parameters) => {
        let address = start!(CodeDirectiveModuleParameters);
        let module = self.node(source, parameters.module_parameters);
        let program = self.optional(source, parameters.program_parameters);
        let common = self.optional(source, parameters.common_code);
        unsafe {
          (*address).module_parameters = module.cast();
          (*address).program_parameters = program.cast();
          (*address).common_code = common.cast();
        }
        address.cast()
      }

      NodeData::DirectiveAddContext { expression } => {
        let address = start!(CodeDirectiveAddContext);
        let expression = self.node(source, *expression);
        unsafe { (*address).expression = expression };
        address.cast()
      }

      NodeData::CompoundDeclaration(compound) => {
        let address = self.nodes.place::<CodeCompoundDeclaration>(key);
        let head = self.head(source, id, node);
        unsafe { (*address).entry = head };
        self.owning_statement = address.cast();
        let assignment = self.node(source, compound.comma_separated_assignment);
        let properties = self.node(source, compound.declaration_properties);
        unsafe {
          (*address).comma_separated_assignment = assignment.cast();
          (*address).declaration_properties = properties.cast();
          (*address).operator_type = compound.operator_type.map(|op| op.0).unwrap_or(0);
        }
        address.cast()
      }

      NodeData::Defer {
        block,
        is_backticked,
      } => {
        let address = start!(CodeDefer);
        let block = self.node(source, *block);
        unsafe {
          (*address).block = block.cast();
          (*address).is_backticked = *is_backticked;
        }
        address.cast()
      }

      NodeData::Using(using) => {
        let address = start!(CodeUsing);
        let expression = self.node(source, using.expression);
        let filter = self.optional(source, using.filter_expression);
        unsafe {
          (*address).expression = expression;
          (*address).filter_type = match using.filter_type {
            FilterType::None => 0,
            FilterType::Only => 1,
            FilterType::Except => 2,
            FilterType::Map => 3,
          };
          (*address).filter_expression = filter;
          (*address).no_parameters = using.no_parameters;
        }
        address.cast()
      }

      NodeData::Placeholder => plain!(),

      NodeData::DirectiveInsert(insert) => {
        let address = start!(CodeDirectiveInsert);
        let expression = self.node(source, insert.expression);
        let redirection = self.optional(source, insert.scope_redirection);
        let break_replacement = self.optional(source, insert.break_replacement);
        let continue_replacement = self.optional(source, insert.continue_replacement);
        let remove_replacement = self.optional(source, insert.remove_replacement);
        unsafe {
          (*address).expression = expression;
          (*address).scope_redirection = redirection;
          (*address).break_replacement = break_replacement;
          (*address).continue_replacement = continue_replacement;
          (*address).remove_replacement = remove_replacement;
        }
        address.cast()
      }

      NodeData::DirectiveProcedureName { argument } => {
        let address = start!(CodeDirectiveProcedureName);
        let argument = self.optional(source, *argument);
        unsafe { (*address).argument = argument };
        address.cast()
      }

      NodeData::DirectiveWildcard { index } => {
        let address = start!(CodeDirectiveWildcard);
        unsafe { (*address).index = *index };
        address.cast()
      }

      NodeData::DirectiveExists(exists) => {
        let address = start!(CodeDirectiveExists);
        let query = self.node(source, exists.query_expression);
        let sync = self.optional(source, exists.sync_expression);
        unsafe {
          (*address).query_expression = query;
          (*address).sync_expression = sync;
        }
        address.cast()
      }

      NodeData::DirectiveContextType => plain!(),

      // Kinds of orangejuice's own, which the reference folds during parsing
      // and does not name in `Code_Node.Kind` (**C§5.3**).
      NodeData::DirectiveFileInfo { .. } | NodeData::DirectiveCallerCode => plain!(),
    };

    self.owning_statement = owning_statement;
    exported
  }

  /// A literal's `value_type` and the union that goes with it (**C§5.3**).
  fn literal(&mut self, source: SourceId, value: &LiteralValue) -> (i16, CodeLiteralValues) {
    match value {
      LiteralValue::Integer(number) => (
        literal_type::NUMBER,
        CodeLiteralValues { unsigned: *number },
      ),
      LiteralValue::Float(number) => (literal_type::NUMBER, CodeLiteralValues { float64: *number }),
      LiteralValue::Text(bytes) => {
        let text = self.text(bytes);
        (literal_type::STRING, CodeLiteralValues { text })
      }
      LiteralValue::Bool(flag) => (
        literal_type::BOOLEAN,
        CodeLiteralValues {
          signed: i64::from(*flag),
        },
      ),
      // A `null` is a pointer literal whose value is a constant zero, which is
      // the most common pointer literal there is (**C§5.3**).
      LiteralValue::Null => {
        let info = self.nodes.arena.alloc(CodePointerLiteralInfo {
          global_symbol: std::ptr::null(),
          data_pointer: std::ptr::null(),
          pointer_literal_type: 2,
          offset_from_symbol: 0,
        });
        (
          literal_type::POINTER,
          CodeLiteralValues {
            pointer_literal_info: info,
          },
        )
      }
      LiteralValue::Array(array) => {
        let element_type = self.optional(source, array.element_type);
        let members = self.list(source, &array.members);
        let info = self.nodes.arena.alloc(CodeArrayLiteralInfo {
          element_type: element_type.cast(),
          alignment: std::ptr::null(),
          array_members: members,
          array_literal_flags: u8::from(array.element_type.is_none()),
        });
        (
          literal_type::ARRAY,
          CodeLiteralValues {
            array_literal_info: info,
          },
        )
      }
      LiteralValue::Struct(literal) => {
        let type_expression = self.optional(source, literal.type_expression);
        let items: Vec<*const CodeNode> = literal
          .arguments
          .iter()
          .map(|argument| self.node(source, argument.expression))
          .collect();
        let arguments = self.nodes.arena.alloc_slice(&items);
        let info = self.nodes.arena.alloc(CodeStructLiteralInfo {
          type_expression: type_expression.cast(),
          arguments,
        });
        (
          literal_type::STRUCT,
          CodeLiteralValues {
            struct_literal_info: info,
          },
        )
      }
    }
  }

  /// The declaration a name resolved to, when this compilation has already
  /// exported it. Following one that has not been exported would drag its
  /// whole subtree in behind it, so it stays null (`docs/spec.md` §10).
  fn resolved_declaration(&mut self, source: SourceId, id: NodeId) -> *const CodeDeclaration {
    let Some(decl) = self.checker.decl_at(source, id) else {
      return std::ptr::null();
    };
    let declaration = self.checker.program().tree().decl(decl);
    let (Some(declared_in), Some(node)) = (declaration.source, declaration.node) else {
      return std::ptr::null();
    };
    match self.nodes.placed((self.generation, declared_in.0, node.0)) {
      Some(address) => {
        self.reach(address);
        address.cast()
      }
      None => std::ptr::null(),
    }
  }

  /// What a call resolved to, on the same terms as
  /// [`Exporter::resolved_declaration`].
  fn already_exported(&mut self, source: SourceId, id: NodeId) -> *const CodeNode {
    self.resolved_declaration(source, id).cast()
  }
}

fn block_type_of(block_type: BlockType) -> i32 {
  match block_type {
    BlockType::Imperative => 1,
    BlockType::DataDeclarations => 2,
    BlockType::Arguments => 3,
    BlockType::Returns => 4,
    BlockType::StructArguments => 5,
    BlockType::Constants => 6,
  }
}

impl Exporter<'_, '_> {
  /// Every toplevel declaration of the program, and every procedure header,
  /// body and struct wherever it was written, exported and sorted into the
  /// arrays a `Message_Typechecked` carries (**C§3.2**).
  pub fn program(&mut self) -> oj_meta::TypecheckedBatch {
    let mut batch = oj_meta::TypecheckedBatch::default();
    let units: Vec<(SourceId, NodeId)> = self
      .checker
      .program()
      .units()
      .map(|unit| (unit.source, unit.parsed.root))
      .collect();
    for (source, root) in units {
      let Some(ast) = self.checker.tree_of(source) else {
        continue;
      };
      let count = ast.len();
      // Only *toplevel* declarations are sent; a metaprogram that wants the
      // ones inside recurses into these (**C§3.2**).
      let toplevel: Vec<NodeId> = match ast.data(root) {
        NodeData::Block(block) => block.statements.clone(),
        _ => Vec::new(),
      };
      let kinds: Vec<(NodeId, oj_syntax::ast::NodeKind)> = (0..count as u32)
        .map(NodeId)
        .map(|id| (id, ast.kind(id)))
        .collect();

      for id in toplevel {
        let entry = self.typechecked(source, id);
        match self.checker.tree_of(source).map(|ast| ast.kind(id)) {
          Some(oj_syntax::ast::NodeKind::Declaration) => batch.declarations.push(entry),
          _ => batch.others.push(entry),
        }
      }
      for (id, kind) in kinds {
        let entry = match kind {
          oj_syntax::ast::NodeKind::ProcedureHeader
          | oj_syntax::ast::NodeKind::ProcedureBody
          | oj_syntax::ast::NodeKind::Struct => self.typechecked(source, id),
          _ => continue,
        };
        match kind {
          oj_syntax::ast::NodeKind::ProcedureHeader => batch.procedure_headers.push(entry),
          oj_syntax::ast::NodeKind::ProcedureBody => batch.procedure_bodies.push(entry),
          _ => batch.structs.push(entry),
        }
      }
    }
    self.resolve_names();
    batch
  }

  fn typechecked(&mut self, source: SourceId, id: NodeId) -> oj_meta::Typechecked {
    let tree = self.tree(source, id);
    oj_meta::Typechecked {
      expression: tree.root,
      subexpressions: tree.expressions,
    }
  }

  /// Fills in the declarations names resolve to, now that everything the
  /// compilation exported has an address. A name written before the
  /// declaration it means could not be pointed at on the first walk.
  pub fn resolve_names(&mut self) {
    for ((generation, source, node), address) in self.nodes.placed_entries() {
      // Only this compilation's nodes: an earlier one's numbers mean nothing
      // to the trees this one is walking.
      if generation != self.generation {
        continue;
      }
      let source = SourceId(source);
      let id = NodeId(node);
      let kind = unsafe { (*address).kind };
      if kind == oj_syntax::ast::NodeKind::Ident as u16 as u8 {
        let resolved = self.resolved_declaration(source, id);
        unsafe { (*address.cast::<CodeIdent>()).resolved_declaration = resolved };
        continue;
      }
      if kind != oj_syntax::ast::NodeKind::ProcedureCall as u16 as u8 {
        continue;
      }
      let called = match self.checker.tree_of(source).map(|ast| ast.data(id)) {
        Some(NodeData::ProcedureCall(call)) => call.procedure_expression,
        _ => continue,
      };
      let resolved = self.already_exported(source, called);
      unsafe { (*address.cast::<CodeProcedureCall>()).resolved_procedure_expression = resolved };
    }
  }
}

use std::fmt::Write as _;

use oj_lexer::Interner;

use crate::ast::*;

/// Prints the tree form of `oj dump ast --tree`: every node's kind, flags and
/// scalar fields, with children indented under labels. Spans are deliberately
/// absent, which makes the output the canonical shape of a tree — the corpus
/// round-trip test compares two of these.
pub fn print_tree(ast: &Ast, root: NodeId, interner: &Interner) -> String {
  let mut printer = TreePrinter {
    ast,
    interner,
    out: String::new(),
    depth: 0,
  };
  printer.node("", root);
  printer.out
}

struct TreePrinter<'a> {
  ast: &'a Ast,
  interner: &'a Interner,
  out: String,
  depth: usize,
}

impl TreePrinter<'_> {
  fn line(&mut self, label: &str, text: &str) {
    for _ in 0..self.depth {
      self.out.push_str("  ");
    }
    if !label.is_empty() {
      self.out.push_str(label);
      self.out.push_str(": ");
    }
    self.out.push_str(text);
    self.out.push('\n');
  }

  fn child(&mut self, label: &str, id: NodeId) {
    self.depth += 1;
    self.node(label, id);
    self.depth -= 1;
  }

  fn children(&mut self, label: &str, ids: &[NodeId]) {
    for (index, id) in ids.iter().enumerate() {
      self.child(&format!("{label}[{index}]"), *id);
    }
  }

  fn optional(&mut self, label: &str, id: Option<NodeId>) {
    if let Some(id) = id {
      self.child(label, id);
    }
  }

  fn text_of(&self, bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
  }

  fn arguments(&mut self, label: &str, arguments: &[Argument]) {
    for (index, argument) in arguments.iter().enumerate() {
      self.optional(&format!("{label}[{index}].name"), argument.name);
      self.child(&format!("{label}[{index}]"), argument.expression);
    }
  }

  fn node(&mut self, label: &str, id: NodeId) {
    let node = self.ast.node(id);
    let kind = node.kind();
    let mut header = kind.name().to_string();
    if !node.flags.is_empty() {
      let _ = write!(header, " flags={:?}", node.flags);
    }

    let data = node.data.clone();
    match &data {
      NodeData::Block(block) => {
        let _ = write!(header, " {:?} {:?}", block.block_type, block.block_flags);
        self.line(label, &header);
        self.children("statement", &block.statements);
      }
      NodeData::Literal(literal) => {
        let value = match &literal.value {
          LiteralValue::Integer(value) => format!("int {value}"),
          LiteralValue::Float(value) => format!("float {:016x}", value.to_bits()),
          LiteralValue::Text(text) => format!("string {:?}", self.text_of(text)),
          LiteralValue::Bool(value) => format!("bool {value}"),
          LiteralValue::Null => "null".to_string(),
          LiteralValue::Array(_) => "array".to_string(),
          LiteralValue::Struct(_) => "struct".to_string(),
        };
        let _ = write!(header, " {value} {:?}", literal.flags);
        self.line(label, &header);
        match &literal.value {
          LiteralValue::Array(array) => {
            self.optional("element_type", array.element_type);
            self.children("member", &array.members);
          }
          LiteralValue::Struct(structure) => {
            self.optional("type", structure.type_expression);
            self.arguments("argument", &structure.arguments);
          }
          _ => {}
        }
      }
      NodeData::Ident(ident) => {
        let name = self.text_of(self.interner.resolve(ident.name));
        let _ = write!(header, " {name} {:?}", ident.flags);
        self.line(label, &header);
      }
      NodeData::UnaryOperator { operator, operand } => {
        let _ = write!(header, " {}", operator.text());
        self.line(label, &header);
        self.child("operand", *operand);
      }
      NodeData::BinaryOperator {
        operator,
        flags,
        left,
        right,
      } => {
        let _ = write!(header, " {} {:?}", operator.text(), flags);
        self.line(label, &header);
        self.child("left", *left);
        self.child("right", *right);
      }
      NodeData::ProcedureBody { block, .. } => {
        self.line(label, &header);
        self.child("block", *block);
      }
      NodeData::ProcedureCall(call) => {
        let _ = write!(header, " {:?}", call.flags);
        self.line(label, &header);
        self.child("procedure", call.procedure_expression);
        self.arguments("argument", &call.arguments);
        if let Some(modification) = &call.context_modification {
          self.children("context", modification);
        }
      }
      NodeData::Context
      | NodeData::DirectiveThis
      | NodeData::DirectiveThrough
      | NodeData::DirectiveCompileTime
      | NodeData::DirectiveContextType
      | NodeData::DirectiveCallerCode
      | NodeData::Placeholder => self.line(label, &header),
      NodeData::While { condition, block } => {
        self.line(label, &header);
        self.child("condition", *condition);
        self.child("block", *block);
      }
      NodeData::If(node) => {
        let _ = write!(header, " {:?}", node.if_flags);
        self.line(label, &header);
        self.child("condition", node.condition);
        self.optional("then", node.then_block);
        self.optional("else", node.else_block);
      }
      NodeData::LoopControl {
        control_type,
        target_ident,
      } => {
        let _ = write!(header, " {control_type:?}");
        self.line(label, &header);
        self.optional("target", *target_ident);
      }
      NodeData::Case(case) => {
        let _ = write!(header, " fallthrough={}", case.marked_as_fallthrough);
        self.line(label, &header);
        self.optional("condition", case.condition);
        self.child("then", case.then_block);
      }
      NodeData::Return { arguments, flags } => {
        let _ = write!(header, " {flags:?}");
        self.line(label, &header);
        self.arguments("value", arguments);
      }
      NodeData::For(node) => {
        let _ = write!(header, " {:?}", node.for_flags);
        self.line(label, &header);
        self.optional("expansion", node.want_replacement_for_expansion);
        self.optional("pointer_expression", node.want_pointer_expression);
        self.optional("reverse_expression", node.want_reverse_expression);
        self.optional("it", node.ident_it);
        self.optional("it_index", node.ident_it_index);
        self.child("iteration", node.iteration_expression);
        self.optional("iteration_right", node.iteration_expression_right);
        self.child("block", node.block);
      }
      NodeData::TypeInstantiation(inst) => {
        let _ = write!(header, " {:?}", inst.inst_flags);
        self.line(label, &header);
        self.optional("pointer_to", inst.pointer_to);
        self.optional("directive_target", inst.type_directive_target);
        self.optional("array_dimension", inst.array_dimension);
        self.optional("element_type", inst.array_element_type);
        self.optional("expression", inst.type_valued_expression);
        self.optional("must_implement", inst.must_implement);
      }
      NodeData::Enum(node) => {
        let _ = write!(
          header,
          " flags_enum={} complete={} specified={}",
          node.is_flags, node.marked_as_complete, node.marked_as_specified
        );
        self.line(label, &header);
        self.optional("internal_type", node.internal_type_inst);
        self.children("note", &node.notes);
        self.optional("block", node.block);
      }
      NodeData::ProcedureHeader(node) => {
        let _ = write!(
          header,
          " {:?} parenthesized_returns={}",
          node.procedure_flags, node.parenthesized_returns
        );
        if let Some(name) = &node.foreign_function_name {
          let _ = write!(header, " foreign={:?}", self.text_of(name));
        }
        if let Some(name) = &node.intrinsic_name {
          let _ = write!(header, " intrinsic={:?}", self.text_of(name));
        }
        if let Some(message) = &node.deprecation_string {
          let _ = write!(header, " deprecated={:?}", self.text_of(message));
        }
        self.line(label, &header);
        self.children("argument", &node.arguments);
        self.children("return", &node.returns);
        self.optional("library", node.library_identifier);
        self.children("modify", &node.modify_directives);
        self.children("note", &node.notes);
        self.optional("body", node.body_or_null);
      }
      NodeData::Struct(node) => {
        let _ = write!(
          header,
          " {:?} has_arguments={}",
          node.textual_flags, node.has_argument_list
        );
        self.line(label, &header);
        self.children("argument", &node.arguments);
        self.optional("alignment", node.alignment_expression);
        self.children("modify", &node.modify_directives);
        self.children("note", &node.notes);
        self.optional("block", node.block);
      }
      NodeData::CommaSeparatedArguments { arguments } => {
        self.line(label, &header);
        for (index, argument) in arguments.iter().enumerate() {
          self.child(
            &format!("argument[{index}] {:?}", argument.modifier),
            argument.node,
          );
        }
      }
      NodeData::DirectiveBytes { expression } | NodeData::DirectiveAddContext { expression } => {
        self.line(label, &header);
        self.child("expression", *expression);
      }
      NodeData::Declaration(declaration) => {
        let _ = write!(header, " {:?}", declaration.flags);
        if let Some(name) = &declaration.program_export_name {
          let _ = write!(header, " export={:?}", self.text_of(name));
        }
        if let Some(symbol) = &declaration.elsewhere_symbol {
          let _ = write!(header, " elsewhere={:?}", self.text_of(symbol));
        }
        self.line(label, &header);
        self.optional("name", declaration.name);
        self.optional("elsewhere_library", declaration.elsewhere_library);
        self.optional("type", declaration.type_inst);
        self.optional("alignment", declaration.alignment_expression);
        self.optional("expression", declaration.expression);
        self.children("note", &declaration.notes);
      }
      NodeData::Cast(cast) => {
        let _ = write!(header, " {:?}", cast.cast_flags);
        self.line(label, &header);
        self.optional("target", cast.target_type);
        self.child("expression", cast.expression);
      }
      NodeData::DirectiveImport(import) => {
        let _ = write!(
          header,
          " {:?} {:?} {:?}",
          self.text_of(&import.name),
          import.import_type,
          import.flags
        );
        self.line(label, &header);
        if let Some(parameters) = &import.module_parameters {
          self.arguments("module_parameter", parameters);
        }
        if let Some(parameters) = &import.program_parameters {
          self.arguments("program_parameter", parameters);
        }
      }
      NodeData::DirectiveLoad { name } => {
        let _ = write!(header, " {:?}", self.text_of(name));
        self.line(label, &header);
      }
      NodeData::DirectiveRun(run) => {
        let _ = write!(header, " {:?}", run.flags);
        self.line(label, &header);
        self.child("procedure", run.procedure);
        self.optional("assertion_string", run.assertion_string);
      }
      NodeData::DirectiveCode { expression, flags } => {
        let _ = write!(header, " {flags:?}");
        self.line(label, &header);
        self.optional("expression", *expression);
      }
      NodeData::DirectivePokeName { module, name } => {
        self.line(label, &header);
        self.child("module", *module);
        self.child("name", *name);
      }
      NodeData::Asm(node) => {
        let features: Vec<String> = node
          .features
          .iter()
          .map(|feature| self.text_of(self.interner.resolve(*feature)))
          .collect();
        let _ = write!(
          header,
          " features={:?} body={:?}",
          features,
          self.text_of(&node.body)
        );
        self.line(label, &header);
      }
      NodeData::DirectiveBake {
        procedure_call,
        bake_type,
      } => {
        let _ = write!(header, " {bake_type:?}");
        self.line(label, &header);
        self.child("call", *procedure_call);
      }
      NodeData::DirectiveModify { block } => {
        self.line(label, &header);
        self.child("block", *block);
      }
      NodeData::DirectiveLibrary {
        name,
        library_flags,
      } => {
        let _ = write!(header, " {:?} {:?}", self.text_of(name), library_flags);
        self.line(label, &header);
      }
      NodeData::ExpressionQuery {
        query_kind,
        expression_to_query,
      } => {
        let _ = write!(header, " {query_kind:?}");
        self.line(label, &header);
        self.child("expression", *expression_to_query);
      }
      NodeData::PushContext {
        to_push,
        block,
        flags,
      } => {
        let _ = write!(header, " {flags:?}");
        self.line(label, &header);
        self.optional("to_push", *to_push);
        self.optional("block", *block);
      }
      NodeData::Note { text } => {
        let name = self.text_of(self.interner.resolve(*text));
        let _ = write!(header, " {name}");
        self.line(label, &header);
      }
      NodeData::DirectivePlace { ident } => {
        self.line(label, &header);
        self.child("ident", *ident);
      }
      NodeData::DirectiveScope { scope_type } => {
        let _ = write!(header, " {scope_type:?}");
        self.line(label, &header);
      }
      NodeData::TypeQuery {
        query_kind,
        type_to_query,
      } => {
        let _ = write!(header, " {query_kind:?}");
        self.line(label, &header);
        self.child("type", *type_to_query);
      }
      NodeData::DirectiveLocation(location) => {
        let _ = write!(
          header,
          " caller={} parentheses={}",
          location.is_caller_location, location.has_parentheses
        );
        self.line(label, &header);
        self.optional("expression", location.expression);
      }
      NodeData::DirectiveModuleParameters(parameters) => {
        self.line(label, &header);
        self.child("module_parameters", parameters.module_parameters);
        self.optional("program_parameters", parameters.program_parameters);
        self.optional("common_code", parameters.common_code);
      }
      NodeData::CompoundDeclaration(compound) => {
        if let Some(operator) = compound.operator_type {
          let _ = write!(header, " {}", operator.text());
        }
        self.line(label, &header);
        self.child("names", compound.comma_separated_assignment);
        self.child("properties", compound.declaration_properties);
      }
      NodeData::Defer {
        block,
        is_backticked,
      } => {
        let _ = write!(header, " backticked={is_backticked}");
        self.line(label, &header);
        self.child("block", *block);
      }
      NodeData::Using(using) => {
        let _ = write!(
          header,
          " {:?} no_parameters={}",
          using.filter_type, using.no_parameters
        );
        self.line(label, &header);
        self.optional("filter", using.filter_expression);
        self.child("expression", using.expression);
      }
      NodeData::DirectiveInsert(insert) => {
        let _ = write!(header, " scope={}", insert.has_scope_redirection);
        self.line(label, &header);
        self.optional("scope_redirection", insert.scope_redirection);
        self.optional("break", insert.break_replacement);
        self.optional("continue", insert.continue_replacement);
        self.optional("remove", insert.remove_replacement);
        self.child("expression", insert.expression);
      }
      NodeData::DirectiveProcedureName { argument } => {
        self.line(label, &header);
        self.optional("argument", *argument);
      }
      NodeData::DirectiveWildcard { index } => {
        let _ = write!(header, " {index}");
        self.line(label, &header);
      }
      NodeData::DirectiveExists(exists) => {
        self.line(label, &header);
        self.child("query", exists.query_expression);
        self.optional("sync", exists.sync_expression);
      }
      NodeData::DirectiveFileInfo { which } => {
        let _ = write!(header, " {}", which.text());
        self.line(label, &header);
      }
    }
  }
}

//! Printing a `Code_*` tree a metaprogram handed back as Jai source.
//!
//! `compiler_modify_procedure` gives the compiler a `Code_Procedure_Body` the
//! metaprogram has edited (**C§3.3**). orangejuice replays a watched workspace
//! rather than stalling it (`docs/spec.md` §6.5), so the modification is
//! carried into the next compilation as *text*: the new body is printed here
//! and spliced over the old one before the file is parsed again.
//!
//! Almost nothing has to be reconstructed. A node this compiler exported and
//! that still holds the bytes it was exported with, with nothing changed
//! anywhere inside the text it was written as, prints as that text — so only
//! the nodes the metaprogram actually made are built back up from their
//! members, and those are the small set a metaprogram can conveniently
//! allocate.

use crate::code::{
  CodeBinaryOperator, CodeBlock, CodeCase, CodeDeclaration, CodeIdent, CodeIf, CodeLiteral,
  CodeLoopControl, CodeNode, CodeProcedureCall, CodeReturn, CodeTypeInstantiation,
  CodeUnaryOperator, CodeWhile, Key, Nodes, literal_type,
};

/// The `Code_Node.Kind` values this printer knows (**C§5.3**).
mod kind {
  pub const BLOCK: u8 = 1;
  pub const LITERAL: u8 = 2;
  pub const IDENT: u8 = 3;
  pub const UNARY_OPERATOR: u8 = 4;
  pub const BINARY_OPERATOR: u8 = 5;
  pub const PROCEDURE_CALL: u8 = 7;
  pub const WHILE: u8 = 9;
  pub const IF: u8 = 10;
  pub const LOOP_CONTROL: u8 = 11;
  pub const CASE: u8 = 12;
  pub const RETURN: u8 = 14;
  pub const TYPE_INSTANTIATION: u8 = 17;
  pub const DECLARATION: u8 = 25;
}

/// `Code_Declaration.Flags`.
const IS_CONSTANT: u32 = 0x1;
const IS_UNINITIALIZED: u32 = 0x80;

/// `Code_Literal.value_flags`.
const FLOAT_LITERAL: u32 = 0x4;
const MINUS_SIGN_LITERAL: u32 = 0x40;

/// `Operator_Type` (**C§5.3**): an ASCII operator is its own character, and
/// everything longer has a number of its own.
fn operator_text(operator: i32) -> Option<&'static str> {
  Some(match operator {
    43 => "+",
    45 => "-",
    42 => "*",
    47 => "/",
    37 => "%",
    38 => "&",
    124 => "|",
    94 => "^",
    126 => "~",
    33 => "!",
    60 => "<",
    62 => ">",
    61 => "=",
    46 => ".",
    131 => "==",
    132 => "!=",
    133 => "&&",
    134 => "||",
    135 => "<=",
    136 => ">=",
    137 => "<<",
    138 => ">>",
    139 => "<<<",
    140 => ">>>",
    145 => "+=",
    146 => "-=",
    147 => "*=",
    148 => "/=",
    149 => "%=",
    150 => "<<=",
    151 => ">>=",
    152 => "<<<=",
    153 => ">>>=",
    154 => "&=",
    155 => "|=",
    156 => "^=",
    157 => "&&=",
    158 => "||=",
    _ => return None,
  })
}

/// Prints one tree, quoting whatever the metaprogram left alone.
pub struct Rewriter<'n> {
  nodes: &'n Nodes,
  changed: Vec<(Key, (u32, u32))>,
  /// The first node kind the printer could not build back up, which is what
  /// the error naming the limit reports.
  unsupported: Option<u8>,
}

impl<'n> Rewriter<'n> {
  pub fn new(nodes: &'n Nodes) -> Self {
    Self {
      nodes,
      changed: nodes.changed_spans(),
      unsupported: None,
    }
  }

  /// The source of any one node, which is what an `#insert` of a `Code` the
  /// metaprogram edited needs: `compiler_get_nodes` hands out a tree it may
  /// write to, and what is spliced has to be what it left there (**C§3.3**).
  pub fn expression(&mut self, node: *const CodeNode) -> Result<String, u8> {
    let mut out = String::new();
    self.node(node, &mut out);
    match self.unsupported {
      Some(kind) => Err(kind),
      None => Ok(out),
    }
  }

  /// The source of the block a `Code_Procedure_Body` now has, braces and all,
  /// or the kind of node that stopped the printer.
  pub fn body(&mut self, block: *const CodeBlock) -> Result<String, u8> {
    let mut out = String::new();
    self.block(block, &mut out);
    match self.unsupported {
      Some(kind) => Err(kind),
      None => Ok(out),
    }
  }

  /// Whether `node` still reads exactly as it was written, which is true only
  /// when neither it nor anything inside its text has been written to.
  fn quotable(&self, node: *const CodeNode) -> Option<&'n [u8]> {
    if !self.nodes.unchanged(node) {
      return None;
    }
    let (key, span) = self.nodes.span_of(node)?;
    let enclosed = self.changed.iter().any(|(other, inner)| {
      other.0 == key.0 && other.1 == key.1 && inner.0 >= span.0 && inner.1 <= span.1
    });
    match enclosed {
      true => None,
      false => self.nodes.source_text(node),
    }
  }

  fn slice(&self, slice: crate::abi::Slice) -> &'n [*const CodeNode] {
    if slice.data.is_null() || slice.count <= 0 {
      return &[];
    }
    unsafe {
      std::slice::from_raw_parts(slice.data.cast::<*const CodeNode>(), slice.count as usize)
    }
  }

  fn block(&mut self, block: *const CodeBlock, out: &mut String) {
    if block.is_null() {
      out.push_str("{\n}");
      return;
    }
    let node = block.cast::<CodeNode>();
    if let Some(text) = self.quotable(node) {
      out.push_str(&String::from_utf8_lossy(text));
      return;
    }
    out.push_str("{\n");
    let block = unsafe { &*block };
    for statement in self.slice(block.statements) {
      let mut printed = String::new();
      self.node(*statement, &mut printed);
      out.push_str(printed.trim_end());
      // A statement that ends in a block closes itself; every other one needs
      // its semicolon, which is the rule `oj-syntax` writes down.
      if !printed.trim_end().ends_with(['}', ';']) {
        out.push(';');
      }
      out.push('\n');
    }
    out.push('}');
  }

  fn node(&mut self, node: *const CodeNode, out: &mut String) {
    if node.is_null() {
      return;
    }
    if let Some(text) = self.quotable(node) {
      out.push_str(&String::from_utf8_lossy(text));
      return;
    }
    let kind = unsafe { (*node).kind };
    match kind {
      kind::BLOCK => self.block(node.cast::<CodeBlock>(), out),
      kind::IDENT => {
        let ident = unsafe { &*node.cast::<CodeIdent>() };
        out.push_str(&unsafe { ident.name.string_lossy() });
      }
      kind::LITERAL => self.literal(node.cast::<CodeLiteral>(), out),
      kind::UNARY_OPERATOR => {
        let unary = unsafe { &*node.cast::<CodeUnaryOperator>() };
        match operator_text(unary.operator_type) {
          Some(text) => {
            out.push_str(text);
            out.push('(');
            self.node(unary.subexpression, out);
            out.push(')');
          }
          None => self.stop(kind),
        }
      }
      kind::BINARY_OPERATOR => self.binary(node.cast::<CodeBinaryOperator>(), out),
      kind::PROCEDURE_CALL => self.call(node.cast::<CodeProcedureCall>(), out),
      kind::RETURN => {
        let ret = unsafe { &*node.cast::<CodeReturn>() };
        out.push_str("return");
        for (index, argument) in self.arguments(ret.arguments_unsorted).iter().enumerate() {
          out.push_str(if index == 0 { " " } else { ", " });
          self.node(*argument, out);
        }
        out.push(';');
      }
      kind::LOOP_CONTROL => {
        let control = unsafe { &*node.cast::<CodeLoopControl>() };
        out.push_str(match control.control_type {
          1 => "continue",
          2 => "remove",
          _ => "break",
        });
        if !control.target_ident.is_null() {
          out.push(' ');
          let ident = unsafe { &*control.target_ident };
          out.push_str(&unsafe { ident.name.string_lossy() });
        }
        out.push(';');
      }
      kind::WHILE => {
        let loop_ = unsafe { &*node.cast::<CodeWhile>() };
        out.push_str("while ");
        self.node(loop_.condition, out);
        out.push(' ');
        self.block(loop_.block, out);
      }
      // `name: T = value`, with whichever of the two halves was written
      // (**L§5.1**). A constant keeps its `::`, and an uninitialized
      // declaration its `---`.
      kind::DECLARATION => {
        let declaration = unsafe { &*node.cast::<CodeDeclaration>() };
        out.push_str(&unsafe { declaration.entry.name.string_lossy() });
        let constant = declaration.flags & IS_CONSTANT != 0;
        out.push_str(match declaration.type_inst.is_null() {
          true => match constant {
            true => " ::",
            false => " :=",
          },
          false => ": ",
        });
        if !declaration.type_inst.is_null() {
          self.node(declaration.type_inst.cast::<CodeNode>(), out);
          out.push_str(match constant {
            true => " :",
            false => " =",
          });
        }
        out.push(' ');
        match declaration.flags & IS_UNINITIALIZED != 0 {
          true => out.push_str("---"),
          false => self.node(declaration.expression, out),
        }
      }
      // A type slot is whatever expression names the type, with the pointer
      // and array wrappers the instantiation records around it (**L§3.13**).
      kind::TYPE_INSTANTIATION => {
        let inst = unsafe { &*node.cast::<CodeTypeInstantiation>() };
        if !inst.pointer_to.is_null() {
          out.push('*');
          self.node(inst.pointer_to.cast::<CodeNode>(), out);
        } else if !inst.array_element_type.is_null() {
          out.push('[');
          self.node(inst.array_dimension, out);
          out.push(']');
          self.node(inst.array_element_type.cast::<CodeNode>(), out);
        } else if !inst.type_valued_expression.is_null() {
          self.node(inst.type_valued_expression, out);
        } else {
          self.stop(kind);
        }
      }
      kind::IF => self.conditional(node.cast::<CodeIf>(), out),
      kind::CASE => {
        let case = unsafe { &*node.cast::<CodeCase>() };
        out.push_str("case ");
        self.node(case.condition, out);
        out.push_str(";\n");
        self.block(case.then_block, out);
      }
      _ => self.stop(kind),
    }
  }

  fn literal(&mut self, literal: *const CodeLiteral, out: &mut String) {
    let literal = unsafe { &*literal };
    match literal.value_type {
      literal_type::NUMBER => {
        if literal.value_flags & FLOAT_LITERAL != 0 {
          out.push_str(&format!("{:?}", unsafe { literal.values.float64 }));
        } else if literal.value_flags & MINUS_SIGN_LITERAL != 0 {
          out.push_str(&unsafe { literal.values.signed }.to_string());
        } else {
          out.push_str(&unsafe { literal.values.unsigned }.to_string());
        }
      }
      literal_type::STRING => {
        out.push('"');
        for byte in unsafe { literal.values.text.bytes() } {
          match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(*byte as char),
            _ => out.push_str(&format!("\\x{byte:02x}")),
          }
        }
        out.push('"');
      }
      literal_type::BOOLEAN => {
        out.push_str(match unsafe { literal.values.unsigned } != 0 {
          true => "true",
          false => "false",
        });
      }
      _ => self.stop(kind::LITERAL),
    }
  }

  fn binary(&mut self, binary: *const CodeBinaryOperator, out: &mut String) {
    let binary = unsafe { &*binary };
    // `a[b]` and `a.b` are written the way they are read; every other operator
    // is parenthesized, since printing loses the precedence the tree had.
    if binary.operator_type == 500 {
      self.node(binary.left, out);
      out.push('[');
      self.node(binary.right, out);
      out.push(']');
      return;
    }
    let Some(text) = operator_text(binary.operator_type) else {
      return self.stop(kind::BINARY_OPERATOR);
    };
    if binary.operator_type == 46 {
      self.node(binary.left, out);
      out.push('.');
      self.node(binary.right, out);
      return;
    }
    out.push('(');
    self.node(binary.left, out);
    out.push(' ');
    out.push_str(text);
    out.push(' ');
    self.node(binary.right, out);
    out.push(')');
  }

  fn call(&mut self, call: *const CodeProcedureCall, out: &mut String) {
    let call = unsafe { &*call };
    self.node(call.procedure_expression, out);
    out.push('(');
    // An argument the call site named keeps its name: `join(xs, separator="!")`
    // means something else read back as four positional arguments (**L§7.4**).
    for (index, argument) in self
      .named_arguments(call.arguments_unsorted)
      .iter()
      .enumerate()
    {
      if index > 0 {
        out.push_str(", ");
      }
      if !argument.name.is_null() {
        let name = unsafe { &*argument.name };
        out.push_str(&unsafe { name.name.string_lossy() });
        out.push_str(" = ");
      }
      self.node(argument.expression, out);
    }
    out.push(')');
  }

  fn conditional(&mut self, conditional: *const CodeIf, out: &mut String) {
    let conditional = unsafe { &*conditional };
    out.push_str("if ");
    self.node(conditional.condition, out);
    out.push(' ');
    self.block(conditional.then_block, out);
    if !conditional.else_block.is_null() {
      out.push_str(" else ");
      self.block(conditional.else_block, out);
    }
  }

  /// The expressions of a `[] Code_Argument`, whose members are a node and the
  /// name it was passed under.
  fn arguments(&self, slice: crate::abi::Slice) -> Vec<*const CodeNode> {
    self
      .named_arguments(slice)
      .iter()
      .map(|argument| argument.expression)
      .collect()
  }

  /// The `[] Code_Argument` itself, for the one caller that has to keep the
  /// name an argument was written under.
  fn named_arguments(&self, slice: crate::abi::Slice) -> &'n [crate::code::CodeArgument] {
    if slice.data.is_null() || slice.count <= 0 {
      return &[];
    }
    unsafe {
      std::slice::from_raw_parts(
        slice.data.cast::<crate::code::CodeArgument>(),
        slice.count as usize,
      )
    }
  }

  fn stop(&mut self, kind: u8) {
    if self.unsupported.is_none() {
      self.unsupported = Some(kind);
    }
  }
}

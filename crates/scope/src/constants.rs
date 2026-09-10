use std::collections::HashMap;

use oj_diag::SourceId;
use oj_lexer::{Interner, Symbol};
use oj_syntax::NodeId;
use oj_syntax::ast::{Ast, LiteralValue, NodeData, OperatorType};

use crate::tree::{DeclId, DeclKind, Resolution, ScopeId, ScopeTree};

/// The constant values M3 can produce without running code: enough to decide
/// the `#if` conditions of the module tree (**L§5.11**, **C§6.5**). Enum
/// members are compared by name, which is all `OS == .LINUX` needs.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
  Bool(bool),
  Int(i64),
  Float(f64),
  String(Box<[u8]>),
  EnumName(Symbol),
  Null,
}

impl ConstValue {
  /// The truth value of a `#if` condition (**L§5.9**): booleans as themselves,
  /// numbers and pointers as "not zero".
  pub fn truth(&self) -> Option<bool> {
    match self {
      Self::Bool(value) => Some(*value),
      Self::Int(value) => Some(*value != 0),
      Self::Float(value) => Some(*value != 0.0),
      Self::Null => Some(false),
      Self::String(_) | Self::EnumName(_) => None,
    }
  }
}

/// The parsed files an evaluation may reach into. A constant declared in one
/// file is routinely read from another, so folding needs the whole program's
/// trees, not just the one it started in.
pub trait AstSource {
  fn ast_of(&self, source: SourceId) -> Option<&Ast>;
}

/// The expression a module parameter took from the `#import` that instantiated
/// its module, and where that was written — the importer's scope and file,
/// since the expression is made of the importer's names (**L§11.3**).
#[derive(Clone, Copy, Debug)]
pub struct ParameterBinding {
  pub scope: ScopeId,
  pub source: SourceId,
  pub expression: NodeId,
}

/// Folds constant expressions far enough to choose `#if` branches. Anything
/// that would need types, `#run` or the interpreter evaluates to `None`, which
/// callers treat as "not yet known" rather than as an error.
pub struct Evaluator<'a> {
  tree: &'a ScopeTree,
  asts: &'a dyn AstSource,
  builtins: &'a HashMap<DeclId, ConstValue>,
  bindings: &'a HashMap<DeclId, ParameterBinding>,
  interner: &'a Interner,
}

const MAX_DEPTH: u32 = 32;

impl<'a> Evaluator<'a> {
  pub fn new(
    tree: &'a ScopeTree,
    asts: &'a dyn AstSource,
    builtins: &'a HashMap<DeclId, ConstValue>,
    bindings: &'a HashMap<DeclId, ParameterBinding>,
    interner: &'a Interner,
  ) -> Self {
    Self {
      tree,
      asts,
      builtins,
      bindings,
      interner,
    }
  }

  pub fn eval(&self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<ConstValue> {
    self.eval_at(scope, source, node, 0)
  }

  /// The value of a `#if` condition, or `None` when M3 cannot decide it.
  pub fn condition(&self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<bool> {
    self.eval(scope, source, node)?.truth()
  }

  fn eval_at(
    &self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    depth: u32,
  ) -> Option<ConstValue> {
    if depth >= MAX_DEPTH {
      return None;
    }
    let ast = self.asts.ast_of(source)?;

    match ast.data(node) {
      NodeData::Literal(literal) => literal_value(literal.value.clone()),
      NodeData::Ident(ident) => self.eval_ident(scope, ident.name, depth),
      NodeData::UnaryOperator { operator, operand } => {
        if *operator == OperatorType::DOT {
          return match ast.data(*operand) {
            NodeData::Ident(ident) => Some(ConstValue::EnumName(ident.name)),
            _ => None,
          };
        }
        let operand = self.eval_at(scope, source, *operand, depth + 1)?;
        unary(*operator, operand)
      }
      NodeData::BinaryOperator {
        operator,
        left,
        right,
        ..
      } => self.eval_binary(scope, source, *operator, *left, *right, depth),
      _ => None,
    }
  }

  fn eval_binary(
    &self,
    scope: ScopeId,
    source: SourceId,
    operator: OperatorType,
    left: NodeId,
    right: NodeId,
    depth: u32,
  ) -> Option<ConstValue> {
    // `&&` and `||` short-circuit, so a decidable left side settles the whole
    // condition even when the right side reaches something M3 cannot fold.
    if operator == OperatorType::LOGICAL_AND || operator == OperatorType::LOGICAL_OR {
      let short_circuit = operator == OperatorType::LOGICAL_OR;
      let left = self.eval_at(scope, source, left, depth + 1)?.truth()?;
      if left == short_circuit {
        return Some(ConstValue::Bool(left));
      }
      let right = self.eval_at(scope, source, right, depth + 1)?.truth()?;
      return Some(ConstValue::Bool(right));
    }

    let left = self.eval_at(scope, source, left, depth + 1)?;
    let right = self.eval_at(scope, source, right, depth + 1)?;
    binary(operator, left, right)
  }

  fn eval_ident(&self, scope: ScopeId, name: Symbol, depth: u32) -> Option<ConstValue> {
    let Resolution::Found(candidates) = self.tree.lookup(scope, name) else {
      return None;
    };
    let [decl] = candidates.as_slice() else {
      return None;
    };
    if let Some(value) = self.builtins.get(decl) {
      return Some(value.clone());
    }

    // A `#module_parameters` parameter is a constant of the module scope, and
    // it is what an `#if` in a parameterized module reads (**L§11.3**): the
    // argument the `#import` gave it, or else the default it declared.
    if let Some(binding) = self.bindings.get(decl) {
      return self.eval_at(binding.scope, binding.source, binding.expression, depth + 1);
    }
    let declaration = self.tree.decl(*decl);
    let is_module_parameter = declaration.kind == DeclKind::Parameter
      && self.tree.scope_kind(declaration.scope).is_program_scope();
    if declaration.kind != DeclKind::Constant && !is_module_parameter {
      return None;
    }
    let source = declaration.source?;
    let ast = self.asts.ast_of(source)?;
    let NodeData::Declaration(payload) = ast.data(declaration.node?) else {
      return None;
    };
    let expression = payload.expression?;
    self.eval_at(declaration.scope, source, expression, depth + 1)
  }

  pub fn interner(&self) -> &Interner {
    self.interner
  }
}

fn literal_value(value: LiteralValue) -> Option<ConstValue> {
  match value {
    LiteralValue::Integer(value) => Some(ConstValue::Int(value as i64)),
    LiteralValue::Float(value) => Some(ConstValue::Float(value)),
    LiteralValue::Text(text) => Some(ConstValue::String(text)),
    LiteralValue::Bool(value) => Some(ConstValue::Bool(value)),
    LiteralValue::Null => Some(ConstValue::Null),
    LiteralValue::Array(_) | LiteralValue::Struct(_) => None,
  }
}

fn unary(operator: OperatorType, operand: ConstValue) -> Option<ConstValue> {
  match (operator, operand) {
    (OperatorType::NOT, value) => Some(ConstValue::Bool(!value.truth()?)),
    (OperatorType::MINUS, ConstValue::Int(value)) => Some(ConstValue::Int(value.checked_neg()?)),
    (OperatorType::MINUS, ConstValue::Float(value)) => Some(ConstValue::Float(-value)),
    (OperatorType::PLUS, value @ (ConstValue::Int(_) | ConstValue::Float(_))) => Some(value),
    (OperatorType::BITWISE_NOT, ConstValue::Int(value)) => Some(ConstValue::Int(!value)),
    _ => None,
  }
}

fn binary(operator: OperatorType, left: ConstValue, right: ConstValue) -> Option<ConstValue> {
  if let Some(equal) = equality(&left, &right) {
    match operator {
      OperatorType::IS_EQUAL => return Some(ConstValue::Bool(equal)),
      OperatorType::IS_NOT_EQUAL => return Some(ConstValue::Bool(!equal)),
      _ => {}
    }
  }

  match (left, right) {
    (ConstValue::Int(left), ConstValue::Int(right)) => integer(operator, left, right),
    (ConstValue::Float(left), ConstValue::Float(right)) => float(operator, left, right),
    (ConstValue::Int(left), ConstValue::Float(right)) => float(operator, left as f64, right),
    (ConstValue::Float(left), ConstValue::Int(right)) => float(operator, left, right as f64),
    _ => None,
  }
}

/// Equality is the one comparison that also works on strings and enum members;
/// unrelated kinds have no answer rather than the answer `false`.
fn equality(left: &ConstValue, right: &ConstValue) -> Option<bool> {
  match (left, right) {
    (ConstValue::EnumName(left), ConstValue::EnumName(right)) => Some(left == right),
    (ConstValue::String(left), ConstValue::String(right)) => Some(left == right),
    (ConstValue::Bool(left), ConstValue::Bool(right)) => Some(left == right),
    (ConstValue::Null, ConstValue::Null) => Some(true),
    _ => None,
  }
}

fn integer(operator: OperatorType, left: i64, right: i64) -> Option<ConstValue> {
  let value = match operator {
    OperatorType::PLUS => ConstValue::Int(left.checked_add(right)?),
    OperatorType::MINUS => ConstValue::Int(left.checked_sub(right)?),
    OperatorType::TIMES => ConstValue::Int(left.checked_mul(right)?),
    OperatorType::DIVIDE => ConstValue::Int(left.checked_div(right)?),
    OperatorType::MODULUS => ConstValue::Int(left.checked_rem(right)?),
    OperatorType::BITWISE_AND => ConstValue::Int(left & right),
    OperatorType::BITWISE_OR => ConstValue::Int(left | right),
    OperatorType::BITWISE_XOR => ConstValue::Int(left ^ right),
    OperatorType::SHIFT_LEFT => ConstValue::Int(left.checked_shl(right.try_into().ok()?)?),
    OperatorType::SHIFT_RIGHT => ConstValue::Int(left.checked_shr(right.try_into().ok()?)?),
    OperatorType::IS_EQUAL => ConstValue::Bool(left == right),
    OperatorType::IS_NOT_EQUAL => ConstValue::Bool(left != right),
    OperatorType::LESS => ConstValue::Bool(left < right),
    OperatorType::GREATER => ConstValue::Bool(left > right),
    OperatorType::LESS_OR_EQUAL => ConstValue::Bool(left <= right),
    OperatorType::GREATER_OR_EQUAL => ConstValue::Bool(left >= right),
    _ => return None,
  };
  Some(value)
}

fn float(operator: OperatorType, left: f64, right: f64) -> Option<ConstValue> {
  let value = match operator {
    OperatorType::PLUS => ConstValue::Float(left + right),
    OperatorType::MINUS => ConstValue::Float(left - right),
    OperatorType::TIMES => ConstValue::Float(left * right),
    OperatorType::DIVIDE => ConstValue::Float(left / right),
    OperatorType::IS_EQUAL => ConstValue::Bool(left == right),
    OperatorType::IS_NOT_EQUAL => ConstValue::Bool(left != right),
    OperatorType::LESS => ConstValue::Bool(left < right),
    OperatorType::GREATER => ConstValue::Bool(left > right),
    OperatorType::LESS_OR_EQUAL => ConstValue::Bool(left <= right),
    OperatorType::GREATER_OR_EQUAL => ConstValue::Bool(left >= right),
    _ => return None,
  };
  Some(value)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tree::{Decl, ScopeKind, Visibility};
  use oj_diag::Span;
  use oj_syntax::ast::DeclarationFlags;

  struct OneFile(Ast);

  impl AstSource for OneFile {
    fn ast_of(&self, _source: SourceId) -> Option<&Ast> {
      Some(&self.0)
    }
  }

  struct Fixture {
    tree: ScopeTree,
    scope: ScopeId,
    interner: Interner,
    builtins: HashMap<DeclId, ConstValue>,
  }

  impl Fixture {
    fn new() -> Self {
      let tree = ScopeTree::new();
      let scope = tree.push_scope(ScopeKind::File, None);
      Self {
        tree,
        scope,
        interner: Interner::new(),
        builtins: HashMap::new(),
      }
    }

    fn builtin(&mut self, name: &str, value: ConstValue) {
      let id = self
        .tree
        .declare(Decl {
          name: self.interner.intern(name.as_bytes()),
          scope: self.scope,
          kind: DeclKind::Builtin,
          visibility: Visibility::Export,
          flags: DeclarationFlags::empty(),
          source: None,
          span: Span::at(0),
          node: None,
          conditional: false,
          branch: None,
          overloadable: false,
        })
        .unwrap();
      self.builtins.insert(id, value);
    }

    /// Parses `source` as a file, declares its constants, and folds the
    /// expression of the constant named `probe`.
    fn fold(&mut self, source: &str, probe: &str) -> Option<ConstValue> {
      let parsed = oj_syntax::parse(source.as_bytes(), SourceId(0), &self.interner);
      assert!(!parsed.has_errors(), "the fixture should parse");
      let NodeData::Block(block) = parsed.ast.data(parsed.root) else {
        unreachable!("a file is a block")
      };

      let mut wanted = None;
      for statement in block.statements.clone() {
        let NodeData::Declaration(declaration) = parsed.ast.data(statement) else {
          continue;
        };
        let Some(NodeData::Ident(ident)) = declaration.name.map(|id| parsed.ast.data(id)) else {
          continue;
        };
        let name = ident.name;
        self
          .tree
          .declare(Decl {
            name,
            scope: self.scope,
            kind: DeclKind::Constant,
            visibility: Visibility::Export,
            flags: declaration.flags,
            source: Some(SourceId(0)),
            span: Span::at(0),
            node: Some(statement),
            conditional: false,
            branch: None,
            overloadable: false,
          })
          .unwrap();
        if self.interner.resolve(name) == probe.as_bytes() {
          wanted = declaration.expression;
        }
      }

      let asts = OneFile(parsed.ast);
      let bindings = HashMap::new();
      let evaluator = Evaluator::new(&self.tree, &asts, &self.builtins, &bindings, &self.interner);
      evaluator.eval(
        self.scope,
        SourceId(0),
        wanted.expect("the probe declaration"),
      )
    }
  }

  #[test]
  fn arithmetic_and_comparisons_fold() {
    let mut fixture = Fixture::new();
    assert_eq!(
      fixture.fold("X :: 2 * 3 + 1;", "X"),
      Some(ConstValue::Int(7))
    );
    assert_eq!(
      fixture.fold("Y :: 8 >= 9;", "Y"),
      Some(ConstValue::Bool(false))
    );
    assert_eq!(
      fixture.fold("Z :: -(1 << 4);", "Z"),
      Some(ConstValue::Int(-16))
    );
  }

  #[test]
  fn a_constant_reads_through_other_constants() {
    let mut fixture = Fixture::new();
    assert_eq!(
      fixture.fold("BITS :: 8;\nWIDE :: BITS * 4 > 16;", "WIDE"),
      Some(ConstValue::Bool(true))
    );
  }

  #[test]
  fn os_compares_against_a_unary_dot_enum_name() {
    let mut fixture = Fixture::new();
    let linux = fixture.interner.intern(b"LINUX");
    fixture.builtin("OS", ConstValue::EnumName(linux));

    assert_eq!(
      fixture.fold("ON_LINUX :: OS == .LINUX;", "ON_LINUX"),
      Some(ConstValue::Bool(true))
    );
    assert_eq!(
      fixture.fold("ON_WINDOWS :: OS == .WINDOWS;", "ON_WINDOWS"),
      Some(ConstValue::Bool(false))
    );
  }

  #[test]
  fn a_decidable_left_side_short_circuits_past_an_unfoldable_right_side() {
    let mut fixture = Fixture::new();
    let linux = fixture.interner.intern(b"LINUX");
    fixture.builtin("OS", ConstValue::EnumName(linux));

    assert_eq!(
      fixture.fold("A :: OS == .LINUX || whatever_this_is;", "A"),
      Some(ConstValue::Bool(true))
    );
    assert_eq!(
      fixture.fold("B :: OS == .WINDOWS && whatever_this_is;", "B"),
      Some(ConstValue::Bool(false))
    );
    assert_eq!(fixture.fold("C :: OS == .LINUX && no_idea;", "C"), None);
  }

  #[test]
  fn anything_needing_the_interpreter_stays_unknown() {
    let mut fixture = Fixture::new();
    assert_eq!(fixture.fold("A :: #run compute();", "A"), None);
    assert_eq!(fixture.fold("B :: size_of(int) == 8;", "B"), None);
    assert_eq!(fixture.fold("C :: undeclared_name;", "C"), None);
  }

  #[test]
  fn conditions_read_numbers_and_null_as_truth_values() {
    assert_eq!(ConstValue::Int(0).truth(), Some(false));
    assert_eq!(ConstValue::Int(3).truth(), Some(true));
    assert_eq!(ConstValue::Null.truth(), Some(false));
    assert_eq!(ConstValue::Bool(true).truth(), Some(true));
    assert_eq!(ConstValue::String(Box::new(*b"x")).truth(), None);
  }
}

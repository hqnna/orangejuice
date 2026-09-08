//! `#run` and `#assert`: the front end's half of compile-time execution
//! (**L§12.1**, **L§12.2**).
//!
//! The checker works out what a run *is* — the type it produces and the one
//! expression or block it evaluates — and then asks whoever installed a
//! [`CompileTime`] engine to produce the value. Nobody has to: `oj dump
//! scopes` and `oj dump types` run without one, and a run whose expression
//! already folded never needs one either, which is what the reference means by
//! "`#run`/`#assert` on constants skip execution" (**C§14**).

use oj_diag::{SourceId, Span};
use oj_scope::ScopeId;
use oj_syntax::ast::{NodeData, NodeId, RunFlags};
use oj_types::TypeId;

use crate::checker::{Checker, Expr};
use crate::constants::Const;

/// One `#run`, as the checker worked it out: everything a back end needs to
/// build the procedure that produces the value.
#[derive(Clone, Debug)]
pub struct RunRequest {
  pub source: SourceId,
  /// The scope the run was written in.
  pub scope: ScopeId,
  /// The `ProcedureHeader` the parser wrapped the run's body in.
  pub header: NodeId,
  /// The block that header carries.
  pub block: NodeId,
  /// The type the run produces, `void` when it produces nothing.
  pub result: TypeId,
  /// The single expression a `#run expr` evaluates. A run whose body is a
  /// block of statements has none and returns through its header instead.
  pub value: Option<NodeId>,
  /// The symbol the run's entry point takes, unique in the compilation.
  pub symbol: String,
  /// Where to point a diagnostic about the run itself.
  pub span: Span,
}

/// What running one `#run` produced.
#[derive(Clone, Debug)]
pub enum RunOutcome {
  /// The run produced this value.
  Value(Const),
  /// The run produced nothing, which is what a statement `#run` does.
  Void,
  /// The run could not be executed. The engine has already reported why.
  Failed,
}

/// Where a `#run` was written and what it evaluates — everything about it that
/// does not depend on the type it turns out to produce.
struct Shape {
  source: SourceId,
  scope: ScopeId,
  header: NodeId,
  block: NodeId,
  value: Option<NodeId>,
  span: Span,
}

/// Whoever can actually execute a `#run`.
///
/// The engine is handed the checker back, because building the run's procedure
/// means asking the same questions any back end asks — and a nested `#run`
/// inside the answer has to be able to reach the engine again, which is why
/// this takes `&self`.
pub trait CompileTime {
  fn evaluate(&self, checker: &mut Checker, request: &RunRequest) -> RunOutcome;
}

impl Checker<'_> {
  /// The type and value of a `#run` or `#assert` expression (**L§12.1**).
  pub(crate) fn run_type(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Expr {
    if let Some(cached) = self.runs.get(&(source, node)) {
      return cached.clone();
    }
    // A run in a branch nobody could decide has not been reached at all, so it
    // neither executes nor reports — and the answer is not memoized, since the
    // same run may be reached for real from somewhere else.
    if self.undecided_static_ifs > 0 {
      return Expr::UNKNOWN;
    }
    if !self.runs_in_flight.insert((source, node)) {
      let span = self
        .ast(source)
        .map_or(Span::at(0), |ast| ast.node(node).span);
      self.error(source, span, "The program contains circular dependencies.");
      return Expr::UNKNOWN;
    }
    let result = self.run_type_uncached(scope, source, node);
    self.runs_in_flight.remove(&(source, node));
    self.runs.insert((source, node), result.clone());
    result
  }

  fn run_type_uncached(&mut self, scope: ScopeId, source: SourceId, node: NodeId) -> Expr {
    let Some(ast) = self.ast(source) else {
      return Expr::UNKNOWN;
    };
    let NodeData::DirectiveRun(run) = ast.data(node) else {
      return Expr::UNKNOWN;
    };
    let run = run.clone();
    let span = ast.node(node).span;
    let header = run.procedure;
    let scope = self.scope_at(source, header, scope);
    let Some(block) = self.run_block(source, header) else {
      return Expr::UNKNOWN;
    };
    let assertion = run.flags.contains(RunFlags::ASSERTION);
    let shape = Shape {
      source,
      scope,
      header,
      block,
      value: self.run_value_expression(source, block, run.flags),
      span,
    };

    // The condition of an `#assert` is the one thing the run evaluates; what
    // it produces is nothing (**L§12.2**).
    let (result, folded) = match shape.value {
      Some(expression) => {
        let expression_scope = self.scope_at(source, expression, scope);
        let info = self.expression_type(expression_scope, source, expression);
        let hardened = self.harden(info.type_id);
        (
          if assertion { TypeId::VOID } else { hardened },
          info.constant,
        )
      }
      None => (self.run_declared_result(source, header, scope), None),
    };

    if assertion {
      return self.assertion(&shape, node, folded);
    }
    if let Some(constant) = folded {
      // A `#run` whose expression is already constant does not execute
      // (**L§12.1**).
      let converted = constant.convert(self.types(), result);
      return Expr::constant(converted.unwrap_or(constant));
    }
    if self.types().is_unknown(result) {
      return Expr::UNKNOWN;
    }

    let request = self.run_request(&shape, result);
    match self.execute(&request) {
      RunOutcome::Value(constant) => Expr::constant(constant),
      RunOutcome::Void => Expr::value(result),
      RunOutcome::Failed => Expr::UNKNOWN,
    }
  }

  /// `#assert cond;` and `#assert cond "message";` (**L§12.2**). The condition
  /// has to be constant; one that is not yet decidable is left alone rather
  /// than reported, since the front end never reports against `unknown`.
  fn assertion(&mut self, shape: &Shape, node: NodeId, folded: Option<Const>) -> Expr {
    let truth = match folded {
      Some(constant) => constant.value.truth(),
      None => {
        let request = self.run_request(shape, TypeId::BOOL);
        match self.execute(&request) {
          RunOutcome::Value(constant) => constant.value.truth(),
          RunOutcome::Void | RunOutcome::Failed => None,
        }
      }
    };
    if truth == Some(false) {
      let message = self.assertion_message(shape.source, node);
      let text = match message {
        Some(message) => format!("Compile-time assertion failed. \"{message}\""),
        None => String::from("Compile-time assertion failed."),
      };
      // The reference underlines the directive itself, not the condition.
      let keyword = Span::new(shape.span.start, shape.span.start + ASSERT_KEYWORD_LENGTH);
      self.error(shape.source, keyword, text);
    }
    Expr::value(TypeId::VOID)
  }

  fn assertion_message(&mut self, source: SourceId, node: NodeId) -> Option<String> {
    let ast = self.ast(source)?;
    let NodeData::DirectiveRun(run) = ast.data(node) else {
      return None;
    };
    let message = run.assertion_string?;
    let NodeData::Literal(literal) = ast.data(message) else {
      return None;
    };
    match &literal.value {
      oj_syntax::ast::LiteralValue::Text(text) => {
        Some(String::from_utf8_lossy(text.as_ref()).into_owned())
      }
      _ => None,
    }
  }

  fn run_request(&mut self, shape: &Shape, result: TypeId) -> RunRequest {
    let symbol = format!("{RUN_SYMBOL_PREFIX}{}", self.next_run_index());
    RunRequest {
      source: shape.source,
      scope: shape.scope,
      header: shape.header,
      block: shape.block,
      result,
      value: shape.value,
      symbol,
      span: shape.span,
    }
  }

  fn execute(&mut self, request: &RunRequest) -> RunOutcome {
    let Some(engine) = self.compile_time.clone() else {
      return RunOutcome::Failed;
    };
    engine.evaluate(self, request)
  }

  /// The block a `#run`'s wrapper header carries.
  fn run_block(&mut self, source: SourceId, header: NodeId) -> Option<NodeId> {
    let ast = self.ast(source)?;
    let NodeData::ProcedureHeader(payload) = ast.data(header) else {
      return None;
    };
    let body = payload.body_or_null?;
    match ast.data(body) {
      NodeData::ProcedureBody { block, .. } => Some(*block),
      _ => None,
    }
  }

  /// The one expression a `#run expr` evaluates. The parser wraps every run in
  /// a block, so a run of an expression is a block of one statement that is
  /// one — anything else returns through the header instead.
  fn run_value_expression(
    &mut self,
    source: SourceId,
    block: NodeId,
    flags: RunFlags,
  ) -> Option<NodeId> {
    if !flags.contains(RunFlags::HAS_IMPLICIT_RETURN_TYPES) {
      return None;
    }
    let ast = self.ast(source)?;
    let NodeData::Block(payload) = ast.data(block) else {
      return None;
    };
    let [only] = payload.statements[..] else {
      return None;
    };
    is_value_expression(ast.data(only)).then_some(only)
  }

  /// The type a `#run -> T { … }` says it produces.
  fn run_declared_result(&mut self, source: SourceId, header: NodeId, scope: ScopeId) -> TypeId {
    let type_id = self.procedure_type(source, header, scope);
    let Some(signature) = self.types().procedure_of(type_id) else {
      return TypeId::VOID;
    };
    match signature.returns.first().copied() {
      Some(result) if signature.returns.len() == 1 => result,
      Some(_) => TypeId::UNKNOWN,
      None => TypeId::VOID,
    }
  }
}

/// Whether a statement is an expression whose value a `#run` yields. A
/// declaration, a `return` or a control-flow statement is a body, not a value.
fn is_value_expression(data: &NodeData) -> bool {
  match data {
    NodeData::BinaryOperator { operator, .. } => !operator.is_assignment(),
    NodeData::Ident(_)
    | NodeData::Literal(_)
    | NodeData::UnaryOperator { .. }
    | NodeData::ProcedureCall(_)
    | NodeData::Cast(_)
    | NodeData::TypeQuery { .. }
    | NodeData::ExpressionQuery { .. }
    | NodeData::TypeInstantiation(_)
    | NodeData::If(_)
    | NodeData::DirectiveRun(_) => true,
    _ => false,
  }
}

/// The prefix of the symbol every compile-time run's entry point takes.
pub const RUN_SYMBOL_PREFIX: &str = "__oj_run_";

/// `#assert`, which is what the reference underlines when one fails.
const ASSERT_KEYWORD_LENGTH: u32 = 7;

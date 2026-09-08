use oj_diag::SourceId;
use oj_scope::ScopeId;
use oj_syntax::ast::{NodeId, OperatorType};
use oj_types::TypeId;

use crate::checker::{Checker, Expr};
use crate::constants::{Const, Value};

impl Checker<'_> {
  /// The constant value of an expression, or `None` when it is not one
  /// (**L§5.11**).
  pub(crate) fn const_value(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<Const> {
    self.expression_type(scope, source, node).constant
  }

  pub(crate) fn const_int(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<i128> {
    self.const_value(scope, source, node)?.as_int()
  }

  /// Folds a binary operator over two constants. Integers fold in `i128` and
  /// are re-narrowed by the caller's unified type; floats fold in `f64`.
  pub(crate) fn fold_binary(
    &mut self,
    operator: OperatorType,
    left: &Expr,
    right: &Expr,
  ) -> Option<Const> {
    let (left, right) = (left.constant.as_ref()?, right.constant.as_ref()?);

    if let (Value::String(a), Value::String(b)) = (&left.value, &right.value) {
      return match operator {
        OperatorType::IS_EQUAL => Some(Const::bool(a == b)),
        OperatorType::IS_NOT_EQUAL => Some(Const::bool(a != b)),
        _ => None,
      };
    }

    if matches!(
      operator,
      OperatorType::LOGICAL_AND | OperatorType::LOGICAL_OR
    ) {
      let (a, b) = (left.value.truth()?, right.value.truth()?);
      return Some(Const::bool(match operator {
        OperatorType::LOGICAL_AND => a && b,
        _ => a || b,
      }));
    }

    if self.types().is_float(left.type_id) || self.types().is_float(right.type_id) {
      let (a, b) = (left.value.as_float()?, right.value.as_float()?);
      return fold_float(operator, a, b);
    }

    let (a, b) = (left.value.as_int()?, right.value.as_int()?);
    fold_int(operator, a, b)
  }
}

fn fold_float(operator: OperatorType, a: f64, b: f64) -> Option<Const> {
  let value = match operator {
    OperatorType::PLUS => a + b,
    OperatorType::MINUS => a - b,
    OperatorType::TIMES => a * b,
    OperatorType::DIVIDE => a / b,
    OperatorType::MODULUS => a % b,
    OperatorType::IS_EQUAL => return Some(Const::bool(a == b)),
    OperatorType::IS_NOT_EQUAL => return Some(Const::bool(a != b)),
    OperatorType::LESS => return Some(Const::bool(a < b)),
    OperatorType::LESS_OR_EQUAL => return Some(Const::bool(a <= b)),
    OperatorType::GREATER => return Some(Const::bool(a > b)),
    OperatorType::GREATER_OR_EQUAL => return Some(Const::bool(a >= b)),
    _ => return None,
  };
  Some(Const::new(TypeId::UNTYPED_FLOAT64, Value::Float(value)))
}

fn fold_int(operator: OperatorType, a: i128, b: i128) -> Option<Const> {
  let value = match operator {
    OperatorType::PLUS => a.checked_add(b)?,
    OperatorType::MINUS => a.checked_sub(b)?,
    OperatorType::TIMES => a.checked_mul(b)?,
    OperatorType::DIVIDE => a.checked_div(b)?,
    OperatorType::MODULUS => a.checked_rem(b)?,
    OperatorType::BITWISE_AND => a & b,
    OperatorType::BITWISE_OR => a | b,
    OperatorType::BITWISE_XOR => a ^ b,
    OperatorType::SHIFT_LEFT => a.checked_shl(u32::try_from(b).ok()?)?,
    OperatorType::SHIFT_RIGHT => a.checked_shr(u32::try_from(b).ok()?)?,
    OperatorType::IS_EQUAL => return Some(Const::bool(a == b)),
    OperatorType::IS_NOT_EQUAL => return Some(Const::bool(a != b)),
    OperatorType::LESS => return Some(Const::bool(a < b)),
    OperatorType::LESS_OR_EQUAL => return Some(Const::bool(a <= b)),
    OperatorType::GREATER => return Some(Const::bool(a > b)),
    OperatorType::GREATER_OR_EQUAL => return Some(Const::bool(a >= b)),
    _ => return None,
  };
  Some(Const::untyped_int(value))
}

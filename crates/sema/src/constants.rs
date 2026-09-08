use oj_lexer::Symbol;
use oj_types::{IntKind, TypeId, Types};

/// A compile-time value (**L§5.11**). Integers are kept as `i128` so that the
/// whole `u64`/`s64` range fits and a literal can be range-checked against any
/// integer type before it takes one.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
  Int(i128),
  Float(f64),
  Bool(bool),
  String(Box<[u8]>),
  Null,
  Type(TypeId),
  /// A `.NAME` whose enum the context has not supplied yet (**L§5.12**).
  EnumName(Symbol),
}

impl Value {
  /// The truth value of the constant in a condition (**L§5.9**).
  pub fn truth(&self) -> Option<bool> {
    match self {
      Self::Bool(value) => Some(*value),
      Self::Int(value) => Some(*value != 0),
      Self::Float(value) => Some(*value != 0.0),
      Self::Null => Some(false),
      Self::String(text) => Some(!text.is_empty()),
      Self::Type(_) | Self::EnumName(_) => None,
    }
  }

  pub fn as_int(&self) -> Option<i128> {
    match self {
      Self::Int(value) => Some(*value),
      Self::Bool(value) => Some(i128::from(*value)),
      _ => None,
    }
  }

  pub fn as_float(&self) -> Option<f64> {
    match self {
      Self::Float(value) => Some(*value),
      Self::Int(value) => Some(*value as f64),
      _ => None,
    }
  }
}

/// A constant and the type it carries. An untyped literal keeps
/// [`TypeId::UNTYPED_INT`] or an untyped float type until a context gives it
/// one (**L§5.10**).
#[derive(Clone, Debug, PartialEq)]
pub struct Const {
  pub type_id: TypeId,
  pub value: Value,
}

impl Const {
  pub fn new(type_id: TypeId, value: Value) -> Self {
    Self { type_id, value }
  }

  pub fn untyped_int(value: i128) -> Self {
    Self::new(TypeId::UNTYPED_INT, Value::Int(value))
  }

  pub fn bool(value: bool) -> Self {
    Self::new(TypeId::BOOL, Value::Bool(value))
  }

  pub fn string(text: Box<[u8]>) -> Self {
    Self::new(TypeId::STRING, Value::String(text))
  }

  pub fn type_value(type_id: TypeId) -> Self {
    Self::new(TypeId::TYPE, Value::Type(type_id))
  }

  pub fn as_int(&self) -> Option<i128> {
    self.value.as_int()
  }

  pub fn as_type(&self) -> Option<TypeId> {
    match self.value {
      Value::Type(id) => Some(id),
      _ => None,
    }
  }

  /// Rewrites the constant into `target`, wrapping integers the way the
  /// reference does for an explicit cast. Returns `None` when the conversion
  /// is not a constant one.
  pub fn convert(&self, types: &Types, target: TypeId) -> Option<Self> {
    if let Some(kind) = types.integer_kind(target) {
      let value = match &self.value {
        Value::Int(value) => *value,
        Value::Bool(value) => i128::from(*value),
        Value::Float(value) => *value as i128,
        _ => return None,
      };
      return Some(Self::new(target, Value::Int(wrap(value, kind))));
    }
    if types.is_float(target) {
      return Some(Self::new(target, Value::Float(self.value.as_float()?)));
    }
    match types.kind(types.underlying(target)) {
      oj_types::TypeKind::Bool => Some(Self::new(target, Value::Bool(self.value.truth()?))),
      oj_types::TypeKind::String if matches!(self.value, Value::String(_)) => {
        Some(Self::new(target, self.value.clone()))
      }
      _ => None,
    }
  }
}

/// Two's-complement truncation into `kind`, which is what `cast,trunc` and an
/// out-of-range enum value do (**L§5.6**).
pub fn wrap(value: i128, kind: IntKind) -> i128 {
  let bits = kind.size() * 8;
  if bits >= 128 {
    return value;
  }
  let masked = (value as u128) & ((1u128 << bits) - 1);
  if kind.is_signed() && masked >> (bits - 1) & 1 == 1 {
    masked as i128 - (1i128 << bits)
  } else {
    masked as i128
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wrapping_truncates_to_the_target_width() {
    assert_eq!(wrap(0x1_0000_0001, IntKind::U32), 1);
    assert_eq!(wrap(255, IntKind::S8), -1);
    assert_eq!(wrap(-1, IntKind::U8), 255);
    assert_eq!(wrap(-1, IntKind::U64), u64::MAX as i128);
    assert_eq!(wrap(300, IntKind::U16), 300);
  }

  #[test]
  fn truth_follows_the_condition_table() {
    assert_eq!(Value::Int(0).truth(), Some(false));
    assert_eq!(Value::Int(-1).truth(), Some(true));
    assert_eq!(Value::Float(-0.0).truth(), Some(false));
    assert_eq!(Value::Null.truth(), Some(false));
    assert_eq!(Value::Type(TypeId::S8).truth(), None);
  }

  #[test]
  fn converting_a_constant_narrows_and_widens_like_a_cast() {
    let types = Types::new();
    let value = Const::untyped_int(300);
    assert_eq!(
      value.convert(&types, TypeId::U8),
      Some(Const::new(TypeId::U8, Value::Int(44)))
    );
    assert_eq!(
      value.convert(&types, TypeId::FLOAT64),
      Some(Const::new(TypeId::FLOAT64, Value::Float(300.0)))
    );
    assert_eq!(
      value.convert(&types, TypeId::BOOL),
      Some(Const::new(TypeId::BOOL, Value::Bool(true)))
    );
  }
}

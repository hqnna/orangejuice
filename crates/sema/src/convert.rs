use oj_types::{ArrayKind, MemberFlags, TypeId, TypeKind};

use crate::checker::{Checker, Expr};
use crate::constants::Value;

/// How far one implicit conversion is, for overload resolution (**L§7.5**).
/// The ordering is the reference's: exact, then `#as`/`isa` steps, then a
/// literal's conversion, then numeric widening, then a polymorph
/// instantiation, then varargs.
pub(crate) const EXACT: u32 = 0;
pub(crate) const AS_STEP: u32 = 1;
pub(crate) const LITERAL: u32 = 8;
pub(crate) const WIDENING: u32 = 16;
pub(crate) const POINTER: u32 = 24;
pub(crate) const ANY: u32 = 64;
pub(crate) const POLYMORPH: u32 = 128;
pub(crate) const VARARGS: u32 = 1024;

/// How deep a chain of `#as` members is worth following (**L§8.4**).
const MAX_AS_DEPTH: u32 = 8;

impl Checker<'_> {
  /// Whether `value` converts implicitly to `target`, and how far it is
  /// (**L§5.10**). `None` means the conversion needs a cast.
  ///
  /// A type the front end could not work out converts to anything at distance
  /// zero, so an unfinished milestone never turns into a type error.
  pub(crate) fn implicit_conversion(&mut self, value: &Expr, target: TypeId) -> Option<u32> {
    let from = value.type_id;
    if from == target {
      return Some(EXACT);
    }
    // A type built out of something the front end could not work out — a
    // `*unknown`, a `[] unknown` — is no more decidable than the thing itself.
    if self.mentions_unknown(from) || self.mentions_unknown(target) {
      return Some(EXACT);
    }
    if matches!(self.types().kind(from), TypeKind::Polymorph(_))
      || matches!(self.types().kind(target), TypeKind::Polymorph(_))
    {
      return Some(POLYMORPH);
    }

    // Every value converts to `Any`, except an untyped struct or array literal
    // and a bare unary-dot enum name, which have no type to record
    // (**L§3.8**). A number literal takes the type it defaults to.
    if target == TypeId::ANY {
      return (!matches!(
        self.types().kind(from),
        TypeKind::UntypedEnum | TypeKind::UntypedLiteral
      ))
      .then_some(ANY);
    }

    if self.types().is_untyped(from) {
      return self.untyped_conversion(value, target);
    }

    // An `isa` variant converts toward its base, one step at a time; a
    // `distinct` one never does (**L§3.11**).
    if let TypeKind::Variant(definition) = *self.types().kind(from) {
      let info = self.types().variant_info(definition);
      if info.is_isa() {
        let base = info.base;
        let mut inner = value.clone();
        inner.type_id = base;
        return self
          .implicit_conversion(&inner, target)
          .map(|distance| distance + AS_STEP);
      }
    }

    // A numeric constant behaves like a literal of its value: it adapts to the
    // type the context asks for as long as it fits (**L§5.10** rule 2).
    if let Some(Value::Int(number)) = value.constant.as_ref().map(|constant| &constant.value)
      && self
        .types()
        .enum_of(self.types().underlying(from))
        .is_none()
      && let Some(kind) = self.types().integer_kind(self.types().underlying(target))
      && self
        .types()
        .enum_of(self.types().underlying(target))
        .is_none()
      && kind.holds(*number)
    {
      return Some(LITERAL);
    }

    // `-> void` returns one void value, and a header with no returns returns
    // none; the reference treats the two as the same procedure (**L§3.1**).
    if let (Some(left), Some(right)) = (
      self.types().procedure_of(from).cloned(),
      self.types().procedure_of(target).cloned(),
    ) && left.arguments == right.arguments
      && left.flags == right.flags
      && left.varargs == right.varargs
      && without_void(&left.returns) == without_void(&right.returns)
    {
      return Some(EXACT);
    }

    if let Some(distance) = self.numeric_conversion(from, target) {
      return Some(distance);
    }
    if let Some(distance) = self.pointer_conversion(value, from, target) {
      return Some(distance);
    }
    if let Some(distance) = self.array_conversion(from, target) {
      return Some(distance);
    }
    // A pointer to a struct is dereferenced where the struct itself is wanted,
    // with a null check (**L§5.10** rule 5).
    if let Some(pointee) = self.types().pointee(from)
      && self.types().struct_of(pointee).is_some()
    {
      let mut dereferenced = value.clone();
      dereferenced.type_id = pointee;
      dereferenced.constant = None;
      return self
        .implicit_conversion(&dereferenced, target)
        .map(|distance| distance + POINTER);
    }
    self.as_conversion(from, target, 0)
  }

  /// Whether a type is, or is built out of, one the front end could not work
  /// out yet.
  pub(crate) fn mentions_unknown(&self, type_id: TypeId) -> bool {
    match self.types().kind(type_id) {
      TypeKind::Unknown => true,
      TypeKind::Pointer(pointee) => self.mentions_unknown(*pointee),
      TypeKind::Array { element, .. } => self.mentions_unknown(*element),
      TypeKind::Variant(definition) => {
        self.mentions_unknown(self.types().variant_info(*definition).base)
      }
      TypeKind::Procedure(signature) => signature
        .arguments
        .iter()
        .chain(&signature.returns)
        .any(|type_id| self.mentions_unknown(*type_id)),
      _ => false,
    }
  }

  /// An untyped literal takes the type the context asks for, if it fits
  /// (**L§5.10** rules 1, 10 and 11).
  fn untyped_conversion(&mut self, value: &Expr, target: TypeId) -> Option<u32> {
    let underlying = self.types().underlying(target);
    match *self.types().kind(value.type_id) {
      TypeKind::UntypedInt => {
        if self.types().is_float(underlying) {
          return Some(LITERAL);
        }
        if let Some(kind) = self.types().integer_kind(underlying) {
          // The literal has to fit exactly: `foo: s32 = 0x0001_0203_0405_0600;`
          // is a loss of information (**L§5.10**). A hexadecimal or binary
          // literal is a bit pattern, so it only has to fit the width.
          let Some(constant) = value.constant.as_ref() else {
            return Some(LITERAL);
          };
          let Value::Int(number) = constant.value else {
            return Some(LITERAL);
          };
          if kind.holds(number) {
            return Some(LITERAL);
          }
          let width = i128::from(u8::try_from(kind.size() * 8).unwrap_or(64));
          return (constant.bit_pattern && number >= 0 && number < (1i128 << width))
            .then_some(LITERAL);
        }
        None
      }
      TypeKind::UntypedFloat(_) => self.types().is_float(underlying).then_some(LITERAL),
      // A bare `.NAME` needs the enum the context supplies (**L§5.12**).
      TypeKind::UntypedEnum => {
        let definition = self.types().enum_of(underlying)?;
        let name = match value.constant.as_ref().map(|constant| &constant.value) {
          Some(Value::EnumName(name)) => *name,
          _ => return Some(LITERAL),
        };
        self
          .types()
          .enum_info(definition)
          .value_of(name)
          .map(|_| LITERAL)
      }
      // `.{…}` and `.[…]` take the struct or array type the context wants
      // (**L§5.7**, **L§5.8**).
      TypeKind::UntypedLiteral => matches!(
        self.types().kind(underlying),
        TypeKind::Struct(_) | TypeKind::Array { .. } | TypeKind::String
      )
      .then_some(LITERAL),
      _ => None,
    }
  }

  /// Widening between numbers (**L§5.10** rules 3 and 4). A runtime integer
  /// never becomes a float, and a float never narrows.
  ///
  /// Nothing here looks through a variant: an `isa` one was already peeled a
  /// step at a time, and a `distinct` one converts to nothing (**L§3.11**). An
  /// enum is not an integer either, in either direction (**L§9**).
  fn numeric_conversion(&mut self, from: TypeId, target: TypeId) -> Option<u32> {
    if let (Some(from_kind), Some(to_kind)) = (
      plain_integer_kind(self, from),
      plain_integer_kind(self, target),
    ) {
      return to_kind.contains_range_of(from_kind).then_some(WIDENING);
    }
    if plain_float_kind(self, from) == Some(oj_types::FloatKind::F32)
      && plain_float_kind(self, target) == Some(oj_types::FloatKind::F64)
    {
      return Some(WIDENING);
    }
    None
  }

  /// The pointer conversions of **L§3.2** and **L§5.10** rule 5.
  fn pointer_conversion(&mut self, value: &Expr, from: TypeId, target: TypeId) -> Option<u32> {
    // A constant string converts to `*u8` and `*s8`; a computed one does not.
    if from == TypeId::STRING
      && value
        .constant
        .as_ref()
        .is_some_and(|constant| matches!(constant.value, Value::String(_)))
      && let Some(pointee) = self.types().pointee(target)
      && matches!(pointee, TypeId::U8 | TypeId::S8)
    {
      return Some(POINTER);
    }

    let (source, destination) = (from, target);
    let is_null = value
      .constant
      .as_ref()
      .is_some_and(|constant| constant.value == Value::Null);
    if is_null
      && matches!(
        self.types().kind(destination),
        TypeKind::Pointer(_) | TypeKind::Procedure(_)
      )
    {
      return Some(EXACT);
    }

    let from_pointee = self.types().pointee(source)?;
    let to_pointee = self.types().pointee(destination)?;
    if from_pointee == to_pointee {
      return Some(EXACT);
    }
    if to_pointee == TypeId::VOID || from_pointee == TypeId::VOID {
      return Some(POINTER);
    }
    // `*[N] T` is the address of the first element (**L§3.2**).
    if let Some((element, ArrayKind::Fixed(_))) = self.types().array_of(from_pointee)
      && element == to_pointee
    {
      return Some(POINTER);
    }
    // `count` and `data` sit at the same offsets in `[] T` and `[..] T`, so a
    // pointer to one is a pointer to the other (**L§3.3**).
    if let (Some((from_element, ArrayKind::Resizable)), Some((to_element, ArrayKind::View))) = (
      self.types().array_of(from_pointee),
      self.types().array_of(to_pointee),
    ) && from_element == to_element
    {
      return Some(POINTER);
    }
    // `*Derived` → `*Base` follows the same `#as` chain the values do.
    self
      .as_conversion(from_pointee, to_pointee, 0)
      .map(|distance| distance + AS_STEP)
  }

  /// Fixed and resizable arrays convert to views, and nothing else does
  /// (**L§3.3**).
  fn array_conversion(&mut self, from: TypeId, target: TypeId) -> Option<u32> {
    let (from_element, from_kind) = self.types().array_of(from)?;
    let (to_element, to_kind) = self.types().array_of(target)?;
    if to_kind != ArrayKind::View || from_kind == ArrayKind::View || from_element != to_element {
      return None;
    }
    Some(POINTER)
  }

  /// `S` converts to the type of any `#as` member it has, transitively
  /// (**L§8.4**). `#Context` reaches `Context_Base` this way, since its `base`
  /// member is `#as using`.
  pub(crate) fn as_conversion(&mut self, from: TypeId, target: TypeId, depth: u32) -> Option<u32> {
    if depth >= MAX_AS_DEPTH {
      return None;
    }
    self.complete_type(from);
    let definition = self.types().struct_of(self.types().underlying(from))?;
    let candidates: Vec<TypeId> = self
      .types()
      .struct_info(definition)
      .members
      .iter()
      .filter(|member| {
        member.flags.contains(MemberFlags::AS) && !member.flags.contains(MemberFlags::IMPORTED)
      })
      .map(|member| member.type_id)
      .collect();

    candidates
      .into_iter()
      .filter_map(|member| {
        if member == target {
          return Some(AS_STEP);
        }
        self
          .as_conversion(member, target, depth + 1)
          .map(|distance| distance + AS_STEP)
      })
      .min()
  }
}

/// The integer type a type *is*, rather than the one it wraps.
fn plain_integer_kind(checker: &Checker<'_>, type_id: TypeId) -> Option<oj_types::IntKind> {
  match checker.types().kind(type_id) {
    TypeKind::Integer(kind) => Some(*kind),
    _ => None,
  }
}

fn plain_float_kind(checker: &Checker<'_>, type_id: TypeId) -> Option<oj_types::FloatKind> {
  match checker.types().kind(type_id) {
    TypeKind::Float(kind) => Some(*kind),
    _ => None,
  }
}

/// A lone `void` return is the same as none at all (**L§3.1**).
fn without_void(returns: &[TypeId]) -> &[TypeId] {
  match returns {
    [TypeId::VOID] => &[],
    other => other,
  }
}

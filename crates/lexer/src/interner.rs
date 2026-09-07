use std::borrow::Cow;
use std::sync::RwLock;

use indexmap::IndexSet;

/// An interned identifier or note name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(pub u32);

/// The compiler-wide name table. Identifiers are ASCII (**L§2.3**) but a note
/// name is whatever `@` was followed by, so names are interned as bytes.
///
/// Interning leaks each new name, which bounds the leak by the number of
/// distinct names in a run and lets [`Interner::resolve`] hand out a
/// `&'static [u8]` to any thread without holding the lock.
#[derive(Default)]
pub struct Interner {
  names: RwLock<IndexSet<&'static [u8]>>,
}

impl Interner {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn intern(&self, name: &[u8]) -> Symbol {
    if let Some(index) = self
      .names
      .read()
      .expect("the interner is not poisoned")
      .get_index_of(name)
    {
      return Symbol(index as u32);
    }

    let mut names = self.names.write().expect("the interner is not poisoned");
    if let Some(index) = names.get_index_of(name) {
      return Symbol(index as u32);
    }
    let leaked: &'static [u8] = Box::leak(name.to_vec().into_boxed_slice());
    Symbol(names.insert_full(leaked).0 as u32)
  }

  pub fn resolve(&self, symbol: Symbol) -> &'static [u8] {
    self
      .names
      .read()
      .expect("the interner is not poisoned")
      .get_index(symbol.0 as usize)
      .copied()
      .expect("the symbol was issued by this interner")
  }

  pub fn resolve_lossy(&self, symbol: Symbol) -> Cow<'static, str> {
    String::from_utf8_lossy(self.resolve(symbol))
  }

  pub fn len(&self) -> usize {
    self
      .names
      .read()
      .expect("the interner is not poisoned")
      .len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn equal_names_intern_to_one_symbol() {
    let interner = Interner::new();

    let first = interner.intern(b"read_package");
    let second = interner.intern(b"read_package");
    let other = interner.intern(b"write_package");

    assert_eq!(first, second);
    assert_ne!(first, other);
    assert_eq!(interner.len(), 2);
  }

  #[test]
  fn symbols_resolve_back_to_their_bytes() {
    let interner = Interner::new();
    let symbol = interner.intern("café".as_bytes());

    assert_eq!(interner.resolve(symbol), "café".as_bytes());
    assert_eq!(interner.resolve_lossy(symbol), "café");
  }

  #[test]
  fn a_name_that_is_not_utf8_resolves_lossily() {
    let interner = Interner::new();
    let symbol = interner.intern(&[b'a', 0xff]);

    assert_eq!(interner.resolve(symbol), [b'a', 0xff]);
    assert_eq!(interner.resolve_lossy(symbol), "a\u{fffd}");
  }
}

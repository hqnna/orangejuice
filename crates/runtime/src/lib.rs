//! The data segments a compile-time execution owns.
//!
//! A global a `#run` touches has to live somewhere the compiler can address,
//! write to, read back and — at the end — copy into the executable. That
//! storage is here rather than in the JIT, because the same bytes outlive any
//! one JIT module: two `#run`s in the same compilation see one `x`
//! (`docs/spec.md` §6.5).
//!
//! Every allocation remembers the bytes it started with, so [`Segments::reset`]
//! can put the writable ones back the way the front end laid them out —
//! that is the reset of **L§12.3**, and `#no_reset` is exactly the allocation
//! it skips.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::collections::HashMap;

/// Which segment a global lives in, which is what decides whether compile-time
/// writes survive into the executable (**L§4.7**, **L§12.3**).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Segment {
  /// Ordinary global data: reset to its initializer when the executable is
  /// written.
  Writable,
  /// `#no_reset`: whatever compile time left in it is what the program starts
  /// with.
  NoReset,
  /// Literals and the type table: never written, never reset.
  ReadOnly,
}

/// One global's storage in the compiler's own address space.
#[derive(Debug)]
struct Allocation {
  pointer: *mut u8,
  layout: Layout,
  size: usize,
  segment: Segment,
  /// The bytes the front end asked for, which a reset puts back.
  initial: Box<[u8]>,
}

/// The compile-time data segments of one compilation.
///
/// Allocations are keyed by the symbol the back end gave the global, which is
/// what the JIT resolves against.
#[derive(Debug, Default)]
pub struct Segments {
  allocations: Vec<Allocation>,
  by_symbol: HashMap<String, usize>,
}

// The allocations are plain bytes the compiler owns; nothing inside them is
// shared with another thread while a `#run` is executing.
unsafe impl Send for Segments {}

impl Segments {
  pub fn new() -> Self {
    Self::default()
  }

  /// The address of a global, allocating it the first time it is asked for.
  /// A second request with the same symbol gets the same address, which is how
  /// two `#run`s share one global.
  pub fn allocate(
    &mut self,
    symbol: &str,
    size: u64,
    alignment: u64,
    segment: Segment,
    initial: &[u8],
  ) -> usize {
    if let Some(index) = self.by_symbol.get(symbol) {
      return self.allocations[*index].pointer as usize;
    }
    // A zero-sized global still needs a distinct address, since the JIT
    // resolves its symbol like any other.
    let size = size.max(1) as usize;
    let alignment = alignment.max(1) as usize;
    let layout = Layout::from_size_align(size, alignment)
      .unwrap_or_else(|_| Layout::from_size_align(size, 1).expect("a byte-aligned layout"));
    let pointer = unsafe { alloc_zeroed(layout) };
    assert!(
      !pointer.is_null(),
      "out of memory for a compile-time global"
    );
    let mut bytes = vec![0u8; size];
    let copied = initial.len().min(size);
    bytes[..copied].copy_from_slice(&initial[..copied]);
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, size) };

    self
      .by_symbol
      .insert(symbol.to_string(), self.allocations.len());
    self.allocations.push(Allocation {
      pointer,
      layout,
      size,
      segment,
      initial: bytes.into_boxed_slice(),
    });
    pointer as usize
  }

  pub fn address(&self, symbol: &str) -> Option<usize> {
    self
      .by_symbol
      .get(symbol)
      .map(|index| self.allocations[*index].pointer as usize)
  }

  /// The bytes a global holds now, which is what a `#no_reset` writes into the
  /// executable.
  pub fn bytes(&self, symbol: &str) -> Option<&[u8]> {
    let index = *self.by_symbol.get(symbol)?;
    let allocation = &self.allocations[index];
    Some(unsafe { std::slice::from_raw_parts(allocation.pointer, allocation.size) })
  }

  /// Writes an address into a global, which is what the type table's pointers
  /// into itself need once the image has one (**L§17**).
  pub fn write_pointer(&mut self, symbol: &str, at: u64, value: u64) {
    let Some(index) = self.by_symbol.get(symbol) else {
      return;
    };
    let allocation = &self.allocations[*index];
    let at = at as usize;
    if at + 8 > allocation.size {
      return;
    }
    unsafe {
      std::ptr::copy_nonoverlapping(value.to_le_bytes().as_ptr(), allocation.pointer.add(at), 8);
    }
  }

  pub fn segment(&self, symbol: &str) -> Option<Segment> {
    let index = *self.by_symbol.get(symbol)?;
    Some(self.allocations[index].segment)
  }

  /// Puts every writable global back the way it started, which is what the
  /// reference does when it writes the executable (**L§12.3**). A `#no_reset`
  /// allocation keeps whatever compile time left in it.
  pub fn reset(&mut self) {
    for allocation in &mut self.allocations {
      if allocation.segment != Segment::Writable {
        continue;
      }
      unsafe {
        std::ptr::copy_nonoverlapping(
          allocation.initial.as_ptr(),
          allocation.pointer,
          allocation.size,
        );
      }
    }
  }

  pub fn len(&self) -> usize {
    self.allocations.len()
  }

  pub fn is_empty(&self) -> bool {
    self.allocations.is_empty()
  }
}

impl Drop for Segments {
  fn drop(&mut self) {
    for allocation in &self.allocations {
      unsafe { dealloc(allocation.pointer, allocation.layout) };
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn one_symbol_keeps_one_address() {
    let mut segments = Segments::new();
    let first = segments.allocate("x", 8, 8, Segment::Writable, &[]);
    let second = segments.allocate("x", 8, 8, Segment::Writable, &[]);
    assert_eq!(first, second);
    assert_eq!(segments.len(), 1);
  }

  #[test]
  fn an_allocation_starts_out_as_its_initializer() {
    let mut segments = Segments::new();
    segments.allocate("x", 4, 4, Segment::Writable, &[1, 2, 3, 4]);
    assert_eq!(segments.bytes("x"), Some(&[1u8, 2, 3, 4][..]));
  }

  #[test]
  fn a_reset_restores_writable_data_and_leaves_no_reset_alone() {
    let mut segments = Segments::new();
    let writable = segments.allocate("x", 4, 4, Segment::Writable, &[0, 0, 0, 0]);
    let kept = segments.allocate("y", 4, 4, Segment::NoReset, &[0, 0, 0, 0]);
    unsafe {
      std::ptr::write_bytes(writable as *mut u8, 7, 4);
      std::ptr::write_bytes(kept as *mut u8, 9, 4);
    }
    segments.reset();
    assert_eq!(segments.bytes("x"), Some(&[0u8, 0, 0, 0][..]));
    assert_eq!(segments.bytes("y"), Some(&[9u8, 9, 9, 9][..]));
  }

  #[test]
  fn alignment_is_honoured() {
    let mut segments = Segments::new();
    let address = segments.allocate("aligned", 32, 16, Segment::ReadOnly, &[]);
    assert_eq!(address % 16, 0);
    assert_eq!(segments.segment("aligned"), Some(Segment::ReadOnly));
  }
}

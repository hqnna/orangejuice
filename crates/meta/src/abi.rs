//! The shapes a `#compiler` procedure sees across the call boundary.
//!
//! These mirror Preload and `Compiler.jai` structs byte for byte (**L§17**,
//! **C§3.2**); `crates/meta/tests/layout.rs` measures the real ones against
//! them so a change in the distribution cannot pass unnoticed.

/// `string :: struct { count: s64; data: *u8; }` (**L§3.4**).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Str {
  pub count: i64,
  pub data: *const u8,
}

impl Str {
  pub const EMPTY: Self = Self {
    count: 0,
    data: std::ptr::null(),
  };

  /// # Safety
  /// `data` must point at `count` readable bytes for as long as the result is
  /// used, which for a string the compiler hands back means the arena of the
  /// [`Meta`](crate::Meta) that produced it.
  pub unsafe fn bytes(&self) -> &[u8] {
    if self.data.is_null() || self.count <= 0 {
      return &[];
    }
    unsafe { std::slice::from_raw_parts(self.data, self.count as usize) }
  }

  /// # Safety
  /// As [`Str::bytes`].
  pub unsafe fn string_lossy(&self) -> String {
    String::from_utf8_lossy(unsafe { self.bytes() }).into_owned()
  }
}

/// `[] T :: struct { count: s64; data: *T; }` (**L§3.3**).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Slice {
  pub count: i64,
  pub data: *const u8,
}

impl Slice {
  pub const EMPTY: Self = Self {
    count: 0,
    data: std::ptr::null(),
  };
}

/// `Source_Code_Location` (**L§17**).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SourceCodeLocation {
  pub fully_pathed_filename: Str,
  pub line_number: i64,
  pub character_number: i64,
}

/// `Version_Info` (**C§3.3**).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VersionInfo {
  pub major: i32,
  pub minor: i32,
  pub micro: i32,
}

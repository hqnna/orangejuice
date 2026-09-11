//! Where the tests find the distribution orangejuice ships.
//!
//! orangejuice *is* a Jai distribution: `modules/` sits at the root of this
//! repository the way it does in any other, and `docs/examples/` is the
//! acceptance suite (`docs/spec.md` §8). Nothing here looks for a reference
//! compiler — there is none to look for.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// The modules directory a compilation reads, overriding the one the compiler
/// would find beside its own binary.
pub const MODULES_ENV: &str = "OJ_MODULES";

/// The root of the cargo workspace this crate is built from, which is also the
/// root of the distribution it ships.
///
/// Canonical so that a path printed in a diagnostic is readable rather than
/// `crates/testsupport/../../modules/...`. The compiler does not need the help:
/// `module_identity` in `oj-scope` is what makes two spellings of one file one
/// module, and `a_module_reached_two_ways_is_one_module` holds it to that.
pub fn workspace_root() -> &'static Path {
  static ROOT: LazyLock<PathBuf> = LazyLock::new(|| {
    let relative = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    std::fs::canonicalize(relative).unwrap_or_else(|_| relative.to_path_buf())
  });
  &ROOT
}

/// The distribution under test: `modules/` and `docs/examples/` live here.
pub fn distribution() -> &'static Path {
  workspace_root()
}

/// The standard modules (**L§11.1**).
pub fn modules() -> PathBuf {
  distribution().join("modules")
}

/// The acceptance suite: whole programs with the output each must print. It
/// lives under `docs/` because it is the language tour as much as it is a
/// test (`docs/examples/README.md`).
pub fn examples() -> PathBuf {
  distribution().join("docs").join("examples")
}

/// Points a compilation this process drives at the distribution under test,
/// rather than at whatever sits beside the test binary.
///
/// # Safety
///
/// Sets a process-wide environment variable. Cargo runs each integration test
/// binary in its own process, but the tests *within* one run on threads, so a
/// binary that calls this must do so from every test that needs it and must
/// not otherwise write the environment.
pub unsafe fn use_own_modules() {
  unsafe { std::env::set_var(MODULES_ENV, modules()) };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_repository_is_a_distribution() {
    assert!(modules().join("Preload.jai").is_file());
    assert!(modules().join("Basic").join("module.jai").is_file());
    assert!(examples().is_dir());
  }
}

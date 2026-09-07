use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub const JAI_DIR_ENV: &str = "OJ_JAI_DIR";

pub const MISSING_JAI_DIR_MESSAGE: &str = concat!(
  "skipping: no jai distribution found. ",
  "Set OJ_JAI_DIR to a directory containing modules/, ",
  "or unpack the beta 0.2.009 distribution into vendor/jai."
);

/// The root of the cargo workspace this crate is built from.
pub fn workspace_root() -> &'static Path {
  Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// The jai distribution to test against, or `None` when it is not installed.
pub fn jai_dir() -> Option<PathBuf> {
  resolve_jai_dir(std::env::var_os(JAI_DIR_ENV).as_deref(), workspace_root())
}

pub fn resolve_jai_dir(env: Option<&OsStr>, workspace_root: &Path) -> Option<PathBuf> {
  let candidate = match env {
    Some(value) if !value.is_empty() => PathBuf::from(value),
    _ => workspace_root.join("vendor").join("jai"),
  };
  candidate.join("modules").is_dir().then_some(candidate)
}

/// Binds the jai distribution, or returns from the calling test with a message
/// explaining why it was skipped.
#[macro_export]
macro_rules! jai_dir_or_skip {
  () => {
    match $crate::jai_dir() {
      Some(dir) => dir,
      None => {
        eprintln!("{}", $crate::MISSING_JAI_DIR_MESSAGE);
        return;
      }
    }
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_environment_overrides_the_vendored_distribution() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("modules")).unwrap();

    let workspace = tempfile::tempdir().unwrap();
    let resolved = resolve_jai_dir(Some(temp.path().as_os_str()), workspace.path());

    assert_eq!(resolved.as_deref(), Some(temp.path()));
  }

  #[test]
  fn a_distribution_without_modules_is_not_a_distribution() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();

    assert_eq!(
      resolve_jai_dir(Some(temp.path().as_os_str()), workspace.path()),
      None
    );
    assert_eq!(resolve_jai_dir(None, workspace.path()), None);
  }

  #[test]
  fn the_default_location_is_vendor_jai_under_the_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    let vendored = workspace.path().join("vendor").join("jai");
    std::fs::create_dir_all(vendored.join("modules")).unwrap();

    assert_eq!(
      resolve_jai_dir(None, workspace.path()).as_deref(),
      Some(vendored.as_path())
    );
    assert_eq!(
      resolve_jai_dir(Some(OsStr::new("")), workspace.path()).as_deref(),
      Some(vendored.as_path())
    );
  }
}

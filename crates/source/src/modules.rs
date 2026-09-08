use std::fmt;
use std::path::{Component, Path, PathBuf};

/// The directories searched by `#import "Name"`, in order (**L§11.2**). The
/// default is `[<dir of the first source file>/modules, <compiler dir>/modules]`
/// so that a local `modules` folder overrides the distribution's; `-import_dir`
/// entries are prepended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportPath {
  directories: Vec<PathBuf>,
}

impl ImportPath {
  pub fn new(directories: impl IntoIterator<Item = PathBuf>) -> Self {
    Self {
      directories: directories.into_iter().collect(),
    }
  }

  /// The default search path for a build whose first source file is
  /// `first_file` and whose distribution lives in `jai_dir`.
  pub fn default_for(first_file: &Path, jai_dir: &Path) -> Self {
    Self::new([
      directory_of(first_file).join("modules"),
      jai_dir.join("modules"),
    ])
  }

  /// Adds a `-import_dir` directory, which takes precedence over everything
  /// already on the path.
  pub fn prepend(&mut self, directory: PathBuf) {
    self.directories.insert(0, directory);
  }

  pub fn directories(&self) -> &[PathBuf] {
    &self.directories
  }
}

/// How a module was found: `Name.jai` or `Name/module.jai` (**L§11.2**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
  SingleFile,
  Directory,
}

/// A located module: the file to parse first, and the directory that its
/// `#load`s and relative imports are resolved against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedModule {
  pub kind: ModuleKind,
  pub entry: PathBuf,
  pub directory: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleError {
  NotFound { name: String },
  Ambiguous { name: String, directory: PathBuf },
}

impl fmt::Display for ModuleError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NotFound { name } => write!(f, "Could not find module '{name}'."),
      Self::Ambiguous { name, directory } => write!(
        f,
        "Module '{name}' is ambiguous: {}/{name}.jai and {}/{name}/module.jai both exist.",
        directory.display(),
        directory.display()
      ),
    }
  }
}

/// The name a module is looked up by. Module names may contain `-`, digits and
/// `/` (`Frotz-6_9_105`, `Toolchains/Android`), but never `.` or an absolute
/// root: those would escape the search directories.
pub fn is_valid_module_name(name: &str) -> bool {
  !name.is_empty()
    && Path::new(name)
      .components()
      .all(|component| matches!(component, Component::Normal(_)))
}

/// Searches `path` for `name`, taking the first directory that holds either
/// spelling of the module; holding both is an ambiguity error (**L§11.2**).
pub fn resolve_module(name: &str, path: &ImportPath) -> Result<ResolvedModule, ModuleError> {
  if !is_valid_module_name(name) {
    return Err(ModuleError::NotFound {
      name: name.to_string(),
    });
  }

  for directory in path.directories() {
    let single_file = directory.join(format!("{name}.jai"));
    let module_directory = directory.join(name);
    let entry = module_directory.join("module.jai");

    match (single_file.is_file(), entry.is_file()) {
      (true, true) => {
        return Err(ModuleError::Ambiguous {
          name: name.to_string(),
          directory: directory.clone(),
        });
      }
      (true, false) => {
        return Ok(ResolvedModule {
          kind: ModuleKind::SingleFile,
          directory: directory_of(&single_file).to_path_buf(),
          entry: single_file,
        });
      }
      (false, true) => {
        return Ok(ResolvedModule {
          kind: ModuleKind::Directory,
          directory: module_directory,
          entry,
        });
      }
      (false, false) => {}
    }
  }

  Err(ModuleError::NotFound {
    name: name.to_string(),
  })
}

/// `#import,dir "../Vulkan_Render"`: a directory module named by a path
/// relative to the importing file (**L§11.2**).
pub fn resolve_directory_module(from_file: &Path, relative: &str) -> ResolvedModule {
  let directory = directory_of(from_file).join(relative);
  ResolvedModule {
    kind: ModuleKind::Directory,
    entry: directory.join("module.jai"),
    directory,
  }
}

/// `#import,file "path/to/file.jai"`: one file used as a module, relative to
/// the importing file (**L§11.2**).
pub fn resolve_file_module(from_file: &Path, relative: &str) -> ResolvedModule {
  let entry = directory_of(from_file).join(relative);
  ResolvedModule {
    kind: ModuleKind::SingleFile,
    directory: directory_of(&entry).to_path_buf(),
    entry,
  }
}

/// `#load "relative/path.jai"`, resolved against the loading file's directory
/// (**L§11.1**).
pub fn resolve_load(from_file: &Path, relative: &str) -> PathBuf {
  directory_of(from_file).join(relative)
}

fn directory_of(file: &Path) -> &Path {
  file.parent().unwrap_or(Path::new("."))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn touch(path: PathBuf) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"").unwrap();
    path
  }

  #[test]
  fn the_default_path_prefers_a_local_modules_folder() {
    let path = ImportPath::default_for(Path::new("/w/src/first.jai"), Path::new("/opt/jai"));

    assert_eq!(
      path.directories(),
      [
        PathBuf::from("/w/src/modules"),
        PathBuf::from("/opt/jai/modules")
      ]
    );
  }

  #[test]
  fn an_import_dir_takes_precedence_over_the_default_path() {
    let mut path = ImportPath::default_for(Path::new("/w/first.jai"), Path::new("/opt/jai"));
    path.prepend(PathBuf::from("/w/extra"));

    assert_eq!(path.directories()[0], PathBuf::from("/w/extra"));
    assert_eq!(path.directories().len(), 3);
  }

  #[test]
  fn a_single_file_module_resolves_to_that_file() {
    let dir = tempfile::tempdir().unwrap();
    let entry = touch(dir.path().join("modules/Basic.jai"));
    let path = ImportPath::new([dir.path().join("modules")]);

    assert_eq!(
      resolve_module("Basic", &path).unwrap(),
      ResolvedModule {
        kind: ModuleKind::SingleFile,
        entry,
        directory: dir.path().join("modules"),
      }
    );
  }

  #[test]
  fn a_directory_module_resolves_to_its_module_jai() {
    let dir = tempfile::tempdir().unwrap();
    let entry = touch(dir.path().join("modules/Sound/module.jai"));
    let path = ImportPath::new([dir.path().join("modules")]);

    assert_eq!(
      resolve_module("Sound", &path).unwrap(),
      ResolvedModule {
        kind: ModuleKind::Directory,
        entry,
        directory: dir.path().join("modules/Sound"),
      }
    );
  }

  #[test]
  fn a_module_name_may_name_a_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path().join("modules/Toolchains/Android.jai"));
    let path = ImportPath::new([dir.path().join("modules")]);

    assert_eq!(
      resolve_module("Toolchains/Android", &path).unwrap().kind,
      ModuleKind::SingleFile
    );
  }

  #[test]
  fn the_first_directory_holding_the_module_wins() {
    let dir = tempfile::tempdir().unwrap();
    let local = touch(dir.path().join("local/Basic.jai"));
    touch(dir.path().join("dist/Basic.jai"));
    let path = ImportPath::new([dir.path().join("local"), dir.path().join("dist")]);

    assert_eq!(resolve_module("Basic", &path).unwrap().entry, local);
  }

  #[test]
  fn both_spellings_in_one_directory_are_ambiguous() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path().join("modules/Basic.jai"));
    touch(dir.path().join("modules/Basic/module.jai"));
    let path = ImportPath::new([dir.path().join("modules")]);

    assert_eq!(
      resolve_module("Basic", &path),
      Err(ModuleError::Ambiguous {
        name: "Basic".to_string(),
        directory: dir.path().join("modules"),
      })
    );
  }

  #[test]
  fn a_module_that_is_nowhere_on_the_path_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let path = ImportPath::new([dir.path().to_path_buf()]);

    assert_eq!(
      resolve_module("Basic", &path),
      Err(ModuleError::NotFound {
        name: "Basic".to_string()
      })
    );
  }

  #[test]
  fn a_module_name_cannot_escape_the_search_directories() {
    assert!(is_valid_module_name("Toolchains/Android"));
    assert!(is_valid_module_name("Frotz-6_9_105"));
    assert!(!is_valid_module_name("../Basic"));
    assert!(!is_valid_module_name("/etc/passwd"));
    assert!(!is_valid_module_name(""));
  }

  #[test]
  fn loads_and_relative_imports_resolve_against_the_importing_file() {
    let from = Path::new("/w/modules/Sound/module.jai");

    assert_eq!(
      resolve_load(from, "internal/mixer.jai"),
      PathBuf::from("/w/modules/Sound/internal/mixer.jai")
    );
    assert_eq!(
      resolve_file_module(from, "../shared/trees.jai"),
      ResolvedModule {
        kind: ModuleKind::SingleFile,
        entry: PathBuf::from("/w/modules/Sound/../shared/trees.jai"),
        directory: PathBuf::from("/w/modules/Sound/../shared"),
      }
    );
    assert_eq!(
      resolve_directory_module(from, "../Vulkan_Render"),
      ResolvedModule {
        kind: ModuleKind::Directory,
        entry: PathBuf::from("/w/modules/Sound/../Vulkan_Render/module.jai"),
        directory: PathBuf::from("/w/modules/Sound/../Vulkan_Render"),
      }
    );
  }
}

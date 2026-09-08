//! Objects, libraries and the link line.
//!
//! orangejuice links through the platform C driver rather than driving
//! `ld.lld` itself the way the reference does (**C§11**): the driver already
//! knows where `crt1.o`, the dynamic linker and the C runtime live, which is
//! the part of that command line that is not ours to reproduce. What is ours —
//! the object list, the `#library` resolution, `-rpath '$ORIGIN'` and
//! `-export-dynamic` — is built here.

use std::path::{Path, PathBuf};
use std::process::Command;

use oj_ir::Library;

/// What the program is linked into (`Build_Options.output_type`, **C§4**).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputType {
  #[default]
  Executable,
  DynamicLibrary,
  ObjectFile,
  NoOutput,
}

#[derive(Clone, Debug, Default)]
pub struct Request {
  pub objects: Vec<PathBuf>,
  pub output: PathBuf,
  pub output_type: OutputType,
  pub libraries: Vec<Library>,
  /// `Build_Options.additional_linker_arguments`, verbatim.
  pub additional_arguments: Vec<String>,
}

/// The link command, as it will be run. Building it is separate from running
/// it so that `OUTPUT_LINK_LINE` and the tests can look at it (**C§11**).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkLine {
  pub program: PathBuf,
  pub arguments: Vec<String>,
}

impl LinkLine {
  pub fn display(&self) -> String {
    let mut line = self.program.display().to_string();
    for argument in &self.arguments {
      line.push(' ');
      line.push_str(argument);
    }
    line
  }
}

/// The C driver that does the linking: `$CC` if it is set, then whatever the
/// `cc` crate finds for this target, then the usual names.
pub fn driver() -> PathBuf {
  if let Some(value) = std::env::var_os("CC").filter(|value| !value.is_empty()) {
    return PathBuf::from(value);
  }
  let found = cc::Build::new()
    .target(oj_codegen_triple())
    .host(oj_codegen_triple())
    .opt_level(0)
    .cargo_metadata(false)
    .cargo_warnings(false)
    .try_get_compiler()
    .ok()
    .map(|compiler| compiler.path().to_path_buf());
  found.unwrap_or_else(|| PathBuf::from("cc"))
}

fn oj_codegen_triple() -> &'static str {
  "x86_64-unknown-linux-gnu"
}

/// Builds the link line for a request without running it.
pub fn link_line(request: &Request) -> LinkLine {
  let mut arguments = Vec::new();
  if request.output_type == OutputType::DynamicLibrary {
    arguments.push(String::from("-shared"));
  }
  arguments.push(String::from("-o"));
  arguments.push(request.output.display().to_string());
  for object in &request.objects {
    arguments.push(object.display().to_string());
  }

  // A non-system `#library` is looked for beside the file that declared it
  // (**L§12.2**).
  let mut directories: Vec<String> = Vec::new();
  for library in &request.libraries {
    let Some(directory) = library.directory.as_ref().filter(|_| !library.system) else {
      continue;
    };
    let directory = directory.display().to_string();
    if !directories.contains(&directory) {
      directories.push(directory);
    }
  }
  for directory in directories {
    arguments.push(format!("-L{directory}"));
  }
  for library in &request.libraries {
    arguments.push(format!("-l{}", link_name(library)));
  }

  // The reference links every executable so that it finds its own libraries
  // and so that its symbols are visible to `dlopen` (**C§11**).
  arguments.push(String::from("-Wl,-rpath,$ORIGIN"));
  arguments.push(String::from("-Wl,--export-dynamic"));
  arguments.push(String::from("-Wl,--build-id"));
  arguments.extend(request.additional_arguments.iter().cloned());

  LinkLine {
    program: driver(),
    arguments,
  }
}

/// What follows `-l`. A `#library,system "libc"` names the *file* the loader
/// looks for, so the `lib` prefix comes off; a plain `#library "raylib"` names
/// the library the way `-l` already spells it (**C§11**, **L§12.2**).
fn link_name(library: &Library) -> &str {
  match library.system {
    true => library.name.strip_prefix("lib").unwrap_or(&library.name),
    false => &library.name,
  }
}

/// Runs the link, or copies the single object out when no linking is asked
/// for.
pub fn link(request: &Request) -> Result<Option<LinkLine>, String> {
  match request.output_type {
    OutputType::NoOutput => return Ok(None),
    OutputType::ObjectFile => {
      let [object] = &request.objects[..] else {
        return Err(String::from(
          "an object-file build must produce exactly one object",
        ));
      };
      std::fs::copy(object, &request.output)
        .map_err(|error| format!("could not write {}: {error}", request.output.display()))?;
      return Ok(None);
    }
    OutputType::Executable | OutputType::DynamicLibrary => {}
  }

  let line = link_line(request);
  let output = Command::new(&line.program)
    .args(&line.arguments)
    .output()
    .map_err(|error| {
      format!(
        "could not run the linker {}: {error}",
        line.program.display()
      )
    })?;
  if !output.status.success() {
    let text = String::from_utf8_lossy(&output.stderr);
    return Err(format!(
      "the linker failed:\n{}\n{}",
      line.display(),
      text.trim_end()
    ));
  }
  Ok(Some(line))
}

/// The name an executable gets when nothing named one: the source file's stem,
/// with no extension (**C§4**).
pub fn default_output_name(source: &Path) -> String {
  source
    .file_stem()
    .map(|stem| stem.to_string_lossy().into_owned())
    .unwrap_or_else(|| String::from("a.out"))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn request() -> Request {
    Request {
      objects: vec![PathBuf::from("first.o")],
      output: PathBuf::from("first"),
      ..Request::default()
    }
  }

  #[test]
  fn an_executable_names_its_objects_and_carries_the_reference_flags() {
    let line = link_line(&request());
    assert!(line.arguments.contains(&String::from("-o")));
    assert!(line.arguments.contains(&String::from("first")));
    assert!(line.arguments.contains(&String::from("first.o")));
    assert!(line.arguments.contains(&String::from("-Wl,-rpath,$ORIGIN")));
    assert!(
      line
        .arguments
        .contains(&String::from("-Wl,--export-dynamic"))
    );
    assert!(!line.arguments.contains(&String::from("-shared")));
  }

  #[test]
  fn a_dynamic_library_is_linked_shared() {
    let line = link_line(&Request {
      output_type: OutputType::DynamicLibrary,
      ..request()
    });
    assert_eq!(line.arguments.first(), Some(&String::from("-shared")));
  }

  #[test]
  fn a_system_library_is_a_plain_l_flag_and_a_local_one_adds_its_directory() {
    let line = link_line(&Request {
      libraries: vec![
        Library {
          name: String::from("libc"),
          system: true,
          directory: Some(PathBuf::from("/ignored")),
        },
        Library {
          name: String::from("raylib"),
          system: false,
          directory: Some(PathBuf::from("/tmp/game")),
        },
      ],
      ..request()
    });
    assert!(line.arguments.contains(&String::from("-lc")));
    assert!(line.arguments.contains(&String::from("-lraylib")));
    assert!(line.arguments.contains(&String::from("-L/tmp/game")));
    assert!(!line.arguments.contains(&String::from("-L/ignored")));
  }

  #[test]
  fn additional_arguments_go_last() {
    let line = link_line(&Request {
      additional_arguments: vec![String::from("-static")],
      ..request()
    });
    assert_eq!(line.arguments.last(), Some(&String::from("-static")));
  }

  #[test]
  fn the_default_output_name_is_the_stem_of_the_source() {
    assert_eq!(
      default_output_name(Path::new("/tmp/how_to/001_first.jai")),
      "001_first"
    );
  }
}

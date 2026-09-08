//! What one compilation's metaprogram has asked the compiler for.

use std::path::PathBuf;

use crate::abi::{Slice, Str};

/// How a metaprogram's own diagnostic is reported (**C§3.3**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportMode {
  Error,
  ErrorContinuable,
  Warning,
  Info,
}

impl ReportMode {
  /// The values `Report` gives its members (**C§3.3**).
  pub fn from_value(value: u8) -> Self {
    match value {
      0 => Self::Error,
      1 => Self::ErrorContinuable,
      2 => Self::Warning,
      _ => Self::Info,
    }
  }

  pub fn is_error(self) -> bool {
    matches!(self, Self::Error | Self::ErrorContinuable)
  }
}

/// One `compiler_report` a metaprogram made.
#[derive(Clone, Debug)]
pub struct Report {
  pub message: String,
  pub filename: String,
  pub line: i64,
  pub character: i64,
  pub mode: ReportMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceStatus {
  Ok,
  Failed,
}

/// Where the `Build_Options` members the driver acts on sit, measured against
/// the distribution's own declaration rather than written down (**C§4**).
#[derive(Clone, Copy, Debug, Default)]
pub struct BuildOptionsLayout {
  pub output_executable_name: Option<u64>,
  pub output_path: Option<u64>,
}

/// A compilation a metaprogram asked for (**C§3.1**). Its build options are
/// kept as the bytes the program sees, since `Build_Options` is the
/// distribution's struct rather than one of ours; what the driver acts on is
/// read back out of them by member offset.
#[derive(Clone, Debug)]
pub struct Workspace {
  pub id: i64,
  pub name: String,
  pub options: Vec<u8>,
  pub files: Vec<PathBuf>,
  pub strings: Vec<String>,
  pub status: WorkspaceStatus,
  /// Whether source has been added, after which the options are frozen
  /// (**C§3.1**).
  pub started: bool,
  pub destroyed: bool,
}

/// The compile-time state of one compilation: the workspaces its metaprogram
/// created and what it told the compiler along the way.
#[derive(Debug, Default)]
pub struct Meta {
  pub base_path: String,
  pub command_line: Vec<String>,
  pub version: String,
  pub version_numbers: (i32, i32, i32),
  pub workspaces: Vec<Workspace>,
  /// The workspace compile-time code belongs to, which `w = -1` means
  /// (**C§3.1**).
  pub current: i64,
  pub reports: Vec<Report>,
  /// The bytes of `Build_Options` as the distribution declares it, already
  /// filled in with the defaults its members carry, and its size.
  pub default_build_options: Vec<u8>,
  /// Where the members of those bytes are.
  pub build_options_layout: BuildOptionsLayout,
  /// Directories `compiler_add_library_search_directory` added (**C§3.3**).
  pub library_directories: Vec<PathBuf>,
  /// Strings and slices handed to compile-time code, which have to outlive the
  /// call that produced them.
  arena: Vec<Box<[u8]>>,
}

impl Meta {
  pub fn new() -> Self {
    Self::default()
  }

  /// Copies `text` into the compilation's arena and describes it the way Jai
  /// does. The bytes live as long as the compilation.
  pub fn intern(&mut self, text: &[u8]) -> Str {
    let boxed: Box<[u8]> = text.to_vec().into_boxed_slice();
    let data = boxed.as_ptr();
    let count = boxed.len() as i64;
    self.arena.push(boxed);
    Str { count, data }
  }

  /// A `[] string` over strings copied into the arena, as
  /// `get_toplevel_command_line` returns (**C§3.3**).
  pub fn intern_strings(&mut self, items: &[String]) -> Slice {
    let strings: Vec<Str> = items
      .iter()
      .map(|item| self.intern(item.as_bytes()))
      .collect();
    let mut bytes = Vec::with_capacity(strings.len() * size_of::<Str>());
    for string in &strings {
      bytes.extend_from_slice(&string.count.to_ne_bytes());
      bytes.extend_from_slice(&(string.data as usize).to_ne_bytes());
    }
    let boxed = bytes.into_boxed_slice();
    let data = boxed.as_ptr();
    self.arena.push(boxed);
    Slice {
      count: items.len() as i64,
      data,
    }
  }

  /// Creates a workspace and returns its handle. Handles start at 1, since 0
  /// is the failure the reference documents (**C§3.1**).
  pub fn create_workspace(&mut self, name: String) -> i64 {
    let id = self.workspaces.len() as i64 + 1;
    let options = self.default_build_options.clone();
    self.workspaces.push(Workspace {
      id,
      name,
      options,
      files: Vec::new(),
      strings: Vec::new(),
      status: WorkspaceStatus::Ok,
      started: false,
      destroyed: false,
    });
    id
  }

  /// The workspace a handle names, with `-1` meaning the current one
  /// (**C§3.1**).
  pub fn workspace(&mut self, handle: i64) -> Option<&mut Workspace> {
    let handle = if handle == -1 { self.current } else { handle };
    self
      .workspaces
      .iter_mut()
      .find(|workspace| workspace.id == handle)
  }

  pub fn report(&mut self, report: Report) {
    self.reports.push(report);
  }

  pub fn has_errors(&self) -> bool {
    self.reports.iter().any(|report| report.mode.is_error())
      || self
        .workspaces
        .iter()
        .any(|workspace| workspace.status == WorkspaceStatus::Failed)
  }

  /// One `string` member of a workspace.s build options, read back out of the
  /// bytes the metaprogram set (**C§4**). A pointer into compile-time memory is
  /// what a Jai string carries, so this is the one place the compiler follows
  /// one back.
  pub fn workspace_string(
    &self,
    workspace: &Workspace,
    member: impl FnOnce(&BuildOptionsLayout) -> Option<u64>,
  ) -> Option<String> {
    let offset = member(&self.build_options_layout)? as usize;
    let bytes = workspace.options.get(offset..offset + size_of::<Str>())?;
    let count = i64::from_ne_bytes(bytes[..8].try_into().ok()?);
    let data = usize::from_ne_bytes(bytes[8..16].try_into().ok()?) as *const u8;
    if count <= 0 || data.is_null() {
      return Some(String::new());
    }
    let text = unsafe { std::slice::from_raw_parts(data, count as usize) };
    Some(String::from_utf8_lossy(text).into_owned())
  }

  /// The workspaces a driver still has to build: the ones source was added to
  /// and that nobody destroyed.
  pub fn buildable(&self) -> impl Iterator<Item = &Workspace> {
    self
      .workspaces
      .iter()
      .filter(|workspace| workspace.started && !workspace.destroyed)
  }
}

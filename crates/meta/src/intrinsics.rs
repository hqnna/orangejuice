//! The `#compiler` procedures themselves.
//!
//! Every one is built to `oj_ir::abi_of`: a return that is not a scalar comes
//! back through storage handed in as the first argument, an argument that is
//! not a scalar arrives by pointer, and a trailing `*#Context` closes the list
//! (**L§7.6**, **L§10.1**). The context is never read — a compiler procedure
//! is the compiler — but it has to be in the signature for the call to line up.

use std::ffi::c_void;
use std::path::PathBuf;

use crate::abi::{Slice, SourceCodeLocation, Str, VersionInfo};
use crate::state::{Report, ReportMode, WorkspaceStatus};
use crate::with;

/// Every `#compiler` symbol this compiler answers, with the address to bind it
/// to. `oj-jit` reads this before it materializes a compile-time module.
pub(crate) fn table() -> Vec<(&'static str, usize)> {
  vec![
    (
      "compiler_get_version_info",
      compiler_get_version_info as *const () as usize,
    ),
    (
      "compiler_get_base_path",
      compiler_get_base_path as *const () as usize,
    ),
    (
      "get_toplevel_command_line",
      get_toplevel_command_line as *const () as usize,
    ),
    ("compiler_report", compiler_report as *const () as usize),
    (
      "compiler_set_workspace_status",
      compiler_set_workspace_status as *const () as usize,
    ),
    (
      "compiler_create_workspace",
      compiler_create_workspace as *const () as usize,
    ),
    (
      "compiler_destroy_workspace",
      compiler_destroy_workspace as *const () as usize,
    ),
    ("get_name", get_name as *const () as usize),
    ("get_build_options", get_build_options as *const () as usize),
    ("set_build_options", set_build_options as *const () as usize),
    ("add_build_file", add_build_file as *const () as usize),
    ("add_build_string", add_build_string as *const () as usize),
    (
      "compiler_add_library_search_directory",
      compiler_add_library_search_directory as *const () as usize,
    ),
  ]
}

/// `compiler_get_version_info :: (version_info_return: *Version_Info) -> string`
unsafe extern "C" fn compiler_get_version_info(
  result: *mut Str,
  out: *mut VersionInfo,
  _context: *mut c_void,
) {
  let version = with(|meta| {
    let numbers = meta.version_numbers;
    let text = meta.version.clone();
    (meta.intern(text.as_bytes()), numbers)
  });
  let (text, numbers) = version.unwrap_or((Str::EMPTY, (0, 0, 0)));
  if !out.is_null() {
    unsafe {
      *out = VersionInfo {
        major: numbers.0,
        minor: numbers.1,
        micro: numbers.2,
      };
    }
  }
  unsafe { write_str(result, text) };
}

/// `compiler_get_base_path :: () -> string`
unsafe extern "C" fn compiler_get_base_path(result: *mut Str, _context: *mut c_void) {
  let path = with(|meta| {
    let base = meta.base_path.clone();
    meta.intern(base.as_bytes())
  });
  unsafe { write_str(result, path.unwrap_or(Str::EMPTY)) };
}

/// `get_toplevel_command_line :: () -> [] string`
unsafe extern "C" fn get_toplevel_command_line(result: *mut Slice, _context: *mut c_void) {
  let line = with(|meta| {
    let items = meta.command_line.clone();
    meta.intern_strings(&items)
  });
  if !result.is_null() {
    unsafe { *result = line.unwrap_or(Slice::EMPTY) };
  }
}

/// `compiler_report :: (message: string, loc := #caller_location, mode := Report.ERROR)`
unsafe extern "C" fn compiler_report(
  message: *const Str,
  location: *const SourceCodeLocation,
  mode: u8,
  _context: *mut c_void,
) {
  let message = unsafe { read_str(message) };
  let (filename, line, character) = unsafe { read_location(location) };
  with(|meta| {
    meta.report(Report {
      message,
      filename,
      line,
      character,
      mode: ReportMode::from_value(mode),
    })
  });
}

/// `compiler_set_workspace_status :: (status: Workspace_Status, w: Workspace = -1)`
unsafe extern "C" fn compiler_set_workspace_status(status: u8, w: i64, _context: *mut c_void) {
  let status = if status == 0 {
    WorkspaceStatus::Ok
  } else {
    WorkspaceStatus::Failed
  };
  with(|meta| {
    if let Some(workspace) = meta.workspace(w) {
      workspace.status = status;
    }
  });
}

/// `compiler_create_workspace :: (name := "") -> Workspace`
unsafe extern "C" fn compiler_create_workspace(name: *const Str, _context: *mut c_void) -> i64 {
  let name = unsafe { read_str(name) };
  with(|meta| meta.create_workspace(name)).unwrap_or(0)
}

/// `compiler_destroy_workspace :: (w: Workspace)`
unsafe extern "C" fn compiler_destroy_workspace(w: i64, _context: *mut c_void) {
  with(|meta| {
    if let Some(workspace) = meta.workspace(w) {
      workspace.destroyed = true;
    }
  });
}

/// `get_name :: (w: Workspace = -1) -> string`
unsafe extern "C" fn get_name(result: *mut Str, w: i64, _context: *mut c_void) {
  let name = with(|meta| {
    let name = meta.workspace(w).map(|workspace| workspace.name.clone());
    meta.intern(name.unwrap_or_default().as_bytes())
  });
  unsafe { write_str(result, name.unwrap_or(Str::EMPTY)) };
}

/// `get_build_options :: (w: Workspace = -1) -> Build_Options`
unsafe extern "C" fn get_build_options(result: *mut u8, w: i64, _context: *mut c_void) {
  let options = with(|meta| match meta.workspace(w) {
    Some(workspace) => workspace.options.clone(),
    None => meta.default_build_options.clone(),
  });
  let Some(options) = options else {
    return;
  };
  if result.is_null() || options.is_empty() {
    return;
  }
  unsafe { std::ptr::copy_nonoverlapping(options.as_ptr(), result, options.len()) };
}

/// `set_build_options :: (options: Build_Options, w: Workspace = -1, loc := #caller_location)`
unsafe extern "C" fn set_build_options(
  options: *const u8,
  w: i64,
  _location: *const SourceCodeLocation,
  _context: *mut c_void,
) {
  if options.is_null() {
    return;
  }
  with(|meta| {
    let size = meta.default_build_options.len();
    if size == 0 {
      return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(options, size) }.to_vec();
    if let Some(workspace) = meta.workspace(w) {
      workspace.options = bytes;
    }
  });
}

/// `add_build_file :: (filename: string, w: Workspace, loc := #caller_location)`
unsafe extern "C" fn add_build_file(
  filename: *const Str,
  w: i64,
  location: *const SourceCodeLocation,
  _context: *mut c_void,
) {
  let filename = unsafe { read_str(filename) };
  // The path is relative to the file the call was written in (**C§3.1**).
  let (from, _, _) = unsafe { read_location(location) };
  with(|meta| {
    let path = resolve(&from, &filename);
    if let Some(workspace) = meta.workspace(w) {
      workspace.files.push(path);
      workspace.started = true;
    }
  });
}

/// `add_build_string :: (data: string, w: Workspace, code := #code,null, loc := #caller_location)`
unsafe extern "C" fn add_build_string(
  data: *const Str,
  w: i64,
  _code: u64,
  _location: *const SourceCodeLocation,
  _context: *mut c_void,
) {
  let text = unsafe { read_str(data) };
  with(|meta| {
    if let Some(workspace) = meta.workspace(w) {
      workspace.strings.push(text);
      workspace.started = true;
    }
  });
}

/// `compiler_add_library_search_directory :: (path: string)`
unsafe extern "C" fn compiler_add_library_search_directory(
  path: *const Str,
  _context: *mut c_void,
) {
  let path = unsafe { read_str(path) };
  with(|meta| meta.library_directories.push(PathBuf::from(path)));
}

unsafe fn write_str(result: *mut Str, value: Str) {
  if !result.is_null() {
    unsafe { *result = value };
  }
}

unsafe fn read_str(value: *const Str) -> String {
  if value.is_null() {
    return String::new();
  }
  unsafe { (*value).string_lossy() }
}

unsafe fn read_location(location: *const SourceCodeLocation) -> (String, i64, i64) {
  if location.is_null() {
    return (String::new(), 0, 0);
  }
  let location = unsafe { &*location };
  (
    unsafe { location.fully_pathed_filename.string_lossy() },
    location.line_number,
    location.character_number,
  )
}

/// A path a metaprogram gave, made absolute against the file the call was
/// written in (**C§3.1**).
fn resolve(from: &str, name: &str) -> PathBuf {
  let path = PathBuf::from(name);
  if path.is_absolute() || from.is_empty() {
    return path;
  }
  match PathBuf::from(from).parent() {
    Some(directory) => directory.join(path),
    None => path,
  }
}

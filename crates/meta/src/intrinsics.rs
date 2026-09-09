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
use crate::code::CodeNode;
use crate::message::Message;
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
      "add_build_string_scoped_by_message",
      add_build_string_scoped_by_message as *const () as usize,
    ),
    (
      "compiler_add_library_search_directory",
      compiler_add_library_search_directory as *const () as usize,
    ),
    (
      "compiler_begin_intercept",
      compiler_begin_intercept as *const () as usize,
    ),
    (
      "compiler_end_intercept",
      compiler_end_intercept as *const () as usize,
    ),
    (
      "compiler_wait_for_message",
      compiler_wait_for_message as *const () as usize,
    ),
    (
      "get_current_workspace",
      get_current_workspace as *const () as usize,
    ),
    ("remap_import", remap_import as *const () as usize),
    ("provide_import", provide_import as *const () as usize),
    ("add_global_data", add_global_data as *const () as usize),
    ("add_data_segment", add_data_segment as *const () as usize),
    (
      "set_build_options_dc",
      set_build_options_dc as *const () as usize,
    ),
    (
      "compile_time_debug_break",
      compile_time_debug_break as *const () as usize,
    ),
    (
      "compiler_set_memory_breakpoint",
      compiler_set_memory_breakpoint as *const () as usize,
    ),
    ("developer_debug", developer_debug as *const () as usize),
    ("get_type", get_type as *const () as usize),
    (
      "compiler_get_struct_location",
      compiler_get_struct_location as *const () as usize,
    ),
    (
      "compiler_set_type_info_flags",
      compiler_set_type_info_flags as *const () as usize,
    ),
    (
      "compiler_report_errors_for_unresolved_identifiers",
      compiler_report_errors_for_unresolved_identifiers as *const () as usize,
    ),
    (
      "compiler_report_errors_for_untyped_declarations_with_these_notes",
      compiler_report_errors_for_untyped_declarations_with_these_notes as *const () as usize,
    ),
    (
      "compiler_get_nodes",
      compiler_get_nodes as *const () as usize,
    ),
    ("compiler_get_code", compiler_get_code as *const () as usize),
    ("get_root_type", get_root_type as *const () as usize),
    (
      "compiler_make_procedure_live",
      compiler_make_procedure_live as *const () as usize,
    ),
    (
      "compiler_custom_link_command_is_complete",
      compiler_custom_link_command_is_complete as *const () as usize,
    ),
    ("write_string", write_string as *const () as usize),
    ("write_strings", write_strings as *const () as usize),
  ]
}

/// `set_build_options_dc :: (options: Build_Options_During_Compile, w: Workspace = -1)`
unsafe extern "C" fn set_build_options_dc(options: *const u8, w: i64, _context: *mut c_void) {
  if options.is_null() {
    return;
  }
  with(|meta| {
    let size = meta.during_compile_layout.size;
    if size == 0 {
      return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(options, size) }.to_vec();
    if let Some(workspace) = meta.workspace(w) {
      workspace.during_compile = Some(bytes);
    }
  });
}
/// `compiler_get_nodes :: (code: Code) -> (root: *Code_Node, expressions: [] *Code_Node)`
///
/// A `Code` is the address of the `Code_Node` the program was written at, so
/// the root is the argument itself; what the compiler adds is the flattened
/// list of everything under it (**C§3.3**, **C§5.3**). A pointer this
/// compilation did not export answers with nothing rather than reading memory
/// that is not ours.
unsafe extern "C" fn compiler_get_nodes(
  code: *const CodeNode,
  expressions: *mut Slice,
  _context: *mut c_void,
) -> *const CodeNode {
  let tree = with(|meta| meta.nodes.borrow().tree(code)).flatten();
  if !expressions.is_null() {
    unsafe { *expressions = tree.map(|tree| tree.expressions).unwrap_or(Slice::EMPTY) };
  }
  match tree {
    Some(tree) => tree.root,
    None => std::ptr::null(),
  }
}

/// `compiler_get_code :: (node: *Code_Node, code_to_copy_scope_from: Code = #code,null) -> Code`
///
/// A `Code` and a `*Code_Node` are the same address here, so making one out of
/// the other is the identity — what the reference copies along with it is the
/// scope the names resolve in, which orangejuice keeps on the node itself
/// (`docs/spec.md` §10).
unsafe extern "C" fn compiler_get_code(
  node: *const CodeNode,
  _scope_source: *const CodeNode,
  _context: *mut c_void,
) -> *const CodeNode {
  node
}

/// `get_root_type :: (code: Code) -> (status: Get_Root_Type_Status, type: Type)`
///
/// A node exported by orangejuice carries no `Type_Info` address, because the
/// table it would point into belongs to the workspace being compiled rather
/// than to the metaprogram reading it (`docs/spec.md` §10), so a well-formed
/// `Code` reports `NOT_TYPED` rather than a pointer into the wrong table.
unsafe extern "C" fn get_root_type(
  code: *const CodeNode,
  result: *mut *const c_void,
  _context: *mut c_void,
) -> u8 {
  if !result.is_null() {
    unsafe { *result = std::ptr::null() };
  }
  if code.is_null() {
    return 2;
  }
  match with(|meta| meta.nodes.borrow().tree(code)).flatten() {
    Some(_) => 4,
    None => 3,
  }
}

/// `compiler_make_procedure_live :: (w: Workspace, header: *Code_Procedure_Header)`
///
/// IR generation starts at the entry point and follows calls (**L§11.6**), so
/// a procedure nothing reachable calls is never lowered; this is how a
/// metaprogram says to lower one anyway. The header says which: its own name,
/// and the file its `enclosing_load` names. As with `provide_import`, the
/// compilation is already over, so the workspace is compiled again with the
/// procedure among its roots (`docs/spec.md` §10).
unsafe extern "C" fn compiler_make_procedure_live(
  w: i64,
  header: *const crate::code::CodeProcedureHeader,
  _context: *mut c_void,
) {
  if header.is_null() {
    return;
  }
  let header = unsafe { &*header };
  let name = unsafe { header.name.string_lossy() };
  if name.is_empty() {
    return;
  }
  let file = match header.base.location.enclosing_load.is_null() {
    true => String::new(),
    false => unsafe {
      (*header.base.location.enclosing_load)
        .fully_pathed_filename
        .string_lossy()
    },
  };
  with(|meta| meta.make_procedure_live(w, file, name));
}

/// `compiler_custom_link_command_is_complete :: (w: Workspace)`
///
/// The reference is waiting for this before it finishes the workspace; here
/// the compilation is already over by the time the metaprogram sees the phase
/// (`docs/spec.md` §10), so what this records is that the metaprogram did its
/// own link.
unsafe extern "C" fn compiler_custom_link_command_is_complete(w: i64, _context: *mut c_void) {
  with(|meta| {
    if let Some(workspace) = meta.workspace(w) {
      workspace.link_command_complete = true;
    }
  });
}

/// `write_string :: (s: string, to_standard_error := false) #no_context #compiler`
/// (Runtime_Support). At compile time the compiler is the one that writes, so
/// that what a metaprogram prints is interleaved with the compiler's own
/// output (**L§16**).
unsafe extern "C" fn write_string(text: *const Str, to_standard_error: bool) {
  let text = unsafe { read_str(text) };
  write_out(&text, to_standard_error);
}

/// `write_strings :: (strings: ..string, to_standard_error := false)`
unsafe extern "C" fn write_strings(strings: *const Slice, to_standard_error: bool) {
  if strings.is_null() {
    return;
  }
  let strings = unsafe { &*strings };
  if strings.data.is_null() || strings.count <= 0 {
    return;
  }
  let items =
    unsafe { std::slice::from_raw_parts(strings.data as *const Str, strings.count as usize) };
  for item in items {
    let text = unsafe { item.string_lossy() };
    write_out(&text, to_standard_error);
  }
}

fn write_out(text: &str, to_standard_error: bool) {
  use std::io::Write as _;
  if to_standard_error {
    let mut stream = std::io::stderr();
    let _ = stream.write_all(text.as_bytes());
    let _ = stream.flush();
  } else {
    let mut stream = std::io::stdout();
    let _ = stream.write_all(text.as_bytes());
    let _ = stream.flush();
  }
}

/// `compile_time_debug_break :: () #compiler #no_context` (Runtime_Support).
/// The reference stops in its bytecode debugger here; orangejuice runs compile
/// time code natively, so there is nothing to stop, and a `debug_break` at
/// compile time goes by (`docs/spec.md` §6.5).
unsafe extern "C" fn compile_time_debug_break() {}

/// `compiler_set_memory_breakpoint :: (pointer: *void)` and
/// `developer_debug :: (x: *void)`, both hooks for stopping a bytecode
/// interpreter that orangejuice does not have (`docs/spec.md` §6.5).
unsafe extern "C" fn compiler_set_memory_breakpoint(_pointer: *mut c_void, _context: *mut c_void) {}

unsafe extern "C" fn developer_debug(_value: *mut c_void, _context: *mut c_void) {}

/// `get_type :: (ti: *Type_Info) -> Type`
///
/// A `Type` *is* its `Type_Info`'s address (`docs/spec.md` §10), which is what
/// makes two of them compare equal exactly when the types are the same, so
/// this is the identity — as it is in the reference, where the comment says it
/// is only valid on a `*Type_Info` the compiler made for this workspace.
unsafe extern "C" fn get_type(info: *const c_void, _context: *mut c_void) -> *const c_void {
  info
}

/// `compiler_get_struct_location :: (w: Workspace, info: *Type_Info_Struct) -> Source_Code_Location`
///
/// The pointer is an address in the type table image the running code reads,
/// and whoever started that code said where each struct in it was written
/// (**C§3.3**). A pointer that is not one of those answers with an empty
/// location, the way the reference does for a struct it has no source for.
unsafe extern "C" fn compiler_get_struct_location(
  result: *mut SourceCodeLocation,
  _w: i64,
  info: *const c_void,
  _context: *mut c_void,
) {
  let found = with(|meta| {
    meta
      .struct_location(info as usize)
      .map(|(path, line, character)| (meta.intern(path.as_bytes()), line, character))
  })
  .flatten();
  if result.is_null() {
    return;
  }
  let location = match found {
    Some((filename, line_number, character_number)) => SourceCodeLocation {
      fully_pathed_filename: filename,
      line_number,
      character_number,
    },
    None => SourceCodeLocation {
      fully_pathed_filename: Str::EMPTY,
      line_number: 0,
      character_number: 0,
    },
  };
  unsafe { *result = location };
}

/// `compiler_set_type_info_flags :: (type: Type, flags: Type_Info_Flags)`
///
/// The flags are or-ed into whatever the struct was declared with, and the
/// type table leaves out what they say to leave out (**L§8.7**, **L§17**).
unsafe extern "C" fn compiler_set_type_info_flags(
  type_info: *const c_void,
  flags: u32,
  _context: *mut c_void,
) {
  with(|meta| {
    // A `Type` at compile time is the address of its record in the image the
    // running code reads, so the type it names is looked up there
    // (**L§3.10**).
    if let Some(type_id) = meta.type_at(type_info as usize) {
      meta.type_info_flags.push((type_id, flags));
    }
  });
}

/// `compiler_report_errors_for_unresolved_identifiers :: (filename: string, w: Workspace = -1)`
///
/// The reference reports here what it was still waiting on; orangejuice
/// reports an identifier nothing declares as part of the compilation it
/// belongs to (**L§4.3**), which has already happened by the time a
/// metaprogram reads a message (`docs/spec.md` §10). Nothing is left to say.
unsafe extern "C" fn compiler_report_errors_for_unresolved_identifiers(
  _filename: *const Str,
  _w: i64,
  _context: *mut c_void,
) {
}

/// `compiler_report_errors_for_untyped_declarations_with_these_notes :: (w: Workspace, labels: ..string)`
///
/// As above: a declaration orangejuice could not type has already been
/// reported by the compilation that could not type it.
unsafe extern "C" fn compiler_report_errors_for_untyped_declarations_with_these_notes(
  _w: i64,
  _labels: *const Slice,
  _context: *mut c_void,
) {
}

/// `get_current_workspace :: () -> Workspace` (Preload)
unsafe extern "C" fn get_current_workspace(_context: *mut c_void) -> i64 {
  with(|meta| meta.current).unwrap_or(0)
}

/// `remap_import :: (w: Workspace, host_module_name: string, import_name: string, replacement_name: string)`
unsafe extern "C" fn remap_import(
  w: i64,
  host: *const Str,
  import: *const Str,
  replacement: *const Str,
  _context: *mut c_void,
) {
  let host = unsafe { read_str(host) };
  let import = unsafe { read_str(import) };
  let replacement = unsafe { read_str(replacement) };
  with(|meta| {
    if let Some(workspace) = meta.workspace(w) {
      workspace.remaps.push((host, import, replacement));
    }
  });
}
/// `provide_import :: (w: Workspace, message: *Message_Failed_Import, type: Provided_Import_Type, value: string)`
///
/// The reference is holding the import open, waiting for this; orangejuice has
/// already compiled the workspace by the time the metaprogram reads the
/// message, so the answer makes it compile again with the import in place and
/// the message stream starts over (`docs/spec.md` §10).
unsafe extern "C" fn provide_import(
  w: i64,
  message: *const Message,
  kind: u8,
  value: *const Str,
  _context: *mut c_void,
) {
  let value = unsafe { read_str(value) };
  with(|meta| meta.provide_import(w, message as usize, kind, value));
}

/// `add_global_data :: (data: [] u8, segment: Data_Segment_Index, user_segment: *Data_Segment = null, w: Workspace = -1) -> [] u8`
///
/// The bytes are copied into storage the compilation owns and handed back as a
/// slice of it. That slice is what a `#run` returns, and what a pointer among
/// a run's bytes names is laid down beside them in the executable
/// (**L§12.1**), which is how `image.data` reaches the program.
unsafe extern "C" fn add_global_data(
  result: *mut Slice,
  data: *const Slice,
  segment: u16,
  _user_segment: *mut c_void,
  w: i64,
  _context: *mut c_void,
) {
  let bytes = match data.is_null() {
    true => Vec::new(),
    false => {
      let data = unsafe { &*data };
      match data.data.is_null() || data.count <= 0 {
        true => Vec::new(),
        false => unsafe { std::slice::from_raw_parts(data.data, data.count as usize) }.to_vec(),
      }
    }
  };
  let slice = with(|meta| meta.add_global_data(w, bytes, segment)).unwrap_or(Slice::EMPTY);
  if !result.is_null() {
    unsafe { *result = slice };
  }
}

/// `add_data_segment :: (section_name: string, characteristics, alignment: s32 = 16, w: Workspace = -1) -> (segment: *Data_Segment, actual_segment_will_be_created: bool)`
///
/// A segment of one's own is recorded and handed back, but orangejuice puts
/// what goes in it with the rest of the program's data rather than in a
/// section of its own — which is the answer the second return value is for
/// (`docs/spec.md` §10).
unsafe extern "C" fn add_data_segment(
  name: *const Str,
  characteristics: u32,
  alignment: i32,
  w: i64,
  created: *mut bool,
  _context: *mut c_void,
) -> *mut c_void {
  let name = unsafe { read_str(name) };
  if !created.is_null() {
    unsafe { *created = false };
  }
  with(|meta| meta.add_data_segment(w, name, characteristics, alignment))
    .unwrap_or(std::ptr::null_mut())
}

/// `compiler_begin_intercept :: (w: Workspace, flags: Intercept_Flags = 0)`
unsafe extern "C" fn compiler_begin_intercept(w: i64, flags: u32, _context: *mut c_void) {
  with(|meta| meta.begin_intercept(w, flags));
}

/// `compiler_end_intercept :: (w: Workspace)`
unsafe extern "C" fn compiler_end_intercept(_w: i64, _context: *mut c_void) {
  with(|meta| meta.end_intercept());
}

/// `compiler_wait_for_message :: () -> *Message`
///
/// The reference compiles a workspace on another thread while the metaprogram
/// watches; orangejuice compiles it here, the first time the metaprogram asks
/// for a message, and then hands the stream over one message at a time
/// (`docs/spec.md` §10). The compiling re-enters the compiler, so it happens
/// with nothing of this state borrowed.
unsafe extern "C" fn compiler_wait_for_message(_context: *mut c_void) -> *const Message {
  let pending = with(|meta| {
    meta
      .workspace_awaiting_compilation()
      .map(|workspace| (workspace, meta.compiler.clone()))
  })
  .flatten();
  if let Some((workspace, Some(compile))) = pending {
    let compiled = compile(&workspace);
    with(|meta| meta.queue_messages(&compiled));
  }
  with(|meta| meta.next_message())
    .flatten()
    .unwrap_or(std::ptr::null())
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
  // A workspace whose options were never set — the compilation the program
  // itself is, which exists before the compiler has folded the defaults — has
  // the defaults, not nothing: the caller's storage is its own otherwise.
  let options = with(|meta| match meta.workspace(w) {
    Some(workspace) if !workspace.options.is_empty() => workspace.options.clone(),
    _ => meta.default_build_options.clone(),
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

/// `add_build_string :: (data: string, w: Workspace, message: *Message, loc := #caller_location)`,
/// whose own symbol is `add_build_string_scoped_by_message` (**C§3.3**).
///
/// The message says which scope the string joins: a `Message_File` its file's,
/// a `Message_Import` that import's module, and a null message the main
/// program's. The compilation it belongs to is already over by the time a
/// metaprogram reads a message, so — as with `provide_import` — the workspace
/// is compiled again with the string in place (`docs/spec.md` §10).
unsafe extern "C" fn add_build_string_scoped_by_message(
  data: *const Str,
  w: i64,
  message: *const Message,
  _location: *const SourceCodeLocation,
  _context: *mut c_void,
) {
  let text = unsafe { read_str(data) };
  with(|meta| meta.add_scoped_string(w, message as usize, text));
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

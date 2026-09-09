//! What one compilation's metaprogram has asked the compiler for.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use crate::abi::{Slice, Str};
use crate::message::{
  ErrorCode, ImportStatus, Kind, Message, MessageComplete, MessageFailedImport, MessageFile,
  MessageImport, MessagePhase, ModuleType, Phase, Stored,
};

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
  pub output_type: Option<u64>,
  pub append_executable_filename_extension: Option<u64>,
  /// `runtime_support_definitions`, which decides what a compilation takes
  /// from Runtime_Support (**C§4**).
  pub runtime_support_definitions: Option<u64>,
  /// `use_custom_link_command`, which hands the link to the metaprogram
  /// (**C§3.2**).
  pub use_custom_link_command: Option<u64>,
  /// Where the arguments after a lone `-` are handed to a metaprogram
  /// (**C§2.1**).
  pub compile_time_command_line: Option<u64>,
}

/// Where the `Build_Options_During_Compile` members the driver acts on sit
/// (**C§3.1**).
#[derive(Clone, Copy, Debug, Default)]
pub struct DuringCompileLayout {
  pub size: usize,
  pub do_output: Option<u64>,
  pub append_executable_filename_extension: Option<u64>,
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
  /// The compilation's own workspace, which nobody created and nobody builds
  /// again (**C§3.1**).
  pub implicit: bool,
  /// `remap_import` calls: host module, import name, replacement (**C§3.3**).
  pub remaps: Vec<(String, String, String)>,
  /// The bytes of the last `set_build_options_dc`, which may be set while the
  /// workspace is already compiling (**C§3.1**).
  pub during_compile: Option<Vec<u8>>,
  /// Whether a metaprogram watched this workspace compile, in which case the
  /// driver has nothing left to do for it.
  pub intercepted: bool,
  /// Whether the metaprogram ran its own link command and said so
  /// (**C§3.3**).
  pub link_command_complete: bool,
  /// `provide_import` answers a metaprogram gave after it was told an import
  /// failed (**C§3.3**). The workspace is compiled again with them in place.
  pub provided_imports: Vec<ProvidedImport>,
}

/// One `add_global_data` (**C§3.3**): the bytes, where they sit in the
/// compiler's own memory, and which segment they were asked for.
#[derive(Clone, Debug)]
pub struct GlobalData {
  pub address: usize,
  pub bytes: Vec<u8>,
  /// `Data_Segment_Index` as the distribution numbers it.
  pub segment: u16,
  pub workspace: i64,
}

/// One `add_data_segment` (**C§3.3**). `Data_Segment` is opaque to the
/// metaprogram — it only ever holds the pointer — so what it holds is ours.
#[derive(Clone, Debug)]
pub struct DataSegment {
  pub name: String,
  pub characteristics: u32,
  pub alignment: i32,
  pub workspace: i64,
}

/// One `provide_import` (**C§3.3**): the import it answers, and what to import
/// instead. `kind` is `Provided_Import_Type` as the distribution numbers it.
#[derive(Clone, Debug)]
pub struct ProvidedImport {
  pub host: String,
  pub import: String,
  pub kind: u8,
  pub value: String,
}

impl Workspace {
  /// One `string` member of the build options a metaprogram set, read back out
  /// of the bytes it wrote (**C§4**). A pointer into compile-time memory is
  /// what a Jai string carries, so this is the one place the compiler follows
  /// one back.
  /// One byte-sized member of the build options — an `enum u8` or a `bool`.
  pub fn option_u8(
    &self,
    layout: &BuildOptionsLayout,
    member: impl FnOnce(&BuildOptionsLayout) -> Option<u64>,
  ) -> Option<u8> {
    let offset = member(layout)? as usize;
    self.options.get(offset).copied()
  }

  /// One `string` member of a `Build_Options_During_Compile` a metaprogram
  /// set, or `None` when it set none at all.
  pub fn during_compile_string(
    &self,
    layout: &DuringCompileLayout,
    member: impl FnOnce(&DuringCompileLayout) -> Option<u64>,
  ) -> Option<String> {
    let bytes = self.during_compile.as_ref()?;
    let offset = member(layout)? as usize;
    read_string(bytes.get(offset..offset + size_of::<Str>())?)
  }

  /// One `bool` member of a `Build_Options_During_Compile`.
  pub fn during_compile_bool(
    &self,
    layout: &DuringCompileLayout,
    member: impl FnOnce(&DuringCompileLayout) -> Option<u64>,
  ) -> Option<bool> {
    let bytes = self.during_compile.as_ref()?;
    let offset = member(layout)? as usize;
    bytes.get(offset).map(|value| *value != 0)
  }
  pub fn option_string(
    &self,
    layout: &BuildOptionsLayout,
    member: impl FnOnce(&BuildOptionsLayout) -> Option<u64>,
  ) -> Option<String> {
    let offset = member(layout)? as usize;
    read_string(self.options.get(offset..offset + size_of::<Str>())?)
  }
}

/// A Jai `string` sitting in compile-time memory, followed back to its bytes.
fn read_string(bytes: &[u8]) -> Option<String> {
  let count = i64::from_ne_bytes(bytes[..8].try_into().ok()?);
  let data = usize::from_ne_bytes(bytes[8..16].try_into().ok()?) as *const u8;
  if count <= 0 || data.is_null() {
    return Some(String::new());
  }
  let text = unsafe { std::slice::from_raw_parts(data, count as usize) };
  Some(String::from_utf8_lossy(text).into_owned())
}
/// What compiling one workspace produced, as the message stream needs it
/// (**C§3.2**). The driver fills this in; `oj-meta` turns it into messages.
#[derive(Clone, Debug, Default)]
pub struct Compiled {
  /// Every module instantiation, in the order it was made.
  pub modules: Vec<CompiledModule>,
  /// Every file, with the index of the module it belongs to.
  pub files: Vec<CompiledFile>,
  pub executable: Option<PathBuf>,
  pub object_files: Vec<String>,
  pub system_libraries: Vec<String>,
  pub user_libraries: Vec<String>,
  /// The `TYPECHECKED` batches the compilation produced (**C§3.2**), already
  /// exported into the nodes a watching metaprogram reads.
  pub typechecked: Vec<TypecheckedBatch>,
  /// The imports that did not happen, which a metaprogram may answer with
  /// `provide_import` (**C§3.2**).
  pub failed_imports: Vec<CompiledFailedImport>,
  /// The metaprogram is the one that links, so the compiler stopped at the
  /// objects and says so with a `READY_FOR_CUSTOM_LINK_COMMAND` phase rather
  /// than the two write-executable ones (**C§3.2**).
  pub custom_link_command: bool,
  pub errors: usize,
  pub failed: bool,
}

/// One `Message_Typechecked`'s worth of exported trees (**C§3.2**). Only
/// toplevel declarations are sent; headers, bodies and structs are sent
/// wherever they were written.
#[derive(Clone, Debug, Default)]
pub struct TypecheckedBatch {
  pub declarations: Vec<crate::code::Typechecked>,
  pub procedure_headers: Vec<crate::code::Typechecked>,
  pub procedure_bodies: Vec<crate::code::Typechecked>,
  pub structs: Vec<crate::code::Typechecked>,
  pub others: Vec<crate::code::Typechecked>,
}

/// One import a compilation could not make (**C§3.2**), as the message stream
/// needs it. `import_code` is where the `#import` was exported to, when the
/// compilation exported it.
#[derive(Clone, Debug)]
pub struct CompiledFailedImport {
  pub status: ImportStatus,
  pub host: String,
  pub target: String,
  pub import_code: usize,
}

#[derive(Clone, Debug)]
pub struct CompiledModule {
  pub name: String,
  pub entry: PathBuf,
  pub module_type: ModuleType,
}

#[derive(Clone, Debug)]
pub struct CompiledFile {
  pub path: PathBuf,
  pub module: Option<usize>,
  pub from_a_string: bool,
}

/// What one metaprogram is watching (**C§3.1**).
#[derive(Debug, Default)]
pub struct Intercept {
  pub workspace: i64,
  pub flags: u32,
  /// Whether the workspace has been compiled and its messages queued.
  pub compiled: bool,
  /// Whether `COMPLETE` has been handed over, after which waiting is an error.
  pub finished: bool,
  /// A `provide_import` arrived, so the workspace is compiled again with the
  /// answer in place and the stream starts over (**C§3.2**).
  pub recompile: bool,
  pub queue: VecDeque<usize>,
}

/// How the compiler compiles a workspace on the metaprogram's behalf. The
/// driver installs this, since `oj-meta` is below it.
pub type Compiler = Rc<dyn Fn(&Workspace) -> Compiled>;
/// The compile-time state of one compilation: the workspaces its metaprogram
/// created and what it told the compiler along the way.
#[derive(Default)]
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
  /// Where the members of a `Build_Options_During_Compile` are.
  pub during_compile_layout: DuringCompileLayout,
  /// The workspace the metaprogram is watching, if it began intercepting.
  pub intercept: Option<Intercept>,
  /// Every message produced so far. A metaprogram keeps the pointers it was
  /// handed, so nothing here is ever dropped before the compilation ends.
  pub messages: Vec<Stored>,
  /// The failed import each `Message_Failed_Import` stands for, by the address
  /// the metaprogram was handed, so that `provide_import` knows which import
  /// it is answering (**C§3.3**).
  failed_imports: std::collections::HashMap<usize, CompiledFailedImport>,
  /// How a workspace gets compiled, installed by the driver.
  pub compiler: Option<Compiler>,
  /// Directories `compiler_add_library_search_directory` added (**C§3.3**).
  pub library_directories: Vec<PathBuf>,
  /// `compiler_set_type_info_flags` calls, by the type they name and the flags
  /// they set (**C§3.3**). Flags only ever accumulate: the reference says they
  /// are or-ed with whatever the struct was declared with.
  pub type_info_flags: Vec<(u32, u32)>,
  /// The blobs `add_global_data` put into the compilation, by where they live
  /// in the compiler's own memory (**C§3.3**). A pointer among a `#run`'s
  /// bytes that lands in one of these names data the executable has to carry.
  global_data: Vec<GlobalData>,
  /// The segments `add_data_segment` created, kept so that the pointer handed
  /// back stays valid for the whole compilation (**C§3.3**).
  segments: Vec<std::pin::Pin<Box<DataSegment>>>,
  /// Where each struct in the type table image the compile-time code is
  /// running against was written (**C§3.3**), by the address of its
  /// `Type_Info_Struct`. Whoever runs the code installs this, since only that
  /// side knows where the image was placed.
  struct_locations: std::collections::HashMap<usize, (String, i64, i64)>,
  /// Which type each record of that image belongs to, by its address: a
  /// `Type` at compile time is that address (**L§3.10**).
  types_at: std::collections::HashMap<usize, u32>,
  /// The trees a metaprogram has been handed, and the storage they live in
  /// (**C§5.3**). A metaprogram keeps every pointer it was given, so this
  /// lasts as long as the compilation does.
  pub nodes: Rc<RefCell<crate::code::Nodes>>,
  /// Strings and slices handed to compile-time code, which have to outlive the
  /// call that produced them.
  arena: Vec<Box<[u8]>>,
}

impl Meta {
  /// A compilation, with the workspace the program itself is (**C§3.1**).
  /// `get_current_workspace` names it, and `w = -1` means it.
  pub fn new() -> Self {
    let mut meta = Self::default();
    let id = meta.create_workspace(String::new());
    meta.current = id;
    if let Some(workspace) = meta.workspace(id) {
      workspace.implicit = true;
    }
    meta
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

  /// Writes the arguments after a lone `-` into the `Build_Options` every
  /// workspace starts from, which is where a metaprogram reads them
  /// (**C§2.1**). The slice points into the compilation's arena, so it lives
  /// as long as the compile-time code that reads it.
  pub fn seed_command_line(&mut self, at: u64) {
    let size = size_of::<Slice>();
    let at = at as usize;
    if self.default_build_options.len() < at + size {
      return;
    }
    let items = self.command_line.clone();
    let slice = self.intern_strings(&items);
    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(&slice.count.to_ne_bytes());
    bytes.extend_from_slice(&(slice.data as usize).to_ne_bytes());
    self.default_build_options[at..at + size].copy_from_slice(&bytes);
    for workspace in &mut self.workspaces {
      if workspace.options.len() >= at + size {
        workspace.options[at..at + size].copy_from_slice(&bytes);
      }
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
      implicit: false,
      remaps: Vec::new(),
      during_compile: None,
      intercepted: false,
      link_command_complete: false,
      provided_imports: Vec::new(),
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

  /// Starts watching a workspace (**C§3.1**). Only one at a time, which is
  /// what the reference's single message stream amounts to.
  pub fn begin_intercept(&mut self, workspace: i64, flags: u32) {
    let workspace = if workspace == -1 {
      self.current
    } else {
      workspace
    };
    self.intercept = Some(Intercept {
      workspace,
      flags,
      ..Intercept::default()
    });
  }

  pub fn end_intercept(&mut self) {
    if let Some(intercept) = self.intercept.take()
      && let Some(workspace) = self.workspace(intercept.workspace)
    {
      workspace.intercepted = true;
    }
  }

  /// Copies bytes a metaprogram wants in the target into storage this
  /// compilation owns, and describes them the way a `[] u8` does (**C§3.3**).
  /// The address stays valid for the whole compilation, which is what lets a
  /// pointer to it be recognised later.
  pub fn add_global_data(&mut self, workspace: i64, bytes: Vec<u8>, segment: u16) -> Slice {
    let interned = self.intern(&bytes);
    let slice = Slice {
      count: interned.count,
      data: interned.data,
    };
    let workspace = match workspace {
      -1 => self.current,
      other => other,
    };
    self.global_data.push(GlobalData {
      address: slice.data as usize,
      bytes,
      segment,
      workspace,
    });
    slice
  }

  /// Records a segment of a metaprogram's own and hands back the pointer it
  /// will name it by (**C§3.3**).
  pub fn add_data_segment(
    &mut self,
    workspace: i64,
    name: String,
    characteristics: u32,
    alignment: i32,
  ) -> *mut std::ffi::c_void {
    let workspace = match workspace {
      -1 => self.current,
      other => other,
    };
    let mut segment = Box::pin(DataSegment {
      name,
      characteristics,
      alignment,
      workspace,
    });
    // SAFETY: the address is handed to a metaprogram as an opaque
    // `*Data_Segment` and never read back through this reference.
    let address = unsafe { segment.as_mut().get_unchecked_mut() as *mut DataSegment }.cast();
    self.segments.push(segment);
    address
  }

  /// Says where the structs of the image the code about to run reads are, so
  /// that `compiler_get_struct_location` can answer for a `*Type_Info_Struct`
  /// (**C§3.3**). Each run reads its own image, so each run installs its own.
  pub fn set_struct_locations(
    &mut self,
    locations: std::collections::HashMap<usize, (String, i64, i64)>,
  ) {
    self.struct_locations = locations;
  }

  /// Where the struct a `*Type_Info_Struct` belongs to was written.
  pub fn struct_location(&self, address: usize) -> Option<(String, i64, i64)> {
    self.struct_locations.get(&address).cloned()
  }

  /// Says which type each record of the image the code about to run reads
  /// belongs to. A `Type` at compile time is that record's address, so this is
  /// what turns one back into the type the compiler knows (**L§3.10**).
  pub fn set_types_at(&mut self, types: std::collections::HashMap<usize, u32>) {
    self.types_at = types;
  }

  /// The type a compile-time `Type` value names.
  pub fn type_at(&self, address: usize) -> Option<u32> {
    self.types_at.get(&address).copied()
  }

  /// The blobs `add_global_data` put into this compilation: where each one
  /// lives in the compiler's memory, and how long it is. A pointer among a
  /// `#run`'s bytes that lands inside one names data the executable carries.
  pub fn global_data(&self) -> &[GlobalData] {
    &self.global_data
  }

  /// The blob a compile-time address points into, and how far in.
  pub fn global_data_at(&self, address: usize) -> Option<(&GlobalData, u64)> {
    self.global_data.iter().find_map(|data| {
      let end = data.address.checked_add(data.bytes.len())?;
      (address >= data.address && address < end).then(|| (data, (address - data.address) as u64))
    })
  }

  /// Records what a metaprogram answered a failed import with, and asks for
  /// the workspace to be compiled again with the answer in place (**C§3.2**).
  /// A message that names no failed import of ours is ignored, the way the
  /// reference ignores one it did not send.
  pub fn provide_import(&mut self, workspace: i64, message: usize, kind: u8, value: String) {
    let Some(failed) = self.failed_imports.get(&message).cloned() else {
      return;
    };
    let provided = ProvidedImport {
      host: failed.host,
      import: failed.target,
      kind,
      value,
    };
    if let Some(target) = self.workspace(workspace) {
      // An import may be replaced only once (**C§3.2**), which is also what
      // keeps a metaprogram that answers every failure from asking for the
      // workspace to be compiled forever.
      if target
        .provided_imports
        .iter()
        .any(|already| already.host == provided.host && already.import == provided.import)
      {
        return;
      }
      target.provided_imports.push(provided);
    }
    if let Some(intercept) = self.intercept.as_mut() {
      intercept.recompile = true;
    }
  }

  /// The workspace that has to be compiled before the next message can be
  /// handed over, if there is one. The caller does the compiling, since it
  /// re-enters the compiler and this state must not be borrowed while it does.
  pub fn workspace_awaiting_compilation(&self) -> Option<Workspace> {
    let intercept = self.intercept.as_ref()?;
    if intercept.compiled && !intercept.recompile {
      return None;
    }
    self
      .workspaces
      .iter()
      .find(|workspace| workspace.id == intercept.workspace)
      .cloned()
  }

  /// Turns what compiling a workspace produced into the message stream the
  /// reference sends (**C§3.2**): one `IMPORT` per module instantiation and
  /// one `FILE` per file, then the phases, an `ERROR` for each error, and
  /// `COMPLETE` last.
  pub fn queue_messages(&mut self, compiled: &Compiled) {
    let Some(workspace) = self.intercept.as_ref().map(|i| i.workspace) else {
      return;
    };
    let head = |kind: Kind| Message { kind, workspace };
    // A `provide_import` starts the workspace over, so what is left of the
    // previous stream is not what happened (**C§3.2**). The messages
    // themselves stay alive: the metaprogram still holds their pointers.
    let again = match self.intercept.as_mut() {
      Some(intercept) => std::mem::take(&mut intercept.recompile),
      None => false,
    };
    if again && let Some(intercept) = self.intercept.as_mut() {
      intercept.queue.clear();
      intercept.finished = false;
    }

    let mut imports = Vec::with_capacity(compiled.modules.len());
    for module in &compiled.modules {
      let module_name = self.intern(module.name.as_bytes());
      let filename = match module.module_type {
        ModuleType::File => self.intern(module.entry.display().to_string().as_bytes()),
        _ => Str::EMPTY,
      };
      let stored = Stored::Import(Box::new(MessageImport {
        message: head(Kind::Import),
        module_type: module.module_type,
        module_name,
        fully_pathed_filename: filename,
      }));
      imports.push(stored.as_ptr().cast::<MessageImport>());
      self.push_message(stored);
    }

    let mut by_path = std::collections::HashMap::new();
    for file in &compiled.files {
      let filename = self.intern(file.path.display().to_string().as_bytes());
      let enclosing_import = file
        .module
        .and_then(|index| imports.get(index).copied())
        .unwrap_or(std::ptr::null());
      let stored = Stored::File(Box::new(MessageFile {
        message: head(Kind::File),
        fully_pathed_filename: filename,
        enclosing_import,
        from_a_string: file.from_a_string,
      }));
      by_path.insert(file.path.clone(), stored.as_ptr().cast::<MessageFile>());
      self.push_message(stored);
    }
    // `Code_Node.enclosing_load` is the `Message_File` of the file the node was
    // written in, and those messages exist only now (**C§3.2**).
    self.nodes.borrow_mut().attach_files(&by_path);

    // An import that did not happen is one the metaprogram may still answer,
    // so it is told before anything else can depend on it (**C§3.2**).
    self.failed_imports.clear();
    for failed in &compiled.failed_imports {
      let host_module_name = self.intern(failed.host.as_bytes());
      let target_module_name = self.intern(failed.target.as_bytes());
      let stored = Stored::FailedImport(Box::new(MessageFailedImport {
        message: head(Kind::FailedImport),
        status: failed.status,
        host_module_name,
        target_module_name,
        import_code: failed.import_code as *const crate::code::CodeDirectiveImport,
      }));
      self
        .failed_imports
        .insert(stored.as_ptr() as usize, failed.clone());
      self.push_message(stored);
    }

    // A batch of things whose typechecking finished, which the reference sends
    // as they come in and orangejuice sends once the compilation it replays is
    // over (`docs/spec.md` §10).
    for batch in &compiled.typechecked {
      let declarations = self.nodes_slice(&batch.declarations);
      let procedure_headers = self.nodes_slice(&batch.procedure_headers);
      let procedure_bodies = self.nodes_slice(&batch.procedure_bodies);
      let structs = self.nodes_slice(&batch.structs);
      let others = self.nodes_slice(&batch.others);
      let everything: Vec<crate::code::Typechecked> = batch
        .declarations
        .iter()
        .chain(&batch.procedure_headers)
        .chain(&batch.procedure_bodies)
        .chain(&batch.structs)
        .chain(&batch.others)
        .copied()
        .collect();
      let all = self.nodes_slice(&everything);
      self.push_message(Stored::Typechecked(Box::new(
        crate::code::MessageTypechecked {
          message: head(Kind::Typechecked),
          declarations,
          procedure_headers,
          procedure_bodies,
          structs,
          others,
          all,
        },
      )));
    }

    let executable = compiled
      .executable
      .as_ref()
      .map(|path| {
        let text = path.display().to_string();
        self.intern(text.as_bytes())
      })
      .unwrap_or(Str::EMPTY);
    let objects = self.intern_strings(&compiled.object_files);
    let system_libraries = self.intern_strings(&compiled.system_libraries);
    let user_libraries = self.intern_strings(&compiled.user_libraries);

    let phase = |phase: Phase| MessagePhase {
      message: head(Kind::Phase),
      phase,
      executable_name: Str::EMPTY,
      executable_write_failed: false,
      linker_exit_code: 0,
      num_items_waiting_to_typecheck: 0,
      compiler_generated_object_files: Slice::EMPTY,
      support_object_files: Slice::EMPTY,
      system_libraries: Slice::EMPTY,
      user_libraries: Slice::EMPTY,
    };
    self.push_message(Stored::Phase(Box::new(phase(Phase::AllSourceCodeParsed))));
    self.push_message(Stored::Phase(Box::new(phase(Phase::TypecheckedAllWeCan))));
    for _ in 0..compiled.errors {
      self.push_message(Stored::Plain(Box::new(head(Kind::Error))));
    }
    if !compiled.failed {
      self.push_message(Stored::Phase(Box::new(phase(Phase::AllTargetCodeBuilt))));
      // The metaprogram links: it is handed what the compiler made and runs
      // its own command, rather than being told about a link that happened
      // (**C§3.2**).
      if compiled.custom_link_command {
        self.push_message(Stored::Phase(Box::new(MessagePhase {
          executable_name: executable,
          compiler_generated_object_files: objects,
          system_libraries,
          user_libraries,
          ..phase(Phase::ReadyForCustomLinkCommand)
        })));
        self.push_message(Stored::Complete(Box::new(MessageComplete {
          message: head(Kind::Complete),
          error_code: ErrorCode::None,
        })));
        if let Some(intercept) = self.intercept.as_mut() {
          intercept.compiled = true;
        }
        return;
      }
      self.push_message(Stored::Phase(Box::new(MessagePhase {
        executable_name: executable,
        ..phase(Phase::PreWriteExecutable)
      })));
      self.push_message(Stored::Phase(Box::new(MessagePhase {
        executable_name: executable,
        executable_write_failed: compiled.executable.is_none(),
        compiler_generated_object_files: objects,
        system_libraries,
        user_libraries,
        ..phase(Phase::PostWriteExecutable)
      })));
    }
    self.push_message(Stored::Complete(Box::new(MessageComplete {
      message: head(Kind::Complete),
      error_code: if compiled.failed {
        ErrorCode::CompilationFailed
      } else {
        ErrorCode::None
      },
    })));

    if let Some(intercept) = self.intercept.as_mut() {
      intercept.compiled = true;
    }
    // A workspace whose compilation failed has failed, whether or not its
    // metaprogram says so: that is what makes the compiler exit non-zero when
    // the program it was asked to build did not build (**C§2.1**). A workspace
    // compiled again after a `provide_import` is judged by that attempt alone,
    // since the one that failed is not what happened.
    if compiled.failed || again {
      let status = match compiled.failed {
        true => WorkspaceStatus::Failed,
        false => WorkspaceStatus::Ok,
      };
      if let Some(target) = self.workspace(workspace) {
        target.status = status;
      }
    }
  }

  /// Copies exported entries into the node arena, which is where a metaprogram
  /// reads them from rather than out of a message that has gone away.
  fn nodes_slice(&mut self, items: &[crate::code::Typechecked]) -> Slice {
    self.nodes.borrow_mut().arena.alloc_slice(items)
  }

  fn push_message(&mut self, message: Stored) {
    let index = self.messages.len();
    self.messages.push(message);
    if let Some(intercept) = self.intercept.as_mut() {
      intercept.queue.push_back(index);
    }
  }

  /// The next message a metaprogram waiting on the stream receives. `None`
  /// means the stream is exhausted, which the reference calls an error.
  pub fn next_message(&mut self) -> Option<*const Message> {
    let index = self.intercept.as_mut()?.queue.pop_front()?;
    let message = self.messages.get(index)?;
    if message.kind() == Kind::Complete
      && let Some(intercept) = self.intercept.as_mut()
    {
      intercept.finished = true;
    }
    Some(self.messages[index].as_ptr())
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

  /// The workspaces a driver still has to build: the ones source was added to
  /// and that nobody destroyed.
  pub fn buildable(&self) -> impl Iterator<Item = &Workspace> {
    self.workspaces.iter().filter(|workspace| {
      workspace.started && !workspace.destroyed && !workspace.intercepted && !workspace.implicit
    })
  }
}

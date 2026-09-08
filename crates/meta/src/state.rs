//! What one compilation's metaprogram has asked the compiler for.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use crate::abi::{Slice, Str};
use crate::message::{
  ErrorCode, Kind, Message, MessageComplete, MessageFile, MessageImport, MessagePhase, ModuleType,
  Phase, Stored,
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
  /// The compilation.s own workspace, which nobody created and nobody builds
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
  pub errors: usize,
  pub failed: bool,
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
  /// How a workspace gets compiled, installed by the driver.
  pub compiler: Option<Compiler>,
  /// Directories `compiler_add_library_search_directory` added (**C§3.3**).
  pub library_directories: Vec<PathBuf>,
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

  /// The workspace that has to be compiled before the next message can be
  /// handed over, if there is one. The caller does the compiling, since it
  /// re-enters the compiler and this state must not be borrowed while it does.
  pub fn workspace_awaiting_compilation(&self) -> Option<Workspace> {
    let intercept = self.intercept.as_ref()?;
    if intercept.compiled {
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

    for file in &compiled.files {
      let filename = self.intern(file.path.display().to_string().as_bytes());
      let enclosing_import = file
        .module
        .and_then(|index| imports.get(index).copied())
        .unwrap_or(std::ptr::null());
      self.push_message(Stored::File(Box::new(MessageFile {
        message: head(Kind::File),
        fully_pathed_filename: filename,
        enclosing_import,
        from_a_string: file.from_a_string,
      })));
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

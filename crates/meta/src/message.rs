//! The messages a metaprogram intercepts (**C§3.2**).
//!
//! Each struct mirrors the one `Compiler.jai` declares, member for member, so
//! that handing a metaprogram a pointer to one of ours is the whole of the
//! conversion. `crates/meta/tests/layout.rs` measures the distribution's
//! against these.

use crate::abi::{Slice, Str};

/// `Message.kind` (**C§3.2**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
  Uninitialized = 0,
  File = 1,
  Import = 2,
  FailedImport = 3,
  Phase = 4,
  Typechecked = 5,
  Complete = 6,
  DebugDump = 7,
  Error = 8,
  PerformanceReport = 9,
}

/// `Message_Import.module_type` (**C§3.2**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ModuleType {
  Uninitialized = 0,
  Preload = 1,
  RuntimeSupport = 2,
  MainProgram = 3,
  File = 4,
}

/// `Message_Phase.phase` (**C§3.2**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Phase {
  AllSourceCodeParsed = 0,
  TypecheckedAllWeCan = 1,
  AllTargetCodeBuilt = 2,
  PreWriteExecutable = 3,
  PostWriteExecutable = 4,
  ReadyForCustomLinkCommand = 5,
}

/// `Message_Complete.error_code` (**C§3.2**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCode {
  None = 0,
  CompilationFailed = 1,
  CompilerShutdown = 2,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Message {
  pub kind: Kind,
  pub workspace: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MessageFile {
  pub message: Message,
  pub fully_pathed_filename: Str,
  pub enclosing_import: *const MessageImport,
  pub from_a_string: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MessageImport {
  pub message: Message,
  pub module_type: ModuleType,
  pub module_name: Str,
  pub fully_pathed_filename: Str,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MessagePhase {
  pub message: Message,
  pub phase: Phase,
  pub executable_name: Str,
  pub executable_write_failed: bool,
  pub linker_exit_code: i32,
  pub num_items_waiting_to_typecheck: i32,
  pub compiler_generated_object_files: Slice,
  pub support_object_files: Slice,
  pub system_libraries: Slice,
  pub user_libraries: Slice,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MessageComplete {
  pub message: Message,
  pub error_code: ErrorCode,
}

/// One message the compiler produced, kept alive for as long as the
/// compilation is: a metaprogram holds the pointer it was handed for as long
/// as it likes.
#[derive(Debug)]
pub enum Stored {
  Plain(Box<Message>),
  File(Box<MessageFile>),
  Import(Box<MessageImport>),
  Phase(Box<MessagePhase>),
  Complete(Box<MessageComplete>),
}

impl Stored {
  /// The address a metaprogram receives, which it casts by `kind`.
  pub fn as_ptr(&self) -> *const Message {
    match self {
      Self::Plain(message) => &raw const **message,
      Self::File(message) => (&raw const **message).cast(),
      Self::Import(message) => (&raw const **message).cast(),
      Self::Phase(message) => (&raw const **message).cast(),
      Self::Complete(message) => (&raw const **message).cast(),
    }
  }

  pub fn kind(&self) -> Kind {
    match self {
      Self::Plain(message) => message.kind,
      Self::File(message) => message.message.kind,
      Self::Import(message) => message.message.kind,
      Self::Phase(message) => message.message.kind,
      Self::Complete(message) => message.message.kind,
    }
  }
}

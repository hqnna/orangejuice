//! The `Compiler` module's half of the compiler (**C§3**).
//!
//! `modules/Compiler/Compiler.jai` declares its procedures
//! `#compiler`: they have no body, and the compiler is what answers them. This
//! crate is those answers. Each one is an `extern "C"` procedure built to the
//! calling convention `oj_ir::abi_of` describes — a value that does not fit a
//! register arrives by pointer, a return that does not fit comes back through
//! caller storage handed in first, and a trailing `*#Context` closes the list
//! — so the JIT can bind the symbol straight to it (`docs/spec.md` §6.5).
//!
//! The state they work on is one [`Meta`] per compilation, installed on the
//! thread that runs compile-time code for as long as that compilation lasts.
//! A trampoline has nowhere to carry a compiler pointer, so that is how it
//! finds one; a call made with no compilation installed does nothing, which is
//! what a `#run` executed by a test without a driver behind it sees.

use std::cell::RefCell;
use std::path::PathBuf;

mod abi;
mod code;
mod intrinsics;
mod message;
mod rewrite;
mod state;

pub use abi::{Slice, SourceCodeLocation, Str, VersionInfo};
pub use code::{
  Arena, CodeArgument, CodeArrayLiteralInfo, CodeAsm, CodeBinaryOperator, CodeBlock, CodeCase,
  CodeCast, CodeCommaSeparatedArgument, CodeCommaSeparatedArguments, CodeCompoundDeclaration,
  CodeDeclaration, CodeDefer, CodeDirectiveAddContext, CodeDirectiveBake, CodeDirectiveBytes,
  CodeDirectiveCode, CodeDirectiveExists, CodeDirectiveImport, CodeDirectiveInsert,
  CodeDirectiveLibrary, CodeDirectiveLoad, CodeDirectiveLocation, CodeDirectiveModify,
  CodeDirectiveModuleParameters, CodeDirectivePlace, CodeDirectivePokeName,
  CodeDirectiveProcedureName, CodeDirectiveRun, CodeDirectiveScope, CodeDirectiveWildcard,
  CodeEnum, CodeExpressionQuery, CodeExtract, CodeFor, CodeIdent, CodeIf, CodeLiteral,
  CodeLiteralValues, CodeLoopControl, CodeMakeVarargs, CodeNode, CodeNote, CodePointerLiteralInfo,
  CodeProcedureBody, CodeProcedureCall, CodeProcedureHeader, CodePushContext, CodeResolvedOverload,
  CodeReturn, CodeScopeEntry, CodeStruct, CodeStructLiteralInfo, CodeTypeDefinition,
  CodeTypeInstantiation, CodeTypeQuery, CodeUnaryOperator, CodeUsing, CodeWhile,
  ContextModification, Key, Location, MessageTypechecked, Nodes, Tree, Typechecked, literal_type,
};
pub use message::{
  ErrorCode, ImportStatus, Kind, Message, MessageComplete, MessageFailedImport, MessageFile,
  MessageImport, MessagePhase, ModuleType, Phase, Stored,
};
pub use rewrite::Rewriter;
pub use state::{
  BuildOptionsLayout, Compiled, CompiledFailedImport, CompiledFile, CompiledModule, Compiler,
  DataSegment, DuringCompileLayout, GlobalData, Intercept, Meta, ProvidedImport, Report,
  ReportMode, ScopedString, StringScope, TypecheckedBatch, Workspace, WorkspaceStatus,
};

thread_local! {
  static INSTALLED: RefCell<Option<Meta>> = const { RefCell::new(None) };
}

/// Installs the compilation compile-time code will work on, and hands back
/// whatever the previous one was so that a nested compilation can restore it.
pub fn install(meta: Meta) -> Option<Meta> {
  INSTALLED.with(|slot| slot.borrow_mut().replace(meta))
}

/// Takes the installed compilation back, which is how a driver reads what the
/// metaprogram asked for once its `#run`s are done.
pub fn uninstall() -> Option<Meta> {
  INSTALLED.with(|slot| slot.borrow_mut().take())
}

/// Runs `read` against the installed compilation, if there is one.
pub fn with<R>(read: impl FnOnce(&mut Meta) -> R) -> Option<R> {
  INSTALLED.with(|slot| slot.borrow_mut().as_mut().map(read))
}

/// The `#compiler` procedures this compiler answers, by the symbol the JIT
/// binds them under (**C§3.3**). A `#compiler "name"` names its own symbol;
/// everything else takes the name it was declared with.
pub fn intrinsics() -> &'static [(&'static str, usize)] {
  static TABLE: std::sync::OnceLock<Vec<(&str, usize)>> = std::sync::OnceLock::new();
  TABLE.get_or_init(intrinsics::table)
}

/// The address the JIT should bind `symbol` to, when this compiler answers it.
pub fn intrinsic(symbol: &str) -> Option<usize> {
  intrinsics()
    .iter()
    .find(|(name, _)| *name == symbol)
    .map(|(_, address)| *address)
}

/// Where the distribution whose `modules/` the compilation reads lives, which
/// is what `compiler_get_base_path` reports (**C§3.3**).
pub fn base_path_of(distribution: Option<&PathBuf>) -> String {
  distribution
    .map(|dir| dir.display().to_string())
    .unwrap_or_default()
}

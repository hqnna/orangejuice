//! The backend-independent typed IR of orangejuice.
//!
//! [`lower`] turns a typechecked [`oj_sema::Checker`] into a [`Program`] of
//! procedures, blocks and instructions that name types by [`oj_types::TypeId`]
//! and nothing else. Lowering starts at the program's entry point and follows
//! calls, so a procedure nothing reachable calls is never lowered — which is
//! both the dead code elimination of **L§11.6** and what keeps the front end's
//! unfinished milestones out of the back end.

mod ir;
mod lower;
mod print;
mod typetable;

pub use ir::{
  Abi, AbiParameter, AsmBinding, BinaryOp, Block, BlockId, Callee, ConstLink, Constant,
  ConvertKind, Global, GlobalId, GlobalInit, Inst, Library, Local, LocalId, ParameterKind, ProcId,
  Procedure, ProcedureFlags, Program, Terminator, TypeTableImage, UnaryOp, ValueId, abi_of,
  is_scalar,
};
pub use lower::{
  Lowered, MODIFY_ACCEPT, MODIFY_REASON, MODIFY_VARIABLE_SIZE, MODIFY_VARIABLES, Modify, Run,
  lower, lower_library, lower_modify, lower_run, lower_with_roots, modify_result_size,
};
pub use print::{print_ir, summary};
pub use typetable::TypeImage;

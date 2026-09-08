//! The Jai typechecker.
//!
//! [`Checker`] gives every declaration of a resolved [`oj_scope::Program`] a
//! type, building the struct, enum, variant and procedure types it names on the
//! way (**L§3**, **L§8**, **L§9**). Resolution is demand-driven and memoized:
//! asking for a declaration's type resolves what it depends on first, which is
//! what makes data scopes order-independent (**L§11.7**).

mod aggregate;
mod body;
mod call;
mod cast;
mod checker;
mod constants;
mod convert;
mod decl;
mod dump;
mod expr;
mod fold;
mod literal;
mod member;
mod operators;
mod overload;
mod poly;
mod query;
mod run;
mod typeexpr;

pub use checker::{Checker, DeclType, Expr};
pub use constants::{Const, Value};
pub use dump::{print_types, summary};
pub use poly::InstanceId;
pub use query::{CallPlan, InstanceInfo, PlannedArgument, ProcedureBody};
pub use run::{CompileTime, RUN_SYMBOL_PREFIX, RunOutcome, RunRequest};

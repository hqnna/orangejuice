//! Compile-time execution for orangejuice (**L§12**, `docs/spec.md` §6.5).
//!
//! [`Engine`] is what a `#run` reaches: it lowers the run, compiles it into
//! the ORC JIT of [`Orc`], calls it, and reads the answer back as a front-end
//! constant. The globals it touches live in `oj-runtime`'s data segments, so
//! two runs of one compilation see the same `x` and a `#no_reset` global can
//! hand its bytes to the executable at the end.

mod engine;
mod orc;

pub use engine::Engine;
pub use orc::Orc;

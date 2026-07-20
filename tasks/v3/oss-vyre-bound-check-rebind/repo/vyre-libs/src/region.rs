//! Shared Region builder  -  moved into the standalone `vyre-harness`
//! crate so external Cat-A wrappers (`downstream dataflow engine`, `decodex`, `multimatch`)
//! can construct provenance-tagged Programs without depending on the
//! rest of `vyre-libs`. This module is a thin re-export so existing
//! call sites keep compiling unchanged.

pub use vyre_harness::region::{
    reparent_program_children, tag_program, wrap, wrap_anonymous, wrap_child,
};

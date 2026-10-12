//! Compatibility re-export for the DFA compile surface.
//!
//! The compile data model lives in `vyre-primitives` so decode and
//! other lower-level dialects do not depend on `vyre-libs::matching`.

pub use vyre_primitives::matching::{
    dfa_compile, dfa_compile_with_budget, CompiledDfa, DfaCompileError, DEFAULT_DFA_BUDGET_BYTES,
};

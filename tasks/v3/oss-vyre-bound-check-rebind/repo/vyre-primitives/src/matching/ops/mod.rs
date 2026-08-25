//! Back-compat shim  -  the authoritative modules live one level up
//! (`vyre_primitives::matching::bracket_match`). This `ops` submodule
//! preserves the older import path for consumers written before the
//! flatten.
//!
//! New code should use the parent path.

/// GPU-Native Stack-based Bracket Matching
pub mod bracket_match {
    #[cfg(any(test, feature = "cpu-parity"))]
    pub use crate::matching::bracket_match::cpu_ref;
    #[cfg(any(test, feature = "cpu-parity"))]
    pub use crate::matching::bracket_match::cpu_ref_into;
    pub use crate::matching::bracket_match::{
        bracket_match, bracket_match_dispatch_grid, pack_u32,
        BRACKET_MATCH_PARALLEL_WORKGROUP_SIZE, CLOSE_BRACE, MATCH_NONE, OPEN_BRACE, OP_ID, OTHER,
    };
}

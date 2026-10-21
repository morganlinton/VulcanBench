//! Evidence object emission.
//!
//! When the Rust frontend compiles a translation unit, it emits an
//! object artifact containing:
//! - The parsed AST (for debugging / parity)
//! - The verified Vyre IR program
//! - Borrow-check evidence
//!
//! The object writer is intentionally not exposed until the Rust frontend can
//! emit verified IR and borrow-check evidence. Keeping the builder private to
//! this crate prevents downstream users from depending on an empty artifact.

/// Object artifact builder.
pub struct RustObjectBuilder;

impl RustObjectBuilder {
    /// Create a new object builder.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RustObjectBuilder {
    fn default() -> Self {
        Self::new()
    }
}

//! Backend-owned lowering contracts.
//!
//! Core owns the stable lowering protocol only. Concrete target lowering
//! belongs to backend crates so frontend IR consumers do not link shader or
//! device-specific machinery.

/// Effects-typed lowering pipeline (P-1.0-V1.3): walks a `Program`
/// and computes the row of effect kinds the backend will see.
pub mod effects;

/// Subgroup-first lowering (Phase 2.3): converts workgroup-tree reductions
/// to `subgroup_add` / `subgroup_shuffle` when the backend supports them.
pub mod subgroup_lowering;

pub use effects::{compute_program_effects, ProgramEffects};
pub use subgroup_lowering::lower_subgroup_reductions;

use crate::ir_inner::model::types::DataType;
use std::{error::Error, fmt};

/// Error raised while progressively lowering a [`crate::ir::Program`] into a backend IR
/// and then into a concrete target artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringError {
    message: String,
}

impl LoweringError {
    /// Construct an actionable lowering error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        debug_assert!(message.contains("Fix:"));
        Self { message }
    }

    /// Construct an invalid-program lowering error.
    #[must_use]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(message)
    }

    /// Construct an unsupported-type lowering error.
    #[must_use]
    pub fn unsupported_type(data_type: &DataType) -> Self {
        Self::new(format!(
            "unsupported data type {data_type:?} for this lowering target. Fix: add target support or route the op to a backend that declares this type capability."
        ))
    }

    /// Construct an unsupported-operation lowering error.
    #[must_use]
    pub fn unsupported_op(op: impl fmt::Debug) -> Self {
        Self::new(format!(
            "unsupported operation {op:?} for this lowering target. Fix: add target lowering support or route the op to a backend that declares this operation capability."
        ))
    }

    /// Construct a target builder validation lowering error.
    #[must_use]
    pub fn validation(error: impl std::error::Error) -> Self {
        Self::new(format!(
            "target builder validation failed: {error}\nSource: {:#?}\nFix: repair the backend lowering contract before dispatch.", error.source()
        ))
    }

    /// Construct a target writer lowering error.
    #[must_use]
    pub fn writer(error: impl fmt::Display) -> Self {
        Self::new(format!(
            "target writer failed: {error}. Fix: repair the backend writer integration."
        ))
    }

    /// Return the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for LoweringError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_contains_fix_hint() {
        let err = LoweringError::new("buffer overflow. Fix: split the input.");
        assert!(err.message().contains("Fix:"));
    }

    #[test]
    fn unsupported_type_message() {
        let err = LoweringError::unsupported_type(&DataType::Bytes);
        assert!(err.message().contains("Bytes"));
        assert!(err.message().contains("Fix:"));
    }

    #[test]
    fn unsupported_op_message() {
        let err = LoweringError::unsupported_op("warp_shuffle");
        assert!(err.message().contains("warp_shuffle"));
        assert!(err.message().contains("Fix:"));
    }

    #[test]
    fn display_is_message() {
        let err = LoweringError::new("test. Fix: handle it.");
        assert_eq!(err.to_string(), "test. Fix: handle it.");
    }

    #[test]
    fn is_std_error() {
        let err = LoweringError::new("fail. Fix: retry.");
        let _: &dyn Error = &err;
    }

    #[test]
    fn clone_and_eq() {
        let a = LoweringError::new("same. Fix: ok.");
        let b = a.clone();
        assert_eq!(a, b);
    }
}

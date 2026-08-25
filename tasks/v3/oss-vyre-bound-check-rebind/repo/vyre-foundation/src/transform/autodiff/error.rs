//! Autodiff error types.

use std::fmt;

/// Errors returned by the autodiff IR transform.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AutodiffError {
    /// An expression node is not differentiable (integer, bitwise, comparison).
    NotDifferentiable {
        /// Human-readable description of the non-differentiable op.
        op: String,
        /// Actionable fix hint.
        fix: String,
    },
    /// A buffer referenced in the `outputs` or `inputs` set was not found
    /// in the Program's buffer declarations.
    BufferNotFound {
        /// Missing buffer name.
        name: String,
    },
    /// The Program contains an IR construct the transform doesn't handle yet.
    UnsupportedNode {
        /// Node kind description.
        kind: String,
    },
}

impl fmt::Display for AutodiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDifferentiable { op, fix } => {
                write!(f, "autodiff: op `{op}` is not differentiable. Fix: {fix}")
            }
            Self::BufferNotFound { name } => {
                write!(f, "autodiff: buffer `{name}` not found in Program. Fix: check buffer declarations match the output/input sets.")
            }
            Self::UnsupportedNode { kind } => {
                write!(f, "autodiff: unsupported IR node `{kind}`. Fix: expand autodiff coverage or restructure the Program.")
            }
        }
    }
}

impl std::error::Error for AutodiffError {}

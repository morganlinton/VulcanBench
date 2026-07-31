//! Error types for IR validation, wire-format decoding, and GPU operations.
//!
//! Vyre unifies all failure modes under a single `Error` enum so that
//! frontends, backends, and the conform gate speak the same language.
//! Every variant carries an actionable `Fix:` message that tells the caller
//! exactly what invariant was violated and how to recover.

use thiserror::Error;

/// Shorthand result type used throughout the vyre public API.
///
/// All fallible vyre operations return `Result<T>` so that callers only need
/// to learn one error representation. The unified type ensures that a
/// frontend emitting bad IR, a backend hitting an adapter limit, or a
/// wire-format decoder seeing truncated bytes all produce the same top-level
/// failure.
pub type Result<T> = std::result::Result<T, Error>;

/// The unified failure enum for every vyre operation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum Error {
    /// A recursive composition cycle was found during operation inlining.
    #[error(
        "IR inlining cycle at operation `{op_id}`. Fix: remove the recursive Expr::Call chain or split the recursive algorithm into an explicit bounded Loop."
    )]
    InlineCycle {
        /// The operation identifier that closed the cycle.
        op_id: String,
    },

    /// Operation inlining could not resolve an operation id.
    #[error(
        "IR inlining could not resolve operation `{op_id}`. Fix: register a Category A operation with this id before lowering or replace the call with inline IR."
    )]
    InlineUnknownOp {
        /// The missing operation identifier.
        op_id: String,
    },

    /// Operation inlining rejected an operation that must dispatch separately.
    #[error(
        "IR inlining rejected non-inlinable operation `{op_id}`. Fix: this op processes buffer inputs and must be dispatched as a separate kernel, not composed via Expr::Call."
    )]
    InlineNonInlinable {
        /// The operation identifier that cannot be inlined.
        op_id: String,
    },

    /// The number of arguments passed to an inlined operation did not match.
    #[error(
        "IR inlining argument count mismatch for operation `{op_id}`: expected {expected}, got {got}. Fix: pass exactly one argument for each ReadOnly or Uniform input buffer declared by the callee program."
    )]
    InlineArgCountMismatch {
        /// The operation identifier being expanded.
        op_id: String,
        /// The number of arguments the callee expects.
        expected: usize,
        /// The number of arguments the caller provided.
        got: usize,
    },

    /// The inlined operation never wrote to its declared output buffer.
    #[error(
        "IR inlining found no output write for operation `{op_id}`. Fix: Ensure the op's program() body writes to its output buffer at least once."
    )]
    InlineNoOutput {
        /// The operation identifier being expanded.
        op_id: String,
    },

    /// The inlined operation declared an invalid number of output buffers.
    #[error(
        "IR inlining found {got} declared output buffers for operation `{op_id}`. Fix: mark exactly one result buffer with BufferDecl::output(...)."
    )]
    InlineOutputCountMismatch {
        /// The operation identifier being expanded.
        op_id: String,
        /// The actual number of buffers marked as outputs.
        got: usize,
    },

    /// Wire-format payload failed validation checks.
    #[error(
        "Wire-format validation failed: {message}. Fix: recompile the frontend program set and ensure the compiler only emits valid instructions."
    )]
    WireFormatValidation {
        /// Human-readable description of the validation failure.
        message: String,
    },

    /// target-text lowering failed before a shader could be emitted.
    #[error(
        "vyre target-text lowering: {message}. Fix: inspect the Program shape, backend capability report, and emitted shader diagnostics before retrying."
    )]
    Lowering {
        /// Human-readable description of the lowering failure.
        message: String,
    },

    /// Reference interpreter execution failed.
    #[error(
        "vyre reference interpreter: {message}. Fix: validate the Program and input buffer set before invoking the reference backend."
    )]
    Interp {
        /// Human-readable description of the interpreter failure.
        message: String,
    },

    /// GPU execution failed.
    #[error(
        "GPU pipeline failed: {message}. Fix: verify a concrete driver is linked and the compiled buffers fit the target adapter limits."
    )]
    Gpu {
        /// Description of the GPU failure.
        message: String,
    },

    /// Decode configuration failed validation.
    #[error(
        "Decode configuration failed: {message}. Fix: provide valid TOML and non-zero decode thresholds."
    )]
    DecodeConfig {
        /// Description of the configuration failure.
        message: String,
    },

    /// Decode execution or readback failed validation.
    #[error(
        "Decode pipeline failed: {message}. Fix: inspect shader output sizing and source-region validation."
    )]
    Decode {
        /// Description of the decode failure.
        message: String,
    },

    /// Decompression execution, sizing, or readback failed validation.
    #[error(
        "Decompression pipeline failed: {message}. Fix: validate frame metadata, split oversized payloads, and inspect GPU decompression status words."
    )]
    Decompress {
        /// Description of the decompression failure.
        message: String,
    },

    /// DFA compilation or scanning failed.
    #[error(
        "DFA pipeline failed: {message}. Fix: validate DFA transition tables, output links, and target adapter limits."
    )]
    Dfa {
        /// Description of the DFA failure.
        message: String,
    },

    /// Dataflow graph execution failed.
    #[error(
        "Dataflow pipeline failed: {message}. Fix: validate graph inputs, buffer sizing, and target adapter limits."
    )]
    Dataflow {
        /// Description of the dataflow failure.
        message: String,
    },

    /// Prefix-array construction failed before allocation or upload.
    #[error(
        "Prefix construction failed: {message}. Fix: split the input before building prefix arrays or reduce per-file scan size."
    )]
    Prefix {
        /// Description of the prefix construction failure.
        message: String,
    },

    /// CSR graph construction or validation failed.
    #[error(
        "CSR graph construction failed: {message}. Fix: cap graph size and ensure every edge endpoint is within node_count."
    )]
    Csr {
        /// Description of the CSR failure.
        message: String,
    },

    /// Serialization or deserialization failed.
    #[error(
        "Serialization failed: {message}. Fix: verify the wire payload is not truncated or corrupted."
    )]
    Serialization {
        /// Description of the serialization failure.
        message: String,
    },

    /// Rule formula construction or evaluation failed.
    #[error(
        "Rule evaluation failed: {message}. Fix: validate rule pattern ids, thresholds, and verdict buffer sizing before lowering."
    )]
    RuleEval {
        /// Description of the rule failure.
        message: String,
    },

    /// Wire-format schema version mismatch.
    #[error(
        "Wire-format version mismatch: expected {expected}, found {found}. Fix: re-encode with a matching vyre version or upgrade this runtime."
    )]
    VersionMismatch {
        /// The schema version this runtime understands.
        expected: u32,
        /// The schema version present on the wire.
        found: u32,
    },

    /// Unknown dialect on the wire.
    #[error(
        "Unknown dialect `{name}` (requested version `{requested}`). Fix: link the dialect crate providing `{name}` into this runtime or drop the op that uses it before encoding."
    )]
    UnknownDialect {
        /// The dialect identifier on the wire (e.g. `"workgroup"`).
        name: String,
        /// The version string the encoder recorded for the dialect.
        requested: String,
    },

    /// Unknown op inside a known dialect.
    #[error(
        "Unknown op `{op}` in dialect `{dialect}`. Fix: upgrade the runtime to a version that includes this op, or drop the op before encoding."
    )]
    UnknownOp {
        /// The dialect that should contain the op.
        dialect: String,
        /// The op identifier that could not be resolved.
        op: String,
    },
}

impl Error {
    /// Build a target-text lowering error with actionable guidance.
    #[must_use]
    pub fn lowering(message: impl Into<String>) -> Self {
        Self::Lowering {
            message: message.into(),
        }
    }

    /// Build a reference-interpreter error with actionable guidance.
    #[must_use]
    pub fn interp(message: impl Into<String>) -> Self {
        Self::Interp {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowering_helper_contains_fix_hint() {
        let err = Error::lowering("buffer too large");
        let msg = err.to_string();
        assert!(msg.contains("buffer too large"));
        assert!(msg.contains("Fix:"));
    }

    #[test]
    fn interp_helper_contains_fix_hint() {
        let err = Error::interp("division by zero");
        let msg = err.to_string();
        assert!(msg.contains("division by zero"));
        assert!(msg.contains("Fix:"));
    }

    #[test]
    fn inline_cycle_display() {
        let err = Error::InlineCycle {
            op_id: "math::add".into(),
        };
        assert!(err.to_string().contains("math::add"));
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn version_mismatch_display() {
        let err = Error::VersionMismatch {
            expected: 6,
            found: 5,
        };
        let msg = err.to_string();
        assert!(msg.contains("6"));
        assert!(msg.contains("5"));
    }

    #[test]
    fn unknown_dialect_display() {
        let err = Error::UnknownDialect {
            name: "my-dialect".into(),
            requested: "1.0".into(),
        };
        assert!(err.to_string().contains("my-dialect"));
    }

    #[test]
    fn error_is_clone_and_eq() {
        let a = Error::lowering("test");
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn inline_arg_count_mismatch_display() {
        let err = Error::InlineArgCountMismatch {
            op_id: "test::op".into(),
            expected: 3,
            got: 1,
        };
        let msg = err.to_string();
        assert!(msg.contains("expected 3"));
        assert!(msg.contains("got 1"));
    }
}

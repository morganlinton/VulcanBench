//! CPU reference execution contract for operation types.

use crate::ir_inner::model::program::Program;
pub use vyre_spec::CpuFn;

/// CPU reference implementation for an operation.
pub trait CpuOp {
    /// Execute one flat byte payload and append the byte output to `output`.
    fn cpu(input: &[u8], output: &mut Vec<u8>);
}

/// Marker trait for Category A operations with an executable IR program.
pub trait CategoryAOp {
    /// Build the canonical Category A IR program.
    fn program() -> Program;
}

/// Failing CPU adapter for intrinsics whose existing reference accepts structured buffers.
///
/// This is the explicit reference-oracle sentinel for Category C ops whose
/// typed CPU reference is intentionally not exposed through the flat ABI. The
/// function clears the output buffer and returns no flat result. Runtime
/// dispatchers must reject this sentinel through [`is_cpu_reference_sentinel`]
/// before invocation so callers cannot consume an empty byte vector as a valid
/// CPU reference result.
///
/// Each op can register its own CPU ref via `vyre-reference`, and
/// `DialectRegistry::get_lowering(ReferenceBackend)` dispatches to it
/// directly rather than going through this sentinel.
///
/// AUDIT_2026-05-23: Deprecated - CPU sentinels are fallback holes.
/// Category C ops must implement typed GPU lowerings instead.
#[deprecated(
    note = "structured_intrinsic_cpu is a non-executable fallback sentinel. Implement typed GPU lowering for the op."
)]
pub fn structured_intrinsic_cpu(input: &[u8], output: &mut Vec<u8>) {
    let _ = input;
    output.clear();
}

/// True when [`structured_intrinsic_cpu`] is set as an op's CPU lowering.
///
/// Conformance tooling uses this to flag operations that still expose only the
/// structured-reference sentinel, so parity status is recorded explicitly
/// instead of pretending a flat CPU adapter exists.
#[must_use]
pub fn is_cpu_reference_sentinel(f: CpuFn) -> bool {
    #[allow(deprecated)]
    std::ptr::fn_addr_eq(f, structured_intrinsic_cpu as CpuFn)
}

/// Compatibility wrapper for older conformance tooling.
#[deprecated(
    note = "use is_cpu_reference_sentinel; CPU reference sentinels are explicit oracles, not runtime fallbacks"
)]
#[must_use]
pub fn is_fallback_cpu_ref(f: CpuFn) -> bool {
    is_cpu_reference_sentinel(f)
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn is_cpu_reference_sentinel_detects_structured_intrinsic() {
        assert!(is_cpu_reference_sentinel(structured_intrinsic_cpu));
    }

    #[test]
    fn is_cpu_reference_sentinel_rejects_other_fn() {
        #[allow(clippy::ptr_arg)] // Must match `CpuFn` (`&mut Vec<u8>`), not `&mut [u8]`.
        fn custom_cpu(_input: &[u8], _output: &mut Vec<u8>) {}
        assert!(!is_cpu_reference_sentinel(custom_cpu));
    }

    #[test]
    fn structured_intrinsic_clears_output_without_flat_result() {
        let mut output = vec![1, 2, 3];
        structured_intrinsic_cpu(b"input", &mut output);
        assert!(output.is_empty());
    }
}

//! Substructural-type discipline substrate consumer (P-PRIM-14).
//!
//! Wires `vyre_primitives::types::linear_check::check_linear_use` into
//! the dispatch path so backends can verify each buffer's declared
//! discipline before lowering. The vyre-foundation linear-type validator
//! walks the program counting uses; this consumer is the per-buffer
//! decision step it reaches for.

use vyre_primitives::types::{
    check_linear_use as primitive_check_linear, LinearDiscipline, LinearTypeError,
};

/// Verify the declared discipline against the observed use count.
/// Bumps the dataflow-fixpoint substrate counter so observability
/// dashboards register every per-buffer linear-type decision.
///
/// # Errors
///
/// Returns [`LinearTypeError`] when the count violates the
/// discipline (Linear/Relevant dropped, or Linear/Affine reused).
pub fn verify_use_count(discipline: LinearDiscipline, uses: u32) -> Result<(), LinearTypeError> {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_check_linear(discipline, uses)
}

/// Convenience: bool form. True iff `uses` satisfies `discipline`.
#[must_use]
pub fn use_count_ok(discipline: LinearDiscipline, uses: u32) -> bool {
    verify_use_count(discipline, uses).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_one_use_passes() {
        assert!(use_count_ok(LinearDiscipline::Linear, 1));
    }

    #[test]
    fn linear_zero_uses_dropped() {
        let err = verify_use_count(LinearDiscipline::Linear, 0).unwrap_err();
        assert!(matches!(err, LinearTypeError::Dropped { .. }));
    }

    #[test]
    fn affine_two_uses_reused() {
        let err = verify_use_count(LinearDiscipline::Affine, 2).unwrap_err();
        assert!(matches!(err, LinearTypeError::Reused { uses: 2, .. }));
    }

    #[test]
    fn relevant_zero_dropped() {
        assert!(matches!(
            verify_use_count(LinearDiscipline::Relevant, 0).unwrap_err(),
            LinearTypeError::Dropped { .. }
        ));
    }

    #[test]
    fn unrestricted_accepts_any_count() {
        for uses in [0u32, 1, 100, u32::MAX] {
            assert!(use_count_ok(LinearDiscipline::Unrestricted, uses));
        }
    }

    /// Closure-bar: substrate output equals primitive output exactly.
    #[test]
    fn matches_primitive_directly() {
        let cases = [
            (LinearDiscipline::Linear, 0),
            (LinearDiscipline::Linear, 1),
            (LinearDiscipline::Linear, 2),
            (LinearDiscipline::Affine, 0),
            (LinearDiscipline::Affine, 1),
            (LinearDiscipline::Affine, 5),
            (LinearDiscipline::Relevant, 0),
            (LinearDiscipline::Relevant, 1),
            (LinearDiscipline::Relevant, 9),
            (LinearDiscipline::Unrestricted, 0),
            (LinearDiscipline::Unrestricted, 100),
        ];
        for (d, u) in cases {
            assert_eq!(
                verify_use_count(d, u),
                primitive_check_linear(d, u),
                "drift on ({:?}, {})",
                d,
                u
            );
        }
    }

    /// Adversarial: extreme use count must be reported with the
    /// actual count in the error, not truncated.
    #[test]
    fn reused_error_carries_actual_count() {
        let err = verify_use_count(LinearDiscipline::Linear, 1234).unwrap_err();
        assert!(matches!(
            err,
            LinearTypeError::Reused {
                discipline: LinearDiscipline::Linear,
                uses: 1234
            }
        ));
    }
}

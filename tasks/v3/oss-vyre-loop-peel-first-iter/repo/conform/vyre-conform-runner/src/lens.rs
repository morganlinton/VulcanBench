//! Conformance lenses re-exported from the shared test harness.
//!
//! The runner intentionally does not fork lens semantics. Witness,
//! CPU-vs-backend, fixpoint, and convergence parity are all owned by
//! `vyre-test-harness` so CI, local conformance runs, and backend-specific
//! parity tests apply one oracle contract.

pub use vyre_test_harness::lens::{convergence, cpu_vs_backend, fixpoint, witness, LensOutcome};

#[cfg(test)]
mod tests {
    use super::LensOutcome;

    #[test]
    fn runner_lens_reexport_preserves_outcome_contract() {
        let pass = LensOutcome::Pass { cases: 3 };
        assert!(pass.is_ok());
        assert!(pass.is_pass());

        let fail = LensOutcome::Fail {
            case_index: 1,
            detail: "generated failure".to_string(),
        };
        assert!(!fail.is_ok());
        assert!(!fail.is_pass());
    }
}

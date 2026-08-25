//! Effects-typed signature checker substrate consumer (P-PRIM-13).
//!
//! Wires `vyre_primitives::effects::type_checker::check_effect_row`
//! into the dispatch path so the optimizer / backends can validate
//! that a Program's observed effect row fits inside a caller-declared
//! effect signature before lowering. Same primitive downstream analyzer / external
//! dialects can use to type-check user-authored regions.
//!
//! The recursion thesis: `vyre_primitives::effects` ships the
//! type-row primitive to user dialects; this consumer exercises it
//! on every dispatch.

use vyre_primitives::effects::{
    check_effect_row as primitive_check, fits_signature as primitive_fits, EffectRow,
    EffectTypeError,
};

/// Verify that `observed` fits inside `signature`. Bumps the
/// dataflow-fixpoint substrate counter so every dispatch-time
/// signature check is visible in observability dashboards.
///
/// # Errors
///
/// Returns [`EffectTypeError`] when the observed row produces an
/// effect not permitted by the signature. The error's
/// `unpermitted` field is the offending bitmask.
pub fn check_signature(signature: EffectRow, observed: EffectRow) -> Result<(), EffectTypeError> {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_check(signature, observed)
}

/// Convenience: bool form for substrate paths that don't need the
/// failure detail.
#[must_use]
pub fn signature_fits(signature: EffectRow, observed: EffectRow) -> bool {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_fits(signature, observed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::effects::EffectKind;

    fn row_of(kinds: &[EffectKind]) -> EffectRow {
        let mut r = EffectRow::empty();
        for &k in kinds {
            r = r.union(EffectRow::single(k));
        }
        r
    }

    #[test]
    fn empty_observed_fits_anything() {
        let sig = row_of(&[EffectKind::BufferWrite, EffectKind::HostIo]);
        assert!(check_signature(sig, EffectRow::empty()).is_ok());
        assert!(signature_fits(sig, EffectRow::empty()));
    }

    #[test]
    fn excess_effect_returns_unpermitted_bit() {
        let sig = EffectRow::single(EffectKind::BufferWrite);
        let observed = row_of(&[EffectKind::BufferWrite, EffectKind::Trap]);
        let err = check_signature(sig, observed).unwrap_err();
        assert_eq!(err.unpermitted, EffectRow::single(EffectKind::Trap));
    }

    /// Closure-bar: substrate output equals primitive output exactly.
    #[test]
    fn matches_primitive_directly() {
        let sig = row_of(&[EffectKind::BufferWrite, EffectKind::Atomic]);
        let observed = row_of(&[EffectKind::BufferWrite, EffectKind::GpuDispatch]);
        let via_substrate = check_signature(sig, observed);
        let via_primitive = primitive_check(sig, observed);
        assert_eq!(via_substrate, via_primitive);
    }

    /// Adversarial: signature with every effect must accept any
    /// observed row.
    #[test]
    fn full_signature_accepts_anything() {
        let full = row_of(&[
            EffectKind::BufferWrite,
            EffectKind::Atomic,
            EffectKind::HostIo,
            EffectKind::GpuDispatch,
            EffectKind::Barrier,
            EffectKind::AsyncLoad,
            EffectKind::Trap,
        ]);
        assert!(signature_fits(full, EffectRow::empty()));
        assert!(signature_fits(full, EffectRow::single(EffectKind::Trap)));
        assert!(signature_fits(full, full));
    }

    /// Adversarial: signature_fits must agree with check_signature
    /// on the same inputs.
    #[test]
    fn fits_and_check_agree() {
        let sig = EffectRow::single(EffectKind::BufferWrite);
        let cases = [
            EffectRow::empty(),
            EffectRow::single(EffectKind::BufferWrite),
            EffectRow::single(EffectKind::Atomic),
            row_of(&[EffectKind::BufferWrite, EffectKind::Atomic]),
        ];
        for &observed in &cases {
            assert_eq!(
                signature_fits(sig, observed),
                check_signature(sig, observed).is_ok(),
                "disagreement on observed = {:?}",
                observed
            );
        }
    }
}

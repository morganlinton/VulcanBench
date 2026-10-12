//! Effects-typed type-checker primitive (P-PRIM-13).
//!
//! Given a declared signature row (the maximum set of effects the
//! caller permits a Region to produce) and an observed row (the
//! actual effects produced), the type-checker primitive answers
//! "does the observed row fit inside the declared signature?".
//!
//! In effect-system terms: a Region with row R is well-typed against
//! signature S iff every bit set in R is also set in S. The primitive
//! returns the unhandled-effect bitmask (R & !S)  -  empty means
//! well-typed; non-empty pinpoints the violating effects.
//!
//! This is the pure-substrate layer the optimizer's `validate` pass
//! consumes when checking that a Program's overall ProgramEffects
//! bitmask fits inside the user-declared effect signature on the
//! Program's entry Region.

use super::handler_apply::EffectRow;

/// Verdict returned by [`check_effect_row`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectTypeError {
    /// Effects the observed row produces that the signature does not
    /// permit. Zero means the row was well-typed.
    pub unpermitted: EffectRow,
}

/// Check whether `observed` fits inside `signature`.
///
/// Returns `Ok(())` when every observed effect is permitted by the
/// signature, otherwise returns the offending bitmask in
/// [`EffectTypeError::unpermitted`]. The check is `(observed & !signature)`
///  -  a single u32 op, lock-free.
///
/// # Errors
///
/// Returns [`EffectTypeError`] when the observed row contains effects
/// not permitted by the signature.
pub const fn check_effect_row(
    signature: EffectRow,
    observed: EffectRow,
) -> Result<(), EffectTypeError> {
    let unpermitted = observed.bits() & !signature.bits();
    if unpermitted == 0 {
        Ok(())
    } else {
        Err(EffectTypeError {
            unpermitted: EffectRow::from_bits(unpermitted),
        })
    }
}

/// Convenience: returns true iff `observed` is well-typed against
/// `signature`. Equivalent to `check_effect_row(...).is_ok()`.
#[must_use]
pub const fn fits_signature(signature: EffectRow, observed: EffectRow) -> bool {
    (observed.bits() & !signature.bits()) == 0
}

#[cfg(test)]
mod tests {
    use super::super::handler_apply::EffectKind;
    use super::*;

    #[test]
    fn empty_observed_always_fits() {
        let sig = EffectRow::empty();
        let observed = EffectRow::empty();
        assert!(check_effect_row(sig, observed).is_ok());
        assert!(fits_signature(sig, observed));
    }

    fn row_of(kinds: &[EffectKind]) -> EffectRow {
        let mut r = EffectRow::empty();
        for &k in kinds {
            r = r.union(EffectRow::single(k));
        }
        r
    }

    #[test]
    fn subset_fits() {
        // signature permits BufferWrite + Atomic; observed produces
        // BufferWrite only.
        let sig = row_of(&[EffectKind::BufferWrite, EffectKind::Atomic]);
        let observed = EffectRow::single(EffectKind::BufferWrite);
        assert_eq!(check_effect_row(sig, observed), Ok(()));
    }

    #[test]
    fn equal_rows_fit() {
        let row = row_of(&[EffectKind::HostIo, EffectKind::GpuDispatch]);
        assert!(fits_signature(row, row));
    }

    /// Closure-bar: when the observed row introduces an effect not
    /// in the signature, the unpermitted bitmask must contain
    /// exactly that effect.
    #[test]
    fn excess_effect_pinpointed_in_unpermitted() {
        // signature permits BufferWrite only.
        let sig = EffectRow::single(EffectKind::BufferWrite);
        // observed produces BufferWrite + Atomic.
        let observed = row_of(&[EffectKind::BufferWrite, EffectKind::Atomic]);
        let err = check_effect_row(sig, observed).unwrap_err();
        // unpermitted must equal the Atomic bit only.
        assert_eq!(err.unpermitted, EffectRow::single(EffectKind::Atomic));
    }

    /// Adversarial: when observed has every effect, signature with
    /// none yields unpermitted == observed.
    #[test]
    fn empty_signature_rejects_everything() {
        let sig = EffectRow::empty();
        let observed = row_of(&[
            EffectKind::BufferWrite,
            EffectKind::Atomic,
            EffectKind::HostIo,
            EffectKind::GpuDispatch,
            EffectKind::Barrier,
            EffectKind::AsyncLoad,
            EffectKind::Trap,
        ]);
        let err = check_effect_row(sig, observed).unwrap_err();
        assert_eq!(err.unpermitted, observed);
    }

    /// Adversarial: signature wider than what observed produces
    /// must still report Ok (signature is the upper bound, not an
    /// exact match).
    #[test]
    fn wider_signature_accepts_narrow_observed() {
        let sig = row_of(&[
            EffectKind::BufferWrite,
            EffectKind::Atomic,
            EffectKind::HostIo,
        ]);
        let observed = EffectRow::empty();
        assert!(fits_signature(sig, observed));
    }

    /// Idempotence: checking the same row against itself always
    /// passes (every bit is permitted by itself).
    #[test]
    fn self_signature_is_identity() {
        for kind in [
            EffectKind::BufferWrite,
            EffectKind::Atomic,
            EffectKind::HostIo,
            EffectKind::GpuDispatch,
            EffectKind::Barrier,
            EffectKind::AsyncLoad,
            EffectKind::Trap,
        ] {
            let row = EffectRow::single(kind);
            assert!(fits_signature(row, row));
        }
    }
}

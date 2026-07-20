//! C frontend workspace error type and phase-transition validators.

use super::CFrontendPhase;
use super::CFrontendRegionId;

/// Error returned by resident C frontend workspace validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CFrontendWorkspaceError {
    /// A required region has zero capacity.
    #[error("{region:?} capacity is zero. Fix: reserve at least one resident record for every C frontend region so the megakernel never falls back to host state")]
    ZeroCapacity {
        /// Region with zero capacity.
        region: CFrontendRegionId,
    },
    /// Region word arithmetic overflowed.
    #[error("{region:?} word layout overflowed. Fix: {fix}")]
    WordOverflow {
        /// Region being sized when arithmetic overflowed.
        region: CFrontendRegionId,
        /// Actionable remediation.
        fix: &'static str,
    },
    /// Total workspace words exceed the ABI cap.
    #[error("C frontend workspace needs {total_words} words, cap is {max_words}. Fix: {fix}")]
    WorkspaceTooLarge {
        /// Requested total words.
        total_words: u32,
        /// Maximum accepted words.
        max_words: u32,
        /// Actionable remediation.
        fix: &'static str,
    },
    /// A requested phase transition is illegal.
    #[error("illegal C frontend phase transition {from:?} -> {to:?}. Fix: parser megakernel phases must advance linearly or transition to Fault with a diagnostic")]
    InvalidPhaseTransition {
        /// Current phase.
        from: CFrontendPhase,
        /// Requested phase.
        to: CFrontendPhase,
    },
}

/// Return true if `from -> to` is accepted by the resident phase machine.
#[must_use]
pub const fn is_valid_c_frontend_phase_transition(
    from: CFrontendPhase,
    to: CFrontendPhase,
) -> bool {
    matches!(to, CFrontendPhase::Fault)
        || matches!(from.next_success(), Some(next) if next.id() == to.id())
        || matches!(
            (from, to),
            (CFrontendPhase::Complete, CFrontendPhase::ResidentReady)
        )
}

/// Validate a resident C frontend phase transition.
///
/// # Errors
///
/// Returns [`CFrontendWorkspaceError::InvalidPhaseTransition`] if the
/// transition skips a successful phase or attempts to leave `Fault`.
pub fn validate_c_frontend_phase_transition(
    from: CFrontendPhase,
    to: CFrontendPhase,
) -> Result<(), CFrontendWorkspaceError> {
    if is_valid_c_frontend_phase_transition(from, to) {
        Ok(())
    } else {
        Err(CFrontendWorkspaceError::InvalidPhaseTransition { from, to })
    }
}

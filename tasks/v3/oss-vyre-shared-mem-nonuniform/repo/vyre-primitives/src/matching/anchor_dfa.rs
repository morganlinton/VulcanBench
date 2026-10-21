//! Anchor-DFA plan for software, SPIR-V, and accelerator experiments.
//!
//! This module owns the shared scan-side representation: extracted anchor
//! literals, the DFA state budget, verifier binding, and fallback diagnostic.
//! It reuses the existing DFA compiler instead of creating a second transition
//! table builder.

use std::error::Error;
use std::fmt;

use crate::matching::dfa_compile::{dfa_compile_with_budget, DfaCompileError};

/// Schema version for anchor-DFA plans.
pub const ANCHOR_DFA_PLAN_SCHEMA_VERSION: u32 = 1;

/// One extracted anchor literal tied to a source pattern id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorDfaLiteral {
    /// Source pattern id that owns this anchor.
    pub pattern_id: u32,
    /// Literal bytes that seed the DFA prefilter.
    pub bytes: Vec<u8>,
}

/// Borrowed candidate used to build an anchor-DFA plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorDfaCandidate<'anchor> {
    /// Source pattern id that owns this anchor.
    pub pattern_id: u32,
    /// Literal bytes that seed the DFA prefilter.
    pub bytes: &'anchor [u8],
}

impl<'anchor> AnchorDfaCandidate<'anchor> {
    /// Construct a borrowed anchor-DFA candidate.
    #[must_use]
    pub const fn new(pattern_id: u32, bytes: &'anchor [u8]) -> Self {
        Self { pattern_id, bytes }
    }
}

/// Shared anchor-DFA plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorDfaPlan {
    /// Plan schema version.
    pub schema_version: u32,
    /// Extracted anchors in source order.
    pub anchors: Vec<AnchorDfaLiteral>,
    /// Caller-supplied DFA state budget.
    pub dfa_state_budget: u32,
    /// States produced by the existing DFA compiler.
    pub dfa_state_count: u32,
    /// Verifier fragment that must confirm candidate matches.
    pub verifier_fragment_id: String,
    /// Diagnostic used when the plan rejects an unsupported offload shape.
    pub fallback_rejection_reason: String,
    /// True when software and offloaded outputs must match exactly.
    pub match_parity_required: bool,
}

impl AnchorDfaPlan {
    /// Return true when the plan carries all scan/offload contract fields.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.schema_version == ANCHOR_DFA_PLAN_SCHEMA_VERSION
            && !self.anchors.is_empty()
            && self.dfa_state_count != 0
            && self.dfa_state_count <= self.dfa_state_budget
            && !self.verifier_fragment_id.is_empty()
            && self.fallback_rejection_reason.is_empty()
            && self.match_parity_required
    }
}

/// Structured anchor-DFA build failure.
#[derive(Debug, Clone)]
pub enum AnchorDfaPlanError {
    /// No anchor candidates were supplied.
    EmptyAnchorSet,
    /// One anchor literal was empty.
    EmptyAnchor {
        /// Pattern id with an empty anchor.
        pattern_id: u32,
    },
    /// Verifier fragment id was missing.
    MissingVerifierFragmentId,
    /// DFA state budget was zero.
    ZeroDfaStateBudget,
    /// DFA byte budget overflowed while translating state budget to compiler budget.
    DfaBudgetOverflow,
    /// Existing DFA compiler rejected the anchors.
    DfaCompile(DfaCompileError),
    /// Staging anchor literals failed.
    ReserveFailed {
        /// Allocation failure detail.
        message: String,
    },
}

impl fmt::Display for AnchorDfaPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAnchorSet => write!(
                formatter,
                "anchor-DFA plan received no anchors. Fix: extract at least one literal anchor or use the non-anchor regex path."
            ),
            Self::EmptyAnchor { pattern_id } => write!(
                formatter,
                "anchor-DFA plan received an empty anchor for pattern {pattern_id}. Fix: route this pattern to verifier-only or NFA execution."
            ),
            Self::MissingVerifierFragmentId => write!(
                formatter,
                "anchor-DFA plan is missing a verifier fragment id. Fix: bind every anchor-DFA plan to the verifier fragment that proves full-match semantics."
            ),
            Self::ZeroDfaStateBudget => write!(
                formatter,
                "anchor-DFA plan received a zero DFA state budget. Fix: configure a positive DFA state budget or reject the offload."
            ),
            Self::DfaBudgetOverflow => write!(
                formatter,
                "anchor-DFA state budget overflowed the DFA compiler byte budget. Fix: lower the state budget or shard anchor groups."
            ),
            Self::DfaCompile(source) => write!(formatter, "{source}"),
            Self::ReserveFailed { message } => write!(
                formatter,
                "anchor-DFA staging allocation failed: {message}. Fix: shard anchor groups before planning."
            ),
        }
    }
}

impl Error for AnchorDfaPlanError {}

/// Build an anchor-DFA plan using the existing DFA compiler.
///
/// # Errors
///
/// Returns [`AnchorDfaPlanError`] when anchors are empty, verifier binding is
/// missing, the state budget cannot be translated into the existing DFA budget,
/// or DFA compilation rejects the anchors.
pub fn build_anchor_dfa_plan(
    anchors: &[AnchorDfaCandidate<'_>],
    dfa_state_budget: u32,
    verifier_fragment_id: &str,
) -> Result<AnchorDfaPlan, AnchorDfaPlanError> {
    if anchors.is_empty() {
        return Err(AnchorDfaPlanError::EmptyAnchorSet);
    }
    if dfa_state_budget == 0 {
        return Err(AnchorDfaPlanError::ZeroDfaStateBudget);
    }
    if verifier_fragment_id.is_empty() {
        return Err(AnchorDfaPlanError::MissingVerifierFragmentId);
    }
    let mut owned = Vec::new();
    owned
        .try_reserve(anchors.len())
        .map_err(|source| AnchorDfaPlanError::ReserveFailed {
            message: source.to_string(),
        })?;
    let mut pattern_slices = Vec::new();
    pattern_slices
        .try_reserve(anchors.len())
        .map_err(|source| AnchorDfaPlanError::ReserveFailed {
            message: source.to_string(),
        })?;
    for anchor in anchors {
        if anchor.bytes.is_empty() {
            return Err(AnchorDfaPlanError::EmptyAnchor {
                pattern_id: anchor.pattern_id,
            });
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve(anchor.bytes.len())
            .map_err(|source| AnchorDfaPlanError::ReserveFailed {
                message: source.to_string(),
            })?;
        bytes.extend_from_slice(anchor.bytes);
        owned.push(AnchorDfaLiteral {
            pattern_id: anchor.pattern_id,
            bytes,
        });
    }
    pattern_slices.extend(owned.iter().map(|anchor| anchor.bytes.as_slice()));
    let state_budget = usize::try_from(dfa_state_budget)
        .map_err(|_| AnchorDfaPlanError::DfaBudgetOverflow)?;
    let dfa_budget_bytes = state_budget
        .checked_mul(1024)
        .ok_or(AnchorDfaPlanError::DfaBudgetOverflow)?;
    let dfa =
        dfa_compile_with_budget(&pattern_slices, dfa_budget_bytes).map_err(AnchorDfaPlanError::DfaCompile)?;
    Ok(AnchorDfaPlan {
        schema_version: ANCHOR_DFA_PLAN_SCHEMA_VERSION,
        anchors: owned,
        dfa_state_budget,
        dfa_state_count: dfa.state_count,
        verifier_fragment_id: verifier_fragment_id.to_string(),
        fallback_rejection_reason: String::new(),
        match_parity_required: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_dfa_plan_records_exact_anchors_state_budget_and_verifier() {
        let candidates = [
            AnchorDfaCandidate::new(7, b"abc"),
            AnchorDfaCandidate::new(9, b"xyz"),
        ];

        let plan = build_anchor_dfa_plan(&candidates, 128, "regex-verifier:v1")
            .expect("Fix: literal anchors should build an anchor-DFA plan");

        assert_eq!(plan.schema_version, ANCHOR_DFA_PLAN_SCHEMA_VERSION);
        assert_eq!(plan.anchors.len(), 2);
        assert_eq!(plan.anchors[0].pattern_id, 7);
        assert_eq!(plan.anchors[0].bytes, b"abc");
        assert_eq!(plan.anchors[1].pattern_id, 9);
        assert_eq!(plan.anchors[1].bytes, b"xyz");
        assert_eq!(plan.dfa_state_budget, 128);
        assert!(plan.dfa_state_count >= 4);
        assert_eq!(plan.verifier_fragment_id, "regex-verifier:v1");
        assert!(plan.fallback_rejection_reason.is_empty());
        assert!(plan.match_parity_required);
        assert!(plan.is_complete());
    }

    #[test]
    fn anchor_dfa_plan_rejects_empty_anchor_with_fix() {
        let error = build_anchor_dfa_plan(
            &[AnchorDfaCandidate::new(3, b"")],
            64,
            "regex-verifier:v1",
        )
        .expect_err("Fix: empty anchors must reject before DFA compilation");

        assert!(matches!(
            error,
            AnchorDfaPlanError::EmptyAnchor { pattern_id: 3 }
        ));
        assert!(
            error.to_string().contains("Fix:"),
            "anchor-DFA errors must carry operator guidance"
        );
    }
}

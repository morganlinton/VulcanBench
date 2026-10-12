//! SPIR-V anchor-DFA offload evidence.
//!
//! Anchor extraction and DFA construction belong to `vyre-primitives`. This
//! module records whether a SPIR-V offload candidate can consume that plan and
//! which verifier work remains necessary for exact match semantics.

use vyre_primitives::matching::{AnchorDfaPlan, ANCHOR_DFA_PLAN_SCHEMA_VERSION};

/// Schema version for SPIR-V anchor-DFA offload evidence.
pub const SPIRV_ANCHOR_DFA_OFFLOAD_SCHEMA_VERSION: u32 = 1;

/// SPIR-V offload evidence for one anchor-DFA plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpirvAnchorDfaOffloadEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Anchor-DFA plan schema version consumed by this offload evidence.
    pub anchor_plan_schema_version: u32,
    /// Number of anchor literals in the plan.
    pub anchor_count: u32,
    /// DFA state budget recorded by the scan-owned plan.
    pub dfa_state_budget: u32,
    /// DFA state count produced by the scan-owned plan.
    pub dfa_state_count: u32,
    /// Verifier fragment that must confirm candidate matches.
    pub verifier_fragment_id: String,
    /// Number of anchor hits reported by the offload candidate.
    pub anchor_hit_count: u64,
    /// Number of verifier calls required after anchor filtering.
    pub verifier_call_count: u64,
    /// True when this plan is eligible for SPIR-V offload.
    pub spirv_offload_candidate: bool,
    /// Unsupported feature diagnostic when the plan is not offloadable.
    pub unsupported_feature_diagnostic: String,
    /// True when software and SPIR-V outputs must match exactly.
    pub match_parity_required: bool,
}

impl SpirvAnchorDfaOffloadEvidence {
    /// Build SPIR-V offload evidence from a scan-owned anchor-DFA plan.
    #[must_use]
    pub fn from_anchor_dfa_plan(
        plan: &AnchorDfaPlan,
        anchor_hit_count: u64,
        verifier_call_count: u64,
        spirv_offload_candidate: bool,
        unsupported_feature_diagnostic: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SPIRV_ANCHOR_DFA_OFFLOAD_SCHEMA_VERSION,
            anchor_plan_schema_version: plan.schema_version,
            anchor_count: u32::try_from(plan.anchors.len()).unwrap_or(u32::MAX),
            dfa_state_budget: plan.dfa_state_budget,
            dfa_state_count: plan.dfa_state_count,
            verifier_fragment_id: plan.verifier_fragment_id.clone(),
            anchor_hit_count,
            verifier_call_count,
            spirv_offload_candidate,
            unsupported_feature_diagnostic: unsupported_feature_diagnostic.into(),
            match_parity_required: plan.match_parity_required,
        }
    }

    /// Return true when evidence proves SPIR-V did not overclaim offloadability.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.schema_version == SPIRV_ANCHOR_DFA_OFFLOAD_SCHEMA_VERSION
            && self.anchor_plan_schema_version == ANCHOR_DFA_PLAN_SCHEMA_VERSION
            && self.anchor_count != 0
            && self.dfa_state_count != 0
            && self.dfa_state_count <= self.dfa_state_budget
            && !self.verifier_fragment_id.is_empty()
            && self.match_parity_required
            && if self.spirv_offload_candidate {
                self.unsupported_feature_diagnostic.is_empty()
            } else {
                self.unsupported_feature_diagnostic.contains("Fix:")
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::matching::{build_anchor_dfa_plan, AnchorDfaCandidate};

    #[test]
    fn spirv_anchor_dfa_evidence_consumes_scan_owned_plan() {
        let plan = build_anchor_dfa_plan(
            &[AnchorDfaCandidate::new(1, b"token")],
            128,
            "regex-verifier:v1",
        )
        .expect("Fix: anchor-DFA plan should build for literal anchor");

        let evidence =
            SpirvAnchorDfaOffloadEvidence::from_anchor_dfa_plan(&plan, 12, 12, true, "");

        assert_eq!(evidence.schema_version, SPIRV_ANCHOR_DFA_OFFLOAD_SCHEMA_VERSION);
        assert_eq!(evidence.anchor_plan_schema_version, ANCHOR_DFA_PLAN_SCHEMA_VERSION);
        assert_eq!(evidence.anchor_count, 1);
        assert_eq!(evidence.dfa_state_budget, 128);
        assert_eq!(evidence.dfa_state_count, plan.dfa_state_count);
        assert_eq!(evidence.verifier_fragment_id, "regex-verifier:v1");
        assert_eq!(evidence.anchor_hit_count, 12);
        assert_eq!(evidence.verifier_call_count, 12);
        assert!(evidence.spirv_offload_candidate);
        assert!(evidence.unsupported_feature_diagnostic.is_empty());
        assert!(evidence.match_parity_required);
        assert!(evidence.is_complete());
    }

    #[test]
    fn spirv_anchor_dfa_evidence_requires_unsupported_diagnostic_for_rejection() {
        let plan = build_anchor_dfa_plan(
            &[AnchorDfaCandidate::new(1, b"token")],
            128,
            "regex-verifier:v1",
        )
        .expect("Fix: anchor-DFA plan should build for literal anchor");

        let evidence = SpirvAnchorDfaOffloadEvidence::from_anchor_dfa_plan(
            &plan,
            0,
            0,
            false,
            "unsupported SPIR-V subgroup width for anchor DFA. Fix: route through software DFA or choose a backend with compatible subgroup support.",
        );

        assert!(!evidence.spirv_offload_candidate);
        assert!(evidence.unsupported_feature_diagnostic.contains("Fix:"));
        assert!(evidence.is_complete());
    }
}

//! Shared research-to-plan coverage checks for VX rows.
//!
//! This is intentionally independent of the plan gate and research-audit
//! report models. Both callers map their row type into this small contract so
//! row-to-source, row-to-local-evidence, row-to-proof, and row-to-seam coverage
//! cannot drift.

use crate::research_key::backtick_research_keys;

/// Row view needed by research-plan coverage.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResearchPlanCoverageRow<'a> {
    pub(crate) line: usize,
    pub(crate) id: &'a str,
    pub(crate) local_evidence: &'a str,
    pub(crate) research_basis: &'a str,
    pub(crate) proof_gate: &'a str,
    pub(crate) dedup_seam: &'a str,
}

/// One research-plan coverage finding.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ResearchPlanCoverageFinding {
    pub(crate) path: String,
    pub(crate) key: String,
    pub(crate) text: String,
    pub(crate) policy: String,
}

/// Validate VX rows have source, local-evidence, proof-gate, and seam coverage.
pub(crate) fn research_plan_coverage_findings(
    plan_path: &str,
    rows: &[ResearchPlanCoverageRow<'_>],
) -> Vec<ResearchPlanCoverageFinding> {
    let mut findings = Vec::new();
    for row in rows {
        if !has_research_source(row.research_basis) {
            findings.push(finding(
                plan_path,
                row,
                "VX row lacks a backtick research key, local source path, or explicit Internal Vyre evidence contract",
                "research-plan-row-source",
            ));
        }
        if !has_rooted_local_evidence(row.local_evidence) {
            findings.push(finding(
                plan_path,
                row,
                "VX row lacks rooted local evidence path or explicit active-plan evidence",
                "research-plan-row-local-evidence",
            ));
        }
        if !has_concrete_proof_gate(row.proof_gate) {
            findings.push(finding(
                plan_path,
                row,
                "VX row proof gate does not name a concrete rejecting/asserting/auditing evidence mechanism",
                "research-plan-row-proof-gate",
            ));
        }
        if !has_concrete_seam(row.dedup_seam) {
            findings.push(finding(
                plan_path,
                row,
                "VX row dedup seam does not name one shared owner, boundary, schema, helper, primitive, or validator",
                "research-plan-row-dedup-seam",
            ));
        }
    }
    findings
}

fn has_research_source(research_basis: &str) -> bool {
    !backtick_research_keys(research_basis).is_empty()
        || research_basis.contains("Internal Vyre")
        || backtick_tokens(research_basis)
            .iter()
            .any(|token| looks_like_local_source(token))
}

fn has_rooted_local_evidence(local_evidence: &str) -> bool {
    local_evidence.starts_with("This file")
        || backtick_tokens(local_evidence)
            .iter()
            .any(|token| looks_like_local_source(token))
}

fn has_concrete_proof_gate(proof_gate: &str) -> bool {
    let lower = proof_gate.to_ascii_lowercase();
    [
        "assert",
        "audit",
        "bench",
        "block",
        "evidence",
        "fail",
        "gate",
        "reject",
        "test",
        "validate",
        "validator",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn has_concrete_seam(dedup_seam: &str) -> bool {
    let lower = dedup_seam.to_ascii_lowercase();
    [
        "one ",
        "boundary",
        "contract",
        "dedup",
        "feeds",
        "helper",
        "owns",
        "primitive",
        "schema",
        "shared",
        "source",
        "validator",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn looks_like_local_source(token: &str) -> bool {
    token == "Cargo.toml"
        || token.contains('/')
        || token.ends_with(".rs")
        || token.ends_with(".md")
        || token.ends_with(".toml")
}

fn backtick_tokens(text: &str) -> Vec<String> {
    text.split('`')
        .enumerate()
        .filter_map(|(index, token)| {
            if index % 2 == 1 && !token.trim().is_empty() {
                Some(token.trim().to_string())
            } else {
                None
            }
        })
        .collect()
}

fn finding(
    plan_path: &str,
    row: &ResearchPlanCoverageRow<'_>,
    text: impl Into<String>,
    policy: impl Into<String>,
) -> ResearchPlanCoverageFinding {
    ResearchPlanCoverageFinding {
        path: plan_path.to_string(),
        key: row.id.to_string(),
        text: format!("line {}: {}", row.line, text.into()),
        policy: policy.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_rejects_plan_rows_without_source_proof_or_seam() {
        let row = ResearchPlanCoverageRow {
            line: 42,
            id: "VX-999",
            local_evidence: "Plan prose only.",
            research_basis: "generic paper",
            proof_gate: "manual note",
            dedup_seam: "new thing",
        };

        let findings = research_plan_coverage_findings("docs/optimization/PLAN.md", &[row]);

        assert!(findings
            .iter()
            .any(|finding| finding.policy == "research-plan-row-source"));
        assert!(findings
            .iter()
            .any(|finding| finding.policy == "research-plan-row-local-evidence"));
        assert!(findings
            .iter()
            .any(|finding| finding.policy == "research-plan-row-proof-gate"));
        assert!(findings
            .iter()
            .any(|finding| finding.policy == "research-plan-row-dedup-seam"));
    }

    #[test]
    fn coverage_accepts_external_source_and_shared_seam() {
        let row = ResearchPlanCoverageRow {
            line: 9,
            id: "VX-009",
            local_evidence: "`xtask/src/example.rs` owns the evidence path.",
            research_basis: "`MLIR_PASS`",
            proof_gate: "Gate test rejects malformed rows.",
            dedup_seam: "One shared validator feeds the command.",
        };

        let findings = research_plan_coverage_findings("docs/optimization/PLAN.md", &[row]);

        assert_eq!(findings, Vec::new());
    }
}

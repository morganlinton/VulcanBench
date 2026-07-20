use std::collections::BTreeMap;

use serde::Serialize;

use crate::hash::sha256_hex;

pub(crate) use crate::artifact_paths::RESEARCH_AUDIT_ARTIFACT;

pub(super) const PLAN_PATH: &str = "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md";
pub(super) const COMMAND_MATRIX_PATH: &str = "docs/optimization/XTASK_COMMAND_MATRIX.md";
pub(super) const LEGACY_DOCS_PATH: &str = "docs/optimization/LEGACY_DOCS.md";
pub(super) const SCHEMA_VERSION: u32 = 6;
pub(super) const SOURCE_DIGEST_PREFIX: &str = "research-audit-source:v6:";
const SOURCE_DIGEST_MATERIAL_LABEL: &str = "research-audit:v6";
pub(super) const MIN_PLAN_ROWS: usize = crate::vx_plan_table::VX_PLAN_MIN_ROWS;
pub(super) const MAX_FINDINGS: usize = 100;
pub(super) const MAX_HOTSPOTS: usize = 40;
pub(super) const RAW_COUNTER_FAMILIES: &[&str] = &[
    "loc_hotspots",
    "claim_drift",
    "baseline_gaps",
    "innovation_coverage",
    "high_risk_vx_linkage",
    "stale_doc_markers",
    "repo_boundary_findings",
    "megakernel_protocol_boundary_findings",
    "script_policy_findings",
    "rust_toml_loader_findings",
    "source_ledger_findings",
    "competitor_issue_findings",
    "research_plan_coverage_findings",
    "archive_replay_findings",
    "rules_as_data_findings",
];

pub(super) const CLAIM_MARKERS: &[&str] = &[
    "100x",
    "cuda",
    "gpu",
    "wgpu",
    "metal",
    "hyperscan",
    "tree-sitter",
    "codeql",
    "cugraph",
    "graphblas",
    "faster",
    "speedup",
    "baseline",
];

pub(super) const BASELINE_MARKERS: &[&str] = &[
    "against",
    "baseline",
    "bench",
    "benchmark",
    "compare",
    "differential",
    "parity",
    "throughput",
];

pub(super) const NEGATIVE_CASE_MARKERS: &[&str] = &[
    "adversarial",
    "diagnostic",
    "digest",
    "drift",
    "exact",
    "fail",
    "identical",
    "negative",
    "parity",
    "reject",
    "unsupported",
    "witness",
];

#[derive(Debug, Clone, Serialize)]
pub(super) struct ResearchAuditReport {
    pub(super) schema_version: u32,
    pub(super) generator_command: String,
    pub(super) plan_path: &'static str,
    pub(super) command_matrix_path: &'static str,
    pub(super) plan_row_count: usize,
    pub(super) minimum_plan_row_count: usize,
    pub(super) axis_count: usize,
    pub(super) defined_research_key_count: usize,
    pub(super) used_research_key_count: usize,
    pub(super) raw_counter_families: Vec<String>,
    pub(super) loc_hotspots: Vec<LocHotspot>,
    pub(super) claim_drift: Vec<ClaimDriftFinding>,
    pub(super) baseline_gaps: Vec<BaselineGap>,
    pub(super) innovation_coverage: Vec<InnovationCoverage>,
    pub(super) high_risk_vx_linkage: Vec<VxLinkageFinding>,
    pub(super) stale_doc_markers: Vec<DocMarkerFinding>,
    pub(super) repo_boundary_findings: Vec<RepoBoundaryFinding>,
    pub(super) megakernel_protocol_boundary_findings: Vec<SourceLedgerFinding>,
    pub(super) script_policy_findings: Vec<ScriptPolicyFinding>,
    pub(super) rust_toml_loader_findings: Vec<RustTomlLoaderFinding>,
    pub(super) source_ledger_findings: Vec<SourceLedgerFinding>,
    pub(super) competitor_issue_findings: Vec<SourceLedgerFinding>,
    pub(super) research_plan_coverage_findings: Vec<SourceLedgerFinding>,
    pub(super) archive_replay_findings: Vec<ArchiveReplayFinding>,
    pub(super) rules_as_data_findings: Vec<SourceLedgerFinding>,
    pub(super) blockers: Vec<String>,
    pub(super) source_digest: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct LocHotspot {
    pub(super) path: String,
    pub(super) loc: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ClaimDriftFinding {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) marker: String,
    pub(super) text: String,
    pub(super) evidence_hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct BaselineGap {
    pub(super) vx_id: String,
    pub(super) axis: String,
    pub(super) research_basis: String,
    pub(super) proof_gate: String,
    pub(super) trigger: String,
    pub(super) required_evidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct InnovationCoverage {
    pub(super) vx_id: String,
    pub(super) axis: String,
    pub(super) research_keys: Vec<String>,
    pub(super) has_named_external_source: bool,
    pub(super) owner_lane: String,
    pub(super) baseline_type: String,
    pub(super) workload_family: String,
    pub(super) has_local_path_evidence: bool,
    pub(super) negative_case_family: String,
    pub(super) gpu_claim_policy: String,
    pub(super) has_gpu_partition_rationale: bool,
    pub(super) has_transfer_accounting: bool,
    pub(super) has_baseline_field: bool,
    pub(super) grounding_severity: String,
    pub(super) missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct VxLinkageFinding {
    pub(super) command: String,
    pub(super) source_file: String,
    pub(super) duplicate_risk_score: u32,
    pub(super) covered_by_vx_row: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct DocMarkerFinding {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) marker: String,
    pub(super) text: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RepoBoundaryFinding {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) text: String,
    pub(super) boundary: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ScriptPolicyFinding {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) text: String,
    pub(super) policy: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RustTomlLoaderFinding {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) text: String,
    pub(super) policy: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SourceLedgerFinding {
    pub(super) path: String,
    pub(super) key: String,
    pub(super) text: String,
    pub(super) policy: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ArchiveReplayFinding {
    pub(super) audit_path: String,
    pub(super) line: usize,
    pub(super) archived_reference: String,
    pub(super) current_lookup: String,
    pub(super) replay_fixture_id: String,
    pub(super) blocker_status: String,
    pub(super) stale_reason: String,
}

#[derive(Debug, Clone)]
pub(super) struct VxRow {
    pub(super) line: usize,
    pub(super) id: String,
    pub(super) axis: String,
    pub(super) local_evidence: String,
    pub(super) research_basis: String,
    pub(super) work: String,
    pub(super) proof_gate: String,
    pub(super) dedup_seam: String,
}

pub(super) fn source_digest(
    plan: &str,
    command_matrix: &str,
    row_count: usize,
    minimum_plan_row_count: usize,
    hotspot_count: usize,
    claim_drift_count: usize,
    baseline_gap_count: usize,
    innovation_coverage_count: usize,
    high_risk_vx_linkage_count: usize,
    stale_doc_marker_count: usize,
    repo_boundary_finding_count: usize,
    megakernel_protocol_boundary_finding_count: usize,
    script_policy_finding_count: usize,
    rust_toml_loader_finding_count: usize,
    source_ledger_finding_count: usize,
    competitor_issue_finding_count: usize,
    research_plan_coverage_finding_count: usize,
    archive_replay_finding_count: usize,
    rules_as_data_finding_count: usize,
    finding_material: &str,
) -> String {
    let mut counts = BTreeMap::new();
    counts.insert("row_count", row_count);
    counts.insert("minimum_plan_row_count", minimum_plan_row_count);
    counts.insert("hotspot_count", hotspot_count);
    counts.insert("claim_drift_count", claim_drift_count);
    counts.insert("baseline_gap_count", baseline_gap_count);
    counts.insert("innovation_coverage_count", innovation_coverage_count);
    counts.insert("high_risk_vx_linkage_count", high_risk_vx_linkage_count);
    counts.insert("stale_doc_marker_count", stale_doc_marker_count);
    counts.insert("repo_boundary_finding_count", repo_boundary_finding_count);
    counts.insert(
        "megakernel_protocol_boundary_finding_count",
        megakernel_protocol_boundary_finding_count,
    );
    counts.insert("script_policy_finding_count", script_policy_finding_count);
    counts.insert(
        "rust_toml_loader_finding_count",
        rust_toml_loader_finding_count,
    );
    counts.insert("source_ledger_finding_count", source_ledger_finding_count);
    counts.insert(
        "competitor_issue_finding_count",
        competitor_issue_finding_count,
    );
    counts.insert(
        "research_plan_coverage_finding_count",
        research_plan_coverage_finding_count,
    );
    counts.insert(
        "archive_replay_finding_count",
        archive_replay_finding_count,
    );
    counts.insert(
        "rules_as_data_finding_count",
        rules_as_data_finding_count,
    );
    let material = format!(
        "{SOURCE_DIGEST_MATERIAL_LABEL}\nplan={}\ncommand_matrix={}\ncounts={counts:?}\nfinding_material={}\n",
        sha256_hex(plan.as_bytes()),
        sha256_hex(command_matrix.as_bytes()),
        sha256_hex(finding_material.as_bytes())
    );
    format!("{SOURCE_DIGEST_PREFIX}{}", sha256_hex(material.as_bytes()))
}

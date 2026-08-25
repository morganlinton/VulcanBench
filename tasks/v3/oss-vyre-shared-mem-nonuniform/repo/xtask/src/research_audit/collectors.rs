use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::model::{
    ArchiveReplayFinding, BaselineGap, ClaimDriftFinding, DocMarkerFinding, InnovationCoverage,
    LocHotspot, RepoBoundaryFinding, RustTomlLoaderFinding, ScriptPolicyFinding,
    SourceLedgerFinding, VxLinkageFinding, VxRow, BASELINE_MARKERS, CLAIM_MARKERS,
    COMMAND_MATRIX_PATH, LEGACY_DOCS_PATH, MAX_FINDINGS, MAX_HOTSPOTS, MIN_PLAN_ROWS, PLAN_PATH,
};
use crate::markdown_table::{markdown_cells, trim_code_ticks};
use crate::repo_boundary;

const DRIVER_MEGAKERNEL_PROTOCOL_SCAN_ROOTS: &[&str] = &[
    "vyre-driver/src",
    "vyre-driver-cuda/src",
    "vyre-driver-metal/src",
    "vyre-driver-wgpu/src",
];
const ARCHIVE_REPLAY_AUDITS_TOML_PATH: &str = "docs/optimization/ARCHIVE_REPLAY_AUDITS.toml";
const ARCHIVE_REPLAY_AUDITS_TOML: &str =
    include_str!("../../../docs/optimization/ARCHIVE_REPLAY_AUDITS.toml");
const ARCHIVE_REPLAY_AUDITS_SCHEMA_VERSION: u32 = 1;
const ARCHIVE_REPLAY_AUDITS_CONTRACT: &str = "vyre-archive-replay-audits:v1";

static ARCHIVE_REPLAY_AUDITS: std::sync::OnceLock<Result<ArchiveReplayAuditConfig, String>> =
    std::sync::OnceLock::new();

#[derive(Debug, serde::Deserialize)]
struct ArchiveReplayAuditConfig {
    schema_version: u32,
    contract: String,
    audit: Vec<ArchiveReplayAuditSource>,
}

#[derive(Debug, serde::Deserialize)]
struct ArchiveReplayAuditSource {
    path: String,
    fixture_prefix: String,
}

pub(super) fn collect_loc_hotspots(root: &Path) -> Vec<LocHotspot> {
    let mut hotspots = Vec::new();
    collect_loc_hotspots_in(root, root, &mut hotspots);
    hotspots.sort_by(|left, right| {
        right
            .loc
            .cmp(&left.loc)
            .then_with(|| left.path.cmp(&right.path))
    });
    hotspots.truncate(MAX_HOTSPOTS);
    hotspots
}

pub(super) fn collect_claim_drift(root: &Path) -> Vec<ClaimDriftFinding> {
    let mut findings = Vec::new();
    collect_markdown_claims(root, &root.join("README.md"), &mut findings);
    collect_markdown_claims(root, &root.join("docs"), &mut findings);
    findings.truncate(MAX_FINDINGS);
    findings
}

pub(super) fn collect_baseline_gaps(rows: &[VxRow]) -> Vec<BaselineGap> {
    let mut gaps = Vec::new();
    for row in rows {
        let lower_proof = row.proof_gate.to_ascii_lowercase();
        let lower_work = row.work.to_ascii_lowercase();
        let has_baseline_marker = BASELINE_MARKERS
            .iter()
            .any(|marker| lower_proof.contains(*marker));
        let innovation = row.work.starts_with("Innovation candidate:");
        let comparison_work = BASELINE_MARKERS
            .iter()
            .any(|marker| lower_work.contains(*marker));
        if (innovation || comparison_work) && !has_baseline_marker {
            let trigger = if innovation {
                "innovation-candidate"
            } else {
                "explicit-comparison-work"
            };
            gaps.push(BaselineGap {
                vx_id: row.id.clone(),
                axis: row.axis.clone(),
                research_basis: row.research_basis.clone(),
                proof_gate: row.proof_gate.clone(),
                trigger: trigger.to_string(),
                required_evidence:
                    "proof gate should name benchmark, parity, differential, or baseline evidence"
                        .to_string(),
            });
        }
        if gaps.len() >= MAX_FINDINGS {
            break;
        }
    }
    gaps
}

pub(super) fn collect_vx_linkage(command_matrix: &str, plan: &str) -> Vec<VxLinkageFinding> {
    let mut findings = Vec::new();
    for line in command_matrix.lines() {
        if !line.trim_start().starts_with("| `") {
            continue;
        }
        let cells = markdown_cells(line);
        if cells.len() != 9 {
            continue;
        }
        let command = trim_code_ticks(&cells[0]).to_string();
        let source_file = trim_code_ticks(&cells[7]).to_string();
        let duplicate_risk_score = cells[6].parse::<u32>().unwrap_or_default();
        if duplicate_risk_score > 40 {
            findings.push(VxLinkageFinding {
                command,
                covered_by_vx_row: plan.contains(&source_file),
                source_file,
                duplicate_risk_score,
            });
        }
    }
    findings
}

pub(super) fn collect_stale_doc_markers(root: &Path) -> Vec<DocMarkerFinding> {
    let mut findings = Vec::new();
    collect_stale_doc_markers_in(root, &root.join("docs"), &mut findings);
    findings.truncate(MAX_FINDINGS);
    findings
}

pub(super) fn collect_repo_boundary_findings(root: &Path) -> Vec<RepoBoundaryFinding> {
    let mut findings = Vec::new();
    collect_repo_boundary_findings_in(root, &root.join("scripts"), &mut findings);
    findings
}

pub(super) fn collect_rust_toml_loader_findings(root: &Path) -> Vec<RustTomlLoaderFinding> {
    let mut findings = Vec::new();
    collect_rust_toml_loader_findings_in(root, &root.join("xtask/src"), &mut findings);
    findings.truncate(MAX_FINDINGS);
    findings
}

pub(super) fn collect_megakernel_protocol_boundary_findings(
    root: &Path,
) -> Vec<SourceLedgerFinding> {
    let mut findings = Vec::new();
    for scan_root in DRIVER_MEGAKERNEL_PROTOCOL_SCAN_ROOTS {
        collect_megakernel_protocol_boundary_findings_in(
            root,
            &root.join(scan_root),
            &mut findings,
        );
        if findings.len() >= MAX_FINDINGS {
            break;
        }
    }
    findings
}

pub(super) fn collect_archive_replay_findings(root: &Path) -> Vec<ArchiveReplayFinding> {
    let config = match archive_replay_audit_config() {
        Ok(config) => config,
        Err(error) => {
            return vec![ArchiveReplayFinding {
                audit_path: ARCHIVE_REPLAY_AUDITS_TOML_PATH.to_string(),
                line: 0,
                archived_reference: ARCHIVE_REPLAY_AUDITS_TOML_PATH.to_string(),
                current_lookup: "archive-replay-config-invalid".to_string(),
                replay_fixture_id: "archive-replay-config".to_string(),
                blocker_status: "stale".to_string(),
                stale_reason: error,
            }];
        }
    };
    let mut findings = Vec::new();
    for audit in &config.audit {
        let audit_path = audit.path.trim();
        if audit_path.is_empty() {
            findings.push(ArchiveReplayFinding {
                audit_path: ARCHIVE_REPLAY_AUDITS_TOML_PATH.to_string(),
                line: 0,
                archived_reference: "<empty audit path>".to_string(),
                current_lookup: "archive-audit-path-empty".to_string(),
                replay_fixture_id: archive_replay_fixture_id(&audit.fixture_prefix, 0, audit_path),
                blocker_status: "stale".to_string(),
                stale_reason: "archive replay audit entry has an empty path".to_string(),
            });
            continue;
        }
        let Ok(text) = fs::read_to_string(root.join(audit_path)) else {
            findings.push(ArchiveReplayFinding {
                audit_path: audit_path.to_string(),
                line: 0,
                archived_reference: audit_path.to_string(),
                current_lookup: "archive-audit-file-missing".to_string(),
                replay_fixture_id: archive_replay_fixture_id(&audit.fixture_prefix, 0, audit_path),
                blocker_status: "stale".to_string(),
                stale_reason: "configured archive audit file no longer exists".to_string(),
            });
            continue;
        };
        for (line_index, line) in text.lines().enumerate() {
            for archived_reference in archive_replay_references(line) {
                let current_lookup = if root.join(&archived_reference).exists() {
                    "file-present;symbol-replay-required"
                } else {
                    "missing-file"
                };
                let blocker_status = if current_lookup == "missing-file" {
                    "stale"
                } else {
                    "replay-required"
                };
                let stale_reason = if blocker_status == "stale" {
                    "archived reference no longer resolves in the current tree"
                } else {
                    "archived reference resolves to a current file but needs symbol-level replay before import"
                };
                findings.push(ArchiveReplayFinding {
                    audit_path: audit_path.to_string(),
                    line: line_index + 1,
                    archived_reference: archived_reference.clone(),
                    current_lookup: current_lookup.to_string(),
                    replay_fixture_id: archive_replay_fixture_id(
                        &audit.fixture_prefix,
                        line_index + 1,
                        &archived_reference,
                    ),
                    blocker_status: blocker_status.to_string(),
                    stale_reason: stale_reason.to_string(),
                });
                if findings.len() >= MAX_FINDINGS {
                    return findings;
                }
            }
        }
    }
    findings
}

fn archive_replay_audit_config() -> Result<&'static ArchiveReplayAuditConfig, String> {
    let parsed = ARCHIVE_REPLAY_AUDITS.get_or_init(|| {
        let config = crate::toml_config::parse_embedded_toml::<ArchiveReplayAuditConfig>(
            ARCHIVE_REPLAY_AUDITS_TOML_PATH,
            ARCHIVE_REPLAY_AUDITS_TOML,
        )?;
        if config.schema_version != ARCHIVE_REPLAY_AUDITS_SCHEMA_VERSION {
            return Err(format!(
                "Fix: {ARCHIVE_REPLAY_AUDITS_TOML_PATH} schema_version must be {ARCHIVE_REPLAY_AUDITS_SCHEMA_VERSION}"
            ));
        }
        if config.contract != ARCHIVE_REPLAY_AUDITS_CONTRACT {
            return Err(format!(
                "Fix: {ARCHIVE_REPLAY_AUDITS_TOML_PATH} contract must be {ARCHIVE_REPLAY_AUDITS_CONTRACT}"
            ));
        }
        if config.audit.is_empty() {
            return Err(format!(
                "Fix: {ARCHIVE_REPLAY_AUDITS_TOML_PATH} must declare at least one audit source"
            ));
        }
        Ok(config)
    });
    parsed.as_ref().map_err(Clone::clone)
}

fn archive_replay_references(line: &str) -> Vec<String> {
    line.split('`')
        .enumerate()
        .filter_map(|(index, raw)| {
            if index % 2 == 0 {
                None
            } else {
                archive_replay_path_reference(raw)
            }
        })
        .collect()
}

fn archive_replay_path_reference(raw: &str) -> Option<String> {
    let candidate = raw
        .trim()
        .trim_matches(|ch: char| matches!(ch, ',' | ';' | ':' | ')' | '(' | '[' | ']'));
    if candidate.is_empty()
        || candidate.starts_with("http://")
        || candidate.starts_with("https://")
        || candidate.contains(' ')
        || candidate.contains("::")
        || candidate.starts_with("VX-")
    {
        return None;
    }
    let path = Path::new(candidate);
    if path.extension().is_none() && !candidate.contains('/') {
        return None;
    }
    Some(candidate.to_string())
}

fn archive_replay_fixture_id(prefix: &str, line: usize, archived_reference: &str) -> String {
    let mut normalized = archived_reference
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while normalized.contains("--") {
        normalized = normalized.replace("--", "-");
    }
    format!(
        "archive-replay:{}:{}:{}",
        prefix.trim(),
        line,
        normalized.trim_matches('-')
    )
}

pub(super) fn collect_blockers(
    rows: &[VxRow],
    defined_research_keys: &BTreeSet<String>,
    used_research_keys: &BTreeSet<String>,
    high_risk_vx_linkage: &[VxLinkageFinding],
    repo_boundary_findings: &[RepoBoundaryFinding],
    megakernel_protocol_boundary_findings: &[SourceLedgerFinding],
    script_policy_findings: &[ScriptPolicyFinding],
    rust_toml_loader_findings: &[RustTomlLoaderFinding],
    source_ledger_findings: &[SourceLedgerFinding],
    competitor_issue_findings: &[SourceLedgerFinding],
    research_plan_coverage_findings: &[SourceLedgerFinding],
    archive_replay_findings: &[ArchiveReplayFinding],
    rules_as_data_findings: &[SourceLedgerFinding],
    baseline_gaps: &[BaselineGap],
    innovation_coverage: &[InnovationCoverage],
) -> Vec<String> {
    let mut blockers = Vec::new();
    if rows.len() < MIN_PLAN_ROWS {
        blockers.push(format!(
            "{PLAN_PATH} has {} VX row(s); expected at least {MIN_PLAN_ROWS}",
            rows.len()
        ));
    }
    if !rows.iter().any(|row| row.id == "VX-300") {
        blockers.push(format!("{PLAN_PATH} is missing VX-300 research audit coverage"));
    }
    if !rows.iter().any(|row| row.id == "VX-360") {
        blockers.push(format!("{PLAN_PATH} is missing VX-360 plan quality coverage"));
    }
    if !rows.iter().any(|row| row.id == "VX-420") {
        blockers.push(format!("{PLAN_PATH} is missing VX-420 frontier leaderboard coverage"));
    }
    if defined_research_keys.is_empty() {
        blockers.push(format!("{PLAN_PATH} defines no external research keys"));
    }
    for used in used_research_keys {
        if !defined_research_keys.contains(used) {
            blockers.push(format!("research key `{used}` is used by a VX row but not defined"));
        }
    }
    for finding in high_risk_vx_linkage {
        if !finding.covered_by_vx_row {
            blockers.push(format!(
                "high-risk command `{}` source `{}` is not cited by a VX row",
                finding.command, finding.source_file
            ));
        }
    }
    for gap in baseline_gaps {
        blockers.push(format!(
            "baseline gap in `{}` on axis `{}` via `{}`: {}",
            gap.vx_id, gap.axis, gap.trigger, gap.required_evidence
        ));
    }
    for coverage in innovation_coverage {
        if !coverage.missing.is_empty() {
            blockers.push(format!(
                "innovation coverage gap in `{}` on axis `{}` missing {}",
                coverage.vx_id,
                coverage.axis,
                coverage.missing.join(", ")
            ));
        }
    }
    for finding in repo_boundary_findings {
        blockers.push(format!(
            "repo boundary finding at {}:{} touches private Santh visibility seam",
            finding.path, finding.line
        ));
    }
    for finding in megakernel_protocol_boundary_findings {
        blockers.push(format!(
            "megakernel protocol boundary finding at {} violates {}: {}",
            finding.path, finding.policy, finding.text
        ));
    }
    for finding in script_policy_findings {
        blockers.push(format!(
            "script policy finding at {}:{} violates {}",
            finding.path, finding.line, finding.policy
        ));
    }
    for finding in rust_toml_loader_findings {
        blockers.push(format!(
            "Rust TOML loader finding at {}:{} violates {}",
            finding.path, finding.line, finding.policy
        ));
    }
    for finding in source_ledger_findings {
        blockers.push(format!(
            "research source ledger finding for `{}` violates {}: {}",
            finding.key, finding.policy, finding.text
        ));
    }
    for finding in competitor_issue_findings {
        blockers.push(format!(
            "competitor issue ledger finding for `{}` violates {}: {}",
            finding.key, finding.policy, finding.text
        ));
    }
    for finding in research_plan_coverage_findings {
        blockers.push(format!(
            "research-plan coverage finding for `{}` violates {}: {}",
            finding.key, finding.policy, finding.text
        ));
    }
    for finding in archive_replay_findings {
        if finding.blocker_status != "confirmed-current" {
            blockers.push(format!(
                "archive replay finding at {}:{} {}: {}",
                finding.audit_path, finding.line, finding.blocker_status, finding.stale_reason
            ));
        }
    }
    for finding in rules_as_data_findings {
        blockers.push(format!(
            "rules-as-data finding for `{}` violates {}: {}",
            finding.key, finding.policy, finding.text
        ));
    }
    blockers
}

fn collect_loc_hotspots_in(root: &Path, dir: &Path, hotspots: &mut Vec<LocHotspot>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel_path = rel_path(root, &path);
        if skip_path(&rel_path) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            collect_loc_hotspots_in(root, &path, hotspots);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            if let Ok(source) = fs::read_to_string(&path) {
                let loc = source.lines().count();
                if loc >= 500 {
                    hotspots.push(LocHotspot { path: rel_path, loc });
                }
            }
        }
    }
}

fn collect_markdown_claims(root: &Path, path: &Path, findings: &mut Vec<ClaimDriftFinding>) {
    if findings.len() >= MAX_FINDINGS {
        return;
    }
    let rel = rel_path(root, path);
    if skip_path(&rel) || rel == PLAN_PATH || rel == LEGACY_DOCS_PATH {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_markdown_claims(root, &entry.path(), findings);
                if findings.len() >= MAX_FINDINGS {
                    return;
                }
            }
        }
        return;
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    for (line_index, line) in text.lines().enumerate() {
        if findings.len() >= MAX_FINDINGS {
            return;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("release/evidence") || lower.contains("artifact") {
            continue;
        }
        if let Some(marker) = CLAIM_MARKERS
            .iter()
            .find(|marker| lower.contains(*marker))
        {
            findings.push(ClaimDriftFinding {
                path: rel.clone(),
                line: line_index + 1,
                marker: (*marker).to_string(),
                text: compact_line(line),
                evidence_hint: "claim should cite a release evidence artifact or VX proof gate"
                    .to_string(),
            });
        }
    }
}

fn collect_stale_doc_markers_in(root: &Path, path: &Path, findings: &mut Vec<DocMarkerFinding>) {
    if findings.len() >= MAX_FINDINGS {
        return;
    }
    let rel = rel_path(root, path);
    if skip_path(&rel) || rel == PLAN_PATH || rel == LEGACY_DOCS_PATH || rel == COMMAND_MATRIX_PATH {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_stale_doc_markers_in(root, &entry.path(), findings);
                if findings.len() >= MAX_FINDINGS {
                    return;
                }
            }
        }
        return;
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    for (line_index, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        let marker = if lower.contains("active plan") {
            Some("active plan")
        } else if lower.contains("optimization plan") {
            Some("optimization plan")
        } else if lower.contains("worklist") {
            Some("worklist")
        } else {
            None
        };
        if let Some(marker) = marker {
            findings.push(DocMarkerFinding {
                path: rel.clone(),
                line: line_index + 1,
                marker: marker.to_string(),
                text: compact_line(line),
            });
            if findings.len() >= MAX_FINDINGS {
                return;
            }
        }
    }
}

fn collect_repo_boundary_findings_in(
    root: &Path,
    path: &Path,
    findings: &mut Vec<RepoBoundaryFinding>,
) {
    let rel = rel_path(root, path);
    if skip_path(&rel) {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_repo_boundary_findings_in(root, &entry.path(), findings);
            }
        }
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    for (line_index, line) in text.lines().enumerate() {
        if repo_boundary::touches_private_santh_visibility(line) {
            findings.push(RepoBoundaryFinding {
                path: rel.clone(),
                line: line_index + 1,
                text: compact_line(line),
                boundary: repo_boundary::repo_boundary_description().to_string(),
            });
        }
    }
}

fn collect_rust_toml_loader_findings_in(
    root: &Path,
    path: &Path,
    findings: &mut Vec<RustTomlLoaderFinding>,
) {
    if findings.len() >= MAX_FINDINGS {
        return;
    }
    let rel = rel_path(root, path);
    if skip_path(&rel) || rel == "xtask/src/toml_config.rs" {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_rust_toml_loader_findings_in(root, &entry.path(), findings);
                if findings.len() >= MAX_FINDINGS {
                    return;
                }
            }
        }
        return;
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    if !(text.contains("include_str!(") && text.contains("toml::from_str::<")) {
        return;
    }
    for (line_index, line) in text.lines().enumerate() {
        if line.contains("toml::from_str::<") {
            findings.push(RustTomlLoaderFinding {
                path: rel.clone(),
                line: line_index + 1,
                text: compact_line(line),
                policy:
                    "use xtask/src/toml_config.rs for embedded TOML parsing".to_string(),
            });
            return;
        }
    }
}

fn collect_megakernel_protocol_boundary_findings_in(
    root: &Path,
    path: &Path,
    findings: &mut Vec<SourceLedgerFinding>,
) {
    if findings.len() >= MAX_FINDINGS {
        return;
    }
    let rel = rel_path(root, path);
    if skip_path(&rel) {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_megakernel_protocol_boundary_findings_in(root, &entry.path(), findings);
                if findings.len() >= MAX_FINDINGS {
                    return;
                }
            }
        }
        return;
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("rs")
        || !rel.contains("megakernel")
    {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    for (line_index, line) in text.lines().enumerate() {
        if let Some(policy) = megakernel_protocol_boundary_policy(line) {
            findings.push(SourceLedgerFinding {
                path: rel.clone(),
                key: "megakernel-protocol-boundary".to_string(),
                text: format!("line {}: {}", line_index + 1, compact_line(line)),
                policy: policy.to_string(),
            });
            if findings.len() >= MAX_FINDINGS {
                return;
            }
        }
    }
}

fn megakernel_protocol_boundary_policy(line: &str) -> Option<&'static str> {
    let compact = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.contains("vyre_runtime::megakernel::protocol")
        || compact.contains("megakernel::protocol::")
        || compact.contains("use vyre_runtime::megakernel::{")
            && (compact.contains(" STATUS_WORD")
                || compact.contains(" SLOT_WORDS")
                || compact.contains(" CONTROL_MIN_WORDS")
                || compact.contains(" ARG0_WORD")
                || compact.contains(" OPCODE_WORD"))
    {
        return Some("driver megakernel code must use runtime Megakernel API wrappers instead of protocol internals");
    }
    if compact.contains("protocol::")
        || compact.contains("Megakernel::read_done_count")
        || compact.contains("Megakernel::read_observable")
        || compact.contains("Megakernel::count_done_ring_slots")
    {
        return Some("driver megakernel code must use fallible runtime protocol API wrappers");
    }
    None
}

pub(super) fn skip_path(rel_path: &str) -> bool {
    rel_path == ".git"
        || rel_path.starts_with(".git/")
        || rel_path == "target"
        || rel_path.starts_with("target/")
        || rel_path == "release/evidence"
        || rel_path.starts_with("release/evidence/")
        || rel_path.contains("/target/")
        || rel_path.contains("/node_modules/")
}

pub(super) fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn compact_line(line: &str) -> String {
    let compact = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 220 {
        let mut truncated = compact.chars().take(220).collect::<String>();
        truncated.push_str("...");
        truncated
    } else {
        compact
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_megakernel_protocol_boundary_findings, collect_rust_toml_loader_findings};

    #[test]
    fn duplicate_embedded_toml_loader_body_is_a_research_audit_finding() {
        let dir = tempfile::tempdir().expect("Fix: create Rust TOML loader fixture directory.");
        let src = dir.path().join("xtask/src");
        std::fs::create_dir_all(&src).expect("Fix: create xtask/src fixture directory.");
        std::fs::write(
            src.join("release_train.rs"),
            r#"const RELEASE_TRAIN_TOML: &str = include_str!("../../release/release-train.toml");
fn data() {
    let _ = toml::from_str::<ReleaseTrainData>(RELEASE_TRAIN_TOML);
}
"#,
        )
        .expect("Fix: write duplicate embedded TOML loader fixture.");
        std::fs::write(
            src.join("toml_config.rs"),
            r#"const EXAMPLE_TOML: &str = include_str!("../../release/release-train.toml");
fn data() {
    let _ = toml::from_str::<ReleaseTrainData>(EXAMPLE_TOML);
}
"#,
        )
        .expect("Fix: write canonical embedded TOML loader fixture.");

        let findings = collect_rust_toml_loader_findings(dir.path());

        assert_eq!(
            findings.len(),
            1,
            "Fix: only non-canonical embedded TOML loader bodies should be findings; findings={findings:?}"
        );
        assert_eq!(findings[0].path, "xtask/src/release_train.rs");
        assert!(
            findings[0].policy.contains("xtask/src/toml_config.rs"),
            "Fix: the Rust TOML loader finding must point to the shared config seam."
        );
    }

    #[test]
    fn driver_megakernel_protocol_import_is_a_boundary_finding() {
        let dir =
            tempfile::tempdir().expect("Fix: create megakernel protocol boundary fixture.");
        let src = dir.path().join("vyre-driver-wgpu/src");
        std::fs::create_dir_all(&src).expect("Fix: create driver fixture directory.");
        std::fs::write(
            src.join("megakernel.rs"),
            r#"use vyre_runtime::megakernel::protocol;
fn bad(bytes: &[u8]) -> u32 {
    protocol::read_done_count(bytes)
}
"#,
        )
        .expect("Fix: write protocol boundary fixture.");

        let findings = collect_megakernel_protocol_boundary_findings(dir.path());

        assert_eq!(findings.len(), 2);
        assert!(findings
            .iter()
            .all(|finding| finding.key == "megakernel-protocol-boundary"));
    }
}

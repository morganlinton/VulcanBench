//! Research-grounding audit for the all-axes acceleration plan.
//!
//! This command turns the plan's research and local-observation promises into a
//! cheap JSON artifact. It does not prove the implementation of every VX row;
//! it proves that the plan remains connected to source truth, competitor or
//! research baselines, duplicate-risk seams, and the public `vyre` repository
//! boundary.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

mod collectors;
mod innovation;
mod model;
mod parse;
mod script_policy;
mod source_ledger;
mod validate;

pub(crate) use model::RESEARCH_AUDIT_ARTIFACT;
pub(crate) use validate::validate_research_audit_artifact_bytes;

pub(crate) const RESEARCH_AUDIT_COMMAND_PREFIX: &str = "xtask research-audit";
pub(crate) const RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND: &str =
    "xtask research-audit --output release/evidence/optimization/research-audit.json";
pub(crate) const RESEARCH_AUDIT_SEMANTIC_VALIDATOR: &str =
    "research_audit::validate_research_audit_artifact_bytes";
pub(crate) const RESEARCH_AUDIT_SCHEMA_VERSION: u32 = model::SCHEMA_VERSION;
pub(crate) const RESEARCH_AUDIT_SOURCE_DIGEST_PREFIX: &str = model::SOURCE_DIGEST_PREFIX;
pub(crate) const RESEARCH_AUDIT_REQUIRED_SCALAR_FIELDS: &[&str] = &[
    "schema_version",
    "generator_command",
    "plan_path",
    "command_matrix_path",
    "source_digest",
];
pub(crate) const RESEARCH_AUDIT_REQUIRED_POSITIVE_COUNT_FIELDS: &[&str] = &[
    "plan_row_count",
    "minimum_plan_row_count",
    "axis_count",
    "defined_research_key_count",
    "used_research_key_count",
];
pub(crate) const RESEARCH_AUDIT_REQUIRED_ARRAY_FIELDS: &[&str] = &[
    "raw_counter_families",
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
    "blockers",
];

pub(crate) fn research_audit_required_artifact_fields() -> Vec<&'static str> {
    let mut fields = Vec::new();
    fields.extend_from_slice(RESEARCH_AUDIT_REQUIRED_SCALAR_FIELDS);
    fields.extend_from_slice(RESEARCH_AUDIT_REQUIRED_POSITIVE_COUNT_FIELDS);
    fields.extend_from_slice(RESEARCH_AUDIT_REQUIRED_ARRAY_FIELDS);
    fields
}

pub(crate) fn research_audit_generator_command(output: &Path) -> String {
    if output == Path::new(RESEARCH_AUDIT_ARTIFACT) {
        RESEARCH_AUDIT_DEFAULT_GENERATOR_COMMAND.to_string()
    } else {
        format!("{RESEARCH_AUDIT_COMMAND_PREFIX} --output {}", output.display())
    }
}

use collectors::{
    collect_archive_replay_findings, collect_baseline_gaps, collect_blockers, collect_claim_drift,
    collect_loc_hotspots, collect_megakernel_protocol_boundary_findings,
    collect_repo_boundary_findings, collect_rust_toml_loader_findings, collect_stale_doc_markers,
    collect_vx_linkage,
};
use innovation::collect_innovation_coverage;
use crate::research_key::backtick_research_keys;
use crate::research_plan_coverage::{research_plan_coverage_findings, ResearchPlanCoverageRow};
use model::{
    source_digest, ResearchAuditReport, SourceLedgerFinding, COMMAND_MATRIX_PATH, MIN_PLAN_ROWS,
    PLAN_PATH, RAW_COUNTER_FAMILIES,
};
use parse::{parse_defined_research_keys, parse_vx_rows};
use script_policy::collect_script_policy_findings;
use source_ledger::{collect_competitor_issue_findings, collect_source_ledger_findings};

#[derive(Debug, Clone)]
struct Config {
    output: PathBuf,
}

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            print_usage();
            std::process::exit(2);
        }
    };
    let root = workspace_root();
    let generator_command = research_audit_generator_command(&config.output);
    let report = match build_report(&root, generator_command) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("research-audit: {error}");
            std::process::exit(1);
        }
    };
    write_report(&root.join(&config.output), &report);
    if report.blockers.is_empty() {
        println!(
            "research-audit: wrote {} with {} VX row(s), {} LOC hotspot(s), {} baseline gap(s)",
            config.output.display(),
            report.plan_row_count,
            report.loc_hotspots.len(),
            report.baseline_gaps.len()
        );
    } else {
        eprintln!("research-audit: {} blocker(s):", report.blockers.len());
        for blocker in &report.blockers {
            eprintln!("  - {blocker}");
        }
        std::process::exit(1);
    }
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut output = PathBuf::from(RESEARCH_AUDIT_ARTIFACT);
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--output requires a path".to_string());
                };
                output = PathBuf::from(path.as_str());
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown research-audit option `{other}`")),
        }
        index += 1;
    }
    Ok(Config { output })
}

fn print_usage() {
    eprintln!(
        "USAGE:\n  cargo_full run -p xtask --bin xtask -- research-audit [--output PATH]\n\n\
         Writes a local research-grounding artifact for VX rows, source hotspots,\n\
         competitor baselines, duplicate-risk VX linkage, stale-doc markers, and\n\
         public repository boundary plus archive-audit replay checks."
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn build_report(root: &Path, generator_command: String) -> Result<ResearchAuditReport, String> {
    let plan = read_required(root, PLAN_PATH)?;
    let command_matrix = read_required(root, COMMAND_MATRIX_PATH)?;
    let rows = parse_vx_rows(&plan);
    let axes = rows
        .iter()
        .map(|row| row.axis.as_str())
        .collect::<BTreeSet<_>>();
    let defined_research_keys = parse_defined_research_keys(&plan);
    let used_research_keys = rows
        .iter()
        .flat_map(|row| backtick_research_keys(&row.research_basis))
        .collect::<BTreeSet<_>>();
    let loc_hotspots = collect_loc_hotspots(root);
    let claim_drift = collect_claim_drift(root);
    let baseline_gaps = collect_baseline_gaps(&rows);
    let innovation_coverage = collect_innovation_coverage(&rows);
    let high_risk_vx_linkage = collect_vx_linkage(&command_matrix, &plan);
    let stale_doc_markers = collect_stale_doc_markers(root);
    let repo_boundary_findings = collect_repo_boundary_findings(root);
    let megakernel_protocol_boundary_findings =
        collect_megakernel_protocol_boundary_findings(root);
    let script_policy_findings = collect_script_policy_findings(root);
    let rust_toml_loader_findings = collect_rust_toml_loader_findings(root);
    let source_ledger_findings =
        collect_source_ledger_findings(root, &defined_research_keys, &used_research_keys, &rows);
    let competitor_issue_findings =
        collect_competitor_issue_findings(root, &defined_research_keys, &used_research_keys, &rows);
    let research_plan_coverage_findings =
        research_plan_coverage_findings(PLAN_PATH, &coverage_rows(&rows))
            .into_iter()
            .map(|finding| SourceLedgerFinding {
                path: finding.path,
                key: finding.key,
                text: finding.text,
                policy: finding.policy,
            })
            .collect::<Vec<_>>();
    let archive_replay_findings = collect_archive_replay_findings(root);
    let rules_as_data_findings = crate::rules_as_data::rules_as_data_findings(root)
        .into_iter()
        .map(|finding| SourceLedgerFinding {
            path: finding.path,
            key: finding.key,
            text: finding.text,
            policy: finding.policy,
        })
        .collect::<Vec<_>>();
    let raw_counter_families = RAW_COUNTER_FAMILIES
        .iter()
        .map(|family| (*family).to_string())
        .collect::<Vec<_>>();
    let blockers = collect_blockers(
        &rows,
        &defined_research_keys,
        &used_research_keys,
        &high_risk_vx_linkage,
        &repo_boundary_findings,
        &megakernel_protocol_boundary_findings,
        &script_policy_findings,
        &rust_toml_loader_findings,
        &source_ledger_findings,
        &competitor_issue_findings,
        &research_plan_coverage_findings,
        &archive_replay_findings,
        &rules_as_data_findings,
        &baseline_gaps,
        &innovation_coverage,
    );
    let source_digest = source_digest(
        &plan,
        &command_matrix,
        rows.len(),
        MIN_PLAN_ROWS,
        loc_hotspots.len(),
        claim_drift.len(),
        baseline_gaps.len(),
        innovation_coverage.len(),
        high_risk_vx_linkage.len(),
        stale_doc_markers.len(),
        repo_boundary_findings.len(),
        megakernel_protocol_boundary_findings.len(),
        script_policy_findings.len(),
        rust_toml_loader_findings.len(),
        source_ledger_findings.len(),
        competitor_issue_findings.len(),
        research_plan_coverage_findings.len(),
        archive_replay_findings.len(),
        rules_as_data_findings.len(),
        &format!(
            "{raw_counter_families:?}\n{loc_hotspots:?}\n{claim_drift:?}\n{baseline_gaps:?}\n{innovation_coverage:?}\n{high_risk_vx_linkage:?}\n{stale_doc_markers:?}\n{repo_boundary_findings:?}\n{megakernel_protocol_boundary_findings:?}\n{script_policy_findings:?}\n{rust_toml_loader_findings:?}\n{source_ledger_findings:?}\n{competitor_issue_findings:?}\n{research_plan_coverage_findings:?}\n{archive_replay_findings:?}\n{rules_as_data_findings:?}\n"
        ),
    );
    Ok(ResearchAuditReport {
        schema_version: model::SCHEMA_VERSION,
        generator_command,
        plan_path: PLAN_PATH,
        command_matrix_path: COMMAND_MATRIX_PATH,
        plan_row_count: rows.len(),
        minimum_plan_row_count: MIN_PLAN_ROWS,
        axis_count: axes.len(),
        defined_research_key_count: defined_research_keys.len(),
        used_research_key_count: used_research_keys.len(),
        raw_counter_families,
        loc_hotspots,
        claim_drift,
        baseline_gaps,
        innovation_coverage,
        high_risk_vx_linkage,
        stale_doc_markers,
        repo_boundary_findings,
        megakernel_protocol_boundary_findings,
        script_policy_findings,
        rust_toml_loader_findings,
        source_ledger_findings,
        competitor_issue_findings,
        research_plan_coverage_findings,
        archive_replay_findings,
        rules_as_data_findings,
        blockers,
        source_digest,
    })
}

fn coverage_rows(rows: &[model::VxRow]) -> Vec<ResearchPlanCoverageRow<'_>> {
    rows.iter()
        .map(|row| ResearchPlanCoverageRow {
            line: row.line,
            id: &row.id,
            local_evidence: &row.local_evidence,
            research_basis: &row.research_basis,
            proof_gate: &row.proof_gate,
            dedup_seam: &row.dedup_seam,
        })
        .collect()
}

fn read_required(root: &Path, rel_path: &str) -> Result<String, String> {
    fs::read_to_string(root.join(rel_path)).map_err(|error| format!("read `{rel_path}`: {error}"))
}

fn write_report(path: &Path, report: &ResearchAuditReport) {
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("research-audit: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    let json = match serde_json::to_string_pretty(report) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("research-audit: failed to serialize report: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = fs::write(path, format!("{json}\n")) {
        eprintln!("research-audit: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

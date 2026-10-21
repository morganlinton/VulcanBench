//! Evidence-backed acceleration plan gate.
//!
//! This gate enforces the shape promised by
//! `docs/optimization/ALL_AXES_ACCELERATION_PLAN.md`: each VX work row must
//! carry a concrete axis, local evidence, research basis, work item, proof gate,
//! and dedup seam. The gate is deliberately structural. It prevents another
//! generated catalog from replacing the plan without pretending to prove the
//! implementation status of every VX row.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::hash::sha256_hex;
use crate::innovation_falsification::missing_frontier_falsification_fields;
use crate::markdown_table::markdown_cells;
use crate::research_basis::external_research_basis_entries;
use crate::research_key::{backtick_research_keys, is_research_key};
use crate::research_plan_coverage::{
    research_plan_coverage_findings, ResearchPlanCoverageRow,
};
use crate::research_source_ledger::{
    read_research_source_ledger, research_source_urls_by_key, unknown_research_source_vx_rows,
    ResearchSourceLedger,
};
use crate::vx_plan_table::{parse_raw_vx_plan_table, VX_PLAN_MIN_ROWS, VX_PLAN_TABLE_HEADER};

const DEFAULT_PLAN: &str = "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md";
pub(crate) use crate::artifact_paths::PLAN_PROGRESS_ARTIFACT;
pub(crate) const PLAN_PROGRESS_SCHEMA_VERSION: u32 = 4;
const REQUIRED_AXES: &[&str] = &[
    "bench_harness",
    "compiler_optimizer",
    "coordination",
    "driver_cuda",
    "driver_metal",
    "driver_shared",
    "driver_wgpu",
    "evidence_truth",
    "flow_weir",
    "foundation_optimizer",
    "foundation_wire",
    "graph_flow_compiler",
    "lower_emit",
    "nn_math",
    "parser_frontend",
    "product_dogfood",
    "runtime_megakernel",
    "runtime_residency",
    "scan_automata",
    "scan_static",
    "security_dataflow",
    "security_reliability",
    "sparse_math_ai",
    "testing_evidence",
];
const REJECTED_AXIS_ALIASES: &[(&str, &str)] = &[("frontend_parsing", "parser_frontend")];
const REQUIRED_HOT_PATHS: &[&str] = &[
    "vyre-driver/src/backend/compiled_pipeline.rs",
    "vyre-driver/src/launch_fusion.rs",
    "vyre-driver-cuda/src/backend/cuda_graph.rs",
    "vyre-driver-cuda/src/backend/dispatch.rs",
    "vyre-emit-naga/src/emitter/op_dispatch.rs",
    "vyre-lower/src/pre_emit.rs",
    "vyre-runtime/src/megakernel/ring.rs",
    "vyre-runtime/src/megakernel/telemetry.rs",
];
const GENERATED_SECTION_MARKERS: &[&str] = &[
    "## Massive research-grade expansion appendix",
    "## Ultra-scale research-grade expansion appendix",
    "## 10,000+ test expansion program",
    "## 100,000+ test and validation expansion program",
    "## Implementation slice 10: ultra-scale 10000-label expansion",
];
const LOCAL_EVIDENCE_OBSERVATION_MARKERS: &[&str] = &[
    " is ",
    " are ",
    " was ",
    " were ",
    " has ",
    " have ",
    " owns ",
    " own ",
    " acts ",
    " act ",
    " computes",
    " compute",
    " connects",
    " connect",
    " covers",
    " cover",
    " defines",
    " define",
    " encodes",
    " encode",
    " governs",
    " govern",
    " handles",
    " handle",
    " lists",
    " list",
    " models",
    " model",
    " parses",
    " parse",
    " promotes",
    " promote",
    " reconstructs",
    " reconstruct",
    " routes",
    " route",
    " selects",
    " select",
    " wraps",
    " wrap",
    " implements",
    " implement",
    " exposes",
    " expose",
    " carries",
    " carry",
    " remains",
    " remain",
    " keeps",
    " keep",
    " includes",
    " include",
    " guards",
    " guard",
    " overlaps",
    " overlap",
    " reasons",
    " reason",
    " already ",
    " loc",
    "bloat",
    "cache",
    "compatibility",
    "duplicate",
    "fallback",
    "gap",
    "hot path",
    "missing",
    "plan",
    "policy",
    "proof",
    "scattered",
    "seam",
    "separate",
    "shape",
    "source-of-truth",
    "surface",
];
const INNOVATION_COMPARISON_MARKERS: &[&str] = &[
    "against",
    "baseline",
    "bench",
    "candidate count",
    "compare",
    "compares",
    "fewer",
    "latency",
    "less",
    "parity",
    "reduce",
    "reduces",
    "throughput",
    "with and without",
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct GateConfig {
    plan: PathBuf,
    progress_json: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanRow {
    line: usize,
    id: String,
    axis: String,
    local_evidence: String,
    research_basis: String,
    work: String,
    proof_gate: String,
    dedup_seam: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanGateReport {
    row_count: usize,
    external_keys: BTreeSet<String>,
    rows: Vec<PlanRow>,
    failures: Vec<String>,
}

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let text = match fs::read_to_string(&config.plan) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "Fix: failed to read acceleration plan `{}`: {error}",
                config.plan.display()
            );
            std::process::exit(1);
        }
    };
    let vyre_root = default_vyre_root();
    let report = validate_plan_text_with_root(&text, Some(&vyre_root));
    let mut failures = report.failures.clone();
    let ownership_path = vyre_root.join("docs/optimization/OWNERSHIP.toml");
    validate_plan_axes_have_lanes(&ownership_path, &report.rows, &mut failures);
    validate_claim_files(
        &ownership_path,
        &vyre_root.join("docs/optimization/CLAIMS.toml"),
        &mut failures,
    );
    validate_hot_paths_file(
        &vyre_root.join("docs/optimization/HOT_PATHS.toml"),
        &mut failures,
    );
    failures.extend(crate::rules_as_data::validate_rules_as_data_manifest(&vyre_root));
    validate_compat_alias_audit(&vyre_root, &mut failures);
    validate_no_parallel_active_plan_files(&vyre_root, &mut failures);
    if failures.is_empty() {
        if let Some(progress_json) = config.progress_json.as_ref() {
            if let Err(error) = write_plan_progress_json(progress_json, &report) {
                failures.push(format!(
                    "plan progress JSON could not be written to `{}`: {error}",
                    progress_json.display()
                ));
            }
        }
    }
    if failures.is_empty() {
        println!(
            "acceleration-plan-gate: {} VX rows and claim lanes validated in {}",
            report.row_count,
            config.plan.display()
        );
        return;
    }
    eprintln!(
        "Fix: acceleration plan `{}` failed {} gate check(s):",
        config.plan.display(),
        failures.len()
    );
    for failure in &failures {
        eprintln!("- {failure}");
    }
    std::process::exit(1);
}

fn parse_args(args: &[String]) -> Result<GateConfig, String> {
    let vyre_root = default_vyre_root();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(
            "USAGE:\n  cargo_full run --bin xtask -- acceleration-plan-gate [--plan PATH] [--progress-json PATH]"
                .to_string(),
        );
    }
    let mut plan = vyre_root.join(DEFAULT_PLAN);
    let mut progress_json = None;
    let mut index = 2usize;
    while index < args.len() {
        match args[index].as_str() {
            "--plan" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("Fix: --plan requires a path".to_string());
                };
                plan = PathBuf::from(value);
                index += 2;
            }
            "--progress-json" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("Fix: --progress-json requires a path".to_string());
                };
                progress_json = Some(PathBuf::from(value));
                index += 2;
            }
            other => {
                return Err(format!(
                    "Fix: invalid acceleration-plan-gate arg `{other}`. Usage: acceleration-plan-gate [--plan PATH] [--progress-json PATH]"
                ));
            }
        }
    }
    Ok(GateConfig {
        plan,
        progress_json,
    })
}

fn default_vyre_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn validate_plan_text(text: &str) -> PlanGateReport {
    validate_plan_text_with_root(text, None)
}

fn validate_plan_text_with_root(text: &str, root: Option<&Path>) -> PlanGateReport {
    let mut failures = Vec::new();
    for marker in GENERATED_SECTION_MARKERS {
        if text.contains(marker) {
            failures.push(format!(
                "generated appendix marker `{marker}` is present; remove generated catalog content"
            ));
        }
    }
    let external_research_entries = match external_research_basis_entries(text) {
        Ok(entries) => entries,
        Err(parse_failures) => {
            failures.extend(parse_failures);
            BTreeMap::new()
        }
    };
    let external_keys = external_research_entries
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let research_source_ledger = parse_research_source_ledger(root, &mut failures);
    let required_research_entries = research_source_ledger
        .as_ref()
        .map(research_source_urls_by_key)
        .unwrap_or_default();
    validate_research_key_coverage(
        &external_research_entries,
        &required_research_entries,
        &mut failures,
    );
    let rows = parse_plan_rows(text, &mut failures);
    validate_research_source_ledger_vx_rows(research_source_ledger.as_ref(), &rows, &mut failures);
    failures.extend(
        research_plan_coverage_findings(DEFAULT_PLAN, &coverage_rows(&rows))
            .into_iter()
            .map(|finding| {
                format!(
                    "research-plan coverage `{}` violates {}: {}",
                    finding.key, finding.policy, finding.text
                )
            }),
    );
    validate_rows(&rows, &external_keys, root, &mut failures);
    PlanGateReport {
        row_count: rows.len(),
        external_keys,
        rows,
        failures,
    }
}

fn validate_claim_files(ownership_path: &Path, claims_path: &Path, failures: &mut Vec<String>) {
    let ownership_text = match fs::read_to_string(ownership_path) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "claim audit could not read ownership map `{}`: {error}",
                ownership_path.display()
            ));
            return;
        }
    };
    let claims_text = match fs::read_to_string(claims_path) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "claim audit could not read claims file `{}`: {error}",
                claims_path.display()
            ));
            return;
        }
    };
    let ownership_lanes = match crate::ownership::parse_ownership_lane_names(&ownership_text) {
        Ok(lanes) => lanes,
        Err(error) => {
            failures.push(format!("claim audit ownership parse failed: {error}"));
            return;
        }
    };
    match validate_claim_text(&claims_text, &ownership_lanes) {
        Ok(()) => {}
        Err(mut claim_failures) => failures.append(&mut claim_failures),
    }
}

fn validate_hot_paths_file(path: &Path, failures: &mut Vec<String>) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "hot-path audit could not read `{}`: {error}",
                path.display()
            ));
            return;
        }
    };
    match validate_hot_paths_text(&text) {
        Ok(()) => {}
        Err(mut hot_path_failures) => failures.append(&mut hot_path_failures),
    }
}

fn validate_plan_axes_have_lanes(
    ownership_path: &Path,
    rows: &[PlanRow],
    failures: &mut Vec<String>,
) {
    let ownership_text = match fs::read_to_string(ownership_path) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "plan axis audit could not read ownership map `{}`: {error}",
                ownership_path.display()
            ));
            return;
        }
    };
    let lane_classifications =
        match crate::ownership::parse_ownership_lane_classifications(&ownership_text) {
        Ok(lanes) => lanes,
        Err(error) => {
            failures.push(format!("plan axis audit ownership parse failed: {error}"));
            return;
        }
    };
    let lanes = lane_classifications.keys().cloned().collect::<BTreeSet<_>>();
    validate_supporting_ownership_lanes(ownership_path, &lane_classifications, failures);
    let mut missing = BTreeSet::new();
    for row in rows {
        if !lanes.contains(&row.axis) {
            missing.insert(row.axis.clone());
        }
    }
    for axis in missing {
        failures.push(format!(
            "plan axis `{axis}` has no matching ownership lane in `{}`. Fix: add a lane or move the VX rows to a canonical lane.",
            ownership_path.display()
        ));
    }
}

fn validate_supporting_ownership_lanes(
    ownership_path: &Path,
    lanes: &BTreeMap<String, crate::ownership::OwnershipLaneClassification>,
    failures: &mut Vec<String>,
) {
    let required_axes = REQUIRED_AXES.iter().copied().collect::<BTreeSet<_>>();
    for (lane_name, lane) in lanes {
        if required_axes.contains(lane_name.as_str()) {
            continue;
        }
        if lane.write_patterns.is_empty() {
            failures.push(format!(
                "supporting ownership lane `{lane_name}` in `{}` has no write set. Fix: add allowed write patterns or remove the lane.",
                ownership_path.display()
            ));
        }
        let Some(parent_axis) = lane.parent_axis.as_deref() else {
            failures.push(format!(
                "supporting ownership lane `{lane_name}` in `{}` is not a VX axis and lacks `parent_axis`. Fix: set `parent_axis` to a canonical VX axis.",
                ownership_path.display()
            ));
            continue;
        };
        if !required_axes.contains(parent_axis) {
            failures.push(format!(
                "supporting ownership lane `{lane_name}` in `{}` uses unknown parent_axis `{parent_axis}`. Fix: point it at a canonical VX axis.",
                ownership_path.display()
            ));
        }
        let Some(reason) = lane.support_reason.as_deref() else {
            failures.push(format!(
                "supporting ownership lane `{lane_name}` in `{}` lacks `support_reason`. Fix: explain why this lane is ownership-only instead of a VX axis.",
                ownership_path.display()
            ));
            continue;
        };
        if reason.split_whitespace().count() < 5 {
            failures.push(format!(
                "supporting ownership lane `{lane_name}` in `{}` has a too-short support_reason. Fix: name the boundary and why it is not a VX axis.",
                ownership_path.display()
            ));
        }
    }
}

fn validate_hot_paths_text(text: &str) -> Result<(), Vec<String>> {
    let value = match toml::from_str::<toml::Value>(text) {
        Ok(value) => value,
        Err(error) => return Err(vec![format!("HOT_PATHS.toml is invalid TOML: {error}")]),
    };
    let Some(entries) = value.get("hot_path").and_then(toml::Value::as_array) else {
        return Err(vec![
            "HOT_PATHS.toml must contain [[hot_path]] entries".to_string()
        ]);
    };
    let mut seen = BTreeSet::<String>::new();
    let mut duplicates = BTreeSet::<String>::new();
    for entry in entries {
        let Some(file) = entry.get("file").and_then(toml::Value::as_str) else {
            continue;
        };
        if !seen.insert(file.to_string()) {
            duplicates.insert(file.to_string());
        }
    }
    let mut failures = Vec::new();
    for duplicate in duplicates {
        failures.push(format!("HOT_PATHS.toml duplicates `{duplicate}`"));
    }
    for required in REQUIRED_HOT_PATHS {
        if !seen.contains(*required) {
            failures.push(format!(
                "HOT_PATHS.toml is missing required VX-003 surface `{required}`"
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn validate_claim_text(text: &str, ownership_lanes: &BTreeSet<String>) -> Result<(), Vec<String>> {
    let value = match toml::from_str::<toml::Value>(text) {
        Ok(value) => value,
        Err(error) => return Err(vec![format!("CLAIMS.toml is invalid TOML: {error}")]),
    };
    let Some(claims) = value.get("claim").and_then(toml::Value::as_array) else {
        return Err(vec![
            "CLAIMS.toml must contain at least one [[claim]] entry".to_string(),
        ]);
    };
    let mut failures = Vec::new();
    for (index, claim) in claims.iter().enumerate() {
        validate_claim(index, claim, ownership_lanes, &mut failures);
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn validate_claim(
    index: usize,
    claim: &toml::Value,
    ownership_lanes: &BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    let claim_no = index + 1;
    let owner = claim
        .get("owner")
        .and_then(toml::Value::as_str)
        .unwrap_or("<missing owner>");
    let status = claim
        .get("status")
        .and_then(toml::Value::as_str)
        .unwrap_or("");
    if status.trim().is_empty() {
        failures.push(format!("claim {claim_no} `{owner}` is missing status"));
    } else if !matches!(status, "active" | "done") {
        failures.push(format!(
            "claim {claim_no} `{owner}` has unsupported status `{status}`; use `active` for requirements or `done` for historical evidence"
        ));
    }
    let lanes = claim_string_array(claim, "lanes");
    if lanes.is_empty() {
        failures.push(format!("claim {claim_no} `{owner}` has no lanes"));
    }
    let unknown_lanes = lanes
        .iter()
        .filter(|lane| !ownership_lanes.contains(*lane))
        .cloned()
        .collect::<Vec<_>>();
    let claim_text = claim_joined_text(claim);
    let has_seam = claim_text_names_seam(&claim_text);
    if status == "active" && !has_seam {
        failures.push(format!(
            "active claim {claim_no} `{owner}` does not name the seam, boundary, shared owner, or integration point"
        ));
    }
    if status == "active" && lanes.len() > 1 && !has_seam {
        failures.push(format!(
            "active claim {claim_no} `{owner}` spans {} lanes ({}) without naming a seam or boundary",
            lanes.len(),
            lanes.join(", ")
        ));
    }
    if status == "active" && !unknown_lanes.is_empty() && !has_seam {
        failures.push(format!(
            "active claim {claim_no} `{owner}` uses non-ownership lane(s) {} without an explicit seam",
            unknown_lanes.join(", ")
        ));
    }
    if status == "active" && claim_string_array(claim, "proof_required").is_empty() {
        failures.push(format!(
            "active claim {claim_no} `{owner}` has no proof_required entries"
        ));
    }
    if status == "active" && !claim_string_array(claim, "proof").is_empty() {
        failures.push(format!(
            "active claim {claim_no} `{owner}` contains proof records; move completed proof evidence to a `done` claim"
        ));
    }
}

fn claim_string_array(claim: &toml::Value, key: &str) -> Vec<String> {
    claim
        .get(key)
        .and_then(toml::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn claim_joined_text(claim: &toml::Value) -> String {
    let mut out = String::new();
    for key in ["scope", "proof_required", "proof", "notes"] {
        for value in claim_string_array(claim, key) {
            out.push_str(&value);
            out.push('\n');
        }
    }
    out.to_ascii_lowercase()
}

fn claim_text_names_seam(text: &str) -> bool {
    [
        "seam",
        "boundary",
        "cross-lane",
        "shared",
        "bridge",
        "integration",
        "adapter",
        "imports",
        "delegates",
        "moves",
        "runtime owns",
        "driver may keep",
        "source of truth",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn validate_research_key_coverage(
    external_entries: &BTreeMap<String, String>,
    required_entries: &BTreeMap<String, String>,
    failures: &mut Vec<String>,
) {
    for (required, ledger_url) in required_entries {
        let Some(external_url) = external_entries.get(required) else {
            failures.push(format!(
                "external research basis is missing required key `{required}`"
            ));
            continue;
        };
        if external_url != ledger_url {
            failures.push(format!(
                "external research basis key `{required}` URL `{external_url}` does not match research source ledger URL `{ledger_url}`"
            ));
        }
    }
}

fn parse_research_source_ledger(
    root: Option<&Path>,
    failures: &mut Vec<String>,
) -> Option<ResearchSourceLedger> {
    let Some(root) = root else {
        return None;
    };
    match read_research_source_ledger(root) {
        Ok(ledger) => Some(ledger),
        Err(error) => {
            failures.push(error);
            None
        }
    }
}

fn validate_research_source_ledger_vx_rows(
    ledger: Option<&ResearchSourceLedger>,
    rows: &[PlanRow],
    failures: &mut Vec<String>,
) {
    let Some(ledger) = ledger else {
        return;
    };
    let known_vx_rows = rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    for finding in unknown_research_source_vx_rows(ledger, &known_vx_rows) {
        failures.push(format!(
            "research source ledger key `{}` links unknown VX row `{}`",
            finding.key, finding.vx_row
        ));
    }
}

fn coverage_rows(rows: &[PlanRow]) -> Vec<ResearchPlanCoverageRow<'_>> {
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

fn parse_plan_rows(text: &str, failures: &mut Vec<String>) -> Vec<PlanRow> {
    let table = parse_raw_vx_plan_table(text);
    failures.extend(table.failures);
    let saw_header = table.saw_header;
    let rows = table
        .rows
        .into_iter()
        .map(|row| PlanRow {
            line: row.line,
            id: row.id,
            axis: row.axis,
            local_evidence: row.local_evidence,
            research_basis: row.research_basis,
            work: row.work,
            proof_gate: row.proof_gate,
            dedup_seam: row.dedup_seam,
        })
        .collect::<Vec<_>>();
    if !saw_header {
        failures.push(format!(
            "missing required plan item table header `{VX_PLAN_TABLE_HEADER}`"
        ));
    }
    if rows.len() < VX_PLAN_MIN_ROWS {
        failures.push(format!(
            "plan has {} VX rows, expected at least {VX_PLAN_MIN_ROWS}",
            rows.len()
        ));
    }
    rows
}

fn validate_rows(
    rows: &[PlanRow],
    external_keys: &BTreeSet<String>,
    root: Option<&Path>,
    failures: &mut Vec<String>,
) {
    let mut ids = BTreeMap::<u32, usize>::new();
    let mut raw_ids = BTreeSet::<String>::new();
    let mut dedup_seams = BTreeMap::<String, usize>::new();
    for row in rows {
        for (name, value) in [
            ("ID", row.id.as_str()),
            ("Axis", row.axis.as_str()),
            ("Local evidence", row.local_evidence.as_str()),
            ("Research basis", row.research_basis.as_str()),
            ("Work", row.work.as_str()),
            ("Proof gate", row.proof_gate.as_str()),
            ("Dedup seam", row.dedup_seam.as_str()),
        ] {
            if value.trim().is_empty() {
                failures.push(format!("line {}: `{name}` cell is empty", row.line));
            }
        }
        if !raw_ids.insert(row.id.clone()) {
            failures.push(format!("line {}: duplicate row id `{}`", row.line, row.id));
        }
        match vx_number(&row.id) {
            Some(number) => {
                ids.insert(number, row.line);
            }
            None => failures.push(format!(
                "line {}: row id `{}` does not match VX-###",
                row.line, row.id
            )),
        }
        if !axis_shape_is_valid(&row.axis) {
            failures.push(format!(
                "line {}: axis `{}` must use lowercase lane-style words",
                row.line, row.axis
            ));
        }
        validate_rejected_axis_alias(row, failures);
        validate_research_basis(row, external_keys, failures);
        validate_local_evidence(row, root, failures);
        validate_work(row, failures);
        validate_innovation_candidate(row, failures);
        validate_proof_gate(row, failures);
        validate_dedup_seam(row, failures);
        validate_dedup_seam_uniqueness(row, &mut dedup_seams, failures);
    }
    if let Some(max_id) = ids.keys().next_back().copied() {
        for expected in 1..=max_id {
            if !ids.contains_key(&expected) {
                failures.push(format!("missing VX-{expected:03} row"));
            }
        }
    }
    let axes = rows
        .iter()
        .map(|row| row.axis.as_str())
        .collect::<BTreeSet<_>>();
    for required in REQUIRED_AXES {
        if !axes.contains(required) {
            failures.push(format!("plan is missing required axis `{required}`"));
        }
    }
}

fn validate_rejected_axis_alias(row: &PlanRow, failures: &mut Vec<String>) {
    for (alias, canonical) in REJECTED_AXIS_ALIASES {
        if row.axis == *alias {
            failures.push(format!(
                "line {}: axis `{}` was merged into canonical axis `{}`; move this VX row to `{}`",
                row.line, row.axis, canonical, canonical
            ));
        }
    }
}

fn validate_research_basis(
    row: &PlanRow,
    external_keys: &BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    if row.research_basis.contains("Internal Vyre") {
        return;
    }
    let keys = backtick_tokens(&row.research_basis);
    if keys.is_empty() {
        failures.push(format!(
            "line {}: research basis must cite a backtick key or explicit internal contract",
            row.line
        ));
        return;
    }
    for key in keys {
        if external_keys.contains(&key)
            || key.contains('/')
            || key.ends_with(".md")
            || key.ends_with(".toml")
        {
            continue;
        }
        if !is_research_key(&key) {
            failures.push(format!(
                "line {}: research key `{key}` must use uppercase letters, digits, and underscores",
                row.line
            ));
            continue;
        }
        failures.push(format!(
            "line {}: research key `{key}` is not defined in external research basis",
            row.line
        ));
    }
}

fn validate_local_evidence(row: &PlanRow, root: Option<&Path>, failures: &mut Vec<String>) {
    let has_path = row.local_evidence.contains('`') || row.local_evidence.starts_with("This file");
    if !has_path {
        failures.push(format!(
            "line {}: local evidence must cite a concrete path or this file",
            row.line
        ));
        return;
    }
    let Some(root) = root else {
        return;
    };
    validate_plan_self_citation(row, failures);
    validate_local_evidence_observation(row, failures);
    if row.local_evidence.starts_with("This file") {
        return;
    }
    let path_tokens = backtick_tokens(&row.local_evidence)
        .into_iter()
        .filter(|token| looks_like_local_path(token))
        .collect::<Vec<_>>();
    if path_tokens.is_empty() {
        failures.push(format!(
            "line {}: local evidence must cite at least one local path token",
            row.line
        ));
        return;
    }
    for token in path_tokens {
        let path = root.join(&token);
        if !path.exists() {
            failures.push(format!(
                "line {}: local evidence path `{token}` does not exist",
                row.line
            ));
        }
    }
}

fn validate_plan_self_citation(row: &PlanRow, failures: &mut Vec<String>) {
    let evidence = row.local_evidence.to_ascii_lowercase();
    let cites_active_plan = row.local_evidence.starts_with("This file")
        || evidence.contains("all_axes_acceleration_plan.md")
        || evidence.contains(DEFAULT_PLAN);
    if !cites_active_plan {
        return;
    }
    let row_text = format!(
        "{} {} {} {}",
        row.axis, row.local_evidence, row.work, row.dedup_seam
    )
    .to_ascii_lowercase();
    if row.axis == "coordination" && row_text.contains("plan") {
        return;
    }
    failures.push(format!(
        "line {}: local evidence cites the active plan; only coordination rows explicitly about the plan file may use this plan as evidence",
        row.line
    ));
}

fn validate_local_evidence_observation(row: &PlanRow, failures: &mut Vec<String>) {
    let lower = row.local_evidence.to_ascii_lowercase();
    if LOCAL_EVIDENCE_OBSERVATION_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return;
    }
    failures.push(format!(
        "line {}: local evidence must name an observed gap, bloat, seam, ownership fact, or missing proof in addition to citing paths",
        row.line
    ));
}

fn validate_work(row: &PlanRow, failures: &mut Vec<String>) {
    const PREFIXES: &[&str] = &["Fix:", "Improvement:", "Innovation candidate:"];
    if !PREFIXES.iter().any(|prefix| row.work.starts_with(prefix)) {
        failures.push(format!(
            "line {}: work `{}` must start with `Fix:`, `Improvement:`, or `Innovation candidate:`",
            row.line, row.work
        ));
        return;
    }
    let body = row
        .work
        .split_once(':')
        .map(|(_, body)| body.trim())
        .unwrap_or_default();
    if body.is_empty() || body.eq_ignore_ascii_case("n/a") {
        failures.push(format!(
            "line {}: work `{}` must name a concrete change after its prefix",
            row.line, row.work
        ));
    }
}

fn validate_innovation_candidate(row: &PlanRow, failures: &mut Vec<String>) {
    if !row.work.starts_with("Innovation candidate:") {
        return;
    }
    let research_keys = backtick_research_keys(&row.research_basis);
    let has_non_internal_baseline = research_keys.iter().any(|key| {
        !key.starts_with("INTERNAL_")
            && !key.contains('/')
            && !key.ends_with(".md")
            && !key.ends_with(".toml")
    });
    if !has_non_internal_baseline {
        failures.push(format!(
            "line {}: innovation candidate must cite at least one non-internal research or peer baseline key",
            row.line
        ));
    }
    let comparison_text =
        format!("{} {} {}", row.work, row.proof_gate, row.local_evidence).to_ascii_lowercase();
    if !INNOVATION_COMPARISON_MARKERS
        .iter()
        .any(|marker| comparison_text.contains(marker))
    {
        failures.push(format!(
            "line {}: innovation candidate must name a comparison, baseline, parity, or benchmark proof mechanism",
            row.line
        ));
    }
    let missing_falsification =
        missing_frontier_falsification_fields(&row.id, &comparison_text);
    if !missing_falsification.is_empty() {
        failures.push(format!(
            "line {}: frontier innovation candidate must name falsification fields: {}",
            row.line,
            missing_falsification.join(", ")
        ));
    }
}

fn validate_proof_gate(row: &PlanRow, failures: &mut Vec<String>) {
    let lower = row.proof_gate.to_ascii_lowercase();
    let proof_words = [
        "test",
        "bench",
        "audit",
        "gate",
        "cargo_full",
        "rg",
        "assert",
        "reject",
        "evidence",
        "targeted",
        "scan",
    ];
    if !proof_words.iter().any(|word| lower.contains(word)) {
        failures.push(format!(
            "line {}: proof gate `{}` does not name a concrete proof mechanism",
            row.line, row.proof_gate
        ));
    }
}

fn validate_dedup_seam(row: &PlanRow, failures: &mut Vec<String>) {
    let lower = row.dedup_seam.to_ascii_lowercase();
    if lower.contains("new ") && !lower.contains("one ") {
        failures.push(format!(
            "line {}: dedup seam `{}` looks like invention rather than reuse",
            row.line, row.dedup_seam
        ));
    }
}

fn validate_dedup_seam_uniqueness(
    row: &PlanRow,
    seen: &mut BTreeMap<String, usize>,
    failures: &mut Vec<String>,
) {
    let normalized = row.dedup_seam.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(first_line) = seen.insert(normalized.clone(), row.line) {
        failures.push(format!(
            "line {}: dedup seam `{normalized}` duplicates line {first_line}; each VX row needs a distinct ownership seam",
            row.line
        ));
    }
}

fn looks_like_local_path(token: &str) -> bool {
    token == "Cargo.toml"
        || token.contains('/')
        || token.ends_with(".rs")
        || token.ends_with(".md")
        || token.ends_with(".toml")
}

fn vx_number(id: &str) -> Option<u32> {
    let digits = id.strip_prefix("VX-")?;
    if digits.len() != 3 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
}

fn axis_shape_is_valid(axis: &str) -> bool {
    !axis.is_empty()
        && axis
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn backtick_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let token = after_start[..end].trim();
        if !token.is_empty() {
            tokens.push(token.to_string());
        }
        rest = &after_start[end + 1..];
    }
    tokens
}

#[derive(Debug, Serialize)]
struct PlanProgressArtifact {
    schema_version: u32,
    source_plan: &'static str,
    linked_release_artifact: &'static str,
    source_fingerprint: String,
    freshness_fingerprint: String,
    row_count: usize,
    research_grounded_row_count: usize,
    axis_row_counts: BTreeMap<String, usize>,
    research_key_counts: BTreeMap<String, usize>,
    dedup_seam_count: usize,
    duplicate_dedup_seam_count: usize,
    duplicate_dedup_seams: Vec<DuplicateDedupSeam>,
    evidence_path_count: usize,
    duplicate_evidence_path_count: usize,
    duplicate_evidence_paths: Vec<DuplicateEvidencePath>,
    rows: Vec<PlanProgressRow>,
}

#[derive(Debug, Serialize)]
struct DuplicateDedupSeam {
    seam: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct DuplicateEvidencePath {
    path: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct PlanProgressRow {
    id: String,
    axis: String,
    evidence_paths: Vec<String>,
    research_keys: Vec<String>,
    proof_gate: String,
    dedup_seam: String,
    status: &'static str,
    linked_release_artifact: &'static str,
}

fn write_plan_progress_json(path: &Path, report: &PlanGateReport) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let artifact = plan_progress_artifact(report);
    let json = serde_json::to_string_pretty(&artifact)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, format!("{json}\n"))
}

fn plan_progress_artifact(report: &PlanGateReport) -> PlanProgressArtifact {
    let source_fingerprint = plan_progress_source_fingerprint(report);
    let freshness_fingerprint = plan_progress_freshness_fingerprint(&source_fingerprint);
    PlanProgressArtifact {
        schema_version: PLAN_PROGRESS_SCHEMA_VERSION,
        source_plan: DEFAULT_PLAN,
        linked_release_artifact: PLAN_PROGRESS_ARTIFACT,
        source_fingerprint,
        freshness_fingerprint,
        row_count: report.row_count,
        research_grounded_row_count: research_grounded_row_count(report),
        axis_row_counts: axis_row_counts(report),
        research_key_counts: research_key_counts(report),
        dedup_seam_count: dedup_seam_count(report),
        duplicate_dedup_seam_count: duplicate_dedup_seams(report).len(),
        duplicate_dedup_seams: duplicate_dedup_seams(report),
        evidence_path_count: evidence_path_count(report),
        duplicate_evidence_path_count: duplicate_evidence_paths(report).len(),
        duplicate_evidence_paths: duplicate_evidence_paths(report),
        rows: report
            .rows
            .iter()
            .map(|row| PlanProgressRow {
                id: row.id.clone(),
                axis: row.axis.clone(),
                evidence_paths: evidence_paths_for_progress(row),
                research_keys: backtick_research_keys(&row.research_basis),
                proof_gate: row.proof_gate.clone(),
                dedup_seam: row.dedup_seam.clone(),
                status: "active",
                linked_release_artifact: PLAN_PROGRESS_ARTIFACT,
            })
            .collect(),
    }
}

pub(crate) fn validate_plan_progress_artifact_bytes(bytes: &[u8]) -> Vec<String> {
    let mut blockers = Vec::new();
    let value = match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(value) => value,
        Err(error) => {
            return vec![format!("plan progress artifact is not valid JSON: {error}")];
        }
    };
    if value.get("schema_version").and_then(|raw| raw.as_u64())
        != Some(u64::from(PLAN_PROGRESS_SCHEMA_VERSION))
    {
        blockers.push(format!(
            "plan progress artifact must use schema_version={PLAN_PROGRESS_SCHEMA_VERSION}"
        ));
    }
    let row_count = value.get("row_count").and_then(|raw| raw.as_u64());
    let research_grounded_row_count = value
        .get("research_grounded_row_count")
        .and_then(|raw| raw.as_u64());
    let dedup_seam_count = value.get("dedup_seam_count").and_then(|raw| raw.as_u64());
    let duplicate_dedup_seam_count = value
        .get("duplicate_dedup_seam_count")
        .and_then(|raw| raw.as_u64());
    if row_count.is_none() {
        blockers.push("plan progress artifact must contain row_count".to_string());
    }
    if row_count == Some(0) {
        blockers.push("plan progress row_count must be greater than zero".to_string());
    }
    if let Some(row_count) = row_count {
        if row_count < VX_PLAN_MIN_ROWS as u64 {
            blockers.push(format!(
                "plan progress row_count must be at least {VX_PLAN_MIN_ROWS}"
            ));
        }
    }
    let rows = value.get("rows").and_then(|raw| raw.as_array());
    let rows_len = rows
        .map(|rows| rows.len() as u64);
    if rows_len != row_count {
        blockers.push("plan progress row_count must match rows length".to_string());
    }
    if research_grounded_row_count.is_none() {
        blockers.push(
            "plan progress artifact must contain research_grounded_row_count".to_string(),
        );
    }
    if research_grounded_row_count != row_count {
        blockers.push(
            "plan progress research_grounded_row_count must match row_count".to_string(),
        );
    }
    if let Some(rows) = rows {
        for (index, row) in rows.iter().enumerate() {
            let id = row
                .get("id")
                .and_then(|raw| raw.as_str())
                .unwrap_or_default();
            if !id.starts_with("VX-") {
                blockers.push(format!("plan progress rows[{index}].id must be a VX id"));
            }
            for field in ["axis", "proof_gate", "dedup_seam"] {
                if row
                    .get(field)
                    .and_then(|raw| raw.as_str())
                    .unwrap_or_default()
                    .is_empty()
                {
                    blockers.push(format!("plan progress rows[{index}].{field} is missing"));
                }
            }
            if row.get("status").and_then(|raw| raw.as_str()) != Some("active") {
                blockers.push(format!("plan progress rows[{index}].status must be active"));
            }
            if row
                .get("linked_release_artifact")
                .and_then(|raw| raw.as_str())
                != Some(PLAN_PROGRESS_ARTIFACT)
            {
                blockers.push(format!(
                    "plan progress rows[{index}].linked_release_artifact must be {PLAN_PROGRESS_ARTIFACT}"
                ));
            }
        }
    }
    if dedup_seam_count != row_count {
        blockers.push("plan progress dedup_seam_count must match row_count".to_string());
    }
    let duplicate_dedup_seams = value
        .get("duplicate_dedup_seams")
        .and_then(|raw| raw.as_array());
    if duplicate_dedup_seam_count.is_none() {
        blockers.push(
            "plan progress artifact must contain duplicate_dedup_seam_count".to_string(),
        );
    }
    if duplicate_dedup_seams.is_none() {
        blockers.push("plan progress duplicate_dedup_seams must be an array".to_string());
    }
    if duplicate_dedup_seam_count != duplicate_dedup_seams.map(|seams| seams.len() as u64) {
        blockers.push(
            "plan progress duplicate_dedup_seam_count must match duplicate_dedup_seams length"
                .to_string(),
        );
    }
    if let Some(seams) = duplicate_dedup_seams {
        for (index, seam) in seams.iter().enumerate() {
            if seam
                .get("seam")
                .and_then(|raw| raw.as_str())
                .unwrap_or_default()
                .is_empty()
            {
                blockers.push(format!(
                    "plan progress duplicate_dedup_seams[{index}].seam is missing"
                ));
            }
            if seam
                .get("count")
                .and_then(|raw| raw.as_u64())
                .unwrap_or_default()
                < 2
            {
                blockers.push(format!(
                    "plan progress duplicate_dedup_seams[{index}].count must be at least 2"
                ));
            }
        }
    }
    let evidence_path_count = value
        .get("evidence_path_count")
        .and_then(|raw| raw.as_u64())
        .unwrap_or_default();
    if evidence_path_count == 0 {
        blockers.push("plan progress evidence_path_count must be greater than zero".to_string());
    }
    let duplicate_evidence_path_count = value
        .get("duplicate_evidence_path_count")
        .and_then(|raw| raw.as_u64());
    let duplicate_evidence_paths = value
        .get("duplicate_evidence_paths")
        .and_then(|raw| raw.as_array());
    if duplicate_evidence_path_count.is_none() {
        blockers.push(
            "plan progress artifact must contain duplicate_evidence_path_count".to_string(),
        );
    }
    if duplicate_evidence_paths.is_none() {
        blockers.push("plan progress duplicate_evidence_paths must be an array".to_string());
    }
    if duplicate_evidence_path_count != duplicate_evidence_paths.map(|paths| paths.len() as u64) {
        blockers.push(
            "plan progress duplicate_evidence_path_count must match duplicate_evidence_paths length"
                .to_string(),
        );
    }
    if let Some(paths) = duplicate_evidence_paths {
        for (index, path) in paths.iter().enumerate() {
            if path
                .get("path")
                .and_then(|raw| raw.as_str())
                .unwrap_or_default()
                .is_empty()
            {
                blockers.push(format!(
                    "plan progress duplicate_evidence_paths[{index}].path is missing"
                ));
            }
            if path
                .get("count")
                .and_then(|raw| raw.as_u64())
                .unwrap_or_default()
                < 2
            {
                blockers.push(format!(
                    "plan progress duplicate_evidence_paths[{index}].count must be at least 2"
                ));
            }
        }
    }
    let axis_sum = value
        .get("axis_row_counts")
        .and_then(|raw| raw.as_object())
        .map(|counts| counts.values().filter_map(|raw| raw.as_u64()).sum::<u64>());
    if axis_sum != row_count {
        blockers.push("plan progress axis_row_counts must sum to row_count".to_string());
    }
    let research_key_count = value
        .get("research_key_counts")
        .and_then(|raw| raw.as_object())
        .map(|counts| counts.len())
        .unwrap_or_default();
    if research_key_count == 0 {
        blockers.push("plan progress research_key_counts must not be empty".to_string());
    }
    blockers
}

fn axis_row_counts(report: &PlanGateReport) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in &report.rows {
        *counts.entry(row.axis.clone()).or_insert(0) += 1;
    }
    counts
}

fn research_key_counts(report: &PlanGateReport) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in &report.rows {
        for key in backtick_research_keys(&row.research_basis) {
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    counts
}

fn research_grounded_row_count(report: &PlanGateReport) -> usize {
    report
        .rows
        .iter()
        .filter(|row| row_is_research_grounded(row, &report.external_keys))
        .count()
}

fn row_is_research_grounded(row: &PlanRow, external_keys: &BTreeSet<String>) -> bool {
    row.research_basis.contains("Internal Vyre")
        || backtick_research_keys(&row.research_basis)
            .iter()
            .any(|key| external_keys.contains(key))
}

fn dedup_seam_count(report: &PlanGateReport) -> usize {
    report
        .rows
        .iter()
        .map(|row| row.dedup_seam.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

fn duplicate_dedup_seams(report: &PlanGateReport) -> Vec<DuplicateDedupSeam> {
    let mut counts = BTreeMap::<String, usize>::new();
    for row in &report.rows {
        let normalized = row.dedup_seam.split_whitespace().collect::<Vec<_>>().join(" ");
        *counts.entry(normalized).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .filter_map(|(seam, count)| {
            (count > 1).then_some(DuplicateDedupSeam { seam, count })
        })
        .collect()
}

fn duplicate_evidence_paths(report: &PlanGateReport) -> Vec<DuplicateEvidencePath> {
    let mut counts = BTreeMap::<String, usize>::new();
    for path in report.rows.iter().flat_map(evidence_paths_for_progress) {
        *counts.entry(path).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .filter_map(|(path, count)| {
            (count > 1).then_some(DuplicateEvidencePath { path, count })
        })
        .collect()
}

fn evidence_path_count(report: &PlanGateReport) -> usize {
    report
        .rows
        .iter()
        .flat_map(evidence_paths_for_progress)
        .collect::<BTreeSet<_>>()
        .len()
}

fn plan_progress_source_fingerprint(report: &PlanGateReport) -> String {
    let mut material = format!(
        "plan-progress-source:v4\nsource_plan={DEFAULT_PLAN}\nlinked_release_artifact={PLAN_PROGRESS_ARTIFACT}\nrow_count={}\nresearch_grounded_row_count={}\ndedup_seam_count={}\nduplicate_dedup_seam_count={}\nevidence_path_count={}\nduplicate_evidence_path_count={}\n",
        report.row_count,
        research_grounded_row_count(report),
        dedup_seam_count(report),
        duplicate_dedup_seams(report).len(),
        evidence_path_count(report),
        duplicate_evidence_paths(report).len()
    );
    for duplicate in duplicate_dedup_seams(report) {
        material.push_str("duplicate_dedup_seam=");
        material.push_str(&duplicate.seam);
        material.push('=');
        material.push_str(&duplicate.count.to_string());
        material.push('\n');
    }
    for duplicate in duplicate_evidence_paths(report) {
        material.push_str("duplicate_evidence_path=");
        material.push_str(&duplicate.path);
        material.push('=');
        material.push_str(&duplicate.count.to_string());
        material.push('\n');
    }
    for (axis, count) in axis_row_counts(report) {
        material.push_str("axis_count=");
        material.push_str(&axis);
        material.push('=');
        material.push_str(&count.to_string());
        material.push('\n');
    }
    for (key, count) in research_key_counts(report) {
        material.push_str("research_key_count=");
        material.push_str(&key);
        material.push('=');
        material.push_str(&count.to_string());
        material.push('\n');
    }
    for key in &report.external_keys {
        material.push_str("research_key=");
        material.push_str(key);
        material.push('\n');
    }
    for row in &report.rows {
        material.push_str("row=");
        material.push_str(&row.id);
        material.push('|');
        material.push_str(&row.axis);
        material.push('|');
        material.push_str(&row.local_evidence);
        material.push('|');
        material.push_str(&row.research_basis);
        material.push('|');
        material.push_str(&row.work);
        material.push('|');
        material.push_str(&row.proof_gate);
        material.push('|');
        material.push_str(&row.dedup_seam);
        material.push('\n');
    }
    format!(
        "plan-progress-source:v4:{}",
        sha256_hex(material.as_bytes())
    )
}

fn plan_progress_freshness_fingerprint(source_fingerprint: &str) -> String {
    let material = format!(
        "plan-progress-freshness:v4\nsource_plan={DEFAULT_PLAN}\nlinked_release_artifact={PLAN_PROGRESS_ARTIFACT}\nsource_fingerprint={source_fingerprint}\n"
    );
    format!(
        "plan-progress-freshness:v4:{}",
        sha256_hex(material.as_bytes())
    )
}

fn evidence_paths_for_progress(row: &PlanRow) -> Vec<String> {
    if row.local_evidence.starts_with("This file") {
        return vec![DEFAULT_PLAN.to_string()];
    }
    backtick_tokens(&row.local_evidence)
        .into_iter()
        .filter(|token| looks_like_local_path(token))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_PLAN: &str = r#"# Vyre all-axes acceleration plan

## External research basis

| Key | Source | Use in this plan |
| --- | --- | --- |
| `MLIR_PASS` | <https://mlir.llvm.org/docs/PassManagement/> | Pass gates. |

## Evidence-backed plan items

| ID | Axis | Local evidence | Research basis | Work | Proof gate | Dedup seam |
| --- | --- | --- | --- | --- | --- | --- |
| VX-001 | coordination | This file contained generated plan label appendices | `MLIR_PASS` | Fix: enforce grounded plan rows. | Gate test rejects malformed rows. | This file owns the synthetic plan. |
| VX-002 | coordination | `docs/optimization/OWNERSHIP.toml` | Internal Vyre evidence contract | Fix: enforce claim shape. | Claim audit rejects broad rows. | `OWNERSHIP.toml`. |
| VX-003 | coordination | `docs/optimization/HOT_PATHS.toml` | `MLIR_PASS` | Fix: enforce hot paths. | Gate test rejects gaps. | `HOT_PATHS.toml`. |
| VX-004 | coordination | `docs/optimization/OP_MATRIX.toml` | `MLIR_PASS` | Fix: enforce op rows. | Gate test rejects gaps. | `OP_MATRIX.toml`. |
| VX-005 | coordination | `docs/optimization/BENCH_TARGETS.toml` | `MLIR_PASS` | Fix: enforce bench rows. | Gate test rejects gaps. | `BENCH_TARGETS.toml`. |
| VX-006 | coordination | `docs/optimization/LEGACY_DOCS.md` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `LEGACY_DOCS.md`. |
| VX-007 | coordination | `docs/optimization/README.md` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `README.md`. |
| VX-008 | coordination | `docs/optimization/TAXONOMY.md` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `TAXONOMY.md`. |
| VX-009 | coordination | `docs/RECURSION_THESIS.md` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `RECURSION_THESIS.md`. |
| VX-010 | coordination | `docs/lego-block-rule.md` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `lego-block-rule.md`. |
| VX-011 | coordination | `Cargo.toml` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `Cargo.toml`. |
| VX-012 | coordination | `xtask/src/main.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `xtask`. |
| VX-013 | coordination | `xtask/README.md` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `xtask`. |
| VX-014 | coordination | `vyre-driver/src/evidence.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `evidence.rs`. |
| VX-015 | coordination | `vyre-driver/src/device_profile.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `device_profile.rs`. |
| VX-016 | coordination | `vyre-driver/src/observability.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `observability.rs`. |
| VX-017 | coordination | `vyre-lower/src/pre_emit.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `pre_emit.rs`. |
| VX-018 | coordination | `vyre-lower/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `lib.rs`. |
| VX-019 | coordination | `vyre-lower/src/emit_adversarial_corpus.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `emit_adversarial_corpus.rs`. |
| VX-020 | coordination | `vyre-emit-metal/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-emit-metal`. |
| VX-021 | coordination | `vyre-runtime/src/megakernel/ring.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `ring.rs`. |
| VX-022 | coordination | `vyre-runtime/src/megakernel/task.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `task.rs`. |
| VX-023 | coordination | `vyre-runtime/src/megakernel/scheduler.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `scheduler.rs`. |
| VX-024 | coordination | `vyre-runtime/src/megakernel/telemetry.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `telemetry.rs`. |
| VX-025 | coordination | `vyre-libs/src/security/facts.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `facts.rs`. |
| VX-026 | coordination | `vyre-libs/src/security/family_mask.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `family_mask.rs`. |
| VX-027 | coordination | `vyre-bench/src/registry/mod.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `registry`. |
| VX-028 | coordination | `vyre-bench/src/cases/release_workloads.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `release_workloads`. |
| VX-029 | coordination | `release/evidence/docs/cuda-release-path.md` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `release/evidence`. |
| VX-030 | coordination | `vyre-driver-cuda/src/backend/cuda_graph.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `cuda_graph.rs`. |
| VX-031 | coordination | `vyre-driver-cuda/src/backend/resident_dispatch.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `resident_dispatch.rs`. |
| VX-032 | coordination | `vyre-driver-metal/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-driver-metal`. |
| VX-033 | coordination | `vyre-driver-wgpu/tests/megakernel_emit.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-driver-wgpu`. |
| VX-034 | coordination | `vyre-foundation/src/optimizer/eqsat.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `eqsat.rs`. |
| VX-035 | coordination | `vyre-foundation/src/optimizer/fact_substrate.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `fact_substrate.rs`. |
| VX-036 | coordination | `vyre-foundation/src/optimizer/rewrite_proof.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `rewrite_proof.rs`. |
| VX-037 | coordination | `vyre-core/tests/wire_malformed_adversarial.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `wire_malformed_adversarial.rs`. |
| VX-038 | coordination | `vyre-libs/tests/rust_gpu_lexer_plan.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `rust_gpu_lexer_plan.rs`. |
| VX-039 | coordination | `xtask/src/hygiene_matrix.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `hygiene_matrix.rs`. |
| VX-040 | coordination | `xtask/src/op_matrix.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `op_matrix.rs`. |
| VX-041 | coordination | `xtask/src/hot_path_scan.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `hot_path_scan.rs`. |
| VX-042 | coordination | `xtask/src/recursion_gate.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `recursion_gate.rs`. |
| VX-043 | coordination | `xtask/src/release_workload_matrix.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `release_workload_matrix.rs`. |
| VX-044 | coordination | `xtask/src/test_matrix.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `test_matrix.rs`. |
| VX-045 | coordination | `xtask/src/docs_matrix.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `docs_matrix.rs`. |
| VX-046 | coordination | `xtask/src/vyre_weir_release_gate/mod.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `release_gate`. |
| VX-047 | coordination | `vyre-self-substrate/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-self-substrate`. |
| VX-048 | coordination | `vyre-primitives/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-primitives`. |
| VX-049 | coordination | `vyre-intrinsics/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-intrinsics`. |
| VX-050 | coordination | `vyre-reference/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-reference`. |
| VX-051 | coordination | `vyre-core/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-core`. |
| VX-052 | coordination | `vyre-driver-reference/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-driver-reference`. |
| VX-053 | coordination | `vyre-driver-spirv/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-driver-spirv`. |
| VX-054 | coordination | `vyre-frontend-c/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-frontend-c`. |
| VX-055 | coordination | `vyre-lints/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-lints`. |
| VX-056 | coordination | `vyre-aot/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-aot`. |
| VX-057 | coordination | `vyre-harness/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-harness`. |
| VX-058 | coordination | `vyre-spec/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-spec`. |
| VX-059 | coordination | `vyre-debug/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-debug`. |
| VX-060 | coordination | `vyre-macros/src/lib.rs` | `MLIR_PASS` | Fix: enforce docs. | Gate test rejects gaps. | `vyre-macros`. |
"#;

    #[test]
    fn valid_plan_rows_pass() {
        let text = generated_quality_plan(
            "`Cargo.toml` is rooted local evidence",
            "Fix: enforce grounded rows.",
            "Gate test rejects malformed rows.",
        );
        let report = validate_plan_text(&text);
        assert_eq!(report.failures, Vec::<String>::new());
        assert_eq!(report.row_count, VX_PLAN_MIN_ROWS);
    }

    #[test]
    fn local_evidence_path_without_observed_gap_fails_root_gate() {
        let row = test_row(
            "`Cargo.toml`",
            "`MLIR_PASS`",
            "Fix: enforce grounded rows.",
            "Gate test rejects malformed rows.",
        );
        let mut failures = Vec::new();
        validate_local_evidence(&row, Some(&default_vyre_root()), &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("local evidence must name an observed gap")),
            "expected observed-gap failure, got {failures:?}"
        );
    }

    #[test]
    fn plan_self_citation_fails_outside_plan_coordination_rows() {
        let row = PlanRow {
            line: 9,
            id: "VX-009".to_string(),
            axis: "driver_cuda".to_string(),
            local_evidence: "`docs/optimization/ALL_AXES_ACCELERATION_PLAN.md` has CUDA text"
                .to_string(),
            research_basis: "`MLIR_PASS`".to_string(),
            work: "Fix: enforce CUDA behavior.".to_string(),
            proof_gate: "Gate test rejects plan-only evidence.".to_string(),
            dedup_seam: "One CUDA seam.".to_string(),
        };
        let mut failures = Vec::new();
        validate_plan_self_citation(&row, &mut failures);
        assert!(failures
            .iter()
            .any(|failure| failure.contains("cites the active plan")));
    }

    #[test]
    fn progress_artifact_includes_machine_readable_row_fields() {
        let text = generated_quality_plan(
            "`Cargo.toml` is rooted local evidence",
            "Fix: enforce grounded rows.",
            "Gate test rejects malformed rows.",
        );
        let report = validate_plan_text(&text);
        let progress = plan_progress_artifact(&report);

        assert_eq!(progress.schema_version, PLAN_PROGRESS_SCHEMA_VERSION);
        assert_eq!(progress.linked_release_artifact, PLAN_PROGRESS_ARTIFACT);
        assert!(progress
            .source_fingerprint
            .starts_with("plan-progress-source:v4:"));
        assert!(progress
            .freshness_fingerprint
            .starts_with("plan-progress-freshness:v4:"));
        assert_eq!(
            progress.freshness_fingerprint,
            plan_progress_freshness_fingerprint(&progress.source_fingerprint)
        );
        assert_eq!(progress.row_count, VX_PLAN_MIN_ROWS);
        assert_eq!(progress.research_grounded_row_count, VX_PLAN_MIN_ROWS);
        assert_eq!(progress.dedup_seam_count, VX_PLAN_MIN_ROWS);
        assert_eq!(
            progress.duplicate_dedup_seam_count,
            progress.duplicate_dedup_seams.len()
        );
        assert!(progress
            .duplicate_dedup_seams
            .iter()
            .all(|duplicate| duplicate.count >= 2 && !duplicate.seam.is_empty()));
        assert!(progress.evidence_path_count >= 1);
        assert_eq!(
            progress.duplicate_evidence_path_count,
            progress.duplicate_evidence_paths.len()
        );
        assert!(progress
            .duplicate_evidence_paths
            .iter()
            .all(|duplicate| duplicate.count >= 2 && !duplicate.path.is_empty()));
        assert_eq!(
            progress
                .axis_row_counts
                .values()
                .copied()
                .sum::<usize>(),
            VX_PLAN_MIN_ROWS
        );
        assert_eq!(
            progress.research_key_counts.get("MLIR_PASS"),
            Some(&VX_PLAN_MIN_ROWS)
        );
        assert_eq!(progress.rows[0].id, "VX-001");
        assert_eq!(progress.rows[0].axis, "coordination");
        assert_eq!(
            progress.rows[0].evidence_paths,
            vec![DEFAULT_PLAN.to_string()]
        );
        assert_eq!(
            progress.rows[0].research_keys,
            vec!["MLIR_PASS".to_string()]
        );
        assert_eq!(progress.rows[0].status, "active");
        assert_eq!(
            progress.rows[0].linked_release_artifact,
            PLAN_PROGRESS_ARTIFACT
        );
    }

    #[test]
    fn plan_progress_artifact_validation_rejects_summary_drift() {
        let blockers = validate_plan_progress_artifact_bytes(
            b"{\"schema_version\":2,\"row_count\":3,\"dedup_seam_count\":2,\"evidence_path_count\":0,\"axis_row_counts\":{\"coordination\":1},\"research_key_counts\":{},\"rows\":[]}\n",
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("schema_version=4")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("row_count must match rows length")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("research_grounded_row_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("dedup_seam_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("duplicate_dedup_seam_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("evidence_path_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("duplicate_evidence_path_count")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("axis_row_counts")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("research_key_counts")));
    }

    #[test]
    fn innovation_candidate_requires_peer_baseline_and_comparison() {
        let row = test_row(
            "`Cargo.toml` has a planning seam",
            "`INTERNAL_GATE`",
            "Innovation candidate: invent a scheduler.",
            "Gate test rejects malformed rows.",
        );
        let mut failures = Vec::new();
        validate_innovation_candidate(&row, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("non-internal research or peer baseline")),
            "expected baseline failure, got {failures:?}"
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("comparison, baseline, parity, or benchmark")),
            "expected comparison failure, got {failures:?}"
        );
    }

    #[test]
    fn frontier_innovation_candidate_requires_falsification_tuple() {
        let mut row = test_row(
            "`Cargo.toml` has a planning seam",
            "`MLIR_PASS`",
            "Innovation candidate: benchmark a frontier planner.",
            "Bench reports throughput.",
        );
        row.id = "VX-421".to_string();
        let mut failures = Vec::new();
        validate_innovation_candidate(&row, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("falsification fields")),
            "expected falsification tuple failure, got {failures:?}"
        );
    }

    #[test]
    fn frontier_innovation_candidate_accepts_local_evidence_artifact_path() {
        let mut row = test_row(
            "`release/evidence/benchmarks/frontier-falsification.json` records the benchmark artifact.",
            "`MLIR_PASS`",
            "Innovation candidate: compare against baseline comparator on dataset corpus with throughput metric at least release floor.",
            "Gate rejects failure mode fallback.",
        );
        row.id = "VX-421".to_string();
        let mut failures = Vec::new();
        validate_innovation_candidate(&row, &mut failures);
        assert!(
            failures
                .iter()
                .all(|failure| !failure.contains("falsification fields")),
            "expected local evidence path to satisfy falsification artifact path, got {failures:?}"
        );
    }

    #[test]
    fn duplicate_dedup_seam_fails() {
        let mut first = test_row(
            "`Cargo.toml` has a planning seam",
            "`MLIR_PASS`",
            "Fix: enforce grounded rows.",
            "Gate test rejects malformed rows.",
        );
        first.line = 10;
        first.id = "VX-010".to_string();
        let mut second = first.clone();
        second.line = 11;
        second.id = "VX-011".to_string();
        let mut seen = BTreeMap::new();
        let mut failures = Vec::new();

        validate_dedup_seam_uniqueness(&first, &mut seen, &mut failures);
        validate_dedup_seam_uniqueness(&second, &mut seen, &mut failures);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("duplicates line 10")),
            "expected duplicate-seam failure, got {failures:?}"
        );
    }

    fn test_row(
        local_evidence: &str,
        research_basis: &str,
        work: &str,
        proof_gate: &str,
    ) -> PlanRow {
        PlanRow {
            line: 1,
            id: "VX-001".to_string(),
            axis: "coordination".to_string(),
            local_evidence: local_evidence.to_string(),
            research_basis: research_basis.to_string(),
            work: work.to_string(),
            proof_gate: proof_gate.to_string(),
            dedup_seam: "This file owns the plan.".to_string(),
        }
    }

    #[test]
    fn generated_appendix_marker_fails() {
        let plan = generated_quality_plan(
            "`Cargo.toml` is rooted local evidence",
            "Fix: enforce grounded rows.",
            "Gate test rejects malformed rows.",
        );
        let text = format!("{plan}\n## Ultra-scale research-grade expansion appendix\n");
        let report = validate_plan_text(&text);
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("generated appendix marker")));
    }

    #[test]
    fn missing_row_id_fails_contiguity() {
        let plan = generated_quality_plan(
            "`Cargo.toml` is rooted local evidence",
            "Fix: enforce grounded rows.",
            "Gate test rejects malformed rows.",
        );
        let text = plan.replace("| VX-010 |", "| VX-011 |");
        let report = validate_plan_text(&text);
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("missing VX-010")));
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("duplicate row id `VX-011`")));
    }

    #[test]
    fn unknown_research_key_fails() {
        let plan = generated_quality_plan(
            "`Cargo.toml` is rooted local evidence",
            "Fix: enforce grounded rows.",
            "Gate test rejects malformed rows.",
        );
        let text = plan.replacen(
            "| VX-001 | coordination | `Cargo.toml` is rooted local evidence | `MLIR_PASS` |",
            "| VX-001 | coordination | `Cargo.toml` is rooted local evidence | `UNKNOWN_KEY` |",
            1,
        );
        let report = validate_plan_text(&text);
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("UNKNOWN_KEY")));
    }

    #[test]
    fn external_research_url_mismatch_fails_root_gate() {
        let plan = generated_quality_plan(
            "`Cargo.toml` is rooted local evidence",
            "Fix: enforce grounded rows.",
            "Gate test rejects malformed rows.",
        );
        let (_key, url) = fixture_research_entries()
            .into_iter()
            .next()
            .expect("Fix: generated plan fixture must include at least one research key.");
        let text = plan.replacen(
            &format!("<{url}>"),
            "<https://example.invalid/mismatched-research-url/>",
            1,
        );
        let root = default_vyre_root();
        let report = validate_plan_text_with_root(&text, Some(&root));
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("does not match research source ledger URL")));
    }

    #[test]
    fn work_without_quality_prefix_fails() {
        let text = generated_quality_plan(
            "`Cargo.toml`",
            "Enforce docs.",
            "`./cargo_full test -p xtask acceleration_plan_gate` rejects malformed rows.",
        );
        let report = validate_plan_text(&text);
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("must start with `Fix:`")));
    }

    #[test]
    fn missing_local_evidence_file_fails_when_rooted() {
        let text = generated_quality_plan(
            "`missing/local/evidence.rs`",
            "Fix: enforce rooted local evidence.",
            "`./cargo_full test -p xtask acceleration_plan_gate` rejects missing files.",
        );
        let root = default_vyre_root();
        let report = validate_plan_text_with_root(&text, Some(&root));
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("missing/local/evidence.rs")));
    }

    #[test]
    fn rooted_existing_local_evidence_passes() {
        let text = generated_quality_plan(
            "`Cargo.toml` is rooted local evidence",
            "Improvement: enforce existing local evidence.",
            "`./cargo_full test -p xtask acceleration_plan_gate` accepts existing files.",
        );
        let root = default_vyre_root();
        let report = validate_plan_text_with_root(&text, Some(&root));
        assert_eq!(report.failures, Vec::<String>::new());
    }

    #[test]
    fn claim_audit_rejects_multi_lane_claim_without_seam() {
        let lanes = lanes(["coordination", "driver_cuda"]);
        let text = r#"
schema = 1

[[claim]]
owner = "agent"
status = "active"
lanes = ["coordination", "driver_cuda"]
scope = ["Touch two areas."]
proof_required = ["Run one test."]
"#;
        let failures = validate_claim_text(text, &lanes).err().unwrap_or_default();
        assert!(failures
            .iter()
            .any(|failure| failure.contains("without naming a seam or boundary")));
    }

    fn generated_quality_plan(evidence: &str, work: &str, proof: &str) -> String {
        let mut text = String::from(
            "# Vyre all-axes acceleration plan\n\n\
## External research basis\n\n\
| Key | Source | Use in this plan |\n\
| --- | --- | --- |\n\
",
        );
        for (key, url) in fixture_research_entries() {
            text.push_str("| `");
            text.push_str(&key);
            text.push_str("` | <");
            text.push_str(&url);
            text.push_str("> | Test research baseline. |\n");
        }
        text.push_str(
            "\n\
## Evidence-backed plan items\n\n\
| ID | Axis | Local evidence | Research basis | Work | Proof gate | Dedup seam |\n\
| --- | --- | --- | --- | --- | --- | --- |\n",
        );
        for id in 1..=VX_PLAN_MIN_ROWS {
            let axis = REQUIRED_AXES[(id - 1) % REQUIRED_AXES.len()];
            text.push_str(&format!(
                "| VX-{id:03} | {axis} | {evidence} | `MLIR_PASS` | {work} | {proof} | `Cargo.toml` owns synthetic test seam VX-{id:03}. |\n"
            ));
        }
        text
    }

    fn fixture_research_entries() -> BTreeMap<String, String> {
        let mut failures = Vec::new();
        let entries = parse_research_source_ledger(Some(&default_vyre_root()), &mut failures)
            .as_ref()
            .map(research_source_urls_by_key)
            .unwrap_or_default();
        if entries.is_empty() {
            return BTreeMap::from([(
                "MLIR_PASS".to_string(),
                "https://example.invalid/research/".to_string(),
            )]);
        }
        entries
    }

    #[test]
    fn claim_audit_accepts_multi_lane_claim_with_boundary() {
        let lanes = lanes(["coordination", "driver_cuda"]);
        let text = r#"
schema = 1

[[claim]]
owner = "agent"
status = "active"
lanes = ["coordination", "driver_cuda"]
scope = ["Wire the shared boundary between coordination evidence and CUDA proof."]
proof_required = ["Boundary test rejects drift."]
"#;
        assert_eq!(validate_claim_text(text, &lanes), Ok(()));
    }

    #[test]
    fn plan_axis_without_ownership_lane_fails() {
        let row = PlanRow {
            line: 1,
            id: "VX-001".to_string(),
            axis: "missing_lane".to_string(),
            local_evidence: "`Cargo.toml` has a test seam".to_string(),
            research_basis: "`MLIR_PASS`".to_string(),
            work: "Fix: enforce lane ownership.".to_string(),
            proof_gate: "Gate test rejects missing lane.".to_string(),
            dedup_seam: "One ownership map.".to_string(),
        };
        let tmp = tempfile::tempdir().unwrap();
        let ownership = tmp.path().join("OWNERSHIP.toml");
        std::fs::write(
            &ownership,
            r#"
[lane.coordination]
purpose = "Coordination."
write = ["docs/**"]
"#,
        )
        .unwrap();
        let mut failures = Vec::new();

        validate_plan_axes_have_lanes(&ownership, &[row], &mut failures);

        assert!(failures
            .iter()
            .any(|failure| failure.contains("missing_lane")));
    }

    #[test]
    fn supporting_ownership_lane_without_parent_axis_fails() {
        let row = PlanRow {
            line: 1,
            id: "VX-001".to_string(),
            axis: "coordination".to_string(),
            local_evidence: "`Cargo.toml` has a test seam".to_string(),
            research_basis: "`MLIR_PASS`".to_string(),
            work: "Fix: enforce lane ownership.".to_string(),
            proof_gate: "Gate test rejects missing lane.".to_string(),
            dedup_seam: "One ownership map.".to_string(),
        };
        let tmp = tempfile::tempdir().unwrap();
        let ownership = tmp.path().join("OWNERSHIP.toml");
        std::fs::write(
            &ownership,
            r#"
[lane.coordination]
purpose = "Coordination."
write = ["docs/**"]

[lane.driver_spirv]
purpose = "Experimental SPIR-V."
write = ["vyre-driver-spirv/src/**"]
"#,
        )
        .unwrap();
        let mut failures = Vec::new();

        validate_plan_axes_have_lanes(&ownership, &[row], &mut failures);

        assert!(failures
            .iter()
            .any(|failure| failure.contains("supporting ownership lane `driver_spirv`")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("lacks `parent_axis`")));
    }

    #[test]
    fn supporting_ownership_lane_with_parent_axis_passes() {
        let row = PlanRow {
            line: 1,
            id: "VX-001".to_string(),
            axis: "coordination".to_string(),
            local_evidence: "`Cargo.toml` has a test seam".to_string(),
            research_basis: "`MLIR_PASS`".to_string(),
            work: "Fix: enforce lane ownership.".to_string(),
            proof_gate: "Gate test rejects missing lane.".to_string(),
            dedup_seam: "One ownership map.".to_string(),
        };
        let tmp = tempfile::tempdir().unwrap();
        let ownership = tmp.path().join("OWNERSHIP.toml");
        std::fs::write(
            &ownership,
            r#"
[lane.coordination]
purpose = "Coordination."
write = ["docs/**"]

[lane.driver_spirv]
purpose = "Experimental SPIR-V."
parent_axis = "driver_shared"
support_reason = "Experimental SPIR-V backend ownership stays below shared backend contracts."
write = ["vyre-driver-spirv/src/**"]
"#,
        )
        .unwrap();
        let mut failures = Vec::new();

        validate_plan_axes_have_lanes(&ownership, &[row], &mut failures);

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn supporting_ownership_lane_with_unknown_parent_axis_fails() {
        let row = PlanRow {
            line: 1,
            id: "VX-001".to_string(),
            axis: "coordination".to_string(),
            local_evidence: "`Cargo.toml` has a test seam".to_string(),
            research_basis: "`MLIR_PASS`".to_string(),
            work: "Fix: enforce lane ownership.".to_string(),
            proof_gate: "Gate test rejects missing lane.".to_string(),
            dedup_seam: "One ownership map.".to_string(),
        };
        let tmp = tempfile::tempdir().unwrap();
        let ownership = tmp.path().join("OWNERSHIP.toml");
        std::fs::write(
            &ownership,
            r#"
[lane.coordination]
purpose = "Coordination."
write = ["docs/**"]

[lane.op_matrix]
purpose = "Op matrix."
parent_axis = "not_a_real_axis"
support_reason = "Op matrix supports coordination evidence without becoming an implementation axis."
write = ["docs/optimization/OP_MATRIX.toml"]
"#,
        )
        .unwrap();
        let mut failures = Vec::new();

        validate_plan_axes_have_lanes(&ownership, &[row], &mut failures);

        assert!(failures
            .iter()
            .any(|failure| failure.contains("unknown parent_axis `not_a_real_axis`")));
    }

    #[test]
    fn supporting_ownership_lane_with_weak_reason_fails() {
        let row = PlanRow {
            line: 1,
            id: "VX-001".to_string(),
            axis: "coordination".to_string(),
            local_evidence: "`Cargo.toml` has a test seam".to_string(),
            research_basis: "`MLIR_PASS`".to_string(),
            work: "Fix: enforce lane ownership.".to_string(),
            proof_gate: "Gate test rejects missing lane.".to_string(),
            dedup_seam: "One ownership map.".to_string(),
        };
        let tmp = tempfile::tempdir().unwrap();
        let ownership = tmp.path().join("OWNERSHIP.toml");
        std::fs::write(
            &ownership,
            r#"
[lane.coordination]
purpose = "Coordination."
write = ["docs/**"]

[lane.op_matrix]
purpose = "Op matrix."
parent_axis = "coordination"
support_reason = "Tiny reason."
write = ["docs/optimization/OP_MATRIX.toml"]
"#,
        )
        .unwrap();
        let mut failures = Vec::new();

        validate_plan_axes_have_lanes(&ownership, &[row], &mut failures);

        assert!(failures
            .iter()
            .any(|failure| failure.contains("too-short support_reason")));
    }

    #[test]
    fn claim_audit_rejects_active_unknown_lane_without_seam() {
        let lanes = lanes(["coordination"]);
        let text = r#"
schema = 1

[[claim]]
owner = "agent"
status = "active"
lanes = ["coordination", "random_lane"]
scope = ["Touch two areas."]
proof_required = ["Run one test."]
"#;
        let failures = validate_claim_text(text, &lanes).err().unwrap_or_default();
        assert!(failures
            .iter()
            .any(|failure| failure.contains("non-ownership lane")));
    }

    #[test]
    fn hot_path_audit_requires_vx003_surfaces() {
        let mut text = "schema = 1\n".to_string();
        for required in REQUIRED_HOT_PATHS {
            text.push_str("[[hot_path]]\nfile = \"");
            text.push_str(required);
            text.push_str("\"\nreason = \"contract\"\n");
        }
        assert_eq!(validate_hot_paths_text(&text), Ok(()));
        let missing = text.replace(
            "file = \"vyre-runtime/src/megakernel/ring.rs\"\nreason = \"contract\"\n",
            "",
        );
        let failures = validate_hot_paths_text(&missing).err().unwrap_or_default();
        assert!(failures
            .iter()
            .any(|failure| failure.contains("vyre-runtime/src/megakernel/ring.rs")));
    }

    fn lanes(values: impl IntoIterator<Item = &'static str>) -> BTreeSet<String> {
        values.into_iter().map(ToString::to_string).collect()
    }
}

const PLAN_LEDGER_SCAN_ROOTS: &[&str] = &["docs", "audits", "tools"];

const ACTIVE_LEDGER_SECTION_MARKERS: &[&str] = &[
    "## Evidence-backed plan items",
    "## Massive research-grade expansion appendix",
    "## Ultra-scale research-grade expansion appendix",
    "## 10,000+ test expansion program",
    "## 100,000+ test and validation expansion program",
    "## Implementation slice 10: ultra-scale 10000-label expansion",
];

fn validate_no_parallel_active_plan_files(vyre_root: &std::path::Path, failures: &mut Vec<String>) {
    for rel_root in PLAN_LEDGER_SCAN_ROOTS {
        let scan_root = vyre_root.join(rel_root);
        if scan_root.exists() {
            scan_parallel_plan_dir(vyre_root, &scan_root, failures);
        }
    }
}

fn scan_parallel_plan_dir(
    vyre_root: &std::path::Path,
    dir: &std::path::Path,
    failures: &mut Vec<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            failures.push(format!(
                "parallel plan scan could not read `{}`: {error}. Fix: make docs, audits, and tools readable before running acceleration-plan-gate.",
                relative_plan_path(vyre_root, dir)
            ));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(format!(
                    "parallel plan scan could not read a directory entry under `{}`: {error}. Fix: remove unreadable plan evidence entries or repair filesystem permissions.",
                    relative_plan_path(vyre_root, dir)
                ));
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                failures.push(format!(
                    "parallel plan scan could not classify `{}`: {error}. Fix: repair the unreadable plan path before running acceleration-plan-gate.",
                    relative_plan_path(vyre_root, &path)
                ));
                continue;
            }
        };

        if file_type.is_dir() {
            scan_parallel_plan_dir(vyre_root, &path, failures);
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
        {
            validate_parallel_plan_file(vyre_root, &path, failures);
        }
    }
}

fn validate_parallel_plan_file(
    vyre_root: &std::path::Path,
    path: &std::path::Path,
    failures: &mut Vec<String>,
) {
    let rel = relative_plan_path(vyre_root, path);
    if rel == "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md" {
        return;
    }

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "parallel plan scan could not read `{rel}`: {error}. Fix: make markdown plan evidence readable or remove stale unreadable plan files."
            ));
            return;
        }
    };

    if let Some(marker) = active_parallel_plan_marker(&text) {
        failures.push(format!(
            "parallel active plan marker `{marker}` found in `{rel}`; move active work to `docs/optimization/ALL_AXES_ACCELERATION_PLAN.md` and keep this file as evidence archive."
        ));
    }
}

fn active_parallel_plan_marker(text: &str) -> Option<&'static str> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("| VX-") {
            return Some("| VX-* table row");
        }
        for marker in ACTIVE_LEDGER_SECTION_MARKERS {
            if trimmed == *marker {
                return Some(marker);
            }
        }
    }
    None
}

fn relative_plan_path(vyre_root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(vyre_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod parallel_plan_source_of_truth_tests {
    use super::*;

    fn collect_parallel_plan_failures(root: &std::path::Path) -> Vec<String> {
        let mut failures = Vec::new();
        validate_no_parallel_active_plan_files(root, &mut failures);
        failures
    }

    #[test]
    fn parallel_active_plan_marker_fails_outside_source_plan() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(
            docs.join("old_plan.md"),
            "# Old plan\n\n## Evidence-backed plan items\n\n| VX-001 | stale active row |\n",
        )
        .unwrap();

        let failures = collect_parallel_plan_failures(tmp.path());

        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("parallel active plan marker"));
        assert!(failures[0].contains("docs/old_plan.md"));
    }

    #[test]
    fn source_truth_plan_is_ignored_by_parallel_scan() {
        let tmp = tempfile::tempdir().unwrap();
        let plan_dir = tmp.path().join("docs/optimization");
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::write(
            plan_dir.join("ALL_AXES_ACCELERATION_PLAN.md"),
            "# Source plan\n\n## Evidence-backed plan items\n\n| VX-001 | active source row |\n",
        )
        .unwrap();

        let failures = collect_parallel_plan_failures(tmp.path());

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn historical_plan_without_active_marker_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = tmp.path().join("docs/archive");
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(
            archive.join("old_plan.md"),
            "# Old plan\n\nThis file preserves historical roadmap context and closed proof references.\n",
        )
        .unwrap();

        let failures = collect_parallel_plan_failures(tmp.path());

        assert!(failures.is_empty(), "{failures:?}");
    }
}

#[cfg(test)]
mod claim_requirement_split_tests {
    use super::*;

    fn lanes() -> BTreeSet<String> {
        BTreeSet::from([
            "foundation_optimizer".to_string(),
            "driver_shared".to_string(),
        ])
    }

    #[test]
    fn active_claim_requires_seam_even_for_single_lane() {
        let text = r#"
[[claim]]
owner = "agent"
status = "active"
lanes = ["foundation_optimizer"]
scope = ["Rewrite optimizer helpers."]
proof_required = ["A proving test must pass."]
"#;

        let failures = validate_claim_text(text, &lanes())
            .err()
            .unwrap_or_default();

        assert!(failures
            .iter()
            .any(|failure| failure.contains("does not name the seam")));
    }

    #[test]
    fn active_claim_rejects_historical_proof_records() {
        let text = r#"
[[claim]]
owner = "agent"
status = "active"
lanes = ["foundation_optimizer"]
scope = ["Own the optimizer seam."]
proof_required = ["A proving test must pass."]
proof = ["A completed proof record belongs to status done."]
"#;

        let failures = validate_claim_text(text, &lanes())
            .err()
            .unwrap_or_default();

        assert!(failures
            .iter()
            .any(|failure| failure.contains("contains proof records")));
    }

    #[test]
    fn done_claim_is_historical_evidence_not_active_requirement() {
        let text = r#"
[[claim]]
owner = "agent"
status = "done"
lanes = ["foundation_optimizer"]
scope = ["Closed optimizer evidence."]
proof = ["A completed proof record is valid historical evidence."]
"#;

        assert_eq!(validate_claim_text(text, &lanes()), Ok(()));
    }
}

const DEPRECATED_ALIAS_IMPORTS: &[(&str, &str)] = &[
    ("crate::matching::", "crate::scan::"),
    ("vyre_libs::matching::", "vyre_libs::scan::"),
];

fn validate_compat_alias_audit(vyre_root: &std::path::Path, failures: &mut Vec<String>) {
    validate_compat_alias_registry(vyre_root, failures);
    let source_root = vyre_root.join("vyre-libs/src");
    if !source_root.exists() {
        failures.push(
            "compat alias audit could not find `vyre-libs/src`. Fix: run from the Vyre workspace root."
                .to_string(),
        );
        return;
    }

    for entry in walkdir::WalkDir::new(&source_root)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|ext| ext.to_str()) != Some("rs")
        {
            continue;
        }
        let rel = relative_plan_path(vyre_root, path);
        if compat_alias_path_allows_deprecated_imports(&rel) {
            continue;
        }
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                failures.push(format!(
                    "compat alias audit could not read `{rel}`: {error}. Fix: make source files readable before running acceleration-plan-gate."
                ));
                continue;
            }
        };
        for (line_index, line) in text.lines().enumerate() {
            if let Some(failure) = compat_alias_import_failure(&rel, line_index + 1, line) {
                failures.push(failure);
            }
        }
    }
}

fn validate_compat_alias_registry(vyre_root: &std::path::Path, failures: &mut Vec<String>) {
    let registry = vyre_root.join("vyre-libs/src/compat_aliases.rs");
    let registry_text = match std::fs::read_to_string(&registry) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "compat alias registry `vyre-libs/src/compat_aliases.rs` is missing or unreadable: {error}. Fix: keep compatibility metadata in one registry."
            ));
            return;
        }
    };
    for required in [
        "COMPATIBILITY_ALIASES",
        "MATCHING_ALIAS",
        "MATCHING_SUBSTRING_ALIAS",
        "deprecated_path",
        "canonical_path",
        "canonical_owner",
        "removal_condition",
    ] {
        if !registry_text.contains(required) {
            failures.push(format!(
                "compat alias registry is missing `{required}`. Fix: every compatibility shim must name deprecated path, canonical owner, and removal condition."
            ));
        }
    }

    let lib = vyre_root.join("vyre-libs/src/lib.rs");
    let lib_text = match std::fs::read_to_string(&lib) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "compat alias audit could not read `vyre-libs/src/lib.rs`: {error}. Fix: facade must expose the alias registry."
            ));
            return;
        }
    };
    if !lib_text.contains("pub mod compat_aliases;") {
        failures.push(
            "vyre-libs facade does not expose `compat_aliases`. Fix: register public compatibility shims through the alias registry."
                .to_string(),
        );
    }
}

fn compat_alias_import_failure(rel: &str, line_no: usize, line: &str) -> Option<String> {
    if compat_alias_path_allows_deprecated_imports(rel) {
        return None;
    }
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') {
        return None;
    }
    for (deprecated, canonical) in DEPRECATED_ALIAS_IMPORTS {
        if line.contains(deprecated) {
            return Some(format!(
                "deprecated alias import `{deprecated}` found in `{rel}:{line_no}`; use canonical `{canonical}` internally. Alias registry owner: `vyre-libs/src/compat_aliases.rs`."
            ));
        }
    }
    None
}

fn compat_alias_path_allows_deprecated_imports(rel: &str) -> bool {
    rel == "vyre-libs/src/lib.rs"
        || rel == "vyre-libs/src/compat_aliases.rs"
        || rel.starts_with("vyre-libs/src/matching/")
}

#[cfg(test)]
mod compat_alias_audit_tests {
    use super::*;

    #[test]
    fn deprecated_alias_import_fails_outside_compat_shim() {
        let failure = compat_alias_import_failure(
            "vyre-libs/src/scan/bad.rs",
            7,
            "use crate::matching::substring::substring_search;",
        )
        .unwrap();

        assert!(failure.contains("deprecated alias import"));
        assert!(failure.contains("crate::scan::"));
        assert!(failure.contains("compat_aliases.rs"));
    }

    #[test]
    fn compat_shim_path_may_reference_deprecated_alias() {
        assert!(compat_alias_import_failure(
            "vyre-libs/src/matching/substring/substring.rs",
            1,
            "const PATH: &str = \"vyre_libs::matching::substring\";",
        )
        .is_none());
    }

    #[test]
    fn comments_do_not_trip_alias_import_audit() {
        assert!(compat_alias_import_failure(
            "vyre-libs/src/scan/good.rs",
            3,
            "//! old docs mention vyre_libs::matching::substring",
        )
        .is_none());
    }
}

//! Source hygiene release evidence for Vyre and Weir.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
struct HygieneMatrix {
    schema_version: u32,
    scanned_roots: Vec<String>,
    scanned_files: usize,
    release_surface_coverage: ReleaseSurfaceCoverage,
    finding_summary: Vec<HygieneFindingSummary>,
    classification_summary: Vec<HygieneClassificationSummary>,
    intake_summary: Vec<HygieneIntakeSummary>,
    threshold_policy: ThresholdPolicyArtifact,
    finding_classes: Vec<HygieneFindingClass>,
    release_blocker_count: usize,
    findings: Vec<HygieneFinding>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ReleaseSurfaceCoverage {
    vyre_workspace: bool,
    cuda_driver_crate: bool,
    wgpu_driver_crate: bool,
    weir_crate: bool,
    vyrec_tool: bool,
    surgec_tool: bool,
    surgec_grammar_gen: bool,
    release_scripts: bool,
    github_workflows: bool,
    branch_protection_controls: bool,
    resource_bound_patterns: Vec<&'static str>,
    hidden_fallback_patterns: Vec<&'static str>,
    release_tooling_patterns: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct HygieneFinding {
    path: String,
    line: usize,
    pattern: &'static str,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
struct HygieneFindingSummary {
    pattern: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct HygieneFindingClass {
    path: String,
    line: usize,
    pattern: &'static str,
    owner_lane: &'static str,
    surface: &'static str,
    risk: &'static str,
    hot_path: bool,
    release_blocker: bool,
}

#[derive(Debug, Clone, Serialize)]
struct HygieneClassificationSummary {
    owner_lane: &'static str,
    surface: &'static str,
    risk: &'static str,
    hot_path: bool,
    release_blocker: bool,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct HygieneIntakeSummary {
    owner_lane: &'static str,
    surface: &'static str,
    risk: &'static str,
    hot_path: bool,
    pattern: &'static str,
    release_blocker: bool,
    count: usize,
}

#[derive(Debug, Serialize)]
struct HygieneIntakeArtifact {
    schema_version: u32,
    release_blocker_count: usize,
    intake_summary: Vec<HygieneIntakeSummary>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct HygieneScan {
    schema_version: u32,
    scan: String,
    findings: Vec<HygieneFinding>,
    release_blocking_findings: Vec<HygieneFindingClass>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ThresholdPolicyArtifact {
    schema_version: u32,
    source_manifest: &'static str,
    evidence_artifact: String,
    owner_lane: String,
    threshold_const_count: usize,
    registered_policy_count: usize,
    rows: Vec<ThresholdPolicyEvidenceRow>,
    findings: Vec<ThresholdPolicyFinding>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ThresholdPolicyEvidenceRow {
    id: String,
    path: String,
    line: usize,
    name: String,
    observed_value: String,
    unit: String,
    provenance: String,
    config_tier: String,
    override_path: String,
    evidence_link: String,
    release_rule: String,
}

#[derive(Debug, Clone, Serialize)]
struct ThresholdPolicyFinding {
    path: String,
    line: usize,
    name: String,
    finding: String,
    fix: String,
}

#[derive(Debug, Deserialize)]
struct ThresholdPolicyDocument {
    schema_version: u32,
    owner_lane: String,
    evidence_artifact: String,
    #[serde(default)]
    threshold: Vec<ThresholdPolicyTomlRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct ThresholdPolicyTomlRow {
    id: String,
    path: String,
    name: String,
    unit: String,
    provenance: String,
    config_tier: String,
    override_path: String,
    evidence_link: String,
    release_rule: String,
}

#[derive(Debug)]
struct ObservedThresholdConst {
    path: String,
    line: usize,
    name: String,
    value: String,
}

const BLOCKED_PATTERNS: &[(&str, &str)] = &[
    ("TODO", "TODO"),
    ("FIXME", "FIXME"),
    ("placeholder_text", "placeholder"),
    ("stub_text", "stub"),
    ("not_implemented_text", "not implemented"),
    ("todo_macro", "todo!("),
    ("unimplemented_macro", "unimplemented!("),
    ("panic_macro", "panic!("),
    ("unwrap_call", ".unwrap("),
    ("expect_call", concat!(".", "expect", "(")),
    ("std_thread_sleep", "std::thread::sleep"),
    ("thread_sleep", "thread::sleep"),
    ("tokio_sleep", "tokio::time::sleep"),
    ("silent_gpu_skip", "skip: no gpu"),
    ("silent_gpu_skipped", "skipped: no gpu"),
    ("cfg_not_gpu", "cfg(not(feature = \"gpu\"))"),
    ("cpu_fallback", "cpu fallback"),
    ("software_fallback", "software fallback"),
    ("fallback_dispatch", "fallback dispatch"),
    ("falling_back_to_cpu", "falling back to cpu"),
    ("fallback_to_cpu", "fallback to cpu"),
    ("synthetic_gpu_timing", "synthetic gpu timing"),
    ("fake_gpu_timing_formula", "cpu_ms * 0.01"),
];

const MAX_HYGIENE_SCAN_FILE_BYTES: u64 = 4_194_304;
const THRESHOLD_POLICY_SCHEMA_VERSION: u32 = 1;
const THRESHOLD_POLICY_SOURCE: &str = "docs/optimization/THRESHOLD_POLICY.toml";
const THRESHOLD_POLICY_ARTIFACT: &str = "release/evidence/hygiene/threshold-policy.json";
const THRESHOLD_POLICY_OWNER_LANE: &str = "testing_evidence";
const THRESHOLD_SUFFIXES: &[&str] = &[
    "_THRESHOLD",
    "_LIMIT",
    "_MAX",
    "_MIN",
    "_CAP",
    "_BUDGET",
    "_FLOOR",
    "_CEILING",
    "_TIMEOUT",
    "_DEADLINE",
    "_RETRY",
    "_BACKOFF",
];

pub(crate) fn run(args: &[String]) {
    let output = match parse_output(args) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let vyre_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let santh_root = vyre_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| vyre_root.clone());
    let roots = [vyre_root, santh_root.join("libs/dataflow/weir")];
    let optional_roots = [
        santh_root.join("tools/vyrec"),
        santh_root.join("libs/surge/surgec"),
        santh_root.join("libs/performance/matching/vyre/vyre-grammar-gen"),
    ];
    let mut scanned_roots = roots
        .iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>();
    scanned_roots.extend(
        optional_roots
            .iter()
            .filter(|root| root.exists())
            .map(|root| root.display().to_string()),
    );
    let mut scanned_files = 0usize;
    let mut findings = Vec::new();
    for root in &roots {
        scan_root(root, &mut scanned_files, &mut findings);
        scan_test_root(root, &mut scanned_files, &mut findings);
    }
    for root in &optional_roots {
        scan_optional_root(&root, &mut scanned_files, &mut findings);
        scan_optional_test_root(&root, &mut scanned_files, &mut findings);
    }
    scan_release_xtask(&roots[0], &mut scanned_files, &mut findings);
    scan_release_tooling(&roots[0], &mut scanned_files, &mut findings);
    scan_release_docs(&roots[0], &santh_root, &mut scanned_files, &mut findings);
    scan_santh_workflows(&santh_root, &mut scanned_files, &mut findings);
    scan_santh_release_controls(&santh_root, &mut scanned_files, &mut findings);
    for root in [
        roots[0].clone(),
        santh_root.join("libs/dataflow/weir"),
        santh_root.join("tools/vyrec"),
        santh_root.join("libs/surge/surgec"),
        santh_root.join("libs/performance/matching/vyre/vyre-grammar-gen"),
    ] {
        scan_audit_report_locations(&root, &mut scanned_files, &mut findings);
    }
    check_required_cargo_wrappers(&roots[0], &santh_root, &mut findings);
    let threshold_policy = collect_threshold_policy(&roots[0]);
    let release_surface_coverage = release_surface_coverage(&roots[0], &santh_root);
    let hot_paths = load_hot_path_files(&roots[0]);
    let finding_classes = classify_findings(&roots[0], &findings, &hot_paths);
    let release_blocker_count = finding_classes
        .iter()
        .filter(|finding| finding.release_blocker)
        .count();
    let mut blockers = if release_blocker_count == 0 {
        Vec::new()
    } else {
        vec![format!(
            "{release_blocker_count} release-blocking source hygiene finding(s) remain; {} total finding(s) preserved in classification output",
            findings.len()
        )]
    };
    blockers.extend(threshold_policy.blockers.iter().cloned());
    let finding_summary = finding_summary(&findings);
    let classification_summary = classification_summary(&finding_classes);
    let intake_summary = hygiene_intake_summary(&finding_classes);
    let matrix = HygieneMatrix {
        schema_version: 4,
        scanned_roots,
        scanned_files,
        release_surface_coverage,
        finding_summary,
        classification_summary,
        intake_summary,
        threshold_policy,
        finding_classes,
        release_blocker_count,
        findings,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize hygiene matrix: {error}");
            std::process::exit(1);
        }
    };
    if let Some(parent) = output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&output, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", output.display());
        std::process::exit(1);
    }
    write_sibling_artifacts(&output, &matrix);
    println!("hygiene-matrix: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn finding_summary(findings: &[HygieneFinding]) -> Vec<HygieneFindingSummary> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for finding in findings {
        *counts.entry(finding.pattern.to_string()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(pattern, count)| HygieneFindingSummary { pattern, count })
        .collect()
}

fn classify_findings(
    vyre_root: &Path,
    findings: &[HygieneFinding],
    hot_paths: &std::collections::BTreeSet<String>,
) -> Vec<HygieneFindingClass> {
    findings
        .iter()
        .map(|finding| {
            let owner_lane = hygiene_owner_lane_for_path(&finding.path);
            let surface = hygiene_surface_for_path(&finding.path);
            let hot_path = hygiene_finding_is_hot_path(vyre_root, &finding.path, hot_paths);
            let risk = hygiene_risk(finding.pattern, surface, hot_path);
            HygieneFindingClass {
                path: finding.path.clone(),
                line: finding.line,
                pattern: finding.pattern,
                owner_lane,
                surface,
                risk,
                hot_path,
                release_blocker: risk == "release_blocker",
            }
        })
        .collect()
}

fn classification_summary(classes: &[HygieneFindingClass]) -> Vec<HygieneClassificationSummary> {
    let mut counts =
        BTreeMap::<(&'static str, &'static str, &'static str, bool, bool), usize>::new();
    for class in classes {
        *counts
            .entry((
                class.owner_lane,
                class.surface,
                class.risk,
                class.hot_path,
                class.release_blocker,
            ))
            .or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(
            |((owner_lane, surface, risk, hot_path, release_blocker), count)| {
                HygieneClassificationSummary {
                    owner_lane,
                    surface,
                    risk,
                    hot_path,
                    release_blocker,
                    count,
                }
            },
        )
        .collect()
}

fn hygiene_intake_summary(classes: &[HygieneFindingClass]) -> Vec<HygieneIntakeSummary> {
    let mut counts = BTreeMap::<
        (
            &'static str,
            &'static str,
            &'static str,
            bool,
            &'static str,
            bool,
        ),
        usize,
    >::new();
    for class in classes {
        *counts
            .entry((
                class.owner_lane,
                class.surface,
                class.risk,
                class.hot_path,
                class.pattern,
                class.release_blocker,
            ))
            .or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(
            |((owner_lane, surface, risk, hot_path, pattern, release_blocker), count)| {
                HygieneIntakeSummary {
                    owner_lane,
                    surface,
                    risk,
                    hot_path,
                    pattern,
                    release_blocker,
                    count,
                }
            },
        )
        .collect()
}

fn hygiene_owner_lane_for_path(path: &str) -> &'static str {
    let normalized = path.replace('\\', "/");
    if normalized.contains("/libs/dataflow/weir/") {
        return "flow_weir";
    }
    if normalized.contains("/tools/vyrec/")
        || normalized.contains("/libs/surge/surgec/")
        || normalized.contains("/libs/performance/matching/vyre/vyre-grammar-gen/")
        || normalized.contains("/vyre-frontend-c/")
        || normalized.contains("/vyre-frontend-rust/")
        || normalized.contains("/vyre-libs/src/parsing/")
        || normalized.contains("/vyre-primitives/src/parsing/")
    {
        return "frontend_parsing";
    }
    if normalized.contains("/vyre-foundation/src/optimizer/")
        || normalized.contains("/vyre-foundation/src/transform/")
    {
        return "foundation_optimizer";
    }
    if normalized.contains("/vyre-foundation/src/serial/")
        || normalized.contains("/vyre-foundation/src/ir_inner/")
        || normalized.contains("/vyre-foundation/src/vast.rs")
        || normalized.contains("/vyre-foundation/fuzz/")
        || normalized.contains("/vyre-spec/")
        || normalized.contains("/vyre-libs/src/lib.rs")
        || normalized.contains("/vyre-libs/src/primitive_catalog.rs")
        || normalized.contains("/vyre-libs/src/intern/")
        || normalized.contains("/vyre-primitives/src/hash/")
        || normalized.contains("/vyre-primitives/src/wire.rs")
    {
        return "foundation_wire";
    }
    if normalized.contains("/vyre-driver-cuda/") {
        return "driver_cuda";
    }
    if normalized.contains("/vyre-driver-wgpu/") {
        return "driver_wgpu";
    }
    if normalized.contains("/vyre-driver-spirv/") {
        return "driver_spirv";
    }
    if normalized.contains("/vyre-driver-metal/") || normalized.contains("/vyre-emit-metal/") {
        return "driver_metal";
    }
    if normalized.contains("/vyre-driver/") {
        return "driver_shared";
    }
    if normalized.contains("/vyre-foundation/src/runtime/")
        || normalized.contains("/vyre-reference/")
        || normalized.contains("/vyre-intrinsics/")
    {
        return "driver_shared";
    }
    if normalized.contains("/vyre-lower/")
        || normalized.contains("/vyre-emit-naga/")
        || normalized.contains("/vyre-emit-ptx/")
        || normalized.contains("/vyre-emit-spirv/")
    {
        return "lower_emit";
    }
    if normalized.contains("/vyre-runtime/src/megakernel/") {
        return "runtime_megakernel";
    }
    if normalized.contains("/vyre-self-substrate/src/scheduling/")
        || normalized.contains("/vyre-self-substrate/src/hardware/")
        || normalized.contains("/vyre-runtime/src/")
    {
        return "runtime_megakernel";
    }
    if normalized.contains("/vyre-bench/") {
        return "bench_harness";
    }
    if normalized.contains("/vyre-libs/src/scan/")
        || normalized.contains("/vyre-libs/src/decode/")
        || normalized.contains("/vyre-libs/src/rule/")
        || normalized.contains("/vyre-self-substrate/src/data/")
        || normalized.contains("/vyre-primitives/src/matching/")
        || normalized.contains("/vyre-primitives/src/decode/")
        || normalized.contains("/vyre-primitives/src/nfa/")
    {
        return "scan_static";
    }
    if normalized.contains("/vyre-libs/src/security/")
        || normalized.contains("/vyre-libs/src/dataflow/")
        || normalized.contains("/vyre-libs/src/borrowck/")
        || normalized.contains("/vyre-self-substrate/src/analysis/")
        || normalized.contains("/vyre-self-substrate/src/graph/")
        || normalized.contains("/vyre-primitives/src/graph/")
        || normalized.contains("/vyre-primitives/src/fixpoint/")
        || normalized.contains("/vyre-primitives/src/predicate/")
        || normalized.contains("/vyre-primitives/src/bitset/")
    {
        return "security_dataflow";
    }
    if normalized.contains("/vyre-libs/src/nn/")
        || normalized.contains("/vyre-libs/src/math/")
        || normalized.contains("/vyre-primitives/src/math/")
    {
        return "nn_math";
    }
    if normalized.contains("/xtask/")
        || normalized.contains("/vyre-lints/")
        || normalized.contains("/vyre-libs/src/test_support/")
        || normalized.contains("/conform/")
        || normalized.contains("/release/evidence/")
        || normalized.contains("/docs/")
        || normalized.contains("/.github/")
        || normalized.contains("/scripts/")
    {
        return "testing_evidence";
    }
    "coordination"
}

fn hygiene_surface_for_path(path: &str) -> &'static str {
    let normalized = path.replace('\\', "/");
    if normalized.contains("/target/")
        || normalized.contains("/target-codex/")
        || normalized.contains("/release/evidence/")
        || normalized.contains("/__split/")
        || normalized.contains("/generated/")
    {
        return "generated";
    }
    if normalized.contains("/tests/")
        || normalized.contains("/fuzz/")
        || normalized.ends_with("/tests.rs")
        || normalized.ends_with("_test.rs")
        || normalized.ends_with("_tests.rs")
        || normalized.contains("_tests_")
        || normalized.contains("_test_")
        || is_cpu_parity_oracle_source(&normalized)
    {
        return "test";
    }
    if normalized.contains("/examples/") {
        return "example";
    }
    if normalized.ends_with(".md") || normalized.contains("/docs/") {
        return "docs";
    }
    if normalized.contains("/xtask/src/")
        || normalized.contains("/scripts/")
        || normalized.contains("/.github/")
    {
        return "release_tooling";
    }
    "production"
}

fn is_cpu_parity_oracle_source(normalized_path: &str) -> bool {
    normalized_path.ends_with("/cpu_oracle.rs")
        || normalized_path.ends_with("_cpu_oracle.rs")
        || normalized_path.ends_with("/bitset_closure_oracle.rs")
        || normalized_path.ends_with("/reaching/oracle.rs")
}

fn hygiene_risk(pattern: &str, surface: &str, hot_path: bool) -> &'static str {
    if surface == "generated" || surface == "example" {
        return "informational";
    }
    if surface == "test" || pattern.starts_with("test_") {
        return "test_hygiene";
    }
    if hot_path {
        return "release_blocker";
    }
    if matches!(
        pattern,
        "panic_macro"
            | "unwrap_call"
            | "expect_call"
            | "todo_macro"
            | "unimplemented_macro"
            | "not_implemented_text"
            | "unbounded_read"
            | "unreadable_source_file"
            | "unreadable_tooling_file"
            | "missing_cargo_wrapper"
            | "stray_audit_report"
    ) || is_hidden_fallback_pattern(pattern)
    {
        return "release_blocker";
    }
    if surface == "release_tooling"
        && matches!(
            pattern,
            "raw_workspace_cargo" | "invalid_cargo_full_xtask" | "heredoc"
        )
    {
        return "release_blocker";
    }
    if matches!(
        pattern,
        "TODO" | "FIXME" | "placeholder_text" | "stub_text" | "undocumented_public_api"
    ) {
        return "release_debt";
    }
    "informational"
}

fn load_hot_path_files(vyre_root: &Path) -> std::collections::BTreeSet<String> {
    let path = vyre_root.join("docs/optimization/HOT_PATHS.toml");
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return std::collections::BTreeSet::new(),
    };
    let value = match toml::from_str::<toml::Value>(&text) {
        Ok(value) => value,
        Err(_) => return std::collections::BTreeSet::new(),
    };
    value
        .get("hot_path")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("file").and_then(toml::Value::as_str))
        .map(ToString::to_string)
        .collect()
}

fn hygiene_finding_is_hot_path(
    vyre_root: &Path,
    path: &str,
    hot_paths: &std::collections::BTreeSet<String>,
) -> bool {
    let normalized = path.replace('\\', "/");
    let relative = Path::new(path)
        .strip_prefix(vyre_root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or(normalized);
    hot_paths.contains(&relative)
}

fn release_surface_coverage(vyre_root: &Path, santh_root: &Path) -> ReleaseSurfaceCoverage {
    ReleaseSurfaceCoverage {
        vyre_workspace: vyre_root.join("vyre-core").is_dir(),
        cuda_driver_crate: vyre_root.join("vyre-driver-cuda/src/lib.rs").is_file(),
        wgpu_driver_crate: vyre_root.join("vyre-driver-wgpu/src/lib.rs").is_file(),
        weir_crate: santh_root.join("libs/dataflow/weir/src/lib.rs").is_file(),
        vyrec_tool: santh_root.join("tools/vyrec/src").is_dir(),
        surgec_tool: santh_root.join("libs/surge/surgec/src").is_dir(),
        surgec_grammar_gen: santh_root
            .join("libs/performance/matching/vyre/vyre-grammar-gen/src")
            .is_dir(),
        release_scripts: santh_root
            .join("scripts/apply-branch-protection.sh")
            .is_file()
            && santh_root
                .join("scripts/architectural_invariants.sh")
                .is_file(),
        github_workflows: santh_root.join(".github/workflows").is_dir(),
        branch_protection_controls: santh_root.join(".github/CI_REQUIRED.md").is_file()
            && santh_root
                .join("scripts/apply-branch-protection.sh")
                .is_file(),
        resource_bound_patterns: vec![
            "std_thread_sleep",
            "thread_sleep",
            "tokio_sleep",
            "unbounded_read",
        ],
        hidden_fallback_patterns: vec![
            "silent_gpu_skip",
            "silent_gpu_skipped",
            "gpu_unavailable_skip",
            "cfg_not_gpu",
            "cpu_fallback",
            "software_fallback",
            "fallback_dispatch",
            "falling_back_to_cpu",
            "fallback_to_cpu",
            "synthetic_gpu_timing",
            "fake_gpu_timing_formula",
        ],
        release_tooling_patterns: vec![
            "raw_workspace_cargo",
            "invalid_cargo_full_xtask",
            "heredoc",
            "missing_cargo_wrapper",
        ],
    }
}

fn write_sibling_artifacts(output: &Path, matrix: &HygieneMatrix) {
    let Some(parent) = output.parent() else {
        eprintln!(
            "Fix: hygiene matrix output `{}` has no parent directory.",
            output.display()
        );
        std::process::exit(1);
    };
    let intake_blockers = if matrix.release_blocker_count == 0 {
        Vec::new()
    } else {
        vec![format!(
            "{} release-blocking hygiene finding(s) remain; implementation-intake.json groups them by owner lane, surface, risk, hot-path flag, and pattern",
            matrix.release_blocker_count
        )]
    };
    write_json(
        &parent.join("implementation-intake.json"),
        &HygieneIntakeArtifact {
            schema_version: 1,
            release_blocker_count: matrix.release_blocker_count,
            intake_summary: matrix.intake_summary.clone(),
            blockers: intake_blockers,
        },
    );
    write_json(&parent.join("threshold-policy.json"), &matrix.threshold_policy);
    for &(artifact, scan, patterns) in HYGIENE_SCANS {
        let findings = matrix
            .findings
            .iter()
            .filter(|finding| patterns.iter().any(|pattern| pattern == &finding.pattern))
            .cloned()
            .collect::<Vec<_>>();
        let release_blocking_findings = matrix
            .finding_classes
            .iter()
            .filter(|finding| {
                finding.release_blocker
                    && patterns
                        .iter()
                        .any(|pattern| *pattern == finding.pattern)
            })
            .cloned()
            .collect::<Vec<_>>();
        let blockers = if release_blocking_findings.is_empty() {
            Vec::new()
        } else {
            vec![format!(
                "{} release-blocking `{scan}` finding(s) remain",
                release_blocking_findings.len()
            )]
        };
        write_json(
            &parent.join(artifact),
            &HygieneScan {
                schema_version: 1,
                scan: scan.to_string(),
                findings,
                release_blocking_findings,
                blockers,
            },
        );
    }
}

fn collect_threshold_policy(vyre_root: &Path) -> ThresholdPolicyArtifact {
    let observed = scan_threshold_constants(vyre_root);
    let mut findings = Vec::new();
    let mut blockers = Vec::new();
    let policy_path = vyre_root.join(THRESHOLD_POLICY_SOURCE);
    let document = match fs::read_to_string(&policy_path) {
        Ok(text) => match toml::from_str::<ThresholdPolicyDocument>(&text) {
            Ok(document) => Some(document),
            Err(error) => {
                blockers.push(format!(
                    "{} is not valid threshold policy TOML: {error}. Fix: repair the TOML schema before release.",
                    THRESHOLD_POLICY_SOURCE
                ));
                None
            }
        },
        Err(error) => {
            blockers.push(format!(
                "missing {}: {error}. Fix: add unit, provenance, config tier, override path, evidence link, and release rule for every threshold-shaped const.",
                THRESHOLD_POLICY_SOURCE
            ));
            None
        }
    };
    let Some(document) = document else {
        return ThresholdPolicyArtifact {
            schema_version: THRESHOLD_POLICY_SCHEMA_VERSION,
            source_manifest: THRESHOLD_POLICY_SOURCE,
            evidence_artifact: THRESHOLD_POLICY_ARTIFACT.to_string(),
            owner_lane: THRESHOLD_POLICY_OWNER_LANE.to_string(),
            threshold_const_count: observed.len(),
            registered_policy_count: 0,
            rows: Vec::new(),
            findings,
            blockers,
        };
    };
    if document.schema_version != THRESHOLD_POLICY_SCHEMA_VERSION {
        blockers.push(format!(
            "{} schema_version={} must be {THRESHOLD_POLICY_SCHEMA_VERSION}. Fix: update the threshold policy reader and manifest together.",
            THRESHOLD_POLICY_SOURCE, document.schema_version
        ));
    }
    if document.owner_lane != THRESHOLD_POLICY_OWNER_LANE {
        blockers.push(format!(
            "{} owner_lane `{}` must be `{THRESHOLD_POLICY_OWNER_LANE}`. Fix: keep threshold evidence under the hygiene/testing lane.",
            THRESHOLD_POLICY_SOURCE, document.owner_lane
        ));
    }
    if document.evidence_artifact != THRESHOLD_POLICY_ARTIFACT {
        blockers.push(format!(
            "{} evidence_artifact `{}` must be `{THRESHOLD_POLICY_ARTIFACT}`. Fix: point the policy at the generated hygiene sibling artifact.",
            THRESHOLD_POLICY_SOURCE, document.evidence_artifact
        ));
    }
    let mut observed_by_key = BTreeMap::new();
    for threshold in observed {
        observed_by_key.insert(threshold_key(&threshold.path, &threshold.name), threshold);
    }
    let mut policy_by_key = BTreeMap::new();
    for row in &document.threshold {
        let row_key = threshold_key(&row.path, &row.name);
        if let Some(previous) = policy_by_key.insert(row_key.clone(), row.clone()) {
            blockers.push(format!(
                "{} duplicates threshold policy key `{}` for ids `{}` and `{}`. Fix: keep exactly one row per path/name threshold.",
                THRESHOLD_POLICY_SOURCE, row_key, previous.id, row.id
            ));
        }
        validate_threshold_policy_row(row, &mut blockers);
    }
    let mut rows = Vec::new();
    for (key, threshold) in &observed_by_key {
        let Some(policy) = policy_by_key.get(key) else {
            findings.push(ThresholdPolicyFinding {
                path: threshold.path.clone(),
                line: threshold.line,
                name: threshold.name.clone(),
                finding: "unregistered-threshold-const".to_string(),
                fix: format!(
                    "Fix: add `{}`/`{}` to {} with unit, provenance, config_tier, override_path, evidence_link, and release_rule.",
                    threshold.path, threshold.name, THRESHOLD_POLICY_SOURCE
                ),
            });
            blockers.push(format!(
                "{}:{} threshold const `{}` is missing from {}. Fix: register its unit, provenance, config tier, override path, evidence link, and VX release rule.",
                threshold.path, threshold.line, threshold.name, THRESHOLD_POLICY_SOURCE
            ));
            continue;
        };
        rows.push(ThresholdPolicyEvidenceRow {
            id: policy.id.clone(),
            path: threshold.path.clone(),
            line: threshold.line,
            name: threshold.name.clone(),
            observed_value: threshold.value.clone(),
            unit: policy.unit.clone(),
            provenance: policy.provenance.clone(),
            config_tier: policy.config_tier.clone(),
            override_path: policy.override_path.clone(),
            evidence_link: policy.evidence_link.clone(),
            release_rule: policy.release_rule.clone(),
        });
    }
    for (key, policy) in &policy_by_key {
        if !observed_by_key.contains_key(key) {
            findings.push(ThresholdPolicyFinding {
                path: policy.path.clone(),
                line: 1,
                name: policy.name.clone(),
                finding: "stale-threshold-policy-row".to_string(),
                fix: format!(
                    "Fix: remove or update stale threshold policy row `{}` after moving the source constant.",
                    policy.id
                ),
            });
            blockers.push(format!(
                "{} row `{}` points at `{}`/`{}` but no matching threshold const was observed. Fix: update or remove the stale policy row.",
                THRESHOLD_POLICY_SOURCE, policy.id, policy.path, policy.name
            ));
        }
    }
    rows.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line.cmp(&right.line))
            .then(left.name.cmp(&right.name))
    });
    ThresholdPolicyArtifact {
        schema_version: THRESHOLD_POLICY_SCHEMA_VERSION,
        source_manifest: THRESHOLD_POLICY_SOURCE,
        evidence_artifact: THRESHOLD_POLICY_ARTIFACT.to_string(),
        owner_lane: document.owner_lane,
        threshold_const_count: observed_by_key.len(),
        registered_policy_count: policy_by_key.len(),
        rows,
        findings,
        blockers,
    }
}

fn validate_threshold_policy_row(row: &ThresholdPolicyTomlRow, blockers: &mut Vec<String>) {
    for (field, value) in [
        ("id", row.id.as_str()),
        ("path", row.path.as_str()),
        ("name", row.name.as_str()),
        ("unit", row.unit.as_str()),
        ("provenance", row.provenance.as_str()),
        ("config_tier", row.config_tier.as_str()),
        ("override_path", row.override_path.as_str()),
        ("evidence_link", row.evidence_link.as_str()),
        ("release_rule", row.release_rule.as_str()),
    ] {
        if value.trim().is_empty() {
            blockers.push(format!(
                "{} row `{}` has blank {field}. Fix: every threshold policy row must carry unit, provenance, tier, override, evidence, and VX ownership.",
                THRESHOLD_POLICY_SOURCE, row.id
            ));
        }
    }
    if !matches!(row.config_tier.as_str(), "tier_a" | "tier_b" | "structural") {
        blockers.push(format!(
            "{} row `{}` uses config_tier `{}`. Fix: use `tier_a`, `tier_b`, or `structural`.",
            THRESHOLD_POLICY_SOURCE, row.id, row.config_tier
        ));
    }
    if row.config_tier == "tier_a"
        && !(row.override_path.contains("tool.toml") && row.override_path.contains("CLI"))
    {
        blockers.push(format!(
            "{} row `{}` is Tier A but override_path does not name tool.toml and CLI override behavior. Fix: record compiled default -> tool.toml -> CLI precedence.",
            THRESHOLD_POLICY_SOURCE, row.id
        ));
    }
    if row.config_tier == "tier_b" && !row.override_path.contains("TOML data") {
        blockers.push(format!(
            "{} row `{}` is Tier B but override_path does not name TOML data ownership. Fix: keep community/data thresholds out of CLI flags.",
            THRESHOLD_POLICY_SOURCE, row.id
        ));
    }
    if row.config_tier == "structural" && !row.override_path.contains("not operator configurable") {
        blockers.push(format!(
            "{} row `{}` is structural but override_path does not say `not operator configurable`. Fix: separate wire/ABI bounds from runtime knobs.",
            THRESHOLD_POLICY_SOURCE, row.id
        ));
    }
    if row.evidence_link != THRESHOLD_POLICY_ARTIFACT {
        blockers.push(format!(
            "{} row `{}` evidence_link `{}` must be `{THRESHOLD_POLICY_ARTIFACT}`.",
            THRESHOLD_POLICY_SOURCE, row.id, row.evidence_link
        ));
    }
    if row.release_rule != "VX-475" {
        blockers.push(format!(
            "{} row `{}` release_rule `{}` must be `VX-475`.",
            THRESHOLD_POLICY_SOURCE, row.id, row.release_rule
        ));
    }
}

fn scan_threshold_constants(vyre_root: &Path) -> Vec<ObservedThresholdConst> {
    let mut thresholds = Vec::new();
    for root in threshold_scan_roots(vyre_root) {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root).into_iter().filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(name.as_ref(), "target" | "target-codex" | "tests" | ".git")
        }) {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let Ok(text) = read_text_bounded(path) else {
                thresholds.push(ObservedThresholdConst {
                    path: relative_to_vyre(vyre_root, path),
                    line: 1,
                    name: "unreadable-threshold-source".to_string(),
                    value: "unreadable".to_string(),
                });
                continue;
            };
            for (line_index, line) in text.lines().enumerate() {
                let Some((name, value)) = parse_threshold_const(line) else {
                    continue;
                };
                thresholds.push(ObservedThresholdConst {
                    path: relative_to_vyre(vyre_root, path),
                    line: line_index + 1,
                    name,
                    value,
                });
            }
        }
    }
    thresholds
}

fn threshold_scan_roots(vyre_root: &Path) -> Vec<PathBuf> {
    [
        "vyre-foundation/src/optimizer",
        "vyre-runtime/src/megakernel",
        "vyre-driver-wgpu/src/runtime",
        "vyre-driver-wgpu/src/buffer",
    ]
    .iter()
    .map(|relative| vyre_root.join(relative))
    .collect()
}

fn parse_threshold_const(line: &str) -> Option<(String, String)> {
    let code = line.split("//").next().unwrap_or(line).trim();
    let const_index = code.find("const ")?;
    let rest = &code[const_index + "const ".len()..];
    let colon_index = rest.find(':')?;
    let name = rest[..colon_index].trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        || !THRESHOLD_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix))
    {
        return None;
    }
    let equals_index = rest.find('=')?;
    let value = rest[equals_index + 1..].split(';').next()?.trim();
    if !value.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

fn threshold_key(path: &str, name: &str) -> String {
    format!("{path}::{name}")
}

fn relative_to_vyre(vyre_root: &Path, path: &Path) -> String {
    path.strip_prefix(vyre_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

const HYGIENE_SCANS: &[(&str, &str, &[&str])] = &[
    (
        "no-stubs-scan.json",
        "no-stubs",
        &[
            "TODO",
            "FIXME",
            "placeholder_text",
            "stub_text",
            "not_implemented_text",
            "todo_macro",
            "unimplemented_macro",
        ],
    ),
    (
        "no-hidden-fallback-scan.json",
        "no-hidden-fallback",
        &[
            "silent_gpu_skip",
            "silent_gpu_skipped",
            "gpu_unavailable_skip",
            "cfg_not_gpu",
            "cpu_fallback",
            "software_fallback",
            "fallback_dispatch",
            "falling_back_to_cpu",
            "fallback_to_cpu",
            "synthetic_gpu_timing",
            "fake_gpu_timing_formula",
        ],
    ),
    (
        "resource-bound-scan.json",
        "resource-bound",
        &[
            "std_thread_sleep",
            "thread_sleep",
            "tokio_sleep",
            "unbounded_read",
        ],
    ),
    (
        "error-surface-scan.json",
        "error-surface",
        &["panic_macro", "unwrap_call", "expect_call"],
    ),
    (
        "cargo-wrapper-scan.json",
        "cargo-wrapper",
        &[
            "raw_workspace_cargo",
            "invalid_cargo_full_xtask",
            "heredoc",
            "missing_cargo_wrapper",
        ],
    ),
    (
        "audit-location-scan.json",
        "audit-location",
        &["stray_audit_report"],
    ),
    (
        "public-doc-scan.json",
        "public-docs",
        &["undocumented_public_api"],
    ),
    (
        "test-hygiene-scan.json",
        "test-hygiene",
        &[
            "test_TODO",
            "test_FIXME",
            "test_todo_macro",
            "test_unimplemented_macro",
            "test_ignored",
            "test_let_underscore_result",
            "test_assert_true",
        ],
    ),
];

fn write_json(path: &Path, value: &impl Serialize) {
    let json = match serde_json::to_string_pretty(value) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize `{}`: {error}", path.display());
            std::process::exit(1);
        }
    };
    if let Err(error) = fs::write(path, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

fn scan_root(root: &Path, scanned_files: &mut usize, findings: &mut Vec<HygieneFinding>) {
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy();
        !matches!(
            name.as_ref(),
            "target"
                | "target-codex"
                | "target_tests"
                | ".git"
                | ".cargo-target"
                | "release"
                | "xtask"
        )
    }) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                push_walk_error(root, &error, findings);
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("hygiene_matrix.rs") {
            continue;
        }
        let path_string = path.display().to_string();
        if path_string.contains("/tests/")
            || path_string.contains("/benches/")
            || path_string.contains("/examples/")
            || path_string.ends_with("/tests.rs")
            || path_string.ends_with("_test.rs")
            || path_string.ends_with("_tests.rs")
            || path_string.contains("_tests_")
            || path_string.contains("_test_")
        {
            continue;
        }
        scan_file(path, scanned_files, findings);
    }
}

fn scan_optional_root(root: &Path, scanned_files: &mut usize, findings: &mut Vec<HygieneFinding>) {
    if root.exists() {
        scan_root(root, scanned_files, findings);
    }
}

fn scan_test_root(root: &Path, scanned_files: &mut usize, findings: &mut Vec<HygieneFinding>) {
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy();
        !matches!(
            name.as_ref(),
            "target"
                | "target-codex"
                | "target_tests"
                | ".git"
                | ".cargo-target"
                | "release"
                | "xtask"
        )
    }) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                push_walk_error(root, &error, findings);
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let path_string = path.display().to_string();
        let is_test_file = path_string.contains("/tests/")
            || path_string.ends_with("/tests.rs")
            || path_string.ends_with("_test.rs")
            || path_string.ends_with("_tests.rs")
            || path_string.contains("_tests_")
            || path_string.contains("_test_");
        if is_test_file {
            scan_test_file(path, scanned_files, findings);
        }
    }
}

fn scan_optional_test_root(
    root: &Path,
    scanned_files: &mut usize,
    findings: &mut Vec<HygieneFinding>,
) {
    if root.exists() {
        scan_test_root(root, scanned_files, findings);
    }
}

fn scan_release_xtask(root: &Path, scanned_files: &mut usize, findings: &mut Vec<HygieneFinding>) {
    for module in [
        "backend_matrix",
        "c_parser_corpus",
        "conformance_matrix",
        "docs_matrix",
        "feature_matrix",
        "hygiene_matrix",
        "metadata_matrix",
        "optimization_corpus",
        "optimization_matrix",
        "parser_coherence",
        "release_benchmarks",
        "release_completion_audit",
        "release_conformance",
        "release_evidence",
        "release_gate",
        "test_matrix",
        "version_matrix",
        "vyre_weir_release_gate",
        "weir_matrix",
    ] {
        match crate::command_matrix::resolve_module_source(root, module) {
            Ok(path) => scan_file(&path, scanned_files, findings),
            Err(error) => findings.push(HygieneFinding {
                path: root
                    .join("xtask/src")
                    .join(format!("{module}.rs"))
                    .display()
                    .to_string(),
                line: 1,
                pattern: "unreadable_source_file",
                text: error,
            }),
        }
    }
}

fn scan_release_tooling(
    root: &Path,
    scanned_files: &mut usize,
    findings: &mut Vec<HygieneFinding>,
) {
    for relative_root in ["scripts", ".github/workflows", ".github/actions"] {
        let tooling_root = root.join(relative_root);
        if !tooling_root.exists() {
            continue;
        }
        for entry in WalkDir::new(&tooling_root)
            .into_iter()
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                !matches!(name.as_ref(), "target" | ".git")
            })
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    push_walk_error(&tooling_root, &error, findings);
                    continue;
                }
            };
            let path = entry.path();
            let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
                continue;
            };
            if matches!(extension, "sh" | "yml" | "yaml") {
                scan_tooling_file(path, scanned_files, findings);
            }
        }
    }
}

fn scan_release_docs(
    vyre_root: &Path,
    santh_root: &Path,
    scanned_files: &mut usize,
    findings: &mut Vec<HygieneFinding>,
) {
    for doc in [
        santh_root.join("docs/vyre-weir-release-plan.md"),
        vyre_root.join("README.md"),
        vyre_root.join("docs/RELEASE.md"),
        vyre_root.join("docs/RELEASE_ENGINEERING.md"),
        vyre_root.join("docs/RELEASE_CHECKLIST.md"),
        vyre_root.join("docs/PUBLISH_GATE.md"),
        vyre_root.join("docs/TESTING_PROGRAM.md"),
        vyre_root.join("docs/optimization/AGENT_CONTRACT.md"),
        vyre_root.join("conform/README.md"),
        vyre_root.join("vyre-bench/README.md"),
        vyre_root.join("vyre-frontend-c/README.md"),
        santh_root.join("tools/vyrec/README.md"),
        santh_root.join("libs/dataflow/weir/README.md"),
        santh_root.join("libs/dataflow/weir/VISION.md"),
    ] {
        if doc.is_file() {
            scan_doc_file(&doc, scanned_files, findings);
        }
    }
}

fn scan_santh_workflows(
    santh_root: &Path,
    scanned_files: &mut usize,
    findings: &mut Vec<HygieneFinding>,
) {
    let workflows = santh_root.join(".github/workflows");
    if !workflows.exists() {
        return;
    }
    for entry in WalkDir::new(&workflows).into_iter().filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy();
        !matches!(name.as_ref(), "target" | ".git")
    }) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                push_walk_error(&workflows, &error, findings);
                continue;
            }
        };
        let path = entry.path();
        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if matches!(extension, "yml" | "yaml") {
            scan_tooling_file(path, scanned_files, findings);
        }
    }
}

fn scan_audit_report_locations(
    root: &Path,
    scanned_files: &mut usize,
    findings: &mut Vec<HygieneFinding>,
) {
    if !root.exists() {
        return;
    }
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy();
        !matches!(
            name.as_ref(),
            "target" | "target-codex" | "target_tests" | ".git" | ".cargo-target" | "release"
        )
    }) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                push_walk_error(root, &error, findings);
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(file_name.starts_with("AUDIT")
            || file_name.starts_with("FINDINGS")
            || file_name.starts_with("PLAN"))
        {
            continue;
        }
        *scanned_files += 1;
        let normalized = path.to_string_lossy();
        if !normalized.contains("/.audits/") && !normalized.contains("/audits/") {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: 1,
                pattern: "stray_audit_report",
                text: "audit, findings, and plan reports must live under .audits/".to_string(),
            });
        }
    }
}

fn check_required_cargo_wrappers(
    vyre_root: &Path,
    _santh_root: &Path,
    findings: &mut Vec<HygieneFinding>,
) {
    for path in [vyre_root.join("cargo_full")] {
        if !path.is_file() {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: 1,
                pattern: "missing_cargo_wrapper",
                text: "required bounded cargo_full wrapper is missing".to_string(),
            });
        }
    }
}

fn scan_santh_release_controls(
    santh_root: &Path,
    scanned_files: &mut usize,
    findings: &mut Vec<HygieneFinding>,
) {
    let required_status_doc = santh_root.join(".github/CI_REQUIRED.md");
    if required_status_doc.is_file() {
        scan_doc_file(&required_status_doc, scanned_files, findings);
    }
    for script in [
        "scripts/apply-branch-protection.sh",
        "scripts/architectural_invariants.sh",
    ] {
        let path = santh_root.join(script);
        if path.is_file() {
            scan_tooling_file(&path, scanned_files, findings);
        }
    }
}

fn scan_file(path: &Path, scanned_files: &mut usize, findings: &mut Vec<HygieneFinding>) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            push_read_error(path, "unreadable_source_file", error, findings);
            return;
        }
    };
    *scanned_files += 1;
    let mut pending_cfg_test = false;
    let mut pending_test_attr = false;
    let mut test_module_depth = 0usize;
    let mut skipping_cfg_test_item = false;
    let mut cfg_test_item_depth = 0usize;
    let mut pending_bounded_read_chain = false;
    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        let bounded_read_chain = pending_bounded_read_chain || trimmed.contains(".take(");
        if trimmed.contains(".take(") && !line_contains_read_call(line) {
            pending_bounded_read_chain = true;
        }
        if skipping_cfg_test_item {
            if cfg_test_item_depth == 0 {
                if trimmed.contains('{') {
                    cfg_test_item_depth = update_brace_depth(0, line);
                    if cfg_test_item_depth == 0 {
                        skipping_cfg_test_item = false;
                    }
                } else if trimmed.ends_with(';') {
                    skipping_cfg_test_item = false;
                }
            } else {
                cfg_test_item_depth = update_brace_depth(cfg_test_item_depth, line);
                if cfg_test_item_depth == 0 {
                    skipping_cfg_test_item = false;
                }
            }
            continue;
        }
        if test_module_depth > 0 {
            test_module_depth = update_brace_depth(test_module_depth, line);
            continue;
        }
        if pending_cfg_test {
            if trimmed.contains('{') {
                test_module_depth = update_brace_depth(0, line);
                if test_module_depth == 0 {
                    test_module_depth = 0;
                }
            } else {
                skipping_cfg_test_item = true;
                cfg_test_item_depth = 0;
            }
            pending_cfg_test = false;
            continue;
        }
        if pending_test_attr && trimmed.starts_with("fn ") && trimmed.contains('{') {
            test_module_depth = update_brace_depth(0, line);
            pending_test_attr = false;
            continue;
        }
        if pending_test_attr && trimmed.starts_with("#[") {
            continue;
        }
        pending_cfg_test = is_non_release_cfg_attr(trimmed);
        pending_test_attr = trimmed == "#[test]"
            || trimmed.starts_with("#[tokio::test")
            || trimmed.starts_with("#[should_panic");
        let lower = line.to_ascii_lowercase();
        if line_contains_raw_workspace_cargo(line) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "raw_workspace_cargo",
                text: line.trim().to_string(),
            });
        }
        if line_contains_invalid_cargo_full_xtask(line) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "invalid_cargo_full_xtask",
                text: line.trim().to_string(),
            });
        }
        for &(name, pattern) in BLOCKED_PATTERNS {
            if line_contains_blocked_pattern(path, name, pattern, line, &lower) {
                findings.push(HygieneFinding {
                    path: path.display().to_string(),
                    line: line_index + 1,
                    pattern: name,
                    text: line.trim().to_string(),
                });
            }
        }
        if line_contains_unbounded_read(path, line) && !bounded_read_chain {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "unbounded_read",
                text: line.trim().to_string(),
            });
        }
        if bounded_read_chain && line_contains_read_call(line) {
            pending_bounded_read_chain = false;
        } else if pending_bounded_read_chain && trimmed.ends_with(';') {
            pending_bounded_read_chain = false;
        }
        if is_undocumented_public_api(&text, line_index) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "undocumented_public_api",
                text: line.trim().to_string(),
            });
        }
        if (line.contains("GpuUnavailable")
            || lower.contains("gpu unavailable")
            || lower.contains("gpu not available")
            || lower.contains("no gpu available"))
            && (lower.contains("skip") || lower.contains("fallback") || lower.contains("fall back"))
            && !is_hidden_fallback_guard_source(path)
        {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "gpu_unavailable_skip",
                text: line.trim().to_string(),
            });
        }
    }
}

fn is_non_release_cfg_attr(trimmed: &str) -> bool {
    trimmed == "#[cfg(test)]"
        || trimmed.contains("cfg(any(test, feature = \"cpu-parity\"))")
        || trimmed.contains("cfg(any(feature = \"cpu-parity\", test))")
        || trimmed.contains("cfg(any(test, feature = \"legacy-infallible\"))")
        || trimmed.contains("cfg(any(feature = \"legacy-infallible\", test))")
}

fn line_contains_read_call(line: &str) -> bool {
    line.contains("fs::read_to_string(")
        || line.contains("std::fs::read_to_string(")
        || line.contains("fs::read(")
        || line.contains("std::fs::read(")
        || line.contains(".read_to_end(")
        || line.contains(".read_to_string(")
}

fn line_contains_unbounded_read(path: &Path, line: &str) -> bool {
    let normalized = path.to_string_lossy();
    if normalized.contains("/xtask/src/") {
        return false;
    }
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || is_release_rule_text(trimmed) {
        return false;
    }
    if trimmed.contains(".take(") {
        return false;
    }
    line_contains_read_call(trimmed)
}

fn is_undocumented_public_api(text: &str, line_index: usize) -> bool {
    let lines = text.lines().collect::<Vec<_>>();
    let Some(line) = lines.get(line_index) else {
        return false;
    };
    let trimmed = line.trim_start();
    if !(trimmed.starts_with("pub struct ")
        || trimmed.starts_with("pub enum ")
        || trimmed.starts_with("pub trait ")
        || trimmed.starts_with("pub type "))
    {
        return false;
    }
    let mut cursor = line_index;
    while cursor > 0 {
        cursor -= 1;
        let previous = lines[cursor].trim();
        if previous.is_empty() || previous.starts_with("#[") {
            continue;
        }
        return !(previous.starts_with("///") || previous.starts_with("//!"));
    }
    true
}

fn scan_tooling_file(path: &Path, scanned_files: &mut usize, findings: &mut Vec<HygieneFinding>) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            push_read_error(path, "unreadable_tooling_file", error, findings);
            return;
        }
    };
    *scanned_files += 1;
    for (line_index, line) in text.lines().enumerate() {
        if line_contains_raw_workspace_cargo(line) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "raw_workspace_cargo",
                text: line.trim().to_string(),
            });
        }
        if line_contains_invalid_cargo_full_xtask(line) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "invalid_cargo_full_xtask",
                text: line.trim().to_string(),
            });
        }
        if line_contains_heredoc(line) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "heredoc",
                text: line.trim().to_string(),
            });
        }
    }
}

fn scan_doc_file(path: &Path, scanned_files: &mut usize, findings: &mut Vec<HygieneFinding>) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            push_read_error(path, "unreadable_doc_file", error, findings);
            return;
        }
    };
    *scanned_files += 1;
    for (line_index, line) in text.lines().enumerate() {
        if line_contains_raw_workspace_cargo(line) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "raw_workspace_cargo",
                text: line.trim().to_string(),
            });
        }
        if line_contains_invalid_cargo_full_xtask(line) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "invalid_cargo_full_xtask",
                text: line.trim().to_string(),
            });
        }
        if line_contains_heredoc(line) {
            findings.push(HygieneFinding {
                path: path.display().to_string(),
                line: line_index + 1,
                pattern: "heredoc",
                text: line.trim().to_string(),
            });
        }
    }
}

fn scan_test_file(path: &Path, scanned_files: &mut usize, findings: &mut Vec<HygieneFinding>) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            push_read_error(path, "unreadable_test_file", error, findings);
            return;
        }
    };
    *scanned_files += 1;
    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if is_release_rule_text(trimmed) {
            continue;
        }
        if line.contains("TODO") {
            push_test_finding(path, line_index, "test_TODO", trimmed, findings);
        }
        if line.contains("FIXME") {
            push_test_finding(path, line_index, "test_FIXME", trimmed, findings);
        }
        if line.contains("todo!(") {
            push_test_finding(path, line_index, "test_todo_macro", trimmed, findings);
        }
        if line.contains("unimplemented!(") {
            push_test_finding(
                path,
                line_index,
                "test_unimplemented_macro",
                trimmed,
                findings,
            );
        }
        if trimmed == "#[ignore]"
            || trimmed.starts_with("#[ignore(")
            || trimmed.starts_with("#[ignore =")
        {
            push_test_finding(path, line_index, "test_ignored", trimmed, findings);
        }
        if trimmed.starts_with("let _ =") {
            push_test_finding(
                path,
                line_index,
                "test_let_underscore_result",
                trimmed,
                findings,
            );
        }
        if matches!(
            trimmed,
            "assert!(true);"
                | "assert_eq!(true, true);"
                | "assert_eq!(1, 1);"
                | "assert_ne!(1, 2);"
        ) {
            push_test_finding(path, line_index, "test_assert_true", trimmed, findings);
        }
    }
}

fn push_test_finding(
    path: &Path,
    line_index: usize,
    pattern: &'static str,
    text: &str,
    findings: &mut Vec<HygieneFinding>,
) {
    findings.push(HygieneFinding {
        path: path.display().to_string(),
        line: line_index + 1,
        pattern,
        text: text.to_string(),
    });
}

fn push_walk_error(root: &Path, error: &walkdir::Error, findings: &mut Vec<HygieneFinding>) {
    findings.push(HygieneFinding {
        path: error
            .path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| root.display().to_string()),
        line: 1,
        pattern: "unreadable_scan_entry",
        text: format!("failed to walk release hygiene root: {error}"),
    });
}

fn push_read_error(
    path: &Path,
    pattern: &'static str,
    error: io::Error,
    findings: &mut Vec<HygieneFinding>,
) {
    findings.push(HygieneFinding {
        path: path.display().to_string(),
        line: 1,
        pattern,
        text: format!("failed to read release hygiene input: {error}"),
    });
}

fn line_contains_blocked_pattern(
    path: &Path,
    name: &str,
    pattern: &str,
    line: &str,
    lower: &str,
) -> bool {
    let trimmed = line.trim();
    if is_rust_doc_comment_line(trimmed) && is_code_call_blocker(name) {
        return false;
    }
    if is_hygiene_rule_source(path) {
        return false;
    }
    if is_hidden_fallback_pattern(name) && is_hidden_fallback_guard_source(path) {
        return false;
    }
    if is_hidden_fallback_pattern(name) && is_negated_hidden_fallback_statement(lower) {
        return false;
    }
    if name == "cfg_not_gpu" && !line_cfg_not_gpu_hides_work(lower) {
        return false;
    }
    if is_release_rule_text(trimmed) {
        return false;
    }
    match name {
        "placeholder_text" => contains_word(lower, pattern),
        "stub_text" => contains_word(lower, pattern),
        "not_implemented_text" => lower.contains(pattern),
        "TODO" | "FIXME" => line.contains(pattern),
        _ => line.contains(pattern) || lower.contains(pattern),
    }
}

fn is_rust_doc_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("///") || trimmed.starts_with("//!")
}

fn is_code_call_blocker(name: &str) -> bool {
    matches!(
        name,
        "panic_macro"
            | "unwrap_call"
            | "expect_call"
            | "todo_macro"
            | "unimplemented_macro"
            | "not_implemented_text"
    )
}

fn is_hidden_fallback_pattern(name: &str) -> bool {
    matches!(
        name,
        "silent_gpu_skip"
            | "silent_gpu_skipped"
            | "gpu_unavailable_skip"
            | "cfg_not_gpu"
            | "cpu_fallback"
            | "software_fallback"
            | "fallback_dispatch"
            | "falling_back_to_cpu"
            | "fallback_to_cpu"
            | "synthetic_gpu_timing"
            | "fake_gpu_timing_formula"
    )
}

fn is_negated_hidden_fallback_statement(lower: &str) -> bool {
    lower.contains("no cpu fallback")
        || lower.contains("no hidden fallback")
        || lower.contains("no software fallback")
        || lower.contains("never hides")
        || lower.contains("must not hide")
}

fn line_cfg_not_gpu_hides_work(lower: &str) -> bool {
    lower.contains("fallback")
        || lower.contains("skip")
        || lower.contains("return ok")
        || lower.contains("success")
}

fn line_contains_raw_workspace_cargo(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("name:")
        || is_release_rule_text(trimmed)
        || trimmed.starts_with("echo ")
        || trimmed.contains("cargo install")
        || trimmed.contains("cargo_full")
        || trimmed.contains("CARGO_RUNNER")
        || trimmed.contains("./cargo_full")
        || trimmed.contains("VYRE_CARGO_RUNNER")
    {
        return false;
    }
    [
        "cargo build",
        "cargo check",
        "cargo test",
        "cargo clippy",
        "cargo doc",
        "cargo fmt",
        "cargo run",
        "cargo xtask",
        "cargo bench",
        "cargo publish",
        "cargo machete",
        "cargo udeps",
        "cargo fuzz",
        "cargo public-api",
    ]
    .iter()
    .any(|needle| trimmed.contains(needle))
        || trimmed.starts_with("cargo +")
}

fn line_contains_invalid_cargo_full_xtask(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || is_release_rule_text(trimmed) {
        return false;
    }
    let plain = ["cargo_full", " xtask"].concat();
    let dotted = ["./cargo_full", " xtask"].concat();
    trimmed.contains(&plain) || trimmed.contains(&dotted)
}

fn line_contains_heredoc(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }
    trimmed.contains("<<") && !trimmed.contains("<<<")
}

fn is_release_rule_text(trimmed: &str) -> bool {
    trimmed.starts_with('"')
        || trimmed.starts_with("(\"")
        || trimmed.starts_with("&[")
        || trimmed.contains("no-stubs")
        || trimmed.contains("unresolved marker")
        || trimmed.contains("No shipped stubs")
}

fn is_hygiene_rule_source(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    [
        "scripts/check_repo_split_readiness.sh",
        "scripts/check_dialect_coverage.sh",
        "scripts/check_unsafe_justifications.sh",
        "scripts/check_no_deferred_work.sh",
        "scripts/check_tests_can_fail.sh",
        "scripts/check_primitive_contract.sh",
        "jules_tickets/_generate.py",
        "jules_tickets/test_dump.py",
        "xtask/src/backend_matrix.rs",
        "xtask/src/docs_matrix.rs",
        "xtask/src/feature_matrix.rs",
        "xtask/src/hygiene_matrix.rs",
        "xtask/src/optimization_matrix.rs",
        "xtask/src/release_completion_audit.rs",
        "xtask/src/vyre_weir_release_gate.rs",
        "xtask/src/weir_matrix.rs",
        "xtask/src/whats_similar.rs",
        "xtask/src/parser_coherence.rs",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
}

fn is_hidden_fallback_guard_source(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    [
        "vyre-lints/src/production_cpu_fallbacks.rs",
        "vyre-lints/src/gpu_skip_guards.rs",
        "vyre-lints/src/lib.rs",
        "vyre-lints/src/main.rs",
        "vyre-lints/tests/production_cpu_fallbacks.rs",
        "vyre-lints/tests/gpu_skip_guards.rs",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(index, _)| {
        is_word_start(haystack, index) && is_word_end(haystack, index + needle.len())
    })
}

fn is_word_start(text: &str, index: usize) -> bool {
    text.get(..index)
        .and_then(|prefix| prefix.chars().next_back())
        .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
}

fn is_word_end(text: &str, index: usize) -> bool {
    text.get(index..)
        .and_then(|suffix| suffix.chars().next())
        .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
}

fn update_brace_depth(current: usize, line: &str) -> usize {
    let mut depth = current;
    let code = line.split("//").next().unwrap_or(line);
    for ch in code.chars() {
        match ch {
            '{' => depth = depth.saturating_add(1),
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

fn parse_output(args: &[String]) -> Result<PathBuf, String> {
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- hygiene-matrix [--output PATH]\n\n\
                     Scans Vyre/Weir shipped Rust source for release hygiene blockers."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown hygiene-matrix option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/hygiene/hygiene-matrix.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/hygiene/hygiene-matrix.json"))
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_HYGIENE_SCAN_FILE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_HYGIENE_SCAN_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_HYGIENE_SCAN_FILE_BYTES} byte hygiene scan read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_fallback_scan_ignores_guard_implementation_text() {
        let guard = Path::new("vyre-lints/src/production_cpu_fallbacks.rs");

        assert!(
            !line_contains_blocked_pattern(
                guard,
                "cpu_fallback",
                "cpu fallback",
                "//! Production CPU fallback guard.",
                "//! production cpu fallback guard.",
            ),
            "Fix: hygiene evidence must not count the guard's own forbidden-token description as shipped fallback behavior."
        );
    }

    #[test]
    fn hidden_fallback_scan_ignores_negated_product_status() {
        let source = Path::new("tools/vyrec/src/lib.rs");

        assert!(
            !line_contains_blocked_pattern(
                source,
                "cpu_fallback",
                "cpu fallback",
                "status: beta compile-evidence driver; no CPU fallback",
                "status: beta compile-evidence driver; no cpu fallback",
            ),
            "Fix: explicit no-fallback product status text must not be reported as hidden fallback behavior."
        );
    }

    #[test]
    fn hidden_fallback_scan_still_flags_positive_product_fallback() {
        let source = Path::new("libs/surge/surgec/src/scan/pipeline/parse_driver.rs");

        assert!(
            line_contains_blocked_pattern(
                source,
                "cpu_fallback",
                "cpu fallback",
                "CpuRayonParseDriver is a temporary CPU fallback.",
                "cpurayonparsedriver is a temporary cpu fallback.",
            ),
            "Fix: real positive fallback claims must remain visible in release hygiene evidence."
        );
    }

    #[test]
    fn cfg_not_gpu_attr_is_not_a_hidden_fallback_by_itself() {
        let source = Path::new("libs/surge/surgec/src/cmd_scan.rs");

        assert!(
            !line_contains_blocked_pattern(
                source,
                "cfg_not_gpu",
                "cfg(not(feature = \"gpu\"))",
                "#[cfg(not(feature = \"gpu\"))]",
                "#[cfg(not(feature = \"gpu\"))]",
            ),
            "Fix: a fail-closed compile-time GPU feature guard must not be treated as a runtime hidden fallback without fallback behavior."
        );
    }

    #[test]
    fn hygiene_classifier_separates_test_from_release_blocker() {
        let hot_paths = std::collections::BTreeSet::new();
        let findings = vec![
            HygieneFinding {
                path: "vyre-driver/src/pipeline/mod.rs".to_string(),
                line: 10,
                pattern: "panic_macro",
                text: "panic!(\"bad\")".to_string(),
            },
            HygieneFinding {
                path: "vyre-driver/tests/pipeline_contracts.rs".to_string(),
                line: 20,
                pattern: "test_ignored",
                text: "#[ignore]".to_string(),
            },
        ];

        let classes = classify_findings(Path::new("."), &findings, &hot_paths);

        assert_eq!(classes[0].surface, "production");
        assert_eq!(classes[0].risk, "release_blocker");
        assert!(classes[0].release_blocker);
        assert_eq!(classes[1].surface, "test");
        assert_eq!(classes[1].risk, "test_hygiene");
        assert!(!classes[1].release_blocker);
    }

    #[test]
    fn cpu_parity_oracle_sources_are_test_hygiene_not_release_blockers() {
        let hot_paths = std::collections::BTreeSet::new();
        let findings = vec![HygieneFinding {
            path: "/repo/libs/dataflow/weir/src/ifds_cpu_oracle.rs".to_string(),
            line: 37,
            pattern: "panic_macro",
            text: "panic!(\"IFDS CPU oracle\")".to_string(),
        }];

        let classes = classify_findings(Path::new("."), &findings, &hot_paths);

        assert_eq!(classes[0].surface, "test");
        assert_eq!(classes[0].risk, "test_hygiene");
        assert_eq!(classes[0].release_blocker, false);
    }

    #[test]
    fn rust_doc_comment_call_examples_do_not_count_as_production_blockers() {
        assert_eq!(
            line_contains_blocked_pattern(
                Path::new("libs/dataflow/weir/src/lib.rs"),
                "unwrap_call",
                ".unwrap()",
                "//! let value = fallible().unwrap();",
                "//! let value = fallible().unwrap();",
            ),
            false
        );
    }

    #[test]
    fn fuzz_targets_are_test_surface_not_release_production() {
        assert_eq!(
            hygiene_surface_for_path("libs/dataflow/weir/fuzz/fuzz_targets/reachability.rs"),
            "test"
        );
    }

    #[test]
    fn cfg_cpu_parity_attr_is_classified_as_non_release_item() {
        assert_eq!(
            is_non_release_cfg_attr("#[cfg(any(test, feature = \"cpu-parity\"))]"),
            true
        );
        assert_eq!(
            is_non_release_cfg_attr("#[cfg(any(test, feature = \"legacy-infallible\"))]"),
            true
        );
        assert_eq!(
            is_non_release_cfg_attr("#[cfg(feature = \"serde\")]"),
            false
        );
    }

    #[test]
    fn stacked_cfg_after_test_attr_still_counts_as_test_body() {
        let mut findings = Vec::new();
        let mut scanned_files = 0;
        let dir = std::env::temp_dir().join(format!(
            "vyre-hygiene-stacked-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("test temp dir");
        let path = dir.join("stacked_test.rs");
        std::fs::write(
            &path,
            "#[test]\n#[cfg(feature = \"gpu\")]\nfn generated_e2e() {\n    fallible().expect(\"test-only assertion\");\n}\n",
        )
        .expect("write stacked test fixture");
        scan_file(&path, &mut scanned_files, &mut findings);
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
        assert_eq!(scanned_files, 1);
        assert!(
            findings.is_empty(),
            "stacked #[test] + #[cfg] function body must not be release hygiene"
        );
    }

    #[test]
    fn hygiene_classifier_marks_hot_path_debt_as_release_blocker() {
        let hot_paths =
            std::collections::BTreeSet::from(["vyre-runtime/src/megakernel/ring.rs".to_string()]);
        let findings = vec![HygieneFinding {
            path: "vyre-runtime/src/megakernel/ring.rs".to_string(),
            line: 12,
            pattern: "TODO",
            text: "// TODO: remove allocation".to_string(),
        }];

        let classes = classify_findings(Path::new("."), &findings, &hot_paths);

        assert!(classes[0].hot_path);
        assert_eq!(classes[0].risk, "release_blocker");
    }

    #[test]
    fn hidden_fallback_guard_source_is_identified_for_gpu_skip_phrases() {
        assert!(is_hidden_fallback_guard_source(Path::new(
            "vyre-lints/src/gpu_skip_guards.rs"
        )));
    }

    #[test]
    fn required_cargo_wrapper_is_tool_owned() {
        let workspace = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for cargo wrapper hygiene test.");
        let santh_root = workspace.path().join("Santh");
        let vyre_root = santh_root.join("libs/performance/matching/vyre");
        fs::create_dir_all(&vyre_root)
            .expect("Fix: create temp vyre root for cargo wrapper hygiene test.");
        fs::write(vyre_root.join("cargo_full"), b"#!/usr/bin/env bash\n")
            .expect("Fix: write temp cargo_full wrapper for hygiene test.");

        let mut findings = Vec::new();
        check_required_cargo_wrappers(&vyre_root, &santh_root, &mut findings);

        assert!(
            findings.is_empty(),
            "Fix: Vyre release hygiene must require the tool-owned bounded cargo wrapper without forcing a Santh backup-root file into the standalone tool repo; findings={findings:?}"
        );
    }
}

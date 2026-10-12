//! Release documentation evidence matrix.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct DocsMatrix {
    schema_version: u32,
    curated_proof_docs_preserved: bool,
    docs: Vec<DocEntry>,
    limitation_findings: Vec<DocLimitationFinding>,
    authority_link_findings: Vec<MarkdownLinkFinding>,
    stale_generated_artifact_findings: Vec<StaleGeneratedArtifactFinding>,
    parallel_active_plan_findings: Vec<ParallelActivePlanFinding>,
    behavior_coherence_findings: Vec<BehaviorCoherenceFinding>,
    scan_positioning_findings: Vec<ScanPositioningFinding>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DocEntry {
    id: &'static str,
    path: String,
    exists: bool,
    read_error: Option<String>,
    contains_release_evidence_rule: bool,
    evidence_artifact_refs: Vec<String>,
    evidence_artifact_ref_count: usize,
    missing_evidence_artifact_refs: Vec<String>,
    required_topics: Vec<&'static str>,
    missing_topics: Vec<&'static str>,
    unresolved_markers: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct ReadmeContractEvidence {
    schema_version: u32,
    path: String,
    exists: bool,
    read_error: Option<String>,
    source_bytes: usize,
    required_tokens: Vec<&'static str>,
    missing_tokens: Vec<&'static str>,
    example_count: usize,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DocLimitationFinding {
    path: String,
    line: usize,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
struct MarkdownLinkFinding {
    path: String,
    line: usize,
    target: String,
    resolved: String,
}

#[derive(Debug, Clone, Serialize)]
struct StaleGeneratedArtifactFinding {
    stale_path: String,
    canonical_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ParallelActivePlanFinding {
    path: String,
    line: usize,
    marker: String,
}

#[derive(Debug, Clone, Serialize)]
struct BehaviorCoherenceFinding {
    behavior_id: &'static str,
    path: String,
    missing_tokens: Vec<&'static str>,
    evidence_artifact: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ScanPositioningFinding {
    row_index: Option<usize>,
    engine: String,
    issue: String,
}

#[derive(Debug, Deserialize)]
struct ScanPositioningMatrix {
    schema_version: u32,
    row: Vec<ScanPositioningRow>,
}

#[derive(Debug, Deserialize)]
struct ScanPositioningRow {
    engine: String,
    workload_class: String,
    positioning: String,
    #[serde(default)]
    benchmark_artifact: String,
    #[serde(default)]
    semantic_exclusion: String,
    #[serde(default)]
    unsupported_capability_reason: String,
}

struct RequiredDoc {
    id: &'static str,
    relative: &'static str,
    topics: &'static [&'static str],
}

struct BehaviorContract {
    id: &'static str,
    relative: &'static str,
    evidence_artifact: &'static str,
    required_tokens: &'static [&'static str],
}

const REQUIRED_DOCS: &[RequiredDoc] = &[
    RequiredDoc {
        id: "release-plan",
        relative: "../../../../docs/vyre-weir-release-plan.md",
        topics: &[
            "vyre",
            "weir",
            "release",
            "evidence",
            "benchmark",
            "conformance",
        ],
    },
    RequiredDoc {
        id: "vyre-readme",
        relative: "README.md",
        topics: &[
            "vyre",
            "gpu",
            "bytecode",
            "condition",
            "cuda",
            "wgpu",
            "backend",
            "fallback",
            "quickstart",
            "release/evidence",
        ],
    },
    RequiredDoc {
        id: "vyre-release",
        relative: "docs/RELEASE.md",
        topics: &["release", "version", "evidence", "gate"],
    },
    RequiredDoc {
        id: "vyre-release-engineering",
        relative: "docs/RELEASE_ENGINEERING.md",
        topics: &["release", "evidence", "cargo_full", "tag"],
    },
    RequiredDoc {
        id: "vyre-release-checklist",
        relative: "docs/RELEASE_CHECKLIST.md",
        topics: &["release", "evidence", "cuda", "weir"],
    },
    RequiredDoc {
        id: "vyre-publish-gate",
        relative: "docs/PUBLISH_GATE.md",
        topics: &["publish", "metadata", "cargo_full", "evidence"],
    },
    RequiredDoc {
        id: "vyre-testing",
        relative: "docs/TESTING_PROGRAM.md",
        topics: &["test", "conformance", "property", "benchmark"],
    },
    RequiredDoc {
        id: "vyre-optimization",
        relative: "docs/optimization/AGENT_CONTRACT.md",
        topics: &["optimization", "gpu", "pass", "evidence"],
    },
    RequiredDoc {
        id: "vyre-conformance",
        relative: "conform/README.md",
        topics: &["conformance", "op", "semantic", "evidence"],
    },
    RequiredDoc {
        id: "vyre-bench",
        relative: "vyre-bench/README.md",
        topics: &["benchmark", "cuda", "wgpu", "evidence"],
    },
    RequiredDoc {
        id: "vyre-frontend-c",
        relative: "vyre-frontend-c/README.md",
        topics: &["c", "parser", "linux", "evidence"],
    },
    RequiredDoc {
        id: "parsing-frontends",
        relative: "docs/parsing-and-frontends.md",
        topics: &[
            "parser",
            "tokenization",
            "literal anchor",
            "verifier rule",
            "quadratic guard",
            "host-loop ban",
            "release/evidence/docs/parser-doc-proof.md",
        ],
    },
    RequiredDoc {
        id: "vyrec-readme",
        relative: "../../../../tools/vyrec/README.md",
        topics: &["vyrec", "parser", "cuda", "evidence"],
    },
    RequiredDoc {
        id: "weir-readme",
        relative: "../../../../libs/dataflow/weir/README.md",
        topics: &[
            "weir",
            "dataflow",
            "analysis",
            "evidence",
            "ssa",
            "def-use",
            "reaching-definition",
            "points-to",
            "ifds",
            "callgraph",
            "control-dependence",
            "cross-language",
            "dominators",
            "escape",
            "live",
            "must-initialize",
            "post-dominator",
            "range-check",
            "scc",
            "summary",
            "value-set",
            "witness",
        ],
    },
    RequiredDoc {
        id: "weir-vision",
        relative: "../../../../libs/dataflow/weir/VISION.md",
        topics: &["weir", "dataflow", "analysis", "release"],
    },
    RequiredDoc {
        id: "wgpu-fallback-proof",
        relative: "release/evidence/docs/wgpu-fallback-proof.md",
        topics: &["wgpu", "fallback", "conformance", "evidence"],
    },
];

const UNRESOLVED_MARKERS: &[&str] = &[
    "status: blocked",
    "status: open",
    "status: pending",
    "todo",
    "fixme",
    "placeholder",
    "stub",
    "tbd",
    "to be filled",
];

const AUTHORITY_LINK_DOCS: &[&str] = &["docs/THESIS.md"];
const ACTIVE_ACCELERATION_PLAN: &str = "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md";
const ACTIVE_PLAN_SCAN_ROOTS: &[&str] = &["README.md", "docs", "audits", "vyre-bench"];
const SCAN_POSITIONING_MATRIX: &str = "docs/optimization/SCAN_POSITIONING_MATRIX.toml";
const REQUIRED_SCAN_POSITIONING_ENGINES: &[&str] = &[
    "Vyre",
    "Hyperscan",
    "Vectorscan",
    "Rust regex",
    "Aho-Corasick",
    "memchr",
    "Hardware regex",
    "FPGA offload",
];
const STALE_GENERATED_DOC_ALIASES: &[(&str, &str)] = &[(
    "docs/optimization/COMMAND_MATRIX.md",
    "docs/optimization/XTASK_COMMAND_MATRIX.md",
)];
const BEHAVIOR_CONTRACTS: &[BehaviorContract] = &[
    BehaviorContract {
        id: "release-evidence-external-artifacts",
        relative: "docs/RELEASE_ENGINEERING.md",
        evidence_artifact: "release/evidence/final/expected-artifacts.json",
        required_tokens: &[
            "release-evidence",
            "external-artifacts-only",
            "command_mode",
            "artifact_contracts",
            "blockers",
            "exit",
            "release/evidence/final/expected-artifacts.json",
        ],
    },
    BehaviorContract {
        id: "release-evidence-external-benchmark-freshness",
        relative: "docs/RELEASE_ENGINEERING.md",
        evidence_artifact: "release/evidence/final/release-evidence-run.json",
        required_tokens: &[
            "release-benchmarks",
            "external-artifacts-only",
            "command_mode",
            "source_digest",
            "command_digest",
            "hardware_digest",
            "schema_digest_chain",
            "release/evidence/benchmarks/cuda-release-suite.json",
            "release/evidence/final/release-evidence-run.json",
        ],
    },
    BehaviorContract {
        id: "command-matrix-provenance",
        relative: "docs/optimization/XTASK_COMMAND_MATRIX.md",
        evidence_artifact: "docs/optimization/XTASK_COMMAND_MATRIX.md",
        required_tokens: &[
            "command-matrix",
            "source files",
            "source loc",
            "duplicate-risk score",
            "shared sources",
            "source digest",
            "source-count provenance",
        ],
    },
    BehaviorContract {
        id: "research-audit-schema",
        relative: "docs/optimization/AGENT_CONTRACT.md",
        evidence_artifact: "release/evidence/optimization/research-audit.json",
        required_tokens: &[
            "research-audit",
            "schema",
            "source_digest",
            "source_ledger_findings",
            "blockers",
            "exit",
            "release/evidence/optimization/research-audit.json",
        ],
    },
    BehaviorContract {
        id: "frontier-leaderboard-contract",
        relative: "vyre-bench/README.md",
        evidence_artifact: "release/evidence/benchmarks/frontier-leaderboard.json",
        required_tokens: &[
            "release-benchmarks",
            "frontier-leaderboard",
            "baseline",
            "metric_family",
            "comparator",
            "blockers",
            "release/evidence/benchmarks/frontier-leaderboard.json",
        ],
    },
];
const MAX_RELEASE_DOC_BYTES: u64 = 4_194_304;

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
    let mut docs = Vec::new();
    let mut blockers = Vec::new();
    let mut limitation_findings = Vec::new();
    for required in REQUIRED_DOCS {
        let path = vyre_root.join(required.relative);
        let path_exists = path.is_file();
        let (text, read_error) = if path_exists {
            match read_text_bounded(&path) {
                Ok(text) => (Some(text), None),
                Err(error) => (None, Some(error.to_string())),
            }
        } else {
            (None, None)
        };
        let exists = path_exists;
        let lowered = text
            .as_ref()
            .map(|text| text.to_ascii_lowercase())
            .unwrap_or_default();
        let contains_release_evidence_rule = lowered.contains("evidence");
        let missing_topics = missing_required_topics(&lowered, required.topics);
        let unresolved_markers = UNRESOLVED_MARKERS
            .iter()
            .copied()
            .filter(|marker| {
                text.as_deref()
                    .is_some_and(|text| doc_contains_unresolved_marker(text, marker))
            })
            .collect::<Vec<_>>();
        let evidence_artifact_refs = text
            .as_deref()
            .map(extract_evidence_artifact_refs)
            .unwrap_or_default();
        let missing_evidence_artifact_refs =
            missing_evidence_artifact_refs(&vyre_root, &evidence_artifact_refs);
        if let Some(error) = &read_error {
            blockers.push(format!(
                "required documentation `{}` could not be read at {}: {error}",
                required.id,
                path.display()
            ));
        } else if !exists {
            blockers.push(format!(
                "required documentation `{}` is missing",
                required.id
            ));
        } else if !contains_release_evidence_rule {
            blockers.push(format!(
                "required documentation `{}` does not reference release evidence",
                required.id
            ));
        }
        if exists && evidence_artifact_refs.is_empty() {
            blockers.push(format!(
                "required documentation `{}` does not reference concrete release evidence artifacts",
                required.id
            ));
        }
        if !missing_evidence_artifact_refs.is_empty() {
            blockers.push(format!(
                "required documentation `{}` references {} missing release evidence artifact(s)",
                required.id,
                missing_evidence_artifact_refs.len()
            ));
        }
        for topic in &missing_topics {
            blockers.push(format!(
                "required documentation `{}` does not cover required topic `{topic}`",
                required.id
            ));
        }
        for marker in &unresolved_markers {
            blockers.push(format!(
                "required documentation `{}` contains unresolved marker `{marker}`",
                required.id
            ));
        }
        if let Some(text) = text.as_deref() {
            collect_limitation_findings(&path, text, &mut limitation_findings);
        }
        docs.push(DocEntry {
            id: required.id,
            path: path.display().to_string(),
            exists,
            read_error,
            contains_release_evidence_rule,
            evidence_artifact_ref_count: evidence_artifact_refs.len(),
            evidence_artifact_refs,
            missing_evidence_artifact_refs,
            required_topics: required.topics.to_vec(),
            missing_topics,
            unresolved_markers,
        });
    }
    for finding in &limitation_findings {
        blockers.push(format!(
            "release documentation `{}`:{} contains unapproved limitation wording `{}`",
            finding.path, finding.line, finding.text
        ));
    }
    let authority_link_findings = collect_authority_link_findings(&vyre_root);
    for finding in &authority_link_findings {
        blockers.push(format!(
            "authority documentation `{}`:{} links to missing `{}` resolved as `{}`",
            finding.path, finding.line, finding.target, finding.resolved
        ));
    }
    let stale_generated_artifact_findings = collect_stale_generated_artifact_findings(&vyre_root);
    for finding in &stale_generated_artifact_findings {
        blockers.push(format!(
            "stale generated documentation alias `{}` exists; canonical generated artifact is `{}`",
            finding.stale_path, finding.canonical_path
        ));
    }
    let parallel_active_plan_findings = collect_parallel_active_plan_findings(&vyre_root);
    for finding in &parallel_active_plan_findings {
        blockers.push(format!(
            "documentation `{}`:{} contains parallel active VX marker `{}`; move active work to `{ACTIVE_ACCELERATION_PLAN}`",
            finding.path, finding.line, finding.marker
        ));
    }
    let behavior_coherence_findings = collect_behavior_coherence_findings(&vyre_root);
    for finding in &behavior_coherence_findings {
        blockers.push(format!(
            "user-visible behavior `{}` in `{}` is missing {} docs/help/example/JSON/exit/evidence token(s) for `{}`",
            finding.behavior_id,
            finding.path,
            finding.missing_tokens.len(),
            finding.evidence_artifact
        ));
    }
    let scan_positioning_findings = collect_scan_positioning_findings(&vyre_root);
    for finding in &scan_positioning_findings {
        blockers.push(format!(
            "scan positioning matrix row {:?} for `{}` is invalid: {}",
            finding.row_index, finding.engine, finding.issue
        ));
    }
    let matrix = DocsMatrix {
        schema_version: 5,
        curated_proof_docs_preserved: true,
        docs,
        limitation_findings,
        authority_link_findings,
        stale_generated_artifact_findings,
        parallel_active_plan_findings,
        behavior_coherence_findings,
        scan_positioning_findings,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize docs matrix: {error}");
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
    write_sibling_docs(&output, &matrix);
    println!("docs-matrix: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn collect_limitation_findings(path: &Path, text: &str, findings: &mut Vec<DocLimitationFinding>) {
    for (line_index, line) in text.lines().enumerate() {
        let lowered = line.to_ascii_lowercase();
        if lowered.contains("must not contain")
            || lowered.contains("limitation_findings")
            || lowered.contains("unapproved limitation")
        {
            continue;
        }
        let contains_limitation = lowered.contains("known limitation")
            || lowered.contains("out of scope")
            || lowered.contains("not supported")
            || lowered.contains("future release")
            || lowered.contains("next release");
        if !contains_limitation || lowered.contains("explicitly approved") {
            continue;
        }
        findings.push(DocLimitationFinding {
            path: path.display().to_string(),
            line: line_index + 1,
            text: line.trim().to_string(),
        });
    }
}

fn collect_authority_link_findings(vyre_root: &Path) -> Vec<MarkdownLinkFinding> {
    let mut findings = Vec::new();
    for relative in AUTHORITY_LINK_DOCS {
        let path = vyre_root.join(relative);
        let text = match read_text_bounded(&path) {
            Ok(text) => text,
            Err(error) => {
                findings.push(MarkdownLinkFinding {
                    path: relative.to_string(),
                    line: 0,
                    target: "<read-error>".to_string(),
                    resolved: error.to_string(),
                });
                continue;
            }
        };
        collect_missing_markdown_links(vyre_root, relative, &text, &mut findings);
    }
    findings
}

fn collect_missing_markdown_links(
    vyre_root: &Path,
    source_relative: &str,
    text: &str,
    findings: &mut Vec<MarkdownLinkFinding>,
) {
    for (line_index, line) in text.lines().enumerate() {
        for target in markdown_link_targets(line) {
            if markdown_target_is_external_or_anchor(&target) {
                continue;
            }
            let target_without_fragment = target
                .split_once('#')
                .map(|(path, _fragment)| path)
                .unwrap_or(&target)
                .trim();
            if target_without_fragment.is_empty() {
                continue;
            }
            let resolved = vyre_root
                .join(source_relative)
                .parent()
                .unwrap_or(vyre_root)
                .join(target_without_fragment);
            if resolved.exists() {
                continue;
            }
            findings.push(MarkdownLinkFinding {
                path: source_relative.to_string(),
                line: line_index + 1,
                target,
                resolved: resolved.to_string_lossy().replace('\\', "/"),
            });
        }
    }
}

fn markdown_link_targets(line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = line;
    while let Some(link_start) = rest.find("](") {
        let after_start = &rest[link_start + 2..];
        let Some(link_end) = after_start.find(')') else {
            break;
        };
        let target = after_start[..link_end].trim();
        if !target.is_empty() {
            targets.push(target.to_string());
        }
        rest = &after_start[link_end + 1..];
    }
    targets
}

fn markdown_target_is_external_or_anchor(target: &str) -> bool {
    target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
}

fn collect_stale_generated_artifact_findings(
    vyre_root: &Path,
) -> Vec<StaleGeneratedArtifactFinding> {
    STALE_GENERATED_DOC_ALIASES
        .iter()
        .filter(|(stale, _canonical)| vyre_root.join(stale).exists())
        .map(|(stale, canonical)| StaleGeneratedArtifactFinding {
            stale_path: (*stale).to_string(),
            canonical_path: (*canonical).to_string(),
        })
        .collect()
}

fn collect_parallel_active_plan_findings(vyre_root: &Path) -> Vec<ParallelActivePlanFinding> {
    let mut findings = Vec::new();
    for relative in collect_active_plan_scan_paths(vyre_root) {
        if relative == ACTIVE_ACCELERATION_PLAN {
            continue;
        }
        let path = vyre_root.join(&relative);
        let text = match read_text_bounded(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        for (line_index, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("| VX-") {
                findings.push(ParallelActivePlanFinding {
                    path: relative.clone(),
                    line: line_index + 1,
                    marker: trimmed
                        .split('|')
                        .nth(1)
                        .map(str::trim)
                        .unwrap_or("VX row")
                        .to_string(),
                });
            }
        }
    }
    findings
}

fn collect_active_plan_scan_paths(vyre_root: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    for root in ACTIVE_PLAN_SCAN_ROOTS {
        let path = vyre_root.join(root);
        if path.is_file() {
            if path.extension().is_some_and(|ext| ext == "md") {
                paths.push((*root).to_string());
            }
            continue;
        }
        collect_markdown_paths(vyre_root, &path, &mut paths);
    }
    paths.sort();
    paths.dedup();
    paths
}

fn collect_markdown_paths(vyre_root: &Path, dir: &Path, paths: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_paths(vyre_root, &path, paths);
            continue;
        }
        if !path.extension().is_some_and(|ext| ext == "md") {
            continue;
        }
        if let Ok(relative) = path.strip_prefix(vyre_root) {
            paths.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn collect_behavior_coherence_findings(vyre_root: &Path) -> Vec<BehaviorCoherenceFinding> {
    BEHAVIOR_CONTRACTS
        .iter()
        .filter_map(|contract| {
            let path = vyre_root.join(contract.relative);
            let text = read_text_bounded(&path).unwrap_or_default();
            let lowered = text.to_ascii_lowercase();
            let missing_tokens = contract
                .required_tokens
                .iter()
                .copied()
                .filter(|token| !lowered.contains(&token.to_ascii_lowercase()))
                .collect::<Vec<_>>();
            if missing_tokens.is_empty() {
                return None;
            }
            Some(BehaviorCoherenceFinding {
                behavior_id: contract.id,
                path: contract.relative.to_string(),
                missing_tokens,
                evidence_artifact: contract.evidence_artifact,
            })
        })
        .collect()
}

fn collect_scan_positioning_findings(vyre_root: &Path) -> Vec<ScanPositioningFinding> {
    let mut findings = Vec::new();
    let path = vyre_root.join(SCAN_POSITIONING_MATRIX);
    let text = match read_text_bounded(&path) {
        Ok(text) => text,
        Err(error) => {
            findings.push(ScanPositioningFinding {
                row_index: None,
                engine: "<matrix>".to_string(),
                issue: format!(
                    "could not read `{SCAN_POSITIONING_MATRIX}`: {error}. Fix: keep scan positioning in the canonical matrix file."
                ),
            });
            return findings;
        }
    };
    let matrix = match toml::from_str::<ScanPositioningMatrix>(&text) {
        Ok(matrix) => matrix,
        Err(error) => {
            findings.push(ScanPositioningFinding {
                row_index: None,
                engine: "<matrix>".to_string(),
                issue: format!(
                    "could not parse `{SCAN_POSITIONING_MATRIX}`: {error}. Fix: use [[row]] entries with engine, workload_class, positioning, and proof fields."
                ),
            });
            return findings;
        }
    };
    if matrix.schema_version != 1 {
        findings.push(ScanPositioningFinding {
            row_index: None,
            engine: "<matrix>".to_string(),
            issue: format!(
                "schema_version {} is unsupported; expected 1",
                matrix.schema_version
            ),
        });
    }
    let mut seen_engines = BTreeSet::new();
    let mut seen_rows = BTreeSet::new();
    for (index, row) in matrix.row.iter().enumerate() {
        let row_index = Some(index);
        let engine = row.engine.trim();
        let workload_class = row.workload_class.trim();
        let positioning = row.positioning.trim();
        if engine.is_empty() {
            findings.push(ScanPositioningFinding {
                row_index,
                engine: "<empty>".to_string(),
                issue: "missing engine. Fix: name the compared scan engine.".to_string(),
            });
        } else {
            seen_engines.insert(engine.to_string());
        }
        if workload_class.is_empty() {
            findings.push(ScanPositioningFinding {
                row_index,
                engine: row.engine.clone(),
                issue: "missing workload_class. Fix: state the workload class this row covers."
                    .to_string(),
            });
        }
        if positioning.is_empty() {
            findings.push(ScanPositioningFinding {
                row_index,
                engine: row.engine.clone(),
                issue: "missing positioning. Fix: state how Vyre compares to this engine."
                    .to_string(),
            });
        }
        let row_key = format!("{engine}\n{workload_class}");
        if !engine.is_empty() && !workload_class.is_empty() && !seen_rows.insert(row_key) {
            findings.push(ScanPositioningFinding {
                row_index,
                engine: row.engine.clone(),
                issue: "duplicate engine/workload_class row. Fix: dedup through one canonical positioning row."
                    .to_string(),
            });
        }
        let benchmark = row.benchmark_artifact.trim();
        let semantic_exclusion = row.semantic_exclusion.trim();
        let unsupported = row.unsupported_capability_reason.trim();
        if benchmark.is_empty() && semantic_exclusion.is_empty() && unsupported.is_empty() {
            findings.push(ScanPositioningFinding {
                row_index,
                engine: row.engine.clone(),
                issue: "missing proof field. Fix: provide benchmark_artifact, semantic_exclusion, or unsupported_capability_reason."
                    .to_string(),
            });
        }
        if !benchmark.is_empty() && !vyre_root.join(benchmark).is_file() {
            findings.push(ScanPositioningFinding {
                row_index,
                engine: row.engine.clone(),
                issue: format!(
                    "benchmark_artifact `{benchmark}` does not exist. Fix: point at a concrete release/evidence benchmark artifact or use a semantic exclusion/unsupported reason."
                ),
            });
        }
    }
    for required in REQUIRED_SCAN_POSITIONING_ENGINES {
        if !seen_engines.contains(*required) {
            findings.push(ScanPositioningFinding {
                row_index: None,
                engine: (*required).to_string(),
                issue: "required engine is missing from the scan positioning matrix".to_string(),
            });
        }
    }
    findings
}

fn doc_contains_unresolved_marker(text: &str, marker: &str) -> bool {
    text.lines().any(|line| {
        let lowered = line.to_ascii_lowercase();
        !doc_line_is_release_rule_text(&lowered) && lowered.contains(marker)
    })
}

fn missing_required_topics<'a>(lowered_doc: &str, topics: &'a [&'a str]) -> Vec<&'a str> {
    topics
        .iter()
        .copied()
        .filter(|topic| !lowered_doc.contains(topic))
        .collect()
}

fn doc_line_is_release_rule_text(lowered: &str) -> bool {
    lowered.contains("no-stub")
        || lowered.contains("zero-stub")
        || lowered.contains("no stubs")
        || lowered.contains("no shipped source")
        || lowered.contains("final review finds no")
        || lowered.contains("must not")
        || lowered.contains("not only")
        || lowered.contains("not optional")
        || lowered.contains("not a ")
        || lowered.contains("no todo")
        || lowered.contains("todo/fixme")
        || lowered.contains("stub functions with")
        || lowered.contains("forbidden patterns")
        || lowered.contains("stubs, hidden fallbacks")
}

fn write_sibling_docs(output: &Path, matrix: &DocsMatrix) {
    let Some(parent) = output.parent() else {
        eprintln!(
            "Fix: docs matrix output `{}` has no parent directory.",
            output.display()
        );
        std::process::exit(1);
    };
    for &(artifact, title, doc_ids) in DOC_PROOFS {
        write_markdown_if_missing(
            &parent.join(artifact),
            &render_doc_proof(title, matrix, doc_ids),
        );
    }
    write_vyre_readme_contract(parent);
}

const DOC_PROOFS: &[(&str, &str, &[&str])] = &[
    (
        "release-notes-version-story.md",
        "Release Notes Version Story Evidence",
        &[
            "release-plan",
            "vyre-release",
            "vyre-release-engineering",
            "vyre-release-checklist",
        ],
    ),
    (
        "cuda-release-path.md",
        "CUDA Release Path Documentation Evidence",
        &["release-plan", "vyre-bench"],
    ),
    (
        "wgpu-fallback-proof.md",
        "WGPU Fallback Documentation Evidence",
        &["wgpu-fallback-proof", "release-plan", "vyre-bench"],
    ),
    (
        "megakernel-default-proof.md",
        "Megakernel Default Documentation Evidence",
        &["release-plan", "vyre-optimization"],
    ),
    (
        "optimization-proof.md",
        "Optimization Documentation Evidence",
        &["vyre-optimization", "release-plan"],
    ),
    (
        "egraph-saturation.md",
        "E-Graph Saturation Documentation Evidence",
        &["vyre-optimization", "release-plan"],
    ),
    (
        "c-parser-linux-proof.md",
        "C Parser Linux Corpus Documentation Evidence",
        &["vyre-frontend-c", "vyrec-readme", "release-plan"],
    ),
    (
        "distributed-parser-coherence.md",
        "Distributed Parser Coherence Documentation Evidence",
        &["vyre-frontend-c", "vyrec-readme", "release-plan"],
    ),
    (
        "weir-integration.md",
        "Weir Integration Documentation Evidence",
        &["weir-readme", "weir-vision", "release-plan"],
    ),
    (
        "test-architecture.md",
        "Test Architecture Documentation Evidence",
        &["vyre-testing", "release-plan"],
    ),
    (
        "vyre-readme-proof.md",
        "Vyre README Documentation Evidence",
        &["vyre-readme"],
    ),
    (
        "weir-readme-proof.md",
        "Weir README Documentation Evidence",
        &["weir-readme", "weir-vision"],
    ),
    (
        "parser-doc-proof.md",
        "Parser Documentation Evidence",
        &["vyre-frontend-c", "vyrec-readme"],
    ),
    (
        "benchmark-doc-proof.md",
        "Benchmark Documentation Evidence",
        &["vyre-bench"],
    ),
    (
        "conformance-doc-proof.md",
        "Conformance Documentation Evidence",
        &["vyre-conformance", "release-plan"],
    ),
    (
        "release-notes.md",
        "Release Notes Documentation Evidence",
        &[
            "release-plan",
            "vyre-release",
            "vyre-release-engineering",
            "vyre-release-checklist",
        ],
    ),
    (
        "crate-metadata-proof.md",
        "Crate Metadata Documentation Evidence",
        &["vyre-readme", "release-plan"],
    ),
    (
        "release-hygiene-proof.md",
        "Release Hygiene Documentation Evidence",
        &["release-plan", "vyre-testing"],
    ),
    (
        "cpu-only-100x-proof.md",
        "CPU-Only 100x Proof Documentation Evidence",
        &["release-plan", "vyre-bench"],
    ),
];

fn render_doc_proof(title: &str, matrix: &DocsMatrix, doc_ids: &[&str]) -> String {
    let selected = matrix
        .docs
        .iter()
        .filter(|doc| doc_ids.iter().any(|id| id == &doc.id))
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();
    for doc in &selected {
        if !doc.exists {
            blockers.push(format!("source documentation `{}` is missing", doc.id));
        } else if !doc.contains_release_evidence_rule {
            blockers.push(format!(
                "source documentation `{}` does not reference release evidence",
                doc.id
            ));
        }
        if doc.exists && doc.evidence_artifact_refs.is_empty() {
            blockers.push(format!(
                "source documentation `{}` does not reference concrete release evidence artifacts",
                doc.id
            ));
        }
        if !doc.missing_evidence_artifact_refs.is_empty() {
            blockers.push(format!(
                "source documentation `{}` references {} missing release evidence artifact(s)",
                doc.id,
                doc.missing_evidence_artifact_refs.len()
            ));
        }
        for topic in &doc.missing_topics {
            blockers.push(format!(
                "source documentation `{}` does not cover required topic `{topic}`",
                doc.id
            ));
        }
        for marker in &doc.unresolved_markers {
            blockers.push(format!(
                "source documentation `{}` contains unresolved marker `{marker}`",
                doc.id
            ));
        }
    }
    if selected.len() != doc_ids.len() {
        blockers.push(
            "one or more requested source documentation IDs were not in docs-matrix".to_string(),
        );
    }
    let status = if blockers.is_empty() {
        "ready"
    } else {
        "blocked"
    };
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(title);
    // Release-train identity from the single TOML source, not stale literals.
    let vyre_v = crate::release_train::vyre_version();
    let weir_v = crate::release_train::weir_version();
    out.push_str(&format!(
        "\n\nGenerated by `cargo_full run --bin xtask -- docs-matrix`; do not hand-edit this evidence artifact.\n\nRelease train: `vyre {vyre_v}`, `weir {weir_v}`, `vyre-v{vyre_v}`, `weir-v{weir_v}`, `vyre-{vyre_v}-weir-{weir_v}`.\n\nStatus: ",
    ));
    out.push_str(status);
    out.push_str("\n\nEvidence sources:\n");
    for doc in &selected {
        out.push_str("- `");
        out.push_str(doc.id);
        out.push_str("`: `");
        out.push_str(&doc.path);
        out.push_str("`, exists=");
        out.push_str(if doc.exists { "true" } else { "false" });
        out.push_str(", references_evidence=");
        out.push_str(if doc.contains_release_evidence_rule {
            "true"
        } else {
            "false"
        });
        out.push_str(", evidence_artifact_ref_count=");
        out.push_str(&doc.evidence_artifact_ref_count.to_string());
        out.push_str(", missing_evidence_artifact_refs=");
        if doc.missing_evidence_artifact_refs.is_empty() {
            out.push_str("[]");
        } else {
            out.push_str(&format!("{:?}", doc.missing_evidence_artifact_refs));
        }
        out.push_str(", missing_topics=");
        if doc.missing_topics.is_empty() {
            out.push_str("[]");
        } else {
            out.push_str(&format!("{:?}", doc.missing_topics));
        }
        out.push_str(", unresolved_markers=");
        if doc.unresolved_markers.is_empty() {
            out.push_str("[]");
        } else {
            out.push_str(&format!("{:?}", doc.unresolved_markers));
        }
        out.push('\n');
    }
    out.push_str("\nConcrete evidence artifacts referenced by source docs:\n");
    let mut artifact_refs = selected
        .iter()
        .flat_map(|doc| doc.evidence_artifact_refs.iter().cloned())
        .collect::<Vec<_>>();
    artifact_refs.sort();
    artifact_refs.dedup();
    if artifact_refs.is_empty() {
        out.push_str("- none\n");
    } else {
        for artifact in artifact_refs {
            out.push_str("- `");
            out.push_str(&artifact);
            out.push_str("`\n");
        }
    }
    out.push_str("\nRelease contract:\n");
    out.push_str("- Every listed source document must exist.\n");
    out.push_str("- Every listed source document must reference concrete `release/evidence/...` artifacts.\n");
    out.push_str("- Required topics must be present and unresolved markers must be absent.\n");
    out.push_str("- JSON contract artifacts generated by this command are the machine-readable gate source; this Markdown is explanatory evidence.\n");
    out.push_str("\nBlockers:\n");
    if blockers.is_empty() {
        out.push_str("- none\n");
    } else {
        for blocker in blockers {
            out.push_str("- ");
            out.push_str(&blocker);
            out.push('\n');
        }
    }
    out
}

fn write_markdown_if_missing(path: &Path, text: &str) {
    if path.exists() {
        return;
    }
    write_markdown(path, text);
}

fn extract_evidence_artifact_refs(text: &str) -> Vec<String> {
    let mut refs = text
        .split(|ch: char| {
            ch.is_whitespace() || matches!(ch, '`' | '"' | '\'' | '(' | ')' | '[' | ']' | ',' | ';')
        })
        .filter_map(|token| {
            let trimmed = token.trim_matches(|ch: char| matches!(ch, '.' | ':' | ',' | ';'));
            if trimmed.contains("release/evidence/") || trimmed.starts_with("evidence/") {
                Some(trimmed.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

fn missing_evidence_artifact_refs(vyre_root: &Path, refs: &[String]) -> Vec<String> {
    let mut missing = refs
        .iter()
        .filter(|reference| !is_generated_docs_evidence_ref(reference))
        .filter(|reference| {
            let path = if reference.starts_with("release/evidence/") {
                vyre_root.join(reference)
            } else if let Some(stripped) = reference.strip_prefix("evidence/") {
                vyre_root.join("release/evidence").join(stripped)
            } else {
                return false;
            };
            !path.exists()
        })
        .cloned()
        .collect::<Vec<_>>();
    missing.sort();
    missing.dedup();
    missing
}

fn is_generated_docs_evidence_ref(reference: &str) -> bool {
    reference.starts_with("release/evidence/docs/") || reference.starts_with("evidence/docs/")
}

fn write_vyre_readme_contract(parent: &Path) {
    let vyre_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let readme = vyre_root.join("README.md");
    let exists = readme.is_file();
    let mut blockers = Vec::new();
    let (text, read_error) = if exists {
        match read_text_bounded(&readme) {
            Ok(text) => (text, None),
            Err(error) => (
                String::new(),
                Some(format!(
                    "Vyre README could not be read at {}: {error}",
                    readme.display()
                )),
            ),
        }
    } else {
        (String::new(), None)
    };
    let lowered = text.to_ascii_lowercase();
    let required_tokens = vec![
        // Single source of truth: the release-train TOML version, not a stale
        // hardcoded literal (this was pinned at "0.6.3" while the train moved on).
        crate::release_train::vyre_version(),
        "vyre",
        "gpu",
        "cuda",
        "wgpu",
        "bytecode",
        "condition",
        "vyre::program",
        "release/evidence",
        "cargo add vyre",
    ];
    let missing_tokens = required_tokens
        .iter()
        .copied()
        .filter(|token| !lowered.contains(&token.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let example_count = text.matches("```rust").count()
        + text.matches("```toml").count()
        + text.matches("```bash").count()
        + text.matches("```sh").count();
    if let Some(error) = &read_error {
        blockers.push(error.clone());
    }
    if !exists {
        blockers.push(format!("Vyre README is missing at {}", readme.display()));
    }
    if exists && text.trim().is_empty() {
        blockers.push("Vyre README is empty".to_string());
    }
    for token in &missing_tokens {
        blockers.push(format!("Vyre README is missing required token `{token}`"));
    }
    if example_count == 0 {
        blockers.push(
            "Vyre README must include at least one Rust, TOML, or shell example block".to_string(),
        );
    }
    write_json(
        &parent.join("vyre-readme-contracts.json"),
        &ReadmeContractEvidence {
            schema_version: 2,
            path: readme.display().to_string(),
            exists,
            read_error,
            source_bytes: text.len(),
            required_tokens,
            missing_tokens,
            example_count,
            blockers,
        },
    );
}

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

fn write_markdown(path: &Path, text: &str) {
    if let Err(error) = fs::write(path, text) {
        eprintln!("Fix: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
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
                    "USAGE:\n  cargo_full run --bin xtask -- docs-matrix [--output PATH]\n\n\
                     Writes release documentation evidence matrix."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown docs-matrix option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/docs/docs-matrix.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/docs/docs-matrix.json"))
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_RELEASE_DOC_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_RELEASE_DOC_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_RELEASE_DOC_BYTES} byte release documentation read cap",
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
    fn parser_tokenization_policy_topics_require_anchor_verifier_guard_and_host_loop_ban() {
        let topics = [
            "tokenization",
            "literal anchor",
            "verifier rule",
            "quadratic guard",
            "host-loop ban",
        ];
        let missing = missing_required_topics("tokenization uses a literal anchor", &topics);

        assert_eq!(
            missing,
            vec!["verifier rule", "quadratic guard", "host-loop ban"]
        );

        let complete = missing_required_topics(
            "tokenization policy: literal anchor, verifier rule, quadratic guard, host-loop ban",
            &topics,
        );
        assert!(complete.is_empty());
    }

    #[test]
    fn missing_authority_markdown_link_is_reported() {
        let mut findings = Vec::new();
        let tmp = tempfile::tempdir().unwrap();

        collect_missing_markdown_links(
            tmp.path(),
            "docs/THESIS.md",
            "Read [root thesis](../THESIS.md).\n",
            &mut findings,
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "docs/THESIS.md");
        assert_eq!(findings[0].line, 1);
        assert_eq!(findings[0].target, "../THESIS.md");
    }

    #[test]
    fn existing_authority_markdown_link_passes() {
        let mut findings = Vec::new();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("THESIS.md"), "# Thesis\n").unwrap();

        collect_missing_markdown_links(
            tmp.path(),
            "docs/THESIS.md",
            "Read [root thesis](../THESIS.md).\n",
            &mut findings,
        );

        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn stale_generated_doc_alias_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let alias = tmp.path().join("docs/optimization/COMMAND_MATRIX.md");
        std::fs::create_dir_all(alias.parent().unwrap()).unwrap();
        std::fs::write(&alias, "# stale\n").unwrap();

        let findings = collect_stale_generated_artifact_findings(tmp.path());

        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].canonical_path,
            "docs/optimization/XTASK_COMMAND_MATRIX.md"
        );
    }

    #[test]
    fn behavior_coherence_finding_reports_missing_user_visible_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::create_dir_all(tmp.path().join("docs/optimization")).unwrap();
        std::fs::create_dir_all(tmp.path().join("vyre-bench")).unwrap();
        std::fs::write(
            tmp.path().join("docs/RELEASE_ENGINEERING.md"),
            "release-evidence writes blockers and expected artifacts.\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("docs/optimization/XTASK_COMMAND_MATRIX.md"),
            "command-matrix source files source loc duplicate-risk score shared sources source digest source-count provenance.\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("docs/optimization/AGENT_CONTRACT.md"),
            "research-audit schema source_digest source_ledger_findings blockers exit release/evidence/optimization/research-audit.json.\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("vyre-bench/README.md"),
            "release-benchmarks frontier-leaderboard baseline metric_family comparator blockers release/evidence/benchmarks/frontier-leaderboard.json.\n",
        )
        .unwrap();

        let findings = collect_behavior_coherence_findings(tmp.path());

        let release = findings
            .iter()
            .find(|finding| finding.behavior_id == "release-evidence-external-artifacts")
            .expect("Fix: release-evidence behavior fixture must report missing tokens.");
        assert!(release
            .missing_tokens
            .iter()
            .any(|token| *token == "external-artifacts-only"));
        assert!(findings
            .iter()
            .all(|finding| finding.behavior_id != "command-matrix-provenance"));
    }

    #[test]
    fn parallel_active_plan_marker_is_reported_outside_canonical_plan() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs/optimization")).unwrap();
        std::fs::write(
            tmp.path().join("docs/optimization/ALL_AXES_ACCELERATION_PLAN.md"),
            "| VX-001 | canonical |\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("docs/optimization/OLD_PLAN.md"),
            "| VX-999 | stale active row |\n",
        )
        .unwrap();

        let findings = collect_parallel_active_plan_findings(tmp.path());

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "docs/optimization/OLD_PLAN.md");
        assert_eq!(findings[0].line, 1);
        assert_eq!(findings[0].marker, "VX-999");
    }
}

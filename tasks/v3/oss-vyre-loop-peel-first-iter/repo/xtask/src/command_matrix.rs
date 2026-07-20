//! `cargo_full run --bin xtask -- command-matrix` - xtask command ownership matrix.
//!
//! This matrix is the VX-003 answer to xtask bloat: every command has an
//! owner lane, source LOC, proof kind, primary release evidence artifact,
//! and duplicate-risk score in one generated artifact.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

use crate::hash::sha256_hex;
use crate::ownership::{load_ownership_lanes, owner_lane_for_file, OwnershipLaneRule};
use crate::release_evidence::expected_artifacts_for_command;

const MAX_COMMAND_SOURCE_BYTES: u64 = 2_097_152;
const CANONICAL_COMMAND_MATRIX: &str = "docs/optimization/XTASK_COMMAND_MATRIX.md";
const ACTIVE_ACCELERATION_PLAN: &str = "docs/optimization/ALL_AXES_ACCELERATION_PLAN.md";
const GENERATED_COMMAND: &str =
    "cargo_full run -p xtask --bin xtask -- command-matrix --output docs/optimization/XTASK_COMMAND_MATRIX.md";
const SOURCE_COUNT_PROVENANCE_VERSION: &str = "command-matrix-source-count:v1";
const DUPLICATE_RISK_VX_THRESHOLD: u32 = 40;
const REQUIRED_DUPLICATE_REPORT_COMMANDS: &[&str] =
    &["lego-audit", "whats-similar", "source-similar"];
const ARTIFACT_PATHS_SOURCE: &str = "xtask/src/artifact_paths.rs";
const BENCH_TARGETS_DATA_SOURCE: &str = "docs/optimization/BENCH_TARGETS.toml";
const BENCHMARK_EVIDENCE_SEMANTICS_SOURCE: &str = "xtask/src/benchmark_evidence_semantics.rs";
const EXPECTED_ARTIFACTS_SOURCE: &str = "xtask/src/release_evidence/expected_artifacts.rs";
const COMPETITOR_ISSUE_LEDGER_DATA_SOURCE: &str =
    "docs/optimization/COMPETITOR_ISSUE_LEDGER.toml";
const ARCHIVE_REPLAY_AUDITS_DATA_SOURCE: &str = "docs/optimization/ARCHIVE_REPLAY_AUDITS.toml";
const FRONTIER_LEADERBOARD_BASELINES_DATA_SOURCE: &str =
    "docs/optimization/FRONTIER_LEADERBOARD_BASELINES.toml";
const HASH_SOURCE: &str = "xtask/src/hash.rs";
const INNOVATION_FALSIFICATION_SOURCE: &str = "xtask/src/innovation_falsification.rs";
const LAUNCH_CONTRACT_SOURCE: &str = "xtask/src/launch_contract.rs";
const MARKDOWN_TABLE_SOURCE: &str = "xtask/src/markdown_table.rs";
const OWNERSHIP_SOURCE: &str = "xtask/src/ownership.rs";
const RELEASE_BENCHMARKS_SOURCE: &str = "xtask/src/release_benchmarks.rs";
const RELEASE_TRAIN_DATA_SOURCE: &str = "release/release-train.toml";
const RELEASE_TRAIN_SOURCE: &str = "xtask/src/release_train.rs";
const REPO_BOUNDARY_DATA_SOURCE: &str = "release/repo-boundary.toml";
const REPO_BOUNDARY_SOURCE: &str = "xtask/src/repo_boundary.rs";
const RESEARCH_AUDIT_SOURCE: &str = "xtask/src/research_audit.rs";
const RESEARCH_BASIS_SOURCE: &str = "xtask/src/research_basis.rs";
const RESEARCH_KEY_SOURCE: &str = "xtask/src/research_key.rs";
const RESEARCH_PLAN_COVERAGE_SOURCE: &str = "xtask/src/research_plan_coverage.rs";
const RESEARCH_SOURCE_LEDGER_SOURCE: &str = "xtask/src/research_source_ledger.rs";
const RESEARCH_SOURCE_LEDGER_DATA_SOURCE: &str = "docs/optimization/RESEARCH_SOURCE_LEDGER.toml";
const RULES_AS_DATA_SOURCE: &str = "xtask/src/rules_as_data.rs";
const RULES_AS_DATA_MANIFEST_SOURCE: &str = "docs/optimization/RULES_AS_DATA_MANIFEST.toml";
const THRESHOLD_POLICY_DATA_SOURCE: &str = "docs/optimization/THRESHOLD_POLICY.toml";
const TOML_CONFIG_SOURCE: &str = "xtask/src/toml_config.rs";
const VX_PLAN_TABLE_SOURCE: &str = "xtask/src/vx_plan_table.rs";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResearchAffinityContract {
    command: &'static str,
    affinity: &'static str,
    required_shared_sources: &'static [&'static str],
    proof_artifact_fragment: &'static str,
}

const RESEARCH_AFFINITY_CONTRACTS: &[ResearchAffinityContract] = &[
    ResearchAffinityContract {
        command: "acceleration-plan-gate",
        affinity: "active-plan-research-grounding",
        required_shared_sources: &[
            RESEARCH_BASIS_SOURCE,
            RESEARCH_KEY_SOURCE,
            RESEARCH_SOURCE_LEDGER_SOURCE,
            RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
        ],
        proof_artifact_fragment: "release/evidence/optimization/",
    },
    ResearchAffinityContract {
        command: "release-benchmarks",
        affinity: "frontier-benchmark-baseline",
        required_shared_sources: &[
            FRONTIER_LEADERBOARD_BASELINES_DATA_SOURCE,
            RESEARCH_KEY_SOURCE,
            RESEARCH_SOURCE_LEDGER_SOURCE,
            RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
        ],
        proof_artifact_fragment: "release/evidence/benchmarks/",
    },
    ResearchAffinityContract {
        command: "research-audit",
        affinity: "research-audit-grounding",
        required_shared_sources: &[
            RESEARCH_BASIS_SOURCE,
            RESEARCH_KEY_SOURCE,
            RESEARCH_SOURCE_LEDGER_SOURCE,
            RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
            COMPETITOR_ISSUE_LEDGER_DATA_SOURCE,
        ],
        proof_artifact_fragment: "release/evidence/optimization/",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandSpec {
    command: &'static str,
    module: &'static str,
    owner_lane: &'static str,
    proof_kind: ProofKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProofKind {
    PlanGate,
    HotPathGate,
    DedupGate,
    MatrixEvidence,
    ReleaseEvidence,
    Benchmark,
    LintGate,
    ParserEvidence,
    Utility,
}

impl ProofKind {
    fn as_str(self) -> &'static str {
        match self {
            ProofKind::PlanGate => "plan-gate",
            ProofKind::HotPathGate => "hot-path-gate",
            ProofKind::DedupGate => "dedup-gate",
            ProofKind::MatrixEvidence => "matrix-evidence",
            ProofKind::ReleaseEvidence => "release-evidence",
            ProofKind::Benchmark => "benchmark",
            ProofKind::LintGate => "lint-gate",
            ProofKind::ParserEvidence => "parser-evidence",
            ProofKind::Utility => "utility",
        }
    }

    fn duplicate_risk_weight(self) -> u32 {
        match self {
            ProofKind::PlanGate => 30,
            ProofKind::HotPathGate => 30,
            ProofKind::DedupGate => 30,
            ProofKind::MatrixEvidence => 28,
            ProofKind::ReleaseEvidence => 35,
            ProofKind::Benchmark => 25,
            ProofKind::LintGate => 24,
            ProofKind::ParserEvidence => 22,
            ProofKind::Utility => 10,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandMatrixRow {
    command: String,
    module: String,
    source_file: String,
    shared_sources: Vec<String>,
    owner_lane: String,
    proof_kind: String,
    primary_evidence_artifact: String,
    source_file_count: usize,
    source_loc: usize,
    duplicate_risk_score: u32,
    source_manifest_digest: String,
    shared_source_digest: String,
    source_count_provenance: String,
    generated_command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceStats {
    rel_path: String,
    file_count: usize,
    loc: usize,
    source_manifest_digest: String,
    shared_source_digest: String,
    source_count_provenance: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceDigestEntry {
    rel_path: String,
    byte_len: usize,
    loc: usize,
    content_digest: String,
}

#[derive(Debug, Eq, PartialEq)]
enum Mode {
    Print,
    Write(PathBuf),
    Check(PathBuf),
}

pub(crate) fn run(args: &[String]) {
    let mode = match parse_mode(args) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("Fix: {error}");
            print_usage();
            process::exit(2);
        }
    };
    let root = workspace_root();
    let ownership_path = root.join("docs/optimization/OWNERSHIP.toml");
    let ownership_lanes = match load_ownership_lanes(&ownership_path) {
        Ok(lanes) => lanes,
        Err(error) => {
            eprintln!(
                "Fix: command-matrix could not load ownership map `{}`: {error}",
                ownership_path.display()
            );
            process::exit(1);
        }
    };
    let rows = match collect_command_matrix_rows(&root, &ownership_lanes) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Fix: command-matrix failed: {error}");
            process::exit(1);
        }
    };
    let rendered = render_markdown(&rows);
    match mode {
        Mode::Print => print!("{rendered}"),
        Mode::Write(path) => {
            if let Err(error) = write_text(&path, &rendered) {
                eprintln!("Fix: could not write `{}`: {error}", path.display());
                process::exit(1);
            }
            println!("command-matrix: wrote {}", path.display());
        }
        Mode::Check(path) => match read_text_bounded(&path, MAX_COMMAND_SOURCE_BYTES) {
            Ok(current) if current == rendered => {
                if let Err(failures) = validate_command_matrix_contracts(&root, &rows) {
                    eprintln!(
                        "Fix: command matrix contract validation failed {} check(s):",
                        failures.len()
                    );
                    for failure in failures {
                        eprintln!("- {failure}");
                    }
                    process::exit(1);
                }
                println!("command-matrix: {} is current", path.display());
            }
            Ok(_) => {
                eprintln!(
                    "Fix: command matrix `{}` is stale. Run `cargo_full run -p xtask --bin xtask -- command-matrix --output {}`.",
                    path.display(),
                    path.display()
                );
                process::exit(1);
            }
            Err(error) => {
                eprintln!("Fix: could not read `{}`: {error}", path.display());
                process::exit(1);
            }
        },
    }
}

fn command_specs() -> &'static [CommandSpec] {
    &[
        CommandSpec {
            command: "quick-check",
            module: "quick",
            owner_lane: "coordination",
            proof_kind: ProofKind::Utility,
        },
        CommandSpec {
            command: "acceleration-plan-gate",
            module: "acceleration_plan_gate",
            owner_lane: "coordination",
            proof_kind: ProofKind::PlanGate,
        },
        CommandSpec {
            command: "abstraction-gate",
            module: "abstraction_gate",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "bench-crossback",
            module: "bench_crossback",
            owner_lane: "bench_harness",
            proof_kind: ProofKind::Benchmark,
        },
        CommandSpec {
            command: "backend-matrix",
            module: "backend_matrix",
            owner_lane: "driver_shared",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "bench-release",
            module: "bench_release",
            owner_lane: "bench_harness",
            proof_kind: ProofKind::Benchmark,
        },
        CommandSpec {
            command: "shrink",
            module: "shrink",
            owner_lane: "coordination",
            proof_kind: ProofKind::Utility,
        },
        CommandSpec {
            command: "check-cat-a",
            module: "check_cat_a",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "check-tier-deps",
            module: "check_tier_deps",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "compile",
            module: "compile",
            owner_lane: "driver_shared",
            proof_kind: ProofKind::ParserEvidence,
        },
        CommandSpec {
            command: "c-parser-bench",
            module: "c_parser_bench",
            owner_lane: "bench_harness",
            proof_kind: ProofKind::Benchmark,
        },
        CommandSpec {
            command: "c-parser-corpus",
            module: "c_parser_corpus",
            owner_lane: "bench_harness",
            proof_kind: ProofKind::ParserEvidence,
        },
        CommandSpec {
            command: "conformance-matrix",
            module: "conformance_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "dep-drift",
            module: "dep_drift",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "docs-matrix",
            module: "docs_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "feature-matrix",
            module: "feature_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "print-composition",
            module: "print_composition",
            owner_lane: "coordination",
            proof_kind: ProofKind::DedupGate,
        },
        CommandSpec {
            command: "list-ops",
            module: "list_ops",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "metadata-matrix",
            module: "metadata_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "op-matrix",
            module: "op_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "optimization-matrix",
            module: "optimization_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "package-readiness",
            module: "package_readiness",
            owner_lane: "coordination",
            proof_kind: ProofKind::ReleaseEvidence,
        },
        CommandSpec {
            command: "optimization-corpus",
            module: "optimization_corpus",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "parser-coherence",
            module: "parser_coherence",
            owner_lane: "coordination",
            proof_kind: ProofKind::ParserEvidence,
        },
        CommandSpec {
            command: "platform-boundary",
            module: "platform_boundary",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "catalog",
            module: "catalog",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "release-gate",
            module: "release_gate",
            owner_lane: "coordination",
            proof_kind: ProofKind::ReleaseEvidence,
        },
        CommandSpec {
            command: "release-workload-matrix",
            module: "release_workload_matrix",
            owner_lane: "bench_harness",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "release-benchmarks",
            module: "release_benchmarks",
            owner_lane: "bench_harness",
            proof_kind: ProofKind::Benchmark,
        },
        CommandSpec {
            command: "release-conformance",
            module: "release_conformance",
            owner_lane: "coordination",
            proof_kind: ProofKind::ReleaseEvidence,
        },
        CommandSpec {
            command: "release-completion-audit",
            module: "release_completion_audit",
            owner_lane: "coordination",
            proof_kind: ProofKind::ReleaseEvidence,
        },
        CommandSpec {
            command: "release-evidence",
            module: "release_evidence",
            owner_lane: "coordination",
            proof_kind: ProofKind::ReleaseEvidence,
        },
        CommandSpec {
            command: "vyre-release-gate",
            module: "vyre_weir_release_gate",
            owner_lane: "coordination",
            proof_kind: ProofKind::ReleaseEvidence,
        },
        CommandSpec {
            command: "vyre-weir-release-gate",
            module: "vyre_weir_release_gate",
            owner_lane: "coordination",
            proof_kind: ProofKind::ReleaseEvidence,
        },
        CommandSpec {
            command: "recursion-gate",
            module: "recursion_gate",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "heuristic-audit",
            module: "heuristic_audit",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "hygiene-matrix",
            module: "hygiene_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "trace-f32",
            module: "trace_f32",
            owner_lane: "coordination",
            proof_kind: ProofKind::Utility,
        },
        CommandSpec {
            command: "verify-rewrite-proofs",
            module: "verify_rewrite_proofs",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "version-matrix",
            module: "version_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "weir-matrix",
            module: "weir_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "gate1",
            module: "gate1",
            owner_lane: "coordination",
            proof_kind: ProofKind::DedupGate,
        },
        CommandSpec {
            command: "lego-audit",
            module: "lego_audit",
            owner_lane: "coordination",
            proof_kind: ProofKind::DedupGate,
        },
        CommandSpec {
            command: "lego-quick",
            module: "lego_quick",
            owner_lane: "coordination",
            proof_kind: ProofKind::DedupGate,
        },
        CommandSpec {
            command: "whats-similar",
            module: "whats_similar",
            owner_lane: "coordination",
            proof_kind: ProofKind::DedupGate,
        },
        CommandSpec {
            command: "source-similar",
            module: "source_similar",
            owner_lane: "coordination",
            proof_kind: ProofKind::DedupGate,
        },
        CommandSpec {
            command: "hot-path-scan",
            module: "hot_path_scan",
            owner_lane: "coordination",
            proof_kind: ProofKind::HotPathGate,
        },
        CommandSpec {
            command: "test-matrix",
            module: "test_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "lint-shape-tests",
            module: "lint_shape_tests",
            owner_lane: "coordination",
            proof_kind: ProofKind::LintGate,
        },
        CommandSpec {
            command: "launch-state",
            module: "launch_state",
            owner_lane: "coordination",
            proof_kind: ProofKind::ReleaseEvidence,
        },
        CommandSpec {
            command: "command-matrix",
            module: "command_matrix",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
        CommandSpec {
            command: "research-audit",
            module: "research_audit",
            owner_lane: "coordination",
            proof_kind: ProofKind::MatrixEvidence,
        },
    ]
}

fn collect_command_matrix_rows(
    root: &Path,
    ownership_lanes: &[OwnershipLaneRule],
) -> Result<Vec<CommandMatrixRow>, String> {
    let mut stats = BTreeMap::new();
    for spec in command_specs() {
        let source = resolve_module_source(root, spec.module)?;
        let rel_path = source
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let source_stats = module_source_stats(
            root,
            spec.module,
            &source,
            &rel_path,
            shared_sources_for_command(spec.command),
        )?;
        stats.insert(spec.module, source_stats);
    }
    Ok(rows_from_stats(command_specs(), &stats, ownership_lanes))
}

fn module_source_stats(
    root: &Path,
    module: &str,
    source: &Path,
    rel_path: &str,
    shared_sources: &[&str],
) -> Result<SourceStats, String> {
    let primary_text = read_text_bounded(source, MAX_COMMAND_SOURCE_BYTES)
        .map_err(|error| format!("could not read `{rel_path}`: {error}"))?
        .to_string();
    let mut total = primary_text.lines().count();
    let mut digest_entries = vec![source_digest_entry(rel_path, &primary_text)];
    let mut shared_digest_entries = Vec::new();
    let mut counted = BTreeSet::from([rel_path.to_string()]);
    let module_dir = root.join("xtask/src").join(module);
    if module_dir.is_dir() {
        let mut submodule_sources = Vec::new();
        collect_rust_sources_recursive(&module_dir, &mut submodule_sources)?;
        for path in submodule_sources {
            let rel = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if !counted.insert(rel.clone()) {
                continue;
            }
            let text = read_text_bounded(&path, MAX_COMMAND_SOURCE_BYTES)
                .map_err(|error| format!("could not read `{rel}`: {error}"))?;
            total += text.lines().count();
            digest_entries.push(source_digest_entry(&rel, &text));
        }
    }
    for rel in shared_sources {
        if !counted.insert((*rel).to_string()) {
            continue;
        }
        let path = root.join(rel);
        let text = read_text_bounded(&path, MAX_COMMAND_SOURCE_BYTES)
            .map_err(|error| format!("could not read `{rel}`: {error}"))?;
        total += text.lines().count();
        let entry = source_digest_entry(rel, &text);
        shared_digest_entries.push(entry.clone());
        digest_entries.push(entry);
    }
    let counted_sources = counted.iter().cloned().collect::<Vec<_>>();
    let source_count_provenance = source_count_provenance(&counted_sources, total);
    Ok(SourceStats {
        rel_path: rel_path.to_string(),
        file_count: counted_sources.len(),
        loc: total,
        source_manifest_digest: source_manifest_digest(&digest_entries),
        shared_source_digest: if shared_digest_entries.is_empty() {
            "none".to_string()
        } else {
            source_manifest_digest(&shared_digest_entries)
        },
        source_count_provenance,
    })
}

fn shared_sources_for_command(command: &str) -> &'static [&'static str] {
    match command {
        "acceleration-plan-gate" => &[
            ARTIFACT_PATHS_SOURCE,
            HASH_SOURCE,
            INNOVATION_FALSIFICATION_SOURCE,
            MARKDOWN_TABLE_SOURCE,
            RESEARCH_BASIS_SOURCE,
            RESEARCH_KEY_SOURCE,
            RESEARCH_PLAN_COVERAGE_SOURCE,
            RESEARCH_SOURCE_LEDGER_SOURCE,
            RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
            RULES_AS_DATA_SOURCE,
            RULES_AS_DATA_MANIFEST_SOURCE,
            VX_PLAN_TABLE_SOURCE,
        ],
        "research-audit" => &[
            ARTIFACT_PATHS_SOURCE,
            HASH_SOURCE,
            INNOVATION_FALSIFICATION_SOURCE,
            MARKDOWN_TABLE_SOURCE,
            REPO_BOUNDARY_DATA_SOURCE,
            REPO_BOUNDARY_SOURCE,
            RESEARCH_BASIS_SOURCE,
            RESEARCH_KEY_SOURCE,
            RESEARCH_PLAN_COVERAGE_SOURCE,
            RESEARCH_SOURCE_LEDGER_SOURCE,
            RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
            COMPETITOR_ISSUE_LEDGER_DATA_SOURCE,
            ARCHIVE_REPLAY_AUDITS_DATA_SOURCE,
            RULES_AS_DATA_SOURCE,
            RULES_AS_DATA_MANIFEST_SOURCE,
            TOML_CONFIG_SOURCE,
            VX_PLAN_TABLE_SOURCE,
        ],
        "hygiene-matrix" => &[THRESHOLD_POLICY_DATA_SOURCE],
        "launch-state" => &[
            LAUNCH_CONTRACT_SOURCE,
            RELEASE_TRAIN_DATA_SOURCE,
            RELEASE_TRAIN_SOURCE,
            REPO_BOUNDARY_DATA_SOURCE,
            REPO_BOUNDARY_SOURCE,
            TOML_CONFIG_SOURCE,
        ],
        "command-matrix" => &[
            ARTIFACT_PATHS_SOURCE,
            HASH_SOURCE,
            OWNERSHIP_SOURCE,
            EXPECTED_ARTIFACTS_SOURCE,
            RELEASE_BENCHMARKS_SOURCE,
            RESEARCH_AUDIT_SOURCE,
        ],
        "metadata-matrix" | "package-readiness" => &[
            RELEASE_TRAIN_DATA_SOURCE,
            RELEASE_TRAIN_SOURCE,
            TOML_CONFIG_SOURCE,
        ],
        "release-benchmarks" => &[
            ARTIFACT_PATHS_SOURCE,
            BENCHMARK_EVIDENCE_SEMANTICS_SOURCE,
            FRONTIER_LEADERBOARD_BASELINES_DATA_SOURCE,
            HASH_SOURCE,
            RESEARCH_KEY_SOURCE,
            RESEARCH_SOURCE_LEDGER_SOURCE,
            RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
            TOML_CONFIG_SOURCE,
        ],
        "release-evidence" => &[
            ARTIFACT_PATHS_SOURCE,
            BENCHMARK_EVIDENCE_SEMANTICS_SOURCE,
            HASH_SOURCE,
            RELEASE_BENCHMARKS_SOURCE,
            REPO_BOUNDARY_DATA_SOURCE,
            REPO_BOUNDARY_SOURCE,
            RESEARCH_AUDIT_SOURCE,
            TOML_CONFIG_SOURCE,
        ],
        "release-completion-audit" => &[
            BENCH_TARGETS_DATA_SOURCE,
            BENCHMARK_EVIDENCE_SEMANTICS_SOURCE,
            LAUNCH_CONTRACT_SOURCE,
            RELEASE_TRAIN_DATA_SOURCE,
            RELEASE_TRAIN_SOURCE,
            REPO_BOUNDARY_DATA_SOURCE,
            REPO_BOUNDARY_SOURCE,
            TOML_CONFIG_SOURCE,
        ],
        "version-matrix" | "vyre-release-gate" | "vyre-weir-release-gate" => &[
            RELEASE_TRAIN_DATA_SOURCE,
            RELEASE_TRAIN_SOURCE,
            TOML_CONFIG_SOURCE,
        ],
        "hot-path-scan" | "source-similar" => &[OWNERSHIP_SOURCE],
        _ => &[],
    }
}

fn collect_rust_sources_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("could not read module directory `{}`: {error}", dir.display()))?;
    let mut entries = entries
        .by_ref()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "could not read module directory entry under `{}`: {error}",
                dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|error| format!("could not stat `{}`: {error}", path.display()))?;
        if metadata.is_dir() {
            collect_rust_sources_recursive(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn source_digest_entry(rel_path: &str, text: &str) -> SourceDigestEntry {
    SourceDigestEntry {
        rel_path: rel_path.to_string(),
        byte_len: text.len(),
        loc: text.lines().count(),
        content_digest: sha256_hex(text.as_bytes()),
    }
}

fn source_manifest_digest(entries: &[SourceDigestEntry]) -> String {
    let mut entries = entries.to_vec();
    entries.sort_by(|left, right| left.rel_path.cmp(&right.rel_path));
    let mut material = String::new();
    for entry in entries {
        material.push_str(&entry.rel_path);
        material.push('\t');
        material.push_str(&entry.byte_len.to_string());
        material.push('\t');
        material.push_str(&entry.loc.to_string());
        material.push('\t');
        material.push_str(&entry.content_digest);
        material.push('\n');
    }
    sha256_hex(material.as_bytes())
}

fn source_count_provenance(counted_sources: &[String], loc: usize) -> String {
    format!(
        "{SOURCE_COUNT_PROVENANCE_VERSION}:files={}:loc={}:sources={}",
        counted_sources.len(),
        loc,
        counted_sources.join(",")
    )
}

fn rows_from_stats(
    specs: &[CommandSpec],
    stats: &BTreeMap<&str, SourceStats>,
    ownership_lanes: &[OwnershipLaneRule],
) -> Vec<CommandMatrixRow> {
    specs
        .iter()
        .map(|spec| {
            let source = stats
                .get(spec.module)
                .cloned()
                .unwrap_or_else(|| SourceStats {
                    rel_path: format!("xtask/src/{}.rs", spec.module),
                    file_count: 0,
                    loc: 0,
                    source_manifest_digest: String::new(),
                    shared_source_digest: String::new(),
                    source_count_provenance: String::new(),
                });
            let ownership_owner = owner_lane_for_file(&source.rel_path, ownership_lanes);
            let owner_lane = if ownership_owner == "unowned" {
                spec.owner_lane
            } else {
                ownership_owner
            };
            CommandMatrixRow {
                command: spec.command.to_string(),
                module: spec.module.to_string(),
                source_file: source.rel_path,
                shared_sources: shared_sources_for_command(spec.command)
                    .iter()
                    .map(|source| (*source).to_string())
                    .collect(),
                owner_lane: owner_lane.to_string(),
                proof_kind: spec.proof_kind.as_str().to_string(),
                primary_evidence_artifact: primary_evidence_artifact(spec.command).to_string(),
                source_file_count: source.file_count,
                source_loc: source.loc,
                duplicate_risk_score: duplicate_risk_score(source.loc, spec.proof_kind),
                source_manifest_digest: source.source_manifest_digest,
                shared_source_digest: source.shared_source_digest,
                source_count_provenance: source.source_count_provenance,
                generated_command: GENERATED_COMMAND.to_string(),
            }
        })
        .collect()
}

fn duplicate_risk_score(loc: usize, proof_kind: ProofKind) -> u32 {
    let loc_units = u32::try_from(loc / 100).unwrap_or(u32::MAX);
    proof_kind.duplicate_risk_weight().saturating_add(loc_units)
}

fn primary_evidence_artifact(command: &str) -> &'static str {
    expected_artifacts_for_command(command)
        .first()
        .copied()
        .unwrap_or("none")
}

fn render_markdown(rows: &[CommandMatrixRow]) -> String {
    let mut out = String::new();
    out.push_str("# Xtask command matrix\n\n");
    out.push_str("Generated by `cargo_full run -p xtask --bin xtask -- command-matrix --output docs/optimization/XTASK_COMMAND_MATRIX.md`.\n\n");
    out.push_str("Source LOC includes the primary command file, `xtask/src/<module>/**/*.rs` submodules when present, and command-declared shared helper files.\n\n");
    out.push_str(
        "| Command | Owner lane | Proof kind | Primary evidence artifact | Source files | Source LOC | Duplicate-risk score | Source file | Shared sources |\n",
    );
    out.push_str("|---|---|---|---|---:|---:|---:|---|---|\n");
    for row in rows {
        let shared_sources = if row.shared_sources.is_empty() {
            "none".to_string()
        } else {
            row.shared_sources
                .iter()
                .map(|source| format!("`{source}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} | {} | {} | `{}` | {} |\n",
            row.command,
            row.owner_lane,
            row.proof_kind,
            row.primary_evidence_artifact,
            row.source_file_count,
            row.source_loc,
            row.duplicate_risk_score,
            row.source_file,
            shared_sources
        ));
    }
    out
}

fn validate_high_risk_vx_links(root: &Path, rows: &[CommandMatrixRow]) -> Result<(), Vec<String>> {
    let plan_path = root.join(ACTIVE_ACCELERATION_PLAN);
    let plan_text = match read_text_bounded(&plan_path, MAX_COMMAND_SOURCE_BYTES) {
        Ok(text) => text,
        Err(error) => {
            return Err(vec![format!(
                "could not read active VX plan `{}`: {error}",
                plan_path.display()
            )]);
        }
    };
    let failures = missing_high_risk_vx_links(rows, &plan_text);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn validate_command_matrix_contracts(
    root: &Path,
    rows: &[CommandMatrixRow],
) -> Result<(), Vec<String>> {
    let mut failures = Vec::new();
    if let Err(mut high_risk_failures) = validate_high_risk_vx_links(root, rows) {
        failures.append(&mut high_risk_failures);
    }
    failures.extend(missing_command_provenance_contracts(rows));
    failures.extend(missing_research_affinity_contracts(rows));
    failures.extend(missing_required_duplicate_report_artifacts(rows));
    failures.extend(mismatched_primary_evidence_artifacts(rows));
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn missing_high_risk_vx_links(rows: &[CommandMatrixRow], plan_text: &str) -> Vec<String> {
    rows.iter()
        .filter(|row| row.duplicate_risk_score > DUPLICATE_RISK_VX_THRESHOLD)
        .filter_map(|row| {
            if !linked_vx_ids_for_row(row, plan_text).is_empty() {
                return None;
            }
            Some(format!(
                "command `{}` has duplicate-risk score {} but no VX row cites `{}` or documents removal",
                row.command, row.duplicate_risk_score, row.source_file
            ))
        })
        .collect()
}

fn missing_command_provenance_contracts(rows: &[CommandMatrixRow]) -> Vec<String> {
    let mut failures = Vec::new();
    for row in rows {
        if row.generated_command != GENERATED_COMMAND {
            failures.push(format!(
                "command `{}` generated-command provenance `{}` must be `{GENERATED_COMMAND}`",
                row.command, row.generated_command
            ));
        }
        if !is_sha256_hex(&row.source_manifest_digest) {
            failures.push(format!(
                "command `{}` source manifest digest must be a full SHA-256 hex digest",
                row.command
            ));
        }
        if row.shared_sources.is_empty() {
            if row.shared_source_digest != "none" {
                failures.push(format!(
                    "command `{}` shared-source digest must be `none` when no shared sources are declared",
                    row.command
                ));
            }
        } else if !is_sha256_hex(&row.shared_source_digest) {
            failures.push(format!(
                "command `{}` shared-source digest must be a full SHA-256 hex digest",
                row.command
            ));
        }
        if !row
            .source_count_provenance
            .starts_with(SOURCE_COUNT_PROVENANCE_VERSION)
        {
            failures.push(format!(
                "command `{}` source-count provenance must start with `{SOURCE_COUNT_PROVENANCE_VERSION}`",
                row.command
            ));
        }
        let files_token = format!("files={}", row.source_file_count);
        if !row.source_count_provenance.contains(&files_token) {
            failures.push(format!(
                "command `{}` source-count provenance must record `{files_token}`",
                row.command
            ));
        }
        let loc_token = format!("loc={}", row.source_loc);
        if !row.source_count_provenance.contains(&loc_token) {
            failures.push(format!(
                "command `{}` source-count provenance must record `{loc_token}`",
                row.command
            ));
        }
        if !row.source_count_provenance.contains(&row.source_file) {
            failures.push(format!(
                "command `{}` source-count provenance must include primary source `{}`",
                row.command, row.source_file
            ));
        }
        for shared_source in &row.shared_sources {
            if !row.source_count_provenance.contains(shared_source) {
                failures.push(format!(
                    "command `{}` source-count provenance must include shared source `{shared_source}`",
                    row.command
                ));
            }
        }
    }
    failures
}

fn missing_research_affinity_contracts(rows: &[CommandMatrixRow]) -> Vec<String> {
    let mut failures = Vec::new();
    for contract in RESEARCH_AFFINITY_CONTRACTS {
        let Some(row) = rows.iter().find(|row| row.command == contract.command) else {
            failures.push(format!(
                "research-affinity command `{}` for `{}` is missing from the command matrix",
                contract.command, contract.affinity
            ));
            continue;
        };
        if !row
            .primary_evidence_artifact
            .contains(contract.proof_artifact_fragment)
        {
            failures.push(format!(
                "research-affinity command `{}` for `{}` must declare proof artifact containing `{}`",
                contract.command, contract.affinity, contract.proof_artifact_fragment
            ));
        }
        for required in contract.required_shared_sources {
            if !row
                .shared_sources
                .iter()
                .any(|shared_source| shared_source == required)
            {
                failures.push(format!(
                    "research-affinity command `{}` for `{}` must declare shared research source `{required}`",
                    contract.command, contract.affinity
                ));
            }
        }
    }
    failures
}

fn missing_required_duplicate_report_artifacts(rows: &[CommandMatrixRow]) -> Vec<String> {
    REQUIRED_DUPLICATE_REPORT_COMMANDS
        .iter()
        .filter_map(|command| {
            let Some(row) = rows.iter().find(|row| row.command == *command) else {
                return Some(format!(
                    "required duplicate-report command `{command}` is missing from the command matrix"
                ));
            };
            if row.primary_evidence_artifact == "none" {
                return Some(format!(
                    "duplicate-report command `{command}` must declare a primary release evidence artifact"
                ));
            }
            None
        })
        .collect()
}

fn mismatched_primary_evidence_artifacts(rows: &[CommandMatrixRow]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| {
            let expected = expected_artifacts_for_command(&row.command).first().copied()?;
            if row.primary_evidence_artifact == expected {
                return None;
            }
            Some(format!(
                "command `{}` primary evidence artifact `{}` must match release evidence registry `{expected}`",
                row.command, row.primary_evidence_artifact
            ))
        })
        .collect()
}

fn linked_vx_ids_for_row(row: &CommandMatrixRow, plan_text: &str) -> Vec<String> {
    let source_token = format!("`{}`", row.source_file);
    let command_token = format!("`{}`", row.command);
    let removal_token = format!("remove `{}`", row.command);
    plan_text
        .lines()
        .filter(|line| line.starts_with("| VX-"))
        .filter(|line| {
            line.contains(&source_token)
                || line.contains(&command_token)
                || line.contains(&removal_token)
        })
        .filter_map(|line| line.split('|').nth(1).map(str::trim))
        .map(str::to_string)
        .collect()
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_mode(args: &[String]) -> Result<Mode, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        process::exit(0);
    }
    let mut mode = Mode::Print;
    let mut index = 2usize;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--output requires a path".to_string());
                };
                mode = Mode::Write(PathBuf::from(path));
                index += 2;
            }
            "--check" => {
                mode = Mode::Check(PathBuf::from(CANONICAL_COMMAND_MATRIX));
                index += 1;
            }
            other => return Err(format!("unknown command-matrix option `{other}`")),
        }
    }
    Ok(mode)
}

fn print_usage() {
    eprintln!(
        "USAGE:\n  cargo_full run -p xtask --bin xtask -- command-matrix [--output PATH] [--check]\n\n\
         Generates the canonical xtask command matrix with owner lane, source-file count, submodule-aware LOC, proof kind, primary evidence artifact, duplicate-risk score, source digest, and source-count provenance."
    );
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .expect("Fix: xtask must live under the vyre workspace root.")
}

pub(crate) fn resolve_module_source(root: &Path, module: &str) -> Result<PathBuf, String> {
    let file = root.join("xtask/src").join(format!("{module}.rs"));
    if file.is_file() {
        return Ok(file);
    }
    let module_file = root.join("xtask/src").join(module).join("mod.rs");
    if module_file.is_file() {
        return Ok(module_file);
    }
    Err(format!(
        "xtask command module `{module}` has no `xtask/src/{module}.rs` or `xtask/src/{module}/mod.rs` source"
    ))
}

fn read_text_bounded(path: &Path, max_bytes: u64) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} exceeds {max_bytes} bytes", path.display()),
        ));
    }
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(text)
}

fn write_text(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Default for CommandMatrixRow {
        fn default() -> Self {
            Self {
                command: "fixture".to_string(),
                module: "fixture".to_string(),
                source_file: "xtask/src/fixture.rs".to_string(),
                shared_sources: Vec::new(),
                owner_lane: "coordination".to_string(),
                proof_kind: "utility".to_string(),
                primary_evidence_artifact: "none".to_string(),
                source_file_count: 1,
                source_loc: 1,
                duplicate_risk_score: 10,
                source_manifest_digest:
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                shared_source_digest: "none".to_string(),
                source_count_provenance:
                    "command-matrix-source-count:v1:files=1:loc=1:sources=xtask/src/fixture.rs"
                        .to_string(),
                generated_command: GENERATED_COMMAND.to_string(),
            }
        }
    }

    fn row<'a>(rows: &'a [CommandMatrixRow], command: &str) -> &'a CommandMatrixRow {
        rows.iter()
            .find(|row| row.command == command)
            .unwrap_or_else(|| panic!("missing row for {command}"))
    }

    fn fixture_source_stats(rel_path: &str, file_count: usize, loc: usize) -> SourceStats {
        let mut counted_sources = Vec::new();
        counted_sources.push(rel_path.to_string());
        for index in 1..file_count {
            counted_sources.push(format!("{rel_path}#{index}"));
        }
        SourceStats {
            rel_path: rel_path.to_string(),
            file_count,
            loc,
            source_manifest_digest:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            shared_source_digest: "none".to_string(),
            source_count_provenance: source_count_provenance(&counted_sources, loc),
        }
    }

    #[test]
    fn vx003_rows_include_required_targets_with_owner_and_proof_kind() {
        let specs = command_specs()
            .iter()
            .copied()
            .filter(|spec| {
                matches!(
                    spec.command,
                    "hot-path-scan" | "source-similar" | "acceleration-plan-gate"
                )
            })
            .collect::<Vec<_>>();
        let mut stats = BTreeMap::new();
        stats.insert(
            "hot_path_scan",
            fixture_source_stats("xtask/src/hot_path_scan.rs", 1, 400),
        );
        stats.insert(
            "source_similar",
            fixture_source_stats("xtask/src/source_similar.rs", 1, 1379),
        );
        stats.insert(
            "acceleration_plan_gate",
            fixture_source_stats("xtask/src/acceleration_plan_gate.rs", 1, 1523),
        );

        let rows = rows_from_stats(&specs, &stats, &[]);

        let hot = row(&rows, "hot-path-scan");
        assert_eq!(hot.owner_lane, "coordination");
        assert_eq!(hot.proof_kind, "hot-path-gate");
        assert_eq!(hot.source_file_count, 1);
        assert_eq!(hot.source_loc, 400);
        assert_eq!(hot.duplicate_risk_score, 34);

        let similar = row(&rows, "source-similar");
        assert_eq!(similar.owner_lane, "coordination");
        assert_eq!(similar.proof_kind, "dedup-gate");
        assert_eq!(similar.source_file_count, 1);
        assert_eq!(
            similar.primary_evidence_artifact,
            "release/evidence/dedup/source-similar-duplicates.json"
        );
        assert_eq!(similar.source_loc, 1379);
        assert_eq!(similar.duplicate_risk_score, 43);

        let plan = row(&rows, "acceleration-plan-gate");
        assert_eq!(plan.owner_lane, "coordination");
        assert_eq!(plan.proof_kind, "plan-gate");
        assert_eq!(plan.source_file_count, 1);
        assert_eq!(
            plan.primary_evidence_artifact,
            "release/evidence/optimization/acceleration-plan-progress.json"
        );
        assert_eq!(plan.source_loc, 1523);
        assert_eq!(plan.duplicate_risk_score, 45);
    }

    #[test]
    fn shared_source_helpers_are_declared_for_research_key_users() {
        assert_eq!(
            shared_sources_for_command("acceleration-plan-gate"),
            &[
                ARTIFACT_PATHS_SOURCE,
                HASH_SOURCE,
                INNOVATION_FALSIFICATION_SOURCE,
                MARKDOWN_TABLE_SOURCE,
                RESEARCH_BASIS_SOURCE,
                RESEARCH_KEY_SOURCE,
                RESEARCH_PLAN_COVERAGE_SOURCE,
                RESEARCH_SOURCE_LEDGER_SOURCE,
                RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
                RULES_AS_DATA_SOURCE,
                RULES_AS_DATA_MANIFEST_SOURCE,
                VX_PLAN_TABLE_SOURCE
            ]
        );
        assert_eq!(
            shared_sources_for_command("research-audit"),
            &[
                ARTIFACT_PATHS_SOURCE,
                HASH_SOURCE,
                INNOVATION_FALSIFICATION_SOURCE,
                MARKDOWN_TABLE_SOURCE,
                REPO_BOUNDARY_DATA_SOURCE,
                REPO_BOUNDARY_SOURCE,
                RESEARCH_BASIS_SOURCE,
                RESEARCH_KEY_SOURCE,
                RESEARCH_PLAN_COVERAGE_SOURCE,
                RESEARCH_SOURCE_LEDGER_SOURCE,
                RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
                COMPETITOR_ISSUE_LEDGER_DATA_SOURCE,
                ARCHIVE_REPLAY_AUDITS_DATA_SOURCE,
                RULES_AS_DATA_SOURCE,
                RULES_AS_DATA_MANIFEST_SOURCE,
                TOML_CONFIG_SOURCE,
                VX_PLAN_TABLE_SOURCE
            ]
        );
        assert!(shared_sources_for_command("whats-similar").is_empty());
    }

    #[test]
    fn shared_repo_boundary_helper_is_declared_for_public_launch_users() {
        assert_eq!(
            shared_sources_for_command("launch-state"),
            &[
                LAUNCH_CONTRACT_SOURCE,
                RELEASE_TRAIN_DATA_SOURCE,
                RELEASE_TRAIN_SOURCE,
                REPO_BOUNDARY_DATA_SOURCE,
                REPO_BOUNDARY_SOURCE,
                TOML_CONFIG_SOURCE
            ]
        );
        assert_eq!(
            shared_sources_for_command("release-completion-audit"),
            &[
                LAUNCH_CONTRACT_SOURCE,
                RELEASE_TRAIN_DATA_SOURCE,
                RELEASE_TRAIN_SOURCE,
                REPO_BOUNDARY_DATA_SOURCE,
                REPO_BOUNDARY_SOURCE,
                TOML_CONFIG_SOURCE
            ]
        );
        assert!(shared_sources_for_command("research-audit")
            .iter()
            .any(|source| *source == REPO_BOUNDARY_SOURCE));
    }

    #[test]
    fn shared_release_train_helper_is_declared_for_version_story_users() {
        assert_eq!(
            shared_sources_for_command("version-matrix"),
            &[
                RELEASE_TRAIN_DATA_SOURCE,
                RELEASE_TRAIN_SOURCE,
                TOML_CONFIG_SOURCE
            ]
        );
        assert_eq!(
            shared_sources_for_command("vyre-release-gate"),
            &[
                RELEASE_TRAIN_DATA_SOURCE,
                RELEASE_TRAIN_SOURCE,
                TOML_CONFIG_SOURCE
            ]
        );
        assert_eq!(
            shared_sources_for_command("vyre-weir-release-gate"),
            &[
                RELEASE_TRAIN_DATA_SOURCE,
                RELEASE_TRAIN_SOURCE,
                TOML_CONFIG_SOURCE
            ]
        );
        assert_eq!(
            shared_sources_for_command("metadata-matrix"),
            &[
                RELEASE_TRAIN_DATA_SOURCE,
                RELEASE_TRAIN_SOURCE,
                TOML_CONFIG_SOURCE
            ]
        );
        assert_eq!(
            shared_sources_for_command("package-readiness"),
            &[
                RELEASE_TRAIN_DATA_SOURCE,
                RELEASE_TRAIN_SOURCE,
                TOML_CONFIG_SOURCE
            ]
        );
    }

    #[test]
    fn shared_ownership_helper_is_declared_for_owner_resolution_users() {
        assert_eq!(
            shared_sources_for_command("command-matrix"),
            &[
                ARTIFACT_PATHS_SOURCE,
                HASH_SOURCE,
                OWNERSHIP_SOURCE,
                EXPECTED_ARTIFACTS_SOURCE,
                RELEASE_BENCHMARKS_SOURCE,
                RESEARCH_AUDIT_SOURCE
            ]
        );
        assert_eq!(
            shared_sources_for_command("hot-path-scan"),
            &[OWNERSHIP_SOURCE]
        );
        assert_eq!(
            shared_sources_for_command("source-similar"),
            &[OWNERSHIP_SOURCE]
        );
        assert!(shared_sources_for_command("research-audit")
            .iter()
            .all(|source| *source != OWNERSHIP_SOURCE));
    }

    #[test]
    fn shared_frontier_leaderboard_data_is_declared_for_release_benchmarks() {
        assert_eq!(
            shared_sources_for_command("release-benchmarks"),
            &[
                ARTIFACT_PATHS_SOURCE,
                FRONTIER_LEADERBOARD_BASELINES_DATA_SOURCE,
                HASH_SOURCE,
                RESEARCH_KEY_SOURCE,
                RESEARCH_SOURCE_LEDGER_SOURCE,
                RESEARCH_SOURCE_LEDGER_DATA_SOURCE,
                TOML_CONFIG_SOURCE
            ]
        );
        assert!(shared_sources_for_command("release-evidence")
            .iter()
            .any(|source| *source == RELEASE_BENCHMARKS_SOURCE));
    }

    #[test]
    fn research_affinity_contracts_reject_missing_sources_and_artifacts() {
        let rows = vec![CommandMatrixRow {
            command: "release-benchmarks".to_string(),
            module: "release_benchmarks".to_string(),
            source_file: "xtask/src/release_benchmarks.rs".to_string(),
            shared_sources: Vec::new(),
            owner_lane: "testing_evidence".to_string(),
            proof_kind: "benchmark".to_string(),
            primary_evidence_artifact: "none".to_string(),
            ..CommandMatrixRow::default()
        }];

        let failures = missing_research_affinity_contracts(&rows);

        assert!(failures
            .iter()
            .any(|failure| failure.contains("release/evidence/benchmarks/")));
        assert!(failures.iter().any(|failure| failure.contains(
            "docs/optimization/FRONTIER_LEADERBOARD_BASELINES.toml"
        )));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("xtask/src/research_key.rs")));
    }

    #[test]
    fn render_markdown_includes_matrix_columns() {
        let rows = vec![CommandMatrixRow {
            command: "hot-path-scan".to_string(),
            module: "hot_path_scan".to_string(),
            source_file: "xtask/src/hot_path_scan.rs".to_string(),
            shared_sources: vec![OWNERSHIP_SOURCE.to_string()],
            owner_lane: "coordination".to_string(),
            proof_kind: "hot-path-gate".to_string(),
            primary_evidence_artifact: "none".to_string(),
            source_file_count: 1,
            source_loc: 400,
            duplicate_risk_score: 34,
                ..CommandMatrixRow::default()
        }];

        let rendered = render_markdown(&rows);

        assert!(rendered.contains("| Command | Owner lane | Proof kind | Primary evidence artifact | Source files | Source LOC | Duplicate-risk score | Source file | Shared sources |"));
        assert!(rendered.contains("| `hot-path-scan` | `coordination` | `hot-path-gate` | `none` | 1 | 400 | 34 | `xtask/src/hot_path_scan.rs` | `xtask/src/ownership.rs` |"));
    }

    #[test]
    fn module_source_totals_include_command_submodules() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let src = root.join("xtask/src");
        std::fs::create_dir_all(src.join("demo")).unwrap();
        std::fs::write(src.join("demo.rs"), "one\ntwo\n").unwrap();
        std::fs::write(src.join("demo/helper.rs"), "three\nfour\nfive\n").unwrap();
        let source = resolve_module_source(root, "demo").unwrap();

        let source_stats =
            module_source_stats(root, "demo", &source, "xtask/src/demo.rs", &[]).unwrap();

        assert_eq!(source_stats.file_count, 2);
        assert_eq!(source_stats.loc, 5);
        assert!(source_stats.source_count_provenance.contains("files=2"));
        assert!(is_sha256_hex(&source_stats.source_manifest_digest));
    }

    #[test]
    fn duplicate_report_commands_require_release_artifacts() {
        let rows = vec![
            CommandMatrixRow {
                command: "lego-audit".to_string(),
                module: "lego_audit".to_string(),
                source_file: "xtask/src/lego_audit.rs".to_string(),
                shared_sources: Vec::new(),
                owner_lane: "coordination".to_string(),
                proof_kind: "dedup-gate".to_string(),
                primary_evidence_artifact: "release/evidence/dedup/lego-audit-duplicates.json"
                    .to_string(),
                source_file_count: 1,
                source_loc: 1351,
                duplicate_risk_score: 43,
                ..CommandMatrixRow::default()
            },
            CommandMatrixRow {
                command: "whats-similar".to_string(),
                module: "whats_similar".to_string(),
                source_file: "xtask/src/whats_similar.rs".to_string(),
                shared_sources: Vec::new(),
                owner_lane: "coordination".to_string(),
                proof_kind: "dedup-gate".to_string(),
                primary_evidence_artifact:
                    "release/evidence/dedup/registered-op-duplicates.json".to_string(),
                source_file_count: 1,
                source_loc: 777,
                duplicate_risk_score: 37,
                ..CommandMatrixRow::default()
            },
            CommandMatrixRow {
                command: "source-similar".to_string(),
                module: "source_similar".to_string(),
                source_file: "xtask/src/source_similar.rs".to_string(),
                shared_sources: vec![OWNERSHIP_SOURCE.to_string()],
                owner_lane: "coordination".to_string(),
                proof_kind: "dedup-gate".to_string(),
                primary_evidence_artifact:
                    "release/evidence/dedup/source-similar-duplicates.json".to_string(),
                source_file_count: 1,
                source_loc: 1379,
                duplicate_risk_score: 43,
                ..CommandMatrixRow::default()
            },
        ];

        assert!(missing_required_duplicate_report_artifacts(&rows).is_empty());

        let missing = vec![CommandMatrixRow {
            command: "lego-audit".to_string(),
            module: "lego_audit".to_string(),
            source_file: "xtask/src/lego_audit.rs".to_string(),
            shared_sources: Vec::new(),
            owner_lane: "coordination".to_string(),
            proof_kind: "dedup-gate".to_string(),
            primary_evidence_artifact: "none".to_string(),
            source_file_count: 1,
            source_loc: 1351,
            duplicate_risk_score: 43,
                ..CommandMatrixRow::default()
        }];

        let failures = missing_required_duplicate_report_artifacts(&missing);
        assert!(failures
            .iter()
            .any(|failure| failure.contains("lego-audit")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("whats-similar")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("source-similar")));
    }

    #[test]
    fn primary_evidence_artifact_must_match_release_registry() {
        let rows = vec![CommandMatrixRow {
            command: "release-evidence".to_string(),
            module: "release_evidence".to_string(),
            source_file: "xtask/src/release_evidence.rs".to_string(),
            shared_sources: Vec::new(),
            owner_lane: "testing_evidence".to_string(),
            proof_kind: "release-evidence".to_string(),
            primary_evidence_artifact: "none".to_string(),
            source_file_count: 1,
            source_loc: 881,
            duplicate_risk_score: 43,
                ..CommandMatrixRow::default()
        }];

        let failures = mismatched_primary_evidence_artifacts(&rows);

        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("release/evidence/final/release-evidence-run.json"));
    }

    #[test]
    fn high_risk_command_requires_vx_link() {
        let rows = vec![CommandMatrixRow {
            command: "lego-audit".to_string(),
            module: "lego_audit".to_string(),
            source_file: "xtask/src/lego_audit.rs".to_string(),
            shared_sources: Vec::new(),
            owner_lane: "coordination".to_string(),
            proof_kind: "dedup-gate".to_string(),
            primary_evidence_artifact: "release/evidence/dedup/lego-audit-duplicates.json"
                .to_string(),
            source_file_count: 1,
            source_loc: 1351,
            duplicate_risk_score: 43,
                ..CommandMatrixRow::default()
        }];

        let failures = missing_high_risk_vx_links(&rows, "# Plan\n");

        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("lego-audit"));
        assert!(failures[0].contains("xtask/src/lego_audit.rs"));
    }

    #[test]
    fn high_risk_command_passes_when_source_file_is_in_vx_plan() {
        let rows = vec![CommandMatrixRow {
            command: "lego-audit".to_string(),
            module: "lego_audit".to_string(),
            source_file: "xtask/src/lego_audit.rs".to_string(),
            shared_sources: Vec::new(),
            owner_lane: "coordination".to_string(),
            proof_kind: "dedup-gate".to_string(),
            primary_evidence_artifact: "release/evidence/dedup/lego-audit-duplicates.json"
                .to_string(),
            source_file_count: 1,
            source_loc: 1351,
            duplicate_risk_score: 43,
                ..CommandMatrixRow::default()
        }];
        let plan =
            "| VX-114 | testing_evidence | `xtask/src/lego_audit.rs` owns dedup evidence. |\n";

        assert!(missing_high_risk_vx_links(&rows, plan).is_empty());
    }

    #[test]
    fn command_provenance_contract_rejects_malformed_rows() {
        let mut row = CommandMatrixRow {
            command: "command-matrix".to_string(),
            module: "command_matrix".to_string(),
            source_file: "xtask/src/command_matrix.rs".to_string(),
            shared_sources: vec![HASH_SOURCE.to_string()],
            source_manifest_digest: "bad".to_string(),
            shared_source_digest: "none".to_string(),
            source_count_provenance: "manual".to_string(),
            generated_command: "cargo xtask command-matrix".to_string(),
            ..CommandMatrixRow::default()
        };
        row.source_file_count = 2;
        row.source_loc = 99;

        let failures = missing_command_provenance_contracts(&[row]);

        assert!(failures
            .iter()
            .any(|failure| failure.contains("generated-command provenance")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("source manifest digest")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("shared-source digest")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains(SOURCE_COUNT_PROVENANCE_VERSION)));
        assert!(failures.iter().any(|failure| failure.contains("files=2")));
        assert!(failures.iter().any(|failure| failure.contains("loc=99")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("xtask/src/command_matrix.rs")));
        assert!(failures.iter().any(|failure| failure.contains(HASH_SOURCE)));
    }

    #[test]
    fn linked_vx_ids_are_extracted_from_plan_rows() {
        let row = CommandMatrixRow {
            command: "command-matrix".to_string(),
            source_file: "xtask/src/command_matrix.rs".to_string(),
            duplicate_risk_score: 53,
            ..CommandMatrixRow::default()
        };
        let plan = "| VX-413 | evidence_truth | `xtask/src/command_matrix.rs` owns matrix provenance. | `MLIR_PASS` | Improvement: add contracts. | Gate. | Seam. |\n";

        assert_eq!(linked_vx_ids_for_row(&row, plan), vec!["VX-413"]);
        assert!(missing_high_risk_vx_links(&[row], plan).is_empty());
    }
}

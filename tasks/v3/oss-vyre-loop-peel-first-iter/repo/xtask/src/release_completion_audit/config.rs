use std::path::{Path, PathBuf};

use super::ChecklistItem;

pub(crate) struct Config {
    pub manifest: PathBuf,
    pub output: PathBuf,
}

pub(crate) fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut manifest = None;
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --manifest requires a path.".to_string());
                };
                manifest = Some(PathBuf::from(path));
                index += 2;
            }
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- release-completion-audit [--manifest PATH] [--output PATH]\n\n\
                     Writes final prompt-to-artifact release audit evidence."
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "Fix: unknown release-completion-audit option `{other}`."
                ));
            }
        }
    }
    Ok(Config {
        manifest: manifest.unwrap_or_else(default_manifest),
        output: output.unwrap_or_else(default_output),
    })
}

pub(crate) fn default_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/vyre-release-evidence.toml"))
        .unwrap_or_else(|| PathBuf::from("release/vyre-release-evidence.toml"))
}

pub(crate) fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/final/completion-audit.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/final/completion-audit.json"))
}

pub(crate) fn release_checklist() -> Vec<ChecklistItem> {
    vec![
        ChecklistItem {
            requirement_id: "version-story",
            explicit_requirement: "Vyre manifests, dependency hints, lockfile path packages, docs, release notes, packaging, and product-scoped tags use the selected Vyre 0.6.3 / Weir 0.1.0 version story.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- version-matrix --output release/evidence/version/version-matrix.json",
                "release/evidence/version/version-matrix.json",
                "release/evidence/version/release-tag-plan.json",
                "release/evidence/docs/release-notes-version-story.md",
            ],
        },
        ChecklistItem {
            requirement_id: "cuda-first-path",
            explicit_requirement: "CUDA is the documented, benchmarked, optimized primary release substrate.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- backend-matrix --output release/evidence/backends/backend-matrix.json",
                "cargo_full run --bin xtask -- release-benchmarks --backend cuda",
                "release/evidence/benchmarks/cuda-release-suite.json",
                "release/evidence/benchmarks/cuda-ptx-patterns.json",
                "release/evidence/benchmarks/bench-release-axes.json",
            ],
        },
        ChecklistItem {
            requirement_id: "wgpu-fallback",
            explicit_requirement: "WGPU remains a real fallback path with conformance and benchmark evidence, not an untested branch.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- release-benchmarks --backend wgpu",
                "release/evidence/backends/backend-matrix.json",
                "release/evidence/benchmarks/wgpu-fallback-suite.json",
                "release/evidence/conformance/wgpu-conformance.json",
                "release/evidence/docs/wgpu-fallback-proof.md",
            ],
        },
        ChecklistItem {
            requirement_id: "megakernel-default",
            explicit_requirement: "Megakernel is the default high-throughput path where legal and has latency/condition workload evidence.",
            required_artifacts_or_commands: vec![
                "release/evidence/backends/backend-matrix.json",
                "release/evidence/benchmarks/release-workload-matrix.json",
                "release/evidence/benchmarks/megakernel-condition-cuda.json",
                "release/evidence/benchmarks/megakernel-latency-cuda.json",
                "release/evidence/docs/megakernel-default-proof.md",
            ],
        },
        ChecklistItem {
            requirement_id: "optimization-corpus-4096",
            explicit_requirement: "Optimization scale is proved by at least 4096 verified semantic-preserving generated cases.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- optimization-corpus --output release/evidence/optimization/optimization-corpus.json",
                "release/evidence/optimization/optimization-corpus.json",
                "release/evidence/optimization/optimization-corpus-contracts.json",
                "release/evidence/optimization/optimization-family-manifest.json",
                "release/evidence/optimization/optimization-analysis-fixtures.json",
                "release/evidence/optimization/optimization-case-manifest.json",
            ],
        },
        ChecklistItem {
            requirement_id: "optimization-benchmark-proof",
            explicit_requirement: "Optimization families have before/after benchmark evidence and pass-family attribution.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- optimization-matrix --output release/evidence/optimization/optimization-integration-matrix.json",
                "release/evidence/optimization/lower-rewrite-impact-before-after.json",
                "release/evidence/optimization/optimizer-impact-cuda.json",
                "release/evidence/optimization/pass-family-benchmarks.json",
                "release/evidence/optimization/pass-family-benchmark-manifest.json",
            ],
        },
        ChecklistItem {
            requirement_id: "alias-aware-upgrades",
            explicit_requirement: "DSE, store-to-load forwarding, LICM, loop fusion, and loop fission consume Weir alias facts and prove before/after impact.",
            required_artifacts_or_commands: vec![
                "release/evidence/optimization/alias-aware-dse.json",
                "release/evidence/optimization/alias-aware-stlf.json",
                "release/evidence/optimization/alias-aware-licm.json",
                "release/evidence/optimization/alias-aware-fusion-fission.json",
                "release/evidence/benchmarks/alias-aware-before-after.json",
            ],
        },
        ChecklistItem {
            requirement_id: "egraph-saturation",
            explicit_requirement: "Bounded e-graph or egglog-family saturation is present where it beats hand rewrites and has semantic plus benchmark evidence.",
            required_artifacts_or_commands: vec![
                "release/evidence/optimization/egraph-saturation-matrix.json",
                "release/evidence/optimization/egraph-semantic-contracts.json",
                "release/evidence/benchmarks/egraph-before-after.json",
                "release/evidence/docs/egraph-saturation.md",
            ],
        },
        ChecklistItem {
            requirement_id: "weir-analysis-integration",
            explicit_requirement: "Weir exposes SSA, IFDS, reaching defs, points-to, alias, callgraph, slicing, summaries, loops, and fixpoint facts, and Vyre consumes those facts where they unlock safer optimization.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- weir-matrix --output release/evidence/weir/weir-analysis-api-matrix.json",
                "release/evidence/weir/weir-analysis-api-matrix.json",
                "release/evidence/weir/weir-vyre-integration-tests.json",
                "release/evidence/weir/weir-readme-contracts.json",
                "../../../../dataflow/weir/Cargo.toml",
                "release/evidence/optimization/weir-facts-pass-firing.json",
                "release/evidence/benchmarks/dataflow-analysis-release.json",
                "release/evidence/docs/weir-integration.md",
            ],
        },
        ChecklistItem {
            requirement_id: "proof-workloads-12",
            explicit_requirement: "At least 12 proof workload families have reproducible CPU/GPU benchmark artifacts.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- release-workload-matrix --output release/evidence/benchmarks/release-workload-matrix.json --enforce",
                "release/evidence/benchmarks/release-workload-matrix.json",
                "../docs/optimization/BENCH_TARGETS.toml",
                "release/evidence/benchmarks/workload-01-condition-eval.json",
                "release/evidence/benchmarks/workload-02-string-bitmap-scatter.json",
                "release/evidence/benchmarks/workload-03-offset-count-aggregation.json",
                "release/evidence/benchmarks/workload-04-metadata-conditions.json",
                "release/evidence/benchmarks/workload-05-entropy-window.json",
                "release/evidence/benchmarks/workload-06-quantified-condition-loops.json",
                "release/evidence/benchmarks/workload-07-alias-reaching-def.json",
                "release/evidence/benchmarks/workload-08-ifds-witness.json",
                "release/evidence/benchmarks/workload-09-c-ast-traversal.json",
                "release/evidence/benchmarks/workload-10-megakernel-queued-batches.json",
                "release/evidence/benchmarks/workload-11-egraph-saturation.json",
                "release/evidence/benchmarks/workload-12-sparse-output-compaction.json",
                "release/evidence/benchmarks/workload-13-callgraph-reachability.json",
            ],
        },
        ChecklistItem {
            requirement_id: "cpu-only-100x-proof",
            explicit_requirement: "At least ten release workloads prove 100x+ CUDA wins against CPU-SOTA baselines.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- release-workload-matrix --output release/evidence/benchmarks/release-workload-matrix.json --enforce",
                "release/evidence/benchmarks/release-workload-matrix.json",
                "release/evidence/benchmarks/cpu-only-100x-proof.json",
                "release/evidence/benchmarks/megakernel-condition-100x-proof.json",
                "release/evidence/benchmarks/workload-01-condition-eval.json",
                "release/evidence/benchmarks/workload-02-string-bitmap-scatter.json",
                "release/evidence/benchmarks/workload-03-offset-count-aggregation.json",
                "release/evidence/benchmarks/workload-05-entropy-window.json",
                "release/evidence/benchmarks/workload-06-quantified-condition-loops.json",
                "release/evidence/benchmarks/workload-10-megakernel-queued-batches.json",
                "release/evidence/benchmarks/workload-12-sparse-output-compaction.json",
                "release/evidence/docs/cpu-only-100x-proof.md",
            ],
        },
        ChecklistItem {
            requirement_id: "c-parser-linux-subsystem",
            explicit_requirement: "The distributed C parser parses the selected full Linux subsystem corpus with AST, diagnostic, and throughput evidence.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- c-parser-corpus --corpus DIR -I LINUX_INCLUDE_DIR -D CONFIG_SYMBOL=1 --output release/evidence/parser/c-parser-linux-subsystem.json",
                "release/evidence/parser/c-parser-linux-subsystem.json",
                "release/evidence/parser/linux-subsystem-corpus-manifest.json",
                "release/evidence/parser/c-parser-diagnostics-summary.json",
                "release/evidence/parser/c-parser-throughput.json",
            ],
        },
        ChecklistItem {
            requirement_id: "distributed-parser-coherence",
            explicit_requirement: "vyre-frontend-c, tools/vyrec, Weir, and Surge/SurgeC parser ownership boundaries are coherent and documented.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- parser-coherence --output release/evidence/parser/distributed-parser-map.json",
                "release/evidence/parser/distributed-parser-map.json",
                "release/evidence/parser/vyre-frontend-c-contracts.json",
                "release/evidence/parser/vyrec-cli-contracts.json",
                "release/evidence/parser/weir-contracts.json",
                "release/evidence/parser/surgec-contracts.json",
                "release/evidence/parser/surgec-grammar-gen-contracts.json",
                "release/evidence/docs/distributed-parser-coherence.md",
            ],
        },
        ChecklistItem {
            requirement_id: "conformance-hard-gate",
            explicit_requirement: "Conformance blocks release for every claimed op/backend pair.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- conformance-matrix --output release/evidence/conformance/conformance-matrix.json",
                "cargo_full run --bin xtask -- release-conformance --backend all",
                "release/evidence/conformance/cuda-conformance.json",
                "release/evidence/conformance/wgpu-conformance.json",
                "release/evidence/conformance/reference-conformance.json",
                "release/evidence/conformance/release-gate-log.json",
                "../../../../../.github/workflows/conform.yml",
                "../../../../../.github/workflows/gpu-parity.yml",
                "../../../../../.github/workflows/santh-ci.yml",
                "../../../../../.github/workflows/architectural-invariants.yml",
                "../../../../../.github/CI_REQUIRED.md",
                "../../../../../scripts/apply-branch-protection.sh",
            ],
        },
        ChecklistItem {
            requirement_id: "modular-test-architecture",
            explicit_requirement: "Massive tests are split into fixtures, contracts, properties, backend tests, corpus tests, benchmarks, and regressions.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- test-matrix --output release/evidence/tests/test-matrix.json",
                "release/evidence/tests/test-matrix.json",
                "release/evidence/tests/modularization-map.json",
                "release/evidence/tests/oversized-test-closure.json",
                "release/evidence/tests/release-surface-suite-coverage.json",
                "release/evidence/docs/test-architecture.md",
            ],
        },
        ChecklistItem {
            requirement_id: "exhaustive-verification",
            explicit_requirement: "Normal, adversarial, property, conformance, corpus, benchmark, gap, and fuzz verification are present.",
            required_artifacts_or_commands: vec![
                "release/evidence/tests/unit-suite.json",
                "release/evidence/tests/adversarial-suite.json",
                "release/evidence/tests/property-suite.json",
                "release/evidence/tests/conformance-suite.json",
                "release/evidence/tests/corpus-suite.json",
                "release/evidence/tests/benchmark-suite.json",
                "release/evidence/tests/gap-suite.json",
                "release/evidence/tests/fuzz-suite.json",
                "release/evidence/tests/release-surface-suite-coverage.json",
            ],
        },
        ChecklistItem {
            requirement_id: "docs-evidence-linked",
            explicit_requirement: "User, contributor, benchmark, conformance, parser, optimization, and release docs link to concrete evidence.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- docs-matrix --output release/evidence/docs/docs-matrix.json",
                "release/evidence/docs/docs-matrix.json",
                "release/evidence/docs/vyre-readme-contracts.json",
                "release/evidence/docs/vyre-readme-proof.md",
                "release/evidence/docs/weir-readme-proof.md",
                "release/evidence/docs/parser-doc-proof.md",
                "release/evidence/docs/benchmark-doc-proof.md",
                "release/evidence/docs/conformance-doc-proof.md",
                "release/evidence/docs/release-notes.md",
            ],
        },
        ChecklistItem {
            requirement_id: "crate-metadata",
            explicit_requirement: "Every release crate has coherent metadata, features, docs, readme, license, and version policy.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- metadata-matrix --output release/evidence/metadata/metadata-matrix.json",
                "cargo_full run --bin xtask -- feature-matrix --output release/evidence/metadata/feature-matrix.json",
                "cargo_full run --bin xtask -- package-readiness --output release/evidence/package/publish-readiness.json",
                "release/evidence/metadata/metadata-matrix.json",
                "release/evidence/metadata/feature-matrix.json",
                "release/evidence/package/publish-readiness.json",
                "release/evidence/docs/crate-metadata-proof.md",
            ],
        },
        ChecklistItem {
            requirement_id: "release-hygiene",
            explicit_requirement: "No shipped stubs, hidden fallbacks, unbounded resources, hot-path sleeps, unactionable errors, or stale docs remain.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- hygiene-matrix --output release/evidence/hygiene/hygiene-matrix.json",
                "release/evidence/hygiene/hygiene-matrix.json",
                "release/evidence/hygiene/threshold-policy.json",
                "release/evidence/hygiene/no-stubs-scan.json",
                "release/evidence/hygiene/no-hidden-fallback-scan.json",
                "release/evidence/hygiene/resource-bound-scan.json",
                "release/evidence/hygiene/error-surface-scan.json",
                "release/evidence/hygiene/cargo-wrapper-scan.json",
                "release/evidence/hygiene/audit-location-scan.json",
                "release/evidence/hygiene/public-doc-scan.json",
                "release/evidence/hygiene/test-hygiene-scan.json",
                "release/evidence/docs/release-hygiene-proof.md",
                "../../../../../.github/workflows/architectural-invariants.yml",
                "../../../../../.github/CI_REQUIRED.md",
                "../../../../../scripts/apply-branch-protection.sh",
                "../../../../../scripts/architectural_invariants.sh",
            ],
        },
        ChecklistItem {
            requirement_id: "final-completion-audit",
            explicit_requirement: "The final audit maps every explicit requirement, named artifact, generator command, and gate to evidence and refuses completion while any requirement is open, blocked, missing, empty, or semantically weak.",
            required_artifacts_or_commands: vec![
                "cargo_full run --bin xtask -- release-evidence",
                "release/evidence/final/release-evidence-run.json",
                "cargo_full run --bin xtask -- release-completion-audit --output release/evidence/final/completion-audit.json",
                "cargo_full run --bin xtask -- vyre-release-gate",
                "cargo_full run --bin xtask -- launch-state --output release/evidence/final/public-launch-state.json",
                "release/evidence/final/completion-audit.json",
                "release/evidence/final/public-launch-state.json",
                "release/../scripts/final-launch.sh",
                "release/../scripts/publish-release.sh",
                "release/vyre-release-evidence.toml",
            ],
        },
    ]
}

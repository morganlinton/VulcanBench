//! Completion audit coverage for `paradigm-shift-100-concrete.md`.

use std::collections::BTreeSet;

use crate::release_validation_matrix::RELEASE_VALIDATION_MATRIX;

/// Validated 100-item plan audit proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParadigmShiftPlanAuditProof {
    /// Number of numbered plan items found.
    pub item_count: usize,
    /// Number of unique release gates referenced by the item map.
    pub gate_count: usize,
}

/// Plan-audit validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParadigmShiftPlanAuditError {
    /// Required release-definition text is missing.
    MissingPlanText {
        /// Missing text class.
        field: &'static str,
    },
    /// A numbered plan item is missing.
    MissingItem {
        /// Missing item number.
        item: u8,
    },
    /// A plan item has no release evidence mapping.
    MissingEvidence {
        /// Item number.
        item: u8,
    },
    /// An item maps to a release gate that is not in the matrix.
    UnknownGate {
        /// Item number.
        item: u8,
        /// Gate id.
        gate: &'static str,
    },
}

impl std::fmt::Display for ParadigmShiftPlanAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPlanText { field } => write!(
                f,
                "paradigm-shift plan audit missing {field}. Fix: audit the active 100-item plan, not a stale release artifact."
            ),
            Self::MissingItem { item } => write!(
                f,
                "paradigm-shift plan audit missing numbered item {item}. Fix: every item 1..100 must be present and mapped."
            ),
            Self::MissingEvidence { item } => write!(
                f,
                "paradigm-shift plan item {item} has no evidence gates. Fix: map the item to concrete release validation gates."
            ),
            Self::UnknownGate { item, gate } => write!(
                f,
                "paradigm-shift plan item {item} references unknown release gate `{gate}`. Fix: add the gate or correct the item mapping."
            ),
        }
    }
}

impl std::error::Error for ParadigmShiftPlanAuditError {}

/// Validate that the active 100-item plan is fully mapped to current release gates.
pub fn validate_paradigm_shift_plan_audit(
    plan_text: &str,
) -> Result<ParadigmShiftPlanAuditProof, ParadigmShiftPlanAuditError> {
    validate_plan_identity(plan_text)?;
    let items = numbered_items(plan_text);
    for item in 1..=100 {
        if !items.contains(&item) {
            return Err(ParadigmShiftPlanAuditError::MissingItem { item });
        }
    }

    let available_gates = RELEASE_VALIDATION_MATRIX
        .iter()
        .map(|gate| gate.id)
        .collect::<BTreeSet<_>>();
    let mut referenced_gates = BTreeSet::new();
    for item in 1..=100 {
        let evidence = evidence_for_item(item);
        if evidence.is_empty() {
            return Err(ParadigmShiftPlanAuditError::MissingEvidence { item });
        }
        for gate in evidence {
            if !available_gates.contains(gate) {
                return Err(ParadigmShiftPlanAuditError::UnknownGate { item, gate });
            }
            referenced_gates.insert(*gate);
        }
    }

    Ok(ParadigmShiftPlanAuditProof {
        item_count: 100,
        gate_count: referenced_gates.len(),
    })
}

fn validate_plan_identity(plan_text: &str) -> Result<(), ParadigmShiftPlanAuditError> {
    for (field, needle) in [
        (
            "active plan title",
            "# GPU-First Dataflow/Compiler Platform Paradigm-Shift Launch Plan",
        ),
        (
            "vyre/dataflow/vyrec scope",
            "Scope: platform, dataflow, and compiler frontend only.",
        ),
        (
            "completion definition",
            "code, tests, benchmarks, docs, packaging, and public launch path",
        ),
        ("cargo_full publish final step", "cargo_full publish"),
        ("git push final step", "git push"),
    ] {
        if !plan_text.contains(needle) {
            return Err(ParadigmShiftPlanAuditError::MissingPlanText { field });
        }
    }
    Ok(())
}

fn numbered_items(plan_text: &str) -> BTreeSet<u8> {
    let mut items = BTreeSet::new();
    for line in plan_text.lines() {
        let trimmed = line.trim_start();
        let Some((number, _)) = trimmed.split_once(". ") else {
            continue;
        };
        if number.chars().all(|ch| ch.is_ascii_digit()) {
            if let Ok(item) = number.parse::<u8>() {
                items.insert(item);
            }
        }
    }
    items
}

fn evidence_for_item(item: u8) -> &'static [&'static str] {
    match item {
        1 => &[
            "vyre-production-cpu-fallback-lint",
            "vyre-core-gpu-boundary",
        ],
        2 => &["vyre-gpu-probe-contract"],
        3 => &["vyre-gpu-probe-contract", "vyre-architecture-boundary-map"],
        4 => &["vyre-memory-ownership-contract"],
        5 => &["vyre-architecture-boundary-map"],
        6 => &[
            "vyre-cuda-pipeline-modularity",
            "vyre-contributor-module-map",
        ],
        7 => &[
            "vyre-contributor-module-map",
            "release-test-taxonomy-coverage",
        ],
        8 => &["vyre-contributor-module-map", "release-scope-docs"],
        9 => &["vyre-public-api-boundary"],
        10 => &[
            "vyre-production-cpu-fallback-lint",
            "vyre-core-gpu-boundary",
        ],
        11 => &[
            "vyre-cuda-resident-dispatch",
            "release-allocation-regression",
        ],
        12 => &[
            "vyre-cuda-resident-dispatch",
            "vyre-memory-ownership-contract",
        ],
        13 => &[
            "vyre-cuda-resident-dispatch",
            "release-allocation-regression",
        ],
        14 => &["vyre-cuda-megakernel-scheduler"],
        15 => &["vyre-cuda-frontier-queue", "resident-fixed-point"],
        16 => &[
            "cross-crate-perf-contracts",
            "structural-benchmark-pass-selection",
            "vyre-cuda-launch-fusion",
        ],
        17 => &["vyre-cuda-megakernel-scheduler", "vyre-cuda-convergence"],
        18 => &[
            "vyre-cuda-megakernel-scheduler",
            "structural-frontier-partitioning",
        ],
        19 => &["vyre-cuda-megakernel-scheduler", "vyre-cuda-frontier-queue"],
        20 => &[
            "vyre-cuda-megakernel-scheduler",
            "structural-benchmark-pass-selection",
        ],
        21 => &["vyre-cuda-megakernel-scheduler", "release-gpu-evidence"],
        22 => &[
            "vyre-cuda-megakernel-speedup",
            "release-benchmark-baselines",
        ],
        23 => &[
            "vyre-cuda-frontier-queue",
            "vyre-cuda-convergence",
            "vyre-cuda-device-work-queue",
        ],
        24 => &[
            "vyre-cuda-frontier-queue",
            "structural-multi-corpus-batching",
            "vyre-cuda-multi-query-execution",
        ],
        25 => &["vyre-cuda-convergence"],
        26 => &[
            "vyre-cuda-module-cache",
            "vyre-cuda-megakernel-scheduler",
            "release-cuda-ptx-pattern-evidence",
        ],
        27 => &[
            "cross-crate-perf-contracts",
            "structural-diagnostic-aggregation",
            "vyre-cuda-result-compaction",
        ],
        28 => &[
            "vyre-cuda-capability-contracts",
            "vyre-cuda-cooperative-launch",
            "vyre-cuda-kernel-failure-diagnostics",
        ],
        29 => &[
            "vyre-cuda-megakernel-speedup",
            "release-benchmark-baselines",
        ],
        30 => &[
            "release-benchmark-baselines",
            "resident-fixed-point-benchmark",
        ],
        31 => &["resident-fixed-point", "release-allocation-regression"],
        32 => &["resident-fixed-point", "property-reaching-escapes"],
        33 => &["resident-fixed-point", "analysis-coverage"],
        34 => &["resident-fixed-point", "property-points-to"],
        35 => &["resident-fixed-point", "property-slice"],
        36 => &["ifds-resident", "property-ifds"],
        37 => &["graph-layout-coverage", "resident-fixed-point"],
        38 => &["graph-layout-coverage"],
        39 => &["resident-fixed-point", "ifds-resident"],
        40 => &["graph-layout-coverage"],
        41 => &["property-points-to", "property-ifds", "property-slice"],
        42 => &["adversarial-oracles", "release-hostile-input-coverage"],
        43 => &["exact-primitive-parity", "fuzz-bitset-oracles"],
        44 => &["resident-fixed-point", "release-allocation-regression"],
        45 => &[
            "resident-fixed-point-benchmark",
            "ifds-direct-resident-benchmark",
        ],
        46 => &["exact-primitive-parity", "analysis-coverage"],
        47 => &["analysis-coverage", "property-points-to"],
        48 => &["analysis-coverage", "structural-benchmark-pass-selection"],
        49 => &["graph-layout-coverage"],
        50 => &["release-scope-docs", "analysis-coverage"],
        51 => &["vyrec-beta-contract", "release-scope-docs"],
        52 => &["vyrec-c-dialect-matrix", "vyrec-clang-parity-dashboard"],
        53 => &[
            "vyrec-c-preprocess-gpu-resident-state",
            "vyrec-gpu-preprocessing-coverage",
        ],
        54 => &["vyrec-diagnostic-comparison", "structural-token-fact-graph"],
        55 => &[
            "vyrec-gpu-preprocessing-coverage",
            "vyrec-c-parser-throughput-evidence",
        ],
        56 => &["vyrec-gpu-preprocessing-coverage"],
        57 => &[
            "vyrec-c-parser-throughput-evidence",
            "structural-multi-corpus-batching",
        ],
        58 => &[
            "vyrec-gpu-preprocessing-coverage",
            "vyrec-c-parser-throughput-evidence",
        ],
        59 => &[
            "vyrec-parser-semantic-safety",
            "vyrec-diagnostic-comparison",
        ],
        60 => &["semantic-parity-coverage", "vyrec-clang-parity-dashboard"],
        61 => &[
            "vyrec-linux-corpus-parity",
            "vyrec-c-parser-throughput-evidence",
        ],
        62 => &["vyrec-linux-corpus-parity", "release-gap-findings"],
        63 => &["vyrec-diagnostic-comparison"],
        64 => &["vyrec-parser-semantic-safety", "semantic-parity-coverage"],
        65 => &[
            "vyrec-c-parser-throughput-evidence",
            "release-benchmark-baselines",
        ],
        66 => &[
            "vyrec-c-parser-throughput-evidence",
            "release-benchmark-baselines",
        ],
        67 => &["release-scope-docs", "vyrec-beta-contract"],
        68 => &["vyrec-clang-parity-dashboard", "release-gap-findings"],
        69 => &[
            "release-hostile-input-coverage",
            "release-test-taxonomy-coverage",
        ],
        70 => &["release-hostile-input-coverage", "scale-oracle-no-oom"],
        71 => &["structural-token-fact-graph"],
        72 => &["structural-incremental-invalidation"],
        73 => &["structural-multi-corpus-batching"],
        74 => &["structural-frontier-typed-ir"],
        75 => &[
            "structural-frontier-typed-ir",
            "vyre-cuda-megakernel-scheduler",
        ],
        76 => &["structural-frontier-partitioning"],
        77 => &[
            "structural-diagnostic-aggregation",
            "vyre-cuda-device-diagnostic-aggregation",
        ],
        78 => &[
            "optimization-control-plane",
            "optimization-release-evidence",
        ],
        79 => &[
            "optimization-control-plane",
            "optimization-release-evidence",
            "cross-crate-perf-contracts",
            "vyre-cuda-self-optimizer-e2e",
            "vyre-cuda-self-optimizer-const-prop",
            "vyre-cuda-self-optimizer-cse",
            "vyre-cuda-self-optimizer-licm",
            "vyre-cuda-self-optimizer-dead-branch",
            "vyre-cuda-self-optimizer-pattern-match",
            "vyre-cuda-self-optimizer-pipeline-resident",
        ],
        80 => &["structural-optimization-composition"],
        81 => &[
            "structural-benchmark-pass-selection",
            "vyre-cuda-benchmark-pass-selection",
        ],
        82 => &[
            "optimization-control-plane",
            "optimization-release-evidence",
        ],
        83 => &["structural-optimization-composition"],
        84 => &[
            "vyre-cuda-megakernel-scheduler",
            "vyre-memory-ownership-contract",
        ],
        85 => &["cross-crate-perf-contracts"],
        86 => &["release-checklist-gate", "release-gpu-evidence"],
        87 => &["release-test-taxonomy-coverage"],
        88 => &["release-gap-findings", "vyrec-clang-parity-dashboard"],
        89 => &["vyre-production-cpu-fallback-lint"],
        90 => &["release-allocation-regression"],
        91 => &["release-benchmark-baselines"],
        92 => &["vyrec-linux-corpus-parity"],
        93 => &["release-hostile-input-coverage"],
        94 => &["release-public-api-doctests"],
        95 => &["vyre-contributor-module-map", "release-scope-docs"],
        96 => &["release-scope-docs", "vyrec-beta-contract"],
        97 => &["release-deep-review-gate"],
        98 => &["release-gpu-evidence"],
        99 => &["release-checklist-gate", "release-completion-audit-honesty"],
        100 => &[
            "release-crate-metadata-readiness",
            "release-launch-sequence",
            "release-completion-audit-honesty",
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_plan_maps_all_100_items_to_release_gates() {
        let proof = validate_paradigm_shift_plan_audit(include_str!(
            "../../../../release/plans/paradigm-shift-100-concrete.md"
        ))
        .expect("Fix: active 100-item plan should map to release validation gates");

        assert_eq!(proof.item_count, 100);
        assert!(
            proof.gate_count >= 50,
            "Fix: the 100-item plan must map to broad concrete release evidence, not a tiny proxy set."
        );
    }

    #[test]
    fn plan_audit_rejects_stale_release_story() {
        let err = validate_paradigm_shift_plan_audit(include_str!(
            "../../../../release/evidence/final/completion-audit.json"
        ))
        .expect_err("stale final audit artifact must not satisfy the active 100-item plan");

        assert_eq!(
            err,
            ParadigmShiftPlanAuditError::MissingPlanText {
                field: "active plan title",
            }
        );
    }

    #[test]
    fn plan_audit_references_cuda_innovation_gates() {
        let referenced = (1..=100)
            .flat_map(evidence_for_item)
            .copied()
            .collect::<BTreeSet<_>>();

        for gate in [
            "vyre-cuda-device-work-queue",
            "vyre-cuda-launch-fusion",
            "vyre-cuda-multi-query-execution",
            "vyre-cuda-result-compaction",
            "vyre-cuda-device-diagnostic-aggregation",
            "vyre-cuda-kernel-failure-diagnostics",
            "vyre-cuda-benchmark-pass-selection",
            "release-cuda-ptx-pattern-evidence",
            "optimization-release-evidence",
            "vyre-cuda-self-optimizer-e2e",
            "vyre-cuda-self-optimizer-const-prop",
            "vyre-cuda-self-optimizer-cse",
            "vyre-cuda-self-optimizer-licm",
            "vyre-cuda-self-optimizer-dead-branch",
            "vyre-cuda-self-optimizer-pattern-match",
            "vyre-cuda-self-optimizer-pipeline-resident",
        ] {
            assert!(
                referenced.contains(gate),
                "Fix: CUDA innovation gate `{gate}` must map back to a numbered paradigm-shift plan item."
            );
        }
    }
}

//! `flows_to_with_sanitizer`  -  composite source→sink reachability
//! with explicit sanitizer kill, in one fused Region.
//!
//! This is the CodeQL `DataFlow::Configuration` shape compressed into
//! a single emitted Program:
//!
//! ```text
//!   clean    = source AND NOT sanitizers
//!   reach    = csr_forward_traverse(clean, FLOWS_TO_MASK)
//!   alive    = reach AND NOT sanitizers
//!   hits     = alive AND sink
//!   any_hit  = bitset_any(hits) → u32
//! ```
//!
//! Downstream analyzer rules currently express this composition as a chain of
//! three predicates (`flows_to($src, $sink)` AND `not sanitized_by($src, @san)`).
//! That works but emits three separate dispatches and intermediate
//! buffers. Centralising it here lets the rule write one `lhs`-shaped
//! predicate that the optimizer fuses, caches, and CSEs across rules.
//!
//! Soundness: [`Exact`](vyre::soundness::Soundness::Exact)
//! when iterated to fixpoint with the same sanitizer mask supplied
//! at every step. One step alone is
//! [`MayOver`](vyre::soundness::Soundness::MayOver)  -  the
//! caller is responsible for the fixpoint loop, which is the same
//! contract every other reachability primitive in this module honours.

use vyre::ir::Program;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

#[cfg(test)]
use crate::security::flow_composition::sanitized_dataflow_hit_cpu_ref;
use crate::security::flow_composition::sanitized_dataflow_hit_program;

pub(crate) const OP_ID: &str = "vyre-libs::security::flows_to_with_sanitizer";
/// Stable primitive id for a converged sanitizer-gated source-to-sink fixpoint.
pub const FIXPOINT_OP_ID: &str = "vyre-libs::security::flows_to_with_sanitizer::fixpoint";

/// Execution mode for sanitizer-gated source-to-sink flow.
///
/// `OneStep` is the single Region emitted by [`flows_to_with_sanitizer`].
/// It is an intermediate Weir taint state, not a final vulnerability proof.
/// `FixpointConverged` is the contract a driver emits after repeatedly
/// applying the same sanitizer-gated step until a no-change check succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SanitizedFlowExecutionMode {
    /// One sanitizer-gated propagation step.
    OneStep,
    /// The sanitizer-gated propagation loop reached a no-change fixpoint.
    FixpointConverged {
        /// Number of driver iterations completed before convergence was observed.
        iterations: u32,
    },
}

/// Mode-aware soundness contract for sanitizer-gated flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SanitizedFlowSoundnessContract {
    /// Execution mode that produced the flow result.
    pub mode: SanitizedFlowExecutionMode,
    /// Stable primitive id to place in finding evidence.
    pub op_id: &'static str,
    /// Soundness marker for this mode.
    pub soundness: crate::dataflow::Soundness,
    /// Whether the result is bounded by an explicit sanitizer mask.
    pub sanitizer_filter: bool,
    /// Shared fact kind Weir should use when writing this result.
    pub weir_fact_kind: crate::dataflow::SharedFactKind,
    /// Stable Weir role string for blackboard/fact consumers.
    pub weir_role: &'static str,
}

/// Rejection for attempts to use an intermediate sanitizer-flow step as a
/// final proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SanitizedFlowContractViolation {
    /// Rejected execution mode.
    pub mode: SanitizedFlowExecutionMode,
    /// Soundness marker that made the mode invalid for final proof evidence.
    pub soundness: crate::dataflow::Soundness,
    /// Operator-facing fix direction.
    pub fix: &'static str,
}

impl SanitizedFlowExecutionMode {
    /// Return true when the execution mode carries a converged fixpoint proof.
    #[must_use]
    pub const fn is_converged_fixpoint(self) -> bool {
        matches!(self, Self::FixpointConverged { .. })
    }

    /// Stable label for logs, Weir roles, and tests.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OneStep => "one_step",
            Self::FixpointConverged { .. } => "fixpoint_converged",
        }
    }
}

impl SanitizedFlowSoundnessContract {
    /// Convert this contract into serializable primitive evidence for findings.
    #[must_use]
    pub fn primitive_soundness(&self) -> crate::dataflow::DynamicPrimitiveSoundness {
        let evidence = crate::dataflow::DynamicPrimitiveSoundness::new(self.op_id, self.soundness);
        if self.sanitizer_filter {
            evidence.with_sanitizer_filter()
        } else {
            evidence
        }
    }
}

/// Return the mode-aware sanitizer-flow soundness contract.
#[must_use]
pub const fn sanitized_flow_soundness_contract(
    mode: SanitizedFlowExecutionMode,
) -> SanitizedFlowSoundnessContract {
    match mode {
        SanitizedFlowExecutionMode::OneStep => SanitizedFlowSoundnessContract {
            mode,
            op_id: OP_ID,
            soundness: crate::dataflow::Soundness::MayOver,
            sanitizer_filter: true,
            weir_fact_kind: crate::dataflow::SharedFactKind::Taint,
            weir_role: "weir.flow.one_step.sanitizer_gated",
        },
        SanitizedFlowExecutionMode::FixpointConverged { .. } => SanitizedFlowSoundnessContract {
            mode,
            op_id: FIXPOINT_OP_ID,
            soundness: crate::dataflow::Soundness::Exact,
            sanitizer_filter: false,
            weir_fact_kind: crate::dataflow::SharedFactKind::Witness,
            weir_role: "weir.flow.fixpoint_converged.sanitizer_gated",
        },
    }
}

/// Return final-proof sanitizer-flow evidence, rejecting one-step results.
///
/// Finding builders should use this helper when they need proof-grade evidence
/// rather than intermediate Weir taint state.
///
/// # Errors
///
/// Returns [`SanitizedFlowContractViolation`] when `mode` is not a converged
/// fixpoint.
pub fn sanitized_flow_final_soundness_contract(
    mode: SanitizedFlowExecutionMode,
) -> Result<SanitizedFlowSoundnessContract, SanitizedFlowContractViolation> {
    let contract = sanitized_flow_soundness_contract(mode);
    if mode.is_converged_fixpoint() {
        Ok(contract)
    } else {
        Err(SanitizedFlowContractViolation {
            mode,
            soundness: contract.soundness,
            fix: "Fix: run the sanitizer-gated flow driver to a no-change fixpoint before emitting final proof evidence.",
        })
    }
}

/// Return serializable final finding evidence for sanitizer-gated flow.
///
/// # Errors
///
/// Returns [`SanitizedFlowContractViolation`] when `mode` is not a converged
/// fixpoint.
pub fn sanitized_flow_final_finding_soundness(
    mode: SanitizedFlowExecutionMode,
) -> Result<crate::dataflow::DynamicPrimitiveSoundness, SanitizedFlowContractViolation> {
    sanitized_flow_final_soundness_contract(mode).map(|contract| contract.primitive_soundness())
}

/// Build one BFS step of `source \ sanitizers` along dataflow edges,
/// re-killed by `sanitizers` on landing, intersected with `sink`,
/// reduced to a single u32 in `out_scalar_buf`.
///
/// Buffer ownership:
/// * `source_buf`, `sink_buf`, `sanitizer_buf`  -  read-only.
/// * `clean_buf`, `reach_buf`, `alive_buf`, `hits_buf`  -  read-write
///   scratch sized to `bitset_words(shape.node_count)`.
/// * `out_scalar_buf`  -  read-write 1-word output, nonzero iff any
///   non-sanitized source-reachable bit overlaps with sink.
#[must_use]
pub fn flows_to_with_sanitizer(
    shape: ProgramGraphShape,
    source_buf: &str,
    sink_buf: &str,
    sanitizer_buf: &str,
    clean_buf: &str,
    reach_buf: &str,
    alive_buf: &str,
    hits_buf: &str,
    out_scalar_buf: &str,
) -> Program {
    sanitized_dataflow_hit_program(
        OP_ID,
        shape,
        source_buf,
        sink_buf,
        sanitizer_buf,
        clean_buf,
        reach_buf,
        alive_buf,
        hits_buf,
        out_scalar_buf,
    )
}

/// CPU oracle: full one-step semantic for differential testing
/// against the GPU emit.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    source: &[u32],
    sink: &[u32],
    sanitizer: &[u32],
) -> u32 {
    sanitized_dataflow_hit_cpu_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        source,
        sink,
        sanitizer,
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || flows_to_with_sanitizer(ProgramGraphShape::new(4, 3), "source", "sink", "sanitizer", "clean", "reach", "alive", "hits", "out_scalar"),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[0b0001]),              // source = {0}
                to_bytes(&[0b0000]),              // sanitizer = {}
                to_bytes(&[0b0001]),              // clean = {0}
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 1, 2, 3, 3]),       // pg_edge_offsets
                to_bytes(&[1, 2, 3]),             // pg_edge_targets
                to_bytes(&[
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                ]),                               // pg_edge_kind_mask
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0b0001]),              // reach = {0}
                to_bytes(&[0b0000]),              // alive
                to_bytes(&[0b0010]),              // sink = {1}
                to_bytes(&[0b0000]),              // hits
                to_bytes(&[0b0000]),              // out_scalar
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[0b0001]),              // clean = {0}
                to_bytes(&[0b0011]),              // reach = {0,1}
                to_bytes(&[0b0011]),              // alive = {0,1}
                to_bytes(&[0b0010]),              // hits = {1}
                to_bytes(&[0b0001]),              // out_scalar = 1
            ]]
        }),
        category: Some("security"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::facts::{
        finding_from_sanitized_source_to_sink_query, AnalysisFact, AnalysisFactTable,
        AnalysisSourceSpan, FactId, FactKind, SourceToSinkFindingRequest,
    };
    use crate::security::flow_composition::linear_dataflow;
    use crate::dataflow::{PrecisionContract, Soundness};

    #[test]
    fn sanitizer_flow_contract_labels_one_step_and_weir_fixpoint_distinctly() {
        let one_step = sanitized_flow_soundness_contract(SanitizedFlowExecutionMode::OneStep);
        let fixpoint = sanitized_flow_soundness_contract(
            SanitizedFlowExecutionMode::FixpointConverged { iterations: 4 },
        );

        assert_eq!(one_step.mode.label(), "one_step");
        assert_eq!(one_step.op_id, OP_ID);
        assert_eq!(one_step.soundness, Soundness::MayOver);
        assert!(one_step.sanitizer_filter);
        assert_eq!(one_step.weir_fact_kind, crate::dataflow::SharedFactKind::Taint);
        assert_eq!(one_step.weir_role, "weir.flow.one_step.sanitizer_gated");

        assert_eq!(fixpoint.mode.label(), "fixpoint_converged");
        assert_eq!(fixpoint.op_id, FIXPOINT_OP_ID);
        assert_eq!(fixpoint.soundness, Soundness::Exact);
        assert!(!fixpoint.sanitizer_filter);
        assert_eq!(
            fixpoint.weir_fact_kind,
            crate::dataflow::SharedFactKind::Witness
        );
        assert_eq!(
            fixpoint.weir_role,
            "weir.flow.fixpoint_converged.sanitizer_gated"
        );
    }

    #[test]
    fn final_sanitizer_flow_contract_rejects_one_step_as_final_proof() {
        let error = sanitized_flow_final_soundness_contract(SanitizedFlowExecutionMode::OneStep)
            .expect_err("Fix: one-step sanitizer flow must not become final proof evidence");

        assert_eq!(error.mode, SanitizedFlowExecutionMode::OneStep);
        assert_eq!(error.soundness, Soundness::MayOver);
        assert!(error.fix.contains("no-change fixpoint"));
    }

    #[test]
    fn final_sanitizer_flow_finding_evidence_requires_exact_fixpoint_tag() {
        let evidence = sanitized_flow_final_finding_soundness(
            SanitizedFlowExecutionMode::FixpointConverged { iterations: 4 },
        )
        .expect("Fix: converged sanitizer flow should emit final finding evidence");

        assert_eq!(evidence.op_id, FIXPOINT_OP_ID);
        assert_eq!(evidence.soundness, Soundness::Exact);
        assert!(!evidence.sanitizer_filter);

        let soundness = crate::dataflow::validate_dynamic_pipeline(
            PrecisionContract::ZeroFalsePositive,
            &[evidence],
        )
        .expect("Fix: exact fixpoint evidence must satisfy zero-FP finding contracts");

        assert_eq!(soundness, Soundness::Exact);
    }

    #[test]
    fn unsanitized_source_reaches_sink_returns_one() {
        let (off, tgt, msk) = linear_dataflow(4);
        // 0 → 1 → 2 → 3, source = {0}, sink = {1}, no sanitizer.
        let result = cpu_ref(4, &off, &tgt, &msk, &[0b0001], &[0b0010], &[0]);
        assert_eq!(result, 1);
    }

    #[test]
    fn source_killed_by_sanitizer_returns_zero() {
        let (off, tgt, msk) = linear_dataflow(4);
        // Sanitizer covers the source itself  -  nothing flows.
        let result = cpu_ref(4, &off, &tgt, &msk, &[0b0001], &[0b0010], &[0b0001]);
        assert_eq!(result, 0);
    }

    #[test]
    fn landing_killed_by_sanitizer_returns_zero() {
        let (off, tgt, msk) = linear_dataflow(4);
        // Source = {0}, sink = {1}, sanitizer = {1}  -  sink itself is
        // sanitized, so the landing kill drops it before the AND-sink.
        let result = cpu_ref(4, &off, &tgt, &msk, &[0b0001], &[0b0010], &[0b0010]);
        assert_eq!(result, 0);
    }

    #[test]
    fn unrelated_sanitizer_passes_through() {
        let (off, tgt, msk) = linear_dataflow(4);
        // Sanitizer covers node 3 (downstream of sink)  -  irrelevant.
        let result = cpu_ref(4, &off, &tgt, &msk, &[0b0001], &[0b0010], &[0b1000]);
        assert_eq!(result, 1);
    }

    #[test]
    fn empty_source_returns_zero() {
        let (off, tgt, msk) = linear_dataflow(4);
        let result = cpu_ref(4, &off, &tgt, &msk, &[0], &[0b0010], &[0]);
        assert_eq!(result, 0);
    }

    #[test]
    fn empty_sink_returns_zero() {
        let (off, tgt, msk) = linear_dataflow(4);
        let result = cpu_ref(4, &off, &tgt, &msk, &[0b0001], &[0], &[0]);
        assert_eq!(result, 0);
    }

    #[test]
    fn unsanitized_query_hit_emits_fact_backed_finding_bundle() {
        let (off, tgt, msk) = linear_dataflow(4);
        let hit = cpu_ref(4, &off, &tgt, &msk, &[0b0001], &[0b0010], &[0b1000]);
        assert_eq!(hit, 1);

        let table = source_sink_table(3);
        let bundle = finding_from_sanitized_source_to_sink_query(
            &table,
            SourceToSinkFindingRequest {
                finding_id: "finding.security.unsanitized-source-to-sink".to_string(),
                query_id: OP_ID.to_string(),
                backend_id: "cpu-ref".to_string(),
                evidence_digest: "evidence:test".to_string(),
                precision_contract: PrecisionContract::ZeroFalsePositive,
                source_fact_id: FactId(1),
                sink_fact_id: FactId(3),
                path_fact_ids: vec![FactId(2)],
                sanitizer_fact_ids: vec![FactId(4)],
                query_hit: hit,
                confidence_bps: 9900,
                reason: "source reaches sink and considered sanitizer does not kill the path"
                    .to_string(),
            },
        )
        .expect("Fix: positive sanitized source-to-sink query should build proof bundle")
        .expect("Fix: positive sanitized source-to-sink query should emit finding");

        assert_eq!(bundle.query_id, OP_ID);
        assert_eq!(bundle.precision_contract, PrecisionContract::ZeroFalsePositive);
        assert_eq!(bundle.soundness, Soundness::MayOver);
        assert_eq!(bundle.primitive_soundness.len(), 1);
        assert_eq!(bundle.primitive_soundness[0].op_id, OP_ID);
        assert_eq!(bundle.primitive_soundness[0].soundness, Soundness::MayOver);
        assert!(bundle.primitive_soundness[0].sanitizer_filter);
        assert_eq!(
            bundle.fact_ids,
            vec![FactId(1), FactId(2), FactId(4), FactId(3)]
        );
        assert_eq!(bundle.proof_path[0].role, "source");
        assert_eq!(bundle.proof_path[1].role, "dataflow-path");
        assert_eq!(bundle.proof_path[2].role, "sanitizer-considered");
        assert_eq!(bundle.proof_path[3].role, "sink");
    }

    #[test]
    fn sanitizer_killed_query_emits_no_finding_but_validates_considered_facts() {
        let (off, tgt, msk) = linear_dataflow(4);
        let hit = cpu_ref(4, &off, &tgt, &msk, &[0b0001], &[0b0010], &[0b0010]);
        assert_eq!(hit, 0);

        let table = source_sink_table(1);
        let bundle = finding_from_sanitized_source_to_sink_query(
            &table,
            SourceToSinkFindingRequest {
                finding_id: "finding.security.sanitized-source-to-sink".to_string(),
                query_id: OP_ID.to_string(),
                backend_id: "cpu-ref".to_string(),
                evidence_digest: "evidence:test".to_string(),
                precision_contract: PrecisionContract::ZeroFalsePositive,
                source_fact_id: FactId(1),
                sink_fact_id: FactId(3),
                path_fact_ids: vec![FactId(2)],
                sanitizer_fact_ids: vec![FactId(4)],
                query_hit: hit,
                confidence_bps: 9900,
                reason: "source-to-sink path is killed by sanitizer".to_string(),
            },
        )
        .expect("Fix: sanitized no-hit query should still validate referenced facts");

        assert_eq!(
            bundle, None,
            "Fix: sanitizer-killed source-to-sink query must not emit a finding."
        );
    }

    fn source_sink_table(sanitizer_subject: u64) -> AnalysisFactTable {
        let source = AnalysisFact::exact(
            FactId(1),
            FactKind::Source,
            AnalysisSourceSpan::byte_range(1, 0, 4),
            0,
        );
        let mut edge = AnalysisFact::exact(
            FactId(2),
            FactKind::Dataflow,
            AnalysisSourceSpan::byte_range(1, 5, 9),
            0,
        );
        edge.object = Some(1);
        edge.provenance.push(FactId(1));
        let sink = AnalysisFact::exact(
            FactId(3),
            FactKind::Sink,
            AnalysisSourceSpan::byte_range(1, 10, 14),
            1,
        );
        let sanitizer = AnalysisFact::exact(
            FactId(4),
            FactKind::Sanitizer,
            AnalysisSourceSpan::byte_range(1, 15, 19),
            sanitizer_subject,
        );
        AnalysisFactTable::new(vec![source, edge, sink, sanitizer])
    }
}

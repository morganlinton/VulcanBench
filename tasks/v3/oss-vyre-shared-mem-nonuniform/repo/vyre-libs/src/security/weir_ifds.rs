//! Bridge from Vyre security facts into Weir IFDS dispatch.
//!
//! Security owns source/sink/sanitizer facts. Weir owns IFDS exploded-graph
//! execution and witness extraction. This module is the narrow seam between
//! them: it validates that fact and graph buffers exist, builds a Weir IFDS
//! dispatch request, and returns [`external_dataflow_engine::reachability_witness::PathSeed`]
//! values for proof-path materialization.

use std::collections::BTreeMap;

use external_dataflow_engine::{
    ifds_gpu::{ifds_gpu_step, IfdsShape, OP_ID as WEIR_IFDS_GPU_OP_ID},
    reachability_witness::{ExtractedPath, PathSeed},
};
use vyre_foundation::ir::Program;

use crate::{
    dataflow::{DynamicPrimitiveSoundness, Soundness},
    security::facts::{
        AnalysisFact, AnalysisFactError, AnalysisFactTable, AnalysisSourceSpan, FactId, FactKind,
        FindingProofBundle,
        SourceToSinkFindingRequest,
    },
};

/// Backend id used when Vyre security routes taint through Weir IFDS.
pub const WEIR_IFDS_SECURITY_BACKEND_ID: &str = "weir-ifds-gpu";

/// Buffer names required to route a security taint query through Weir IFDS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeirIfdsSecurityBuffers {
    /// ProgramGraph CSR edge-offset buffer.
    pub pg_edge_offsets: String,
    /// ProgramGraph CSR edge-target buffer.
    pub pg_edge_targets: String,
    /// ProgramGraph edge-kind mask buffer.
    pub pg_edge_kind_mask: String,
    /// ProgramGraph node-tag buffer.
    pub pg_node_tags: String,
    /// Columnar fact-id buffer.
    pub fact_ids: String,
    /// Columnar fact-kind buffer.
    pub fact_kinds: String,
    /// Columnar fact-subject buffer.
    pub fact_subjects: String,
    /// Columnar fact-object buffer.
    pub fact_objects: String,
    /// Input IFDS frontier bitset.
    pub frontier_in: String,
    /// Output IFDS frontier bitset.
    pub frontier_out: String,
}

impl WeirIfdsSecurityBuffers {
    /// Build a buffer-name bundle.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        pg_edge_offsets: impl Into<String>,
        pg_edge_targets: impl Into<String>,
        pg_edge_kind_mask: impl Into<String>,
        pg_node_tags: impl Into<String>,
        fact_ids: impl Into<String>,
        fact_kinds: impl Into<String>,
        fact_subjects: impl Into<String>,
        fact_objects: impl Into<String>,
        frontier_in: impl Into<String>,
        frontier_out: impl Into<String>,
    ) -> Self {
        Self {
            pg_edge_offsets: pg_edge_offsets.into(),
            pg_edge_targets: pg_edge_targets.into(),
            pg_edge_kind_mask: pg_edge_kind_mask.into(),
            pg_node_tags: pg_node_tags.into(),
            fact_ids: fact_ids.into(),
            fact_kinds: fact_kinds.into(),
            fact_subjects: fact_subjects.into(),
            fact_objects: fact_objects.into(),
            frontier_in: frontier_in.into(),
            frontier_out: frontier_out.into(),
        }
    }

    fn validate(&self) -> Result<(), WeirIfdsSecurityRouteError> {
        for (field, value) in [
            ("pg_edge_offsets", &self.pg_edge_offsets),
            ("pg_edge_targets", &self.pg_edge_targets),
            ("pg_edge_kind_mask", &self.pg_edge_kind_mask),
            ("pg_node_tags", &self.pg_node_tags),
            ("fact_ids", &self.fact_ids),
            ("fact_kinds", &self.fact_kinds),
            ("fact_subjects", &self.fact_subjects),
            ("fact_objects", &self.fact_objects),
            ("frontier_in", &self.frontier_in),
            ("frontier_out", &self.frontier_out),
        ] {
            if value.trim().is_empty() {
                return Err(WeirIfdsSecurityRouteError::MissingBuffer { field });
            }
        }
        if self.frontier_in == self.frontier_out {
            return Err(WeirIfdsSecurityRouteError::AliasedFrontierBuffers {
                buffer: self.frontier_in.clone(),
            });
        }
        Ok(())
    }
}

/// Weir IFDS dispatch selected for a Vyre security source-to-sink query.
#[derive(Clone, Debug, PartialEq)]
pub struct WeirIfdsSecurityDispatch {
    /// Query id; currently [`external_dataflow_engine::ifds_gpu::OP_ID`].
    pub query_id: String,
    /// Backend id for finding evidence.
    pub backend_id: String,
    /// Weir exploded-supergraph shape.
    pub shape: IfdsShape,
    /// Checked exploded-node count.
    pub node_count: u32,
    /// Buffer names used by the dispatch route.
    pub buffers: WeirIfdsSecurityBuffers,
    /// Source fact used as the IFDS seed source.
    pub source_fact_id: FactId,
    /// Sink fact used as the IFDS seed target.
    pub sink_fact_id: FactId,
    /// Witness seeds returned to Weir reachability-witness extraction.
    pub witness_seeds: Vec<PathSeed>,
    /// Soundness evidence for final finding bundles.
    pub primitive_soundness: Vec<DynamicPrimitiveSoundness>,
}

impl WeirIfdsSecurityDispatch {
    /// Build the Weir IFDS GPU step Program for this dispatch route.
    ///
    /// # Errors
    ///
    /// Returns [`WeirIfdsSecurityRouteError`] if Weir rejects the shape.
    pub fn step_program(&self) -> Result<Program, WeirIfdsSecurityRouteError> {
        ifds_gpu_step(self.shape, &self.buffers.frontier_in, &self.buffers.frontier_out)
            .map_err(|reason| WeirIfdsSecurityRouteError::BuildProgram { reason })
    }
}

/// Routing failure while adapting security facts to Weir IFDS.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WeirIfdsSecurityRouteError {
    /// Security fact table validation failed before routing.
    #[error(transparent)]
    InvalidFacts {
        /// Underlying fact-table validation failure.
        #[from]
        source: AnalysisFactError,
    },
    /// A required graph or fact buffer name was empty.
    #[error("missing Weir IFDS buffer `{field}`. Fix: provide graph and fact buffers before routing through IFDS.")]
    MissingBuffer {
        /// Missing buffer field.
        field: &'static str,
    },
    /// Input and output frontier buffers used the same storage name.
    #[error("frontier buffer `{buffer}` is aliased. Fix: use distinct IFDS frontier input and output buffers.")]
    AliasedFrontierBuffers {
        /// Aliased buffer name.
        buffer: String,
    },
    /// A source or sink fact id was not present in the table.
    #[error("missing {role} fact {fact_id:?}. Fix: route only fact-backed source-to-sink queries.")]
    MissingRoleFact {
        /// Source or sink role.
        role: &'static str,
        /// Missing fact id.
        fact_id: FactId,
    },
    /// A fact had the wrong role for source/sink routing.
    #[error("{role} fact {fact_id:?} had kind {actual:?}. Fix: normalize source and sink facts before IFDS routing.")]
    InvalidRoleFactKind {
        /// Source or sink role.
        role: &'static str,
        /// Fact id with the wrong kind.
        fact_id: FactId,
        /// Actual fact kind.
        actual: FactKind,
    },
    /// Source or sink subject did not fit Weir witness node ids.
    #[error("{role} fact subject {subject} does not fit a u32 Weir node id. Fix: remap corpus node ids before IFDS routing.")]
    NodeIdOverflow {
        /// Source or sink role.
        role: &'static str,
        /// Original subject id.
        subject: u64,
    },
    /// Source or sink subject was outside the exploded-supergraph domain.
    #[error("{role} node {node_id} is outside IFDS node_count {node_count}. Fix: route with a shape matching the graph buffers.")]
    NodeOutOfDomain {
        /// Source or sink role.
        role: &'static str,
        /// Source or sink node id.
        node_id: u32,
        /// Checked Weir node count.
        node_count: u32,
    },
    /// Weir rejected the IFDS shape.
    #[error("{reason}")]
    InvalidShape {
        /// Weir shape-validation failure.
        reason: String,
    },
    /// Weir rejected Program construction.
    #[error("{reason}")]
    BuildProgram {
        /// Weir Program-construction failure.
        reason: String,
    },
}

/// One statement in a security finding witness path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityWitnessStatement {
    /// Source-language adapter id.
    pub adapter: String,
    /// Human-readable statement description.
    pub description: String,
    /// Repository-relative file.
    pub file: String,
    /// Statement node id.
    pub node_id: u32,
    /// Byte start, inclusive.
    pub byte_start: u32,
    /// Byte end, exclusive.
    pub byte_end: u32,
    /// Incoming edge kind from the previous statement, absent for the source.
    pub incoming_edge_kind: Option<u32>,
    /// Exact source bytes for this statement.
    pub source_bytes: Vec<u8>,
}

/// Security finding witness path built from a Weir extracted path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityFindingWitnessPath {
    /// Finding id this witness explains.
    pub finding_id: String,
    /// Rule id that emitted the finding.
    pub rule_id: String,
    /// Query id that produced the finding.
    pub query_id: String,
    /// Backend id that produced the finding.
    pub backend_id: String,
    /// Soundness marker carried by the finding.
    pub soundness: Soundness,
    /// Source span from the fact-backed finding.
    pub source_span: AnalysisSourceSpan,
    /// Sink span from the fact-backed finding.
    pub sink_span: AnalysisSourceSpan,
    /// Per-hop edge-kind masks, source-to-sink.
    pub edge_kinds: Vec<u32>,
    /// Ordered witness statements, source-to-sink.
    pub statements: Vec<SecurityWitnessStatement>,
}

/// Failure while attaching a Weir witness path to a security finding.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SecurityWitnessPathError {
    /// Rule id was blank.
    #[error("rule_id is blank. Fix: attach stable rule ids to Weir witness paths.")]
    EmptyRuleId,
    /// Weir extracted an empty path.
    #[error("Weir witness path is empty. Fix: only attach successful non-empty extracted paths.")]
    EmptyExtractedPath,
    /// Edge-kind count did not match path hops.
    #[error("edge kind count {edge_kinds} does not match path hop count {hops}. Fix: provide one edge kind per witness transition.")]
    EdgeKindCountMismatch {
        /// Supplied edge kind count.
        edge_kinds: usize,
        /// Required path hop count.
        hops: usize,
    },
    /// Source or sink proof role was absent from the finding.
    #[error("finding {finding_id} has no `{role}` proof step. Fix: build fact-backed source and sink proof roles before witness attachment.")]
    MissingProofRole {
        /// Finding id being converted.
        finding_id: String,
        /// Missing role.
        role: &'static str,
    },
    /// Source bytes for a witness statement file were unavailable.
    #[error("source bytes for `{file}` are missing. Fix: pass source bytes for every file referenced by the Weir path.")]
    MissingSourceBytes {
        /// Missing file path.
        file: String,
    },
    /// Statement byte range did not fit the supplied source bytes.
    #[error("statement span {byte_start}..{byte_end} is invalid for `{file}` with {source_len} bytes. Fix: use the same source snapshot for extraction and reporting.")]
    InvalidStatementSpan {
        /// File containing the invalid statement.
        file: String,
        /// Statement byte start.
        byte_start: u32,
        /// Statement byte end.
        byte_end: u32,
        /// Available source byte length.
        source_len: usize,
    },
}

/// Attach a Weir extracted path to a Vyre security finding.
///
/// # Errors
///
/// Returns [`SecurityWitnessPathError`] when the rule id, finding proof roles,
/// edge-kind list, or source-byte slices are invalid.
pub fn security_witness_path_from_weir(
    finding: &FindingProofBundle,
    rule_id: impl Into<String>,
    extracted_path: &ExtractedPath,
    edge_kinds: &[u32],
    source_files: &BTreeMap<String, Vec<u8>>,
) -> Result<SecurityFindingWitnessPath, SecurityWitnessPathError> {
    let rule_id = rule_id.into();
    if rule_id.trim().is_empty() {
        return Err(SecurityWitnessPathError::EmptyRuleId);
    }
    if extracted_path.statements.is_empty() {
        return Err(SecurityWitnessPathError::EmptyExtractedPath);
    }
    let hops = extracted_path.statements.len().saturating_sub(1);
    if edge_kinds.len() != hops {
        return Err(SecurityWitnessPathError::EdgeKindCountMismatch {
            edge_kinds: edge_kinds.len(),
            hops,
        });
    }
    let source_span = proof_role_span(finding, "source")?;
    let sink_span = proof_role_span(finding, "sink")?;
    let mut statements = Vec::with_capacity(extracted_path.statements.len());
    for (index, statement) in extracted_path.statements.iter().enumerate() {
        let file_bytes =
            source_files
                .get(&statement.file)
                .ok_or_else(|| SecurityWitnessPathError::MissingSourceBytes {
                    file: statement.file.clone(),
                })?;
        let start = statement.byte_start as usize;
        let end = statement.byte_end as usize;
        if end < start || end > file_bytes.len() {
            return Err(SecurityWitnessPathError::InvalidStatementSpan {
                file: statement.file.clone(),
                byte_start: statement.byte_start,
                byte_end: statement.byte_end,
                source_len: file_bytes.len(),
            });
        }
        statements.push(SecurityWitnessStatement {
            adapter: statement.adapter.clone(),
            description: statement.description.clone(),
            file: statement.file.clone(),
            node_id: statement.node_id,
            byte_start: statement.byte_start,
            byte_end: statement.byte_end,
            incoming_edge_kind: index.checked_sub(1).map(|edge_index| edge_kinds[edge_index]),
            source_bytes: file_bytes[start..end].to_vec(),
        });
    }
    Ok(SecurityFindingWitnessPath {
        finding_id: finding.finding_id.clone(),
        rule_id,
        query_id: finding.query_id.clone(),
        backend_id: finding.backend_id.clone(),
        soundness: finding.soundness,
        source_span,
        sink_span,
        edge_kinds: edge_kinds.to_vec(),
        statements,
    })
}

/// Route a fact-backed security source-to-sink query through Weir IFDS.
///
/// This does not execute the GPU dispatch. It validates the route, records the
/// exact Weir primitive evidence, and produces witness seeds for the Weir
/// reachability-witness layer to materialize path statements.
///
/// # Errors
///
/// Returns [`WeirIfdsSecurityRouteError`] when facts, buffers, or shape are not
/// valid for IFDS routing.
pub fn route_security_taint_through_weir_ifds(
    table: &AnalysisFactTable,
    request: &SourceToSinkFindingRequest,
    shape: IfdsShape,
    buffers: WeirIfdsSecurityBuffers,
) -> Result<WeirIfdsSecurityDispatch, WeirIfdsSecurityRouteError> {
    table.validate()?;
    buffers.validate()?;
    let node_count = shape
        .node_count()
        .map_err(|reason| WeirIfdsSecurityRouteError::InvalidShape { reason })?;
    let source = require_role_fact(table, request.source_fact_id, "source", FactKind::Source)?;
    let sink = require_role_fact(table, request.sink_fact_id, "sink", FactKind::Sink)?;
    let source_node = fact_node_id(source, "source", node_count)?;
    let sink_node = fact_node_id(sink, "sink", node_count)?;
    let witness_seed = PathSeed {
        source_file: fact_file(source),
        source_node,
        sink_file: fact_file(sink),
        sink_node,
    };

    Ok(WeirIfdsSecurityDispatch {
        query_id: WEIR_IFDS_GPU_OP_ID.to_string(),
        backend_id: WEIR_IFDS_SECURITY_BACKEND_ID.to_string(),
        shape,
        node_count,
        buffers,
        source_fact_id: source.id,
        sink_fact_id: sink.id,
        witness_seeds: vec![witness_seed],
        primitive_soundness: vec![DynamicPrimitiveSoundness::new(
            WEIR_IFDS_GPU_OP_ID,
            Soundness::Exact,
        )],
    })
}

fn require_role_fact<'a>(
    table: &'a AnalysisFactTable,
    fact_id: FactId,
    role: &'static str,
    expected: FactKind,
) -> Result<&'a AnalysisFact, WeirIfdsSecurityRouteError> {
    let fact = table
        .get(fact_id)
        .ok_or(WeirIfdsSecurityRouteError::MissingRoleFact { role, fact_id })?;
    if fact.kind != expected {
        return Err(WeirIfdsSecurityRouteError::InvalidRoleFactKind {
            role,
            fact_id,
            actual: fact.kind,
        });
    }
    Ok(fact)
}

fn fact_node_id(
    fact: &AnalysisFact,
    role: &'static str,
    node_count: u32,
) -> Result<u32, WeirIfdsSecurityRouteError> {
    let node_id =
        u32::try_from(fact.subject).map_err(|_| WeirIfdsSecurityRouteError::NodeIdOverflow {
            role,
            subject: fact.subject,
        })?;
    if node_id >= node_count {
        return Err(WeirIfdsSecurityRouteError::NodeOutOfDomain {
            role,
            node_id,
            node_count,
        });
    }
    Ok(node_id)
}

fn fact_file(fact: &AnalysisFact) -> String {
    fact.payload
        .get("file")
        .or_else(|| fact.payload.get("path"))
        .cloned()
        .unwrap_or_else(|| format!("file:{}", fact.span.file_id))
}

fn proof_role_span(
    finding: &FindingProofBundle,
    role: &'static str,
) -> Result<AnalysisSourceSpan, SecurityWitnessPathError> {
    finding
        .proof_path
        .iter()
        .find(|step| step.role == role)
        .map(|step| step.span.clone())
        .ok_or_else(|| SecurityWitnessPathError::MissingProofRole {
            finding_id: finding.finding_id.clone(),
            role,
        })
}

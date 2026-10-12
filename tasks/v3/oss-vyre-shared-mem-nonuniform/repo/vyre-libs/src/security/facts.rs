//! Canonical security-analysis fact and finding proof schema.
//!
//! This module is intentionally data-only. The GPU kernels remain the existing
//! Lego primitives (`flows_to`, `sanitizer_dominates`, `auth_check_dominates`,
//! `path_reconstruct`, and graph primitives). These types define the stable
//! source-to-fact and finding-to-evidence contract those kernels consume and
//! emit through higher-level analysis pipelines.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::dataflow::{
    validate_dynamic_pipeline, DynamicPrimitiveSoundness, DynamicSoundnessViolation,
    PrecisionContract, SharedFactHeader, SharedFactKind, Soundness,
};

/// Stable fact identifier.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
pub struct FactId(pub u64);

impl FactId {
    /// Returns true when this id is non-zero.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// Source span attached to one fact or proof step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnalysisSourceSpan {
    /// Stable file id from the analysis corpus.
    pub file_id: u32,
    /// Start byte offset, inclusive.
    pub start_byte: u32,
    /// End byte offset, exclusive.
    pub end_byte: u32,
    /// Start line, one-based when known, zero when unknown.
    pub start_line: u32,
    /// Start column, one-based when known, zero when unknown.
    pub start_column: u32,
    /// End line, one-based when known, zero when unknown.
    pub end_line: u32,
    /// End column, one-based when known, zero when unknown.
    pub end_column: u32,
}

impl AnalysisSourceSpan {
    /// Build a byte-only span.
    #[must_use]
    pub const fn byte_range(file_id: u32, start_byte: u32, end_byte: u32) -> Self {
        Self {
            file_id,
            start_byte,
            end_byte,
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
        }
    }

    /// Validate span ordering.
    ///
    /// # Errors
    /// Returns [`AnalysisFactError`] when the byte range is reversed.
    pub fn validate(&self, context: &str) -> Result<(), AnalysisFactError> {
        if self.end_byte < self.start_byte {
            return Err(AnalysisFactError::InvalidSpan {
                context: context.to_string(),
                start_byte: self.start_byte,
                end_byte: self.end_byte,
            });
        }
        Ok(())
    }
}

/// Canonical fact kinds used by the analysis layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum FactKind {
    /// Syntax or semantic node fact.
    Node,
    /// Graph edge fact.
    Edge,
    /// Symbol identity fact.
    Symbol,
    /// Call edge or callsite fact.
    Call,
    /// Dataflow relation fact.
    Dataflow,
    /// Control-flow relation fact.
    Control,
    /// Authorization check or policy fact.
    Auth,
    /// Sanitizer fact.
    Sanitizer,
    /// Security sink fact.
    Sink,
    /// Attacker/source fact.
    Source,
    /// Type fact.
    Type,
    /// Lifetime or ownership fact.
    Lifetime,
    /// Concurrency fact.
    Concurrency,
    /// Provenance or derivation fact.
    Provenance,
}

impl FactKind {
    /// Stable numeric tag for columnar GPU inputs.
    #[must_use]
    pub const fn tag(self) -> u16 {
        match self {
            Self::Node => 1,
            Self::Edge => 2,
            Self::Symbol => 3,
            Self::Call => 4,
            Self::Dataflow => 5,
            Self::Control => 6,
            Self::Auth => 7,
            Self::Sanitizer => 8,
            Self::Sink => 9,
            Self::Source => 10,
            Self::Type => 11,
            Self::Lifetime => 12,
            Self::Concurrency => 13,
            Self::Provenance => 14,
        }
    }
}

/// One normalized analysis fact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFact {
    /// Stable fact id.
    pub id: FactId,
    /// Fact family.
    pub kind: FactKind,
    /// Source span for this fact.
    pub span: AnalysisSourceSpan,
    /// Primary subject node/symbol/object id.
    pub subject: u64,
    /// Optional object node/symbol/object id.
    pub object: Option<u64>,
    /// Deterministic payload map. Keys are sorted by `BTreeMap`.
    pub payload: BTreeMap<String, String>,
    /// Parent facts used to derive this fact.
    pub provenance: Vec<FactId>,
    /// Confidence in basis points, 0..=10000.
    pub confidence_bps: u16,
    /// Required reason when confidence is below 10000 or the fact is inferred.
    pub reason: String,
}

impl AnalysisFact {
    /// Build a fact with exact confidence and no parent facts.
    #[must_use]
    pub fn exact(id: FactId, kind: FactKind, span: AnalysisSourceSpan, subject: u64) -> Self {
        Self {
            id,
            kind,
            span,
            subject,
            object: None,
            payload: BTreeMap::new(),
            provenance: Vec::new(),
            confidence_bps: 10_000,
            reason: "exact-parser-fact".to_string(),
        }
    }

    /// Validate intrinsic fact fields.
    ///
    /// # Errors
    /// Returns [`AnalysisFactError`] when ids, spans, confidence, payload, or
    /// self-provenance are invalid.
    pub fn validate(&self) -> Result<(), AnalysisFactError> {
        if !self.id.is_valid() {
            return Err(AnalysisFactError::InvalidFactId { id: self.id });
        }
        self.span.validate("fact")?;
        if self.confidence_bps > 10_000 {
            return Err(AnalysisFactError::InvalidConfidence {
                id: self.id,
                confidence_bps: self.confidence_bps,
            });
        }
        if self.confidence_bps < 10_000 && self.reason.trim().is_empty() {
            return Err(AnalysisFactError::MissingInferenceReason { id: self.id });
        }
        if self.provenance.iter().any(|parent| *parent == self.id) {
            return Err(AnalysisFactError::SelfProvenance { id: self.id });
        }
        for key in self.payload.keys() {
            if key.trim().is_empty() {
                return Err(AnalysisFactError::InvalidPayloadKey { id: self.id });
            }
        }
        Ok(())
    }

    /// Convert this security fact into the shared dataflow fact header.
    #[must_use]
    pub fn shared_header(&self, producer: &str) -> SharedFactHeader {
        let mut header = SharedFactHeader::new(
            producer,
            shared_kind_for_security_fact(self.kind),
            self.id.0,
            self.subject,
            Soundness::Exact,
        )
        .with_span(self.span.file_id, self.span.start_byte, self.span.end_byte);
        if let Some(object) = self.object {
            header = header.with_object(object);
        }
        header
    }
}

fn shared_kind_for_security_fact(kind: FactKind) -> SharedFactKind {
    match kind {
        FactKind::Source => SharedFactKind::Source,
        FactKind::Sink => SharedFactKind::Sink,
        FactKind::Sanitizer => SharedFactKind::Sanitizer,
        FactKind::Dataflow => SharedFactKind::Taint,
        FactKind::Edge | FactKind::Call | FactKind::Control => SharedFactKind::GraphEdge,
        FactKind::Auth => SharedFactKind::Dominance,
        FactKind::Lifetime => SharedFactKind::BorrowOrigin,
        FactKind::Provenance => SharedFactKind::Witness,
        FactKind::Type => SharedFactKind::Range,
        FactKind::Node | FactKind::Symbol | FactKind::Concurrency => SharedFactKind::Taint,
    }
}

/// Canonical fact table.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFactTable {
    /// Facts in producer order. Conversion to columns sorts by id.
    pub facts: Vec<AnalysisFact>,
}

impl AnalysisFactTable {
    /// Build a fact table from facts.
    #[must_use]
    pub fn new(facts: Vec<AnalysisFact>) -> Self {
        Self { facts }
    }

    /// Validate table-level uniqueness and provenance references.
    ///
    /// # Errors
    /// Returns [`AnalysisFactError`] when any table invariant is broken.
    pub fn validate(&self) -> Result<(), AnalysisFactError> {
        let mut ids = BTreeSet::new();
        for fact in &self.facts {
            fact.validate()?;
            if !ids.insert(fact.id) {
                return Err(AnalysisFactError::DuplicateFactId { id: fact.id });
            }
        }
        for fact in &self.facts {
            for parent in &fact.provenance {
                if !ids.contains(parent) {
                    return Err(AnalysisFactError::MissingProvenanceParent {
                        id: fact.id,
                        parent: *parent,
                    });
                }
            }
        }
        Ok(())
    }

    /// Convert facts into deterministic GPU-ready columns sorted by fact id.
    ///
    /// # Errors
    /// Returns [`AnalysisFactError`] when the fact table is invalid.
    pub fn to_columnar(&self) -> Result<AnalysisFactColumns, AnalysisFactError> {
        self.validate()?;
        let mut facts = self.facts.iter().collect::<Vec<_>>();
        facts.sort_by_key(|fact| fact.id);
        let mut columns = AnalysisFactColumns::default();
        for fact in facts {
            columns.ids.push(fact.id.0);
            columns.kinds.push(fact.kind.tag());
            columns.file_ids.push(fact.span.file_id);
            columns.start_bytes.push(fact.span.start_byte);
            columns.end_bytes.push(fact.span.end_byte);
            columns.subjects.push(fact.subject);
            columns.objects.push(fact.object.unwrap_or(0));
            columns.confidence_bps.push(fact.confidence_bps);
            columns
                .payload_digests
                .push(payload_digest(&fact.payload, &fact.reason));
            columns
                .provenance_offsets
                .push(columns.provenance_ids.len() as u32);
            columns
                .provenance_ids
                .extend(fact.provenance.iter().map(|parent| parent.0));
        }
        columns
            .provenance_offsets
            .push(columns.provenance_ids.len() as u32);
        Ok(columns)
    }

    /// Return true when the table contains `id`.
    #[must_use]
    pub fn contains(&self, id: FactId) -> bool {
        self.facts.iter().any(|fact| fact.id == id)
    }

    /// Return one fact by id.
    #[must_use]
    pub fn get(&self, id: FactId) -> Option<&AnalysisFact> {
        self.facts.iter().find(|fact| fact.id == id)
    }
}

/// Columnar fact representation for GPU uploads.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFactColumns {
    /// Fact ids.
    pub ids: Vec<u64>,
    /// Stable [`FactKind`] tags.
    pub kinds: Vec<u16>,
    /// File ids.
    pub file_ids: Vec<u32>,
    /// Start byte offsets.
    pub start_bytes: Vec<u32>,
    /// End byte offsets.
    pub end_bytes: Vec<u32>,
    /// Subject ids.
    pub subjects: Vec<u64>,
    /// Object ids, zero when absent.
    pub objects: Vec<u64>,
    /// Confidence values in basis points.
    pub confidence_bps: Vec<u16>,
    /// Payload/reason digests.
    pub payload_digests: Vec<[u8; 32]>,
    /// Offsets into `provenance_ids`, one extra sentinel at the end.
    pub provenance_offsets: Vec<u32>,
    /// Flattened parent fact ids.
    pub provenance_ids: Vec<u64>,
}

/// One source-backed step in a finding proof path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FindingProofStep {
    /// Fact id used by this proof step.
    pub fact_id: FactId,
    /// Source span shown for this proof step.
    pub span: AnalysisSourceSpan,
    /// Human-readable role, such as `source`, `edge`, `sanitizer`, or `sink`.
    pub role: String,
}

impl FindingProofStep {
    /// Build one proof step.
    #[must_use]
    pub fn new(fact_id: FactId, span: AnalysisSourceSpan, role: impl Into<String>) -> Self {
        Self {
            fact_id,
            span,
            role: role.into(),
        }
    }

    fn validate(&self, table: &AnalysisFactTable) -> Result<(), AnalysisFactError> {
        if !table.contains(self.fact_id) {
            return Err(AnalysisFactError::FindingReferencesMissingFact {
                finding_id: "<proof-step>".to_string(),
                fact_id: self.fact_id,
            });
        }
        if self.role.trim().is_empty() {
            return Err(AnalysisFactError::InvalidProofRole {
                fact_id: self.fact_id,
            });
        }
        self.span.validate("finding proof step")
    }
}

/// Fact-backed security finding proof bundle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FindingProofBundle {
    /// Stable finding id.
    pub finding_id: String,
    /// Query Program or analysis query id that produced the finding.
    pub query_id: String,
    /// Backend that produced or verified the finding.
    pub backend_id: String,
    /// Driver evidence bundle digest, report digest, or replay id.
    pub evidence_digest: String,
    /// Consumer precision contract this finding claims to satisfy.
    pub precision_contract: PrecisionContract,
    /// Conservative composed soundness marker for the primitive evidence.
    pub soundness: Soundness,
    /// Primitive soundness evidence used by the query.
    pub primitive_soundness: Vec<DynamicPrimitiveSoundness>,
    /// Facts used by the finding.
    pub fact_ids: Vec<FactId>,
    /// Ordered proof path.
    pub proof_path: Vec<FindingProofStep>,
    /// Confidence in basis points, 0..=10000.
    pub confidence_bps: u16,
    /// Operator-facing explanation.
    pub reason: String,
}

/// Request for converting a sanitized source-to-sink query result into a finding proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceToSinkFindingRequest {
    /// Stable finding id.
    pub finding_id: String,
    /// Query Program or analysis query id that produced the hit scalar.
    pub query_id: String,
    /// Backend that produced or verified the hit scalar.
    pub backend_id: String,
    /// Driver evidence bundle digest, report digest, or replay id.
    pub evidence_digest: String,
    /// Consumer precision contract this finding must satisfy.
    pub precision_contract: PrecisionContract,
    /// Source fact id.
    pub source_fact_id: FactId,
    /// Sink fact id.
    pub sink_fact_id: FactId,
    /// Dataflow/control/call/edge facts used to prove the path.
    pub path_fact_ids: Vec<FactId>,
    /// Sanitizer facts considered by the query.
    pub sanitizer_fact_ids: Vec<FactId>,
    /// Scalar result returned by the sanitized source-to-sink query.
    pub query_hit: u32,
    /// Confidence in basis points, 0..=10000.
    pub confidence_bps: u16,
    /// Operator-facing reason for the finding.
    pub reason: String,
}

impl FindingProofBundle {
    /// Validate that a finding is backed by the supplied fact table.
    ///
    /// # Errors
    /// Returns [`AnalysisFactError`] when required identity fields are empty,
    /// confidence is invalid, any referenced fact is missing, or the proof path
    /// is empty.
    pub fn validate_against(&self, table: &AnalysisFactTable) -> Result<(), AnalysisFactError> {
        table.validate()?;
        if self.finding_id.trim().is_empty() {
            return Err(AnalysisFactError::InvalidFindingIdentity {
                field: "finding_id",
            });
        }
        if self.query_id.trim().is_empty() {
            return Err(AnalysisFactError::InvalidFindingIdentity { field: "query_id" });
        }
        if self.backend_id.trim().is_empty() {
            return Err(AnalysisFactError::InvalidFindingIdentity {
                field: "backend_id",
            });
        }
        if self.evidence_digest.trim().is_empty() {
            return Err(AnalysisFactError::InvalidFindingIdentity {
                field: "evidence_digest",
            });
        }
        if self.reason.trim().is_empty() {
            return Err(AnalysisFactError::InvalidFindingIdentity { field: "reason" });
        }
        if self.primitive_soundness.is_empty() {
            return Err(AnalysisFactError::FindingHasNoSoundnessEvidence {
                finding_id: self.finding_id.clone(),
            });
        }
        let joined = validate_dynamic_pipeline(self.precision_contract, &self.primitive_soundness)
            .map_err(|violation| AnalysisFactError::FindingSoundnessViolation {
                finding_id: self.finding_id.clone(),
                violation,
            })?;
        if joined != self.soundness {
            return Err(AnalysisFactError::FindingSoundnessMismatch {
                finding_id: self.finding_id.clone(),
                declared: self.soundness,
                computed: joined,
            });
        }
        if self.confidence_bps > 10_000 {
            return Err(AnalysisFactError::InvalidFindingConfidence {
                finding_id: self.finding_id.clone(),
                confidence_bps: self.confidence_bps,
            });
        }
        if self.fact_ids.is_empty() {
            return Err(AnalysisFactError::FindingHasNoFacts {
                finding_id: self.finding_id.clone(),
            });
        }
        if self.proof_path.is_empty() {
            return Err(AnalysisFactError::FindingHasNoProofPath {
                finding_id: self.finding_id.clone(),
            });
        }
        for fact_id in &self.fact_ids {
            if !table.contains(*fact_id) {
                return Err(AnalysisFactError::FindingReferencesMissingFact {
                    finding_id: self.finding_id.clone(),
                    fact_id: *fact_id,
                });
            }
        }
        for step in &self.proof_path {
            step.validate(table).map_err(|error| match error {
                AnalysisFactError::FindingReferencesMissingFact { fact_id, .. } => {
                    AnalysisFactError::FindingReferencesMissingFact {
                        finding_id: self.finding_id.clone(),
                        fact_id,
                    }
                }
                other => other,
            })?;
        }
        Ok(())
    }
}

/// Convert a sanitized source-to-sink query result into a fact-backed finding.
///
/// `query_hit == 0` means the query found no unsanitized source-to-sink path,
/// so this function returns `Ok(None)` after validating the referenced source,
/// sink, path, and sanitizer facts. `query_hit != 0` emits a
/// [`FindingProofBundle`] and validates it against the same fact table.
///
/// # Errors
/// Returns [`AnalysisFactError`] when any referenced fact is missing, has the
/// wrong kind for its role, or the emitted finding bundle is incomplete.
pub fn finding_from_sanitized_source_to_sink_query(
    table: &AnalysisFactTable,
    request: SourceToSinkFindingRequest,
) -> Result<Option<FindingProofBundle>, AnalysisFactError> {
    table.validate()?;
    let source = require_fact_kind(table, request.source_fact_id, "source", &[FactKind::Source])?;
    let sink = require_fact_kind(table, request.sink_fact_id, "sink", &[FactKind::Sink])?;
    for fact_id in &request.path_fact_ids {
        let _ = require_fact_kind(
            table,
            *fact_id,
            "path",
            &[
                FactKind::Dataflow,
                FactKind::Edge,
                FactKind::Call,
                FactKind::Control,
            ],
        )?;
    }
    for fact_id in &request.sanitizer_fact_ids {
        let _ = require_fact_kind(table, *fact_id, "sanitizer", &[FactKind::Sanitizer])?;
    }
    if request.query_hit == 0 {
        return Ok(None);
    }
    let primitive_soundness = vec![DynamicPrimitiveSoundness::new(
        request.query_id.clone(),
        Soundness::MayOver,
    )
    .with_sanitizer_filter()];
    let soundness = validate_dynamic_pipeline(request.precision_contract, &primitive_soundness)
        .map_err(|violation| AnalysisFactError::FindingSoundnessViolation {
            finding_id: request.finding_id.clone(),
            violation,
        })?;

    let mut fact_ids = Vec::new();
    push_unique_fact(&mut fact_ids, source.id);
    for fact_id in &request.path_fact_ids {
        push_unique_fact(&mut fact_ids, *fact_id);
    }
    for fact_id in &request.sanitizer_fact_ids {
        push_unique_fact(&mut fact_ids, *fact_id);
    }
    push_unique_fact(&mut fact_ids, sink.id);

    let mut proof_path = Vec::new();
    proof_path.push(FindingProofStep::new(
        source.id,
        source.span.clone(),
        "source",
    ));
    for fact_id in &request.path_fact_ids {
        if let Some(fact) = table.get(*fact_id) {
            proof_path.push(FindingProofStep::new(
                fact.id,
                fact.span.clone(),
                "dataflow-path",
            ));
        }
    }
    for fact_id in &request.sanitizer_fact_ids {
        if let Some(fact) = table.get(*fact_id) {
            proof_path.push(FindingProofStep::new(
                fact.id,
                fact.span.clone(),
                "sanitizer-considered",
            ));
        }
    }
    proof_path.push(FindingProofStep::new(sink.id, sink.span.clone(), "sink"));

    let bundle = FindingProofBundle {
        finding_id: request.finding_id,
        query_id: request.query_id,
        backend_id: request.backend_id,
        evidence_digest: request.evidence_digest,
        precision_contract: request.precision_contract,
        soundness,
        primitive_soundness,
        fact_ids,
        proof_path,
        confidence_bps: request.confidence_bps,
        reason: request.reason,
    };
    bundle.validate_against(table)?;
    Ok(Some(bundle))
}

fn require_fact_kind<'a>(
    table: &'a AnalysisFactTable,
    fact_id: FactId,
    role: &'static str,
    expected: &'static [FactKind],
) -> Result<&'a AnalysisFact, AnalysisFactError> {
    let fact =
        table
            .get(fact_id)
            .ok_or_else(|| AnalysisFactError::FindingReferencesMissingFact {
                finding_id: format!("<{role}>"),
                fact_id,
            })?;
    if !expected.contains(&fact.kind) {
        return Err(AnalysisFactError::UnexpectedFactKind {
            id: fact_id,
            role,
            expected: expected
                .iter()
                .map(|kind| format!("{kind:?}"))
                .collect::<Vec<_>>()
                .join("|"),
            actual: fact.kind,
        });
    }
    Ok(fact)
}

fn push_unique_fact(facts: &mut Vec<FactId>, fact_id: FactId) {
    if !facts.contains(&fact_id) {
        facts.push(fact_id);
    }
}

/// Analysis fact/finding validation failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AnalysisFactError {
    /// A fact id was zero.
    #[error("invalid fact id {id:?}. Fix: assign non-zero stable fact ids before analysis.")]
    InvalidFactId {
        /// Invalid id.
        id: FactId,
    },
    /// A fact id appeared more than once.
    #[error("duplicate fact id {id:?}. Fix: deduplicate facts before GPU columnar packing.")]
    DuplicateFactId {
        /// Duplicate id.
        id: FactId,
    },
    /// A source span had reversed byte order.
    #[error(
        "{context} span has start_byte {start_byte} after end_byte {end_byte}. Fix: normalize parser spans before analysis."
    )]
    InvalidSpan {
        /// Span context.
        context: String,
        /// Start byte.
        start_byte: u32,
        /// End byte.
        end_byte: u32,
    },
    /// Confidence exceeded 10000 basis points.
    #[error(
        "fact {id:?} confidence {confidence_bps} exceeds 10000. Fix: store confidence in basis points."
    )]
    InvalidConfidence {
        /// Fact id.
        id: FactId,
        /// Invalid confidence.
        confidence_bps: u16,
    },
    /// Inferred fact had no explanation.
    #[error("fact {id:?} is inferred but has no reason. Fix: record why the fact is trusted.")]
    MissingInferenceReason {
        /// Fact id.
        id: FactId,
    },
    /// Fact listed itself as provenance.
    #[error("fact {id:?} lists itself as provenance. Fix: remove cyclic fact derivation.")]
    SelfProvenance {
        /// Fact id.
        id: FactId,
    },
    /// Payload key was blank.
    #[error("fact {id:?} has a blank payload key. Fix: normalize payload keys before packing.")]
    InvalidPayloadKey {
        /// Fact id.
        id: FactId,
    },
    /// A provenance parent id was missing.
    #[error(
        "fact {id:?} references missing provenance parent {parent:?}. Fix: emit parent facts before derived facts."
    )]
    MissingProvenanceParent {
        /// Derived fact id.
        id: FactId,
        /// Missing parent id.
        parent: FactId,
    },
    /// A finding identity field was blank.
    #[error("finding field `{field}` is blank. Fix: findings must be fact-backed and replayable.")]
    InvalidFindingIdentity {
        /// Invalid field name.
        field: &'static str,
    },
    /// Finding confidence exceeded 10000 basis points.
    #[error(
        "finding `{finding_id}` confidence {confidence_bps} exceeds 10000. Fix: store confidence in basis points."
    )]
    InvalidFindingConfidence {
        /// Finding id.
        finding_id: String,
        /// Invalid confidence.
        confidence_bps: u16,
    },
    /// Finding referenced no facts.
    #[error("finding `{finding_id}` references no facts. Fix: do not emit LLM-only findings.")]
    FindingHasNoFacts {
        /// Finding id.
        finding_id: String,
    },
    /// Finding had no proof path.
    #[error(
        "finding `{finding_id}` has no proof path. Fix: include source-to-sink/auth path steps."
    )]
    FindingHasNoProofPath {
        /// Finding id.
        finding_id: String,
    },
    /// Finding had no primitive soundness evidence.
    #[error(
        "finding `{finding_id}` has no primitive soundness evidence. Fix: attach the query primitive ids and soundness tags before reporting."
    )]
    FindingHasNoSoundnessEvidence {
        /// Finding id.
        finding_id: String,
    },
    /// Finding primitive evidence violated its precision contract.
    #[error(
        "finding `{finding_id}` soundness evidence violates its precision contract: {violation:?}."
    )]
    FindingSoundnessViolation {
        /// Finding id.
        finding_id: String,
        /// Canonical dynamic soundness violation.
        violation: DynamicSoundnessViolation,
    },
    /// Finding declared a composed soundness marker inconsistent with evidence.
    #[error(
        "finding `{finding_id}` declares soundness {declared:?} but primitive evidence computes {computed:?}. Fix: recompute soundness from primitive evidence."
    )]
    FindingSoundnessMismatch {
        /// Finding id.
        finding_id: String,
        /// Declared soundness.
        declared: Soundness,
        /// Computed soundness.
        computed: Soundness,
    },
    /// Finding referenced an absent fact.
    #[error(
        "finding `{finding_id}` references missing fact {fact_id:?}. Fix: include all proof facts in the fact table."
    )]
    FindingReferencesMissingFact {
        /// Finding id.
        finding_id: String,
        /// Missing fact id.
        fact_id: FactId,
    },
    /// Proof role was blank.
    #[error("proof step for fact {fact_id:?} has a blank role. Fix: name each proof step role.")]
    InvalidProofRole {
        /// Fact id.
        fact_id: FactId,
    },
    /// A fact was present but had the wrong kind for its query role.
    #[error(
        "fact {id:?} has kind {actual:?} for role `{role}`, expected {expected}. Fix: normalize analysis facts before query proof emission."
    )]
    UnexpectedFactKind {
        /// Fact id.
        id: FactId,
        /// Query role.
        role: &'static str,
        /// Expected kind list.
        expected: String,
        /// Actual fact kind.
        actual: FactKind,
    },
}

fn payload_digest(payload: &BTreeMap<String, String>, reason: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, b"format", b"vyre-analysis-payload-v1");
    for (key, value) in payload {
        hash_field(&mut hasher, b"key", key.as_bytes());
        hash_field(&mut hasher, b"value", value.as_bytes());
    }
    hash_field(&mut hasher, b"reason", reason.as_bytes());
    *hasher.finalize().as_bytes()
}

fn hash_field(hasher: &mut blake3::Hasher, label: &[u8], value: &[u8]) {
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(offset: u32) -> AnalysisSourceSpan {
        AnalysisSourceSpan::byte_range(7, offset, offset + 4)
    }

    fn fact(id: u64, kind: FactKind, subject: u64) -> AnalysisFact {
        AnalysisFact::exact(FactId(id), kind, span(id as u32), subject)
    }

    fn table() -> AnalysisFactTable {
        let mut source = fact(1, FactKind::Source, 10);
        source
            .payload
            .insert("name".to_string(), "req.user".to_string());
        let mut edge = fact(2, FactKind::Dataflow, 10);
        edge.object = Some(20);
        edge.provenance.push(FactId(1));
        let mut sink = fact(3, FactKind::Sink, 20);
        sink.payload
            .insert("kind".to_string(), "sql.query".to_string());
        AnalysisFactTable::new(vec![sink, edge, source])
    }

    #[test]
    fn fact_table_to_columnar_sorts_by_fact_id_and_preserves_provenance_offsets() {
        let columns = table()
            .to_columnar()
            .expect("Fix: canonical fact table should validate and pack");

        assert_eq!(columns.ids, vec![1, 2, 3]);
        assert_eq!(
            columns.kinds,
            vec![
                FactKind::Source.tag(),
                FactKind::Dataflow.tag(),
                FactKind::Sink.tag()
            ]
        );
        assert_eq!(columns.file_ids, vec![7, 7, 7]);
        assert_eq!(columns.subjects, vec![10, 10, 20]);
        assert_eq!(columns.objects, vec![0, 20, 0]);
        assert_eq!(columns.provenance_offsets, vec![0, 0, 1, 1]);
        assert_eq!(columns.provenance_ids, vec![1]);
    }

    #[test]
    fn fact_table_rejects_duplicate_ids() {
        let error = AnalysisFactTable::new(vec![
            fact(1, FactKind::Source, 1),
            fact(1, FactKind::Sink, 2),
        ])
        .validate()
        .expect_err("Fix: duplicate fact ids must be rejected");

        assert_eq!(error, AnalysisFactError::DuplicateFactId { id: FactId(1) });
    }

    #[test]
    fn fact_table_rejects_missing_provenance_parent() {
        let mut derived = fact(2, FactKind::Dataflow, 10);
        derived.provenance.push(FactId(99));

        let error = AnalysisFactTable::new(vec![fact(1, FactKind::Source, 10), derived])
            .validate()
            .expect_err("Fix: missing provenance parents must be rejected");

        assert_eq!(
            error,
            AnalysisFactError::MissingProvenanceParent {
                id: FactId(2),
                parent: FactId(99),
            }
        );
    }

    #[test]
    fn fact_table_rejects_inferred_fact_without_reason() {
        let mut inferred = fact(4, FactKind::Auth, 40);
        inferred.confidence_bps = 7500;
        inferred.reason.clear();

        let error = AnalysisFactTable::new(vec![inferred])
            .validate()
            .expect_err("Fix: inferred facts need a reason");

        assert_eq!(
            error,
            AnalysisFactError::MissingInferenceReason { id: FactId(4) }
        );
    }

    #[test]
    fn finding_proof_bundle_validates_fact_backing_and_proof_path() {
        let fact_table = table();
        let bundle = FindingProofBundle {
            finding_id: "finding.sql.source-to-sink.1".to_string(),
            query_id: "vyre-libs::security::flows_to_with_sanitizer".to_string(),
            backend_id: "cpu-ref".to_string(),
            evidence_digest: "evidence:abc123".to_string(),
            precision_contract: PrecisionContract::ZeroFalsePositive,
            soundness: Soundness::Exact,
            primitive_soundness: vec![DynamicPrimitiveSoundness::new(
                "vyre-libs::security::sanitizer_dominates",
                Soundness::Exact,
            )],
            fact_ids: vec![FactId(1), FactId(2), FactId(3)],
            proof_path: vec![
                FindingProofStep::new(FactId(1), span(1), "source"),
                FindingProofStep::new(FactId(2), span(2), "dataflow-edge"),
                FindingProofStep::new(FactId(3), span(3), "sink"),
            ],
            confidence_bps: 9800,
            reason: "source reaches sql sink without sanitizer dominance".to_string(),
        };

        bundle
            .validate_against(&fact_table)
            .expect("Fix: fact-backed proof bundle should validate");
    }

    #[test]
    fn finding_proof_bundle_rejects_llm_only_finding_without_facts() {
        let fact_table = table();
        let bundle = FindingProofBundle {
            finding_id: "finding.llm-only".to_string(),
            query_id: "manual".to_string(),
            backend_id: "cpu-ref".to_string(),
            evidence_digest: "evidence:abc123".to_string(),
            precision_contract: PrecisionContract::ZeroFalsePositive,
            soundness: Soundness::Exact,
            primitive_soundness: vec![DynamicPrimitiveSoundness::new(
                "manual",
                Soundness::Exact,
            )],
            fact_ids: Vec::new(),
            proof_path: vec![FindingProofStep::new(FactId(1), span(1), "source")],
            confidence_bps: 5000,
            reason: "model guessed from code text".to_string(),
        };

        let error = bundle
            .validate_against(&fact_table)
            .expect_err("Fix: factless findings must be rejected");

        assert_eq!(
            error,
            AnalysisFactError::FindingHasNoFacts {
                finding_id: "finding.llm-only".to_string(),
            }
        );
    }

    #[test]
    fn finding_proof_bundle_rejects_missing_fact_reference() {
        let fact_table = table();
        let bundle = FindingProofBundle {
            finding_id: "finding.missing-fact".to_string(),
            query_id: "vyre-libs::security::flows_to".to_string(),
            backend_id: "cpu-ref".to_string(),
            evidence_digest: "evidence:abc123".to_string(),
            precision_contract: PrecisionContract::ZeroFalsePositive,
            soundness: Soundness::Exact,
            primitive_soundness: vec![DynamicPrimitiveSoundness::new(
                "vyre-libs::security::sanitizer_dominates",
                Soundness::Exact,
            )],
            fact_ids: vec![FactId(1), FactId(42)],
            proof_path: vec![FindingProofStep::new(FactId(1), span(1), "source")],
            confidence_bps: 9000,
            reason: "source reaches sink".to_string(),
        };

        let error = bundle
            .validate_against(&fact_table)
            .expect_err("Fix: findings must not reference absent facts");

        assert_eq!(
            error,
            AnalysisFactError::FindingReferencesMissingFact {
                finding_id: "finding.missing-fact".to_string(),
                fact_id: FactId(42),
            }
        );
    }

    #[test]
    fn finding_proof_bundle_rejects_zero_false_positive_unfiltered_mayover() {
        let fact_table = table();
        let bundle = FindingProofBundle {
            finding_id: "finding.unfiltered-mayover".to_string(),
            query_id: "vyre-libs::security::flows_to".to_string(),
            backend_id: "cpu-ref".to_string(),
            evidence_digest: "evidence:abc123".to_string(),
            precision_contract: PrecisionContract::ZeroFalsePositive,
            soundness: Soundness::MayOver,
            primitive_soundness: vec![DynamicPrimitiveSoundness::new(
                "vyre-libs::security::flows_to",
                Soundness::MayOver,
            )],
            fact_ids: vec![FactId(1), FactId(2), FactId(3)],
            proof_path: vec![
                FindingProofStep::new(FactId(1), span(1), "source"),
                FindingProofStep::new(FactId(2), span(2), "dataflow-edge"),
                FindingProofStep::new(FactId(3), span(3), "sink"),
            ],
            confidence_bps: 9000,
            reason: "unfiltered over-approximate flow should not ship as zero-FP".to_string(),
        };

        let error = bundle
            .validate_against(&fact_table)
            .expect_err("Fix: unfiltered MayOver must not validate as zero false positive");

        match error {
            AnalysisFactError::FindingSoundnessViolation {
                finding_id,
                violation,
            } => {
                assert_eq!(finding_id, "finding.unfiltered-mayover");
                assert_eq!(violation.op_id, "vyre-libs::security::flows_to");
                assert_eq!(violation.soundness, Soundness::MayOver);
                assert_eq!(violation.contract, PrecisionContract::ZeroFalsePositive);
            }
            other => panic!("unexpected soundness validation error: {other:?}"),
        }
    }

    #[test]
    fn finding_proof_bundle_rejects_declared_soundness_mismatch() {
        let fact_table = table();
        let bundle = FindingProofBundle {
            finding_id: "finding.soundness-mismatch".to_string(),
            query_id: "vyre-libs::security::flows_to_with_sanitizer".to_string(),
            backend_id: "cpu-ref".to_string(),
            evidence_digest: "evidence:abc123".to_string(),
            precision_contract: PrecisionContract::ZeroFalsePositive,
            soundness: Soundness::Exact,
            primitive_soundness: vec![DynamicPrimitiveSoundness::new(
                "vyre-libs::security::flows_to_with_sanitizer",
                Soundness::MayOver,
            )
            .with_sanitizer_filter()],
            fact_ids: vec![FactId(1), FactId(2), FactId(3)],
            proof_path: vec![
                FindingProofStep::new(FactId(1), span(1), "source"),
                FindingProofStep::new(FactId(2), span(2), "dataflow-edge"),
                FindingProofStep::new(FactId(3), span(3), "sink"),
            ],
            confidence_bps: 9000,
            reason: "declared exact despite MayOver primitive evidence".to_string(),
        };

        let error = bundle
            .validate_against(&fact_table)
            .expect_err("Fix: declared soundness must match primitive evidence join");

        assert_eq!(
            error,
            AnalysisFactError::FindingSoundnessMismatch {
                finding_id: "finding.soundness-mismatch".to_string(),
                declared: Soundness::Exact,
                computed: Soundness::MayOver,
            }
        );
    }
}

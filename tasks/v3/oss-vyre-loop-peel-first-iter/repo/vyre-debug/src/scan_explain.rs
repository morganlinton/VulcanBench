//! Scan decomposition explanation reports.

use serde::Serialize;
use vyre_foundation::serial::wire::encode::{
    ScanDatabaseHeader, ScanDatabaseSectionKind, UnsupportedScanFeature,
};
use vyre_lower::{
    DescriptorIntentKind, DescriptorIntentSet, DescriptorIntentStrategy, IntentAnnotatedDescriptor,
};

pub const SCAN_EXPLAIN_REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ScanFactorRole {
    Literal,
    Prefix,
    Suffix,
    Infix,
    Outfix,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ScanExplainFactor {
    pub role: ScanFactorRole,
    pub pattern_index: u32,
    pub bytes: Vec<u8>,
    pub digest: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ScanExplainEngine {
    pub strategy: DescriptorIntentStrategy,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ScanExplainExactnessClass {
    Exact,
    PrefilterVerified,
    VerifierRequired,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ScanExplainRejectedEngine {
    pub engine_id: String,
    pub pattern_index: u32,
    pub reason: String,
    pub exactness_class: ScanExplainExactnessClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ScanExplainVerifierFragment {
    pub source: String,
    pub bytes: u64,
    pub digest: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ScanExplainRouteEvidence {
    pub exactness_class: ScanExplainExactnessClass,
    pub verifier_cost_estimate_bytes: u64,
    pub literal_selectivity_basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct ScanExplainReport {
    pub schema_version: u32,
    pub pattern_set_id: String,
    pub extracted_factors: Vec<ScanExplainFactor>,
    pub selected_engines: Vec<ScanExplainEngine>,
    pub rejected_engines: Vec<ScanExplainRejectedEngine>,
    pub verifier_fragments: Vec<ScanExplainVerifierFragment>,
    pub route_evidence: ScanExplainRouteEvidence,
    pub streaming_state_bytes: u64,
    pub table_bytes: u64,
    pub baseline_id: String,
    pub unsupported_features: Vec<UnsupportedScanFeature>,
    pub artifact_links: Vec<String>,
}

impl ScanExplainReport {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.schema_version == SCAN_EXPLAIN_REPORT_SCHEMA_VERSION
            && !self.pattern_set_id.trim().is_empty()
            && !self.extracted_factors.is_empty()
            && !self.selected_engines.is_empty()
            && !self.route_evidence.literal_selectivity_basis.trim().is_empty()
            && !self.baseline_id.trim().is_empty()
            && !self.artifact_links.is_empty()
            && self.table_bytes > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanExplainError {
    MissingPatternSetId,
    MissingFactors,
    MissingBaselineId,
    MissingArtifactLinks,
    MissingDescriptorIntents,
    MissingScanDatabase,
}

impl std::fmt::Display for ScanExplainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPatternSetId => {
                f.write_str("scan explain report is missing pattern_set_id")
            }
            Self::MissingFactors => f.write_str("scan explain report has no extracted factors"),
            Self::MissingBaselineId => f.write_str("scan explain report is missing baseline_id"),
            Self::MissingArtifactLinks => {
                f.write_str("scan explain report is missing artifact links")
            }
            Self::MissingDescriptorIntents => {
                f.write_str("scan explain report is missing descriptor intents")
            }
            Self::MissingScanDatabase => {
                f.write_str("scan explain report is missing scan database header")
            }
        }
    }
}

impl std::error::Error for ScanExplainError {}

pub fn scan_explain_report(
    pattern_set_id: impl Into<String>,
    extracted_factors: Vec<ScanExplainFactor>,
    descriptor: &IntentAnnotatedDescriptor,
    scan_database: &ScanDatabaseHeader,
    baseline_id: impl Into<String>,
    artifact_links: Vec<String>,
) -> Result<ScanExplainReport, ScanExplainError> {
    let pattern_set_id = pattern_set_id.into();
    if pattern_set_id.trim().is_empty() {
        return Err(ScanExplainError::MissingPatternSetId);
    }
    if extracted_factors.is_empty() {
        return Err(ScanExplainError::MissingFactors);
    }
    let baseline_id = baseline_id.into();
    if baseline_id.trim().is_empty() {
        return Err(ScanExplainError::MissingBaselineId);
    }
    if artifact_links.is_empty() {
        return Err(ScanExplainError::MissingArtifactLinks);
    }

    let selected_engines = selected_engines_from_intents(&descriptor.intents)?;
    let verifier_fragments =
        verifier_fragments_from_sources(&descriptor.intents, scan_database)?;
    let rejected_engines = rejected_engines_from_unsupported_features(scan_database);
    let streaming_state_bytes =
        streaming_state_bytes_from_sources(&descriptor.intents, scan_database);
    let table_bytes = scan_database
        .table_sections
        .iter()
        .map(|section| section.byte_len)
        .sum();
    let route_evidence = route_evidence_from_sources(
        &extracted_factors,
        &selected_engines,
        &rejected_engines,
        &verifier_fragments,
        scan_database,
    );

    Ok(ScanExplainReport {
        schema_version: SCAN_EXPLAIN_REPORT_SCHEMA_VERSION,
        pattern_set_id,
        extracted_factors,
        selected_engines,
        rejected_engines,
        verifier_fragments,
        route_evidence,
        streaming_state_bytes,
        table_bytes,
        baseline_id,
        unsupported_features: scan_database.unsupported_features.clone(),
        artifact_links,
    })
}

fn selected_engines_from_intents(
    intents: &DescriptorIntentSet,
) -> Result<Vec<ScanExplainEngine>, ScanExplainError> {
    if intents.intents.is_empty() {
        return Err(ScanExplainError::MissingDescriptorIntents);
    }
    let mut engines = Vec::new();
    for intent in &intents.intents {
        let strategy = intent.kind.strategy();
        if engines
            .iter()
            .any(|engine: &ScanExplainEngine| engine.strategy == strategy)
        {
            continue;
        }
        engines.push(ScanExplainEngine {
            strategy,
            reason: intent_reason(intent.kind),
        });
    }
    Ok(engines)
}

fn rejected_engines_from_unsupported_features(
    scan_database: &ScanDatabaseHeader,
) -> Vec<ScanExplainRejectedEngine> {
    let mut rejected = Vec::new();
    for feature in &scan_database.unsupported_features {
        for engine_id in rejected_candidate_engines(&feature.feature) {
            rejected.push(ScanExplainRejectedEngine {
                engine_id: engine_id.to_string(),
                pattern_index: feature.pattern_index,
                reason: sanitized_rejection_reason(&feature.feature),
                exactness_class: ScanExplainExactnessClass::VerifierRequired,
            });
        }
    }
    rejected
}

fn rejected_candidate_engines(feature: &str) -> &'static [&'static str] {
    let lowered = feature.to_ascii_lowercase();
    if lowered.contains("look") || lowered.contains("capture") || lowered.contains("verifier") {
        &["cuda", "wgpu", "metal", "dpu", "fpga"]
    } else {
        &["cuda", "wgpu", "metal"]
    }
}

fn sanitized_rejection_reason(feature: &str) -> String {
    let mut sanitized = feature.replace("/credentials/", "/credentials/<redacted>/");
    sanitized = sanitized.replace("C:\\credentials\\", "C:\\credentials\\<redacted>\\");
    for marker in ["token=", "secret=", "password=", "api_key="] {
        if let Some(index) = sanitized.to_ascii_lowercase().find(marker) {
            let keep = index + marker.len();
            sanitized.truncate(keep);
            sanitized.push_str("<redacted>");
            break;
        }
    }
    if sanitized.trim().is_empty() {
        "candidate route rejected by scan database feature policy".to_string()
    } else {
        sanitized
    }
}

fn route_evidence_from_sources(
    extracted_factors: &[ScanExplainFactor],
    selected_engines: &[ScanExplainEngine],
    rejected_engines: &[ScanExplainRejectedEngine],
    verifier_fragments: &[ScanExplainVerifierFragment],
    scan_database: &ScanDatabaseHeader,
) -> ScanExplainRouteEvidence {
    let verifier_cost_estimate_bytes = verifier_fragments
        .iter()
        .map(|fragment| fragment.bytes)
        .sum::<u64>()
        + scan_database.unsupported_features.len() as u64 * 64;
    let has_prefilter = selected_engines
        .iter()
        .any(|engine| engine.strategy == DescriptorIntentStrategy::Prefilter);
    let exactness_class = if rejected_engines
        .iter()
        .any(|engine| engine.exactness_class == ScanExplainExactnessClass::Unsupported)
    {
        ScanExplainExactnessClass::Unsupported
    } else if !verifier_fragments.is_empty() && !rejected_engines.is_empty() {
        ScanExplainExactnessClass::VerifierRequired
    } else if has_prefilter && !verifier_fragments.is_empty() {
        ScanExplainExactnessClass::PrefilterVerified
    } else {
        ScanExplainExactnessClass::Exact
    };

    ScanExplainRouteEvidence {
        exactness_class,
        verifier_cost_estimate_bytes,
        literal_selectivity_basis: literal_selectivity_basis(extracted_factors),
    }
}

fn literal_selectivity_basis(extracted_factors: &[ScanExplainFactor]) -> String {
    let literal_factors = extracted_factors
        .iter()
        .filter(|factor| {
            matches!(
                factor.role,
                ScanFactorRole::Literal
                    | ScanFactorRole::Prefix
                    | ScanFactorRole::Suffix
                    | ScanFactorRole::Infix
            )
        })
        .collect::<Vec<_>>();
    let total_bytes = literal_factors
        .iter()
        .map(|factor| factor.bytes.len())
        .sum::<usize>();
    let shortest = literal_factors
        .iter()
        .map(|factor| factor.bytes.len())
        .min()
        .unwrap_or(0);
    format!(
        "literal_factors={};total_literal_bytes={total_bytes};shortest_literal_bytes={shortest}",
        literal_factors.len()
    )
}

fn verifier_fragments_from_sources(
    intents: &DescriptorIntentSet,
    scan_database: &ScanDatabaseHeader,
) -> Result<Vec<ScanExplainVerifierFragment>, ScanExplainError> {
    if scan_database.table_sections.is_empty() {
        return Err(ScanExplainError::MissingScanDatabase);
    }
    let mut fragments = Vec::new();
    for intent in &intents.intents {
        if intent.kind == DescriptorIntentKind::Verifier {
            fragments.push(ScanExplainVerifierFragment {
                source: "descriptor-intent".to_string(),
                bytes: 0,
                digest: intent.section_digest,
            });
        }
    }
    for section in &scan_database.table_sections {
        if section.kind == ScanDatabaseSectionKind::VerifierFragments {
            fragments.push(ScanExplainVerifierFragment {
                source: "scan-database-section".to_string(),
                bytes: section.byte_len,
                digest: section.section_digest,
            });
        }
    }
    Ok(fragments)
}

fn streaming_state_bytes_from_sources(
    intents: &DescriptorIntentSet,
    scan_database: &ScanDatabaseHeader,
) -> u64 {
    let intent_bytes = intents
        .intents
        .iter()
        .filter(|intent| intent.kind == DescriptorIntentKind::StreamingState)
        .map(|intent| u64::from(intent.stream_state_bytes))
        .sum::<u64>();
    let section_bytes = scan_database
        .table_sections
        .iter()
        .filter(|section| section.kind == ScanDatabaseSectionKind::StreamingState)
        .map(|section| section.byte_len)
        .sum::<u64>();
    intent_bytes.max(section_bytes)
}

fn intent_reason(kind: DescriptorIntentKind) -> String {
    match kind {
        DescriptorIntentKind::LiteralPrefilter => {
            "literal factors route to a prefilter before automata or verifier work".to_string()
        }
        DescriptorIntentKind::AutomataTransition => {
            "automata transition tables route to the scan transition engine".to_string()
        }
        DescriptorIntentKind::Verifier => {
            "verifier fragments confirm candidates after prefilter or automata matches".to_string()
        }
        DescriptorIntentKind::OutputCompaction => {
            "output compaction owns sparse match materialization".to_string()
        }
        DescriptorIntentKind::RelationSeed => {
            "relation seeds hand scan output into graph or flow facts".to_string()
        }
        DescriptorIntentKind::StreamingState => {
            "streaming state carries cross-chunk scan state".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::DataType;
    use vyre_foundation::serial::wire::encode::{
        ScanDatabaseCompatibilityRecord, ScanDatabaseMode,
        ScanDatabaseReaderCompatibility, ScanDatabaseSectionHeader,
    };
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, DescriptorIntent,
        DescriptorIntentKind, DescriptorIntentSet, Dispatch, KernelBody,
        KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };

    fn descriptor(intents: DescriptorIntentSet) -> IntentAnnotatedDescriptor {
        let descriptor = KernelDescriptor {
            id: "scan-explain-test".to_string(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "scan_table".to_string(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(1)],
            },
        };
        IntentAnnotatedDescriptor::try_new(descriptor, intents).unwrap()
    }

    fn literal_intents() -> DescriptorIntentSet {
        DescriptorIntentSet::new(vec![
            DescriptorIntent::new(DescriptorIntentKind::LiteralPrefilter, 10)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::Verifier, 11)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::OutputCompaction, 12)
                .with_binding_slot(0)
                .with_op_result(0),
        ])
    }

    fn regex_intents() -> DescriptorIntentSet {
        DescriptorIntentSet::new(vec![
            DescriptorIntent::new(DescriptorIntentKind::LiteralPrefilter, 20)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::AutomataTransition, 21)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::Verifier, 22)
                .with_binding_slot(0)
                .with_op_result(0),
            DescriptorIntent::new(DescriptorIntentKind::StreamingState, 23)
                .with_binding_slot(0)
                .with_stream_state_bytes(128),
        ])
    }

    fn database() -> ScanDatabaseHeader {
        ScanDatabaseHeader {
            pattern_set_digest: [4u8; 32],
            compiler_version: "vyre-debug-scan-explain-test".to_string(),
            mode: ScanDatabaseMode::Streaming,
            table_sections: vec![
                ScanDatabaseSectionHeader {
                    kind: ScanDatabaseSectionKind::LiteralTable,
                    offset: 0,
                    byte_len: 64,
                    section_digest: 100,
                },
                ScanDatabaseSectionHeader {
                    kind: ScanDatabaseSectionKind::VerifierFragments,
                    offset: 64,
                    byte_len: 32,
                    section_digest: 101,
                },
                ScanDatabaseSectionHeader {
                    kind: ScanDatabaseSectionKind::StreamingState,
                    offset: 96,
                    byte_len: 96,
                    section_digest: 102,
                },
            ],
            unsupported_features: vec![UnsupportedScanFeature {
                pattern_index: 9,
                feature: "Fix: look-around stays verifier-only".to_string(),
            }],
            compatibility: ScanDatabaseCompatibilityRecord {
                construct_tier_digest: 0x51ca_51ca,
                dialect_digest: 0xd1a1_ec7,
                reader_compatibility: ScanDatabaseReaderCompatibility::RequiresVerifier,
            },
        }
    }

    #[test]
    fn scan_explain_report_covers_literal_case() {
        let report = scan_explain_report(
            "literal-pattern-set",
            vec![ScanExplainFactor {
                role: ScanFactorRole::Literal,
                pattern_index: 0,
                bytes: b"needle".to_vec(),
                digest: 44,
            }],
            &descriptor(literal_intents()),
            &database(),
            "hyperscan-nsdi-literal",
            vec!["release/evidence/benchmarks/frontier-leaderboard.json".to_string()],
        )
        .unwrap();

        assert!(report.is_complete());
        assert!(report
            .selected_engines
            .iter()
            .any(|engine| engine.strategy == DescriptorIntentStrategy::Prefilter));
        assert_eq!(report.verifier_fragments.len(), 2);
        assert_eq!(report.table_bytes, 192);
        assert_eq!(
            report.route_evidence.literal_selectivity_basis,
            "literal_factors=1;total_literal_bytes=6;shortest_literal_bytes=6"
        );
        assert!(report
            .rejected_engines
            .iter()
            .any(|engine| engine.engine_id == "cuda"));
    }

    #[test]
    fn scan_explain_report_covers_regex_streaming_case() {
        let report = scan_explain_report(
            "regex-pattern-set",
            vec![
                ScanExplainFactor {
                    role: ScanFactorRole::Prefix,
                    pattern_index: 1,
                    bytes: b"abc".to_vec(),
                    digest: 45,
                },
                ScanExplainFactor {
                    role: ScanFactorRole::Suffix,
                    pattern_index: 1,
                    bytes: b"xyz".to_vec(),
                    digest: 46,
                },
            ],
            &descriptor(regex_intents()),
            &database(),
            "hyperscan-nsdi-regex",
            vec!["release/evidence/benchmarks/frontier-leaderboard.json".to_string()],
        )
        .unwrap();

        assert!(report
            .selected_engines
            .iter()
            .any(|engine| engine.strategy == DescriptorIntentStrategy::Automata));
        assert_eq!(report.streaming_state_bytes, 128);
        assert_eq!(report.unsupported_features.len(), 1);
        assert_eq!(
            report.route_evidence.exactness_class,
            ScanExplainExactnessClass::VerifierRequired
        );
        assert!(report.route_evidence.verifier_cost_estimate_bytes >= 96);
        assert!(report.rejected_engines.iter().any(|engine| {
            engine.engine_id == "dpu"
                && engine.reason.contains("verifier-only")
                && !engine.reason.contains("credentials")
        }));
    }

    #[test]
    fn scan_explain_report_rejects_missing_artifact_links() {
        let error = scan_explain_report(
            "regex-pattern-set",
            vec![ScanExplainFactor {
                role: ScanFactorRole::Infix,
                pattern_index: 2,
                bytes: b"mid".to_vec(),
                digest: 47,
            }],
            &descriptor(regex_intents()),
            &database(),
            "hyperscan-nsdi-regex",
            Vec::new(),
        )
        .unwrap_err();

        assert_eq!(error, ScanExplainError::MissingArtifactLinks);
    }
}

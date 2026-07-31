//! Generated-relation analyzer contracts for security fact queries.
//!
//! This module keeps relation-query evidence at the security boundary while
//! reusing the canonical [`super::facts`] fact and finding proof schema. It is
//! intentionally host-side metadata/oracle code: GPU execution still belongs to
//! the existing graph/dataflow primitives and Weir flow boundary.

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::dataflow::PrecisionContract;

use super::facts::{
    finding_from_sanitized_source_to_sink_query, AnalysisFact, AnalysisFactError,
    AnalysisFactTable, FactId, FactKind, FindingProofBundle, SourceToSinkFindingRequest,
};

/// Schema version for generated security relation analyzer evidence.
pub const SECURITY_RELATION_ANALYZER_SCHEMA_VERSION: u32 = 1;

/// One selected generated relation-query family.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SecurityRelationQueryFamily {
    /// Unsanitized source-to-sink reachability.
    UnsanitizedSourceToSink,
}

impl SecurityRelationQueryFamily {
    /// Stable query-family id for evidence rows.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsanitizedSourceToSink => "unsanitized-source-to-sink",
        }
    }
}

/// Static analyzer identity and finding metadata supplied by the caller.
#[derive(Debug, Clone, Copy)]
pub struct GeneratedSecurityRelationAnalyzerSpec<'a> {
    /// Generated analyzer id.
    pub analyzer_id: &'a str,
    /// Relation query family.
    pub family: SecurityRelationQueryFamily,
    /// User-facing rule id used to construct finding ids.
    pub rule_id: &'a str,
    /// Backend or oracle id that produced the relation result.
    pub backend_id: &'a str,
    /// Evidence digest or replay id for the generated analyzer run.
    pub evidence_digest: &'a str,
    /// Consumer precision contract for finding proof bundles.
    pub precision_contract: PrecisionContract,
    /// Baseline id, such as `weir-ifds` or `vyre-security-current`.
    pub baseline_id: &'a str,
}

/// Benchmark/runtime accounting supplied by the caller.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct GeneratedSecurityRelationAnalyzerRunStats {
    /// Generated analyzer compile time.
    pub compile_time_ns: u64,
    /// Generated analyzer run time.
    pub run_time_ns: u64,
    /// Resident or host memory bytes consumed by relation execution.
    pub memory_bytes: u64,
    /// Weir comparator tuple count.
    pub weir_tuple_count: u32,
    /// Current Vyre comparator tuple count.
    pub vyre_tuple_count: u32,
    /// Weir comparator finding count.
    pub weir_finding_count: u32,
    /// Current Vyre comparator finding count.
    pub vyre_finding_count: u32,
}

/// Evidence row emitted by a generated relation analyzer.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GeneratedSecurityRelationAnalyzerEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Generated analyzer id.
    pub analyzer_id: String,
    /// Relation query family id.
    pub query_family: &'static str,
    /// Baseline id used for differential comparison.
    pub baseline_id: String,
    /// Input fact rows.
    pub input_fact_count: u32,
    /// Source relation tuples.
    pub source_tuple_count: u32,
    /// Sink relation tuples.
    pub sink_tuple_count: u32,
    /// Dataflow/control/call/edge relation tuples.
    pub path_tuple_count: u32,
    /// Sanitizer relation tuples.
    pub sanitizer_tuple_count: u32,
    /// Total relation tuples consumed by the generated analyzer.
    pub generated_tuple_count: u32,
    /// Findings emitted by the generated analyzer.
    pub generated_finding_count: u32,
    /// Weir comparator tuple count.
    pub weir_tuple_count: u32,
    /// Current Vyre comparator tuple count.
    pub vyre_tuple_count: u32,
    /// Weir comparator finding count.
    pub weir_finding_count: u32,
    /// Current Vyre comparator finding count.
    pub vyre_finding_count: u32,
    /// Generated analyzer compile time.
    pub compile_time_ns: u64,
    /// Generated analyzer run time.
    pub run_time_ns: u64,
    /// Memory bytes consumed by relation execution.
    pub memory_bytes: u64,
}

/// Generated analyzer output plus evidence.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GeneratedSecurityRelationAnalyzerReport {
    /// Normalized evidence row.
    pub evidence: GeneratedSecurityRelationAnalyzerEvidence,
    /// Fact-backed findings emitted by the analyzer.
    pub findings: Vec<FindingProofBundle>,
}

/// Relation-analyzer validation failure.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SecurityRelationAnalyzerError {
    /// Underlying fact or finding proof validation failed.
    Fact(AnalysisFactError),
    /// Required caller-supplied identity field was blank.
    BlankIdentity {
        /// Field name.
        field: &'static str,
    },
    /// Runtime/benchmark accounting was missing.
    MissingAccounting {
        /// Field name.
        field: &'static str,
    },
    /// Comparator baseline disagreed with generated relation output.
    BaselineMismatch {
        /// Baseline id.
        baseline_id: String,
        /// Compared field.
        field: &'static str,
        /// Generated value.
        generated: u32,
        /// Comparator value.
        comparator: u32,
    },
}

impl Display for SecurityRelationAnalyzerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fact(error) => Display::fmt(error, f),
            Self::BlankIdentity { field } => write!(
                f,
                "generated security relation analyzer field `{field}` is blank. Fix: record stable analyzer, rule, backend, evidence, and baseline ids."
            ),
            Self::MissingAccounting { field } => write!(
                f,
                "generated security relation analyzer accounting field `{field}` is zero. Fix: record compile time, run time, and memory bytes in benchmark evidence."
            ),
            Self::BaselineMismatch {
                baseline_id,
                field,
                generated,
                comparator,
            } => write!(
                f,
                "generated security relation analyzer baseline `{baseline_id}` disagrees for `{field}`: generated={generated}, comparator={comparator}. Fix: repair relation lowering or comparator fixture before accepting evidence."
            ),
        }
    }
}

impl Error for SecurityRelationAnalyzerError {}

impl From<AnalysisFactError> for SecurityRelationAnalyzerError {
    fn from(error: AnalysisFactError) -> Self {
        Self::Fact(error)
    }
}

/// Run the selected generated relation analyzer against canonical security facts.
///
/// # Errors
///
/// Returns [`SecurityRelationAnalyzerError`] when the fact table is malformed,
/// relation proof bundles are invalid, required metadata is blank, runtime
/// accounting is missing, or comparator counts disagree.
pub fn run_generated_security_relation_analyzer(
    table: &AnalysisFactTable,
    spec: GeneratedSecurityRelationAnalyzerSpec<'_>,
    stats: GeneratedSecurityRelationAnalyzerRunStats,
) -> Result<GeneratedSecurityRelationAnalyzerReport, SecurityRelationAnalyzerError> {
    validate_spec(spec)?;
    validate_stats(stats)?;
    table.validate()?;

    let relation_sets = SecurityRelationSets::from_table(table)?;
    let findings = match spec.family {
        SecurityRelationQueryFamily::UnsanitizedSourceToSink => {
            run_unsanitized_source_to_sink(table, spec, &relation_sets)?
        }
    };
    let generated_tuple_count = relation_sets.generated_tuple_count()?;
    let generated_finding_count = u32::try_from(findings.len()).map_err(|_| {
        SecurityRelationAnalyzerError::BaselineMismatch {
            baseline_id: spec.baseline_id.to_string(),
            field: "generated_finding_count",
            generated: u32::MAX,
            comparator: 0,
        }
    })?;
    compare_baseline(
        spec.baseline_id,
        "tuple_count",
        generated_tuple_count,
        stats.weir_tuple_count,
    )?;
    compare_baseline(
        spec.baseline_id,
        "tuple_count",
        generated_tuple_count,
        stats.vyre_tuple_count,
    )?;
    compare_baseline(
        spec.baseline_id,
        "finding_count",
        generated_finding_count,
        stats.weir_finding_count,
    )?;
    compare_baseline(
        spec.baseline_id,
        "finding_count",
        generated_finding_count,
        stats.vyre_finding_count,
    )?;

    Ok(GeneratedSecurityRelationAnalyzerReport {
        evidence: GeneratedSecurityRelationAnalyzerEvidence {
            schema_version: SECURITY_RELATION_ANALYZER_SCHEMA_VERSION,
            analyzer_id: spec.analyzer_id.to_string(),
            query_family: spec.family.as_str(),
            baseline_id: spec.baseline_id.to_string(),
            input_fact_count: u32::try_from(table.facts.len()).unwrap_or(u32::MAX),
            source_tuple_count: u32::try_from(relation_sets.sources.len()).unwrap_or(u32::MAX),
            sink_tuple_count: u32::try_from(relation_sets.sinks.len()).unwrap_or(u32::MAX),
            path_tuple_count: u32::try_from(relation_sets.paths.len()).unwrap_or(u32::MAX),
            sanitizer_tuple_count: u32::try_from(relation_sets.sanitizers.len())
                .unwrap_or(u32::MAX),
            generated_tuple_count,
            generated_finding_count,
            weir_tuple_count: stats.weir_tuple_count,
            vyre_tuple_count: stats.vyre_tuple_count,
            weir_finding_count: stats.weir_finding_count,
            vyre_finding_count: stats.vyre_finding_count,
            compile_time_ns: stats.compile_time_ns,
            run_time_ns: stats.run_time_ns,
            memory_bytes: stats.memory_bytes,
        },
        findings,
    })
}

fn validate_spec(
    spec: GeneratedSecurityRelationAnalyzerSpec<'_>,
) -> Result<(), SecurityRelationAnalyzerError> {
    for (field, value) in [
        ("analyzer_id", spec.analyzer_id),
        ("rule_id", spec.rule_id),
        ("backend_id", spec.backend_id),
        ("evidence_digest", spec.evidence_digest),
        ("baseline_id", spec.baseline_id),
    ] {
        if value.trim().is_empty() {
            return Err(SecurityRelationAnalyzerError::BlankIdentity { field });
        }
    }
    Ok(())
}

fn validate_stats(
    stats: GeneratedSecurityRelationAnalyzerRunStats,
) -> Result<(), SecurityRelationAnalyzerError> {
    for (field, value) in [
        ("compile_time_ns", stats.compile_time_ns),
        ("run_time_ns", stats.run_time_ns),
        ("memory_bytes", stats.memory_bytes),
    ] {
        if value == 0 {
            return Err(SecurityRelationAnalyzerError::MissingAccounting { field });
        }
    }
    Ok(())
}

fn compare_baseline(
    baseline_id: &str,
    field: &'static str,
    generated: u32,
    comparator: u32,
) -> Result<(), SecurityRelationAnalyzerError> {
    if generated != comparator {
        return Err(SecurityRelationAnalyzerError::BaselineMismatch {
            baseline_id: baseline_id.to_string(),
            field,
            generated,
            comparator,
        });
    }
    Ok(())
}

struct SecurityRelationSets<'a> {
    sources: Vec<&'a AnalysisFact>,
    sinks: Vec<&'a AnalysisFact>,
    paths: Vec<&'a AnalysisFact>,
    sanitizers: Vec<&'a AnalysisFact>,
}

impl<'a> SecurityRelationSets<'a> {
    fn from_table(table: &'a AnalysisFactTable) -> Result<Self, AnalysisFactError> {
        table.validate()?;
        let mut sources = Vec::new();
        let mut sinks = Vec::new();
        let mut paths = Vec::new();
        let mut sanitizers = Vec::new();
        for fact in &table.facts {
            match fact.kind {
                FactKind::Source => sources.push(fact),
                FactKind::Sink => sinks.push(fact),
                FactKind::Dataflow | FactKind::Edge | FactKind::Call | FactKind::Control => {
                    paths.push(fact)
                }
                FactKind::Sanitizer => sanitizers.push(fact),
                _ => {}
            }
        }
        Ok(Self {
            sources,
            sinks,
            paths,
            sanitizers,
        })
    }

    fn generated_tuple_count(&self) -> Result<u32, SecurityRelationAnalyzerError> {
        let total = self
            .sources
            .len()
            .checked_add(self.sinks.len())
            .and_then(|value| value.checked_add(self.paths.len()))
            .and_then(|value| value.checked_add(self.sanitizers.len()))
            .ok_or(SecurityRelationAnalyzerError::BaselineMismatch {
                baseline_id: "<generated>".to_string(),
                field: "tuple_count",
                generated: u32::MAX,
                comparator: 0,
            })?;
        u32::try_from(total).map_err(|_| SecurityRelationAnalyzerError::BaselineMismatch {
            baseline_id: "<generated>".to_string(),
            field: "tuple_count",
            generated: u32::MAX,
            comparator: 0,
        })
    }
}

fn run_unsanitized_source_to_sink(
    table: &AnalysisFactTable,
    spec: GeneratedSecurityRelationAnalyzerSpec<'_>,
    relation_sets: &SecurityRelationSets<'_>,
) -> Result<Vec<FindingProofBundle>, SecurityRelationAnalyzerError> {
    let mut findings = Vec::new();
    for source in &relation_sets.sources {
        for sink in &relation_sets.sinks {
            let path_fact_ids = relation_sets
                .paths
                .iter()
                .filter(|path| relates(path, source.subject, sink.subject))
                .map(|path| path.id)
                .collect::<Vec<_>>();
            if path_fact_ids.is_empty() {
                continue;
            }
            let sanitizer_fact_ids = relation_sets
                .sanitizers
                .iter()
                .filter(|sanitizer| relates(sanitizer, source.subject, sink.subject))
                .map(|sanitizer| sanitizer.id)
                .collect::<Vec<_>>();
            if !sanitizer_fact_ids.is_empty() {
                continue;
            }
            let request = SourceToSinkFindingRequest {
                finding_id: format!("{}:{}:{}", spec.rule_id, source.id.0, sink.id.0),
                query_id: spec.analyzer_id.to_string(),
                backend_id: spec.backend_id.to_string(),
                evidence_digest: spec.evidence_digest.to_string(),
                precision_contract: spec.precision_contract,
                source_fact_id: source.id,
                sink_fact_id: sink.id,
                path_fact_ids,
                sanitizer_fact_ids,
                query_hit: 1,
                confidence_bps: source.confidence_bps.min(sink.confidence_bps),
                reason: "generated relation analyzer found an unsanitized source-to-sink path"
                    .to_string(),
            };
            if let Some(bundle) = finding_from_sanitized_source_to_sink_query(table, request)? {
                findings.push(bundle);
            }
        }
    }
    findings.sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
    Ok(findings)
}

fn relates(fact: &AnalysisFact, source_subject: u64, sink_subject: u64) -> bool {
    fact.subject == source_subject && fact.object == Some(sink_subject)
}

/// Collect unique fact ids from relation analyzer findings.
#[must_use]
pub fn generated_relation_finding_fact_ids(findings: &[FindingProofBundle]) -> Vec<FactId> {
    let mut ids = Vec::new();
    for finding in findings {
        for fact_id in &finding.fact_ids {
            if !ids.contains(fact_id) {
                ids.push(*fact_id);
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataflow::PrecisionContract;
    use crate::security::{AnalysisSourceSpan, FactId};

    fn span(byte: u32) -> AnalysisSourceSpan {
        AnalysisSourceSpan::byte_range(1, byte, byte + 1)
    }

    fn fact(id: u64, kind: FactKind, subject: u64, object: Option<u64>) -> AnalysisFact {
        let mut fact = AnalysisFact::exact(FactId(id), kind, span(id as u32), subject);
        fact.object = object;
        fact
    }

    fn spec() -> GeneratedSecurityRelationAnalyzerSpec<'static> {
        GeneratedSecurityRelationAnalyzerSpec {
            analyzer_id: "generated.security.unsanitized-source-sink",
            family: SecurityRelationQueryFamily::UnsanitizedSourceToSink,
            rule_id: "SEC-GEN-001",
            backend_id: "generated-relation-oracle",
            evidence_digest: "relation-evidence-digest",
            precision_contract: PrecisionContract::ZeroFalsePositive,
            baseline_id: "weir-ifds+vyre-current",
        }
    }

    fn stats(tuple_count: u32, finding_count: u32) -> GeneratedSecurityRelationAnalyzerRunStats {
        GeneratedSecurityRelationAnalyzerRunStats {
            compile_time_ns: 11,
            run_time_ns: 17,
            memory_bytes: 128,
            weir_tuple_count: tuple_count,
            vyre_tuple_count: tuple_count,
            weir_finding_count: finding_count,
            vyre_finding_count: finding_count,
        }
    }

    #[test]
    fn generated_relation_analyzer_emits_fact_backed_finding_and_evidence() {
        let table = AnalysisFactTable::new(vec![
            fact(1, FactKind::Source, 10, None),
            fact(2, FactKind::Sink, 20, None),
            fact(3, FactKind::Dataflow, 10, Some(20)),
        ]);

        let report =
            run_generated_security_relation_analyzer(&table, spec(), stats(3, 1)).unwrap();

        assert_eq!(
            report.evidence.schema_version,
            SECURITY_RELATION_ANALYZER_SCHEMA_VERSION
        );
        assert_eq!(report.evidence.source_tuple_count, 1);
        assert_eq!(report.evidence.sink_tuple_count, 1);
        assert_eq!(report.evidence.path_tuple_count, 1);
        assert_eq!(report.evidence.generated_tuple_count, 3);
        assert_eq!(report.evidence.generated_finding_count, 1);
        assert_eq!(report.evidence.compile_time_ns, 11);
        assert_eq!(report.evidence.run_time_ns, 17);
        assert_eq!(report.evidence.memory_bytes, 128);
        assert_eq!(report.findings[0].finding_id, "SEC-GEN-001:1:2");
        assert_eq!(
            generated_relation_finding_fact_ids(&report.findings),
            vec![FactId(1), FactId(3), FactId(2)]
        );
    }

    #[test]
    fn generated_relation_analyzer_suppresses_sanitized_path() {
        let table = AnalysisFactTable::new(vec![
            fact(1, FactKind::Source, 10, None),
            fact(2, FactKind::Sink, 20, None),
            fact(3, FactKind::Dataflow, 10, Some(20)),
            fact(4, FactKind::Sanitizer, 10, Some(20)),
        ]);

        let report =
            run_generated_security_relation_analyzer(&table, spec(), stats(4, 0)).unwrap();

        assert!(report.findings.is_empty());
        assert_eq!(report.evidence.sanitizer_tuple_count, 1);
        assert_eq!(report.evidence.generated_finding_count, 0);
    }

    #[test]
    fn generated_relation_analyzer_rejects_baseline_count_mismatch() {
        let table = AnalysisFactTable::new(vec![
            fact(1, FactKind::Source, 10, None),
            fact(2, FactKind::Sink, 20, None),
            fact(3, FactKind::Dataflow, 10, Some(20)),
        ]);

        let error = run_generated_security_relation_analyzer(&table, spec(), stats(2, 1))
            .expect_err("baseline tuple mismatch must reject");

        assert!(matches!(
            error,
            SecurityRelationAnalyzerError::BaselineMismatch {
                field: "tuple_count",
                generated: 3,
                comparator: 2,
                ..
            }
        ));
    }

    #[test]
    fn generated_relation_analyzer_requires_compile_run_and_memory_accounting() {
        let table = AnalysisFactTable::new(vec![
            fact(1, FactKind::Source, 10, None),
            fact(2, FactKind::Sink, 20, None),
            fact(3, FactKind::Dataflow, 10, Some(20)),
        ]);
        let mut missing = stats(3, 1);
        missing.compile_time_ns = 0;

        let error = run_generated_security_relation_analyzer(&table, spec(), missing)
            .expect_err("missing compile time must reject");

        assert!(matches!(
            error,
            SecurityRelationAnalyzerError::MissingAccounting {
                field: "compile_time_ns"
            }
        ));
    }
}

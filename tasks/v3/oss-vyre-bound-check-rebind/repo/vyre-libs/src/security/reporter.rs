//! Deterministic security finding reporter output contract.
//!
//! Scan engines may use literal, regex, vector, or graph planners underneath,
//! but reporter-facing bytes must stay stable: exact file, line, column, rule,
//! confidence, ordering, and diagnostics are part of the public detection
//! contract.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{FindingProofBundle, FindingProofStep};

/// Stable reporter schema version for JSON/SARIF/CLI byte contracts.
pub const SECURITY_REPORTER_SCHEMA_VERSION: u32 = 1;

/// Planner path that produced or verified a finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SecurityReporterPlannerPath {
    /// Literal matcher path.
    Literal,
    /// Regex automata path.
    Regex,
    /// Vector or ANN-assisted path.
    Vector,
    /// Graph/dataflow path.
    Graph,
}

impl SecurityReporterPlannerPath {
    /// Stable lowercase token for this planner path, used in reporter output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::Regex => "regex",
            Self::Vector => "vector",
            Self::Graph => "graph",
        }
    }
}

/// One finding plus reporter-only metadata that is not owned by the proof bundle.
#[derive(Clone, Debug)]
pub struct SecurityReporterFinding {
    /// Stable user-facing rule id.
    pub rule_id: String,
    /// Planner path used for this finding.
    pub planner_path: SecurityReporterPlannerPath,
    /// Fact-backed finding proof.
    pub bundle: FindingProofBundle,
}

/// Source file id to path mapping used by proof spans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityReporterSourceFile {
    /// Stable file id used by [`super::AnalysisSourceSpan`].
    pub file_id: u32,
    /// Display path emitted in JSON, SARIF, and CLI output.
    pub path: String,
}

/// Exact reporter bytes for all supported output modes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityReporterOutputBytes {
    /// Deterministic compact JSON bytes, newline terminated.
    pub json: Vec<u8>,
    /// Deterministic SARIF 2.1.0 bytes, newline terminated.
    pub sarif: Vec<u8>,
    /// Deterministic CLI bytes, newline terminated when findings exist.
    pub cli: Vec<u8>,
    /// Process exit code represented by this finding set.
    pub exit_code: i32,
}

/// Render fact-backed security findings into stable JSON, SARIF, and CLI bytes.
///
/// # Errors
/// Returns [`SecurityReporterError`] when rule ids, spans, file mappings,
/// confidence, or JSON serialization are invalid.
pub fn render_security_reporter_output(
    findings: &[SecurityReporterFinding],
    source_files: &[SecurityReporterSourceFile],
) -> Result<SecurityReporterOutputBytes, SecurityReporterError> {
    let file_paths = source_files
        .iter()
        .map(|file| (file.file_id, file.path.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut records = findings
        .iter()
        .map(|finding| reporter_record(finding, &file_paths))
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.column.cmp(&right.column))
            .then_with(|| left.rule_id.cmp(&right.rule_id))
            .then_with(|| left.finding_id.cmp(&right.finding_id))
    });
    let exit_code = if records.is_empty() { 0 } else { 1 };
    Ok(SecurityReporterOutputBytes {
        json: json_bytes(&records)?,
        sarif: sarif_bytes(&records)?,
        cli: cli_bytes(&records),
        exit_code,
    })
}

fn reporter_record(
    finding: &SecurityReporterFinding,
    file_paths: &BTreeMap<u32, &str>,
) -> Result<SecurityReporterRecord, SecurityReporterError> {
    if finding.rule_id.trim().is_empty() {
        return Err(SecurityReporterError::BlankRuleId {
            finding_id: finding.bundle.finding_id.clone(),
        });
    }
    let primary = finding
        .bundle
        .proof_path
        .first()
        .ok_or_else(|| SecurityReporterError::MissingProofPath {
            finding_id: finding.bundle.finding_id.clone(),
        })?;
    let path = file_paths
        .get(&primary.span.file_id)
        .ok_or(SecurityReporterError::MissingSourceFile {
            file_id: primary.span.file_id,
        })?;
    if primary.span.start_line == 0 || primary.span.start_column == 0 {
        return Err(SecurityReporterError::MissingLineColumn {
            finding_id: finding.bundle.finding_id.clone(),
        });
    }
    if finding.bundle.confidence_bps > 10_000 {
        return Err(SecurityReporterError::InvalidConfidence {
            finding_id: finding.bundle.finding_id.clone(),
            confidence_bps: finding.bundle.confidence_bps,
        });
    }
    Ok(SecurityReporterRecord {
        finding_id: finding.bundle.finding_id.clone(),
        rule_id: finding.rule_id.trim().to_string(),
        query_id: finding.bundle.query_id.clone(),
        backend_id: finding.bundle.backend_id.clone(),
        planner_path: finding.planner_path.as_str().to_string(),
        file_id: primary.span.file_id,
        path: (*path).to_string(),
        line: primary.span.start_line,
        column: primary.span.start_column,
        end_line: primary.span.end_line,
        end_column: primary.span.end_column,
        confidence_bps: finding.bundle.confidence_bps,
        evidence_digest: finding.bundle.evidence_digest.clone(),
        reason: finding.bundle.reason.clone(),
        proof_roles: proof_roles(&finding.bundle.proof_path),
    })
}

fn proof_roles(proof_path: &[FindingProofStep]) -> Vec<String> {
    proof_path
        .iter()
        .map(|step| step.role.trim().to_string())
        .collect()
}

fn json_bytes(records: &[SecurityReporterRecord]) -> Result<Vec<u8>, SecurityReporterError> {
    let mut bytes = serde_json::to_vec(&SecurityReporterJson {
        schema_version: SECURITY_REPORTER_SCHEMA_VERSION,
        finding_count: records.len(),
        findings: records,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sarif_bytes(records: &[SecurityReporterRecord]) -> Result<Vec<u8>, SecurityReporterError> {
    let rules = records
        .iter()
        .map(|record| record.rule_id.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|rule_id| serde_json::json!({ "id": rule_id }))
        .collect::<Vec<_>>();
    let results = records
        .iter()
        .map(|record| {
            serde_json::json!({
                "ruleId": &record.rule_id,
                "level": "warning",
                "message": { "text": &record.reason },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": &record.path },
                        "region": {
                            "startLine": record.line,
                            "startColumn": record.column,
                            "endLine": record.end_line,
                            "endColumn": record.end_column
                        }
                    }
                }],
                "properties": {
                    "finding_id": &record.finding_id,
                    "query_id": &record.query_id,
                    "backend_id": &record.backend_id,
                    "planner_path": &record.planner_path,
                    "confidence_bps": record.confidence_bps,
                    "evidence_digest": &record.evidence_digest,
                    "proof_roles": &record.proof_roles
                }
            })
        })
        .collect::<Vec<_>>();
    let mut bytes = serde_json::to_vec(&serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "vyre-security",
                    "rules": rules
                }
            },
            "results": results
        }]
    }))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn cli_bytes(records: &[SecurityReporterRecord]) -> Vec<u8> {
    let mut out = String::new();
    for record in records {
        out.push_str(&format!(
            "{}:{}:{}: {} {}bp {} [{}]: {}\n",
            record.path,
            record.line,
            record.column,
            record.rule_id,
            record.confidence_bps,
            record.finding_id,
            record.planner_path,
            record.reason
        ));
    }
    out.into_bytes()
}

#[derive(Serialize)]
struct SecurityReporterJson<'a> {
    schema_version: u32,
    finding_count: usize,
    findings: &'a [SecurityReporterRecord],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SecurityReporterRecord {
    finding_id: String,
    rule_id: String,
    query_id: String,
    backend_id: String,
    planner_path: String,
    file_id: u32,
    path: String,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    confidence_bps: u16,
    evidence_digest: String,
    reason: String,
    proof_roles: Vec<String>,
}

/// Reporter rendering errors.
#[derive(Debug, thiserror::Error)]
pub enum SecurityReporterError {
    /// Rule id is blank.
    #[error("finding `{finding_id}` has a blank rule id. Fix: attach stable rule ids before reporter rendering.")]
    BlankRuleId {
        /// Finding id.
        finding_id: String,
    },
    /// Finding has no proof path.
    #[error("finding `{finding_id}` has no proof path. Fix: reporter output needs an exact source span.")]
    MissingProofPath {
        /// Finding id.
        finding_id: String,
    },
    /// No file path exists for the proof span file id.
    #[error("source file id {file_id} has no reporter path mapping. Fix: pass the corpus file table to reporter rendering.")]
    MissingSourceFile {
        /// Missing file id.
        file_id: u32,
    },
    /// Primary source span lacks line/column data.
    #[error("finding `{finding_id}` primary span has no line/column. Fix: populate one-based line and column before reporting.")]
    MissingLineColumn {
        /// Finding id.
        finding_id: String,
    },
    /// Finding confidence is outside 0..=10000 basis points.
    #[error("finding `{finding_id}` confidence {confidence_bps} exceeds 10000. Fix: store confidence in basis points.")]
    InvalidConfidence {
        /// Finding id.
        finding_id: String,
        /// Invalid confidence.
        confidence_bps: u16,
    },
    /// JSON serialization failed.
    #[error("security reporter serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use crate::dataflow::{DynamicPrimitiveSoundness, PrecisionContract, Soundness};

    use super::*;
    use crate::security::{AnalysisSourceSpan, FactId};

    #[test]
    fn reporter_output_bytes_are_stable_and_sorted_across_planner_paths() {
        let findings = vec![
            finding("f.regex", "SEC-REGEX", SecurityReporterPlannerPath::Regex, 9, 4),
            finding("f.literal", "SEC-LITERAL", SecurityReporterPlannerPath::Literal, 2, 7),
            finding("f.graph", "SEC-GRAPH", SecurityReporterPlannerPath::Graph, 9, 1),
            finding("f.vector", "SEC-VECTOR", SecurityReporterPlannerPath::Vector, 4, 3),
        ];
        let output = render_security_reporter_output(
            &findings,
            &[SecurityReporterSourceFile {
                file_id: 1,
                path: "src/app.rs".to_string(),
            }],
        )
        .expect("Fix: reporter rendering should accept valid finding bundles.");

        assert_eq!(output.exit_code, 1);
        let cli = String::from_utf8(output.cli).expect("Fix: CLI bytes must be UTF-8.");
        assert!(
            cli.find("src/app.rs:2:7: SEC-LITERAL")
                < cli.find("src/app.rs:4:3: SEC-VECTOR"),
            "Fix: CLI output must be sorted by file, line, column, rule, finding id; cli={cli}"
        );
        assert!(
            cli.contains("src/app.rs:9:1: SEC-GRAPH 9800bp f.graph [graph]: source reaches sink"),
            "Fix: CLI output must include exact location, rule, confidence, finding id, planner path, and reason; cli={cli}"
        );
        let json = String::from_utf8(output.json).expect("Fix: JSON bytes must be UTF-8.");
        assert!(json.contains(r#""finding_count":4"#));
        assert!(json.contains(r#""planner_path":"regex""#));
        assert!(json.ends_with('\n'));
        let sarif = String::from_utf8(output.sarif).expect("Fix: SARIF bytes must be UTF-8.");
        assert!(sarif.contains(r#""version":"2.1.0""#));
        assert!(sarif.contains(r#""ruleId":"SEC-GRAPH""#));
        assert!(sarif.contains(r#""confidence_bps":9800"#));
    }

    #[test]
    fn reporter_rejects_missing_line_column() {
        let mut missing = finding(
            "f.missing-location",
            "SEC-MISSING",
            SecurityReporterPlannerPath::Literal,
            0,
            0,
        );
        missing.bundle.proof_path[0].span.start_line = 0;

        let error = render_security_reporter_output(
            &[missing],
            &[SecurityReporterSourceFile {
                file_id: 1,
                path: "src/app.rs".to_string(),
            }],
        )
        .expect_err("Fix: reporter must reject source spans without one-based line/column.");

        assert!(matches!(error, SecurityReporterError::MissingLineColumn { .. }));
    }

    fn finding(
        finding_id: &str,
        rule_id: &str,
        planner_path: SecurityReporterPlannerPath,
        line: u32,
        column: u32,
    ) -> SecurityReporterFinding {
        SecurityReporterFinding {
            rule_id: rule_id.to_string(),
            planner_path,
            bundle: FindingProofBundle {
                finding_id: finding_id.to_string(),
                query_id: "vyre-libs::security::flows_to_with_sanitizer".to_string(),
                backend_id: "cpu-ref".to_string(),
                evidence_digest: "evidence:abc123".to_string(),
                precision_contract: PrecisionContract::ZeroFalsePositive,
                soundness: Soundness::Exact,
                primitive_soundness: vec![DynamicPrimitiveSoundness::new(
                    "vyre-libs::security::flows_to",
                    Soundness::Exact,
                )],
                fact_ids: vec![FactId(1)],
                proof_path: vec![FindingProofStep::new(
                    FactId(1),
                    AnalysisSourceSpan {
                        file_id: 1,
                        start_byte: 8,
                        end_byte: 16,
                        start_line: line,
                        start_column: column,
                        end_line: line,
                        end_column: column + 8,
                    },
                    "source",
                )],
                confidence_bps: 9800,
                reason: "source reaches sink".to_string(),
            },
        }
    }
}

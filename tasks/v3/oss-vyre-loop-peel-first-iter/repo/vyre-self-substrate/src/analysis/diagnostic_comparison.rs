//! Structured clang diagnostic comparison validation.

use std::collections::BTreeSet;

/// Diagnostic field compared against clang.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticComparisonField {
    /// Source location.
    Location,
    /// Severity.
    Severity,
    /// Diagnostic category.
    Category,
    /// Primary message class.
    PrimaryMessageClass,
}

/// One diagnostic comparison record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticComparisonRecord<'a> {
    /// Covered field.
    pub field: DiagnosticComparisonField,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path.
    pub evidence: &'a str,
}

/// Diagnostic comparison proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticComparisonProof {
    /// Covered field count.
    pub field_count: usize,
}

/// Committed clang diagnostic oracle source proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClangDiagnosticOracleSourceProof {
    /// Required test assertion marker count.
    pub assertion_marker_count: usize,
}

/// Diagnostic comparison validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticComparisonError {
    /// Records are empty.
    EmptyRecords,
    /// Metadata is empty.
    EmptyMetadata {
        /// Field being compared.
        comparison_field: DiagnosticComparisonField,
        /// Metadata field.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Command.
        command: String,
    },
    /// Required diagnostic field is missing.
    MissingComparisonField {
        /// Missing field.
        field: DiagnosticComparisonField,
    },
    /// Committed clang diagnostic oracle source is missing required evidence.
    OracleSourceMissingEvidence {
        /// Missing evidence field.
        evidence: &'static str,
    },
}

impl std::fmt::Display for DiagnosticComparisonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "diagnostic comparison records are empty. Fix: compare location, severity, category, and primary message class against clang."
            ),
            Self::EmptyMetadata {
                comparison_field,
                field,
            } => write!(
                f,
                "diagnostic comparison {comparison_field:?} has empty {field}. Fix: every field needs command and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { command } => write!(
                f,
                "diagnostic comparison command `{command}` does not use ./cargo_full. Fix: run diagnostic comparison through cargo_full."
            ),
            Self::MissingComparisonField { field } => write!(
                f,
                "diagnostic comparison is missing {field:?}. Fix: add structured comparison evidence for that field."
            ),
            Self::OracleSourceMissingEvidence { evidence } => write!(
                f,
                "clang diagnostic oracle source is missing {evidence}. Fix: preserve committed clang diagnostic extraction for severity, category, message, location, recovery, and fix-its."
            ),
        }
    }
}

impl std::error::Error for DiagnosticComparisonError {}

const REQUIRED_FIELDS: &[DiagnosticComparisonField] = &[
    DiagnosticComparisonField::Location,
    DiagnosticComparisonField::Severity,
    DiagnosticComparisonField::Category,
    DiagnosticComparisonField::PrimaryMessageClass,
];

/// Validate structured diagnostic comparison coverage.
pub fn validate_diagnostic_comparison(
    records: &[DiagnosticComparisonRecord<'_>],
) -> Result<DiagnosticComparisonProof, DiagnosticComparisonError> {
    if records.is_empty() {
        return Err(DiagnosticComparisonError::EmptyRecords);
    }
    let mut fields = BTreeSet::new();
    for record in records {
        for (field, value) in [("command", record.command), ("evidence", record.evidence)] {
            if value.trim().is_empty() {
                return Err(DiagnosticComparisonError::EmptyMetadata {
                    comparison_field: record.field,
                    field,
                });
            }
        }
        if !record.command.trim_start().starts_with("./cargo_full ") {
            return Err(DiagnosticComparisonError::CommandDoesNotUseCargoFull {
                command: record.command.to_owned(),
            });
        }
        fields.insert(record.field);
    }
    for field in REQUIRED_FIELDS {
        if !fields.contains(field) {
            return Err(DiagnosticComparisonError::MissingComparisonField { field: *field });
        }
    }
    Ok(DiagnosticComparisonProof {
        field_count: fields.len(),
    })
}

/// Validate committed clang diagnostic oracle source files.
pub fn validate_clang_diagnostic_oracle_sources(
    test_source: &str,
    support_source: &str,
) -> Result<ClangDiagnosticOracleSourceProof, DiagnosticComparisonError> {
    for (evidence, needle) in [
        (
            "clang diagnostics test entrypoint",
            "clang_diagnostics_oracle_records_severity_location_and_fixits",
        ),
        ("expected expression diagnostic", "expected expression"),
        (
            "missing semicolon diagnostic",
            "expected ';' after return statement",
        ),
        ("severity assertion", "expression_error.severity"),
        ("category assertion", "expression_error.category"),
        ("sequence index assertion", "sequence_index"),
        ("recovery assertion", "recovered_after_error"),
        ("line assertion", "location.line"),
        ("column assertion", "location.column"),
        ("fix-it length assertion", "fixits.len()"),
        ("fix-it replacement assertion", "fixits[0].replacement"),
        ("fix-it span assertion", "fixits[0].start_column"),
    ] {
        source_contains(test_source, evidence, needle)?;
    }

    for (evidence, needle) in [
        ("clang executable invocation", "Command::new(\"clang\")"),
        ("parseable fix-it flag", "-fdiagnostics-parseable-fixits"),
        ("C language mode", "-x"),
        ("diagnostic fact struct", "struct ClangDiagnosticFact"),
        ("severity field", "severity: String"),
        ("category field", "category: String"),
        ("message field", "message: String"),
        ("location field", "location: ClangDiagnosticLocation"),
        ("fixits field", "fixits: Vec<ClangFixIt>"),
        ("raw line field", "raw_line: String"),
        ("recovery field", "recovered_after_error: bool"),
        ("diagnostic parser", "parse_diagnostic_line"),
        ("fix-it parser", "parse_fixit_line"),
        ("location parser", "parse_location"),
        ("line-column parser", "parse_line_col"),
    ] {
        source_contains(support_source, evidence, needle)?;
    }

    let assertion_marker_count = test_source.matches("assert").count();
    if assertion_marker_count < 10 {
        return Err(DiagnosticComparisonError::OracleSourceMissingEvidence {
            evidence: "at least ten diagnostic oracle assertions",
        });
    }

    Ok(ClangDiagnosticOracleSourceProof {
        assertion_marker_count,
    })
}

fn source_contains(
    source: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), DiagnosticComparisonError> {
    if source.contains(needle) {
        Ok(())
    } else {
        Err(DiagnosticComparisonError::OracleSourceMissingEvidence { evidence })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_comparison_accepts_all_required_fields() {
        let proof = validate_diagnostic_comparison(&records())
            .expect("Fix: complete diagnostic comparison should pass");

        assert_eq!(proof.field_count, 4);
    }

    #[test]
    fn diagnostic_comparison_rejects_missing_message_class() {
        let mut records = records();
        records.pop();

        assert_eq!(
            validate_diagnostic_comparison(&records)
                .expect_err("missing message class should fail"),
            DiagnosticComparisonError::MissingComparisonField {
                field: DiagnosticComparisonField::PrimaryMessageClass,
            }
        );
    }

    #[test]
    fn diagnostic_comparison_rejects_raw_cargo() {
        let mut records = records();
        records[0].command = "cargo test";

        assert_eq!(
            validate_diagnostic_comparison(&records).expect_err("raw cargo should fail"),
            DiagnosticComparisonError::CommandDoesNotUseCargoFull {
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn diagnostic_comparison_accepts_committed_clang_oracle_sources() {
        let proof = validate_clang_diagnostic_oracle_sources(
            include_str!("../../../vyre-frontend-c/tests/clang_diagnostics_oracle.rs"),
            include_str!("../../../vyre-frontend-c/tests/support/clang_diagnostics.rs"),
        )
        .expect("Fix: committed clang diagnostic oracle sources should pass");

        assert!(proof.assertion_marker_count >= 10);
    }

    #[test]
    fn diagnostic_comparison_rejects_oracle_without_fixits() {
        let test_source = r#"
            fn clang_diagnostics_oracle_records_severity_location_and_fixits() {
                let expression_error = diagnostic();
                assert_eq!(expression_error.severity, "error");
                assert_eq!(expression_error.category, "error");
                assert_eq!(expression_error.sequence_index, 0);
                assert!(!expression_error.recovered_after_error);
                assert_eq!(expression_error.location.line, 1);
                assert_eq!(expression_error.location.column, 23);
            }
        "#;
        let support_source = r#"
            use std::process::Command;
            struct ClangDiagnosticFact {
                severity: String,
                category: String,
                message: String,
                location: ClangDiagnosticLocation,
                fixits: Vec<ClangFixIt>,
                raw_line: String,
                recovered_after_error: bool,
            }
            fn clang_diagnostics() { Command::new("clang").args(["-fdiagnostics-parseable-fixits", "-x", "c"]); }
            fn parse_diagnostic_line() {}
            fn parse_fixit_line() {}
            fn parse_location() {}
            fn parse_line_col() {}
        "#;

        assert_eq!(
            validate_clang_diagnostic_oracle_sources(test_source, support_source)
                .expect_err("oracle without fix-it assertions should fail"),
            DiagnosticComparisonError::OracleSourceMissingEvidence {
                evidence: "expected expression diagnostic",
            }
        );
    }

    fn records() -> Vec<DiagnosticComparisonRecord<'static>> {
        REQUIRED_FIELDS.iter().copied().map(record).collect()
    }

    fn record(field: DiagnosticComparisonField) -> DiagnosticComparisonRecord<'static> {
        DiagnosticComparisonRecord {
            field,
            command: "./cargo_full test -j1 -p vyrec",
            evidence: "release/parity/vyrec-diagnostic-comparison.md",
        }
    }
}

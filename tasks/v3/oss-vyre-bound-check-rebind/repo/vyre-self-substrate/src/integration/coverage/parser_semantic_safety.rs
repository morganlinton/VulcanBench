//! Vyrec parser recovery and semantic invariant safety validation.

use std::collections::BTreeSet;

/// Parser recovery behavior that must be covered.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ParserRecoveryCase {
    /// Malformed input emits diagnostics.
    MalformedEmitsDiagnostic,
    /// Malformed input does not produce a fake successful AST.
    NoFakeAstSuccess,
    /// Diagnostic location remains precise after recovery.
    PreciseRecoveryLocation,
}

/// Semantic invariant that must be covered.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticInvariantCase {
    /// Invalid lvalue/rvalue state is rejected.
    InvalidLvalueRejected,
    /// Unknown declaration scope is rejected.
    UnknownScopeRejected,
    /// Invalid type conversion state is rejected.
    InvalidConversionRejected,
    /// Conflicting namespace state is rejected.
    ConflictingNamespaceRejected,
}

/// One parser recovery evidence record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParserRecoveryRecord<'a> {
    /// Covered case.
    pub case: ParserRecoveryCase,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path.
    pub evidence: &'a str,
}

/// One semantic invariant evidence record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticInvariantRecord<'a> {
    /// Covered invariant.
    pub case: SemanticInvariantCase,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path.
    pub evidence: &'a str,
}

/// Parser/semantic safety proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParserSemanticSafetyProof {
    /// Parser recovery case count.
    pub parser_case_count: usize,
    /// Semantic invariant count.
    pub semantic_case_count: usize,
}

/// Committed parser/semantic source-safety proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParserSemanticSourceSafetyProof {
    /// Assertion marker count in committed semantic safety tests.
    pub assertion_marker_count: usize,
}

/// Parser/semantic safety validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParserSemanticSafetyError {
    /// Parser records are empty.
    EmptyParserRecords,
    /// Semantic records are empty.
    EmptySemanticRecords,
    /// Command/evidence metadata is empty.
    EmptyMetadata {
        /// Field.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Command.
        command: String,
    },
    /// Required parser recovery case is missing.
    MissingParserCase {
        /// Case.
        case: ParserRecoveryCase,
    },
    /// Required semantic invariant is missing.
    MissingSemanticCase {
        /// Case.
        case: SemanticInvariantCase,
    },
    /// Committed source evidence is missing required parser/semantic safety proof.
    SourceMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed source evidence missed a threshold.
    SourceThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: usize,
        /// Required value.
        required: usize,
    },
}

impl std::fmt::Display for ParserSemanticSafetyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyParserRecords => write!(
                f,
                "parser recovery evidence is empty. Fix: prove malformed input diagnostics, no fake AST success, and precise recovery locations."
            ),
            Self::EmptySemanticRecords => write!(
                f,
                "semantic invariant evidence is empty. Fix: prove impossible AST/semantic states are rejected before lowering."
            ),
            Self::EmptyMetadata { field } => write!(
                f,
                "parser/semantic safety evidence has empty {field}. Fix: every record needs command and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { command } => write!(
                f,
                "parser/semantic safety command `{command}` does not use ./cargo_full. Fix: run evidence through cargo_full."
            ),
            Self::MissingParserCase { case } => write!(
                f,
                "parser recovery evidence is missing {case:?}. Fix: add explicit recovery evidence for that behavior."
            ),
            Self::MissingSemanticCase { case } => write!(
                f,
                "semantic invariant evidence is missing {case:?}. Fix: add explicit rejection evidence for that impossible state."
            ),
            Self::SourceMissingEvidence { evidence } => write!(
                f,
                "parser/semantic source safety is missing {evidence}. Fix: preserve source-backed rejection tests and validators for malformed semantic objects."
            ),
            Self::SourceThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "parser/semantic source safety {field}={observed} missed required {required}. Fix: restore adversarial semantic rejection assertions."
            ),
        }
    }
}

impl std::error::Error for ParserSemanticSafetyError {}

const REQUIRED_PARSER_CASES: &[ParserRecoveryCase] = &[
    ParserRecoveryCase::MalformedEmitsDiagnostic,
    ParserRecoveryCase::NoFakeAstSuccess,
    ParserRecoveryCase::PreciseRecoveryLocation,
];

const REQUIRED_SEMANTIC_CASES: &[SemanticInvariantCase] = &[
    SemanticInvariantCase::InvalidLvalueRejected,
    SemanticInvariantCase::UnknownScopeRejected,
    SemanticInvariantCase::InvalidConversionRejected,
    SemanticInvariantCase::ConflictingNamespaceRejected,
];

/// Validate parser recovery and semantic invariant evidence.
pub fn validate_parser_semantic_safety(
    parser_records: &[ParserRecoveryRecord<'_>],
    semantic_records: &[SemanticInvariantRecord<'_>],
) -> Result<ParserSemanticSafetyProof, ParserSemanticSafetyError> {
    if parser_records.is_empty() {
        return Err(ParserSemanticSafetyError::EmptyParserRecords);
    }
    if semantic_records.is_empty() {
        return Err(ParserSemanticSafetyError::EmptySemanticRecords);
    }

    let mut parser_cases = BTreeSet::new();
    for record in parser_records {
        validate_record(record.command, record.evidence)?;
        parser_cases.insert(record.case);
    }
    let mut semantic_cases = BTreeSet::new();
    for record in semantic_records {
        validate_record(record.command, record.evidence)?;
        semantic_cases.insert(record.case);
    }

    for case in REQUIRED_PARSER_CASES {
        if !parser_cases.contains(case) {
            return Err(ParserSemanticSafetyError::MissingParserCase { case: *case });
        }
    }
    for case in REQUIRED_SEMANTIC_CASES {
        if !semantic_cases.contains(case) {
            return Err(ParserSemanticSafetyError::MissingSemanticCase { case: *case });
        }
    }

    Ok(ParserSemanticSafetyProof {
        parser_case_count: parser_cases.len(),
        semantic_case_count: semantic_cases.len(),
    })
}

fn validate_record(command: &str, evidence: &str) -> Result<(), ParserSemanticSafetyError> {
    if command.trim().is_empty() {
        return Err(ParserSemanticSafetyError::EmptyMetadata { field: "command" });
    }
    if evidence.trim().is_empty() {
        return Err(ParserSemanticSafetyError::EmptyMetadata { field: "evidence" });
    }
    if !command.trim_start().starts_with("./cargo_full ") {
        return Err(ParserSemanticSafetyError::CommandDoesNotUseCargoFull {
            command: command.to_owned(),
        });
    }
    Ok(())
}

/// Validate committed source-backed parser/semantic safety evidence.
pub fn validate_parser_semantic_source_safety(
    object_decode_tests_source: &str,
    semantic_graph_source: &str,
) -> Result<ParserSemanticSourceSafetyProof, ParserSemanticSafetyError> {
    for (evidence, needle) in [
        (
            "out-of-range semantic tree link test",
            "decode_object_semantic_graph_rejects_out_of_range_tree_links",
        ),
        (
            "out-of-range semantic edge test",
            "decode_object_semantic_graph_rejects_out_of_range_edges",
        ),
        (
            "unknown semantic role test",
            "decode_object_semantic_graph_rejects_unknown_node_role",
        ),
        (
            "empty semantic node section test",
            "decode_object_semantic_graph_rejects_empty_node_section",
        ),
        (
            "missing parent scope test",
            "decode_object_sema_scope_rejects_missing_parent_scope",
        ),
        (
            "multiple root scope test",
            "decode_object_sema_scope_rejects_multiple_roots",
        ),
        (
            "unknown declaration kind test",
            "decode_object_sema_scope_rejects_unknown_decl_kind",
        ),
        ("explicit expect_err assertions", "expect_err"),
    ] {
        source_contains(object_decode_tests_source, evidence, needle)?;
    }

    for (evidence, needle) in [
        ("semantic node validator", "validate_semantic_pg_nodes"),
        ("semantic edge validator", "validate_semantic_pg_edges"),
        ("unknown semantic category rejection", "unknown category"),
        ("unknown semantic role rejection", "unknown role"),
        ("inverted source span rejection", "inverted source span"),
        ("semantic parent bounds rejection", "parent"),
        ("semantic child bounds rejection", "outside"),
        ("single root invariant", "expected exactly one"),
        (
            "empty node section rejection",
            "semantic node section is empty",
        ),
        (
            "edge node bounds rejection",
            "outside {node_count} decoded semantic nodes",
        ),
        ("actionable fix messages", "Fix:"),
    ] {
        source_contains(semantic_graph_source, evidence, needle)?;
    }

    let assertion_marker_count = object_decode_tests_source.matches("assert").count();
    if assertion_marker_count < 20 {
        return Err(ParserSemanticSafetyError::SourceThresholdMiss {
            field: "assertion markers",
            observed: assertion_marker_count,
            required: 20,
        });
    }

    Ok(ParserSemanticSourceSafetyProof {
        assertion_marker_count,
    })
}

fn source_contains(
    source: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), ParserSemanticSafetyError> {
    if source.contains(needle) {
        Ok(())
    } else {
        Err(ParserSemanticSafetyError::SourceMissingEvidence { evidence })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_semantic_safety_accepts_required_cases() {
        let proof = validate_parser_semantic_safety(&parser_records(), &semantic_records())
            .expect("Fix: complete parser/semantic safety should pass");

        assert_eq!(proof.parser_case_count, 3);
        assert_eq!(proof.semantic_case_count, 4);
    }

    #[test]
    fn parser_semantic_safety_rejects_missing_no_fake_ast_case() {
        let mut parser_records = parser_records();
        parser_records.remove(1);

        assert_eq!(
            validate_parser_semantic_safety(&parser_records, &semantic_records())
                .expect_err("missing parser recovery case should fail"),
            ParserSemanticSafetyError::MissingParserCase {
                case: ParserRecoveryCase::NoFakeAstSuccess,
            }
        );
    }

    #[test]
    fn parser_semantic_safety_rejects_missing_semantic_case_and_raw_cargo() {
        let mut missing_semantic_records = semantic_records();
        missing_semantic_records.pop();
        assert_eq!(
            validate_parser_semantic_safety(&parser_records(), &missing_semantic_records)
                .expect_err("missing semantic invariant should fail"),
            ParserSemanticSafetyError::MissingSemanticCase {
                case: SemanticInvariantCase::ConflictingNamespaceRejected,
            }
        );

        let mut parser_records = parser_records();
        parser_records[0].command = "cargo test";
        assert_eq!(
            validate_parser_semantic_safety(&parser_records, &semantic_records())
                .expect_err("raw cargo should fail"),
            ParserSemanticSafetyError::CommandDoesNotUseCargoFull {
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn parser_semantic_safety_accepts_committed_object_decode_sources() {
        let proof = validate_parser_semantic_source_safety(
            include_str!("../../../../vyre-frontend-c/src/api/object_decode/tests/mod.rs"),
            include_str!("../../../../vyre-frontend-c/src/api/object_decode/semantic_graph.rs"),
        )
        .expect("Fix: committed parser/semantic source safety evidence should pass");

        assert!(proof.assertion_marker_count >= 20);
    }

    #[test]
    fn parser_semantic_safety_rejects_missing_scope_rejection_source() {
        let tests_source = r#"
            fn decode_object_semantic_graph_rejects_out_of_range_tree_links() { expect_err("Fix:"); }
            fn decode_object_semantic_graph_rejects_out_of_range_edges() { expect_err("Fix:"); }
            fn decode_object_semantic_graph_rejects_unknown_node_role() { expect_err("Fix:"); }
            fn decode_object_semantic_graph_rejects_empty_node_section() { expect_err("Fix:"); }
            fn decode_object_sema_scope_rejects_multiple_roots() { expect_err("Fix:"); }
            fn decode_object_sema_scope_rejects_unknown_decl_kind() { expect_err("Fix:"); }
        "#;
        let graph_source = r#"
            fn validate_semantic_pg_nodes() {
                "unknown category"; "unknown role"; "inverted source span"; "parent"; "outside";
                "expected exactly one"; "semantic node section is empty"; "Fix:";
            }
            fn validate_semantic_pg_edges() { "outside {node_count} decoded semantic nodes"; }
        "#;

        assert_eq!(
            validate_parser_semantic_source_safety(tests_source, graph_source)
                .expect_err("missing parent scope test should fail"),
            ParserSemanticSafetyError::SourceMissingEvidence {
                evidence: "missing parent scope test",
            }
        );
    }

    fn parser_records() -> Vec<ParserRecoveryRecord<'static>> {
        REQUIRED_PARSER_CASES
            .iter()
            .copied()
            .map(parser_record)
            .collect()
    }

    fn semantic_records() -> Vec<SemanticInvariantRecord<'static>> {
        REQUIRED_SEMANTIC_CASES
            .iter()
            .copied()
            .map(semantic_record)
            .collect()
    }

    fn parser_record(case: ParserRecoveryCase) -> ParserRecoveryRecord<'static> {
        ParserRecoveryRecord {
            case,
            command: "./cargo_full test -j1 -p vyrec",
            evidence: "release/parity/vyrec-parser-recovery.md",
        }
    }

    fn semantic_record(case: SemanticInvariantCase) -> SemanticInvariantRecord<'static> {
        SemanticInvariantRecord {
            case,
            command: "./cargo_full test -j1 -p vyrec",
            evidence: "release/parity/vyrec-semantic-invariants.md",
        }
    }
}

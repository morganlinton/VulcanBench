//! Vyrec C dialect matrix validation.

use std::collections::BTreeSet;

/// Frontend phase tracked against clang.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CDialectPhase {
    /// C preprocessing.
    Preprocessing,
    /// Lexing and token classification.
    Lexing,
    /// Parsing.
    Parsing,
    /// Semantic analysis.
    SemanticAnalysis,
    /// Diagnostics.
    Diagnostics,
    /// Lowering/codegen steps intentionally outside this beta release.
    UnsupportedLowerStep,
}

/// Phase compatibility state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CDialectSupport {
    /// Compatible with the declared clang scope.
    Compatible,
    /// Partially compatible with explicit gap evidence.
    Partial,
    /// Unsupported in this release.
    Unsupported,
}

/// One dialect matrix row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CDialectMatrixRow<'a> {
    /// Phase.
    pub phase: CDialectPhase,
    /// C dialect label, e.g. C11/GNU11.
    pub dialect: &'a str,
    /// Support state.
    pub support: CDialectSupport,
    /// Exact test command or release evidence command.
    pub command: &'a str,
    /// Evidence path.
    pub evidence: &'a str,
}

/// Dialect matrix proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CDialectMatrixProof {
    /// Number of rows.
    pub row_count: usize,
    /// Number of unsupported lower-step rows.
    pub unsupported_lower_rows: usize,
}

/// Committed C dialect target manifest proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CDialectTargetManifestProof {
    /// Number of frozen translation-unit source entries detected.
    pub source_count: usize,
}

/// Dialect matrix validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CDialectMatrixError {
    /// Matrix is empty.
    EmptyMatrix,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Phase.
        phase: CDialectPhase,
        /// Field.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Phase.
        phase: CDialectPhase,
        /// Command.
        command: String,
    },
    /// Required phase is missing.
    MissingPhase {
        /// Missing phase.
        phase: CDialectPhase,
    },
    /// Parser or semantic phase is marked unsupported instead of exposed as partial/failing evidence.
    HidesParserOrSemanticGap {
        /// Phase.
        phase: CDialectPhase,
    },
    /// A lower-step row is not marked unsupported.
    LowerStepNotUnsupported,
    /// Committed C dialect target manifest is missing required evidence.
    ManifestMissingEvidence {
        /// Missing evidence field.
        evidence: &'static str,
    },
    /// Committed C dialect target manifest does not meet a threshold.
    ManifestThresholdMiss {
        /// Field name.
        field: &'static str,
        /// Observed value.
        observed: usize,
        /// Required value.
        required: usize,
    },
}

impl std::fmt::Display for CDialectMatrixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMatrix => write!(
                f,
                "C dialect matrix is empty. Fix: list preprocessing, lexing, parsing, semantic analysis, diagnostics, and unsupported lower steps."
            ),
            Self::EmptyMetadata { phase, field } => write!(
                f,
                "C dialect matrix row {phase:?} has empty {field}. Fix: every row needs dialect, command, and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { phase, command } => write!(
                f,
                "C dialect matrix row {phase:?} uses `{command}` instead of ./cargo_full. Fix: make dialect evidence reproducible through cargo_full."
            ),
            Self::MissingPhase { phase } => write!(
                f,
                "C dialect matrix is missing {phase:?}. Fix: document every pre-lowering phase and unsupported lower steps."
            ),
            Self::HidesParserOrSemanticGap { phase } => write!(
                f,
                "C dialect matrix marks {phase:?} unsupported. Fix: parser and semantic gaps must be partial/failing evidence, not beta limitations."
            ),
            Self::LowerStepNotUnsupported => write!(
                f,
                "C dialect matrix lower-step row is not unsupported. Fix: this beta release may only mark lower steps unsupported."
            ),
            Self::ManifestMissingEvidence { evidence } => write!(
                f,
                "C dialect target manifest is missing {evidence}. Fix: keep the frozen clang parity target explicit and GPU-first."
            ),
            Self::ManifestThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "C dialect target manifest {field}={observed} missed required {required}. Fix: restore the full frozen Linux subsystem target coverage."
            ),
        }
    }
}

impl std::error::Error for CDialectMatrixError {}

const REQUIRED_PHASES: &[CDialectPhase] = &[
    CDialectPhase::Preprocessing,
    CDialectPhase::Lexing,
    CDialectPhase::Parsing,
    CDialectPhase::SemanticAnalysis,
    CDialectPhase::Diagnostics,
    CDialectPhase::UnsupportedLowerStep,
];

/// Validate Vyrec C dialect matrix evidence.
pub fn validate_c_dialect_matrix(
    rows: &[CDialectMatrixRow<'_>],
) -> Result<CDialectMatrixProof, CDialectMatrixError> {
    if rows.is_empty() {
        return Err(CDialectMatrixError::EmptyMatrix);
    }

    let mut phases = BTreeSet::new();
    let mut unsupported_lower_rows = 0_usize;
    for row in rows {
        for (field, value) in [
            ("dialect", row.dialect),
            ("command", row.command),
            ("evidence", row.evidence),
        ] {
            if value.trim().is_empty() {
                return Err(CDialectMatrixError::EmptyMetadata {
                    phase: row.phase,
                    field,
                });
            }
        }
        if !row.command.trim_start().starts_with("./cargo_full ") {
            return Err(CDialectMatrixError::CommandDoesNotUseCargoFull {
                phase: row.phase,
                command: row.command.to_owned(),
            });
        }
        if matches!(
            row.phase,
            CDialectPhase::Parsing | CDialectPhase::SemanticAnalysis
        ) && row.support == CDialectSupport::Unsupported
        {
            return Err(CDialectMatrixError::HidesParserOrSemanticGap { phase: row.phase });
        }
        if row.phase == CDialectPhase::UnsupportedLowerStep {
            if row.support != CDialectSupport::Unsupported {
                return Err(CDialectMatrixError::LowerStepNotUnsupported);
            }
            unsupported_lower_rows += 1;
        }
        phases.insert(row.phase);
    }

    for phase in REQUIRED_PHASES {
        if !phases.contains(phase) {
            return Err(CDialectMatrixError::MissingPhase { phase: *phase });
        }
    }

    Ok(CDialectMatrixProof {
        row_count: rows.len(),
        unsupported_lower_rows,
    })
}

/// Validate the committed frozen C dialect target manifest source.
pub fn validate_c_dialect_target_manifest(
    manifest: &str,
) -> Result<CDialectTargetManifestProof, CDialectMatrixError> {
    for (evidence, needle) in [
        ("schema", "schema = \"vyrec.parity.target.v1\""),
        ("Linux lib/math target id", "id = \"linux-lib-math-v6.8\""),
        (
            "frozen Linux commit",
            "commit = \"90d1f30371ae3337beb01666b226320728d35c70\"",
        ),
        ("subsystem root", "subsystem_root = \"lib/math\""),
        ("GNU11 language", "language = \"gnu11\""),
        ("lowering excluded", "lowering = false"),
        ("pre-lowering required", "pre_lowering_required = true"),
        (
            "semantic analysis parity scope",
            "clang_parity_through = \"semantic-analysis\"",
        ),
        (
            "CPU oracle-only execution",
            "cpu_execution_allowed = \"oracle-only\"",
        ),
        ("GPU execution required", "gpu_execution_required = true"),
        (
            "zero silent CPU demotions",
            "zero_silent_cpu_demotions = true",
        ),
        ("zero false no-GPU skips", "zero_false_no_gpu_skips = true"),
        (
            "resident GPU frontend gate",
            "resident_gpu_frontend_required = true",
        ),
        ("clang baseline required", "clang_baseline_required = true"),
        ("preprocessor proof category", "preprocessor = ["),
        ("lexer proof category", "lexer = ["),
        ("parser proof category", "parser = ["),
        ("semantic analysis proof category", "semantic_analysis = ["),
        ("ABI layout proof category", "abi_layout = ["),
        ("performance proof category", "performance = ["),
    ] {
        manifest_contains(manifest, evidence, needle)?;
    }

    let source_count = manifest.matches("lib/math/").filter(|_| true).count();
    if source_count < 12 {
        return Err(CDialectMatrixError::ManifestThresholdMiss {
            field: "lib/math source/header mentions",
            observed: source_count,
            required: 12,
        });
    }

    Ok(CDialectTargetManifestProof { source_count })
}

fn manifest_contains(
    manifest: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), CDialectMatrixError> {
    if manifest.contains(needle) {
        Ok(())
    } else {
        Err(CDialectMatrixError::ManifestMissingEvidence { evidence })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_matrix_accepts_all_phases_and_lower_step_scope() {
        let proof =
            validate_c_dialect_matrix(&rows()).expect("Fix: complete dialect matrix should pass");

        assert_eq!(proof.row_count, 6);
        assert_eq!(proof.unsupported_lower_rows, 1);
    }

    #[test]
    fn dialect_matrix_rejects_hidden_semantic_gap() {
        let mut rows = rows();
        rows[3].support = CDialectSupport::Unsupported;

        assert_eq!(
            validate_c_dialect_matrix(&rows).expect_err("semantic unsupported should fail"),
            CDialectMatrixError::HidesParserOrSemanticGap {
                phase: CDialectPhase::SemanticAnalysis,
            }
        );
    }

    #[test]
    fn dialect_matrix_rejects_raw_cargo_and_missing_phase() {
        let mut raw_cargo_rows = rows();
        raw_cargo_rows[0].command = "cargo test";
        assert_eq!(
            validate_c_dialect_matrix(&raw_cargo_rows).expect_err("raw cargo should fail"),
            CDialectMatrixError::CommandDoesNotUseCargoFull {
                phase: CDialectPhase::Preprocessing,
                command: "cargo test".to_owned(),
            }
        );

        let mut missing_phase_rows = rows();
        missing_phase_rows.pop();
        assert_eq!(
            validate_c_dialect_matrix(&missing_phase_rows)
                .expect_err("missing lower step should fail"),
            CDialectMatrixError::MissingPhase {
                phase: CDialectPhase::UnsupportedLowerStep,
            }
        );
    }

    #[test]
    fn dialect_matrix_accepts_committed_linux_math_target_manifest() {
        let proof = validate_c_dialect_target_manifest(include_str!(
            "../../../../vyre-frontend-c/parity/linux_math_v6_8.toml"
        ))
        .expect("Fix: committed Linux lib/math target manifest should pass");

        assert!(proof.source_count >= 12);
    }

    #[test]
    fn dialect_matrix_rejects_cpu_execution_manifest() {
        let manifest = r#"
            schema = "vyrec.parity.target.v1"
            id = "linux-lib-math-v6.8"
            commit = "90d1f30371ae3337beb01666b226320728d35c70"
            subsystem_root = "lib/math"
            language = "gnu11"
            lowering = false
            pre_lowering_required = true
            clang_parity_through = "semantic-analysis"
            cpu_execution_allowed = "production"
            gpu_execution_required = true
            zero_silent_cpu_demotions = true
            zero_false_no_gpu_skips = true
            resident_gpu_frontend_required = true
            clang_baseline_required = true
            preprocessor = []
            lexer = []
            parser = []
            semantic_analysis = []
            abi_layout = []
            performance = []
        "#;

        assert_eq!(
            validate_c_dialect_target_manifest(manifest)
                .expect_err("production CPU execution should fail"),
            CDialectMatrixError::ManifestMissingEvidence {
                evidence: "CPU oracle-only execution",
            }
        );
    }

    fn rows() -> Vec<CDialectMatrixRow<'static>> {
        vec![
            row(CDialectPhase::Preprocessing, CDialectSupport::Partial),
            row(CDialectPhase::Lexing, CDialectSupport::Compatible),
            row(CDialectPhase::Parsing, CDialectSupport::Partial),
            row(CDialectPhase::SemanticAnalysis, CDialectSupport::Partial),
            row(CDialectPhase::Diagnostics, CDialectSupport::Compatible),
            row(
                CDialectPhase::UnsupportedLowerStep,
                CDialectSupport::Unsupported,
            ),
        ]
    }

    fn row(phase: CDialectPhase, support: CDialectSupport) -> CDialectMatrixRow<'static> {
        CDialectMatrixRow {
            phase,
            dialect: "gnu11",
            support,
            command: "./cargo_full test -j1 -p vyrec",
            evidence: "release/parity/vyrec-c-dialect-matrix.md",
        }
    }
}

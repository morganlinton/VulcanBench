//! Linux corpus clang-parity evidence validation.

use std::collections::BTreeSet;

/// One Linux subsystem corpus slice used for clang parity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxCorpusParitySlice<'a> {
    /// Stable slice id.
    pub id: &'a str,
    /// Linux subsystem name.
    pub subsystem: &'a str,
    /// Source/header fixture path.
    pub fixture_path: &'a str,
    /// Provenance for the fixture, e.g. commit hash and source path.
    pub provenance: &'a str,
    /// Exact clang command used as oracle.
    pub clang_command: &'a str,
    /// Exact cargo_full command used for Vyrec.
    pub vyrec_command: &'a str,
    /// Minimized reproducer path for any mismatch.
    pub minimized_reproducer: &'a str,
}

/// Linux corpus parity proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxCorpusParityProof {
    /// Number of corpus slices.
    pub slice_count: usize,
    /// Number of distinct subsystems.
    pub subsystem_count: usize,
}

/// Validated committed Linux corpus artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxCorpusArtifactProof {
    /// C files in the Linux subsystem corpus.
    pub file_count: u64,
    /// Source bytes in the Linux subsystem corpus.
    pub total_source_bytes: u64,
}

/// Validated C parser proof-document alignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CParserLinuxProofDocProof {
    /// Number of proof-document contract clauses checked.
    pub clause_count: usize,
}

/// Linux corpus parity validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinuxCorpusParityError {
    /// No slices supplied.
    EmptySlices,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Slice id.
        id: String,
        /// Field.
        field: &'static str,
    },
    /// Duplicate slice id.
    DuplicateSlice {
        /// Slice id.
        id: String,
    },
    /// Clang oracle command is not explicit.
    MissingClangOracle {
        /// Slice id.
        id: String,
        /// Command.
        command: String,
    },
    /// Vyrec command does not use cargo_full.
    VyrecCommandDoesNotUseCargoFull {
        /// Slice id.
        id: String,
        /// Command.
        command: String,
    },
    /// Provenance does not include a commit-like reference.
    MissingCommitProvenance {
        /// Slice id.
        id: String,
    },
    /// Committed Linux corpus artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed Linux corpus artifact numeric field is missing.
    ArtifactMissingNumber {
        /// Missing field.
        field: &'static str,
    },
    /// Committed Linux corpus artifact threshold is missed.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
}

impl std::fmt::Display for LinuxCorpusParityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySlices => write!(
                f,
                "Linux corpus parity slices are empty. Fix: add real Linux subsystem fixtures with clang and Vyrec commands."
            ),
            Self::EmptyMetadata { id, field } => write!(
                f,
                "Linux corpus parity slice `{id}` has empty {field}. Fix: record subsystem, fixture, provenance, commands, and minimizer output."
            ),
            Self::DuplicateSlice { id } => write!(
                f,
                "Linux corpus parity slice `{id}` is duplicated. Fix: keep one provenance owner per fixture slice."
            ),
            Self::MissingClangOracle { id, command } => write!(
                f,
                "Linux corpus parity slice `{id}` clang command `{command}` is not explicit. Fix: record the exact clang oracle invocation."
            ),
            Self::VyrecCommandDoesNotUseCargoFull { id, command } => write!(
                f,
                "Linux corpus parity slice `{id}` Vyrec command `{command}` does not use ./cargo_full. Fix: run Vyrec parity through cargo_full."
            ),
            Self::MissingCommitProvenance { id } => write!(
                f,
                "Linux corpus parity slice `{id}` lacks commit provenance. Fix: record Linux commit hash and original source path."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "Linux corpus parity artifact is missing {evidence}. Fix: commit Linux subsystem manifest, diagnostics, throughput, and CUDA parser evidence."
            ),
            Self::ArtifactMissingNumber { field } => write!(
                f,
                "Linux corpus parity artifact has no numeric {field}. Fix: record exact corpus and parser counters."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "Linux corpus parity artifact {field}={observed} missed required {required}. Fix: rerun the Linux subsystem corpus gate or repair parser coverage."
            ),
        }
    }
}

impl std::error::Error for LinuxCorpusParityError {}

/// Validate Linux corpus parity fixture evidence.
pub fn validate_linux_corpus_parity(
    slices: &[LinuxCorpusParitySlice<'_>],
) -> Result<LinuxCorpusParityProof, LinuxCorpusParityError> {
    if slices.is_empty() {
        return Err(LinuxCorpusParityError::EmptySlices);
    }
    let mut ids = BTreeSet::new();
    let mut subsystems = BTreeSet::new();
    for slice in slices {
        for (field, value) in [
            ("id", slice.id),
            ("subsystem", slice.subsystem),
            ("fixture_path", slice.fixture_path),
            ("provenance", slice.provenance),
            ("clang_command", slice.clang_command),
            ("vyrec_command", slice.vyrec_command),
            ("minimized_reproducer", slice.minimized_reproducer),
        ] {
            if value.trim().is_empty() {
                return Err(LinuxCorpusParityError::EmptyMetadata {
                    id: slice.id.to_owned(),
                    field,
                });
            }
        }
        if !ids.insert(slice.id) {
            return Err(LinuxCorpusParityError::DuplicateSlice {
                id: slice.id.to_owned(),
            });
        }
        if !slice.clang_command.contains("clang") {
            return Err(LinuxCorpusParityError::MissingClangOracle {
                id: slice.id.to_owned(),
                command: slice.clang_command.to_owned(),
            });
        }
        if !slice
            .vyrec_command
            .trim_start()
            .starts_with("./cargo_full ")
        {
            return Err(LinuxCorpusParityError::VyrecCommandDoesNotUseCargoFull {
                id: slice.id.to_owned(),
                command: slice.vyrec_command.to_owned(),
            });
        }
        if !has_commit_like_provenance(slice.provenance) {
            return Err(LinuxCorpusParityError::MissingCommitProvenance {
                id: slice.id.to_owned(),
            });
        }
        subsystems.insert(slice.subsystem);
    }

    Ok(LinuxCorpusParityProof {
        slice_count: slices.len(),
        subsystem_count: subsystems.len(),
    })
}

/// Validate committed Linux subsystem corpus, diagnostics, and CUDA parser evidence.
pub fn validate_committed_linux_corpus_artifacts(
    manifest: &str,
    diagnostics: &str,
    throughput: &str,
    full_corpus: &str,
) -> Result<LinuxCorpusArtifactProof, LinuxCorpusParityError> {
    for (artifact, evidence, needle) in [
        (manifest, "manifest schema", "\"schema_version\": 1"),
        (
            manifest,
            "Linux subsystem marker",
            "\"linux_subsystem_candidate\": true",
        ),
        (
            manifest,
            "Linux lib subsystem",
            "\"linux_subsystem\": \"lib\"",
        ),
        (manifest, "Linux root", "\"linux_root\""),
        (
            manifest,
            "Kconfig provenance",
            "\"linux_kbuild_file_in_corpus\": true",
        ),
        (
            manifest,
            "deterministic file list hash",
            "\"deterministic_file_list_sha256\"",
        ),
        (
            manifest,
            "recursive source collection",
            "\"source_collection_mode\": \"recursive_all_c_files\"",
        ),
        (manifest, "include directory evidence", "\"include_dirs\""),
        (manifest, "kernel macro evidence", "\"__KERNEL__=1\""),
        (diagnostics, "diagnostics schema", "\"schema_version\": 1"),
        (diagnostics, "zero failed files", "\"failed_files\": 0"),
        (diagnostics, "empty diagnostic failures", "\"failures\": []"),
        (throughput, "throughput schema", "\"schema_version\": 1"),
        (
            throughput,
            "throughput Linux subsystem",
            "\"linux_subsystem\": \"lib\"",
        ),
        (
            throughput,
            "throughput CUDA backend",
            "\"resident_vyre_parse_backend_id\": \"cuda\"",
        ),
        (
            throughput,
            "raw GPU syntax input",
            "\"resident_vyre_parse_input_mode\": \"raw_bytes_gpu_syntax\"",
        ),
        (
            throughput,
            "pipeline cache enabled",
            "\"resident_vyre_pipeline_cache_enabled\": true",
        ),
        (
            throughput,
            "zero host token upload",
            "\"resident_vyre_parse_host_token_stream_upload_bytes\": 0",
        ),
        (full_corpus, "full corpus blocker list", "\"blockers\": []"),
        (full_corpus, "full corpus failures list", "\"failures\": []"),
        (
            full_corpus,
            "full corpus CUDA backend",
            "\"resident_vyre_parse_backend_id\": \"cuda\"",
        ),
    ] {
        artifact_contains(artifact, evidence, needle)?;
    }

    let manifest_files = number_field(manifest, "file_count")?;
    let throughput_files = number_field(throughput, "total_files")?;
    let parsed_files = number_field(throughput, "parsed_files")?;
    let full_total_files = number_field(full_corpus, "total_files")?;
    let full_parsed_files = number_field(full_corpus, "parsed_files")?;
    let total_source_bytes = number_field(manifest, "total_source_bytes")?;
    let throughput_source_bytes = number_field(throughput, "total_source_bytes")?;
    let full_source_bytes = number_field(full_corpus, "total_source_bytes")?;
    let gpu_dispatch_count = number_field(throughput, "resident_vyre_parse_gpu_dispatch_count")?;
    let speedup_x1000 = number_field(throughput, "resident_vyre_vs_tree_sitter_speedup_x1000")?;

    require_at_least("file_count", manifest_files, 250)?;
    require_at_least("total_source_bytes", total_source_bytes, 4 * 1024 * 1024)?;
    require_at_least(
        "resident_vyre_parse_gpu_dispatch_count",
        gpu_dispatch_count,
        1,
    )?;
    require_at_least(
        "resident_vyre_vs_tree_sitter_speedup_x1000",
        speedup_x1000,
        100_000,
    )?;
    require_equal("throughput total_files", throughput_files, manifest_files)?;
    require_equal("throughput parsed_files", parsed_files, manifest_files)?;
    require_equal("full corpus total_files", full_total_files, manifest_files)?;
    require_equal(
        "full corpus parsed_files",
        full_parsed_files,
        manifest_files,
    )?;
    require_equal(
        "throughput total_source_bytes",
        throughput_source_bytes,
        total_source_bytes,
    )?;
    require_equal(
        "full corpus total_source_bytes",
        full_source_bytes,
        total_source_bytes,
    )?;

    Ok(LinuxCorpusArtifactProof {
        file_count: manifest_files,
        total_source_bytes,
    })
}

/// Validate the C parser proof doc matches the committed Linux `lib` subsystem evidence.
pub fn validate_c_parser_linux_proof_doc(
    proof_doc: &str,
) -> Result<CParserLinuxProofDocProof, LinuxCorpusParityError> {
    for (evidence, needle) in [
        (
            "C parser Linux proof title",
            "# C parser Linux subsystem proof",
        ),
        (
            "c-parser-linux-subsystem artifact",
            "release/evidence/parser/c-parser-linux-subsystem.json",
        ),
        ("Linux lib allowed subsystem", "`lib`"),
        (
            "manifest/corpus alignment",
            "linux-subsystem-corpus-manifest.json",
        ),
        ("diagnostics alignment", "c-parser-diagnostics-summary.json"),
        ("throughput alignment", "c-parser-throughput.json"),
        ("full corpus floor", "250"),
        ("source byte floor", "4194304"),
        ("parsed equals total", "Parsed files must equal total files"),
        ("zero failed files", "failed files must be zero"),
        ("semantic graph bytes", "semantic_graph_bytes"),
    ] {
        artifact_contains(proof_doc, evidence, needle)?;
    }

    Ok(CParserLinuxProofDocProof { clause_count: 11 })
}

fn has_commit_like_provenance(provenance: &str) -> bool {
    provenance
        .split(|ch: char| !ch.is_ascii_hexdigit())
        .any(|part| part.len() >= 7)
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), LinuxCorpusParityError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(LinuxCorpusParityError::ArtifactMissingEvidence { evidence })
    }
}

fn require_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), LinuxCorpusParityError> {
    if observed >= required {
        Ok(())
    } else {
        Err(LinuxCorpusParityError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn require_equal(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), LinuxCorpusParityError> {
    if observed == required {
        Ok(())
    } else {
        Err(LinuxCorpusParityError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn number_field(artifact: &str, field: &'static str) -> Result<u64, LinuxCorpusParityError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(LinuxCorpusParityError::ArtifactMissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(LinuxCorpusParityError::ArtifactMissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(LinuxCorpusParityError::ArtifactMissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| LinuxCorpusParityError::ArtifactMissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_corpus_parity_accepts_provenanced_clang_slices() {
        let proof = validate_linux_corpus_parity(&[
            slice(
                "sched-core",
                "kernel/sched",
                "abcdef123456 kernel/sched/core.c",
            ),
            slice("mm-vmalloc", "mm", "1234567abcdef mm/vmalloc.c"),
        ])
        .expect("Fix: valid Linux corpus slices should pass");

        assert_eq!(proof.slice_count, 2);
        assert_eq!(proof.subsystem_count, 2);
    }

    #[test]
    fn linux_corpus_parity_rejects_missing_clang_and_raw_cargo() {
        let mut no_clang = slice(
            "sched-core",
            "kernel/sched",
            "abcdef123456 kernel/sched/core.c",
        );
        no_clang.clang_command = "cc -E kernel/sched/core.c";
        assert_eq!(
            validate_linux_corpus_parity(&[no_clang]).expect_err("missing clang should fail"),
            LinuxCorpusParityError::MissingClangOracle {
                id: "sched-core".to_owned(),
                command: "cc -E kernel/sched/core.c".to_owned(),
            }
        );

        let mut raw_cargo = slice(
            "sched-core",
            "kernel/sched",
            "abcdef123456 kernel/sched/core.c",
        );
        raw_cargo.vyrec_command = "cargo test -p vyrec";
        assert_eq!(
            validate_linux_corpus_parity(&[raw_cargo]).expect_err("raw cargo should fail"),
            LinuxCorpusParityError::VyrecCommandDoesNotUseCargoFull {
                id: "sched-core".to_owned(),
                command: "cargo test -p vyrec".to_owned(),
            }
        );
    }

    #[test]
    fn linux_corpus_parity_rejects_missing_commit_provenance() {
        assert_eq!(
            validate_linux_corpus_parity(&[slice(
                "sched-core",
                "kernel/sched",
                "kernel/sched/core.c"
            )])
            .expect_err("missing commit provenance should fail"),
            LinuxCorpusParityError::MissingCommitProvenance {
                id: "sched-core".to_owned(),
            }
        );
    }

    #[test]
    fn linux_corpus_parity_accepts_committed_linux_subsystem_artifacts() {
        let proof = validate_committed_linux_corpus_artifacts(
            include_str!(
                "../../../../release/evidence/parser/linux-subsystem-corpus-manifest.json"
            ),
            include_str!("../../../../release/evidence/parser/c-parser-diagnostics-summary.json"),
            include_str!("../../../../release/evidence/parser/c-parser-throughput.json"),
            include_str!("../../../../release/evidence/parser/c-parser-linux-subsystem.json"),
        )
        .expect("Fix: committed Linux subsystem parser artifacts should pass");

        assert_eq!(proof.file_count, 490);
        assert!(proof.total_source_bytes >= 4 * 1024 * 1024);
    }

    #[test]
    fn linux_corpus_parity_accepts_committed_proof_doc_alignment() {
        let proof = validate_c_parser_linux_proof_doc(include_str!(
            "../../../../release/evidence/docs/c-parser-linux-proof.md"
        ))
        .expect("Fix: C parser proof doc should match committed Linux lib corpus evidence");

        assert_eq!(proof.clause_count, 11);
    }

    #[test]
    fn linux_corpus_parity_rejects_proof_doc_missing_lib_subsystem() {
        let doc = "# C parser Linux subsystem proof\n\
release/evidence/parser/c-parser-linux-subsystem.json\n\
linux-subsystem-corpus-manifest.json\n\
c-parser-diagnostics-summary.json\n\
c-parser-throughput.json\n\
250\n\
4194304\n\
Parsed files must equal total files and failed files must be zero.\n\
semantic_graph_bytes\n\
linux_subsystem must be one of `kernel`, `fs`, `mm`, `net`, or `drivers`.";

        assert_eq!(
            validate_c_parser_linux_proof_doc(doc)
                .expect_err("proof doc must allow committed Linux lib corpus"),
            LinuxCorpusParityError::ArtifactMissingEvidence {
                evidence: "Linux lib allowed subsystem",
            }
        );
    }

    #[test]
    fn linux_corpus_parity_rejects_partial_committed_artifacts() {
        let err = validate_committed_linux_corpus_artifacts(
            r#"{"schema_version": 1,"linux_subsystem_candidate": true,"linux_subsystem": "lib","linux_root":"linux","linux_kbuild_file_in_corpus": true,"deterministic_file_list_sha256":"x","source_collection_mode": "recursive_all_c_files","include_dirs":[],"macros":["__KERNEL__=1"],"file_count":12,"total_source_bytes":1024}"#,
            include_str!("../../../../release/evidence/parser/c-parser-diagnostics-summary.json"),
            include_str!("../../../../release/evidence/parser/c-parser-throughput.json"),
            include_str!("../../../../release/evidence/parser/c-parser-linux-subsystem.json"),
        )
        .expect_err("partial Linux corpus manifest should fail");

        assert_eq!(
            err,
            LinuxCorpusParityError::ArtifactThresholdMiss {
                field: "file_count",
                observed: 12,
                required: 250,
            }
        );
    }

    fn slice(
        id: &'static str,
        subsystem: &'static str,
        provenance: &'static str,
    ) -> LinuxCorpusParitySlice<'static> {
        LinuxCorpusParitySlice {
            id,
            subsystem,
            fixture_path: "release/corpus/linux/kernel/sched/core.c",
            provenance,
            clang_command: "clang -E -fsyntax-only kernel/sched/core.c",
            vyrec_command: "./cargo_full test -j1 -p vyrec linux_corpus_parity",
            minimized_reproducer: "release/corpus/minimized/sched-core.c",
        }
    }
}

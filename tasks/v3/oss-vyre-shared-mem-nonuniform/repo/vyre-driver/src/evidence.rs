//! Backend-neutral evidence, provenance, and replay metadata.
//!
//! This module is the shared driver-layer contract for source provenance and
//! dispatch evidence. Benchmark reports, conformance artifacts, replay
//! capsules, and consumer APIs should import this surface instead of owning
//! parallel fingerprint or artifact schemas.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use vyre_foundation::ir::Program;
use vyre_foundation::serial::wire::encode::PROGRAM_WIRE_DIGEST_VERSION;

use crate::backend::{BackendError, DispatchConfig, TimedDispatchResult, VyreBackend};
use crate::pipeline::{
    dispatch_policy_cache_digest, dispatch_policy_cache_string, hex_encode,
    try_normalized_program_cache_digest, PipelineReproManifest,
};

/// Version label for the normalized Program digest used by compiled-pipeline
/// caches. The byte contract currently lives in `pipeline::hashing`; evidence
/// records the same label in its digest ledger so cache identity and replay
/// evidence cannot silently drift.
pub const NORMALIZED_PROGRAM_DIGEST_VERSION: &str = "vyre-pipeline-cache-norm-v2";

/// Version label for commit/dirty-state source fingerprints.
pub const SOURCE_FINGERPRINT_VERSION: &str = "vyre-source-fingerprint-v1";
const MAX_SOURCE_FINGERPRINT_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Version label for source-tree content fingerprints.
pub const SOURCE_TREE_FINGERPRINT_VERSION: &str = "source-tree-v1";

/// Version label for dispatch workload/config fingerprints.
pub const WORKLOAD_FINGERPRINT_VERSION: &str = "vyre-dispatch-workload-v1";

/// Version label for backend environment fingerprints.
pub const ENVIRONMENT_FINGERPRINT_VERSION: &str = "vyre-evidence-environment-v1";

/// Git and source-tree provenance for evidence-producing runs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceProvenance {
    /// Raw git facts captured from the source workspace.
    pub git: BTreeMap<String, String>,
    /// Commit/dirty-state source identity used by release evidence gates.
    pub source_fingerprint: String,
    /// Source-tree content identity used to tolerate evidence-only commit drift.
    pub source_tree_fingerprint: String,
}

impl SourceProvenance {
    /// Capture provenance for the current working directory.
    #[must_use]
    pub fn capture_current() -> Self {
        Self::capture_at(Path::new("."))
    }

    /// Capture provenance for `workspace_root`.
    #[must_use]
    pub fn capture_at(workspace_root: &Path) -> Self {
        let git = capture_git_info_at(workspace_root);
        let source_fingerprint = source_fingerprint(&git);
        let source_tree_fingerprint = source_tree_fingerprint_at(workspace_root);
        Self {
            git,
            source_fingerprint,
            source_tree_fingerprint,
        }
    }

    /// Validate that required provenance fields are non-empty and shaped.
    ///
    /// # Errors
    /// Returns [`BackendError::InvalidProgram`] when an evidence producer
    /// attempts to emit a weak source identity.
    pub fn validate(&self) -> Result<(), BackendError> {
        if self.source_fingerprint.trim().is_empty() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: source_fingerprint must be non-empty before emitting driver evidence."
                    .to_string(),
            });
        }
        if self.source_tree_fingerprint.trim().is_empty() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: source_tree_fingerprint must be non-empty before emitting driver evidence."
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// Capture git facts for the current working directory.
#[must_use]
pub fn capture_git_info() -> BTreeMap<String, String> {
    capture_git_info_at(Path::new("."))
}

/// Capture git facts for `workspace_root`.
#[must_use]
pub fn capture_git_info_at(workspace_root: &Path) -> BTreeMap<String, String> {
    let mut info = BTreeMap::new();

    if let Ok(commit) = shell(workspace_root, &["rev-parse", "HEAD"]) {
        info.insert("commit".to_string(), commit);
    }
    if let Ok(branch) = shell(workspace_root, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        info.insert("branch".to_string(), branch);
    }
    let dirty_status = shell_bytes(
        workspace_root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
            ":!release/evidence/**",
        ],
    );
    let dirty = match dirty_status.as_ref() {
        Ok(status) if status.is_empty() => "false",
        Ok(status) => {
            if let Some(fingerprint) = dirty_worktree_fingerprint(workspace_root, status) {
                info.insert("dirty_worktree_fingerprint".to_string(), fingerprint);
            }
            "true"
        }
        Err(_) => "unknown",
    };
    info.insert("dirty".to_string(), dirty.to_string());

    if let Ok(parent) = shell(workspace_root, &["rev-parse", "HEAD^"]) {
        info.insert("parent_commit".to_string(), parent);
    }
    if let Ok(timestamp) = shell(workspace_root, &["log", "-1", "--format=%ct"]) {
        info.insert("commit_timestamp".to_string(), timestamp);
    }

    info
}

/// Build the commit/dirty-state source fingerprint used by release evidence.
#[must_use]
pub fn source_fingerprint(git: &BTreeMap<String, String>) -> String {
    if let Some(commit) = git.get("commit").filter(|commit| !commit.is_empty()) {
        let dirty = git.get("dirty").map(String::as_str).unwrap_or("unknown");
        if dirty == "true" {
            let worktree = git
                .get("dirty_worktree_fingerprint")
                .filter(|fingerprint| !fingerprint.is_empty())
                .map(String::as_str)
                .unwrap_or("unknown");
            return format!("git:{commit}:dirty=true:worktree={worktree}");
        }
        return format!("git:{commit}:dirty={dirty}");
    }
    format!(
        "crate:{}:{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    )
}

/// Capture a source-tree fingerprint for the current working directory.
#[must_use]
pub fn source_tree_fingerprint() -> String {
    source_tree_fingerprint_at(Path::new("."))
}

/// Capture a source-tree fingerprint for `workspace_root`.
#[must_use]
pub fn source_tree_fingerprint_at(workspace_root: &Path) -> String {
    match shell_bytes(
        workspace_root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    ) {
        Ok(paths) => format!(
            "source-tree-v1:{}",
            source_tree_fingerprint_from_paths(workspace_root, &paths)
        ),
        Err(_) => format!(
            "crate-source:{}:{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ),
    }
}

fn source_tree_fingerprint_from_paths(workspace_root: &Path, paths: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    update_hash_field(&mut hasher, b"format", b"vyre-bench-source-tree-v1");
    for path in paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter(|path| !source_tree_path_is_benchmark_provenance_ignored(path))
    {
        update_hash_field(&mut hasher, b"path", path);
        let path = String::from_utf8_lossy(path);
        match read_source_fingerprint_file_bounded(&workspace_root.join(path.as_ref())) {
            Ok(Some(bytes)) => update_hash_field(&mut hasher, b"content", &bytes),
            Ok(None) => update_hash_field(
                &mut hasher,
                b"content-oversized",
                MAX_SOURCE_FINGERPRINT_FILE_BYTES.to_string().as_bytes(),
            ),
            Err(error) => {
                update_hash_field(&mut hasher, b"read-error", error.to_string().as_bytes())
            }
        }
    }
    hasher.finalize().to_hex().to_string()
}

fn source_tree_path_is_benchmark_provenance_ignored(path: &[u8]) -> bool {
    path == b"cargo_full"
        || path.starts_with(b".github/")
        || path.starts_with(b"release/evidence/")
        || path.starts_with(b"scripts/")
        || path.starts_with(b"xtask/")
        || source_tree_path_is_test_evidence(path)
}

fn source_tree_path_is_test_evidence(path: &[u8]) -> bool {
    path.starts_with(b"tests/")
        || path_contains(path, b"/tests/")
        || path.ends_with(b"/tests.rs")
        || path.ends_with(b"_tests.rs")
        || path.ends_with(b"_test.rs")
        || path_contains(path, b"_tests_")
        || path_contains(path, b"_test_")
}

fn path_contains(path: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && path.windows(needle.len()).any(|window| window == needle)
}

fn dirty_worktree_fingerprint(workspace_root: &Path, status: &[u8]) -> Option<String> {
    let diff = shell_bytes(
        workspace_root,
        &[
            "diff",
            "--binary",
            "HEAD",
            "--",
            ".",
            ":!release/evidence/**",
        ],
    )
    .ok()?;
    let untracked = shell_bytes(
        workspace_root,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            ".",
            ":!release/evidence/**",
        ],
    )
    .unwrap_or_default();
    Some(dirty_worktree_fingerprint_from_parts(
        workspace_root,
        status,
        &diff,
        &untracked,
    ))
}

fn dirty_worktree_fingerprint_from_parts(
    workspace_root: &Path,
    status: &[u8],
    diff: &[u8],
    untracked: &[u8],
) -> String {
    let mut hasher = blake3::Hasher::new();
    update_hash_field(&mut hasher, b"format", b"vyre-bench-dirty-source-v1");
    update_hash_field(&mut hasher, b"status", status);
    update_hash_field(&mut hasher, b"diff", diff);
    for path in untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        update_hash_field(&mut hasher, b"untracked-path", path);
        let path = String::from_utf8_lossy(path);
        match read_source_fingerprint_file_bounded(&workspace_root.join(path.as_ref())) {
            Ok(Some(bytes)) => update_hash_field(&mut hasher, b"untracked-content", &bytes),
            Ok(None) => update_hash_field(
                &mut hasher,
                b"untracked-content-oversized",
                MAX_SOURCE_FINGERPRINT_FILE_BYTES.to_string().as_bytes(),
            ),
            Err(_) => {}
        }
    }
    hasher.finalize().to_hex().to_string()
}

fn read_source_fingerprint_file_bounded(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    let mut reader = fs::File::open(path)?;
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            return Ok(Some(bytes));
        }
        let read = read as u64;
        total = total.saturating_add(read);
        if total > MAX_SOURCE_FINGERPRINT_FILE_BYTES {
            return Ok(None);
        }
        bytes.extend_from_slice(&chunk[..read as usize]);
    }
}

fn update_hash_field(hasher: &mut blake3::Hasher, label: &[u8], value: &[u8]) {
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn digest_to_hex(digest: [u8; 32]) -> String {
    hex_encode(&digest)
}

fn evidence_environment_digest(backend_id: &str, backend_version: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    update_hash_field(
        &mut hasher,
        b"format",
        ENVIRONMENT_FINGERPRINT_VERSION.as_bytes(),
    );
    update_hash_field(&mut hasher, b"backend-id", backend_id.as_bytes());
    update_hash_field(&mut hasher, b"backend-version", backend_version.as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn shell(workspace_root: &Path, args: &[&str]) -> Result<String, String> {
    let stdout = shell_bytes(workspace_root, args)?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_string())
}

fn shell_bytes(workspace_root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace_root)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Timing evidence normalized across host and device timing sources.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DispatchTimingEvidence {
    /// Host-observed dispatch duration.
    pub wall_ns: Option<u64>,
    /// Device-observed elapsed time when available.
    pub device_ns: Option<u64>,
    /// Host enqueue duration when available.
    pub enqueue_ns: Option<u64>,
    /// Host wait/readback duration when available.
    pub wait_ns: Option<u64>,
}

impl DispatchTimingEvidence {
    /// Build timing evidence from a timed dispatch result.
    #[must_use]
    pub fn from_timed_dispatch(result: &TimedDispatchResult) -> Self {
        Self {
            wall_ns: Some(result.wall_ns),
            device_ns: result.device_ns,
            enqueue_ns: result.enqueue_ns,
            wait_ns: result.wait_ns,
        }
    }

    /// Return true when the evidence has at least one timing source.
    #[must_use]
    pub fn has_timing(&self) -> bool {
        self.wall_ns.is_some()
            || self.device_ns.is_some()
            || self.enqueue_ns.is_some()
            || self.wait_ns.is_some()
    }
}

/// One artifact referenced by an evidence bundle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceArtifact {
    /// Stable artifact kind, such as `pipeline_manifest`, `benchmark_report`, or `replay_capsule`.
    pub kind: String,
    /// Backend that produced or owns the artifact when applicable.
    pub backend_id: Option<String>,
    /// Relative or consumer-provided artifact path.
    pub path: Option<String>,
    /// Content digest or identity digest when available.
    pub digest: Option<String>,
}

impl EvidenceArtifact {
    /// Build an artifact row.
    #[must_use]
    pub fn new(
        kind: impl Into<String>,
        backend_id: Option<String>,
        path: Option<String>,
        digest: Option<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            backend_id,
            path,
            digest,
        }
    }

    /// Build an artifact row from a compiled-pipeline manifest.
    #[must_use]
    pub fn from_pipeline_manifest(manifest: &PipelineReproManifest) -> Self {
        Self {
            kind: "pipeline_manifest".to_string(),
            backend_id: Some(manifest.backend_id.clone()),
            path: None,
            digest: Some(manifest.program_digest.clone()),
        }
    }
}

/// Replay metadata attached to a dispatch or conformance failure.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReplayEvidence {
    /// Human-runnable replay command.
    pub command: String,
    /// Capsule digest when the replay payload has been materialized.
    pub capsule_digest: Option<String>,
}

impl ReplayEvidence {
    /// Build replay evidence.
    #[must_use]
    pub fn new(command: impl Into<String>, capsule_digest: Option<String>) -> Self {
        Self {
            command: command.into(),
            capsule_digest,
        }
    }
}

/// Versioned digest ledger for every identity lane that participates in
/// evidence replay, provenance, and cache correlation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceDigestLedger {
    /// Ledger schema version.
    pub schema: u32,
    /// Version label for `program_wire_digest`.
    pub program_wire_version: String,
    /// BLAKE3 digest of canonical VIR0 Program wire bytes.
    pub program_wire_digest: String,
    /// Version label for `normalized_program_digest`.
    pub normalized_program_version: String,
    /// Normalized Program digest used by pipeline caches.
    pub normalized_program_digest: String,
    /// Version label for `workload_digest`.
    pub workload_version: String,
    /// Dispatch workload/config digest.
    pub workload_digest: String,
    /// Version label for `source_fingerprint`.
    pub source_version: String,
    /// Commit/dirty-state source fingerprint.
    pub source_fingerprint: String,
    /// Version label for `source_tree_fingerprint`.
    pub source_tree_version: String,
    /// Source-tree content fingerprint.
    pub source_tree_fingerprint: String,
    /// Version label for `environment_digest`.
    pub environment_version: String,
    /// Backend id/version digest.
    pub environment_digest: String,
}

impl EvidenceDigestLedger {
    /// Current digest-ledger schema.
    pub const SCHEMA: u32 = 1;

    /// Build the digest ledger from the same inputs used to build an evidence
    /// bundle.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::InvalidProgram`] when the normalized Program
    /// digest cannot be built.
    pub fn for_inputs(
        backend_id: &str,
        backend_version: &str,
        program: &Program,
        config: &DispatchConfig,
        source: &SourceProvenance,
    ) -> Result<Self, BackendError> {
        let normalized_program_digest =
            try_normalized_program_cache_digest(program).map_err(|error| {
                BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: failed to build evidence Program digest: {error}. Validate and normalize the Program before dispatch evidence emission."
                    ),
                }
            })?;
        Ok(Self {
            schema: Self::SCHEMA,
            program_wire_version: PROGRAM_WIRE_DIGEST_VERSION.to_string(),
            program_wire_digest: digest_to_hex(program.fingerprint()),
            normalized_program_version: NORMALIZED_PROGRAM_DIGEST_VERSION.to_string(),
            normalized_program_digest: digest_to_hex(normalized_program_digest),
            workload_version: WORKLOAD_FINGERPRINT_VERSION.to_string(),
            workload_digest: digest_to_hex(dispatch_policy_cache_digest(config)),
            source_version: SOURCE_FINGERPRINT_VERSION.to_string(),
            source_fingerprint: source.source_fingerprint.clone(),
            source_tree_version: SOURCE_TREE_FINGERPRINT_VERSION.to_string(),
            source_tree_fingerprint: source.source_tree_fingerprint.clone(),
            environment_version: ENVIRONMENT_FINGERPRINT_VERSION.to_string(),
            environment_digest: evidence_environment_digest(backend_id, backend_version),
        })
    }

    /// Validate ledger version labels and digest shapes.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::InvalidProgram`] when any ledger lane is missing,
    /// malformed, or versioned against the wrong contract.
    pub fn validate(&self) -> Result<(), BackendError> {
        if self.schema != Self::SCHEMA {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: evidence digest ledger schema {} is unsupported; regenerate evidence with schema {}.",
                    self.schema,
                    Self::SCHEMA
                ),
            });
        }
        validate_ledger_version(
            "program_wire_version",
            &self.program_wire_version,
            PROGRAM_WIRE_DIGEST_VERSION,
        )?;
        validate_ledger_version(
            "normalized_program_version",
            &self.normalized_program_version,
            NORMALIZED_PROGRAM_DIGEST_VERSION,
        )?;
        validate_ledger_version(
            "workload_version",
            &self.workload_version,
            WORKLOAD_FINGERPRINT_VERSION,
        )?;
        validate_ledger_version(
            "source_version",
            &self.source_version,
            SOURCE_FINGERPRINT_VERSION,
        )?;
        validate_ledger_version(
            "source_tree_version",
            &self.source_tree_version,
            SOURCE_TREE_FINGERPRINT_VERSION,
        )?;
        validate_ledger_version(
            "environment_version",
            &self.environment_version,
            ENVIRONMENT_FINGERPRINT_VERSION,
        )?;
        validate_hex_digest("program_wire_digest", &self.program_wire_digest)?;
        validate_hex_digest("normalized_program_digest", &self.normalized_program_digest)?;
        validate_hex_digest("workload_digest", &self.workload_digest)?;
        validate_hex_digest("environment_digest", &self.environment_digest)?;
        if self.source_fingerprint.trim().is_empty() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: evidence digest ledger source_fingerprint must be non-empty."
                    .to_string(),
            });
        }
        if self.source_tree_fingerprint.trim().is_empty() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: evidence digest ledger source_tree_fingerprint must be non-empty."
                    .to_string(),
            });
        }
        Ok(())
    }
}

fn validate_ledger_version(label: &str, actual: &str, expected: &str) -> Result<(), BackendError> {
    if actual != expected {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: evidence digest ledger {label} must be `{expected}`, got `{actual}`."
            ),
        });
    }
    Ok(())
}

fn validate_hex_digest(label: &str, value: &str) -> Result<(), BackendError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(BackendError::InvalidProgram {
            fix: format!("Fix: evidence digest ledger {label} must be a 64-character hex digest."),
        });
    }
    Ok(())
}

/// Shared evidence bundle for dispatch, benchmark, conformance, and replay surfaces.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceBundle {
    /// Bundle schema version.
    pub schema: u32,
    /// Backend that produced the result or artifact.
    pub backend_id: String,
    /// Backend implementation version.
    pub backend_version: String,
    /// Canonical normalized Program digest as lowercase hex.
    pub program_digest: String,
    /// Dispatch policy fields that affect generated backend code.
    pub dispatch_policy: String,
    /// Versioned digest ledger binding Program, workload, source, and backend
    /// environment identity.
    pub digest_ledger: EvidenceDigestLedger,
    /// Source provenance for the code that produced this evidence.
    pub source: SourceProvenance,
    /// Timing evidence for the dispatch or run.
    pub timing: DispatchTimingEvidence,
    /// Artifacts referenced by this bundle.
    pub artifacts: Vec<EvidenceArtifact>,
    /// Replay metadata when a replay capsule exists.
    pub replay: Option<ReplayEvidence>,
}

impl EvidenceBundle {
    /// Current evidence bundle schema.
    pub const SCHEMA: u32 = 1;

    /// Build an evidence bundle for a backend/program/config tuple.
    ///
    /// # Errors
    /// Returns [`BackendError`] when the Program cannot be fingerprinted or
    /// provenance is too weak to emit.
    pub fn for_program(
        backend: &dyn VyreBackend,
        program: &Program,
        config: &DispatchConfig,
        source: SourceProvenance,
    ) -> Result<Self, BackendError> {
        source.validate()?;
        let backend_id = backend.id();
        let backend_version = backend.version();
        let digest_ledger = EvidenceDigestLedger::for_inputs(
            backend_id,
            backend_version,
            program,
            config,
            &source,
        )?;
        Ok(Self {
            schema: Self::SCHEMA,
            backend_id: backend_id.to_string(),
            backend_version: backend_version.to_string(),
            program_digest: digest_ledger.normalized_program_digest.clone(),
            dispatch_policy: dispatch_policy_cache_string(config),
            digest_ledger,
            source,
            timing: DispatchTimingEvidence::default(),
            artifacts: Vec::new(),
            replay: None,
        })
    }

    /// Attach timing from a backend dispatch result.
    #[must_use]
    pub fn with_timed_dispatch(mut self, result: &TimedDispatchResult) -> Self {
        self.timing = DispatchTimingEvidence::from_timed_dispatch(result);
        self
    }

    /// Attach an artifact row.
    #[must_use]
    pub fn with_artifact(mut self, artifact: EvidenceArtifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    /// Attach replay metadata.
    #[must_use]
    pub fn with_replay(mut self, replay: ReplayEvidence) -> Self {
        self.replay = Some(replay);
        self
    }

    /// Validate the bundle's load-bearing fields.
    ///
    /// # Errors
    /// Returns [`BackendError::InvalidProgram`] when a bundle is missing a
    /// required identity field or carries malformed digest metadata.
    pub fn validate(&self) -> Result<(), BackendError> {
        if self.schema != Self::SCHEMA {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: evidence bundle schema {} is unsupported; regenerate evidence with schema {}.",
                    self.schema,
                    Self::SCHEMA
                ),
            });
        }
        if self.backend_id.trim().is_empty() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: evidence bundle backend_id must be non-empty.".to_string(),
            });
        }
        if self.program_digest.len() != 64
            || !self
                .program_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: evidence bundle program_digest must be a 64-character hex digest."
                    .to_string(),
            });
        }
        self.digest_ledger.validate()?;
        if self.digest_ledger.normalized_program_digest != self.program_digest {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: evidence bundle program_digest must match digest_ledger.normalized_program_digest.".to_string(),
            });
        }
        if self.digest_ledger.source_fingerprint != self.source.source_fingerprint {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: evidence bundle source_fingerprint must match digest_ledger.source_fingerprint.".to_string(),
            });
        }
        if self.digest_ledger.source_tree_fingerprint != self.source.source_tree_fingerprint {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: evidence bundle source_tree_fingerprint must match digest_ledger.source_tree_fingerprint.".to_string(),
            });
        }
        self.source.validate()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::backend::{private, CompiledPipeline, OutputBuffers};
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node};

    #[derive(Clone)]
    struct EvidenceTestBackend;

    impl private::Sealed for EvidenceTestBackend {}

    impl VyreBackend for EvidenceTestBackend {
        fn id(&self) -> &'static str {
            "evidence-test"
        }

        fn version(&self) -> &'static str {
            "test-version"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(vec![42_u32.to_le_bytes().to_vec()])
        }
    }

    #[derive(Clone)]
    struct VersionedEvidenceTestBackend {
        id: &'static str,
        version: &'static str,
    }

    impl private::Sealed for VersionedEvidenceTestBackend {}

    impl VyreBackend for VersionedEvidenceTestBackend {
        fn id(&self) -> &'static str {
            self.id
        }

        fn version(&self) -> &'static str {
            self.version
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(vec![42_u32.to_le_bytes().to_vec()])
        }
    }

    struct EvidencePipeline;

    impl private::Sealed for EvidencePipeline {}

    impl CompiledPipeline for EvidencePipeline {
        fn id(&self) -> &str {
            "evidence-test:pipeline"
        }

        fn dispatch(
            &self,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<OutputBuffers, BackendError> {
            Ok(vec![42_u32.to_le_bytes().to_vec()])
        }
    }

    fn evidence_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(1),
                BufferDecl::output("output", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "output",
                Expr::u32(0),
                Expr::load("input", Expr::u32(0)),
            )],
        )
    }

    fn source() -> SourceProvenance {
        SourceProvenance {
            git: BTreeMap::from([
                ("commit".to_string(), "abc123".to_string()),
                ("dirty".to_string(), "false".to_string()),
            ]),
            source_fingerprint: "git:abc123:dirty=false".to_string(),
            source_tree_fingerprint: "source-tree-v1:test".to_string(),
        }
    }

    fn changed_ledger_lanes(
        left: &EvidenceDigestLedger,
        right: &EvidenceDigestLedger,
    ) -> Vec<&'static str> {
        let mut changed = Vec::new();
        if left.program_wire_digest != right.program_wire_digest {
            changed.push("program_wire_digest");
        }
        if left.normalized_program_digest != right.normalized_program_digest {
            changed.push("normalized_program_digest");
        }
        if left.workload_digest != right.workload_digest {
            changed.push("workload_digest");
        }
        if left.source_fingerprint != right.source_fingerprint {
            changed.push("source_fingerprint");
        }
        if left.source_tree_fingerprint != right.source_tree_fingerprint {
            changed.push("source_tree_fingerprint");
        }
        if left.environment_digest != right.environment_digest {
            changed.push("environment_digest");
        }
        changed
    }

    #[test]
    fn evidence_bundle_records_backend_program_policy_source_timing_and_artifacts() {
        let backend = EvidenceTestBackend;
        let program = evidence_program();
        let mut config = DispatchConfig::default();
        config.workgroup_override = Some([8, 1, 1]);
        let timed = TimedDispatchResult {
            outputs: vec![42_u32.to_le_bytes().to_vec()],
            wall_ns: 100,
            device_ns: Some(70),
            enqueue_ns: Some(10),
            wait_ns: Some(20),
        };
        let pipeline = Arc::new(EvidencePipeline);
        let manifest = PipelineReproManifest::new(
            backend.id(),
            pipeline.id(),
            try_normalized_program_cache_digest(&program)
                .expect("Fix: evidence test Program must fingerprint"),
            dispatch_policy_cache_string(&config),
            Some(true),
        );

        let bundle = EvidenceBundle::for_program(&backend, &program, &config, source())
            .expect("Fix: evidence bundle should build for valid source/program")
            .with_timed_dispatch(&timed)
            .with_artifact(EvidenceArtifact::from_pipeline_manifest(&manifest))
            .with_replay(ReplayEvidence::new(
                "vyre-conform dispatch --backend evidence-test --ops evidence.test",
                Some("capsule-digest".to_string()),
            ));

        bundle
            .validate()
            .expect("Fix: complete evidence bundle should validate");
        assert_eq!(bundle.backend_id, "evidence-test");
        assert_eq!(bundle.backend_version, "test-version");
        assert_eq!(bundle.program_digest.len(), 64);
        assert_eq!(
            bundle.program_digest,
            bundle.digest_ledger.normalized_program_digest
        );
        assert_eq!(
            bundle.digest_ledger.program_wire_version,
            PROGRAM_WIRE_DIGEST_VERSION
        );
        assert_eq!(
            bundle.digest_ledger.normalized_program_version,
            NORMALIZED_PROGRAM_DIGEST_VERSION
        );
        assert_eq!(
            bundle.digest_ledger.workload_version,
            WORKLOAD_FINGERPRINT_VERSION
        );
        assert_eq!(bundle.dispatch_policy, "ulp=None:wg=Some([8, 1, 1])");
        assert_eq!(bundle.source.source_fingerprint, "git:abc123:dirty=false");
        assert_eq!(bundle.timing.device_ns, Some(70));
        assert_eq!(bundle.artifacts[0].kind, "pipeline_manifest");
        assert_eq!(
            bundle.replay.as_ref().map(|replay| replay.command.as_str()),
            Some("vyre-conform dispatch --backend evidence-test --ops evidence.test")
        );
    }

    #[test]
    fn digest_ledger_scopes_program_source_workload_and_environment_changes() {
        let backend = VersionedEvidenceTestBackend {
            id: "evidence-test",
            version: "test-version",
        };
        let program = evidence_program();
        let config = DispatchConfig::default();
        let source = source();
        let base = EvidenceBundle::for_program(&backend, &program, &config, source.clone())
            .expect("Fix: base evidence bundle must build")
            .digest_ledger;

        let changed_program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(1),
                BufferDecl::output("output", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store("output", Expr::u32(0), Expr::u32(7))],
        );
        let program_changed =
            EvidenceBundle::for_program(&backend, &changed_program, &config, source.clone())
                .expect("Fix: changed Program evidence bundle must build")
                .digest_ledger;
        assert_eq!(
            changed_ledger_lanes(&base, &program_changed),
            vec!["program_wire_digest", "normalized_program_digest"],
            "Fix: Program body mutations must not perturb source, workload, or environment digest lanes."
        );

        let source_changed = SourceProvenance {
            source_fingerprint: "git:def456:dirty=false".to_string(),
            ..source.clone()
        };
        let source_ledger =
            EvidenceBundle::for_program(&backend, &program, &config, source_changed)
                .expect("Fix: changed source evidence bundle must build")
                .digest_ledger;
        assert_eq!(
            changed_ledger_lanes(&base, &source_ledger),
            vec!["source_fingerprint"],
            "Fix: source fingerprint mutations must stay in the source lane."
        );

        let source_tree_changed = SourceProvenance {
            source_tree_fingerprint: "source-tree-v1:changed".to_string(),
            ..source.clone()
        };
        let source_tree_ledger =
            EvidenceBundle::for_program(&backend, &program, &config, source_tree_changed)
                .expect("Fix: changed source-tree evidence bundle must build")
                .digest_ledger;
        assert_eq!(
            changed_ledger_lanes(&base, &source_tree_ledger),
            vec!["source_tree_fingerprint"],
            "Fix: source-tree mutations must stay in the source-tree lane."
        );

        let mut workload_changed = DispatchConfig::default();
        workload_changed.workgroup_override = Some([8, 1, 1]);
        let workload_ledger =
            EvidenceBundle::for_program(&backend, &program, &workload_changed, source.clone())
                .expect("Fix: changed workload evidence bundle must build")
                .digest_ledger;
        assert_eq!(
            changed_ledger_lanes(&base, &workload_ledger),
            vec!["workload_digest"],
            "Fix: workload/config mutations must stay in the workload digest lane."
        );

        let environment_changed = VersionedEvidenceTestBackend {
            id: "evidence-test",
            version: "test-version-2",
        };
        let environment_ledger =
            EvidenceBundle::for_program(&environment_changed, &program, &config, source)
                .expect("Fix: changed environment evidence bundle must build")
                .digest_ledger;
        assert_eq!(
            changed_ledger_lanes(&base, &environment_ledger),
            vec!["environment_digest"],
            "Fix: backend environment mutations must stay in the environment digest lane."
        );
    }

    #[test]
    fn evidence_bundle_rejects_digest_ledger_mismatch() {
        let backend = EvidenceTestBackend;
        let program = evidence_program();
        let mut bundle =
            EvidenceBundle::for_program(&backend, &program, &DispatchConfig::default(), source())
                .expect("Fix: evidence bundle should build before ledger mutation");
        bundle.digest_ledger.normalized_program_digest =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();

        let error = bundle
            .validate()
            .expect_err("Fix: evidence validation must reject a mismatched digest ledger");
        assert!(
            error.to_string().contains("digest_ledger"),
            "Fix: digest ledger mismatch rejection must name the mismatched field: {error}"
        );
    }

    #[test]
    fn evidence_bundle_rejects_weak_source_provenance() {
        let backend = EvidenceTestBackend;
        let program = evidence_program();
        let invalid = SourceProvenance {
            git: BTreeMap::new(),
            source_fingerprint: " ".to_string(),
            source_tree_fingerprint: "source-tree-v1:test".to_string(),
        };

        let error =
            EvidenceBundle::for_program(&backend, &program, &DispatchConfig::default(), invalid)
                .expect_err("Fix: evidence bundle must reject blank source_fingerprint");

        assert!(
            error.to_string().contains("source_fingerprint"),
            "Fix: source provenance rejection must name the weak field: {error}"
        );
    }

    #[test]
    fn clean_source_fingerprint_keeps_commit_dirty_contract() {
        let git = BTreeMap::from([
            ("commit".to_string(), "abc123".to_string()),
            ("dirty".to_string(), "false".to_string()),
        ]);

        assert_eq!(
            source_fingerprint(&git),
            "git:abc123:dirty=false",
            "Fix: clean source fingerprints must remain stable for existing release evidence contracts."
        );
    }

    #[test]
    fn dirty_source_fingerprint_carries_worktree_digest() {
        let git = BTreeMap::from([
            ("commit".to_string(), "abc123".to_string()),
            ("dirty".to_string(), "true".to_string()),
            (
                "dirty_worktree_fingerprint".to_string(),
                "worktree-hash".to_string(),
            ),
        ]);

        assert_eq!(
            source_fingerprint(&git),
            "git:abc123:dirty=true:worktree=worktree-hash",
            "Fix: dirty source fingerprints must distinguish different dirty worktree states."
        );
    }

    #[test]
    fn dirty_source_fingerprint_without_digest_fails_closed() {
        let git = BTreeMap::from([
            ("commit".to_string(), "abc123".to_string()),
            ("dirty".to_string(), "true".to_string()),
        ]);

        assert_eq!(
            source_fingerprint(&git),
            "git:abc123:dirty=true:worktree=unknown",
            "Fix: dirty source fingerprints must not fall back to the broad legacy dirty=true contract."
        );
    }

    #[test]
    fn dirty_worktree_digest_changes_with_status_diff_and_untracked_content() {
        let workspace = Path::new(".");
        let base =
            dirty_worktree_fingerprint_from_parts(workspace, b" M a.rs\0", b"-old\n+new\n", b"");
        let changed_status =
            dirty_worktree_fingerprint_from_parts(workspace, b" M b.rs\0", b"-old\n+new\n", b"");
        let changed_diff =
            dirty_worktree_fingerprint_from_parts(workspace, b" M a.rs\0", b"-old\n+newer\n", b"");
        let changed_untracked_inventory =
            dirty_worktree_fingerprint_from_parts(workspace, b"?? c.rs\0", b"", b"c.rs\0");
        let untracked_workspace = temp_workspace("vyre-driver-dirty-fingerprint");
        fs::write(untracked_workspace.join("c.rs"), b"one")
            .expect("Fix: write first untracked content fingerprint fixture.");
        let untracked_one = dirty_worktree_fingerprint_from_parts(
            &untracked_workspace,
            b"?? c.rs\0",
            b"",
            b"c.rs\0",
        );
        fs::write(untracked_workspace.join("c.rs"), b"two")
            .expect("Fix: write second untracked content fingerprint fixture.");
        let untracked_two = dirty_worktree_fingerprint_from_parts(
            &untracked_workspace,
            b"?? c.rs\0",
            b"",
            b"c.rs\0",
        );
        let _ = fs::remove_dir_all(&untracked_workspace);

        assert_ne!(
            base, changed_status,
            "Fix: dirty source fingerprints must change when modified paths change."
        );
        assert_ne!(
            base, changed_diff,
            "Fix: dirty source fingerprints must change when tracked diff bytes change."
        );
        assert_ne!(
            base, changed_untracked_inventory,
            "Fix: dirty source fingerprints must change when untracked inventory changes."
        );
        assert_ne!(
            untracked_one, untracked_two,
            "Fix: dirty source fingerprints must change when untracked file content changes."
        );
    }

    #[test]
    fn source_tree_fingerprint_ignores_generated_release_evidence() {
        let workspace = temp_workspace("vyre-driver-source-tree-fingerprint");
        fs::create_dir_all(workspace.join("src")).expect("Fix: create source fixture directory.");
        fs::create_dir_all(workspace.join("release/evidence/benchmarks"))
            .expect("Fix: create generated evidence fixture directory.");
        fs::write(workspace.join("src/lib.rs"), b"pub fn source() {}\n")
            .expect("Fix: write source-tree fingerprint source fixture.");
        fs::write(
            workspace.join("release/evidence/benchmarks/workload.json"),
            b"{\"old\":true}\n",
        )
        .expect("Fix: write source-tree fingerprint evidence fixture.");
        let paths = b"src/lib.rs\0release/evidence/benchmarks/workload.json\0";

        let base = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join("release/evidence/benchmarks/workload.json"),
            b"{\"new\":true}\n",
        )
        .expect("Fix: mutate generated evidence fixture.");
        let evidence_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join("src/lib.rs"),
            b"pub fn source_changed() {}\n",
        )
        .expect("Fix: mutate source fixture.");
        let source_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        let _ = fs::remove_dir_all(&workspace);

        assert_eq!(
            base, evidence_changed,
            "Fix: generated release evidence must not invalidate committed benchmark source provenance."
        );
        assert_ne!(
            base, source_changed,
            "Fix: source-tree provenance must still change when real source files change."
        );
    }

    #[test]
    fn source_tree_fingerprint_ignores_release_tooling_source() {
        let workspace = temp_workspace("vyre-driver-source-tree-tooling");
        fs::create_dir_all(workspace.join("vyre-bench/src"))
            .expect("Fix: create benchmark source fixture directory.");
        fs::create_dir_all(workspace.join(".github/workflows"))
            .expect("Fix: create workflow fixture directory.");
        fs::create_dir_all(workspace.join("scripts"))
            .expect("Fix: create release script fixture directory.");
        fs::create_dir_all(workspace.join("xtask/src"))
            .expect("Fix: create release tooling fixture directory.");
        fs::write(workspace.join("cargo_full"), b"#!/usr/bin/env bash\n")
            .expect("Fix: write cargo wrapper fixture.");
        fs::write(
            workspace.join("vyre-bench/src/lib.rs"),
            b"pub fn benchmark() {}\n",
        )
        .expect("Fix: write benchmark source fixture.");
        fs::write(
            workspace.join("xtask/src/hygiene_matrix.rs"),
            b"pub fn tooling() {}\n",
        )
        .expect("Fix: write release tooling fixture.");
        fs::write(
            workspace.join("scripts/install_lego_quick_hook.sh"),
            b"#!/usr/bin/env bash\n",
        )
        .expect("Fix: write release script fixture.");
        fs::write(
            workspace.join(".github/workflows/ci.yml"),
            b"run: ./cargo_full test --workspace\n",
        )
        .expect("Fix: write workflow fixture.");
        let paths = b".github/workflows/ci.yml\0cargo_full\0scripts/install_lego_quick_hook.sh\0vyre-bench/src/lib.rs\0xtask/src/hygiene_matrix.rs\0";

        let base = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join("cargo_full"),
            b"#!/usr/bin/env bash\nexec cargo \"$@\"\n",
        )
        .expect("Fix: mutate cargo wrapper fixture.");
        let wrapper_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join("scripts/install_lego_quick_hook.sh"),
            b"#!/usr/bin/env bash\n./cargo_full run --bin xtask -- lego-quick\n",
        )
        .expect("Fix: mutate release script fixture.");
        let script_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join(".github/workflows/ci.yml"),
            b"run: ./cargo_full test --workspace --all-targets\n",
        )
        .expect("Fix: mutate workflow fixture.");
        let workflow_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join("xtask/src/hygiene_matrix.rs"),
            b"pub fn tooling_changed() {}\n",
        )
        .expect("Fix: mutate release tooling fixture.");
        let tooling_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join("vyre-bench/src/lib.rs"),
            b"pub fn benchmark_changed() {}\n",
        )
        .expect("Fix: mutate benchmark source fixture.");
        let benchmark_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        let _ = fs::remove_dir_all(&workspace);

        assert_eq!(
            base, tooling_changed,
            "Fix: release evidence/tooling generators must not invalidate benchmark runtime source provenance."
        );
        assert_eq!(
            base, wrapper_changed,
            "Fix: bounded cargo wrapper changes must not invalidate benchmark runtime source provenance."
        );
        assert_eq!(
            base, script_changed,
            "Fix: release scripts must not invalidate benchmark runtime source provenance."
        );
        assert_eq!(
            base, workflow_changed,
            "Fix: CI workflow edits must not invalidate benchmark runtime source provenance."
        );
        assert_ne!(
            base, benchmark_changed,
            "Fix: benchmark source edits must still invalidate benchmark source provenance."
        );
    }

    #[test]
    fn source_tree_fingerprint_ignores_test_evidence() {
        let workspace = temp_workspace("vyre-driver-source-tree-tests");
        fs::create_dir_all(workspace.join("vyre-libs/src"))
            .expect("Fix: create library source fixture directory.");
        fs::create_dir_all(workspace.join("vyre-libs/tests/support"))
            .expect("Fix: create integration test support fixture directory.");
        fs::create_dir_all(workspace.join("vyre-libs/src/graph"))
            .expect("Fix: create inline test fixture directory.");
        fs::write(
            workspace.join("vyre-libs/src/lib.rs"),
            b"pub fn source() {}\n",
        )
        .expect("Fix: write source-tree fingerprint source fixture.");
        fs::write(
            workspace.join("vyre-libs/tests/filter_roundtrip.rs"),
            b"#[test]\nfn roundtrip() {}\n",
        )
        .expect("Fix: write integration test fixture.");
        fs::write(
            workspace.join("vyre-libs/tests/support/filter.rs"),
            b"pub fn helper() {}\n",
        )
        .expect("Fix: write test support fixture.");
        fs::write(
            workspace.join("vyre-libs/src/graph/tests.rs"),
            b"#[test]\nfn graph_contract() {}\n",
        )
        .expect("Fix: write inline tests fixture.");
        let paths = b"vyre-libs/src/lib.rs\0vyre-libs/tests/filter_roundtrip.rs\0vyre-libs/tests/support/filter.rs\0vyre-libs/src/graph/tests.rs\0";

        let base = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join("vyre-libs/tests/filter_roundtrip.rs"),
            b"#[test]\nfn roundtrip_modularized() {}\n",
        )
        .expect("Fix: mutate integration test fixture.");
        fs::write(
            workspace.join("vyre-libs/tests/support/filter.rs"),
            b"pub fn helper_modularized() {}\n",
        )
        .expect("Fix: mutate test support fixture.");
        fs::write(
            workspace.join("vyre-libs/src/graph/tests.rs"),
            b"#[test]\nfn graph_contract_modularized() {}\n",
        )
        .expect("Fix: mutate inline tests fixture.");
        let tests_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        fs::write(
            workspace.join("vyre-libs/src/lib.rs"),
            b"pub fn source_changed() {}\n",
        )
        .expect("Fix: mutate production source fixture.");
        let source_changed = source_tree_fingerprint_from_paths(&workspace, paths);
        let _ = fs::remove_dir_all(&workspace);

        assert_eq!(
            base, tests_changed,
            "Fix: test-only modularization must not invalidate runtime benchmark source provenance."
        );
        assert_ne!(
            base, source_changed,
            "Fix: source-tree provenance must still change when production source changes."
        );
    }

    #[test]
    fn source_fingerprint_ignores_generated_release_evidence_dirty_status() {
        let workspace = temp_workspace("vyre-driver-source-fingerprint-evidence");
        fs::create_dir_all(workspace.join("src"))
            .expect("Fix: create source fingerprint fixture source directory.");
        fs::create_dir_all(workspace.join("release/evidence/benchmarks"))
            .expect("Fix: create source fingerprint fixture evidence directory.");
        fs::write(workspace.join("src/lib.rs"), b"pub fn source() {}\n")
            .expect("Fix: write source fingerprint source fixture.");
        fs::write(
            workspace.join("release/evidence/benchmarks/workload.json"),
            b"{\"old\":true}\n",
        )
        .expect("Fix: write tracked generated evidence fixture.");
        git_fixture(&workspace, &["init", "--quiet", "--initial-branch", "main"]);
        git_fixture(
            &workspace,
            &["config", "user.email", "vyre@example.invalid"],
        );
        git_fixture(&workspace, &["config", "user.name", "Vyre Test"]);
        git_fixture(
            &workspace,
            &[
                "add",
                "src/lib.rs",
                "release/evidence/benchmarks/workload.json",
            ],
        );
        git_fixture(&workspace, &["commit", "--quiet", "-m", "seed"]);

        fs::write(
            workspace.join("release/evidence/benchmarks/workload.json"),
            b"{\"new\":true}\n",
        )
        .expect("Fix: mutate tracked generated evidence fixture.");
        fs::write(
            workspace.join("release/evidence/benchmarks/new-workload.json"),
            b"{\"new\":true}\n",
        )
        .expect("Fix: write untracked generated evidence fixture.");
        let evidence_only = capture_git_info_at(&workspace);
        fs::write(
            workspace.join("src/lib.rs"),
            b"pub fn source_changed() {}\n",
        )
        .expect("Fix: mutate real source fixture.");
        let source_changed = capture_git_info_at(&workspace);
        let _ = fs::remove_dir_all(&workspace);

        assert_eq!(
            evidence_only.get("dirty").map(String::as_str),
            Some("false"),
            "Fix: generated release evidence writes must not mark benchmark source provenance dirty."
        );
        assert_eq!(
            source_changed.get("dirty").map(String::as_str),
            Some("true"),
            "Fix: real source edits must still mark benchmark source provenance dirty."
        );
    }

    fn temp_workspace(prefix: &str) -> std::path::PathBuf {
        let workspace = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Fix: system clock must support unix epoch duration for temp test id.")
                .as_nanos()
        ));
        fs::create_dir_all(&workspace).expect("Fix: create temporary provenance test workspace.");
        workspace
    }

    fn git_fixture(workspace: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(workspace)
            .output()
            .expect("Fix: git fixture command must start.");
        assert!(
            output.status.success(),
            "Fix: git fixture command `git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
}

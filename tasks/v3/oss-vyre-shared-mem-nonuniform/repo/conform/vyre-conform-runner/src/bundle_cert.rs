//! Bundle-level conformance certificate.
//!
//! The per-op [`Certificate`](crate::cert::Certificate) proves that a
//! single op behaves identically on a backend and on the reference
//! backend. A bundle cert widens that guarantee to a whole fused
//! document: every rule Program, dispatched over a named corpus,
//! produces a byte-identical output stream on the issuing reference
//! backend.
//!
//! ## Why
//!
//! warpscan ships with rule bundles. At internet scale the cost of a
//! single cross-backend divergence is "malware missed" (LAW 8). The
//! bundle cert collapses that risk to a startup check:
//!
//! 1. `security-analysis-consumer compile --cert-corpus <dir>`  -  running reference sweep
//!    captures `reference_output_blake3` into `<bundle>.cert`.
//! 2. `warpscan scan --verify-cert`  -  on startup, warpscan re-runs the
//!    same corpus through the live backend, blake3s the outputs, and
//!    refuses to proceed if it diverges from the cert.
//!
//! The cert is content-addressable: identical inputs produce byte-
//! identical certs, so the same file serves as both integrity proof
//! and pipeline-cache key for the compiled backend pipeline.
//!
//! ## Design
//!
//! - **Input determinism**: the corpus is supplied as a list of
//!   [`CorpusWitness`] records, each naming one dispatch. They're
//!   sorted by `name` before hashing so the same logical corpus
//!   produces the same cert regardless of enumeration order.
//! - **Output determinism**: outputs are captured as
//!   `Vec<Vec<u8>>` (one byte buffer per output buffer in the
//!   Program) and length-prefixed in the hash stream, so unlike plain
//!   concatenation the hash survives empty outputs without
//!   collision.
//! - **Signatures**: the cert body hashes, not the sig. Callers
//!   supply `signature_ed25519` + `pubkey` separately so the cert
//!   can round-trip through CI systems that sign artifacts
//!   out-of-band.

// hex::encode superseded the manual write! loop in hex32 (L2).
// The import stays in case future helpers need fmt::Write.
#[allow(unused_imports)]
use std::fmt::Write;

use serde::{Deserialize, Serialize};
use vyre::ir::Program;
use vyre::{BackendError, VyreBackend};

use vyre_reference::value::Value;

use crate::dispatch_grid;
use crate::witness_plan::{plan_witness_inputs_into, WitnessInputPlan};

/// Conformance certificate for a whole compiled document (bundle).
///
/// Unlike the per-op [`Certificate`](crate::cert::Certificate) this
/// cert spans an entire fused Program plus a named corpus of
/// witnesses.  A consumer that can reproduce both hashes on their
/// backend is guaranteed to see identical rule hits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleCertificate {
    /// Certificate format version (follows vyre minor).
    pub version: String,
    /// `blake3(program.to_wire())`  -  bundle wire bytes.
    pub bundle_blake3: String,
    /// `blake3(canonicalized corpus input stream)`.
    pub corpus_blake3: String,
    /// `blake3(canonicalized reference-output stream)`.
    pub reference_output_blake3: String,
    /// Number of witness inputs (aka dispatches per pass).
    pub witness_count: u64,
    /// ISO 8601 UTC timestamp.
    pub timestamp: String,
    /// Ed25519 signature over the canonical JSON body (hex).
    pub signature_ed25519: String,
    /// Ed25519 public key (hex).
    pub pubkey: String,
}

/// One corpus witness: a named input set fed to the Program as a
/// single dispatch.
#[derive(Debug, Clone)]
pub struct CorpusWitness {
    /// Stable label for this witness  -  used as the sort key when
    /// canonicalising the corpus hash.
    pub name: String,
    /// Logical witness input buffers. Declared outputs may be omitted; static
    /// read-write buffers may be omitted and are zero-filled before dispatch.
    pub inputs: Vec<Vec<u8>>,
}

/// Errors surfaced by bundle-cert issue / verify.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BundleCertError {
    /// Corpus is empty  -  nothing to certify.
    #[error("bundle cert requires at least one witness. Fix: supply a non-empty corpus.")]
    EmptyCorpus,
    /// Bundle wire bytes are empty  -  upstream serialization bug.
    #[error(
        "bundle wire bytes empty  -  upstream to_wire() regressed. Fix: re-run security-analysis-consumer compile."
    )]
    EmptyBundle,
    /// CPU reference refused a witness.
    #[error(
        "reference interpreter rejected witness `{witness}`: {message}. Fix: inspect the Program body  -  reference must accept what the backend does."
    )]
    ReferenceFailed {
        /// Name of the witness that tripped.
        witness: String,
        /// Rendered error surfaced by `vyre_reference::reference_eval`.
        message: String,
    },
    /// Backend rejected a witness during verification.
    #[error(
        "backend rejected witness `{witness}` during cert verification: {source}. Fix: ensure the backend supports every op in the bundle."
    )]
    BackendFailed {
        /// Name of the witness that tripped.
        witness: String,
        /// Source error surfaced by the backend.
        source: BackendError,
    },
    /// `bundle_blake3` mismatched on verify.
    #[error(
        "bundle wire hash mismatch: cert declares {expected}, observed {observed}. Fix: the Program has been modified since the cert was issued  -  recompile."
    )]
    BundleHashMismatch {
        /// Hash the cert declared.
        expected: String,
        /// Hash observed at verify time.
        observed: String,
    },
    /// `corpus_blake3` mismatched on verify.
    #[error(
        "corpus input hash mismatch: cert declares {expected}, observed {observed}. Fix: the corpus has drifted since cert was issued  -  ensure identical witnesses."
    )]
    CorpusHashMismatch {
        /// Hash the cert declared.
        expected: String,
        /// Hash observed at verify time.
        observed: String,
    },
    /// `reference_output_blake3` mismatched on verify  -  divergence.
    #[error(
        "reference output hash mismatch: cert declares {expected}, backend produced {observed}. Fix: backend diverges from the certified reference. Either the backend regressed or the bundle was certified on a stale reference."
    )]
    OutputHashMismatch {
        /// Hash the cert declared.
        expected: String,
        /// Hash observed at verify time.
        observed: String,
    },
    /// A cert field is still the "TBD" sentinel.
    #[error("cert field `{0}` is still set to the reserved value 'TBD'  -  sign before shipping")]
    UnsetField(&'static str),
    /// Two witnesses in the corpus share the same name.
    ///
    /// CRITIQUE_CONFORM_2026-04-23 H5: duplicate witness names hash
    /// deterministically (the canonicalisation sort is stable), so
    /// the cert verifies, but any downstream display or cache that
    /// indexes by name silently overwrites one entry with the other.
    /// A forged corpus pairing one benign and one malicious witness
    /// with the same name can therefore smuggle through.
    #[error(
        "duplicate witness name `{name}` in corpus. Fix: witness names must be unique  -  rename one of the colliding entries before issuing the cert."
    )]
    DuplicateWitnessName {
        /// Name that appeared more than once.
        name: String,
    },
    /// Cert-declared witness count doesn't match the corpus it
    /// was built against.
    ///
    /// CRITIQUE_CONFORM_2026-04-23 L1: `witness_count` was stored
    /// but never validated on verify. A tampered cert could claim
    /// a misleading count without affecting the hash chain. Now
    /// verify rejects the mismatch with both values named.
    #[error(
        "witness count mismatch: cert declares {expected}, corpus has {observed}. Fix: the cert was built against a different corpus size  -  re-issue with the current corpus."
    )]
    WitnessCountMismatch {
        /// Count declared in the cert.
        expected: u64,
        /// Count observed on verify.
        observed: u64,
    },
    /// Logical witness inputs could not be expanded into the Program dispatch
    /// input stream.
    #[error(
        "witness planner rejected witness `{witness}`: {message}. Fix: make corpus inputs follow logical fixture order and provide bytes for runtime-sized read-write buffers."
    )]
    WitnessPlanningFailed {
        /// Name of the witness that tripped.
        witness: String,
        /// Planner diagnostic.
        message: String,
    },
}

/// Canonicalise a corpus into a deterministic input stream + hash.
///
/// Sorts witnesses by `name`, then for each writes
/// `len(name) || name || witness_count || for each input: len || bytes`.
/// A consumer that receives the same witness set in any order
/// produces the same hash.
fn canonicalise_corpus(
    corpus: &[CorpusWitness],
) -> Result<(Vec<usize>, [u8; 32]), BundleCertError> {
    let mut sorted_indices: Vec<usize> = (0..corpus.len()).collect();
    sorted_indices.sort_by(|&left, &right| corpus[left].name.cmp(&corpus[right].name));

    // CRITIQUE_CONFORM_2026-04-23 H5: reject duplicate names *after*
    // the stable sort so the error names one colliding entry exactly
    // once. A deterministic hash of `[dup, dup]` previously passed
    // verification while any downstream index-by-name consumer
    // silently dropped the second entry.
    if let Some(dup) = sorted_indices
        .windows(2)
        .find_map(|pair| (corpus[pair[0]].name == corpus[pair[1]].name).then_some(pair[0]))
    {
        return Err(BundleCertError::DuplicateWitnessName {
            name: corpus[dup].name.clone(),
        });
    }

    let mut hasher = blake3::Hasher::new();
    for idx in &sorted_indices {
        let w = &corpus[*idx];
        hasher.update(&(w.name.len() as u64).to_le_bytes());
        hasher.update(w.name.as_bytes());
        hasher.update(&(w.inputs.len() as u64).to_le_bytes());
        for input in &w.inputs {
            hasher.update(&(input.len() as u64).to_le_bytes());
            hasher.update(input);
        }
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(hasher.finalize().as_bytes());
    Ok((sorted_indices, hash))
}

#[inline]
fn hash_output_stream(hasher: &mut blake3::Hasher, stream: &[Vec<u8>]) {
    hasher.update(&(stream.len() as u64).to_le_bytes());
    for buf in stream {
        hasher.update(&(buf.len() as u64).to_le_bytes());
        hasher.update(buf);
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    // CRITIQUE_CONFORM_2026-04-23 L2: previous impl `let _ = write!(&mut
    // out, ...)` silently discarded the Result. String::write_str is
    // infallible today, but swallowing the Result would mask a
    // regression if it ever changed  -  violating the 'never swallow
    // errors' standard. Use hex::encode, which produces the same
    // output and propagates any internal failure as a panic with a
    // meaningful message rather than silently truncating the string.
    hex::encode(bytes)
}

fn reference_dispatch<'a>(
    program: &Program,
    witness: &'a CorpusWitness,
    input_plan: &'a WitnessInputPlan,
    planned_inputs: &mut Vec<&'a [u8]>,
    values: &mut Vec<Value>,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), BundleCertError> {
    plan_witness_inputs_into(&witness.inputs, input_plan, planned_inputs).map_err(|message| {
        BundleCertError::WitnessPlanningFailed {
            witness: witness.name.clone(),
            message,
        }
    })?;
    values.clear();
    for input in planned_inputs.iter().copied() {
        values.push(Value::from(input));
    }
    let evaluated = vyre_reference::reference_eval(program, values).map_err(|e| {
        BundleCertError::ReferenceFailed {
            witness: witness.name.clone(),
            message: e.to_string(),
        }
    })?;
    outputs.clear();
    outputs.extend(evaluated.into_iter().map(|value| value.to_bytes()));
    Ok(())
}

fn backend_dispatch<'a>(
    backend: &dyn VyreBackend,
    program: &Program,
    witness: &'a CorpusWitness,
    input_plan: &'a WitnessInputPlan,
    config: &vyre::DispatchConfig,
    borrowed_inputs: &mut Vec<&'a [u8]>,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), BundleCertError> {
    plan_witness_inputs_into(&witness.inputs, input_plan, borrowed_inputs).map_err(|message| {
        BundleCertError::WitnessPlanningFailed {
            witness: witness.name.clone(),
            message,
        }
    })?;
    let dispatched = backend
        .dispatch_borrowed(program, borrowed_inputs, config)
        .map_err(|source| BundleCertError::BackendFailed {
            witness: witness.name.clone(),
            source,
        })?;
    outputs.clear();
    outputs.extend(dispatched.into_iter());
    Ok(())
}

/// Issue a fresh [`BundleCertificate`] from the CPU reference.
///
/// Runs every witness through `vyre_reference::reference_eval`, captures the
/// output stream, blake3s it, and packs the result alongside the
/// bundle wire hash. Caller supplies timestamp + signature (the cert
/// body is the input the signer sees  -  sign after issue).
///
/// # Errors
///
/// - [`BundleCertError::EmptyBundle`]  -  `program_wire_bytes` empty.
/// - [`BundleCertError::EmptyCorpus`]  -  no witnesses.
/// - [`BundleCertError::ReferenceFailed`]  -  the reference interp
///   rejected a witness.
pub fn issue_bundle_cert(
    program: &Program,
    corpus: &[CorpusWitness],
    timestamp: &str,
    signature_ed25519: &str,
    pubkey: &str,
) -> Result<BundleCertificate, BundleCertError> {
    if corpus.is_empty() {
        return Err(BundleCertError::EmptyCorpus);
    }

    let wire_bytes = program
        .to_wire()
        .map_err(|_| BundleCertError::EmptyBundle)?;
    if wire_bytes.is_empty() {
        return Err(BundleCertError::EmptyBundle);
    }
    let bundle_hash = blake3::hash(&wire_bytes);

    let (sorted_indices, corpus_hash) = canonicalise_corpus(corpus)?;
    let input_plan = WitnessInputPlan::for_program(program).map_err(|message| {
        BundleCertError::WitnessPlanningFailed {
            witness: "certificate-issue".to_string(),
            message,
        }
    })?;

    let mut witness_values = Vec::with_capacity(program.buffers().len());
    let mut witness_inputs = Vec::with_capacity(input_plan.source_count());
    let mut witness_outputs = Vec::with_capacity(program.buffers().len());
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(sorted_indices.len() as u64).to_le_bytes());
    for idx in &sorted_indices {
        let w = &corpus[*idx];
        witness_outputs.clear();
        witness_values.clear();
        witness_inputs.clear();
        reference_dispatch(
            program,
            w,
            &input_plan,
            &mut witness_inputs,
            &mut witness_values,
            &mut witness_outputs,
        )?;
        hash_output_stream(&mut hasher, &witness_outputs);
    }
    let output_hash = {
        let hash = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(hash.as_bytes());
        bytes
    };

    Ok(BundleCertificate {
        version: "0.4.1".to_string(),
        bundle_blake3: bundle_hash.to_hex().to_string(),
        corpus_blake3: hex32(&corpus_hash),
        reference_output_blake3: hex32(&output_hash),
        witness_count: sorted_indices.len() as u64,
        timestamp: timestamp.to_string(),
        signature_ed25519: signature_ed25519.to_string(),
        pubkey: pubkey.to_string(),
    })
}

/// Verify a [`BundleCertificate`] against a live backend.
///
/// Re-computes the three hashes from the supplied Program + corpus +
/// backend. Any mismatch surfaces as a structured error that names
/// which stage diverged.
///
/// # Errors
///
/// - [`BundleCertError::BundleHashMismatch`] if the Program wire
///   bytes have drifted.
/// - [`BundleCertError::CorpusHashMismatch`] if the corpus input
///   stream has drifted.
/// - [`BundleCertError::OutputHashMismatch`] if the backend produced
///   outputs that diverge from the certified reference  -  this is
///   the load-bearing assertion.
/// - [`BundleCertError::BackendFailed`] if the backend rejected a
///   witness.
pub fn verify_bundle_with_backend(
    cert: &BundleCertificate,
    program: &Program,
    backend: &dyn VyreBackend,
    corpus: &[CorpusWitness],
) -> Result<(), BundleCertError> {
    let config = dispatch_grid::config_for_program(program).map_err(|message| {
        BundleCertError::ReferenceFailed {
            witness: "certificate-verification".to_string(),
            message,
        }
    })?;
    verify_bundle_with(
        cert,
        program,
        corpus,
        |p, w, input_plan, _values, borrowed_inputs, outputs| {
            backend_dispatch(backend, p, w, input_plan, &config, borrowed_inputs, outputs)
        },
    )
}

/// Verify a [`BundleCertificate`] against the CPU reference. Useful
/// at issue time for self-checks and in CI when a GPU isn't present.
/// The reference-only verifier is guaranteed to match the cert when
/// the cert was issued from the same `(program, corpus)`; treat a
/// failure here as a bug in the hashing, not a correctness failure.
///
/// # Errors
///
/// Same surface as [`verify_bundle_with_backend`], except every
/// divergence reports "reference" rather than a backend id.
pub fn verify_bundle_against_reference(
    cert: &BundleCertificate,
    program: &Program,
    corpus: &[CorpusWitness],
) -> Result<(), BundleCertError> {
    verify_bundle_with(
        cert,
        program,
        corpus,
        |p, w, input_plan, values, borrowed_inputs, outputs| {
            reference_dispatch(p, w, input_plan, borrowed_inputs, values, outputs)
        },
    )
}

/// CRITIQUE_CONFORM_2026-04-23 C1 (companion API): cryptographically
/// verify the Ed25519 signature on a BundleCertificate. The
/// existing `verify_bundle_with_backend` + `verify_bundle_against_reference`
/// entrypoints check the hash chain; they do **not** check the
/// signature. An attacker who can tamper with the hex strings on a
/// shipped cert still has to match the hash chain to produce a
/// cert that verifies via the legacy path, but a bug-compatible
/// downstream consumer that treats "signature field non-empty" as
/// "cryptographically authenticated" is mistaken.
///
/// Callers that require cryptographic authentication must invoke
/// this helper alongside the hash-chain verifier, providing the
/// trusted public key out-of-band. The helper:
/// 1. Validates hex length of `signature_ed25519` (128 hex chars) +
///    the cert-declared `pubkey` (64 hex chars).
/// 2. Confirms the declared pubkey matches the caller-provided
///    trusted key.
/// 3. Verifies the signature over the canonical JSON body of the
///    cert (every field except `signature_ed25519` itself, serialised
///    in a stable field order).
///
/// # Errors
///
/// Returns [`BundleCertError::UnsetField`] when unsigned sentinel fields,
/// malformed hex, key mismatch, or Ed25519 verification failure makes the
/// certificate unauthenticated.
#[must_use = "the signature-verification result must be inspected; dropping it accepts an unverified cert"]
pub fn verify_cert_signature_hex(
    cert: &BundleCertificate,
    trusted_pubkey_hex: &str,
) -> Result<(), BundleCertError> {
    // Hex-length sanity on declared fields (CRITIQUE_CONFORM M2).
    if cert.signature_ed25519 == "TBD" || cert.pubkey == "TBD" {
        return Err(BundleCertError::UnsetField(
            "signature_ed25519 or pubkey still set to 'TBD'  -  sign the cert before shipping.",
        ));
    }
    if cert.signature_ed25519.len() != 128
        || !cert
            .signature_ed25519
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(BundleCertError::UnsetField(
            "signature_ed25519 must be 128 lowercase hex chars (64 raw bytes)",
        ));
    }
    if cert.pubkey.len() != 64 || !cert.pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BundleCertError::UnsetField(
            "pubkey must be 64 lowercase hex chars (32 raw bytes)",
        ));
    }
    if trusted_pubkey_hex.len() != 64 || !trusted_pubkey_hex.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(BundleCertError::UnsetField(
            "trusted_pubkey_hex must be 64 lowercase hex chars (32 raw bytes)",
        ));
    }
    if !cert.pubkey.eq_ignore_ascii_case(trusted_pubkey_hex) {
        return Err(BundleCertError::UnsetField(
            "cert pubkey does not match trusted_pubkey_hex  -  the cert was signed by a different key than the one the caller trusts. This is a fraud signal.",
        ));
    }
    // Cryptographic verification of the signature over the cert's
    // canonical JSON body (every field except signature_ed25519
    // itself, serialised with field order fixed by the
    // BundleCertificate struct declaration). The ed25519-dalek
    // dep already ships in vyre-conform-runner for issue_bundle_cert.
    let sig_bytes = hex::decode(&cert.signature_ed25519).map_err(|_| {
        BundleCertError::UnsetField(
            "signature_ed25519 is not valid hex; impossible after length check, but defensive.",
        )
    })?;
    let pk_bytes = hex::decode(&cert.pubkey).map_err(|_| {
        BundleCertError::UnsetField(
            "pubkey is not valid hex; impossible after length check, but defensive.",
        )
    })?;
    let pk_array: [u8; 32] = pk_bytes.as_slice().try_into().map_err(|_| {
        BundleCertError::UnsetField("pubkey decoded to the wrong byte length; defensive.")
    })?;
    let sig_array: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| {
        BundleCertError::UnsetField(
            "signature_ed25519 decoded to the wrong byte length; defensive.",
        )
    })?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&pk_array).map_err(|_| {
        BundleCertError::UnsetField(
            "pubkey is not a valid Ed25519 compressed point  -  the cert cannot have been signed by this key.",
        )
    })?;
    let signature = ed25519_dalek::Signature::from_bytes(&sig_array);
    // Reconstruct the signable body the same way issue_bundle_cert
    // does: every field except signature_ed25519 itself, in struct-
    // declaration order. A stable JSON encoder keeps this
    // deterministic across runs.
    let signable = serde_json::json!({
        "version": cert.version,
        "bundle_blake3": cert.bundle_blake3,
        "corpus_blake3": cert.corpus_blake3,
        "reference_output_blake3": cert.reference_output_blake3,
        "witness_count": cert.witness_count,
        "timestamp": cert.timestamp,
        "pubkey": cert.pubkey,
    });
    let signable_bytes = serde_json::to_vec(&signable).map_err(|_| {
        BundleCertError::UnsetField(
            "failed to serialise cert body for signature verification  -  impossible on well-formed cert.",
        )
    })?;
    use ed25519_dalek::Verifier;
    verifying_key
        .verify(&signable_bytes, &signature)
        .map_err(|_| {
            BundleCertError::UnsetField(
                "Ed25519 signature does not match cert body. The cert was tampered or signed by a different key.",
            )
        })?;
    Ok(())
}

fn verify_bundle_with<F>(
    cert: &BundleCertificate,
    program: &Program,
    corpus: &[CorpusWitness],
    mut dispatch: F,
) -> Result<(), BundleCertError>
where
    F: for<'a> FnMut(
        &Program,
        &'a CorpusWitness,
        &'a WitnessInputPlan,
        &mut Vec<Value>,
        &mut Vec<&'a [u8]>,
        &mut Vec<Vec<u8>>,
    ) -> Result<(), BundleCertError>,
{
    if corpus.is_empty() {
        return Err(BundleCertError::EmptyCorpus);
    }

    let wire_bytes = program
        .to_wire()
        .map_err(|_| BundleCertError::EmptyBundle)?;
    let observed_bundle = blake3::hash(&wire_bytes).to_hex().to_string();
    if observed_bundle != cert.bundle_blake3 {
        return Err(BundleCertError::BundleHashMismatch {
            expected: cert.bundle_blake3.clone(),
            observed: observed_bundle,
        });
    }

    let (sorted_indices, corpus_hash) = canonicalise_corpus(corpus)?;
    let observed_corpus = hex32(&corpus_hash);
    if observed_corpus != cert.corpus_blake3 {
        return Err(BundleCertError::CorpusHashMismatch {
            expected: cert.corpus_blake3.clone(),
            observed: observed_corpus,
        });
    }

    // CRITIQUE_CONFORM_2026-04-23 L1: witness_count was declared
    // but never validated. Reject mismatches here so a tampered
    // cert that claims a bogus count is surfaced with both values
    // named instead of silently accepted.
    let observed_count = sorted_indices.len() as u64;
    if observed_count != cert.witness_count {
        return Err(BundleCertError::WitnessCountMismatch {
            expected: cert.witness_count,
            observed: observed_count,
        });
    }

    let input_plan = WitnessInputPlan::for_program(program).map_err(|message| {
        BundleCertError::WitnessPlanningFailed {
            witness: "certificate-verification".to_string(),
            message,
        }
    })?;
    let mut values = Vec::with_capacity(program.buffers().len());
    let mut outputs = Vec::with_capacity(program.buffers().len());
    let mut borrowed_inputs = Vec::with_capacity(input_plan.source_count());
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(sorted_indices.len() as u64).to_le_bytes());
    for idx in &sorted_indices {
        let w = &corpus[*idx];
        values.clear();
        borrowed_inputs.clear();
        outputs.clear();
        dispatch(
            program,
            w,
            &input_plan,
            &mut values,
            &mut borrowed_inputs,
            &mut outputs,
        )?;
        hash_output_stream(&mut hasher, &outputs);
    }
    let observed_output = {
        let hash = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(hash.as_bytes());
        hex32(&bytes)
    };
    if observed_output != cert.reference_output_blake3 {
        return Err(BundleCertError::OutputHashMismatch {
            expected: cert.reference_output_blake3.clone(),
            observed: observed_output,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
    use vyre_primitives::wire::pack_u32_slice as bytes_u32;

    /// Smallest non-trivial Program we can dispatch on the reference:
    /// copy the first element of a read-only u32 buffer into a
    /// read-write buffer. Good enough to exercise the byte-identity
    /// pipeline without leaning on a specific feature gate.
    fn copy_first_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("output", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "output",
                Expr::u32(0),
                Expr::load("input", Expr::u32(0)),
            )],
        )
    }

    fn output_first_copy_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::output("output", 0, DataType::U32).with_count(1),
                BufferDecl::storage("input", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "output",
                Expr::u32(0),
                Expr::load("input", Expr::u32(0)),
            )],
        )
    }

    fn sample_corpus() -> Vec<CorpusWitness> {
        vec![
            CorpusWitness {
                name: "alpha".into(),
                inputs: vec![bytes_u32(&[1, 2, 3, 4]), bytes_u32(&[0, 0])],
            },
            CorpusWitness {
                name: "beta".into(),
                inputs: vec![bytes_u32(&[7, 8, 9, 10]), bytes_u32(&[0, 0])],
            },
        ]
    }

    #[test]
    fn issue_populates_all_fields() {
        let program = copy_first_program();
        let cert = issue_bundle_cert(
            &program,
            &sample_corpus(),
            "2026-04-21T00:00:00Z",
            "sig",
            "pub",
        )
        .expect("Fix: issue; restore this invariant before continuing.");
        assert_eq!(cert.witness_count, 2);
        assert_eq!(cert.bundle_blake3.len(), 64);
        assert_eq!(cert.corpus_blake3.len(), 64);
        assert_eq!(cert.reference_output_blake3.len(), 64);
    }

    #[test]
    fn rejects_empty_corpus() {
        let program = copy_first_program();
        let err =
            issue_bundle_cert(&program, &[], "t", "s", "p").expect_err("empty corpus must reject");
        assert!(matches!(err, BundleCertError::EmptyCorpus));
    }

    #[test]
    fn corpus_hash_is_order_independent() {
        let program = copy_first_program();
        let forward = sample_corpus();
        let reversed: Vec<CorpusWitness> = forward.iter().cloned().rev().collect();
        let cert_a = issue_bundle_cert(&program, &forward, "t", "s", "p").unwrap();
        let cert_b = issue_bundle_cert(&program, &reversed, "t", "s", "p").unwrap();
        assert_eq!(cert_a.corpus_blake3, cert_b.corpus_blake3);
        assert_eq!(
            cert_a.reference_output_blake3,
            cert_b.reference_output_blake3
        );
    }

    #[test]
    fn changing_input_changes_cert() {
        let program = copy_first_program();
        let corpus_a = sample_corpus();
        let mut corpus_b = sample_corpus();
        corpus_b[0].inputs[0] = bytes_u32(&[99, 99, 99, 99]);
        let cert_a = issue_bundle_cert(&program, &corpus_a, "t", "s", "p").unwrap();
        let cert_b = issue_bundle_cert(&program, &corpus_b, "t", "s", "p").unwrap();
        assert_ne!(cert_a.corpus_blake3, cert_b.corpus_blake3);
        assert_ne!(
            cert_a.reference_output_blake3,
            cert_b.reference_output_blake3
        );
    }

    #[test]
    fn changing_program_changes_bundle_hash() {
        let prog_a = copy_first_program();
        let prog_b = {
            // Copy with a different entry node (store to output[1])
            // to produce a different wire hash.
            Program::wrapped(
                vec![
                    BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                        .with_count(4),
                    BufferDecl::storage("output", 1, BufferAccess::ReadWrite, DataType::U32)
                        .with_count(2),
                ],
                [1, 1, 1],
                vec![Node::store(
                    "output",
                    Expr::u32(1),
                    Expr::load("input", Expr::u32(0)),
                )],
            )
        };
        let cert_a = issue_bundle_cert(&prog_a, &sample_corpus(), "t", "s", "p").unwrap();
        let cert_b = issue_bundle_cert(&prog_b, &sample_corpus(), "t", "s", "p").unwrap();
        assert_ne!(cert_a.bundle_blake3, cert_b.bundle_blake3);
    }

    #[test]
    fn reference_self_verifies() {
        let program = copy_first_program();
        let corpus = sample_corpus();
        let cert = issue_bundle_cert(&program, &corpus, "t", "s", "p").unwrap();
        verify_bundle_against_reference(&cert, &program, &corpus)
            .expect("Fix: reference self-verifies; restore this invariant before continuing.");
    }

    #[test]
    fn bundle_cert_accepts_logical_witness_order_after_output_buffer() {
        let program = output_first_copy_program();
        let corpus = vec![CorpusWitness {
            name: "logical-input-only".into(),
            inputs: vec![bytes_u32(&[0xA5A5_5A5A])],
        }];

        let cert = issue_bundle_cert(&program, &corpus, "t", "s", "p")
            .expect("Fix: bundle issue must plan logical witness inputs, not raw buffer order.");
        verify_bundle_against_reference(&cert, &program, &corpus)
            .expect("Fix: bundle verify must reuse the same planned witness stream as issue.");
    }

    #[test]
    fn bundle_backend_verifier_accepts_logical_witness_order_after_output_buffer() {
        let program = output_first_copy_program();
        let corpus = vec![CorpusWitness {
            name: "backend-logical-input-only".into(),
            inputs: vec![bytes_u32(&[0xFEED_FACE])],
        }];
        let cert = issue_bundle_cert(&program, &corpus, "t", "s", "p")
            .expect("Fix: bundle issue must certify planned logical witness inputs.");
        let backend = vyre_driver_reference::CpuRefBackend;

        verify_bundle_with_backend(&cert, &program, &backend, &corpus).expect(
            "Fix: bundle backend verification must dispatch the planned logical witness stream.",
        );
    }

    #[test]
    fn bundle_cert_rejects_omitted_runtime_sized_read_write_witness() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "scratch",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let corpus = vec![CorpusWitness {
            name: "missing-runtime-scratch".into(),
            inputs: Vec::new(),
        }];

        let error = issue_bundle_cert(&program, &corpus, "t", "s", "p")
            .expect_err("Fix: omitted runtime-sized read-write witnesses must reject.");

        assert!(
            matches!(error, BundleCertError::WitnessPlanningFailed { .. }),
            "Fix: planner errors must stay explicit, got: {error}"
        );
        assert!(
            error
                .to_string()
                .contains("runtime-sized read-write buffer"),
            "Fix: planner error must name the dynamic read-write contract, got: {error}"
        );
    }

    #[test]
    fn verify_catches_bundle_drift() {
        let program = copy_first_program();
        let corpus = sample_corpus();
        let cert = issue_bundle_cert(&program, &corpus, "t", "s", "p").unwrap();

        let drifted = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("output", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(2),
            ],
            [1, 1, 1],
            vec![Node::store(
                "output",
                Expr::u32(1),
                Expr::load("input", Expr::u32(0)),
            )],
        );
        let err = verify_bundle_against_reference(&cert, &drifted, &corpus)
            .expect_err("bundle drift must reject");
        assert!(matches!(err, BundleCertError::BundleHashMismatch { .. }));
    }

    #[test]
    fn verify_catches_corpus_drift() {
        let program = copy_first_program();
        let corpus = sample_corpus();
        let cert = issue_bundle_cert(&program, &corpus, "t", "s", "p").unwrap();

        let mut drifted_corpus = corpus.clone();
        drifted_corpus[0].inputs[0] = bytes_u32(&[42, 42, 42, 42]);
        let err = verify_bundle_against_reference(&cert, &program, &drifted_corpus)
            .expect_err("corpus drift must reject");
        assert!(matches!(err, BundleCertError::CorpusHashMismatch { .. }));
    }

    #[test]
    fn verify_catches_output_drift() {
        // Craft a cert whose reference_output_blake3 is wrong, then
        // assert verify surfaces OutputHashMismatch  -  not
        // BundleHashMismatch / CorpusHashMismatch.
        let program = copy_first_program();
        let corpus = sample_corpus();
        let mut cert = issue_bundle_cert(&program, &corpus, "t", "s", "p").unwrap();
        cert.reference_output_blake3 = "00".repeat(32);
        let err = verify_bundle_against_reference(&cert, &program, &corpus)
            .expect_err("output drift must reject");
        assert!(matches!(err, BundleCertError::OutputHashMismatch { .. }));
    }
}

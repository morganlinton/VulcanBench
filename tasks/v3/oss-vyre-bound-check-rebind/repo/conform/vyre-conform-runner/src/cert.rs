//! Conformance certificate.
//!
//! Emitted by the runner after verifying an op satisfies its laws on a
//! backend. Byte-identical across backends (modulo `backend_id`) = portable op.

use serde::{Deserialize, Serialize};
use vyre::ir::OpId;

fn serialize_op_id<S>(op_id: &OpId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(op_id.as_ref())
}

fn deserialize_op_id<'de, D>(deserializer: D) -> Result<OpId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let op_id = String::deserialize(deserializer)?;
    Ok(op_id.into())
}

/// Conformance certificate for one (op, backend) pair.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Certificate {
    /// Certificate format version (follows vyre minor).
    pub version: String,
    /// Op identifier.
    #[serde(
        serialize_with = "crate::cert::serialize_op_id",
        deserialize_with = "crate::cert::deserialize_op_id"
    )]
    pub op_id: OpId,
    /// Wire format version at run time.
    pub wire_format_version: u32,
    /// `blake3(program.to_wire())`.
    pub program_blake3: String,
    /// `blake3(sorted witness inputs)`.
    pub witness_set_blake3: String,
    /// Backend that produced the cert.
    pub backend_id: String,
    /// Backend crate version string.
    pub backend_version: String,
    /// Laws verified to hold.
    pub laws_verified: Vec<String>,
    /// ISO 8601 UTC timestamp string.
    pub timestamp: String,
    /// Ed25519 signature over the canonical JSON body (hex).
    pub signature_ed25519: String,
    /// Ed25519 public key (hex).
    pub pubkey: String,
}

impl Certificate {
    /// Fresh certificate with the provided fields and "TBD" signature fields.
    #[must_use]
    pub fn new(
        op_id: impl Into<OpId>,
        backend_id: impl Into<String>,
        backend_version: impl Into<String>,
        laws_verified: Vec<String>,
    ) -> Self {
        Self {
            version: "0.4.1".to_string(),
            op_id: op_id.into(),
            wire_format_version: 1,
            program_blake3: "TBD".to_string(),
            witness_set_blake3: "TBD".to_string(),
            backend_id: backend_id.into(),
            backend_version: backend_version.into(),
            laws_verified,
            timestamp: "1970-01-01T00:00:00Z".to_string(),
            signature_ed25519: "TBD".to_string(),
            pubkey: "TBD".to_string(),
        }
    }

    /// Serialize to canonical JSON.
    ///
    /// # Errors
    /// Returns the underlying serde error when serialization fails.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

/// P5.4  -  Input to [`issue_certificate`]. The issuer runs the
/// witness corpus through the target backend and through the CPU
/// reference, then supplies the pair to this helper which
/// computes `program_blake3`, `witness_set_blake3`, the UTC
/// timestamp, and fills in the provided signature fields.
#[derive(Debug, Clone)]
pub struct IssueInput<'a> {
    /// Stable op id  -  matches the op's fingerprint entry.
    pub op_id: &'a OpId,
    /// `to_wire()` bytes of the **canonicalized** Program.
    pub program_wire_bytes: &'a [u8],
    /// Concatenated witness-input bytes (sorted for determinism).
    pub witness_bytes: &'a [u8],
    /// Backend that produced the outputs.
    pub backend_id: &'a str,
    /// `backend.version()` at issue time.
    pub backend_version: &'a str,
    /// Algebraic laws the runner verified hold on the witness set.
    pub laws_verified: Vec<String>,
    /// UTC ISO-8601 timestamp ("2026-04-20T00:00:00Z").
    pub timestamp: &'a str,
    /// Ed25519 signature over the canonical JSON body (hex).
    pub signature_ed25519: &'a str,
    /// Ed25519 public key (hex).
    pub pubkey: &'a str,
}

/// Compute an OCC certificate from runner inputs. Fills every
/// derived field; the caller supplies the signature + pubkey.
///
/// # Errors
///
/// Returns [`CertificateError::EmptyProgramWire`] if the program
/// bytes are empty (indicating a serialization bug upstream) or
/// [`CertificateError::EmptyWitnessSet`] if no witnesses ran.
pub fn issue_certificate(input: IssueInput<'_>) -> Result<Certificate, CertificateError> {
    if input.program_wire_bytes.is_empty() {
        return Err(CertificateError::EmptyProgramWire);
    }
    if input.witness_bytes.is_empty() {
        return Err(CertificateError::EmptyWitnessSet);
    }
    let program_blake3 = blake3::hash(input.program_wire_bytes).to_hex().to_string();
    let witness_set_blake3 = blake3::hash(input.witness_bytes).to_hex().to_string();

    Ok(Certificate {
        version: "0.4.1".to_string(),
        op_id: input.op_id.clone(),
        wire_format_version: 1,
        program_blake3,
        witness_set_blake3,
        backend_id: input.backend_id.to_string(),
        backend_version: input.backend_version.to_string(),
        laws_verified: input.laws_verified,
        timestamp: input.timestamp.to_string(),
        signature_ed25519: input.signature_ed25519.to_string(),
        pubkey: input.pubkey.to_string(),
    })
}

/// P5.4  -  Structural verification of an OCC. Checks that every
/// field is populated with something other than the "TBD"
/// sentinel and the structural fingerprints parse as 64-char hex.
/// Cryptographic signature verification is a separate step that
/// requires an `ed25519_dalek`-style verifier; kept out of the
/// core crate so the base dep graph stays minimal.
///
/// # Errors
///
/// [`CertificateError::UnsetField`] when any field still carries
/// the "TBD" sentinel; [`CertificateError::BadFingerprint`] when
/// a blake3 field isn't 64 hex chars.
pub fn verify_structural(cert: &Certificate) -> Result<(), CertificateError> {
    for (name, value) in [
        ("program_blake3", &cert.program_blake3),
        ("witness_set_blake3", &cert.witness_set_blake3),
        ("signature_ed25519", &cert.signature_ed25519),
        ("pubkey", &cert.pubkey),
    ] {
        if value == "TBD" {
            return Err(CertificateError::UnsetField(name.to_string()));
        }
    }
    for (name, value) in [
        ("program_blake3", &cert.program_blake3),
        ("witness_set_blake3", &cert.witness_set_blake3),
    ] {
        if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CertificateError::BadFingerprint(name.to_string()));
        }
    }
    Ok(())
}

/// Errors from OCC issuing / structural verification.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CertificateError {
    /// `program_wire_bytes` was empty at issue time  -  upstream
    /// serialization bug.
    #[error("empty program wire bytes  -  Program::to_wire() failed upstream")]
    EmptyProgramWire,
    /// Witness set was empty at issue time  -  nothing to certify.
    #[error("empty witness set  -  no witnesses ran through the backend")]
    EmptyWitnessSet,
    /// A cert field is still the "TBD" sentinel.
    #[error("cert field `{0}` is still set to the reserved value 'TBD'")]
    UnsetField(String),
    /// A blake3 fingerprint field isn't 64 hex chars.
    #[error("cert field `{0}` is not a 64-char hex fingerprint")]
    BadFingerprint(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let op_id: OpId = "primitive.bitwise.xor".into();
        let cert = Certificate::new(
            op_id,
            "backend-a",
            "0.5.0",
            vec!["Commutative".to_string(), "Associative".to_string()],
        );
        let json = cert.to_json().unwrap();
        let back: Certificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, back);
    }

    #[test]
    fn issue_populates_fingerprints_and_timestamp() {
        let sig = "01".repeat(64);
        let key = "02".repeat(32);
        let op_id: OpId = "vyre-libs::nn::softmax".into();
        let input = IssueInput {
            op_id: &op_id,
            program_wire_bytes: b"wire-bytes",
            witness_bytes: b"witness-bytes",
            backend_id: "backend-a",
            backend_version: "24.0.5",
            laws_verified: vec!["Commutative".to_string()],
            timestamp: "2026-04-20T00:00:00Z",
            signature_ed25519: &sig,
            pubkey: &key,
        };
        let cert = issue_certificate(input).unwrap();
        assert_eq!(cert.op_id.as_ref(), "vyre-libs::nn::softmax");
        assert_eq!(cert.backend_id, "backend-a");
        assert_eq!(cert.program_blake3.len(), 64);
        assert_eq!(cert.witness_set_blake3.len(), 64);
        assert_eq!(cert.timestamp, "2026-04-20T00:00:00Z");
    }

    #[test]
    fn issue_rejects_empty_program_wire() {
        let op_id: OpId = "x".into();
        let input = IssueInput {
            op_id: &op_id,
            program_wire_bytes: b"",
            witness_bytes: b"w",
            backend_id: "b",
            backend_version: "v",
            laws_verified: vec![],
            timestamp: "2026-04-20T00:00:00Z",
            signature_ed25519: "a",
            pubkey: "b",
        };
        assert!(matches!(
            issue_certificate(input),
            Err(CertificateError::EmptyProgramWire)
        ));
    }

    #[test]
    fn verify_structural_catches_tbd_sentinel() {
        let cert = Certificate::new(OpId::from("x"), "backend-a", "0.5.0", vec![]);
        let err = verify_structural(&cert).unwrap_err();
        assert!(matches!(err, CertificateError::UnsetField(_)));
    }

    #[test]
    fn verify_structural_accepts_real_cert() {
        let sig = "ab".repeat(32);
        let key = "cd".repeat(16);
        let op_id: OpId = "x".into();
        let input = IssueInput {
            op_id: &op_id,
            program_wire_bytes: b"p",
            witness_bytes: b"w",
            backend_id: "backend-a",
            backend_version: "24.0.5",
            laws_verified: vec![],
            timestamp: "2026-04-20T00:00:00Z",
            signature_ed25519: &sig,
            pubkey: &key,
        };
        let cert = issue_certificate(input).unwrap();
        verify_structural(&cert).expect("Fix: issued cert must pass structural verify; restore this invariant before continuing.");
    }
}

//! Parity-cert artifact (TEST-034).
//!
//! See `contracts/release.md`. A reviewer must be able to run ONE
//! command and produce a signed JSON certificate that proves every op
//! passes every registered backend's byte-identity dispatch against the
//! CPU reference. `prove --out` is the load-bearing gate: it MUST
//! refuse to emit a certificate when any (backend, op) pair diverges
//! from `vyre-reference` byte-for-byte. Acquisition success is not
//! parity  -  TEST-034 was filed because the earlier implementation
//! stopped at `backend.factory()` and never dispatched anything.

use std::process::Command;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::Value;

fn selected_backend_override() -> Option<String> {
    std::env::var("VYRE_BACKEND")
        .ok()
        .filter(|value| !value.trim().is_empty())
}


fn write_signed_shard(
    path: &std::path::Path,
    catalog_hash: &str,
    execution_hash: &str,
    program_hash: &str,
    pairs: Value,
) {
    let pairs_array = pairs
        .as_array()
        .expect("Fix: synthetic test pairs must be an array");
    let plan = serde_json::json!({
        "backend_count": 1,
        "op_count": pairs_array.len(),
        "pair_count": pairs_array.len(),
        "witness_case_count": pairs_array.len(),
        "catalog_hash": catalog_hash,
        "execution_hash": execution_hash,
        "selection": {
            "backend_filter": "cuda",
            "ops_filter": "all",
            "shard_index": 0,
            "shard_count": 2,
            "universe_backend_count": 3,
            "universe_op_count": 2,
            "selected_backend_count": 1,
            "selected_op_count": pairs_array.len()
        }
    });
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let signable = serde_json::json!({
        "wire_format_version": 1u32,
        "program_hash": program_hash,
        "backend_id": "all",
        "plan": plan,
        "pairs": pairs,
    });
    let signable_bytes =
        serde_json::to_vec(&signable).expect("Fix: synthetic shard should serialize");
    let signature = key.sign(&signable_bytes);
    let artifact = serde_json::json!({
        "wire_format_version": 1u32,
        "program_hash": program_hash,
        "backend_id": "all",
        "plan": signable["plan"].clone(),
        "signature": hex::encode(signature.to_bytes()),
        "public_key": hex::encode(key.verifying_key().to_bytes()),
        "pairs": signable["pairs"].clone(),
    });
    std::fs::write(
        path,
        serde_json::to_string_pretty(&artifact).expect("Fix: synthetic shard should serialize"),
    )
    .expect("Fix: synthetic shard should be writable");
}

fn verify_certificate_signature(parsed: &Value) {
    let signature_hex = parsed["signature"]
        .as_str()
        .expect("Fix: certificate must carry signature");
    let public_key_hex = parsed["public_key"]
        .as_str()
        .expect("Fix: certificate must carry public_key");
    let signature_bytes =
        hex::decode(signature_hex).expect("Fix: certificate signature must be hex");
    let public_key_bytes =
        hex::decode(public_key_hex).expect("Fix: certificate public key must be hex");
    let signature = Signature::from_slice(&signature_bytes)
        .expect("Fix: certificate signature must be a 64-byte Ed25519 signature");
    let public_key_array: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .expect("Fix: certificate public key must be 32 bytes");
    let verifying_key = VerifyingKey::from_bytes(&public_key_array)
        .expect("Fix: certificate public key must be a valid Ed25519 verifying key");
    let signable = serde_json::json!({
        "wire_format_version": parsed["wire_format_version"].clone(),
        "program_hash": parsed["program_hash"].clone(),
        "backend_id": parsed["backend_id"].clone(),
        "plan": parsed["plan"].clone(),
        "pairs": parsed["pairs"].clone(),
    });
    let signable_bytes =
        serde_json::to_vec(&signable).expect("Fix: certificate signable body must serialize");
    verifying_key
        .verify(&signable_bytes, &signature)
        .expect("Fix: certificate Ed25519 signature must verify over the canonical body");
}

#[path = "cert_artifact/prove_failure_contracts.rs"]
mod prove_failure_contracts;
#[path = "cert_artifact/gpu_certificate_contracts.rs"]
mod gpu_certificate_contracts;
#[path = "cert_artifact/shard_plan_contracts.rs"]
mod shard_plan_contracts;
#[path = "cert_artifact/merge_contracts.rs"]
mod merge_contracts;
#[path = "cert_artifact/runtime_efficiency_contracts.rs"]
mod runtime_efficiency_contracts;
#[path = "cert_artifact/release_script_contracts.rs"]
mod release_script_contracts;

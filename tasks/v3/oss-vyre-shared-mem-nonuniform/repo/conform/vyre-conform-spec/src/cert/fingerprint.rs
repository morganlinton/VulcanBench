//! Backend capability and behavior fingerprinting.
//!
//! Fingerprints are deterministic hashes of observed backend behavior, not
//! marketing names. A driver update that changes subgroup size or numeric
//! behavior must produce a different fingerprint so cached kernels and cert
//! results cannot drift silently.

use std::fmt;

/// Raw observations collected from a backend probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeObservation {
    /// Backend implementation family, such as `wgpu` or `cuda`.
    pub backend: String,
    /// Adapter or device identity normalized by the probe.
    pub adapter: String,
    /// Observed subgroup width used by compute kernels.
    pub subgroup_size: u32,
    /// Rounding-mode signature from deterministic arithmetic probes.
    pub rounding_signature: u64,
    /// Maximum observed transcendental error in ULPs.
    pub transcendental_ulp: u32,
}

impl ProbeObservation {
    /// Build a probe observation.
    #[must_use]
    pub fn new(
        backend: impl Into<String>,
        adapter: impl Into<String>,
        subgroup_size: u32,
        rounding_signature: u64,
        transcendental_ulp: u32,
    ) -> Self {
        Self {
            backend: backend.into(),
            adapter: adapter.into(),
            subgroup_size,
            rounding_signature,
            transcendental_ulp,
        }
    }
}

/// Stable backend behavior fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BackendFingerprint {
    digest_hex: String,
}

impl BackendFingerprint {
    /// Compute a fingerprint from observed backend behavior.
    ///
    /// CRITIQUE_CONFORM_2026-04-23 M3: the earlier version used
    /// `\0`-delimited `format!` concatenation, which let an attacker
    /// inject a `\0` into `backend` or `adapter` to shift delimiter
    /// boundaries and collide with a different configuration
    /// (example: `backend="wgpu\0v1", adapter="nvidia"` vs
    /// `backend="wgpu", adapter="v1nvidia"` produced the same
    /// canonical bytes).
    ///
    /// Switch to a length-prefixed canonical encoding: each variable
    /// field is prefixed with its 8-byte little-endian length so two
    /// distinct field splits cannot ever produce the same byte
    /// sequence, regardless of NUL bytes or other delimiter
    /// candidates in the payload.
    #[must_use]
    pub fn from_observation(observation: &ProbeObservation) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"vyre.conform.fingerprint.v2");
        let backend_bytes = observation.backend.as_bytes();
        let adapter_bytes = observation.adapter.as_bytes();
        hasher.update(&(backend_bytes.len() as u64).to_le_bytes());
        hasher.update(backend_bytes);
        hasher.update(&(adapter_bytes.len() as u64).to_le_bytes());
        hasher.update(adapter_bytes);
        hasher.update(&observation.subgroup_size.to_le_bytes());
        hasher.update(&observation.rounding_signature.to_le_bytes());
        hasher.update(&observation.transcendental_ulp.to_le_bytes());
        Self {
            digest_hex: hasher.finalize().to_hex().to_string(),
        }
    }

    /// Hex digest suitable for cache keys and cert manifests.
    #[must_use]
    #[inline]
    pub fn as_hex(&self) -> &str {
        &self.digest_hex
    }
}

impl fmt::Display for BackendFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.digest_hex)
    }
}

//! Tier 2.5 hash primitives.
//!
//! The path IS the interface. Callers write
//! `vyre_primitives::hash::fnv1a::fnv1a32(...)`  -  explicit paths;
//! no wildcard re-exports. See `docs/primitives-tier.md` and
//! `docs/lego-block-rule.md`.

/// FNV-1a 32-bit + 64-bit hash primitives.
pub mod fnv1a;

/// Shared BLAKE3 mix/round helpers.
pub mod blake3;

/// CRC-32 (IEEE 802.3 polynomial 0xEDB88320) hash primitive.
pub mod crc32;

/// Adler-32 checksum primitive.
pub mod adler32;

/// Fused CRC-32 + FNV-1a32 + Adler-32 one-pass primitive.
pub mod multi_hash;

/// Hash table primitives.
pub mod table;

/// Vector Symbolic Architecture (VSA) primitives  -  bind + bundle on
/// 10K-dim binary hypervectors. The same Programs serve retrieval,
/// reasoning, and content-addressable Program fingerprint compositions.
pub mod hypervector;

/// Count-Sketch  -  Charikar 2002 frequency-moment estimator. Same
/// Program serves streaming, observability, and profiler latency-distribution
/// sketching.
pub mod sketch;

/// Number-Theoretic Transform  -  exact-integer FFT over GF(p) for
/// FHE / zk / lattice crypto. CPU + per-stage butterfly Program.
/// 32-bit prime variant; 64-bit Goldilocks ships with U64 buffers.
pub mod ntt;

/// Hassanieh-Indyk-Katabi-Price sparse FFT bin-hash primitive (#49).
/// Sparse audio, radio, and imaging analysis composition block.
pub mod sparse_fft;

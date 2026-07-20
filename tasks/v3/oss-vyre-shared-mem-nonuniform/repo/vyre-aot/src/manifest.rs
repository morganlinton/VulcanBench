//! Submission-bundle manifest.
//!
//! The manifest is the single JSON file the launcher reads at startup to
//! discover artifact layout, dispatch config, and metadata. Bundles are
//! self-describing through this file  -  vyre is not present at run-time.

use serde::{Deserialize, Serialize};

use crate::artifact::{BufferEntry, DispatchGeometry, Target};

/// Top-level manifest. Written to `manifest.json` in the bundle root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema version. Bump when the on-disk format changes.
    pub schema: String,

    /// vyre-aot version that emitted this bundle.
    pub aot_version: String,

    /// Free-form name (e.g. "pgolf-vyre-megakernel").
    pub artifact_name: String,

    /// Target backend.
    pub target: Target,

    /// Kernel entry point.
    pub entry_point: String,

    /// Dispatch config baked into the artifact.
    pub dispatch: DispatchGeometry,

    /// Compressed kernel-bytes filename within the bundle.
    pub kernel_file: String,

    /// Compressed weights filename within the bundle.
    pub weights_file: String,

    /// Compression for the kernel file (e.g. "lzma", "none").
    pub kernel_compression: String,

    /// Compression for the weights file (e.g. "brotli-11", "none").
    pub weights_compression: String,

    /// Buffer table.
    pub buffers: Vec<BufferEntry>,

    /// SHA-256 of the uncompressed kernel bytes (hex).
    pub kernel_sha256_hex: String,

    /// SHA-256 of the uncompressed weights bytes (hex).
    pub weights_sha256_hex: String,

    /// Free-form notes the submission writeup may consume.
    #[serde(default)]
    pub notes: String,

    /// VSA fingerprint of the optimized Program (8-lane u32 hypervector)
    /// produced by `vyre_driver::program_vsa_fingerprint`. Two
    /// bundles emitted from semantically-equivalent Programs (modulo
    /// instruction order, commutative-operand swaps) share this
    /// fingerprint, so external caches can dedup without inspecting
    /// kernel bytes. Empty when the producer didn't compute one.
    #[serde(default)]
    pub vsa_fingerprint: Vec<u32>,
}

impl Manifest {
    /// Schema version this build of vyre-aot writes.
    pub const SCHEMA_VERSION: &'static str = "vyre-aot-manifest-v1";
}

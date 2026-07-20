//! Compiled artifact data model.
//!
//! A [`CompiledArtifact`] is what the compiler hands to a launcher: the
//! emitted GPU bytes plus everything the launcher needs to allocate, bind,
//! and dispatch  -  without re-running any vyre IR machinery at runtime.

use serde::{Deserialize, Serialize};

pub use vyre_driver::Target;

/// Memory tier for a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferMemoryKind {
    /// Global HBM/VRAM.
    Global,
    /// Workgroup-shared scratch.
    Shared,
    /// Read-only constant memory.
    Constant,
}

/// Access mode for a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferAccessKind {
    /// Read-only.
    ReadOnly,
    /// Write-only (used for output buffers).
    WriteOnly,
    /// Read and write (used for accumulators, optimizer state).
    ReadWrite,
}

/// One buffer in the compiled binding table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferEntry {
    /// Stable name from the source `Program`.
    pub name: String,
    /// Binding index in the kernel signature.
    pub binding: u32,
    /// Element count declared at build time. `0` for streaming/unbounded.
    pub element_count: u32,
    /// Element size in bytes (4 for u32/f32, 1 for u8/bool, etc.).
    pub element_size_bytes: u32,
    /// Memory tier.
    pub memory_kind: BufferMemoryKind,
    /// Access mode.
    pub access: BufferAccessKind,
}

impl BufferEntry {
    /// Total size in bytes the launcher should allocate.
    pub fn total_bytes(&self) -> u64 {
        u64::from(self.element_count) * u64::from(self.element_size_bytes)
    }
}

/// Test-friendly alias for [`DispatchGeometry`]  -  the integration test
/// surface (manifest_round_trip, launcher_contracts, etc.) was written
/// against an older `DispatchConfig` name. Keep the alias at the public
/// surface so test files import the historical path; the canonical type
/// remains [`DispatchGeometry`].
pub type DispatchConfig = DispatchGeometry;

/// Dispatch geometry baked into the artifact.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DispatchGeometry {
    /// Workgroup size (X, Y, Z).
    pub workgroup_size: [u32; 3],
    /// Grid size (X, Y, Z) at compile-time. `0` indicates the launcher
    /// computes it at run-time from a control-buffer field.
    pub grid_size: [u32; 3],
    /// Bytes of dynamic shared memory.
    pub dynamic_shared_bytes: u32,
}

/// The output of [`mod@crate::compile`]: everything the launcher needs to
/// reconstruct and execute the original `Program` without vyre at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledArtifact {
    /// Target GPU bytecode flavor.
    pub target: Target,
    /// The emitted target-native bytes.
    pub kernel_bytes: Vec<u8>,
    /// Entry-point function name.
    pub entry_point: String,
    /// Buffer-binding table (ordered by binding index).
    pub buffers: Vec<BufferEntry>,
    /// Dispatch config baked in at compile time.
    pub dispatch: DispatchGeometry,
    /// SemVer string of vyre-aot that produced this artifact.
    pub aot_version: String,
    /// VSA fingerprint of the optimized Program (8-lane u32 hypervector)
    /// produced by `vyre_driver::program_vsa_fingerprint`.
    /// Two artifacts that differ only in non-semantic detail (instruction
    /// order, commutative-operand ordering) share this fingerprint, so
    /// downstream caches can dedup without recomputing.
    #[serde(default)]
    pub vsa_fingerprint: Vec<u32>,
}

impl CompiledArtifact {
    /// Total declared bytes across all buffers (excludes streaming/unbounded).
    pub fn total_buffer_bytes(&self) -> u64 {
        self.buffers.iter().map(BufferEntry::total_bytes).sum()
    }
}

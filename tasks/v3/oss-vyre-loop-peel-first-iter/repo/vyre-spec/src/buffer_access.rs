//! Frozen buffer-access tags used by operation and program metadata.

/// Buffer access mode in the frozen data contract.
///
/// Example: `BufferAccess::ReadWrite` records that a storage buffer may be
/// both read and written by a lowered operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum BufferAccess {
    /// Read-only storage buffer.
    ReadOnly,
    /// Read-write storage buffer.
    ReadWrite,
    /// Uniform buffer: small, read-only, and fast path.
    Uniform,
    /// Write-only storage buffer.
    WriteOnly,
    /// Workgroup-local shared memory.
    Workgroup,
}

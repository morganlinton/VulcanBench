//! Primitive CPU references for the BinOp and UnOp expressions that every GPU
//! backend must lower identically.
//!
//! These functions exist so the parity engine has a deterministic, driver-independent
//! ground truth for integer arithmetic, bitwise logic, and comparisons. If a backend
//! produces a different result for any of these operations, the conform gate emits a
//! concrete byte-level divergence.

/// Read up to the first 4 bytes of `input` as a little-endian `u32`, zero-padding.
pub(super) fn read_u32_prefix(bytes: &[u8]) -> u32 {
    let mut padded = [0u8; 4];
    let len = bytes.len().min(4);
    padded[..len].copy_from_slice(&bytes[..len]);
    u32::from_le_bytes(padded)
}

/// Read up to the first 8 bytes of `input` as a little-endian `u64`, zero-padding.
pub(super) fn read_u64_prefix(bytes: &[u8]) -> u64 {
    let mut padded = [0u8; 8];
    let len = bytes.len().min(8);
    padded[..len].copy_from_slice(&bytes[..len]);
    u64::from_le_bytes(padded)
}

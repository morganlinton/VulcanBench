//! Quantized packing primitives.
//!
//! This module gives the spec-level `I4`/quantized type family executable
//! behavior: signed INT4 lanes are packed two per byte in a u32 word stream.
//! GPU kernels operate on u32 storage words so each word carries eight signed
//! 4-bit values.

mod cpu;
mod program_helpers;
mod programs;
#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "cpu-parity"))]
pub use cpu::{
    i4x8_batched_matmul_f32_scaled_cpu, i4x8_batched_matmul_top1_f32_scaled_cpu,
    i4x8_batched_matvec_f32_scaled_cpu, i4x8_dot_f32_scaled_cpu, i4x8_dot_i32_cpu,
    i4x8_matvec_f32_scaled_cpu, try_unpack_i4x8_cpu_into, unpack_i4x8_cpu, unpack_i4x8_cpu_into,
};
#[cfg(any(test, feature = "cpu-parity"))]
pub use cpu::{pack_i4x8_cpu, pack_i4x8_cpu_into, try_pack_i4x8_cpu_into};
pub use programs::{
    i4x8_batched_matmul_f32_scaled, i4x8_batched_matmul_top1_f32_scaled,
    i4x8_batched_matvec_f32_scaled, i4x8_dot_f32_scaled, i4x8_dot_i32, i4x8_matvec_f32_scaled,
    unpack_i4x8,
};

/// Canonical op id for packed signed INT4 unpacking.
pub const UNPACK_I4_OP_ID: &str = "vyre-primitives::math::quantized::unpack_i4x8";

/// Canonical op id for packed signed INT4 dot products.
pub const I4_DOT_I32_OP_ID: &str = "vyre-primitives::math::quantized::i4x8_dot_i32";

/// Canonical op id for fused scaled packed signed INT4 dot products.
pub const I4_DOT_F32_SCALED_OP_ID: &str = "vyre-primitives::math::quantized::i4x8_dot_f32_scaled";

/// Canonical op id for fused scaled packed signed INT4 matrix-vector products.
pub const I4_MATVEC_F32_SCALED_OP_ID: &str =
    "vyre-primitives::math::quantized::i4x8_matvec_f32_scaled";

/// Canonical op id for batched fused scaled packed signed INT4 matvec.
pub const I4_BATCHED_MATVEC_F32_SCALED_OP_ID: &str =
    "vyre-primitives::math::quantized::i4x8_batched_matvec_f32_scaled";

/// Canonical op id for batched fused scaled packed signed INT4 matmul.
pub const I4_BATCHED_MATMUL_F32_SCALED_OP_ID: &str =
    "vyre-primitives::math::quantized::i4x8_batched_matmul_f32_scaled";

/// Canonical op id for fused packed signed INT4 batched matmul top-1 routing.
pub const I4_BATCHED_MATMUL_TOP1_F32_SCALED_OP_ID: &str =
    "vyre-primitives::math::quantized::i4x8_batched_matmul_top1_f32_scaled";

/// Number of signed 4-bit lanes per packed u32 word.
pub const I4_LANES_PER_WORD: u32 = 8;

/// Number of packed signed INT4 words required for `lane_count` lanes.
#[must_use]
pub const fn i4_packed_words(lane_count: u32) -> u32 {
    lane_count.div_ceil(I4_LANES_PER_WORD)
}

#[cfg(feature = "inventory-registry")]
fn u32s(words: &[u32]) -> Vec<u8> {
    crate::prelude::pack_u32_slice(words)
}

#[cfg(feature = "inventory-registry")]
fn i32s(lanes: &[i32]) -> Vec<u8> {
    crate::prelude::pack_i32_slice(lanes)
}

#[cfg(feature = "inventory-registry")]
fn f32s(floats: &[f32]) -> Vec<u8> {
    crate::prelude::pack_f32_slice(floats)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        UNPACK_I4_OP_ID,
        || unpack_i4x8("packed_words", "out_lanes", 8),
        Some(|| vec![vec![u32s(&[0x7621_0F98])]]),
        Some(|| vec![vec![i32s(&[-8, -7, -1, 0, 1, 2, 6, 7])]]),
    ).with_category("math")
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        I4_DOT_I32_OP_ID,
        || i4x8_dot_i32("lhs_packed", "rhs_packed", "out", 8),
        Some(|| vec![vec![u32s(&[0x7621_0F98]), u32s(&[0x7621_0F98])]]),
        Some(|| vec![vec![i32s(&[204])]]),
    ).with_category("math")
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        I4_DOT_F32_SCALED_OP_ID,
        || i4x8_dot_f32_scaled("lhs_packed", "rhs_packed", "lhs_scale", "rhs_scale", "out", 8),
        Some(|| vec![vec![u32s(&[0x7621_0F98]), u32s(&[0x7621_0F98]), f32s(&[1.0]), f32s(&[1.0])]]),
        Some(|| vec![vec![f32s(&[204.0])]]),
    ).with_category("math")
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
        crate::harness::OpEntry::new(
            I4_MATVEC_F32_SCALED_OP_ID,
            || i4x8_matvec_f32_scaled("matrix_packed", "vector_packed", "matrix_scale", "vector_scale", 4, 8),
            Some(|| vec![vec![
                u32s(&[0x7621_0F98; 4]),
                f32s(&[-8.0, -7.0, -1.0, 0.0, 1.0, 2.0, 6.0, 7.0]),
                f32s(&[1.0; 4]),
                f32s(&[1.0]),
            ]]),
        Some(|| vec![vec![f32s(&[204.0; 4])]]),
    ).with_category("math")
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
        crate::harness::OpEntry::new(
            I4_BATCHED_MATVEC_F32_SCALED_OP_ID,
            || i4x8_batched_matvec_f32_scaled("matrix_packed", "vector_packed", "matrix_scale", "vector_scale", 2, 4, 8),
            Some(|| vec![vec![
                u32s(&[0x7621_0F98; 4]),
                f32s(&[-8.0, -7.0, -1.0, 0.0, 1.0, 2.0, 6.0, 7.0, -8.0, -7.0, -1.0, 0.0, 1.0, 2.0, 6.0, 7.0]),
                f32s(&[1.0; 4]),
                f32s(&[1.0; 2]),
            ]]),
            Some(|| vec![vec![f32s(&[204.0; 8])]]),
    ).with_category("math")
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
        crate::harness::OpEntry::new(
            I4_BATCHED_MATMUL_F32_SCALED_OP_ID,
            || i4x8_batched_matmul_f32_scaled("lhs_packed", "rhs_packed", "lhs_scale", "rhs_scale", "out", 2, 4, 8),
            Some(|| vec![vec![
                u32s(&[0x7621_0F98; 4]),
                u32s(&[0x7621_0F98; 2]),
                f32s(&[1.0; 4]),
                f32s(&[1.0; 2]),
            ]]),
            Some(|| vec![vec![f32s(&[204.0; 8])]]),
    ).with_category("math")
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
        crate::harness::OpEntry::new(
            I4_BATCHED_MATMUL_TOP1_F32_SCALED_OP_ID,
            || i4x8_batched_matmul_top1_f32_scaled("lhs_packed", "rhs_packed", "lhs_scale", "rhs_scale", "out_scores", 2, 4, 8),
            Some(|| vec![vec![
                u32s(&[0x7621_0F98; 4]),
                u32s(&[0x7621_0F98; 2]),
                f32s(&[1.0; 4]),
                f32s(&[1.0; 2]),
            ]]),
        Some(|| vec![vec![f32s(&[204.0, 204.0, 0.0, 0.0])]]),
    ).with_category("math")
}

//! Quantization sub-dialect for Parameter Golf recipe.
//!
//! Contains int4 dot, int6/int8 pack/unpack, byte shuffle, GPTQ-SDClip,
//! and GGML K-Quants ops.
pub mod byte_shuffle;
pub mod ggml;
pub mod gptq;
pub mod int4;
pub mod int6;
pub mod int8;

pub use byte_shuffle::byte_shuffle;
pub use ggml::{
    q2_k_linear, q2_k_unpack, q4_k_linear, q4_k_unpack, Q2_K_BLOCKS_PER_SUPER, Q2_K_BLOCK_SIZE,
    Q2_K_SUPER_BLOCK_SIZE, Q4_K_BLOCKS_PER_SUPER, Q4_K_BLOCK_SIZE, Q4_K_SUPER_BLOCK_SIZE,
};
pub use gptq::{gptq_round, gptq_sdclip};
pub use int4::{
    int4_batched_matmul_f32_scaled, int4_batched_matmul_top1_f32_scaled,
    int4_batched_matvec_f32_scaled, int4_dot_f32_scaled, int4_dot_i32, int4_matvec_f32_scaled,
};
pub use int6::{int6_pack, int6_unpack};
pub use int8::{int8_pack, int8_unpack};

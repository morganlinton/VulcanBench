//! Neural-net primitives  -  activation, linear, normalization, attention,
//! optimizer, quantization.
//!
//! Each function is a Category-A composition over vyre-ops primitives
//! and lower-level `vyre-libs::math` functions.
//!
//! Organized into sub-dialects:
//! - `activation`  -  ReLU, LeakyReLU², LogitSoftcap, CrossEntropy, Embedding, SkipGate
//! - `linear`  -  affine linear layer
//! - `norm`  -  LayerNorm, RMSNorm, LayerwiseLNScale
//! - `attention`  -  softmax, scaled_dot_product_attention, QKGain, PartialRoPE
//! - `optim`  -  EMA, AdamW, Muon, Newton-Schulz, MuonEq-R
//! - `quant`  -  int6, int8 pack/unpack, byte_shuffle, GPTQ-SDClip
//!
//! Flat re-exports preserve the pre-0.6 API surface.

#[cfg(feature = "nn-activation")]
pub mod activation;

#[cfg(feature = "nn-activation")]
pub mod backward;

#[cfg(feature = "nn-linear")]
pub mod linear;

#[cfg(feature = "nn-norm")]
pub mod norm;

#[cfg(feature = "nn-attention")]
pub mod attention;

#[cfg(feature = "nn-moe")]
pub mod moe;

#[cfg(any(feature = "nn-linear", feature = "nn-norm"))]
pub(crate) mod rms;

#[cfg(any(
    feature = "nn-inference",
    all(
        feature = "nn-activation",
        feature = "nn-linear",
        feature = "nn-norm",
        feature = "nn-attention",
        feature = "nn-moe"
    )
))]
pub mod inference_graph;

#[cfg(feature = "nn-activation")]
pub mod optim;

#[cfg(feature = "nn-activation")]
pub mod quant;

// Flat re-exports for back-compat.
#[cfg(feature = "nn-activation")]
pub use activation::relu;
#[cfg(feature = "nn-attention")]
pub use attention::{
    attention, attention_reference, flash_attention_2, flash_attention_2_reference, softmax,
    softmax_reference, Attention, Softmax,
};
#[cfg(feature = "nn-activation")]
pub use backward::{
    leaky_relu_sq_backward, ln_scale_backward, logit_softcap_backward, mlp_backward,
    partial_rope_backward, qk_gain_backward, residual_block_backward, skip_gate_backward,
};
#[cfg(feature = "nn-linear")]
pub use linear::{linear, linear_relu, linear_silu, linear_tiled, Linear};
#[cfg(feature = "nn-linear-4bit")]
pub use linear::{
    linear_4bit_affine_grouped_planner_evidence, linear_4bit_affine_grouped_typed,
    QuantizedLinear4BitPlannerEvidence, QuantizedLinear4BitSpec,
    LINEAR_4BIT_AFFINE_GROUPED_OUTPUT_DRIFT_ABS_TOLERANCE,
};
#[cfg(feature = "nn-norm")]
pub use norm::{layer_norm, rms_norm, rms_norm_reference, LayerNorm};

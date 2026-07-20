//! Backward passes for Parameter Golf forward ops.
//!
//! Each backward op takes the forward inputs + output grad and produces
//! input grads. All ops are F32, region-wrapped, inventory-registered.

pub mod leaky_relu_sq_backward;
pub mod ln_scale_backward;
pub mod logit_softcap_backward;
pub mod mlp_backward;
pub mod partial_rope_backward;
pub mod qk_gain_backward;
pub mod residual_block_backward;
pub mod skip_gate_backward;
mod unary_f32;

pub use leaky_relu_sq_backward::leaky_relu_sq_backward;
pub use ln_scale_backward::ln_scale_backward;
pub use logit_softcap_backward::logit_softcap_backward;
pub use mlp_backward::mlp_backward;
pub use partial_rope_backward::partial_rope_backward;
pub use qk_gain_backward::qk_gain_backward;
pub use residual_block_backward::residual_block_backward;
pub use skip_gate_backward::skip_gate_backward;

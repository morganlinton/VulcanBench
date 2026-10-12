//! Activation sub-dialect + utility nn ops.
pub mod cross_entropy;
pub mod embedding;
pub mod gelu;
pub mod leaky_relu_sq;
pub mod logit_softcap;
pub mod mlp_4x_leaky_sq;
pub mod parallel_residual_block;
pub mod relu;
pub mod silu;
pub mod skip_gate;
pub mod swiglu;
pub(crate) mod unary;

pub use cross_entropy::{cross_entropy, try_cross_entropy};
pub use embedding::embedding;
pub use gelu::gelu;
pub use leaky_relu_sq::leaky_relu_sq;
pub use logit_softcap::logit_softcap;
pub use mlp_4x_leaky_sq::mlp_4x_leaky_sq;
pub use parallel_residual_block::parallel_residual_block;
pub use relu::relu;
pub use silu::silu;
pub use skip_gate::skip_gate;
pub use swiglu::swiglu;

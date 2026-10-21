//! Mixture-of-Experts (MoE) sub-dialect.
pub mod expert_mlp;
pub mod gating;
pub mod moe_layer;
pub mod softmax_top_k;
pub mod top_k;
mod topk_selection;

pub use expert_mlp::expert_mlp;
pub use gating::moe_gate;
pub use moe_layer::moe_layer_route_and_accumulate;
pub use softmax_top_k::softmax_top_k;
pub use top_k::top_k;

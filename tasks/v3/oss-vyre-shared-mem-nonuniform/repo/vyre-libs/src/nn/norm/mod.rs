//! Normalization sub-dialect: LayerNorm, RMSNorm, layerwise LN scale.
mod layer_norm;
pub mod layerwise_ln_scale;
mod rms_norm;

pub use layer_norm::{layer_norm, LayerNorm};
pub use layerwise_ln_scale::layerwise_ln_scale;
pub use rms_norm::{rms_norm, rms_norm_reference};

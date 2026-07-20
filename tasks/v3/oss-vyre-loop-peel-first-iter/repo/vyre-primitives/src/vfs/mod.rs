//! GPU-native virtual filesystem primitives.
//!
//! Async DMA resolvers for `#include`-style asset loading directly into
//! the L1 warp-arena without CPU staging.

/// Asynchronous `#include` hash → block-load resolver.
pub mod resolve;

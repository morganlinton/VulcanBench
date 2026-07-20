//! Comprehensive test suite for vyre-libs visual compositions.
//!
//! Tests validate:
//! - **Identity transforms**: confirm no-op when params are neutral
//! - **Program structure**: verify buffer declarations and region tagging
//! - **Edge cases**: zero-radius, 1-pixel, max-radius
//! - **Algebraic properties**: energy conservation, symmetry, commutativity
//! - **Pixel math correctness**: fixed-point arithmetic, clamp boundaries

#![allow(deprecated)]
#[cfg(feature = "visual")]
mod tests {
    include!("__split/visual_compositions_chunk1.rs");
    include!("__split/visual_compositions_chunk2.rs");
}

//! Cross-backend parity matrix: registered backends, wire shapes, and buffer comparison.
//!
//! Implementation lives in two `include!`-d chunks under `__split/`.
#![forbid(unsafe_code)]

include!("__split/parity_matrix_chunk1.rs");
include!("__split/parity_matrix_chunk2.rs");

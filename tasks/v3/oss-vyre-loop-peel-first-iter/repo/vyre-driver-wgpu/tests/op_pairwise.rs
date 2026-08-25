//! Pairwise op-composition proptest. Implementation lives in two
//! `include!`-d chunks under `__split/`.
#![allow(deprecated)]
include!("__split/op_pairwise_chunk1.rs");
include!("__split/op_pairwise_chunk2.rs");

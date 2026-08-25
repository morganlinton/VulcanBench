//! Megakernel protocol layout contracts  -  exact byte/word placement
//! and non-overlap. Implementation lives in two `include!`-d chunks
//! under `__split/`.
#![allow(clippy::assertions_on_constants)]

include!("__split/megakernel_protocol_layout_contracts_chunk1.rs");
include!("__split/megakernel_protocol_layout_contracts_chunk2.rs");

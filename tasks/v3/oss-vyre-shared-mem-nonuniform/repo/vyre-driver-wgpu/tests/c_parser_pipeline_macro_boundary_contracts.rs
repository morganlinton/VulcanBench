//! Generated wrapper test crate for c parser pipeline macro boundary contracts.
//!
//! Implementation lives in `__split/` chunks.
#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use common::{decode_u32_words, u32_bytes};

include!("__split/c_parser_pipeline_macro_boundary_contracts_chunk1.rs");
include!("__split/c_parser_pipeline_macro_boundary_contracts_chunk2.rs");

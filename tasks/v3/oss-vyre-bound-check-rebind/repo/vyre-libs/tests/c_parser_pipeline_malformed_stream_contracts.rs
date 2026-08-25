//! Adversarial contract tests for malformed streams and parser stage boundaries.
#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use common::u32_bytes;

include!("__split/c_parser_pipeline_malformed_stream_contracts_chunk1.rs");
include!("__split/c_parser_pipeline_malformed_stream_contracts_chunk2.rs");

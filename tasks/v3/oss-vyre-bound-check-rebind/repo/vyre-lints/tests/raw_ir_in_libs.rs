//! End-to-end tests for the `raw_ir_in_libs` lint.
//!
//! Each test writes a synthetic vyre-libs source file to a tempdir,
//! runs the lint, and asserts on the exact violation set.
//!
//! Implementation lives in two `include!`-d chunks under `__split/`.

include!("__split/raw_ir_in_libs_chunk1.rs");
include!("__split/raw_ir_in_libs_chunk2.rs");

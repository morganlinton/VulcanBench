//! # vyre-grammar-gen
//!
//! Host-side C11 grammar table generator for the vyre GPU C parser.
//! Produces DFA lexer + LR(1) action/goto tables as binary blobs that
//! `vyre-libs::parsing` loads as ReadOnly storage buffers.
//!
//! See `README.md` for the pipeline and binary-blob wire format.
//!
//! ## Safe defaults
//!
//! **Input size:** No hard cap enforced by the library. `DfaBuilder::new(states,
//! classes)` allocates `states * classes * 4` bytes for the transition table;
//! callers are responsible for bounding those dimensions. `decode_dfa_from_bytes`
//! and `decode_lr_from_bytes` read only the slice they are handed - no unbounded
//! allocation beyond what the `payload_len` header field indicates.
//!
//! **Recursion depth:** No recursion in any public library function. All
//! algorithms (`preprocess_c_host`, DFA construction, wire encoding/decoding) are
//! iterative; stack depth is O(1) with respect to input size.
//!
//! **Outbound network:** None. The library makes no network calls. All I/O is
//! via the caller-supplied byte slices or `String`/`Vec` return values.
//!
//! **Process spawning:** None. The library never spawns child processes or
//! invokes `std::process`. The `vyre-grammar-gen` binary in `src/main.rs`
//! writes files to disk, but the library itself does not.
//!
//! **Filesystem writes:** None by the library. `preprocess_c_host`,
//! `build_c11_lexer_dfa`, `PackedBlob::from_dfa`, `PackedBlob::from_lr`
//! (which returns `Result<Self, String>` and validates table dimensions before
//! packing), and all decode functions operate purely in memory and return
//! owned values. Only the `main.rs` binary writes files (via `std::fs::write`).
//!
//! **Credential exposure:** None. No credentials, tokens, or secrets are
//! read, logged, or transmitted. BLAKE3-128 is used solely for payload
//! integrity verification of binary blobs, not for any authentication purpose.

#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::todo, clippy::unimplemented, clippy::panic))]
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate, clippy::missing_errors_doc)]

pub mod c11_lexer;
pub mod chunk_lexer_cpu;
pub mod dfa;
pub mod host_preprocess;
pub mod lex_c11_max_munch;
pub mod lr;
pub mod max_munch_cpu;
pub mod wire;

pub use c11_lexer::{build_c11_lexer_dfa, build_c11_lexer_dfa_for_host, C11_PATTERNS};
pub use chunk_lexer_cpu::count_chunked_valid_tokens;
pub use dfa::{DfaBuilder, DfaTable};
pub use host_preprocess::preprocess_c_host;
pub use lex_c11_max_munch::lex_c11_max_munch_kinds;
pub use lr::{validate_lr_table, LrBuilder, LrTable};
pub use max_munch_cpu::{kinds_blake3, LexCpuError};
pub use wire::{decode_dfa_from_bytes, decode_lr_from_bytes, BlobKind, PackedBlob, WireError};

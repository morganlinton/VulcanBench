//! Decode / decompression compositions for GPU-resident pipelines.
//!
//! These builders keep encoded bytes in the same IR surface used by the
//! matching kernels so decode→scan chains can stay on-device.

pub mod base64;
mod buffers;
pub mod encodex;
pub mod hex;
pub mod inflate;
mod scan;
pub mod ziftsieve;

/// Streaming decode → scan adapter. Fuses a decoder
/// Program with a scanner Program so decoded bytes hand off through
/// workgroup-shared memory instead of a DRAM round-trip.
pub mod streaming;

pub use base64::{base64_decode, base64_decode_then_aho_corasick, BASE64_DECODE_TABLE_BUFFER};
pub use encodex::{encodex_gpu, encodex_reference};
pub use hex::{
    hex_decode, hex_decode_table, hex_decode_then_aho_corasick, HEX_DECODE_TABLE_BUFFER,
};
pub use inflate::{
    inflate, inflate_stored_block, inflate_stored_block_buffered_then_aho_corasick,
    inflate_stored_block_then_aho_corasick, inflate_stored_block_tiled_then_aho_corasick,
    inflate_then_aho_corasick,
};
pub use ziftsieve::ziftsieve_gpu;
#[cfg(any(test, feature = "cpu-parity"))]
pub use ziftsieve::ziftsieve_reference_extract_literals;

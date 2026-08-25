//! Cat-A hash / checksum compositions.
//!
//! Owns the checksum and hash compositions migrated from the old
//! target-text-string op surface. Each op is a pure serial composition over
//! existing IR primitives (XOR + multiply + shift); no dedicated target builder
//! emitter arm required.

pub mod adler32;
pub mod blake3_compress;
pub mod crc32;
pub mod fnv1a32;
pub mod fnv1a64;
pub mod multi_hash;
mod wrap;

pub use adler32::adler32;
pub use blake3_compress::blake3_compress;
pub use crc32::crc32;
pub use fnv1a32::fnv1a32;
pub use fnv1a64::fnv1a64;
pub use multi_hash::multi_hash;

#[cfg(test)]
pub(crate) fn pack_bytes_as_u32(bytes: &[u8]) -> Vec<u8> {
    vyre_primitives::wire::pack_bytes_as_u32_slice(bytes)
}

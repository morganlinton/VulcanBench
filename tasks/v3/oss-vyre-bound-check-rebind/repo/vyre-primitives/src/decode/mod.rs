//! Tier 2.5 decode primitives.

/// Base64 decode kernel body.
pub mod base64;
/// ASCII hex decode kernel body.
pub mod hex;
/// DEFLATE stored-block inflate kernel body.
pub mod inflate;
/// RLE-segment-length scan + start-position prefix-sum (#P-PRIM-RLE).
/// Foundational primitive for block-oriented compression decoders
/// (LZ4 literal/match runs, zstd FSE literal counts, PNG IDAT chunks,
/// snappy raw runs). Unpacks `(length, value)` from packed u32 segment
/// headers  -  the prefix-sum that produces per-segment output start
/// offsets is `math::prefix_scan` (#5).
pub mod rle_segment_lengths;
/// Indexed LZ4 literal-copy stage for parallel block decoders.
pub mod ziftsieve;

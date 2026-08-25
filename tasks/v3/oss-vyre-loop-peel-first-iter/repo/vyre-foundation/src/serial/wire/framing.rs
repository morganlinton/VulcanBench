//! Primitive framing helpers shared by the wire encoder and decoder.
#![allow(unused_doc_comments)]

/// Canonical wire-header flag bits shared by the encoder and decoder.
pub(crate) use super::tags::{FLAG_COMPRESSED, FLAG_OPAQUE_ENDIAN_FIXED, FLAG_SEALED};
/// Wire-format magic bytes and schema version constant.
///
/// `MAGIC` is the four-byte envelope tag `VIR0`; `WIRE_FORMAT_VERSION`
/// is the little-endian `u16` that immediately follows it. Both are
/// written by `Program::to_wire` and validated by `Program::from_wire`
/// before any payload is decoded. A mismatch surfaces an actionable
/// `Fix:` error rather than an opaque downstream parse failure
/// (audit L.1.47).
pub use magic::{MAGIC, WIRE_FORMAT_VERSION};

/// Append a checked sequence length to the wire buffer.
///
/// Converts `usize` → `u32` so the format stays platform-independent.
/// Every variable-length payload (node list, expression tree, string)
/// is prefixed with this length field.
///
/// # Errors
///
/// Returns an actionable error when `value` does not fit in 32 bits.
pub use put_len_u32::put_len_u32;

/// Append a bounded length-prefixed UTF-8 string.
///
/// The string length is encoded via [`put_len_u32()`] and the raw bytes
/// follow. Used for buffer names, operation identifiers, and other
/// human-readable tokens carried by the IR.
///
/// # Errors
///
/// Returns an actionable error when the string exceeds
/// `MAX_STRING_LEN` or the length cannot fit in the encoded field.
pub use put_string::put_string;

/// Append one little-endian `u32` to the output buffer.
///
/// Used for scalar numeric payloads (tag discriminants, buffer ids,
/// work-group sizes). The wire format is little-endian on all targets.
pub use put_u32::put_u32;

/// Append one raw byte to the output buffer.
///
/// Used for tag bytes and small enum discriminants where a full
/// `u32` would waste four bytes.
pub use put_u8::put_u8;

/// Low-level byte-reader methods for the wire decoder.
///
/// These methods advance `Reader::pos`, bounds-check every access,
/// and produce `Fix:`-prefixed errors on truncation or version skew.
/// They are the symmetric counterpart of the `put_*` encoder helpers.
pub(crate) mod impl_reader;

/// Wire-format magic and version constants.
///
/// Defines the `VIR0` magic tag and the current schema version.
/// Audit L.1.47: version mismatch is detected immediately after magic
/// validation so callers receive an actionable error rather than
/// arbitrary downstream parse failures.
pub mod magic;

/// Length-field encoder for wire-format sequences.
///
/// Bridges Rust `usize` (platform pointer size) to the fixed `u32`
/// length field used in VIR0. Overflow is rejected with a `Fix:` hint
/// so that hostile or oversized programs cannot be serialized.
pub mod put_len_u32;

/// UTF-8 string encoder for the IR wire format.
///
/// Bounds string lengths against `MAX_STRING_LEN` before writing
/// the length-prefixed payload. Guarantees that every encoded string
/// can be decoded without unbounded allocation.
pub mod put_string;

/// Little-endian `u32` framing helper.
///
/// Emits four little-endian bytes for scalar fields. This module
/// contains the canonical implementation; all `u32` wire fields go
/// through here so endianness is consistent across architectures.
pub mod put_u32;

/// Single-byte framing helper.
///
/// Emits one raw byte for compact discriminants. Used for enum tags
/// and boolean-like flags where a full multi-byte word is unnecessary.
pub mod put_u8;

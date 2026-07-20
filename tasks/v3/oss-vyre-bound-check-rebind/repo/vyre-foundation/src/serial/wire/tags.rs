//! Numeric tag conversion for IR wire-format enums.
#![allow(unused_doc_comments)]

/// Wire-header flag bit for compressed payloads.
pub(crate) const FLAG_COMPRESSED: u16 = 1;
/// Wire-header flag bit for sealed payloads.
pub(crate) const FLAG_SEALED: u16 = 1 << 1;
/// Wire-header flag bit asserting every opaque payload is endian-fixed.
///
/// When this bit is present, `Expr::Opaque` and `Node::Opaque` payloads are
/// guaranteed to encode multi-byte numerics with `to_le_bytes` and decode them
/// with `from_le_bytes`. Consumers reject blobs missing this bit so
/// host-endian extension payloads never silently cross architectures.
pub(crate) const FLAG_OPAQUE_ENDIAN_FIXED: u16 = 1 << 2;

mod op_tag_decode;

/// Decode an atomic operation tag from the wire stream.
///
/// Atomic op tags live inside `Expr::Atomic` wire payloads. An unrecognized
/// tag means the producer and consumer disagree on the VIR0 revision.
pub(crate) use atomic_op_from_tag::atomic_op_from_tag;
/// Encode an atomic operation enum for the wire stream.
///
/// Maps each `AtomicOp` to its stable VIR0 tag. Unknown variants are rejected
/// with an actionable error to prevent silent data loss on round-trip.
pub(crate) use atomic_op_tag::atomic_op_tag;
/// Decode a binary operation tag from the wire stream.
///
/// Binary-op tags appear in `Expr::BinOp` payloads. This is the decode
/// counterpart of [`bin_op_tag`]; the two tables must stay bitwise symmetric.
pub(crate) use bin_op_from_tag::bin_op_from_tag;
/// Encode a binary operation enum for the wire stream.
///
/// Every `BinOp` variant added to the spec must receive a unique tag here;
/// otherwise `Program::from_wire(Program::to_wire(p))` breaks (I4).
pub(crate) use bin_op_tag::bin_op_tag;
/// Decode a data type tag from the wire stream.
///
/// Scalar and tensor types map 1-to-1 to tags. `Array` is special-cased:
/// its tag (12) is rejected here because the element-size payload must be
/// read by `Reader::data_type` (see `impl_reader.rs`).
pub(crate) use data_type_from_tag::data_type_from_tag;
// `data_type_tag` was re-exported here but unused at the crate boundary  -
// callers use `put_data_type` which wraps it (ORPH-004). Kept private to
// its defining module to avoid a dead re-export.
/// Encode a data type enum and any required payload for the wire stream.
///
/// For `DataType::Array` this also writes the `element_size` u32 payload;
/// for all other types it emits exactly one tag byte.
pub(crate) use data_type_tag::put_data_type;
/// Decode a unary operation tag from the wire stream.
///
/// Unary-op tags appear in `Expr::UnOp` payloads. Decode/encode symmetry
/// with `un_op_tag` is required for lossless round-trip (I4).
pub(crate) use un_op_from_tag::un_op_from_tag;
/// Encode a unary operation enum for the wire stream.
///
/// Maps each `UnOp` variant to its stable VIR0 tag. Missing a mapping here
/// breaks round-trip for any program that declares the operator.
pub(crate) use un_op_tag::un_op_tag;

/// Decode a `BufferAccess` from its VIR0 wire tag.
///
/// See [`mod@access_tag`] for the inverse mapping. Tag stability is part of the
/// wire-format contract; new variants require a format revision.
pub mod access_from_tag;
/// Encode a `BufferAccess` into its VIR0 wire tag.
///
/// See [`mod@access_from_tag`] for the inverse mapping.
pub mod access_tag;
/// Decode an `AtomicOp` from its VIR0 wire tag.
///
/// See [`mod@atomic_op_tag`] for the inverse mapping.
pub mod atomic_op_from_tag;
/// Encode an `AtomicOp` into its VIR0 wire tag.
///
/// See [`mod@atomic_op_from_tag`] for the inverse mapping.
pub mod atomic_op_tag;
/// Decode a `BinOp` from its VIR0 wire tag.
///
/// See [`mod@bin_op_tag`] for the inverse mapping. Covers audit L.1.27 / I4.
pub mod bin_op_from_tag {
    pub(crate) use super::op_tag_decode::bin_op_from_tag;
}
/// Encode a `BinOp` into its VIR0 wire tag.
///
/// See [`mod@bin_op_from_tag`] for the inverse mapping. Covers audit L.1.27 / I4.
pub mod bin_op_tag;
/// Decode a `DataType` from its VIR0 wire tag.
///
/// See [`mod@data_type_tag`] for the inverse mapping. `Array` is handled at the
/// reader level because it carries an extra `element_size` payload.
pub mod data_type_from_tag;
/// Encode a `DataType` into its VIR0 wire tag and optional payload.
///
/// See [`mod@data_type_from_tag`] for the inverse mapping.
pub mod data_type_tag;
/// Decode a `UnOp` from its VIR0 wire tag.
///
/// See [`mod@un_op_tag`] for the inverse mapping. Covers audit L.1.27 / I4.
pub mod un_op_from_tag {
    pub(crate) use super::op_tag_decode::un_op_from_tag;
}
/// Encode a `UnOp` into its VIR0 wire tag.
///
/// See [`mod@un_op_from_tag`] for the inverse mapping. Covers audit L.1.27 / I4.
pub mod un_op_tag;

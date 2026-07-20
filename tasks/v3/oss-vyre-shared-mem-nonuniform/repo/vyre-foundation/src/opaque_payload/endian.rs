//! Endian-fixed encode/decode helpers for opaque extension payloads.
//!
//! `Expr::Opaque` and `Node::Opaque` carry a `Vec<u8>` payload owned by the
//! dialect that issued the node. The wire framing is byte-identical across
//! architectures, so extension authors MUST NOT use `to_ne_bytes` or any
//! host-endian serialization  -  a Program encoded on one host and decoded on
//! another must reproduce the same `crate::ir::Program::hash` and the same
//! IR. Using `to_le_bytes` everywhere is the only way to honour that
//! contract.
//!
//! This module collects the little-endian primitives an extension
//! implementor is most likely to reach for. They are thin wrappers around
//! the standard-library `to_le_bytes` / `from_le_bytes` methods, intended
//! to make the right choice the obvious one and surface actionable
//! diagnostics when an input buffer is truncated.
//!
//! See `vyre-foundation/tests/opaque_payload_endian.rs` for the regression
//! suite covering F-IR-32.

use std::fmt;

/// Error returned when a decoder is given fewer bytes than the fixed-width
/// integer or float it is trying to reconstruct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaquePayloadTruncated {
    /// Human-readable tag identifying the field that was short.
    pub field: &'static str,
    /// Number of bytes the decoder needed.
    pub expected: usize,
    /// Number of bytes that were actually available.
    pub available: usize,
}

impl fmt::Display for OpaquePayloadTruncated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "opaque payload truncated while reading `{}`: expected {} bytes, got {}. \
             Fix: the extension author's encoder must emit {} byte(s) for this field \
             via `to_le_bytes`; do not truncate or pack-substitute.",
            self.field, self.expected, self.available, self.expected,
        )
    }
}

impl std::error::Error for OpaquePayloadTruncated {}

macro_rules! impl_le_codec {
    ($ty:ty, $push:ident, $read:ident, $width:expr) => {
        #[doc = concat!("Append `value` to `buf` as `",
                                    stringify!($ty), "::to_le_bytes`.")]
        pub fn $push(buf: &mut Vec<u8>, value: $ty) {
            buf.extend_from_slice(&value.to_le_bytes());
        }

        #[doc = concat!("Read the first `",
                                    stringify!($width),
                                    "` bytes of `bytes` as a little-endian `", stringify!($ty),
                                    "`, returning the decoded value and the remaining tail. \
            # Errors\
\
            Returns [`OpaquePayloadTruncated`] if `bytes` is shorter than \
            the fixed width.")]
        #[expect(
            clippy::missing_errors_doc,
            reason = "macro-generated endian readers share the doc contract above"
        )]
        pub fn $read(bytes: &[u8]) -> Result<($ty, &[u8]), OpaquePayloadTruncated> {
            let width = $width;
            if bytes.len() < width {
                return Err(OpaquePayloadTruncated {
                    field: stringify!($ty),
                    expected: width,
                    available: bytes.len(),
                });
            }
            let (head, tail) = bytes.split_at(width);
            let mut array = [0u8; $width];
            array.copy_from_slice(head);
            Ok((<$ty>::from_le_bytes(array), tail))
        }
    };
}

impl_le_codec!(u16, push_u16, read_u16, 2);
impl_le_codec!(u32, push_u32, read_u32, 4);
impl_le_codec!(u64, push_u64, read_u64, 8);
impl_le_codec!(i16, push_i16, read_i16, 2);
impl_le_codec!(i32, push_i32, read_i32, 4);
impl_le_codec!(i64, push_i64, read_i64, 8);
impl_le_codec!(f32, push_f32, read_f32, 4);
impl_le_codec!(f64, push_f64, read_f64, 8);

/// A writer that ONLY exposes little-endian serialization methods.
///
/// `LeBytesWriter` wraps a `Vec<u8>` and prevents accidental use of
/// host-endian helpers such as `to_ne_bytes`. Extension authors are
/// encouraged to use this type when building opaque payloads so the
/// compiler rejects the wrong endianness at the call site.
///
/// # Example
///
/// ```
/// use vyre_foundation::opaque_payload::LeBytesWriter;
/// let mut w = LeBytesWriter::new();
/// w.push_u32(42);
/// w.push_f32(1.0);
/// let payload = w.into_inner();
/// ```
#[derive(Debug, Clone, Default)]
pub struct LeBytesWriter {
    buf: Vec<u8>,
}

impl LeBytesWriter {
    /// Create an empty writer.
    #[must_use]
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Create an empty writer with at least `capacity` bytes reserved.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    /// Append a little-endian `u16`.
    pub fn push_u16(&mut self, value: u16) {
        push_u16(&mut self.buf, value);
    }

    /// Append a little-endian `u32`.
    pub fn push_u32(&mut self, value: u32) {
        push_u32(&mut self.buf, value);
    }

    /// Append a little-endian `u64`.
    pub fn push_u64(&mut self, value: u64) {
        push_u64(&mut self.buf, value);
    }

    /// Append a little-endian `i16`.
    pub fn push_i16(&mut self, value: i16) {
        push_i16(&mut self.buf, value);
    }

    /// Append a little-endian `i32`.
    pub fn push_i32(&mut self, value: i32) {
        push_i32(&mut self.buf, value);
    }

    /// Append a little-endian `i64`.
    pub fn push_i64(&mut self, value: i64) {
        push_i64(&mut self.buf, value);
    }

    /// Append a little-endian `f32`.
    pub fn push_f32(&mut self, value: f32) {
        push_f32(&mut self.buf, value);
    }

    /// Append a little-endian `f64`.
    pub fn push_f64(&mut self, value: f64) {
        push_f64(&mut self.buf, value);
    }

    /// Append a raw byte slice (e.g. a tag or string body).
    pub fn push_slice(&mut self, slice: &[u8]) {
        self.buf.extend_from_slice(slice);
    }

    /// Consume the writer and return the underlying buffer.
    #[must_use]
    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }
}

impl From<LeBytesWriter> for Vec<u8> {
    fn from(writer: LeBytesWriter) -> Self {
        writer.buf
    }
}

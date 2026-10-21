//! Reusable on-wire envelope for `vyre`-level serializable types.
//!
//! Higher layers in the workspace ship their own little binary blobs:
//! `CompiledDfa` in `vyre-primitives`, `GpuLiteralSet` in `vyre-libs`,
//! and (next) `RulePipeline` plus downstream consumer-side caches. Each one
//! re-implemented the same four moves:
//!
//!   1. Write a 4-byte magic + LE u32 version header.
//!   2. Emit length-prefixed `&[u8]` sections.
//!   3. Emit length-prefixed `&[u32]` little-endian word arrays.
//!   4. Decode the same back, producing typed errors that distinguish
//!      "stale cache, recompile" from "blob is corrupt, refuse".
//!
//! This module is the lego block. One implementation, one set of typed
//! errors, one suite of round-trip / version-mismatch / truncation
//! tests  -  every consumer adopts it and its fixes propagate.
//!
//! # Layered usage
//!
//! - `WireWriter` builds the blob; consumers compose multiple sections
//!   in order.
//! - `WireReader` decodes; consumers pull the same sections in the same
//!   order.
//! - [`EnvelopeError`] carries every failure mode; consumers should
//!   forward it (or wrap into their own error enum) without redefining
//!   the variants.
//!
//! The envelope itself is **not** content-aware. Consumers wrap it with
//! their own magic + version constants so two unrelated payloads
//! (e.g. a DFA and a literal set) cannot be confused at decode time.

use std::error::Error;
use std::fmt;

/// Errors returned from [`WireReader`] decode operations. Variants are
/// non-exhaustive so additive framing variants stay backward-compatible.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum EnvelopeError {
    /// Payload ended before the requested section was fully read.
    Truncated {
        /// Byte offset the decoder needed to reach.
        needed: usize,
        /// Bytes actually present in the input slice.
        got: usize,
    },
    /// First four bytes did not match the consumer's expected magic.
    BadMagic {
        /// Magic the consumer expected.
        expected: [u8; 4],
        /// Magic actually present in the blob.
        found: [u8; 4],
    },
    /// Wire version header did not match the consumer's expected
    /// version. Primary signal for cache invalidation: a
    /// `VersionMismatch` is the consumer's cue to discard the cache and
    /// recompile from source.
    VersionMismatch {
        /// Wire version the consumer's build understands.
        expected: u32,
        /// Wire version recorded in the blob's header.
        found: u32,
    },
    /// A section or word-array length could not fit in the envelope's
    /// `u32` length prefix.
    SectionTooLarge {
        /// Length the caller attempted to encode.
        len: usize,
        /// Maximum length representable by the wire format.
        max: usize,
    },
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { needed, got } => write!(
                f,
                "wire envelope truncated: needed {needed} bytes, got {got}. \
                 Fix: regenerate the cache."
            ),
            Self::BadMagic { expected, found } => write!(
                f,
                "wire envelope magic mismatch: expected {expected:?}, found {found:?}. \
                 Fix: this blob was not produced by the matching consumer."
            ),
            Self::VersionMismatch { expected, found } => write!(
                f,
                "wire envelope version {found} does not match runtime {expected}. \
                 Fix: discard the cache and rebuild from source."
            ),
            Self::SectionTooLarge { len, max } => write!(
                f,
                "wire envelope section length {len} exceeds maximum {max}. \
                 Fix: split the payload into smaller sections."
            ),
        }
    }
}

impl Error for EnvelopeError {}

/// Build a typed binary blob with magic + version + sections.
///
/// Consumers create one writer, push sections in their declared order,
/// then call `into_bytes`. Section order is the consumer's contract;
/// the envelope itself only enforces the framing.
#[derive(Debug)]
pub struct WireWriter {
    out: Vec<u8>,
}

impl WireWriter {
    /// Start a writer with the given magic + version header. The header
    /// is emitted immediately so consumers can read offsets predictably.
    #[must_use]
    pub fn new(magic: &[u8; 4], version: u32) -> Self {
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(magic);
        out.extend_from_slice(&version.to_le_bytes());
        Self { out }
    }

    /// Append a length-prefixed byte section. The length is encoded as
    /// a little-endian `u32` (bound: 4 GiB per section).
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError::SectionTooLarge`] when the byte count cannot
    /// fit in the envelope's `u32` length prefix.
    pub fn write_section(&mut self, bytes: &[u8]) -> Result<(), EnvelopeError> {
        let len = u32::try_from(bytes.len()).map_err(|_| EnvelopeError::SectionTooLarge {
            len: bytes.len(),
            max: u32::MAX as usize,
        })?;
        self.out.extend_from_slice(&len.to_le_bytes());
        self.out.extend_from_slice(bytes);
        Ok(())
    }

    /// Append a length-prefixed `u32` word array. Each word is encoded
    /// little-endian; the prefix counts WORDS, not bytes.
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError::SectionTooLarge`] when the word count cannot
    /// fit in the envelope's `u32` length prefix.
    pub fn write_words(&mut self, words: &[u32]) -> Result<(), EnvelopeError> {
        let len = u32::try_from(words.len()).map_err(|_| EnvelopeError::SectionTooLarge {
            len: words.len(),
            max: u32::MAX as usize,
        })?;
        self.out.extend_from_slice(&len.to_le_bytes());
        for w in words {
            self.out.extend_from_slice(&w.to_le_bytes());
        }
        Ok(())
    }

    /// Append a single little-endian `u32`. Useful for fixed-width
    /// header fields (state counts, capability flags, etc.) that don't
    /// need a length prefix.
    pub fn write_u32(&mut self, value: u32) {
        self.out.extend_from_slice(&value.to_le_bytes());
    }

    /// Consume the writer and return the underlying bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.out
    }
}

/// Decode a typed binary blob produced by [`WireWriter`].
#[derive(Debug)]
pub struct WireReader<'a> {
    src: &'a [u8],
    cursor: usize,
}

impl<'a> WireReader<'a> {
    /// Begin a reader; validates the 8-byte magic + version header.
    /// Consumers MUST call this and propagate the error before reading
    /// any sections  -  sections after a bad header cannot be trusted.
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError::Truncated`] for incomplete headers,
    /// [`EnvelopeError::BadMagic`] for magic mismatches, or
    /// [`EnvelopeError::VersionMismatch`] for stale/corrupt versions.
    pub fn new(
        bytes: &'a [u8],
        expected_magic: &[u8; 4],
        expected_version: u32,
    ) -> Result<Self, EnvelopeError> {
        if bytes.len() < 8 {
            return Err(EnvelopeError::Truncated {
                needed: 8,
                got: bytes.len(),
            });
        }
        let mut found_magic = [0u8; 4];
        found_magic.copy_from_slice(&bytes[0..4]);
        if &found_magic != expected_magic {
            return Err(EnvelopeError::BadMagic {
                expected: *expected_magic,
                found: found_magic,
            });
        }
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != expected_version {
            return Err(EnvelopeError::VersionMismatch {
                expected: expected_version,
                found: version,
            });
        }
        Ok(Self {
            src: bytes,
            cursor: 8,
        })
    }

    /// Read a length-prefixed byte section.
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError::Truncated`] when the length prefix or section
    /// bytes exceed the remaining input.
    pub fn read_section(&mut self) -> Result<&'a [u8], EnvelopeError> {
        let n = self.read_u32()? as usize;
        if self.src.len() < self.cursor + n {
            return Err(EnvelopeError::Truncated {
                needed: self.cursor + n,
                got: self.src.len(),
            });
        }
        let slice = &self.src[self.cursor..self.cursor + n];
        self.cursor += n;
        Ok(slice)
    }

    /// Read a length-prefixed `u32` word array.
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError::Truncated`] when the length prefix or encoded
    /// words exceed the remaining input.
    pub fn read_words(&mut self) -> Result<Vec<u32>, EnvelopeError> {
        let n_words = self.read_u32()? as usize;
        let bytes_needed = n_words * 4;
        if self.src.len() < self.cursor + bytes_needed {
            return Err(EnvelopeError::Truncated {
                needed: self.cursor + bytes_needed,
                got: self.src.len(),
            });
        }
        let mut v = Vec::with_capacity(n_words);
        for _ in 0..n_words {
            let w = u32::from_le_bytes([
                self.src[self.cursor],
                self.src[self.cursor + 1],
                self.src[self.cursor + 2],
                self.src[self.cursor + 3],
            ]);
            v.push(w);
            self.cursor += 4;
        }
        Ok(v)
    }

    /// Read a single little-endian `u32` (no length prefix).
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError::Truncated`] when fewer than four bytes remain.
    pub fn read_u32(&mut self) -> Result<u32, EnvelopeError> {
        if self.src.len() < self.cursor + 4 {
            return Err(EnvelopeError::Truncated {
                needed: self.cursor + 4,
                got: self.src.len(),
            });
        }
        let n = u32::from_le_bytes([
            self.src[self.cursor],
            self.src[self.cursor + 1],
            self.src[self.cursor + 2],
            self.src[self.cursor + 3],
        ]);
        self.cursor += 4;
        Ok(n)
    }
}

/// Generic round-trip / robustness assertion helpers for any
/// wire-format consumer.
///
/// Every type that ships its own `to_bytes` / `from_bytes` pair on top
/// of this envelope used to write the same five tests:
///   1. `round_trip`
///   2. `rejects_bad_magic`
///   3. `rejects_version_mismatch`
///   4. `rejects_truncated_header`
///   5. `rejects_truncated_section`
///
/// These helpers reduce that to one call per type. Consumers call
/// `assert_envelope_roundtrip(&value)` and the helper drives the full
/// suite. The bound `T: WireRoundTrip` is provided by consumers as a
/// thin trait that exposes the type's `to_bytes` / `from_bytes` plus
/// its declared magic + version.
pub mod test_helpers {
    use super::{EnvelopeError, WireWriter};

    /// Adapter trait consumers implement to plug their wire format
    /// into [`assert_envelope_roundtrip`]. The `to_bytes` and
    /// `from_bytes` methods are forwarded to the type's own; the
    /// `MAGIC` / `VERSION` consts let the helpers fabricate
    /// deliberately-corrupted blobs.
    pub trait WireRoundTrip: Sized {
        /// Wire-format magic the type stamps on every blob.
        const MAGIC: [u8; 4];
        /// Wire version the type stamps on every blob.
        const VERSION: u32;
        /// Encoder error type. Not exercised here  -  consumers pre-
        /// validate that `to_bytes` returns `Ok` for the sample.
        type EncodeError: std::fmt::Debug;
        /// Decoder error type. Used to confirm that mutated blobs
        /// surface as typed errors instead of panics.
        type DecodeError: std::fmt::Debug;

        /// Encode a sample value.
        ///
        /// # Errors
        /// Forwarded from the type's own encoder.
        fn to_bytes(&self) -> Result<Vec<u8>, Self::EncodeError>;

        /// Decode a previously-encoded blob.
        ///
        /// # Errors
        /// Forwarded from the type's own decoder.
        fn from_bytes(bytes: &[u8]) -> Result<Self, Self::DecodeError>;

        /// Comparison hook so the helper can assert structural equality
        /// after a round trip without requiring `PartialEq` on the type
        /// itself (some engines hold non-comparable buffers / programs).
        fn structurally_eq(&self, other: &Self) -> bool;
    }

    /// Drive the standard wire-format assertion suite against `sample`.
    ///
    /// Asserts:
    ///   - encode succeeds
    ///   - decode of the encoded bytes returns a value that
    ///     `structurally_eq`s the original
    ///   - mutating the magic byte produces a typed decode error
    ///   - mutating the version dword produces a typed decode error
    ///   - truncating the trailing byte produces a typed decode error
    ///   - feeding an 8-byte buffer (header only, zero sections) is a
    ///     decoder concern  -  helper does NOT assert success/failure
    ///     because section-counts vary by consumer.
    ///
    /// Intentionally panics on assertion failure (this is a test
    /// helper, not a runtime path).
    ///
    /// # Panics
    ///
    /// Panics when encoding/decoding fails for the supplied valid sample, when
    /// the encoded header is malformed, or when corruption/truncation does not
    /// surface as a typed decode error.
    pub fn assert_envelope_roundtrip<T>(sample: &T)
    where
        T: WireRoundTrip + std::fmt::Debug,
    {
        let encoded = sample.to_bytes();
        assert!(
            encoded.is_ok(),
            "Fix: encode sample; restore this invariant before continuing: {encoded:?}"
        );
        let Ok(bytes) = encoded else {
            return;
        };
        assert!(
            bytes.len() >= 8,
            "wire blob must include at least the 8-byte header"
        );
        assert_eq!(
            &bytes[0..4],
            T::MAGIC.as_slice(),
            "magic mismatch in encoded blob"
        );
        let version_field = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let expected_version = T::VERSION;
        assert!(
            version_field == expected_version,
            "version mismatch in encoded blob: got {version_field}, expected {expected_version}"
        );

        let decoded = T::from_bytes(&bytes);
        assert!(
            decoded.is_ok(),
            "Fix: decode round trip; restore this invariant before continuing: {decoded:?}"
        );
        let Ok(back) = decoded else {
            return;
        };
        assert!(
            sample.structurally_eq(&back),
            "round-tripped value diverges from original"
        );

        // Mutate magic.
        let mut mutated = bytes.clone();
        mutated[0] ^= 0xFF;
        assert!(
            T::from_bytes(&mutated).is_err(),
            "mutated magic must surface as a typed error"
        );

        // Mutate version.
        let mut mutated = bytes.clone();
        let bumped = T::VERSION.wrapping_add(1);
        mutated[4..8].copy_from_slice(&bumped.to_le_bytes());
        assert!(
            T::from_bytes(&mutated).is_err(),
            "mutated version must surface as a typed error"
        );

        // Truncate one byte off the tail.
        if bytes.len() > 8 {
            let truncated = &bytes[..bytes.len() - 1];
            assert!(
                T::from_bytes(truncated).is_err(),
                "truncated trailing byte must surface as a typed error"
            );
        }
    }

    /// Helper for tests that want to fabricate blobs with arbitrary
    /// magic + version. Returns a header-only buffer (no sections).
    /// Useful for asserting that consumers reject empty-section blobs
    /// when their schema requires N sections.
    #[must_use]
    pub fn header_only(magic: &[u8; 4], version: u32) -> Vec<u8> {
        WireWriter::new(magic, version).into_bytes()
    }

    /// Confirm that the `EnvelopeError` matches an expected variant
    /// (without requiring the consumer's wrapper enum to expose
    /// `PartialEq`).
    ///
    /// # Panics
    ///
    /// Panics when `err` does not match the expected envelope-error category.
    pub fn assert_envelope_error_kind(err: &EnvelopeError, kind: ExpectedEnvelopeError) {
        let matches = matches!(
            (err, kind),
            (
                EnvelopeError::Truncated { .. },
                ExpectedEnvelopeError::Truncated
            ) | (
                EnvelopeError::BadMagic { .. },
                ExpectedEnvelopeError::BadMagic
            ) | (
                EnvelopeError::VersionMismatch { .. },
                ExpectedEnvelopeError::VersionMismatch
            ) | (
                EnvelopeError::SectionTooLarge { .. },
                ExpectedEnvelopeError::SectionTooLarge
            )
        );
        assert!(
            matches,
            "expected envelope error kind {kind:?}, got {err:?}"
        );
    }

    /// Variant tags for [`assert_envelope_error_kind`]. Mirrors
    /// [`EnvelopeError`] but is decoupled from the consumer's wrapper
    /// enum so they can match on it without re-exporting the
    /// variants.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExpectedEnvelopeError {
        /// `EnvelopeError::Truncated`
        Truncated,
        /// `EnvelopeError::BadMagic`
        BadMagic,
        /// `EnvelopeError::VersionMismatch`
        VersionMismatch,
        /// `EnvelopeError::SectionTooLarge`
        SectionTooLarge,
    }
}

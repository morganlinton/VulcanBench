//! IR serialization formats.
//!
//! Vyre programs are frozen data structures that must survive transmission,
//! caching, and versioning. This module defines the two stable serialization
//! formats: a compact binary wire format for machines and a canonical text
//! format for humans.

/// Canonical text representation.
///
/// The text format is human-readable and version-agnostic. It is used for
/// debugging, logging, and diffing IR in tests.
pub mod text;

/// Binary wire format.
///
/// The wire format is a compact little-endian byte stream designed for
/// network transmission and on-disk caching. Every validated `Program` can
/// be round-tripped through this format without loss.
pub mod wire;

/// Output set serialization.
///
/// Persistent encoding of which buffers are writable outputs, used by
/// the wire format and persistent cache layers.
pub mod output_set;

/// Reusable on-wire envelope: magic + version + length-prefixed
/// sections / word arrays. Higher-layer types (`CompiledDfa` in
/// vyre-primitives, `GpuLiteralSet` / `RulePipeline` in vyre-libs,
/// downstream consumer-side caches compose this primitive
/// instead of re-implementing magic / version / truncation handling.
/// One implementation, one set of typed errors, one suite of round-trip
/// tests  -  every consumer adopts it and its fixes propagate.
pub mod envelope;
pub use envelope::{EnvelopeError, WireReader, WireWriter};

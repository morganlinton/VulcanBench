//! Public API surface.
//!
//! Entry points are split by operation granularity so parsing, later
//! compilation, and batch APIs can evolve independently without growing a
//! catch-all public module.

pub mod entrypoints;
pub mod parse_summary;

pub use entrypoints::parse_rust_bytes;
pub use parse_summary::ParseSummary;

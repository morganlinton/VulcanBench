//! Witness sets + composition laws for vyre conformance testing.
//!
//! Canonical, deterministic witness enumeration per DataType. Consumers
//! use these to drive backend-parity testing and algebraic-law verification.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![deny(missing_docs)]

pub mod witness;

pub use witness::U32Witness;

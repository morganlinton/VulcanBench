//! Counterexample shrinking + witness generation for vyre conformance testing.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![deny(missing_docs)]

pub mod minimizer;

pub use minimizer::CounterexampleMinimizer;

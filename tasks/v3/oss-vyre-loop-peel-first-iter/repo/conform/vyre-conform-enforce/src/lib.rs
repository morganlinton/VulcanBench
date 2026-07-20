//! Algebraic-law prover for vyre ops over witness sets.
//!
//! Verifies commutativity / associativity / identity / distributivity by
//! running the op's compose function against witness tuples and flagging
//! any counterexample.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![deny(missing_docs)]

pub mod prover;

pub use prover::{LawProver, LawVerdict};

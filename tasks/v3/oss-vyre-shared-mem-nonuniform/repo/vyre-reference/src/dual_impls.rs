//! Standalone primitive-operation CPU references.
#![allow(missing_docs)]

/// docs
pub mod arith;
#[path = "dual_impls/bitwise/mod.rs"]
/// docs
pub mod bitwise;
/// docs
pub mod common;
/// docs
pub mod compare;
/// docs
pub mod hash;
mod indexed_reference_impls;
/// docs
pub mod memory;
mod scalar_reference_impls;
/// docs
pub mod scan;
/// docs
pub mod workgroup;
pub use common::{EvalError, ReferenceEvaluator};

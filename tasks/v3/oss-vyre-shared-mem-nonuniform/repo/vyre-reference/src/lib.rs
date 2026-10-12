#![forbid(unsafe_code)]
#![allow(
    clippy::only_used_in_recursion,
    clippy::comparison_chain,
    clippy::ptr_arg
)]
//! Pure Rust reference interpreter for vyre IR programs.
//!
//! This module is the executable specification for IR semantics. It is
//! intentionally slow and direct: every current IR expression and node variant
//! has a named evaluator function.

extern crate vyre_foundation as vyre;

/// Dual-reference trait and registry types.
pub mod dual;
/// Canonical dual implementations and reference evaluators.
pub mod dual_impls;
/// Runtime value representation for interpreter inputs and outputs.
pub mod value;

/// Atomic operation reference implementations.
pub mod atomics;
/// CPU operation traits used by concrete reference implementations.
pub mod cpu_op;
/// Registry-driven dispatch entry point (B-B4).
///
/// Routes an op id through the global `DialectRegistry` and invokes
/// the registered `cpu_ref` function. Complements the execution-tree
/// evaluators by giving external dialect crates a zero-patch path to run on
/// the reference interpreter.
pub mod dialect_dispatch;
/// Canonical reference execution tree.
pub mod execution;
/// Flat byte adapter used by [`crate::cpu_op::CpuOp`].
pub mod flat_cpu;
/// IEEE 754 strict floating-point utilities.
pub mod ieee754;
/// Subgroup simulator for lane-collective Cat-C ops.
pub mod subgroup;
/// Workgroup simulation: invocation IDs, shared memory.
pub mod workgroup;

mod oob;
mod ops;

/// Test-only entry point that runs the hashmap interpreter over a Program.
#[cfg(test)]
pub use execution::eval_hashmap_reference;
/// Execute a vyre Program on the pure Rust reference interpreter.
pub use execution::{reference_eval, run_arena_reference, run_storage_graph};

/// Resolve an operation ID to its two independently-written references.
///
/// # Examples
///
/// ```
/// use vyre_reference::{dual_impls, resolve_dual};
///
/// let (reference_a, reference_b) =
///     resolve_dual(dual_impls::bitwise::xor::OP_ID).expect("Fix: xor dual refs must be registered; restore this invariant before continuing.");
///
/// let input = [0b1010_1010_u8, 0b0101_0101];
/// assert_eq!(reference_a(&input), reference_b(&input));
/// ```
pub fn resolve_dual(op_id: &str) -> Option<(dual::ReferenceFn, dual::ReferenceFn)> {
    match op_id {
        dual_impls::arith::add::OP_ID => Some((
            dual_impls::arith::add::reference_a::reference,
            dual_impls::arith::add::reference_b::reference,
        )),
        dual_impls::arith::mul::OP_ID => Some((
            dual_impls::arith::mul::reference_a::reference,
            dual_impls::arith::mul::reference_b::reference,
        )),
        dual_impls::bitwise::xor::OP_ID => Some((
            dual_impls::bitwise::xor::reference_a::reference,
            dual_impls::bitwise::xor::reference_b::reference,
        )),
        dual_impls::bitwise::and::OP_ID => Some((
            dual_impls::bitwise::and::reference_a::reference,
            dual_impls::bitwise::and::reference_b::reference,
        )),
        dual_impls::bitwise::or::OP_ID => Some((
            dual_impls::bitwise::or::reference_a::reference,
            dual_impls::bitwise::or::reference_b::reference,
        )),
        dual_impls::bitwise::not::OP_ID => Some((
            dual_impls::bitwise::not::reference_a::reference,
            dual_impls::bitwise::not::reference_b::reference,
        )),
        dual_impls::bitwise::shift_left::OP_ID => Some((
            dual_impls::bitwise::shift_left::reference_a::reference,
            dual_impls::bitwise::shift_left::reference_b::reference,
        )),
        dual_impls::bitwise::shift_right::OP_ID => Some((
            dual_impls::bitwise::shift_right::reference_a::reference,
            dual_impls::bitwise::shift_right::reference_b::reference,
        )),
        dual_impls::bitwise::popcount::OP_ID => Some((
            dual_impls::bitwise::popcount::reference_a::reference,
            dual_impls::bitwise::popcount::reference_b::reference,
        )),
        dual_impls::bitwise::clz::OP_ID => Some((
            dual_impls::bitwise::clz::reference_a::reference,
            dual_impls::bitwise::clz::reference_b::reference,
        )),
        dual_impls::compare::eq::OP_ID => Some((
            dual_impls::compare::eq::reference_a::reference,
            dual_impls::compare::eq::reference_b::reference,
        )),
        dual_impls::compare::lt::OP_ID => Some((
            dual_impls::compare::lt::reference_a::reference,
            dual_impls::compare::lt::reference_b::reference,
        )),
        _ => None,
    }
}

/// Return the complete list of operation IDs that have dual references registered.
///
/// This is the canonical enumeration used by the differential fuzzing gate.
/// Every new dual-reference pair MUST add its OP_ID here.
pub fn dual_op_ids() -> &'static [&'static str] {
    &[
        dual_impls::arith::add::OP_ID,
        dual_impls::arith::mul::OP_ID,
        dual_impls::bitwise::xor::OP_ID,
        dual_impls::bitwise::and::OP_ID,
        dual_impls::bitwise::or::OP_ID,
        dual_impls::bitwise::not::OP_ID,
        dual_impls::bitwise::shift_left::OP_ID,
        dual_impls::bitwise::shift_right::OP_ID,
        dual_impls::bitwise::popcount::OP_ID,
        dual_impls::bitwise::clz::OP_ID,
        dual_impls::compare::eq::OP_ID,
        dual_impls::compare::lt::OP_ID,
    ]
}

/// The architecture of the `OpEntry` registry.
///
/// We are forced to split the global primitive registries into three separate
/// buckets (Unary, Binary, Variadic) instead of a single unified registry.
///
/// This split is required because of Rust's trait object lifetime limits.
/// When storing function pointers that take references (e.g., `&'a Node<'a>`),
/// higher-ranked trait bounds (HRTB, `for<'a>`) fail to unify on function
/// pointers with heterogeneous arities. A single registry `fn(&[Node])` slice
/// signature would force heap allocation for binary/unary nodes to fit the slice,
/// destroying the zero-allocation invariant of the reference interpreter.
///
/// Thus, we split by arity to allow zero-cost static dispatch.
pub mod registry_architecture {}

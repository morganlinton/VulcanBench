//! In-place expand-with-change-flag substrate consumer.
//!
//! Wires `vyre_primitives::graph::csr_forward_or_changed` so iterative
//! dataflow loops can detect convergence in a single pass: the primitive returns the next
//! frontier AND a boolean changed-flag. Used by reachability /
//! liveness / reaching-defs fixpoint passes that previously had to
//! diff before/after states by hand.

mod dispatch;

#[cfg(any(test, feature = "cpu-parity"))]
mod reference;

#[cfg(test)]
mod tests;

pub use dispatch::{
    forward_closure_via_change_flag_gpu, forward_closure_via_change_flag_gpu_into,
    forward_closure_via_change_flag_gpu_with_scratch_into, ForwardChangedGpuScratch,
};

#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::{
    reference_forward_closure_via_change_flag, reference_forward_closure_via_change_flag_into,
    reference_forward_step_with_change_flag,
};

#[cfg(test)]
pub(crate) use vyre_primitives::graph::csr_forward_or_changed::cpu_ref as csr_foc_cpu;

//! Tier 2.5 fixpoint primitives  -  driver-free convergence loops for
//! bitset transfer functions.
//!
//! The vision's taint/flow semantics all reduce to "iterate a
//! bitset transfer function until the output bitset stops growing."
//! This module packages that pattern as a single primitive:
//!
//! - `bitset_fixpoint`  -  canonical ping-pong with a convergence
//!   flag. One Program that the backend dispatches repeatedly; the
//!   harness / runtime loops until the flag clears or
//!   `max_iterations` is hit.
//! - `persistent_fixpoint`  -  single-dispatch convergence on the GPU.
//!   Wraps a caller-supplied transfer-step body in a forever-loop
//!   with the comparison + ping-pong + termination check inside the
//!   kernel. Replaces every "host iterates to fixpoint" docstring;
//!   convergence happens entirely on device.

pub mod bitset_fixpoint;
pub mod persistent_fixpoint;

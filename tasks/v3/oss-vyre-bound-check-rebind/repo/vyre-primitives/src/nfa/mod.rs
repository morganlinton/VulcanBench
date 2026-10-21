//! NFA primitives  -  subgroup-cooperative epsilon closure and
//! step simulation.
//!
//! G1 (GPU perf innovation #1) is a 32-state-per-subgroup NFA
//! simulator where each lane holds one `u32` state-set bit and
//! epsilon closure is subgroup ballot/shuffle bitwise-or. For NFAs
//! wider than 32 states, callers tile into 32-state windows and stream
//! the transition-table slice per tile.
//!
//! This file is the subsystem entry point. The primitive kernel
//! lives in `subgroup_nfa`; higher-level multi-string / regex scan
//! helpers compose it with tiling and table-building policy.

/// Subgroup-cooperative NFA simulation kernel.
pub mod subgroup_nfa;

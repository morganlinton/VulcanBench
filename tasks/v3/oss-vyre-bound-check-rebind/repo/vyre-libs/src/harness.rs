//! Universal Cat-A op harness registry  -  moved into the standalone
//! `vyre-harness` crate so external wrapper libraries (e.g. `downstream dataflow engine`,
//! `decodex`, `multimatch`) can publish into the same registry
//! without depending on the rest of `vyre-libs`. This module is a
//! thin re-export so existing call sites
//! (`vyre_libs::harness::OpEntry`, etc.) keep compiling unchanged.

pub use vyre_harness::fp_contract;
pub use vyre_harness::{
    all_entries, convergence_contract, fixpoint_contract, ConvergenceContract, ExpectedFn,
    FixpointContract, FixpointRegistration, InputsFn, OpEntry,
};
pub use vyre_harness::{
    region, reparent_program_children, tag_program, wrap, wrap_anonymous, wrap_child,
};

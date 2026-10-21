//! Exploded supergraph primitive (G3).
//!
//! # What this is
//!
//! IFDS / IDE reframes interprocedural dataflow as a reachability
//! problem on the **exploded supergraph**: each `(proc, block,
//! fact)` triple is a graph vertex, and the edges are the flow
//! functions (GEN / KILL + summary + call-to-return). Once
//! expanded, the analysis collapses to a BFS over this graph  -
//! which is the exact shape
//! [`crate::graph::csr_forward_traverse`] already handles.
//!
//! This module owns the **node encoding**  -  the bit-layout that
//! packs `(proc_id, block_id, fact_id)` into a single `u32` node id
//!  -  plus a CPU reference that builds the exploded CSR so tests in
//! `vyre-libs::dataflow::ifds_gpu` can prove the GPU kernel produces
//! byte-identical CSR output.
//!
//! # Bit layout
//!
//! ```text
//!   bits 31..20   proc_id   (12 bits  -  4096 procedures per module)
//!   bits 19..10   block_id  (10 bits  -  1024 blocks per procedure)
//!   bits 9..0     fact_id   (10 bits  -  1024 facts per workgroup;
//!                            matches FACTS_PER_WORKGROUP and the
//!                            NFA subgroup sizing)
//! ```
//!
//! This deliberately leaves no room for >4096 procedures in a
//! single module. Any real codebase that exceeds that split along
//! a module boundary first  -  doing interprocedural dataflow over
//! 10 000+ procs in one pass is a different problem that we don't
//! solve here and shouldn't pretend to.
//!
//! # Status
//!
//! Node encoding, CSR builder, and tests. The GPU Program wrapper
//! (the actual kernel that walks edges in parallel) lives in
//! `vyre-libs::dataflow::ifds_gpu` and composes this encoding with
//! `csr_forward_traverse`.

#[path = "exploded/abi.rs"]
mod abi;
#[path = "exploded/canonicalize.rs"]
mod canonicalize;
#[path = "exploded/cpu_ref.rs"]
mod cpu_ref;
#[path = "exploded/dispatch_plan.rs"]
mod dispatch_plan;
#[path = "exploded/encoding.rs"]
mod encoding;
#[path = "exploded/layout.rs"]
mod layout;
#[path = "exploded/program_ir.rs"]
mod program_ir;
#[path = "exploded/program_key.rs"]
mod program_key;
#[path = "exploded/validation.rs"]
mod validation;

#[cfg(test)]
#[path = "exploded/tests/mod.rs"]
mod tests;

pub use abi::{
    ifds_csr_dispatch_grid, IFDS_CSR_COL_IDX_BUFFER, IFDS_CSR_COL_LEN_BUFFER,
    IFDS_CSR_EMPTY_DISPATCH_GRID, IFDS_CSR_GEN_BLOCK_BUFFER, IFDS_CSR_GEN_FACT_BUFFER,
    IFDS_CSR_GEN_PROC_BUFFER, IFDS_CSR_INTER_DST_BLOCK_BUFFER, IFDS_CSR_INTER_DST_PROC_BUFFER,
    IFDS_CSR_INTER_SRC_BLOCK_BUFFER, IFDS_CSR_INTER_SRC_PROC_BUFFER,
    IFDS_CSR_INTRA_DST_BLOCK_BUFFER, IFDS_CSR_INTRA_PROC_BUFFER, IFDS_CSR_INTRA_SRC_BLOCK_BUFFER,
    IFDS_CSR_KILLED_BUFFER, IFDS_CSR_KILL_BLOCK_BUFFER, IFDS_CSR_KILL_FACT_BUFFER,
    IFDS_CSR_KILL_PROC_BUFFER, IFDS_CSR_ROW_CURSOR_BUFFER, IFDS_CSR_ROW_PTR_BUFFER,
    IFDS_CSR_WORKGROUP_SIZE, OP_ID,
};
pub use canonicalize::{canonicalize_csr_within_rows, canonicalize_csr_within_rows_in_place};
#[cfg(any(test, feature = "cpu-parity"))]
pub use cpu_ref::{
    build_cpu_reference, try_build_cpu_reference, try_build_cpu_reference_into,
    ExplodedIfdsCpuScratch,
};
pub use dispatch_plan::{
    plan_ifds_csr_dispatch, split_ifds_rule_quads_into, split_ifds_rule_triples_into,
    IfdsCsrDispatchPlan, IfdsCsrRuleColumns,
};
pub use encoding::{
    decode_node, dense_to_encoded, encode_node, encoded_to_dense, fits, BLOCK_BITS,
    FACTS_PER_WORKGROUP, FACT_BITS, MAX_BLOCK_ID, MAX_FACT_ID, MAX_PROC_ID, PROC_BITS,
};
pub use layout::{
    IfdsCsrLayout, IfdsCsrProgramCacheKey, IfdsCsrRuleInputFingerprint, IfdsCsrStaticInputKey,
};
pub use program_ir::build_ifds_csr_program;
#[cfg(any(test, feature = "cpu-parity"))]
pub use program_key::ifds_program_cache_key_from_program;
pub use validation::{
    ifds_node_count_checked, ifds_node_count_saturating, max_ifds_col_count,
    validate_ifds_csr_inputs, validate_ifds_csr_layout, validate_ifds_csr_readback,
};

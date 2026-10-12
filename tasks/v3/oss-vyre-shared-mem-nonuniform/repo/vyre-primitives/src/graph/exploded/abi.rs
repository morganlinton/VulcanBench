/// Canonical op id for the IFDS CSR construction program.
pub const OP_ID: &str = "vyre-primitives::graph::exploded_build_ifds_csr";

/// Canonical dispatch input label for intra-procedural procedure ids.
pub const IFDS_CSR_INTRA_PROC_BUFFER: &str = "exploded_ifds_csr intra_proc";
/// Canonical dispatch input label for intra-procedural source blocks.
pub const IFDS_CSR_INTRA_SRC_BLOCK_BUFFER: &str = "exploded_ifds_csr intra_src_block";
/// Canonical dispatch input label for intra-procedural destination blocks.
pub const IFDS_CSR_INTRA_DST_BLOCK_BUFFER: &str = "exploded_ifds_csr intra_dst_block";
/// Canonical dispatch input label for inter-procedural source procedures.
pub const IFDS_CSR_INTER_SRC_PROC_BUFFER: &str = "exploded_ifds_csr inter_src_proc";
/// Canonical dispatch input label for inter-procedural source blocks.
pub const IFDS_CSR_INTER_SRC_BLOCK_BUFFER: &str = "exploded_ifds_csr inter_src_block";
/// Canonical dispatch input label for inter-procedural destination procedures.
pub const IFDS_CSR_INTER_DST_PROC_BUFFER: &str = "exploded_ifds_csr inter_dst_proc";
/// Canonical dispatch input label for inter-procedural destination blocks.
pub const IFDS_CSR_INTER_DST_BLOCK_BUFFER: &str = "exploded_ifds_csr inter_dst_block";
/// Canonical dispatch input label for GEN rule procedures.
pub const IFDS_CSR_GEN_PROC_BUFFER: &str = "exploded_ifds_csr gen_proc";
/// Canonical dispatch input label for GEN rule blocks.
pub const IFDS_CSR_GEN_BLOCK_BUFFER: &str = "exploded_ifds_csr gen_block";
/// Canonical dispatch input label for GEN rule facts.
pub const IFDS_CSR_GEN_FACT_BUFFER: &str = "exploded_ifds_csr gen_fact";
/// Canonical dispatch input label for KILL rule procedures.
pub const IFDS_CSR_KILL_PROC_BUFFER: &str = "exploded_ifds_csr kill_proc";
/// Canonical dispatch input label for KILL rule blocks.
pub const IFDS_CSR_KILL_BLOCK_BUFFER: &str = "exploded_ifds_csr kill_block";
/// Canonical dispatch input label for KILL rule facts.
pub const IFDS_CSR_KILL_FACT_BUFFER: &str = "exploded_ifds_csr kill_fact";
/// Canonical dispatch output label for CSR row pointers.
pub const IFDS_CSR_ROW_PTR_BUFFER: &str = "exploded_ifds_csr row_ptr";
/// Canonical dispatch scratch label for row cursors.
pub const IFDS_CSR_ROW_CURSOR_BUFFER: &str = "exploded_ifds_csr row_cursor";
/// Canonical dispatch output label for CSR column indices.
pub const IFDS_CSR_COL_IDX_BUFFER: &str = "exploded_ifds_csr col_idx";
/// Canonical dispatch output label for emitted column length.
pub const IFDS_CSR_COL_LEN_BUFFER: &str = "exploded_ifds_csr col_len";
/// Canonical dispatch scratch label for the dense kill bitmap.
pub const IFDS_CSR_KILLED_BUFFER: &str = "exploded_ifds_csr killed";
/// Serial workgroup for exploded IFDS CSR construction.
pub const IFDS_CSR_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];

/// Dispatch grid for exploded IFDS CSR construction.
///
/// The generated Program executes the full CSR build inside global lane `0`, so
/// the launch contract is a single block. Scaling this grid only launches idle
/// blocks and burns dispatch overhead without changing observable work.
#[must_use]
pub const fn ifds_csr_dispatch_grid(_intra_count: u32, _total_nodes: u32) -> [u32; 3] {
    [1, 1, 1]
}

/// Minimum grid for empty no-rule IFDS dispatch plans.
pub const IFDS_CSR_EMPTY_DISPATCH_GRID: [u32; 3] = [1, 1, 1];

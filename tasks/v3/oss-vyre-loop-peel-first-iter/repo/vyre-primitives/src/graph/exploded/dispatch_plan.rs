use super::abi::{ifds_csr_dispatch_grid, IFDS_CSR_EMPTY_DISPATCH_GRID};
use super::layout::{
    IfdsCsrLayout, IfdsCsrProgramCacheKey, IfdsCsrRuleInputFingerprint, IfdsCsrStaticInputKey,
};
use super::program_ir::build_ifds_csr_program;
use super::validation::validate_ifds_csr_inputs;
use vyre_foundation::ir::Program;

/// Primitive-owned dispatch plan for exploded IFDS CSR construction.
///
/// Consumers own only rule marshalling and backend invocation. Buffer labels,
/// padded storage widths, readback widths, and grid shape live here so
/// self-substrate and future consumers cannot fork the dispatch contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfdsCsrDispatchPlan {
    /// Validated CSR layout and rule counts.
    pub layout: IfdsCsrLayout,
    /// Primitive-owned generated-Program cache identity.
    pub program_key: IfdsCsrProgramCacheKey,
    /// Dispatch grid override.
    pub grid: [u32; 3],
    /// Padded words for each intra edge field.
    pub intra_field_words: usize,
    /// Padded words for each inter edge field.
    pub inter_field_words: usize,
    /// Padded words for each GEN field.
    pub gen_field_words: usize,
    /// Padded words for each KILL field.
    pub kill_field_words: usize,
    /// Words in the CSR row-pointer output.
    pub row_ptr_words: usize,
    /// Words in the row-cursor scratch output.
    pub row_cursor_words: usize,
    /// Words in the dense kill-bitmap scratch buffer.
    pub killed_words: usize,
    /// Words in the CSR column-index output.
    pub col_idx_words: usize,
    /// Words in the emitted-column-length output.
    pub col_len_words: usize,
    /// Maximum legal emitted column count.
    pub max_col_count: u32,
}

impl IfdsCsrDispatchPlan {
    /// Build the GPU program for this validated dispatch plan.
    #[must_use]
    pub fn program(&self) -> Program {
        build_ifds_csr_program(
            self.layout.num_procs,
            self.layout.blocks_per_proc,
            self.layout.facts_per_proc,
            self.layout.intra_count,
            self.layout.inter_count,
            self.layout.gen_count,
            self.layout.kill_count,
            self.layout.max_col_count,
        )
    }

    /// Stable generated-Program cache identity for this dispatch shape.
    #[must_use]
    pub const fn program_cache_key(&self) -> IfdsCsrProgramCacheKey {
        self.program_key
    }

    /// Stable identity for static rule uploads under this dispatch plan.
    #[must_use]
    pub const fn static_input_key(
        &self,
        rule_fingerprint: IfdsCsrRuleInputFingerprint,
    ) -> IfdsCsrStaticInputKey {
        IfdsCsrStaticInputKey {
            program_key: self.program_key,
            rule_fingerprint,
        }
    }
}
/// Validate caller-owned IFDS rules and return the complete primitive dispatch plan.
pub fn plan_ifds_csr_dispatch(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
) -> Result<IfdsCsrDispatchPlan, String> {
    let layout = validate_ifds_csr_inputs(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
    )?;
    Ok(IfdsCsrDispatchPlan {
        intra_field_words: layout.intra_storage_words,
        inter_field_words: layout.inter_storage_words,
        gen_field_words: layout.gen_storage_words,
        kill_field_words: layout.kill_storage_words,
        row_ptr_words: layout.row_words,
        row_cursor_words: layout.row_cursor_words,
        killed_words: layout.killed_words,
        col_idx_words: layout.col_buffer_words,
        col_len_words: 1,
        max_col_count: layout.max_col_count,
        program_key: IfdsCsrProgramCacheKey::from_layout(&layout),
        layout,
        grid: if layout.empty {
            IFDS_CSR_EMPTY_DISPATCH_GRID
        } else {
            ifds_csr_dispatch_grid(layout.intra_count, layout.total_nodes)
        },
    })
}

/// Caller-owned structure-of-arrays rule columns for IFDS CSR dispatch.
///
/// This lives with the primitive dispatch plan because field order, padding,
/// and rule-domain grouping are part of the primitive ABI. Dispatch consumers
/// reuse one value across calls and upload these columns without re-forking
/// IFDS tuple marshalling.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IfdsCsrRuleColumns {
    /// Intra-procedural procedure ids.
    pub intra_proc: Vec<u32>,
    /// Intra-procedural source blocks.
    pub intra_src_block: Vec<u32>,
    /// Intra-procedural destination blocks.
    pub intra_dst_block: Vec<u32>,
    /// Inter-procedural source procedures.
    pub inter_src_proc: Vec<u32>,
    /// Inter-procedural source blocks.
    pub inter_src_block: Vec<u32>,
    /// Inter-procedural destination procedures.
    pub inter_dst_proc: Vec<u32>,
    /// Inter-procedural destination blocks.
    pub inter_dst_block: Vec<u32>,
    /// GEN rule procedures.
    pub gen_proc: Vec<u32>,
    /// GEN rule blocks.
    pub gen_block: Vec<u32>,
    /// GEN rule facts.
    pub gen_fact: Vec<u32>,
    /// KILL rule procedures.
    pub kill_proc: Vec<u32>,
    /// KILL rule blocks.
    pub kill_block: Vec<u32>,
    /// KILL rule facts.
    pub kill_fact: Vec<u32>,
}

impl IfdsCsrRuleColumns {
    /// Split all IFDS tuple rules into primitive-owned structure-of-arrays
    /// columns, reusing existing allocations.
    ///
    /// # Errors
    ///
    /// Returns an allocation diagnostic if any output column cannot reserve
    /// enough space for its incoming rule slice.
    pub fn prepare(
        &mut self,
        intra_edges: &[(u32, u32, u32)],
        inter_edges: &[(u32, u32, u32, u32)],
        flow_gen: &[(u32, u32, u32)],
        flow_kill: &[(u32, u32, u32)],
    ) -> Result<(), String> {
        split_ifds_rule_triples_into(
            intra_edges,
            &mut self.intra_proc,
            &mut self.intra_src_block,
            &mut self.intra_dst_block,
            "IFDS intra edge columns",
        )?;
        split_ifds_rule_quads_into(
            inter_edges,
            &mut self.inter_src_proc,
            &mut self.inter_src_block,
            &mut self.inter_dst_proc,
            &mut self.inter_dst_block,
            "IFDS inter edge columns",
        )?;
        split_ifds_rule_triples_into(
            flow_gen,
            &mut self.gen_proc,
            &mut self.gen_block,
            &mut self.gen_fact,
            "IFDS GEN columns",
        )?;
        split_ifds_rule_triples_into(
            flow_kill,
            &mut self.kill_proc,
            &mut self.kill_block,
            &mut self.kill_fact,
            "IFDS KILL columns",
        )
    }
}

/// Split IFDS triple rules into primitive-owned structure-of-arrays columns.
///
/// This keeps the rule-column layout beside the dispatch plan so wrappers do
/// not reimplement tuple marshalling before uploading GPU buffers.
///
/// # Errors
///
/// Returns an allocation diagnostic if any output column cannot reserve enough
/// space for the incoming rules.
pub fn split_ifds_rule_triples_into(
    triples: &[(u32, u32, u32)],
    first: &mut Vec<u32>,
    second: &mut Vec<u32>,
    third: &mut Vec<u32>,
    context: &str,
) -> Result<(), String> {
    first.clear();
    second.clear();
    third.clear();
    crate::graph::scratch::reserve_graph_items(
        first,
        triples.len(),
        "exploded IFDS primitive",
        context,
    )?;
    crate::graph::scratch::reserve_graph_items(
        second,
        triples.len(),
        "exploded IFDS primitive",
        context,
    )?;
    crate::graph::scratch::reserve_graph_items(
        third,
        triples.len(),
        "exploded IFDS primitive",
        context,
    )?;
    for &(a, b, c) in triples {
        first.push(a);
        second.push(b);
        third.push(c);
    }
    Ok(())
}

/// Split IFDS quadruple rules into primitive-owned structure-of-arrays columns.
///
/// # Errors
///
/// Returns an allocation diagnostic if any output column cannot reserve enough
/// space for the incoming rules.
pub fn split_ifds_rule_quads_into(
    quads: &[(u32, u32, u32, u32)],
    first: &mut Vec<u32>,
    second: &mut Vec<u32>,
    third: &mut Vec<u32>,
    fourth: &mut Vec<u32>,
    context: &str,
) -> Result<(), String> {
    first.clear();
    second.clear();
    third.clear();
    fourth.clear();
    crate::graph::scratch::reserve_graph_items(
        first,
        quads.len(),
        "exploded IFDS primitive",
        context,
    )?;
    crate::graph::scratch::reserve_graph_items(
        second,
        quads.len(),
        "exploded IFDS primitive",
        context,
    )?;
    crate::graph::scratch::reserve_graph_items(
        third,
        quads.len(),
        "exploded IFDS primitive",
        context,
    )?;
    crate::graph::scratch::reserve_graph_items(
        fourth,
        quads.len(),
        "exploded IFDS primitive",
        context,
    )?;
    for &(a, b, c, d) in quads {
        first.push(a);
        second.push(b);
        third.push(c);
        fourth.push(d);
    }
    Ok(())
}

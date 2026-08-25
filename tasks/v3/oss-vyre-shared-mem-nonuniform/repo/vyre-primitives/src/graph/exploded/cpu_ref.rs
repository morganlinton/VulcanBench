#[cfg(any(test, feature = "cpu-parity"))]
use super::validation::validate_ifds_csr_inputs;

/// CPU-reference CSR builder for the exploded supergraph.
///
/// `intra_edges` are `(src_block, dst_block)` pairs **within** a
/// procedure  -  the standard CFG. `inter_edges` are `(src_proc,
/// src_block, dst_proc, dst_block)` call / return edges. Flow
/// functions are encoded as per-block GEN / KILL bitsets over the
/// fact domain.
///
/// Caller-owned workspace for exploded IFDS CPU-reference CSR construction.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Debug, Default, Clone)]
pub struct ExplodedIfdsCpuScratch {
    /// Flat `(src, dst)` edge list before CSR compaction.
    pub edges_flat: Vec<(u32, u32)>,
    /// Per-dense-node KILL bitmap.
    pub killed: Vec<bool>,
    /// Per-block GEN prefix offsets.
    pub gen_offsets: Vec<usize>,
    /// Per-block GEN fill cursor.
    pub gen_cursor: Vec<usize>,
    /// Flat GEN fact table keyed by `gen_offsets`.
    pub gen_facts: Vec<u32>,
    /// Per-row CSR fill cursor.
    pub cursor: Vec<usize>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl ExplodedIfdsCpuScratch {
    /// Create an empty reusable exploded-IFDS CPU workspace.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Returns `(row_ptr, col_idx)` in the **dense** index space
/// `idx(p, b, f) = p * blocks * facts + b * facts + f`. This is
/// the space every traversal kernel operates in  -  packing via
/// [`encode_node`] is only used at the I/O boundary when the
/// caller needs to report results as `(proc, block, fact)`
/// triples. The two spaces coincide only in the degenerate case
/// `blocks_per_proc == 1 << BLOCK_BITS` and `facts_per_proc == 1 << FACT_BITS`;
/// the dense layout works for any dimensions that fit in
/// 32-bit encoding.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn build_cpu_reference(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)], // (proc, src_block, dst_block)
    inter_edges: &[(u32, u32, u32, u32)], // (src_proc, src_block, dst_proc, dst_block)
    flow_gen: &[(u32, u32, u32)],    // (proc, block, fact)  -  GEN bits
    flow_kill: &[(u32, u32, u32)],   // (proc, block, fact)  -  KILL bits
) -> (Vec<u32>, Vec<u32>) {
    try_build_cpu_reference(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
    )
    .unwrap_or_else(|err| panic!("exploded IFDS CPU reference received malformed input. {err}"))
}

/// Fallible CPU-reference CSR builder for the exploded supergraph.
///
/// This is the allocation-safe oracle entry point for fuzz, hostile-dimension,
/// and parity harnesses. [`build_cpu_reference`] preserves the legacy panicking
/// API by delegating here.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_build_cpu_reference(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
) -> Result<(Vec<u32>, Vec<u32>), String> {
    let mut row_ptr = Vec::new();
    let mut col_idx = Vec::new();
    let mut scratch = ExplodedIfdsCpuScratch::default();
    try_build_cpu_reference_into(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
        &mut row_ptr,
        &mut col_idx,
        &mut scratch,
    )?;
    Ok((row_ptr, col_idx))
}

/// Fallible CPU-reference CSR builder into caller-owned output and scratch.
///
/// Validation happens before output and scratch storage are cleared. This keeps
/// fuzz/parity diagnostics intact when a malformed IFDS domain or rule set is
/// rejected before CSR construction begins.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_build_cpu_reference_into(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
    row_ptr: &mut Vec<u32>,
    col_idx: &mut Vec<u32>,
    scratch: &mut ExplodedIfdsCpuScratch,
) -> Result<(), String> {
    let layout = validate_ifds_csr_inputs(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
    )?;
    if layout.empty {
        return Err(format!(
            "exploded IFDS CPU reference dimensions must be nonzero, got procs={num_procs}, blocks={blocks_per_proc}, facts={facts_per_proc}. Fix: pass a real exploded-supergraph domain before parity comparison."
        ));
    }

    // PHASE7_GRAPH C4: every multiply checked. The previous unchecked
    // chain (`blocks * facts`, then `procs * slots`) wraps silently
    // when the caller passes the maximum dimensions for each field
    // (4096 × 1024 × 1024 = 2^32 = wraps to 0 on 32-bit usize and
    // sits exactly at the overflow boundary on 64-bit). Either case
    // produced a tiny `Vec<Vec<u32>>` and catastrophic OOB writes in
    // the edge-emit loops below.
    let slots_per_proc = layout.slots_per_proc as usize;
    let total_nodes = layout.total_nodes as usize;
    crate::graph::scratch::reserve_graph_items(
        &mut scratch.edges_flat,
        layout.max_col_count as usize,
        "exploded IFDS CPU reference",
        "flat exploded edge list",
    )?;
    scratch.edges_flat.clear();
    let block_count = (num_procs as usize) * (blocks_per_proc as usize);

    let idx = |p: u32, b: u32, f: u32| -> u32 {
        ((p as usize) * slots_per_proc + (b as usize) * facts_per_proc as usize + f as usize) as u32
    };
    let block_idx =
        |p: u32, b: u32| -> usize { (p as usize) * blocks_per_proc as usize + b as usize };
    let in_space =
        |p: u32, b: u32, f: u32| p < num_procs && b < blocks_per_proc && f < facts_per_proc;

    crate::graph::scratch::reserve_graph_items(
        &mut scratch.killed,
        total_nodes,
        "exploded IFDS CPU reference",
        "kill bitmap",
    )?;
    scratch.killed.clear();
    scratch.killed.resize(total_nodes, false);
    for &(p, b, f) in flow_kill {
        if in_space(p, b, f) {
            scratch.killed[idx(p, b, f) as usize] = true;
        }
    }

    let gen_offset_count = block_count
        .checked_add(1)
        .ok_or_else(|| "Fix: exploded IFDS block_count+1 overflows usize.".to_string())?;
    crate::graph::scratch::reserve_graph_items(
        &mut scratch.gen_offsets,
        gen_offset_count,
        "exploded IFDS CPU reference",
        "GEN offsets",
    )?;
    scratch.gen_offsets.clear();
    scratch.gen_offsets.resize(gen_offset_count, 0);
    for &(p, b, f) in flow_gen {
        if in_space(p, b, f) {
            scratch.gen_offsets[block_idx(p, b) + 1] += 1;
        }
    }
    for i in 1..scratch.gen_offsets.len() {
        scratch.gen_offsets[i] += scratch.gen_offsets[i - 1];
    }
    crate::graph::scratch::reserve_graph_items(
        &mut scratch.gen_cursor,
        block_count,
        "exploded IFDS CPU reference",
        "GEN cursor",
    )?;
    scratch.gen_cursor.clear();
    scratch
        .gen_cursor
        .extend_from_slice(&scratch.gen_offsets[..block_count]);
    let gen_fact_count = scratch.gen_offsets[block_count];
    crate::graph::scratch::reserve_graph_items(
        &mut scratch.gen_facts,
        gen_fact_count,
        "exploded IFDS CPU reference",
        "GEN fact table",
    )?;
    scratch.gen_facts.clear();
    scratch.gen_facts.resize(gen_fact_count, 0);
    for &(p, b, f) in flow_gen {
        if in_space(p, b, f) {
            let key = block_idx(p, b);
            let slot = scratch.gen_cursor[key];
            scratch.gen_facts[slot] = f;
            scratch.gen_cursor[key] += 1;
        }
    }

    // Intra-procedural CFG edges, cross-producted with fact-propagation:
    // an edge (B_src -> B_dst) gives rise to an edge in the exploded
    // supergraph between every pair (f, f) that survives the flow
    // function at B_src (fact f propagates iff f is not killed).
    for &(p, src_b, dst_b) in intra_edges {
        if p >= num_procs || src_b >= blocks_per_proc || dst_b >= blocks_per_proc {
            continue;
        }
        for f in 0..facts_per_proc {
            if scratch.killed[idx(p, src_b, f) as usize] {
                continue;
            }
            scratch
                .edges_flat
                .push((idx(p, src_b, f), idx(p, dst_b, f)));
        }
        // GEN edges: standard IFDS 0-fact encoding  -  fact 0 is the
        // tautological "always present" fact. `GEN(src_b, gf)` emits
        // edge `(src_b, 0) → (dst_b, gf)`, so seeding `(entry, 0)`
        // triggers every GEN along the reachable CFG. Callers that
        // don't use the 0-fact convention see GEN as a no-op.
        let gen_key = block_idx(p, src_b);
        for &gf in
            &scratch.gen_facts[scratch.gen_offsets[gen_key]..scratch.gen_offsets[gen_key + 1]]
        {
            scratch
                .edges_flat
                .push((idx(p, src_b, 0), idx(p, dst_b, gf)));
        }
    }

    // Inter-procedural call / return edges propagate every fact
    // (IFDS handles parameter mapping via summary edges in the full
    // algorithm; this CPU reference is the unfiltered
    // "every-fact-flows" upper bound used for correctness tests).
    for &(sp, sb, dp, db) in inter_edges {
        if sp >= num_procs || dp >= num_procs || sb >= blocks_per_proc || db >= blocks_per_proc {
            continue;
        }
        for f in 0..facts_per_proc {
            scratch.edges_flat.push((idx(sp, sb, f), idx(dp, db, f)));
        }
    }

    // Flatten into CSR  -  row_ptr has total_nodes+1 entries.
    if scratch.edges_flat.len() > u32::MAX as usize {
        return Err(format!(
            "exploded IFDS CPU reference edge_count={} exceeds u32 CSR encoding. Fix: shard the IFDS graph before parity comparison.",
            scratch.edges_flat.len()
        ));
    }
    let row_ptr_len = layout.row_words;
    crate::graph::scratch::reserve_graph_items(
        row_ptr,
        row_ptr_len,
        "exploded IFDS CPU reference",
        "CSR row_ptr",
    )?;
    row_ptr.clear();
    row_ptr.resize(row_ptr_len, 0);
    for &(src, _) in &scratch.edges_flat {
        let row = src as usize;
        row_ptr[row + 1] = row_ptr[row + 1].checked_add(1).ok_or_else(|| {
            format!(
                "exploded IFDS CPU reference row {row} edge count overflowed u32. Fix: shard the IFDS graph before parity comparison."
            )
        })?;
    }
    for row in 1..row_ptr.len() {
        row_ptr[row] = row_ptr[row].checked_add(row_ptr[row - 1]).ok_or_else(|| {
            format!(
                "exploded IFDS CPU reference CSR prefix overflowed at row {row}. Fix: shard the IFDS graph before parity comparison."
            )
        })?;
    }
    crate::graph::scratch::reserve_graph_items(
        &mut scratch.cursor,
        total_nodes,
        "exploded IFDS CPU reference",
        "CSR cursor",
    )?;
    scratch.cursor.clear();
    for &offset in &row_ptr[..total_nodes] {
        scratch.cursor.push(offset as usize);
    }
    crate::graph::scratch::reserve_graph_items(
        col_idx,
        scratch.edges_flat.len(),
        "exploded IFDS CPU reference",
        "CSR col_idx",
    )?;
    col_idx.clear();
    col_idx.resize(scratch.edges_flat.len(), 0);
    for &(src, dst) in &scratch.edges_flat {
        let row = src as usize;
        let slot = scratch.cursor[row];
        col_idx[slot] = dst;
        scratch.cursor[row] += 1;
    }
    Ok(())
}

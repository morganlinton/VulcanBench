use super::encoding::fits;
use super::layout::IfdsCsrLayout;

/// Validate CSR data returned by an exploded IFDS backend.
///
/// # Errors
///
/// Returns an actionable diagnostic when row pointers, live column length, or
/// live column indices do not satisfy the primitive CSR contract.
pub fn validate_ifds_csr_readback(
    layout: &IfdsCsrLayout,
    row_ptr: &[u32],
    col_idx: &[u32],
    col_len: u32,
) -> Result<usize, String> {
    if row_ptr.len() != layout.row_words {
        return Err(format!(
            "Fix: exploded IFDS row_ptr readback expected {} word(s), got {}.",
            layout.row_words,
            row_ptr.len()
        ));
    }
    if row_ptr.first().copied() != Some(0) {
        return Err("Fix: exploded IFDS CSR row_ptr[0] must be 0.".to_string());
    }
    if col_len > layout.max_col_count {
        return Err(format!(
            "Fix: exploded IFDS GPU reported col_len {col_len} above allocated maximum {}.",
            layout.max_col_count
        ));
    }
    let live_cols = usize::try_from(col_len).map_err(|_| {
        format!("Fix: exploded IFDS col_len {col_len} cannot be represented as usize.")
    })?;
    if live_cols > col_idx.len() {
        return Err(format!(
            "Fix: exploded IFDS col_len {col_len} exceeds col_idx readback words {}.",
            col_idx.len()
        ));
    }
    for (row, window) in row_ptr.windows(2).enumerate() {
        let start = window[0];
        let end = window[1];
        if start > end {
            return Err(format!(
                "Fix: exploded IFDS row_ptr is not monotonic at row {row}: {start} > {end}."
            ));
        }
        if end > col_len {
            return Err(format!(
                "Fix: exploded IFDS row {row} ends at {end}, beyond live col_len {col_len}."
            ));
        }
    }
    let final_row = row_ptr.last().copied().unwrap_or(0);
    if final_row != col_len {
        return Err(format!(
            "Fix: exploded IFDS final row_ptr value {final_row} must equal col_len {col_len}."
        ));
    }
    for (index, &column) in col_idx.iter().take(live_cols).enumerate() {
        if column >= layout.total_nodes {
            return Err(format!(
                "Fix: exploded IFDS col_idx[{index}]={column} is outside total_nodes {}.",
                layout.total_nodes
            ));
        }
    }
    Ok(live_cols)
}

/// Checked exploded-supergraph node count.
#[must_use]
pub fn ifds_node_count_checked(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
) -> Option<u32> {
    num_procs
        .checked_mul(blocks_per_proc)?
        .checked_mul(facts_per_proc)
}

/// Saturating exploded-supergraph node count for capacity planning UIs.
#[must_use]
pub fn ifds_node_count_saturating(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
) -> u32 {
    num_procs
        .saturating_mul(blocks_per_proc)
        .saturating_mul(facts_per_proc)
}

/// Maximum column count needed by the deterministic IFDS CSR builder.
#[must_use]
pub fn max_ifds_col_count(
    intra_count: u32,
    inter_count: u32,
    gen_count: u32,
    facts_per_proc: u32,
) -> Option<u32> {
    intra_count
        .checked_mul(facts_per_proc)
        .and_then(|v| v.checked_add(intra_count.checked_mul(gen_count)?))
        .and_then(|v| v.checked_add(inter_count.checked_mul(facts_per_proc)?))
}

/// Validate dimensions/counts and return the exact dispatch buffer layout.
pub fn validate_ifds_csr_layout(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_count: u32,
    inter_count: u32,
    gen_count: u32,
) -> Result<IfdsCsrLayout, String> {
    if num_procs == 0 || blocks_per_proc == 0 || facts_per_proc == 0 {
        return Err(format!(
            "Fix: exploded IFDS dimensions must be nonzero, got procs={num_procs}, blocks={blocks_per_proc}, facts={facts_per_proc}."
        ));
    }
    if !fits(
        num_procs.saturating_sub(1),
        blocks_per_proc.saturating_sub(1),
        facts_per_proc.saturating_sub(1),
    ) {
        return Err(format!(
            "Fix: exploded IFDS dimensions exceed packed IFDS limits: procs={num_procs}, blocks={blocks_per_proc}, facts={facts_per_proc}."
        ));
    }
    let slots_per_proc = blocks_per_proc.checked_mul(facts_per_proc).ok_or_else(|| {
        format!(
            "Fix: exploded IFDS blocks*facts overflows u32: {blocks_per_proc}*{facts_per_proc}."
        )
    })?;
    let total_nodes = num_procs.checked_mul(slots_per_proc).ok_or_else(|| {
        format!(
            "Fix: exploded IFDS procs*blocks*facts overflows u32: {num_procs}*{blocks_per_proc}*{facts_per_proc}."
        )
    })?;
    let row_ptr_count = total_nodes.checked_add(1).ok_or_else(|| {
        format!(
            "Fix: exploded IFDS total_nodes={total_nodes} overflows row_ptr count. Shard the IFDS graph before GPU dispatch."
        )
    })?;
    let max_col_count = max_ifds_col_count(intra_count, inter_count, gen_count, facts_per_proc)
        .ok_or_else(|| "Fix: exploded IFDS maximum column count overflows u32.".to_string())?;
    Ok(IfdsCsrLayout {
        empty: false,
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_count,
        inter_count,
        gen_count,
        kill_count: 0,
        intra_storage_words: (intra_count as usize).max(1),
        inter_storage_words: (inter_count as usize).max(1),
        gen_storage_words: (gen_count as usize).max(1),
        kill_storage_words: 1,
        slots_per_proc,
        total_nodes,
        row_words: row_ptr_count as usize,
        row_cursor_words: (total_nodes as usize).max(1),
        killed_words: (total_nodes as usize).max(1),
        max_col_count,
        col_buffer_words: (max_col_count as usize).max(1),
    })
}

fn checked_rule_count(kind: &str, len: usize) -> Result<u32, String> {
    u32::try_from(len)
        .map_err(|_| format!("Fix: exploded IFDS {kind} count {len} exceeds u32 index space."))
}

/// Validate the full IFDS CSR dispatch contract from caller-owned rule slices.
///
/// Returns the exact primitive dispatch layout so consumers do not narrow rule
/// counts or decide padded input-buffer widths locally.
pub fn validate_ifds_csr_inputs(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
) -> Result<IfdsCsrLayout, String> {
    let intra_count = checked_rule_count("intra edge", intra_edges.len())?;
    let inter_count = checked_rule_count("inter edge", inter_edges.len())?;
    let gen_count = checked_rule_count("GEN", flow_gen.len())?;
    let kill_count = checked_rule_count("KILL", flow_kill.len())?;

    if num_procs == 0 || blocks_per_proc == 0 || facts_per_proc == 0 {
        if intra_count == 0 && inter_count == 0 && gen_count == 0 && kill_count == 0 {
            return Ok(IfdsCsrLayout {
                empty: true,
                num_procs,
                blocks_per_proc,
                facts_per_proc,
                intra_count,
                inter_count,
                gen_count,
                kill_count,
                intra_storage_words: 1,
                inter_storage_words: 1,
                gen_storage_words: 1,
                kill_storage_words: 1,
                slots_per_proc: 0,
                total_nodes: 0,
                row_words: 1,
                row_cursor_words: 1,
                killed_words: 1,
                max_col_count: 0,
                col_buffer_words: 1,
            });
        }
        return Err(format!(
            "Fix: exploded IFDS empty dimensions cannot carry rules, got intra={intra_count}, inter={inter_count}, gen={gen_count}, kill={kill_count}."
        ));
    }

    let mut layout = validate_ifds_csr_layout(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_count,
        inter_count,
        gen_count,
    )?;
    layout.kill_count = kill_count;
    layout.kill_storage_words = flow_kill.len().max(1);
    Ok(layout)
}

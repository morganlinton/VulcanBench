/// Bits reserved for each component of the packed node id.
pub const PROC_BITS: u32 = 12;
/// Bits reserved for the basic-block component of the packed node id.
pub const BLOCK_BITS: u32 = 10;
/// Bits reserved for the fact component of the packed node id.
pub const FACT_BITS: u32 = 10;
const _SANITY: () = assert!(PROC_BITS + BLOCK_BITS + FACT_BITS == 32);

/// Max values for each component  -  one less than the available
/// space because zero is a valid id.
pub const MAX_PROC_ID: u32 = (1 << PROC_BITS) - 1;
/// Maximum encodable basic-block id.
pub const MAX_BLOCK_ID: u32 = (1 << BLOCK_BITS) - 1;
/// Maximum encodable fact id.
pub const MAX_FACT_ID: u32 = (1 << FACT_BITS) - 1;

/// Number of facts per workgroup lane. A 32-lane subgroup x
/// 32 bits = 1024 facts; wider subgroup layouts preserve the same budget.
/// Matches the
/// NFA window sizing in `nfa::subgroup_nfa` so both subsystems
/// share occupancy budget.
pub const FACTS_PER_WORKGROUP: usize = 1024;
const BLOCK_SHIFT: u32 = FACT_BITS;
const PROC_SHIFT: u32 = FACT_BITS + BLOCK_BITS;
const FACT_MASK: u32 = MAX_FACT_ID;
const BLOCK_MASK: u32 = MAX_BLOCK_ID;
const PROC_MASK: u32 = MAX_PROC_ID;
/// Pack a `(proc_id, block_id, fact_id)` triple into a 32-bit
/// node id.
///
/// Invalid triples have no non-aliasing `u32` representation, so the
/// failure is explicit instead of silently clamping or masking.
#[must_use]
pub fn encode_node(proc_id: u32, block_id: u32, fact_id: u32) -> Option<u32> {
    fits(proc_id, block_id, fact_id)
        .then_some((proc_id << PROC_SHIFT) | (block_id << BLOCK_SHIFT) | fact_id)
}

/// Unpack a node id back into `(proc_id, block_id, fact_id)`.
#[must_use]
pub fn decode_node(node_id: u32) -> (u32, u32, u32) {
    let proc_id = (node_id >> PROC_SHIFT) & PROC_MASK;
    let block_id = (node_id >> BLOCK_SHIFT) & BLOCK_MASK;
    let fact_id = node_id & FACT_MASK;
    (proc_id, block_id, fact_id)
}

/// Whether a `(proc, block, fact)` triple fits in the packed
/// 32-bit representation. Callers on the production path should
/// verify this before calling [`encode_node`].
#[must_use]
pub fn fits(proc_id: u32, block_id: u32, fact_id: u32) -> bool {
    proc_id <= MAX_PROC_ID && block_id <= MAX_BLOCK_ID && fact_id <= MAX_FACT_ID
}
/// Convert a dense `(proc, block, fact)` index  -  the space
/// [`build_cpu_reference`] operates in  -  into the packed
/// [`encode_node`] form for reporting or cross-subsystem handoff.
#[must_use]
pub fn dense_to_encoded(dense: u32, blocks_per_proc: u32, facts_per_proc: u32) -> Option<u32> {
    let slots_per_proc = blocks_per_proc.checked_mul(facts_per_proc)?;
    if slots_per_proc == 0 {
        return None;
    }
    let p = dense / slots_per_proc;
    let within_proc = dense % slots_per_proc;
    let b = within_proc / facts_per_proc;
    let f = within_proc % facts_per_proc;
    encode_node(p, b, f)
}

/// Inverse of [`dense_to_encoded`].
#[must_use]
pub fn encoded_to_dense(node_id: u32, blocks_per_proc: u32, facts_per_proc: u32) -> Option<u32> {
    let (p, b, f) = decode_node(node_id);
    let proc_span = blocks_per_proc.checked_mul(facts_per_proc)?;
    let proc_offset = p.checked_mul(proc_span)?;
    let block_offset = b.checked_mul(facts_per_proc)?;
    proc_offset.checked_add(block_offset)?.checked_add(f)
}

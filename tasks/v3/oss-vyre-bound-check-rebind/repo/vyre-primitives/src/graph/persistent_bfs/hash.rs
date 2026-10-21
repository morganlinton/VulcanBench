use super::layout::PersistentBfsPlanCacheKind;
use crate::hash::fnv1a::{fnv1a64_initial_state, fnv1a64_update_byte};

fn fnv1a64_mix_u32(hash: &mut u64, value: u32) {
    for byte in value.to_le_bytes() {
        *hash = fnv1a64_update_byte(*hash, byte);
    }
}

/// Stable FNV-1a hash of a persistent-BFS graph layout.
#[must_use]
pub fn persistent_bfs_layout_hash(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> u64 {
    let mut hash = fnv1a64_initial_state();
    fnv1a64_mix_u32(&mut hash, node_count);
    fnv1a64_mix_u32(&mut hash, edge_offsets.len() as u32);
    for &value in edge_offsets {
        fnv1a64_mix_u32(&mut hash, value);
    }
    fnv1a64_mix_u32(&mut hash, edge_targets.len() as u32);
    for &value in edge_targets {
        fnv1a64_mix_u32(&mut hash, value);
    }
    fnv1a64_mix_u32(&mut hash, edge_kind_mask.len() as u32);
    for &value in edge_kind_mask {
        fnv1a64_mix_u32(&mut hash, value);
    }
    hash
}

/// Stable FNV-1a hash of the persistent-BFS program shape.
///
/// This intentionally excludes CSR contents. The generated program is the same
/// for any graph with the same dimensions, frontier width, query count, and
/// dispatch kind; edge data is carried in buffers.
#[must_use]
pub fn persistent_bfs_program_layout_hash(
    node_count: u32,
    edge_count: u32,
    words_per_query: u32,
    query_count: u32,
    kind: PersistentBfsPlanCacheKind,
) -> u64 {
    let mut hash = fnv1a64_initial_state();
    fnv1a64_mix_u32(&mut hash, 0x5042_4653);
    fnv1a64_mix_u32(&mut hash, node_count);
    fnv1a64_mix_u32(&mut hash, edge_count);
    fnv1a64_mix_u32(&mut hash, words_per_query);
    fnv1a64_mix_u32(&mut hash, query_count);
    fnv1a64_mix_u32(
        &mut hash,
        match kind {
            PersistentBfsPlanCacheKind::Single => 0,
            PersistentBfsPlanCacheKind::Batch => 1,
        },
    );
    hash
}

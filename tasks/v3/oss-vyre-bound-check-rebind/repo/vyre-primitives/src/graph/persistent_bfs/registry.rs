use super::layout::OP_ID;
use super::program::persistent_bfs;
use crate::graph::program_graph::ProgramGraphShape;

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || persistent_bfs(ProgramGraphShape::new(4, 4), "fin", "fout", 0xFFFF_FFFF, 4),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 2, 3, 4, 4]),       // pg_edge_offsets
                to_bytes(&[1, 2, 3, 3]),          // pg_edge_targets
                to_bytes(&[1, 1, 1, 1]),          // pg_edge_kind_mask
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0b0001]),              // frontier_in = {0}
                to_bytes(&[0]),                   // frontier_out
                to_bytes(&[0]),                   // changed
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            // After 4 iterations the graph 0→1,0→2,1→3,2→3 is fully closed.
            vec![vec![
                to_bytes(&[0b1111]),              // frontier_out = {0,1,2,3}
                to_bytes(&[1]),                   // changed
            ]]
        }),
    )
}

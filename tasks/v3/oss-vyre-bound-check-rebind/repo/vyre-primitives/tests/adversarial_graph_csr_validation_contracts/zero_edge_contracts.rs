use super::*;

#[test]
fn validate_zero_nodes_zero_edges_placeholder() {
    let shape = ProgramGraphShape::new(0, 0);
    // edge_count == 0 → read_only_buffers() emits count=1 placeholders
    let result = validate_program_graph(shape, &[], &[0], &[0], &[0], &[]);
    assert_eq!(result, Ok(()), "0-node/0-edge placeholder must validate");
}

#[test]
fn validate_nonzero_nodes_zero_edges_placeholder() {
    let shape = ProgramGraphShape::new(3, 0);
    // 3 nodes, 0 edges: edge_targets & edge_kind_mask must still be length 1
    let result = validate_program_graph(
        shape,
        &[0, 0, 0],    // nodes
        &[0, 0, 0, 0], // edge_offsets (3+1 entries, all zero)
        &[0],          // edge_targets placeholder
        &[0],          // edge_kind_mask placeholder
        &[0, 0, 0],    // node_tags
    );
    assert_eq!(
        result,
        Ok(()),
        ">0 nodes with 0 edges placeholder must validate"
    );
}

#[test]
fn csr_cpu_ref_zero_nodes_zero_edges() {
    let out = csr_cpu_ref(0, &[0], &[], &[], &[], 0xFFFF_FFFF);
    assert!(out.is_empty());
}

#[test]
fn csr_cpu_ref_nonzero_nodes_zero_edges() {
    let out = csr_cpu_ref(
        3,
        &[0, 0, 0, 0], // no outgoing edges for any node
        &[0],          // placeholder target
        &[0],          // placeholder mask
        &[0b0001],     // frontier on node 0
        0xFFFF_FFFF,
    );
    assert_eq!(out, vec![0], "zero edges → empty output frontier");
}


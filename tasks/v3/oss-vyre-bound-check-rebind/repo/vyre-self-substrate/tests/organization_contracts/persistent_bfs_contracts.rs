use super::*;

#[test]
fn persistent_bfs_resident_batch_uses_primitive_batch_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("persistent_bfs")
        .join("resident.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let batch_section = wrapper_source
        .split("pub fn bfs_expand_resident_graph_batch_with_scratch_into")
        .nth(1)
        .expect("resident batch BFS wrapper must exist")
        .split("fn ensure_resident_frontier_handles")
        .next()
        .expect("resident batch BFS wrapper must precede resident handle helpers");

    assert!(
        batch_section.contains("let plan = plan_persistent_bfs_resident_batch_dispatch"),
        "persistent BFS resident batch wrapper must delegate flat-frontier batch planning to vyre-primitives"
    );
    assert!(
        !batch_section.contains("graph.words.checked_mul(query_count)")
            && !batch_section.contains("frontier_inputs.len() != expected_words")
            && !batch_section.contains("u32::try_from(query_count)"),
        "persistent BFS resident batch wrapper must not own primitive batch overflow, length, or query-count validation"
    );
}

#[test]
fn persistent_bfs_resident_single_uses_primitive_frontier_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("persistent_bfs")
        .join("resident.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let single_section = wrapper_source
        .split("pub fn bfs_expand_resident_graph_with_scratch_into")
        .nth(1)
        .expect("resident single BFS wrapper must exist")
        .split("pub fn bfs_expand_resident_graph_batch_with_scratch_into")
        .next()
        .expect("resident single BFS wrapper must precede batch wrapper");

    assert!(
        single_section.contains("let plan = plan_persistent_bfs_resident_dispatch"),
        "persistent BFS resident single wrapper must delegate frontier planning to vyre-primitives"
    );
    assert!(
        single_section.contains("resident_dispatch_two_u32_outputs_into"),
        "persistent BFS resident single wrapper must use the shared resident readback dispatch bridge"
    );
    assert!(
        !single_section.contains("frontier_in.len() != graph.words")
            && !single_section.contains("u32::try_from(graph.words)"),
        "persistent BFS resident single wrapper must not own primitive frontier-width or word-count narrowing validation"
    );
}

#[test]
fn persistent_bfs_dispatch_paths_use_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("persistent_bfs")
        .join("dispatch.rs");
    let resident_path = manifest
        .join("src")
        .join("graph")
        .join("persistent_bfs")
        .join("resident.rs");
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let resident_source = std::fs::read_to_string(&resident_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", resident_path.display()));
    let via_section = dispatch_source
        .split("pub fn bfs_expand_via_with_scratch_into")
        .nth(1)
        .expect("non-resident persistent BFS wrapper must exist");
    let upload_section = resident_source
        .split("pub fn upload_resident_bfs_graph")
        .nth(1)
        .expect("resident persistent BFS graph upload must exist")
        .split("pub fn bfs_expand_resident_graph_with_scratch_into")
        .next()
        .expect("resident persistent BFS upload must precede query wrapper");
    let batch_section = resident_source
        .split("pub fn bfs_expand_resident_graph_batch_with_scratch_into")
        .nth(1)
        .expect("resident persistent BFS batch wrapper must exist")
        .split("fn ensure_resident_frontier_handles")
        .next()
        .expect("resident persistent BFS batch wrapper must precede handle helpers");

    assert!(
        via_section.contains("let plan = plan_persistent_bfs_dispatch"),
        "persistent BFS non-resident wrapper must use primitive-returned graph/frontier dispatch plan"
    );
    assert!(
        via_section.contains("refresh_keyed_dispatch_inputs")
            && via_section.contains("decode_u32_output_exact")
            && via_section.contains("changed_words"),
        "persistent BFS non-resident wrapper must reuse keyed graph input refresh and decode the primitive-owned changed scratch width explicitly"
    );
    assert!(
        !via_section.contains("bitset_words(node_count)")
            && !via_section.contains("node_count as usize")
            && !via_section.contains("u32::try_from(words)")
            && !via_section.contains("edge_targets.is_empty()")
            && !via_section.contains("edge_kind_mask.is_empty()"),
        "persistent BFS non-resident wrapper must not recompute frontier words, node scratch size, word narrowing, or edge padding"
    );

    assert!(
        upload_section.contains("let layout = validate_persistent_bfs_graph_layout"),
        "resident persistent BFS upload must use primitive-returned graph layout"
    );
    assert!(
        upload_section.contains("upload_resident_dispatch_inputs"),
        "resident persistent BFS upload must use the graph dispatch bridge for payload packing and failure-clean resident allocation"
    );
    assert!(
        !upload_section.contains("node_count as usize")
            && !upload_section.contains("let nodes = vec!")
            && !upload_section.contains("edge_targets.is_empty()")
            && !upload_section.contains("edge_kind_mask.is_empty()"),
        "resident persistent BFS upload must not recompute node scratch or edge padding layout"
    );

    assert!(
        batch_section.contains("resident_dispatch_two_u32_outputs_into") &&
        !batch_section.contains("u32::try_from(graph.words)"),
        "resident persistent BFS batch wrapper must reuse primitive-narrowed frontier word count and shared resident readback dispatch"
    );
}


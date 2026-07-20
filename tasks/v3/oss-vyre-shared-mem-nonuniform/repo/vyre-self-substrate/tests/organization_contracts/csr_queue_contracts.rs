use super::*;

#[test]
fn csr_frontier_queue_batch_resident_uses_primitive_batch_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("csr_frontier_queue_batch_resident.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("csr_frontier_queue_batch_resident")
        .join("dispatch.rs");
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let resident_source = format!("{wrapper_source}\n{dispatch_source}");

    assert!(
        resident_source.contains("validate_frontier_queue_batch"),
        "resident CSR queue batch wrapper must delegate batch-shape validation to vyre-primitives"
    );
    assert!(
        !resident_source.contains("fn validate_batch(")
            && !resident_source.contains("frontiers.is_empty()")
            && !resident_source.contains("queue_capacity == 0")
            && !resident_source.contains("frontier.len() != graph.words()"),
        "resident CSR queue batch wrapper must not own the primitive batch validation contract"
    );
}

#[test]
fn csr_frontier_queue_resident_uses_primitive_query_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let query_path = manifest
        .join("src")
        .join("graph")
        .join("csr_frontier_queue_resident")
        .join("query.rs");
    let query_section = std::fs::read_to_string(&query_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", query_path.display()));

    assert!(
        query_section.contains("validate_frontier_queue_query"),
        "resident CSR queue query wrapper must delegate queue/frontier validation to vyre-primitives"
    );
    assert!(
        !query_section.contains("if queue_capacity == 0")
            && !query_section.contains("frontier_words.is_empty()")
            && !query_section.contains("frontier_words.len() != graph.words"),
        "resident CSR queue query wrapper must not own primitive queue-capacity or frontier-width validation"
    );
}

#[test]
fn csr_frontier_queue_resident_graph_upload_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let upload_path = manifest
        .join("src")
        .join("graph")
        .join("csr_frontier_queue_resident")
        .join("upload.rs");
    let upload_section = std::fs::read_to_string(&upload_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", upload_path.display()));

    assert!(
        upload_section.contains("let layout =")
            && upload_section.contains("validate_csr_queue_graph"),
        "resident CSR queue graph upload must use primitive-returned graph layout"
    );
    assert!(
        !upload_section.contains("bitset_words(node_count)")
            && !upload_section.contains("let edge_count =")
            && !upload_section.contains("edge_targets,\n        1,")
            && !upload_section.contains("edge_kind_mask,\n        1,"),
        "resident CSR queue graph upload must not recompute primitive frontier width, edge count, or edge padding"
    );
}


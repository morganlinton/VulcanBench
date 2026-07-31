use super::*;

#[test]
fn motif_wrapper_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("motif")
        .join("mod.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("motif")
        .join("dispatch.rs");
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let release_section = dispatch_source
        .split("pub fn match_motif_via(")
        .nth(1)
        .expect("motif dispatch wrappers must exist")
        .split("pub fn motif_matches_via")
        .next()
        .expect("motif match wrapper must precede predicate wrappers");

    assert!(
        release_section.contains("let plan = plan_motif_launch"),
        "motif wrapper must use primitive-returned launch/cache plan"
    );
    assert!(
        wrapper_source.contains("pub use dispatch")
            && release_section.contains("dispatch_two_u32_outputs_from_prepared_into"),
        "motif wrapper must reuse the graph dispatch bridge instead of open-coding buffer/dispatch/decode plumbing"
    );
    assert!(
        !release_section.contains("validate_motif_inputs")
            && !release_section.contains("validate_motif_csr_inputs")
            && !release_section.contains("motif_edges.len() > u32::MAX")
            && !release_section.contains("u32::try_from")
            && !release_section.contains("node_count as usize")
            && !release_section.contains("edge_targets,\n        1,")
            && !release_section.contains("edge_kind_mask,\n        1,"),
        "motif wrapper must not recompute primitive motif edge-count, output layout, witness-count, or edge-buffer padding validation"
    );
}

#[test]
fn path_reconstruct_batch_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("path_reconstruct")
        .join("mod.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("path_reconstruct")
        .join("dispatch.rs");
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let batch_section = dispatch_source
        .split("pub fn reconstruct_paths_via_with_scratch_into")
        .nth(1)
        .expect("batched path reconstruction wrapper must exist")
        .split("dispatch_two_u32_outputs_into")
        .next()
        .expect("batched path reconstruction wrapper must cross the shared graph dispatch bridge");

    assert!(
        batch_section.contains("plan_batched_path_reconstruct_dispatch"),
        "batched path reconstruction wrapper must delegate target/depth layout validation to vyre-primitives"
    );
    assert!(
        wrapper_source.contains("pub use dispatch")
            && dispatch_source.contains("dispatch_two_u32_outputs_from_prepared_into"),
        "path reconstruction wrapper must stay a facade over the shared graph dispatch bridge"
    );
    assert!(
        !batch_section.contains("max_depth == 0")
            && !batch_section.contains("u32::try_from(targets.len())")
            && !batch_section.contains("checked_product_count")
            && !batch_section.contains("target_count.checked_mul(max_depth)"),
        "batched path reconstruction wrapper must not own primitive max-depth, target-count, or path-buffer overflow validation"
    );
}

#[test]
fn adaptive_traverse_resident_paths_use_primitive_frontier_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("adaptive_traverse")
        .join("resident_steps.rs");
    let release_path = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));

    assert!(
        release_path.matches("plan_adaptive_resident_frontier_step").count() >= 2
            && release_path.contains("plan_adaptive_resident_sparse_queue_step")
            && release_path.contains("plan_adaptive_resident_auto_step")
            && release_path.matches(".work.has_active_bits").count() >= 4,
        "adaptive resident sparse/dense, Four-Russians, sparse-queue, and auto paths must delegate frontier validation and zero-work classification to vyre-primitives resident planners"
    );
    assert!(
        release_path.matches("resident_sequence_single_u32_output_into").count() >= 2,
        "adaptive resident sparse/dense and sparse-queue paths must reuse the graph dispatch bridge for resident readback/decode"
    );
    assert!(
        !release_path.contains("frontier_in.len() != graph.words")
            && !release_path.contains("u32::try_from(graph.words)"),
        "adaptive resident wrappers must not own primitive frontier-width or word-count narrowing validation"
    );
}

#[test]
fn adaptive_traverse_resident_upload_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("adaptive_traverse")
        .join("upload.rs");
    let upload_section = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));

    assert!(
        upload_section.contains("let layout = validate_adaptive_traversal_layout"),
        "adaptive resident upload must use primitive-returned graph layout"
    );
    assert!(
        upload_section.contains("upload_resident_dispatch_inputs"),
        "adaptive resident upload must use the graph dispatch bridge for payload packing and failure-clean resident allocation"
    );
    assert!(
        !upload_section.contains("edge_targets.is_empty()")
            && !upload_section.contains("edge_kind_mask.is_empty()")
            && !upload_section.contains("dummy_edge"),
        "adaptive resident upload must not own primitive edge-buffer padding policy"
    );
}


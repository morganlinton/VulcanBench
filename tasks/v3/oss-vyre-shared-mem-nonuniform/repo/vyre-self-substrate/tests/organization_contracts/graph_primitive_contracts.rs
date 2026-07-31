use super::*;

#[test]
fn dominator_frontier_wrapper_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("dominator_frontier")
        .join("mod.rs");
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("dominator_frontier")
        .join("dispatch.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let via_section = dispatch_source
        .split("pub fn compute_dominance_frontier_via_with_scratch_into")
        .nth(1)
        .expect("dominance-frontier dispatch wrapper must exist")
        .split("dispatch_single_u32_output_from_prepared_into")
        .next()
        .expect(
            "dominance-frontier wrapper must cross the shared graph dispatch bridge after setup",
        );

    assert!(
        via_section.contains("let plan = plan_dominator_frontier_launch"),
        "dominance-frontier wrapper must use primitive-returned launch plan without eager IR rebuild"
    );
    assert!(
        wrapper_source.contains("pub use dispatch")
            && dispatch_source.contains("dispatch_single_u32_output_from_prepared_into"),
        "dominance-frontier wrapper must reuse the graph dispatch bridge instead of open-coding buffer/dispatch/decode plumbing"
    );
    assert!(
        !via_section.contains("bitset_words(node_count)")
            && !via_section.contains("u32::try_from(dom_targets.len())")
            && !via_section.contains("u32::try_from(pred_targets.len())"),
        "dominance-frontier wrapper must not recompute primitive frontier words or CSR edge-count narrowing"
    );
}

#[test]
fn csr_bidirectional_wrapper_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("csr_bidirectional")
        .join("mod.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("csr_bidirectional")
        .join("dispatch.rs");
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let via_section = dispatch_source
        .split("pub fn bidirectional_step_via_with_scratch_into")
        .nth(1)
        .expect("bidirectional dispatch wrapper must exist")
        .split("dispatch_single_u32_output_from_prepared_into")
        .next()
        .expect("bidirectional wrapper must cross the shared graph dispatch bridge after setup");

    assert!(
        via_section.contains("let plan = plan_csr_bidirectional_step"),
        "bidirectional wrapper must use primitive-returned dispatch layout"
    );
    assert!(
        wrapper_source.contains("pub use dispatch")
            && dispatch_source.contains("dispatch_single_u32_output_from_prepared_into"),
        "bidirectional wrapper must reuse the graph dispatch bridge instead of open-coding buffer/dispatch/decode plumbing"
    );
    assert!(
        !via_section.contains("bitset_words(node_count)")
            && !via_section.contains("node_count as usize")
            && !via_section.contains("ProgramGraphShape::new(node_count")
            && !via_section.contains("let edge_count = validate_csr_bidirectional_inputs")
            && !via_section.contains("edge_targets.is_empty()")
            && !via_section.contains("edge_kind_mask.is_empty()"),
        "bidirectional wrapper must not recompute primitive frontier words, node scratch length, edge-count layout, or edge-buffer padding"
    );
}

#[test]
fn csr_forward_or_changed_wrapper_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("csr_forward_or_changed")
        .join("mod.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("csr_forward_or_changed")
        .join("dispatch.rs");
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let via_section = dispatch_source
        .split("pub fn forward_closure_via_change_flag_gpu_with_scratch_into")
        .nth(1)
        .expect("forward-or-changed dispatch wrapper must exist")
        .split("for iter in 0..max_iters")
        .next()
        .expect("forward-or-changed wrapper must prepare dispatch before loop");

    assert!(
        via_section.contains("let plan = plan_csr_forward_or_changed_launch")
            && via_section.contains("program_cache.get_or_try_insert_with("),
        "forward-or-changed wrapper must use the primitive-owned launch plan and shared program cache"
    );
    assert!(
        wrapper_source.contains("pub use dispatch")
            && dispatch_source.contains("refresh_keyed_dispatch_inputs")
            && dispatch_source.contains("write_dispatch_input")
            && dispatch_source.contains("dispatch_two_u32_outputs_from_prepared_into"),
        "forward-or-changed wrapper must reuse the graph dispatch bridge without re-copying fixed CSR buffers per iteration"
    );
    assert!(
        !via_section.contains("checked_add(1)")
            && !via_section.contains("edge_targets.len() > u32::MAX")
            && !via_section.contains("edge_kind_mask.len() as u32")
            && !via_section.contains("node_count.max(1) as usize")
            && !via_section.contains("frontier_words = frontier.len()"),
        "forward-or-changed wrapper must not own primitive offset, edge-count, node scratch, or frontier layout validation"
    );
}

#[test]
fn toposort_wrapper_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("toposort")
        .join("mod.rs");
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("toposort")
        .join("dispatch.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let via_section = dispatch_source
        .split("pub fn topo_order_csr_via_with_scratch_into")
        .nth(1)
        .expect("toposort dispatch wrapper must exist")
        .split("dispatch_single_u32_output_from_prepared_into")
        .next()
        .expect("toposort wrapper must cross the shared graph dispatch bridge after setup");

    assert!(
        via_section.contains("let plan =") && via_section.contains("plan_toposort_csr_dispatch"),
        "toposort wrapper must use primitive-returned dispatch layout"
    );
    assert!(
        wrapper_source.contains("pub use dispatch")
            && dispatch_source.contains("dispatch_single_u32_output_from_prepared_into"),
        "toposort wrapper must reuse the graph dispatch bridge instead of open-coding buffer/dispatch/decode plumbing"
    );
    assert!(
        !via_section.contains("let node_words = node_count as usize")
            && !via_section.contains("toposort_program(\n        node_count")
            && !via_section.contains("u32_word_bytes(node_count"),
        "toposort wrapper must not recompute primitive node scratch or program-shape layout"
    );
}

#[test]
fn union_find_wrapper_uses_primitive_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("union_find_emit.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("union_find_emit")
        .join("dispatch.rs");
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let via_section = dispatch_source
        .split("pub fn union_find_alias_via_with_scratch_into")
        .nth(1)
        .expect("union-find dispatch wrapper must exist")
        .split("dispatch_single_u32_output_from_prepared_into")
        .next()
        .expect("union-find wrapper must cross the shared graph dispatch bridge after setup");

    assert!(
        via_section.contains("let layout = validate_union_find_inputs"),
        "union-find wrapper must use primitive-returned dispatch layout"
    );
    assert!(
        wrapper_source.contains("pub use dispatch")
            && dispatch_source.contains("dispatch_single_u32_output_from_prepared_into"),
        "union-find wrapper must stay a facade over dispatch.rs and reuse the graph dispatch bridge"
    );
    assert!(
        !via_section.contains("node_count as usize")
            && !via_section.contains("let (node_count, edge_count)")
            && !via_section.contains("edge_a,\n        1,")
            && !via_section.contains("edge_b,\n        1,"),
        "union-find wrapper must not recompute primitive output width or edge-buffer padding"
    );
}

#[test]
fn exploded_wrapper_uses_primitive_input_layout_contract() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let wrapper_path = manifest
        .join("src")
        .join("graph")
        .join("exploded")
        .join("mod.rs");
    let wrapper_source = std::fs::read_to_string(&wrapper_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", wrapper_path.display()));
    let dispatch_path = manifest
        .join("src")
        .join("graph")
        .join("exploded")
        .join("dispatch.rs");
    let dispatch_source = std::fs::read_to_string(&dispatch_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dispatch_path.display()));
    let via_section = dispatch_source
        .split("pub fn build_ifds_csr_via_with_scratch_into")
        .nth(1)
        .expect("exploded IFDS dispatch wrapper must exist")
        .split("dispatch_ifds_csr_outputs_from_prepared_into")
        .next()
        .expect("exploded IFDS wrapper must cross the shared graph dispatch bridge after setup");

    assert!(
        via_section.contains("let plan = plan_ifds_csr_dispatch"),
        "exploded IFDS wrapper must use primitive-returned input/count layout"
    );
    assert!(
        wrapper_source.contains("pub use dispatch")
            && dispatch_source.contains("refresh_keyed_dispatch_inputs")
            && dispatch_source.contains("dispatch_ifds_csr_outputs_from_prepared_into"),
        "exploded IFDS wrapper must reuse the graph dispatch bridge instead of open-coding 17-input/four-output byte plumbing"
    );
    assert!(
        !via_section.contains("u32::try_from")
            && !via_section.contains("intra_edges.len()")
            && !via_section.contains("inter_edges.len()")
            && !via_section.contains("flow_gen.len()")
            && !via_section.contains("flow_kill.len()")
            && !via_section.contains("validate_ifds_csr_layout")
            && !via_section.contains("&scratch.intra_proc,\n        1,")
            && !via_section.contains("&scratch.inter_src_proc,\n        1,")
            && !via_section.contains("&scratch.gen_proc,\n        1,")
            && !via_section.contains("&scratch.kill_proc,\n        1,"),
        "exploded IFDS wrapper must not own primitive count narrowing, layout validation, or input-buffer padding"
    );
}


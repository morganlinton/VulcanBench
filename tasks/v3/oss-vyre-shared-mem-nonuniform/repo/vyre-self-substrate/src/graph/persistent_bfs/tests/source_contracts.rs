#[test]
fn persistent_bfs_uses_shared_bounded_plan_cache() {
    let source = include_str!("../state.rs");
    assert!(
        source.contains("use crate::graph::plan_cache::GraphPlanCache;"),
        "Fix: persistent BFS must use the shared bounded graph plan cache."
    );
    assert!(
        !source.contains("HashMap<PersistentBfsPlanKey"),
        "Fix: persistent BFS must not carry a private unbounded Program HashMap cache."
    );
}

#[test]
fn bfs_expand_via_scratch_caches_static_graph_inputs() {
    let state_source = include_str!("../state.rs");
    let dispatch_source = include_str!("../dispatch.rs");

    assert!(
        state_source.contains("static_input_key: Option<PersistentBfsStaticInputKey>"),
        "Fix: persistent BFS scratch must remember the prepared static CSR graph inputs."
    );
    assert!(
        state_source.contains("PersistentBfsStaticInputKey")
            && dispatch_source.contains("plan.static_input_key()"),
        "Fix: persistent BFS static input reuse must use the primitive-owned graph input key, not call order."
    );
    assert!(
        dispatch_source.contains("refresh_keyed_dispatch_inputs("),
        "Fix: persistent BFS must use the shared keyed graph dispatch refresh helper."
    );
    assert!(
        dispatch_source.contains("program_cache_key("),
        "Fix: persistent BFS program caching must be shape-keyed instead of graph-content keyed."
    );
    assert!(
        dispatch_source.contains("(5, DispatchInput::u32_slice(frontier_in))")
            && dispatch_source.contains("DispatchInput::zero_u32_words(words, \"bfs_expand_via frontier_out\")")
            && dispatch_source.contains("DispatchInput::zero_u32_words(changed_words, \"bfs_expand_via changed\")"),
        "Fix: repeated persistent BFS dispatches must rewrite only frontier, output, and changed slots."
    );
}

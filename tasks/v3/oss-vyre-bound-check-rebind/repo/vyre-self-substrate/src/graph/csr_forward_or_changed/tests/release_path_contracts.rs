#[test]
fn release_gpu_path_does_not_call_cpu_or_local_saturating_helpers() {
    let release_path = include_str!("../dispatch.rs");
    assert!(!release_path.contains("csr_foc_cpu"));
    assert!(!release_path.contains("reference_"));
    assert!(!release_path.contains("saturating_mul"));
    assert!(!release_path.contains("fill_"));
    assert!(!release_path.contains("Vec::with_capacity"));
    assert!(release_path.contains("reserve_graph_vec"));
    assert!(release_path.contains("copy_csr_forward_seed_frontier_into"));
    assert!(!release_path.contains("fn reserve_forward_changed_vec"));
}

#[test]
fn release_gpu_path_uses_primitive_owned_static_input_key_and_changed_flag_validation() {
    let release_path = include_str!("../dispatch.rs");

    assert!(release_path.contains("CsrForwardOrChangedStaticInputKey"));
    assert!(release_path.contains(".static_input_key(edge_offsets, edge_targets, edge_kind_mask)"));
    assert!(release_path.contains("validate_csr_forward_or_changed_flag"));
    assert!(!release_path.contains("struct ForwardChangedStaticInputKey"));
    assert!(!release_path.contains("fingerprint_u32_slice"));
    assert!(!release_path.contains("U32SliceFingerprint"));
}

#[test]
fn release_gpu_path_uses_parallel_primitive_and_node_grid() {
    let release_path = include_str!("../dispatch.rs");

    assert!(
        release_path.contains("plan_csr_forward_or_changed_launch"),
        "Fix: CSR forward closure GPU path must use the primitive-owned launch plan."
    );
    assert!(
        !release_path.contains("plan_csr_forward_or_changed_dispatch"),
        "Fix: CSR forward closure GPU path must not rebuild an eager primitive dispatch plan when scratch caching is available."
    );
    assert!(
        !release_path.contains("let program = csr_forward_or_changed("),
        "Fix: CSR forward closure GPU path must not dispatch the serial single-invocation primitive."
    );
    assert!(
        release_path.contains("Some(plan.dispatch_grid())"),
        "Fix: CSR forward closure GPU path must launch with the primitive-owned node grid."
    );
    let program_build = release_path
        .find("program_cache.get_or_try_insert_with(")
        .expect(
        "Fix: CSR forward closure GPU path must populate the shared primitive program cache once.",
    );
    let loop_start = release_path
        .find("for iter in 0..max_iters")
        .expect("Fix: CSR forward closure GPU path must have an iteration loop.");
    assert!(
        program_build < loop_start,
        "Fix: CSR forward closure GPU path must cache the primitive program before the fixpoint loop."
    );
    assert!(
        !release_path[loop_start..].contains("plan.program()"),
        "Fix: CSR forward closure GPU path must not rebuild the primitive program on every fixpoint iteration."
    );
}

#[test]
fn release_gpu_path_uses_changed_history_for_short_fixpoints() {
    let release_path = include_str!("../dispatch.rs");
    // The primitive was split into submodules (LAW7 module splits); the
    // dispatch plan, dynamic-slot kernel, and fast-path threshold now live
    // in plan.rs / program_parallel_batch_global.rs / layout.rs.
    let primitive_source = concat!(
        include_str!("../../../../../vyre-primitives/src/graph/csr_forward_or_changed/plan.rs"),
        include_str!(
            "../../../../../vyre-primitives/src/graph/csr_forward_or_changed/program_parallel_batch_global.rs"
        ),
        include_str!("../../../../../vyre-primitives/src/graph/csr_forward_or_changed/layout.rs"),
    );

    assert!(
        primitive_source.contains("pub(crate) fn plan_csr_forward_or_changed_dispatch")
            && primitive_source
                .contains("try_csr_forward_or_changed_parallel_batch_global_dynamic_slot"),
        "Fix: short CSR fixpoint loops must use the primitive dynamic changed-slot kernel through the plan."
    );
    assert!(
        primitive_source.contains("CSR_FORWARD_OR_CHANGED_HISTORY_FAST_PATH_MAX_ITERS"),
        "Fix: changed-history readback must be bounded by a release-path threshold."
    );
    assert!(
        release_path.contains("changed history scratch")
            && release_path.contains("plan.changed_slot_value(iter)")
            && release_path.contains(".changed_read_index(iter)"),
        "Fix: changed history must be zeroed once and advanced/read through primitive-owned iteration policy."
    );
}

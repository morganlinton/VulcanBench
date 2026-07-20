#[test]
fn release_via_path_does_not_call_cpu_or_local_saturating_helpers() {
    let step_source = include_str!("../dispatch.rs");
    let closure_source = include_str!("../closure.rs");
    let start = step_source
        .find("pub fn bidirectional_step_via")
        .expect("Fix: via path marker must exist");
    let end = step_source
        .find("pub(super) fn bidirectional_step_dispatch_prepared_inputs_into")
        .expect("Fix: prepared-step helper marker must exist");
    let release_path = &step_source[start..end];
    assert!(!release_path.contains("reference_csr_bidir"));
    assert!(!release_path.contains("reference_"));
    assert!(!release_path.contains("saturating_mul"));
    assert!(!release_path.contains("fill_"));
    assert!(!release_path.contains("u32_slice_padded_to_words"));
    assert!(release_path.contains("refresh_bidirectional_step_inputs("));
    assert!(!release_path.contains("fn merge_frontier_or_changed"));
    let closure_start = closure_source
        .find("pub fn bidirectional_closure_via_with_scratch_into")
        .expect("Fix: bidirectional closure release path marker must exist.");
    let closure_path = &closure_source[closure_start..];
    let runner_call = closure_path
        .find("run_csr_bidirectional_closure_plan_with_step(")
        .expect(
            "Fix: bidirectional closure must delegate fixpoint semantics to the primitive runner.",
        );
    let program_build = closure_path
        .find("program_cache.get_or_insert_with(")
        .expect(
            "Fix: bidirectional closure step executor must use the shared primitive program cache.",
        );
    assert!(
        runner_call < program_build,
        "Fix: bidirectional closure must pass a cached dispatch step into the primitive-owned runner."
    );
    assert!(
        !closure_path.contains("for _ in 0..max_iters"),
        "Fix: bidirectional closure must not fork the primitive-owned fixpoint loop."
    );
    assert!(
        !closure_path.contains("merge_frontier_or_changed"),
        "Fix: bidirectional closure must not fork primitive frontier merge semantics."
    );
    assert!(
        !closure_path.contains("bidirectional_step_via_with_scratch_into("),
        "Fix: bidirectional closure must not replan/rebuild through the per-step wrapper on every iteration."
    );
}

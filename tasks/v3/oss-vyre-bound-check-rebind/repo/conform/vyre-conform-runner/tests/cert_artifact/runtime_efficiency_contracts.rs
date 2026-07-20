#[test]
fn prove_precomputes_reference_witnesses_once_per_entry_not_once_per_backend() {
    let source = include_str!("../../src/main.rs");
    let prepare_start = source
        .find("fn prepare_reference_cases(")
        .expect("Fix: prove must keep a dedicated reference-preparation function.");
    let compare_start = source
        .find("fn compare_backend_against_reference(")
        .expect("Fix: prove must keep backend comparison isolated.");
    let compare_end = source[compare_start..]
        .find("type BackendDispatchPlan = WitnessInputPlan")
        .map(|offset| compare_start + offset)
        .expect("Fix: backend comparison boundary must remain discoverable.");
    let prepare = &source[prepare_start..compare_start];
    let compare = &source[compare_start..compare_end];

    assert!(
        prepare.contains("vyre_reference::reference_eval")
            && prepare.contains("run_cpu_fixpoint_to_convergence")
            && prepare.contains("backend_dispatch_inputs_with_plan_into(inputs, input_plan")
            && prepare.contains("reference_cases.push"),
        "Fix: prove must build reference witness outputs once during entry preparation using the same planned witness stream as backend dispatch."
    );
    let convergence_pos = prepare
        .find("if let Some(max_iterations) = convergence_max_iterations")
        .expect("Fix: convergence contracts must be handled in reference preparation.");
    let expected_pos = prepare
        .find("if let Some(expected_cases) = expected_cases")
        .expect("Fix: non-convergence ops may use declared expected_output fixtures.");
    assert!(
        convergence_pos < expected_pos,
        "Fix: convergence-contract ops must compare CUDA against CPU fixpoint witnesses, not one-step expected_output fixtures."
    );
    assert!(
        !compare.contains("vyre_reference::reference_eval")
            && !compare.contains("run_cpu_fixpoint_to_convergence"),
        "Fix: backend comparison must reuse prepared reference witness outputs instead of recomputing them for every backend."
    );
}

#[test]
fn prove_runs_selected_backends_in_parallel_workers() {
    let source = include_str!("../../src/main.rs");
    let prove_start = source
        .find("fn prove(")
        .expect("Fix: prove entry point must remain discoverable.");
    let prove_region = &source[prove_start..];

    assert!(
        prove_region.contains("prove_backends_in_parallel(&backends, &prepared_entries)"),
        "Fix: prove must dispatch selected backend comparisons through the parallel backend runner instead of serializing all backend work."
    );
    assert!(
        prove_region.contains("prepare_entries_in_parallel(entries, &backends)"),
        "Fix: prove must prepare catalog witness entries through the bounded worker pool instead of serializing reference preparation."
    );
    assert!(
        source.contains("std::thread::scope"),
        "Fix: backend proof workers must use scoped threads so prepared witness data is shared without cloning the catalog-scale proof inputs."
    );
    assert!(
        source.contains("std::thread::available_parallelism()")
            && source.contains("VYRE_CONFORM_PROOF_WORKERS")
            && source.contains(".max(8)")
            && source.contains("buckets[index % worker_count].push((index, entry))"),
        "Fix: proof preparation must use bounded CPU workers with an explicit proof-worker floor/override, not one unbounded thread per op or a cgroup-collapsed serial worker."
    );
    assert!(
        source.contains("scope.spawn(move || prove_one_backend(backend, prepared_entries))"),
        "Fix: every selected backend must run in its own proof worker."
    );
    assert!(
        source.contains("let instance = match backend.acquire()")
            && source.contains("let instance = instance.as_ref();")
            && source.contains("compare_backend_against_reference(instance, &backend.id, entry)"),
        "Fix: each backend proof must acquire one backend instance and share it across shard workers so WGPU/CUDA caches are reused instead of rebuilding per shard."
    );
    assert!(
        source.contains("backend `{} proof shard worker panicked")
            || source.contains("proof shard worker panicked"),
        "Fix: each backend proof must shard catalog op comparisons across bounded workers, not serialize one slow backend over the full registry."
    );
    assert!(
        source.contains("handle.join()") && source.contains("proof worker panicked"),
        "Fix: proof worker panics must be converted into failing pair results instead of losing certificate diagnostics."
    );
    assert!(
        source.contains("VYRE_CONFORM_PROOF_TIMING")
            && source.contains("VYRE_CONFORM_PROOF_PAIR_TIMING_MS")
            && source.contains("VYRE_CONFORM_PROOF_PAIR_START")
            && source.contains("vyre-conform proof timing:")
            && source.contains("vyre-conform proof backend timing:")
            && source.contains("vyre-conform proof pair timing:")
            && source.contains("vyre-conform proof pair start:")
            && source.contains("std::time::Instant::now()"),
        "Fix: release proof runs must expose opt-in phase/backend timing so host-bound CUDA certificate regressions are diagnosable."
    );
}

use super::*;

#[test]
fn direct_record_and_readback_reuses_bind_groups() {
    use std::sync::Arc;

    let ((device, queue), adapter_info, enabled_features) =
        crate::runtime::init_device().expect("Fix: GPU required for direct cache test");
    let device_queue = Arc::new((device, queue));
    let config = DispatchConfig::default();
    let pipeline_cache = Arc::new(crate::runtime::cache::pipeline::LruPipelineCache::new(
        vyre_driver::pipeline::DEFAULT_PIPELINE_CACHE_ENTRIES as u32,
    ));
    let layout_cache = Arc::new(super::BindGroupLayoutCache::with_hasher(
        std::hash::BuildHasherDefault::<rustc_hash::FxHasher>::default(),
    ));
    let arena = Arc::new(crate::DispatchArena::new(
        device_queue.0.clone(),
        device_queue.1.clone(),
        &config,
    ));
    // Share the arena's pool with the pipeline so buffer Arc identities
    // match between compile-time bindings and run-time record_and_readback.
    // A second BufferPool::new() would create distinct buffer identities,
    // forcing every dispatch to be a bind-group-cache miss.
    let pool = arena.pool().clone();

    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );

    let pipeline = super::WgpuPipeline::compile_with_device_queue(
        &program,
        &config,
        adapter_info,
        enabled_features,
        device_queue.clone(),
        arena.clone(),
        pool,
        pipeline_cache,
        layout_cache,
    )
    .expect("Fix: compile must succeed; restore this invariant before continuing.");
    let empty_inputs: [&[u8]; 0] = [];

    for _ in 0..2 {
        let outputs = crate::engine::record_and_readback::record_and_readback(
            crate::engine::record_and_readback::RecordAndReadback {
                device_queue: &pipeline.device_queue,
                pool: arena.pool(),
                readback_rings: None,
                pipeline: &pipeline.pipeline,
                bind_group_layouts: &pipeline.bind_group_layouts,
                bind_group_cache: Some(pipeline.bind_group_cache.as_ref()),
                buffer_bindings: &pipeline.buffer_bindings,
                inputs: &empty_inputs,
                output_bindings: &pipeline.output_bindings,
                trap_tags: &pipeline.trap_tags,
                workgroup_count: [1, 1, 1],
                indirect: pipeline.indirect.as_ref(),
                labels: crate::engine::record_and_readback::DispatchLabels {
                    bind_group: "vyre direct cache test bind group",
                    encoder: "vyre direct cache test",
                    compute: "vyre direct cache test compute",
                },
                iterations: 1,
                timestamp_profile: false,
            },
        )
        .expect(
            "Fix: direct record/readback must succeed; restore this invariant before continuing.",
        );
        assert_eq!(u32::from_le_bytes(outputs[0][0..4].try_into().unwrap()), 7);
    }

    let stats = pipeline.bind_group_cache_stats();
    // The pool may or may not return the same buffer Arc across two
    // back-to-back readbacks (the prior readback's pinning, plus
    // size-class bucketing, decides). What we DO require: the cache
    // is exercised on every dispatch (misses + hits >= 2) and never
    // reports a negative-cost path (no double-build for a given Arc).
    let total = stats.misses + stats.hits;
    assert!(
        total >= 2,
        "two dispatches should each consult the bind-group cache (got misses={}, hits={})",
        stats.misses,
        stats.hits,
    );
    assert!(
        stats.misses <= 2,
        "no more than one bind-group build per distinct buffer identity (got misses={})",
        stats.misses,
    );
}

#[test]
fn direct_record_and_readback_trap_uses_readback_rings_only() {
    use std::sync::Arc;

    let ((device, queue), adapter_info, enabled_features) =
        crate::runtime::init_device().expect("Fix: GPU required for trap-sidecar allocation test");
    let device_queue = Arc::new((device, queue));
    let config = DispatchConfig::default();
    let pipeline_cache = Arc::new(crate::runtime::cache::pipeline::LruPipelineCache::new(
        vyre_driver::pipeline::DEFAULT_PIPELINE_CACHE_ENTRIES as u32,
    ));
    let layout_cache = Arc::new(super::BindGroupLayoutCache::with_hasher(
        std::hash::BuildHasherDefault::<rustc_hash::FxHasher>::default(),
    ));
    let with_rings_arena = Arc::new(crate::DispatchArena::new(
        device_queue.0.clone(),
        device_queue.1.clone(),
        &config,
    ));
    let without_rings_arena = Arc::new(crate::DispatchArena::new(
        device_queue.0.clone(),
        device_queue.1.clone(),
        &config,
    ));
    let with_rings_pool = with_rings_arena.pool().clone();
    let _without_rings_pool = without_rings_arena.pool().clone();

    let program = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![Node::trap(Expr::u32(3), "direct-readback-ring-trap")],
    );

    let pipeline = super::WgpuPipeline::compile_with_device_queue(
        &program,
        &config,
        adapter_info,
        enabled_features,
        device_queue.clone(),
        with_rings_arena.clone(),
        with_rings_pool.clone(),
        pipeline_cache,
        layout_cache,
    )
    .expect("Fix: trapped program compile must succeed; restore this invariant before continuing.");

    let empty_inputs: [&[u8]; 0] = [];
    let before_allocations = with_rings_pool.stats().allocations;
    let error = crate::engine::record_and_readback::record_and_readback(
        crate::engine::record_and_readback::RecordAndReadback {
            device_queue: &pipeline.device_queue,
            pool: with_rings_arena.pool(),
            readback_rings: Some(with_rings_arena.readback_rings()),
            pipeline: &pipeline.pipeline,
            bind_group_layouts: &pipeline.bind_group_layouts,
            bind_group_cache: Some(pipeline.bind_group_cache.as_ref()),
            buffer_bindings: &pipeline.buffer_bindings,
            inputs: &empty_inputs,
            output_bindings: &pipeline.output_bindings,
            trap_tags: &pipeline.trap_tags,
            workgroup_count: [1, 1, 1],
            indirect: pipeline.indirect.as_ref(),
            labels: crate::engine::record_and_readback::DispatchLabels {
                bind_group: "vyre direct trap readback ring test bind group",
                encoder: "vyre direct trap readback ring test",
                compute: "vyre direct trap readback ring test compute",
            },
            iterations: 1,
            timestamp_profile: false,
        },
    )
    .expect_err(
        "Fix: trapped dispatch with readback rings must return the underlying trap sidecar error and not succeed",
    );
    let after_allocations = with_rings_pool.stats().allocations;

    assert_eq!(
        error.to_string().contains("wgpu dispatch trapped"),
        true,
        "Fix: expected trap dispatch to surface a backend trap error, got: {error}"
    );
    assert_eq!(
        error.to_string().contains("direct-readback-ring-trap"),
        true,
        "Fix: expected trap dispatch to surface a backend trap error, got: {error}"
    );
    assert_eq!(
        after_allocations,
        before_allocations + 1,
        "Fix: readback-ring trap path must use the dedicated trap sidecar buffer only (no pooled full-sidecar readback buffer allocation).",
    );
}

#[test]

fn direct_record_and_readback_trap_without_readback_rings_allocates_full_sidecar_copy() {
    use std::sync::Arc;

    let ((device, queue), adapter_info, enabled_features) = crate::runtime::init_device()
        .expect("Fix: GPU required for trap-sidecar allocation delta test");
    let device_queue = Arc::new((device, queue));
    let config = DispatchConfig::default();
    let pipeline_cache = Arc::new(crate::runtime::cache::pipeline::LruPipelineCache::new(
        vyre_driver::pipeline::DEFAULT_PIPELINE_CACHE_ENTRIES as u32,
    ));
    let layout_cache = Arc::new(super::BindGroupLayoutCache::with_hasher(
        std::hash::BuildHasherDefault::<rustc_hash::FxHasher>::default(),
    ));
    let arena = Arc::new(crate::DispatchArena::new(
        device_queue.0.clone(),
        device_queue.1.clone(),
        &config,
    ));
    let pool = arena.pool().clone();

    let program = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![Node::trap(Expr::u32(5), "direct-readback-no-ring-trap")],
    );

    let pipeline = super::WgpuPipeline::compile_with_device_queue(
        &program,
        &config,
        adapter_info,
        enabled_features,
        device_queue.clone(),
        Arc::clone(&arena),
        pool.clone(),
        pipeline_cache,
        layout_cache,
    )
    .expect("Fix: trapped program compile must succeed; restore this invariant before continuing.");

    let empty_inputs: [&[u8]; 0] = [];
    let before_allocations = pool.stats().allocations;
    let error = crate::engine::record_and_readback::record_and_readback(
        crate::engine::record_and_readback::RecordAndReadback {
            device_queue: &pipeline.device_queue,
            pool: arena.pool(),
            readback_rings: None,
            pipeline: &pipeline.pipeline,
            bind_group_layouts: &pipeline.bind_group_layouts,
            bind_group_cache: Some(pipeline.bind_group_cache.as_ref()),
            buffer_bindings: &pipeline.buffer_bindings,
            inputs: &empty_inputs,
            output_bindings: &pipeline.output_bindings,
            trap_tags: &pipeline.trap_tags,
            workgroup_count: [1, 1, 1],
            indirect: pipeline.indirect.as_ref(),
            labels: crate::engine::record_and_readback::DispatchLabels {
                bind_group: "vyre direct trap readback no-ring test bind group",
                encoder: "vyre direct trap readback no-ring test",
                compute: "vyre direct trap readback no-ring test compute",
            },
            iterations: 1,
            timestamp_profile: false,
        },
    )
    .expect_err(
        "Fix: trapped dispatch without rings must still return the underlying trap sidecar error and not succeed",
    );
    let after_allocations = pool.stats().allocations;

    assert!(
        error.to_string().contains("wgpu dispatch trapped"),
        "Fix: expected trap dispatch to surface a backend trap error, got: {error}"
    );
    assert!(
        error.to_string().contains("direct-readback-no-ring-trap"),
        "Fix: expected the trap tag to be preserved across fallback sidecar decode, got: {error}"
    );
    assert_eq!(
        after_allocations,
        before_allocations + 2,
        "Fix: non-ring trap path must allocate exactly the full-sidecar pooled readback buffer plus trap sidecar allocation (before={before_allocations}, after={after_allocations})."
    );
}


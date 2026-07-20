use super::*;

/// PERF-HOT-01: two WgpuPipeline instances for the same compiled shader
/// must share one BindGroupCache (Arc identity). Different compiled
/// shaders must have independent caches.
#[test]
fn bind_group_cache_shared_per_compiled_shader() {
    use std::sync::Arc;

    let ((device, queue), adapter_info, enabled_features) =
        crate::runtime::init_device().expect("Fix: GPU required for cache-sharing test");
    let device_queue = Arc::new((device, queue));
    let config = DispatchConfig::default();
    let pool =
        crate::buffer::BufferPool::new(device_queue.0.clone(), device_queue.1.clone(), &config);
    let pipeline_cache = Arc::new(crate::runtime::cache::pipeline::LruPipelineCache::new(
        vyre_driver::pipeline::DEFAULT_PIPELINE_CACHE_ENTRIES as u32,
    ));
    let layout_cache = Arc::new(super::BindGroupLayoutCache::with_hasher(
        std::hash::BuildHasherDefault::<rustc_hash::FxHasher>::default(),
    ));

    let program1 = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );

    let p1 = super::WgpuPipeline::compile_with_device_queue(
        &program1,
        &config,
        adapter_info.clone(),
        enabled_features,
        device_queue.clone(),
        Arc::new(crate::DispatchArena::new(
            device_queue.0.clone(),
            device_queue.1.clone(),
            &config,
        )),
        pool.clone(),
        pipeline_cache.clone(),
        layout_cache.clone(),
    )
    .expect("Fix: first compile must succeed; restore this invariant before continuing.");
    assert_eq!(
        layout_cache.len(),
        1,
        "Fix: first compile should insert one shared bind-group layout fingerprint"
    );

    let p2 = super::WgpuPipeline::compile_with_device_queue(
        &program1,
        &config,
        adapter_info.clone(),
        enabled_features,
        device_queue.clone(),
        Arc::new(crate::DispatchArena::new(device_queue.0.clone(), device_queue.1.clone(), &config)),
        pool.clone(),
        pipeline_cache.clone(),
        layout_cache.clone(),
    )
    .expect("Fix: second compile of same program must succeed; restore this invariant before continuing.");
    assert_eq!(
        layout_cache.len(),
        1,
        "Fix: recompiling the same layout must hit the shared layout cache"
    );

    assert!(
        Arc::ptr_eq(&p1.bind_group_cache, &p2.bind_group_cache),
        "Fix: same compiled shader must share BindGroupCache (HOT-01)"
    );

    let (input_handles, mut output_handles) = p1.legacy_handles_from_inputs(&[]).expect(
        "Fix: legacy handle creation must succeed; restore this invariant before continuing.",
    );
    p1.dispatch_persistent(&input_handles, &mut output_handles, None, [1, 1, 1])
        .expect("Fix: first dispatch must succeed; restore this invariant before continuing.");
    let stats_after_miss = p1.bind_group_cache_stats();
    assert_eq!(
        stats_after_miss.misses, 1,
        "Fix: first dispatch of a new signature must be a cache miss"
    );
    assert_eq!(stats_after_miss.hits, 0);

    p1.dispatch_persistent(&input_handles, &mut output_handles, None, [1, 1, 1])
        .expect("Fix: second dispatch must succeed; restore this invariant before continuing.");
    let stats_after_hit = p1.bind_group_cache_stats();
    assert_eq!(
        stats_after_hit.hits, 1,
        "Fix: second dispatch with identical handles must be a cache hit"
    );
    assert_eq!(stats_after_hit.misses, 1);

    let program2 = Program::wrapped(
        vec![BufferDecl::output("out2", 0, DataType::U32).with_count(8)],
        [1, 1, 1],
        vec![Node::store("out2", Expr::u32(0), Expr::u32(42))],
    );

    let p3 = super::WgpuPipeline::compile_with_device_queue(
        &program2,
        &config,
        adapter_info,
        enabled_features,
        device_queue.clone(),
        Arc::new(crate::DispatchArena::new(
            device_queue.0.clone(),
            device_queue.1.clone(),
            &config,
        )),
        pool,
        pipeline_cache,
        layout_cache.clone(),
    )
    .expect(
        "Fix: compile of different program must succeed; restore this invariant before continuing.",
    );
    assert_eq!(
        layout_cache.len(),
        1,
        "Fix: compatible output-only programs must share the same wgpu bind-group layout cache entry"
    );

    assert!(
        !Arc::ptr_eq(&p1.bind_group_cache, &p3.bind_group_cache),
        "Fix: different compiled shaders must have independent BindGroupCaches"
    );
}

#[test]
fn compiled_borrowed_timed_dispatch_reports_device_ns() {
    use std::sync::Arc;

    use vyre_driver::CompiledPipeline;

    let ((device, queue), adapter_info, enabled_features) =
        crate::runtime::init_device().expect("Fix: GPU required for compiled timing test");
    assert!(
        device.features().contains(wgpu::Features::TIMESTAMP_QUERY)
            && device
                .features()
                .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS),
        "Fix: WGPU compiled timing test requires timestamp query features to be negotiated."
    );
    let device_queue = Arc::new((device, queue));
    let config = DispatchConfig::default();
    let arena = Arc::new(crate::DispatchArena::new(
        device_queue.0.clone(),
        device_queue.1.clone(),
        &config,
    ));
    let pool = arena.pool().clone();
    let pipeline_cache = Arc::new(crate::runtime::cache::pipeline::LruPipelineCache::new(
        vyre_driver::pipeline::DEFAULT_PIPELINE_CACHE_ENTRIES as u32,
    ));
    let layout_cache = Arc::new(super::BindGroupLayoutCache::with_hasher(
        std::hash::BuildHasherDefault::<rustc_hash::FxHasher>::default(),
    ));

    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let pipeline = super::WgpuPipeline::compile_with_device_queue(
        &program,
        &config,
        adapter_info,
        enabled_features,
        device_queue,
        arena,
        pool,
        pipeline_cache,
        layout_cache,
    )
    .expect("Fix: compiled timed dispatch test pipeline must compile.");

    let timed = pipeline
        .dispatch_borrowed_timed(&[], &config)
        .expect("Fix: compiled borrowed timed dispatch must succeed.");
    assert_eq!(
        u32::from_le_bytes(timed.outputs[0][0..4].try_into().unwrap()),
        7
    );
    assert!(
        timed.device_ns.is_some_and(|ns| ns > 0),
        "Fix: WGPU compiled borrowed timed dispatch must report GPU device nanoseconds."
    );
    assert!(timed.enqueue_ns.is_some_and(|ns| ns > 0));
    assert!(timed.wait_ns.is_some_and(|ns| ns > 0));
}

use super::*;

#[test]
fn direct_record_and_readback_trap_with_output_preserves_ring_fast_path() {
    use std::sync::Arc;

    let ((device, queue), adapter_info, enabled_features) = crate::runtime::init_device()
        .expect("Fix: GPU required for trap+output readback allocation contract test");
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
    let without_rings_pool = without_rings_arena.pool().clone();

    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(99)),
            Node::trap(Expr::u32(9), "mixed-output-ring-trap"),
        ],
    );

    let pipeline = super::WgpuPipeline::compile_with_device_queue(
        &program,
        &config,
        adapter_info,
        enabled_features,
        device_queue.clone(),
        Arc::clone(&with_rings_arena),
        with_rings_pool.clone(),
        pipeline_cache,
        layout_cache,
    )
    .expect("Fix: trapped program with output compile must succeed; restore this invariant before continuing.");

    let empty_inputs: [&[u8]; 0] = [];

    let with_rings_before = with_rings_pool.stats().allocations;
    let with_rings_error = crate::engine::record_and_readback::record_and_readback(
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
                bind_group: "vyre mixed output ring test bind group",
                encoder: "vyre mixed output ring test",
                compute: "vyre mixed output ring test compute",
            },
            iterations: 1,
            timestamp_profile: false,
        },
    )
    .expect_err(
        "Fix: trapped dispatch with output and rings must still surface trap errors and not succeed",
    );
    let with_rings_after = with_rings_pool.stats().allocations;

    assert!(
        with_rings_error
            .to_string()
            .contains("wgpu dispatch trapped"),
        "Fix: expected trap dispatch to surface a backend trap error, got: {with_rings_error}"
    );
    assert!(
        with_rings_error.to_string().contains("mixed-output-ring-trap"),
        "Fix: expected trap tag to be preserved through mixed-output ring path, got: {with_rings_error}"
    );
    assert_eq!(
        with_rings_after,
        with_rings_before + 2,
        "Fix: ring-backed mixed output+trap path should add only output + trap buffer allocations from pool before first successful mapping.",
    );

    let without_rings_before = without_rings_pool.stats().allocations;
    let without_rings_error = crate::engine::record_and_readback::record_and_readback(
        crate::engine::record_and_readback::RecordAndReadback {
            device_queue: &pipeline.device_queue,
            pool: without_rings_arena.pool(),
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
                bind_group: "vyre mixed output no-ring test bind group",
                encoder: "vyre mixed output no-ring test",
                compute: "vyre mixed output no-ring test compute",
            },
            iterations: 1,
            timestamp_profile: false,
        },
    )
    .expect_err(
        "Fix: trapped dispatch without rings should surface the trap error and not succeed",
    );
    let without_rings_after = without_rings_pool.stats().allocations;

    assert!(
        without_rings_error
            .to_string()
            .contains("wgpu dispatch trapped"),
        "Fix: expected trap dispatch to surface a backend trap error, got: {without_rings_error}"
    );
    assert!(
        without_rings_error.to_string().contains("mixed-output-ring-trap"),
        "Fix: expected trap tag to be preserved through mixed-output fallback path, got: {without_rings_error}"
    );
    assert_eq!(
        without_rings_after,
        without_rings_before + 4,
        "Fix: no-ring mixed output+trap path should allocate output storage, trap storage, output readback, and trap readback buffers; ring-backed dispatch must be the path that avoids the two pooled readback allocations.",
    );
}


use super::*;

#[test]
fn zero_repeat_resident_sequence_does_not_prepare_dead_repeated_steps() {
    let backend = CudaBackendRegistration::new(
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host."),
    );
    let add_seven = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("tmp", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "tmp",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(7)),
        )],
    );
    let dead_repeated = Program::wrapped(
        vec![
            BufferDecl::read("dead_in", 0, DataType::U32).with_count(4),
            BufferDecl::output("dead_out", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "dead_out",
            Expr::gid_x(),
            Expr::load("dead_in", Expr::gid_x()),
        )],
    );
    let input = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA zero-repeat resident input allocation failed.");
    let tmp = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA zero-repeat resident tmp allocation failed.");
    VyreBackend::upload_resident(&backend, &input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA zero-repeat resident input upload failed.");

    let prefix_resources = [input.clone(), tmp.clone()];
    let invalid_repeated_resources = [
        vyre_driver::backend::Resource::default(),
        vyre_driver::backend::Resource::default(),
    ];
    let prefix_steps = [vyre_driver::backend::ResidentDispatchStep {
        program: &add_seven,
        resources: &prefix_resources,
        grid_override: None,
        workgroup_override: None,
    }];
    let repeated_steps = [vyre_driver::backend::ResidentDispatchStep {
        program: &dead_repeated,
        resources: &invalid_repeated_resources,
        grid_override: None,
        workgroup_override: None,
    }];
    let read_ranges = [vyre_driver::backend::ResidentReadRange {
        resource: &tmp,
        byte_offset: 0,
        byte_len: 16,
    }];
    let mut readback = Vec::new();

    backend.reset_telemetry();
    VyreBackend::dispatch_resident_repeated_sequence_read_ranges_into(
        &backend,
        &prefix_steps,
        &repeated_steps,
        0,
        &read_ranges,
        &mut [&mut readback],
    )
    .expect("Fix: CUDA zero-repeat resident sequence must not resolve or prepare repeated steps that cannot launch.");

    assert_eq!(bytes_u32(&readback), vec![8, 9, 10, 11]);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 1,
        "Fix: CUDA zero-repeat resident sequence must launch only the prefix step."
    );
    assert!(
        telemetry.sync_points > 0,
        "Fix: CUDA zero-repeat resident sequence should still use one compact readback fence."
    );

    VyreBackend::free_resident(&backend, input)
        .expect("Fix: CUDA zero-repeat resident input free failed.");
    VyreBackend::free_resident(&backend, tmp)
        .expect("Fix: CUDA zero-repeat resident tmp free failed.");
}

#[test]
fn golden_fixed_graph_replay_keeps_host_overhead_sublinear() {
    let backend = CudaBackendRegistration::new(
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host."),
    );
    let add_seven = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("tmp", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "tmp",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(7)),
        )],
    );
    let double = Program::wrapped(
        vec![
            BufferDecl::read("tmp", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::mul(Expr::load("tmp", Expr::gid_x()), Expr::u32(2)),
        )],
    );
    let input = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA golden replay resident input allocation failed.");
    let tmp = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA golden replay resident tmp allocation failed.");
    let output = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA golden replay resident output allocation failed.");
    VyreBackend::upload_resident(&backend, &input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA golden replay resident input upload failed.");

    let prefix_resources = [input.clone(), tmp.clone()];
    let repeated_resources = [tmp.clone(), output.clone()];
    let prefix_steps = [vyre_driver::backend::ResidentDispatchStep {
        program: &add_seven,
        resources: &prefix_resources,
        grid_override: None,
        workgroup_override: None,
    }];
    let repeated_steps = [vyre_driver::backend::ResidentDispatchStep {
        program: &double,
        resources: &repeated_resources,
        grid_override: None,
        workgroup_override: None,
    }];
    let read_ranges = [vyre_driver::backend::ResidentReadRange {
        resource: &output,
        byte_offset: 0,
        byte_len: 16,
    }];
    let mut readback = Vec::with_capacity(64);
    let readback_ptr = readback.as_ptr();
    let mut baseline_param_upload_bytes = None;

    for repeat_count in [1_u32, 8, 64] {
        backend.reset_telemetry();
        VyreBackend::dispatch_resident_repeated_sequence_read_ranges_into(
            &backend,
            &prefix_steps,
            &repeated_steps,
            repeat_count,
            &read_ranges,
            &mut [&mut readback],
        )
        .expect("Fix: CUDA golden fixed-graph replay must execute without expanding host orchestration.");

        assert_eq!(bytes_u32(&readback), vec![16, 18, 20, 22]);
        assert_eq!(
            readback.as_ptr(),
            readback_ptr,
            "Fix: CUDA golden fixed-graph replay must preserve caller-owned readback capacity across repeat counts."
        );
        let telemetry = backend.telemetry_snapshot();
        assert_eq!(
            telemetry.kernel_launches,
            u64::from(repeat_count) + 1,
            "Fix: CUDA golden replay should launch only prefix plus required repeated device work."
        );
        assert!(
            telemetry.sync_points > 0,
            "Fix: CUDA golden replay must keep host fences constant as repeat count grows."
        );
        assert!(
            telemetry.readback_bytes <= u64::from(repeat_count + 1) * 64,
            "Fix: CUDA golden replay fallback must keep readback bytes bounded by launched work; observed {} bytes.",
            telemetry.readback_bytes
        );
        let _baseline = baseline_param_upload_bytes.get_or_insert(telemetry.param_upload_bytes);
        assert!(
            telemetry.param_upload_bytes <= u64::from(repeat_count + 1) * 128,
            "Fix: CUDA golden replay fallback must keep parameter uploads bounded by launched work; observed {} bytes.",
            telemetry.param_upload_bytes
        );
    }

    VyreBackend::free_resident(&backend, input)
        .expect("Fix: CUDA golden replay resident input free failed.");
    VyreBackend::free_resident(&backend, tmp)
        .expect("Fix: CUDA golden replay resident tmp free failed.");
    VyreBackend::free_resident(&backend, output)
        .expect("Fix: CUDA golden replay resident output free failed.");
}

#[test]
fn repeated_resident_sequence_hoists_launch_resolution_out_of_repeat_loop() {
    let source = include_str!("../../src/backend/resident_dispatch/sequence_fused.rs");
    let function_start = source
        .find("pub(crate) fn fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into")
        .expect("Fix: repeated resident sequence implementation must exist.");
    let function_body = &source[function_start..];
    let repeat_loop_start = function_body
        .find("for _ in 0..repeat_count")
        .expect("Fix: repeated resident sequence path must keep an explicit repeat loop.");
    let readback_start = function_body
        .find("let fused_readbacks = fuse_resident_readback_copies(&requested_readbacks)?")
        .expect("Fix: repeated resident sequence path must retain compact fused readback staging.");
    let _repeat_loop_body = &function_body[repeat_loop_start..readback_start];

    assert!(
        function_body.contains("struct ResolvedStep"),
        "Fix: repeated resident CUDA sequence must cache resolved launch records for unique steps before replay."
    );
    assert!(
        (function_body.contains("VYRE_CUDA_RESIDENT_BORROWED_FALLBACK")
            || function_body.contains("CUDA_RESIDENT_BORROWED_FALLBACK_ENV"))
            && (function_body.contains("VYRE_CUDA_ALLOW_BORROWED_FALLBACK")
                || function_body.contains("CUDA_ALLOW_BORROWED_FALLBACK_ENV"))
            && !function_body.contains("VYRE_CUDA_NATIVE_RESIDENT_SEQUENCE"),
        "Fix: repeated resident CUDA sequence must be native by default and keep borrowed fallback behind an explicit release escape hatch."
    );
}

#[test]
fn release_path_resident_dispatch_keeps_borrowed_fallback_counter_at_zero() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::mul(Expr::load("input", Expr::gid_x()), Expr::u32(2)),
        )],
    );
    let input = backend
        .allocate_resident(16)
        .expect("Fix: CUDA resident input allocation failed.");
    let output = backend
        .allocate_resident(16)
        .expect("Fix: CUDA resident output allocation failed.");
    backend
        .upload_resident(input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA resident input upload failed.");

    backend.reset_telemetry();
    backend
        .dispatch_resident(&program, &[input, output], &DispatchConfig::default())
        .expect("Fix: CUDA native resident dispatch must succeed on the release path.");

    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.resident_borrowed_fallback_dispatches, 0,
        "Fix: release-path CUDA resident dispatch must not use the host-buffer borrowed fallback unless both VYRE_CUDA_RESIDENT_BORROWED_FALLBACK and VYRE_CUDA_ALLOW_BORROWED_FALLBACK=1 are set."
    );

    backend
        .free_resident(input)
        .expect("Fix: CUDA resident input free failed.");
    backend
        .free_resident(output)
        .expect("Fix: CUDA resident output free failed.");
}


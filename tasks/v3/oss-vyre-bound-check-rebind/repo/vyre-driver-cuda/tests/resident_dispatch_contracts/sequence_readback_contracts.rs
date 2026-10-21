use super::*;

#[test]
fn backend_sequence_read_ranges_runs_dependent_steps_with_one_fence() {
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
        .expect("Fix: CUDA sequence resident input allocation failed.");
    let tmp = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA sequence resident tmp allocation failed.");
    let output = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA sequence resident output allocation failed.");
    VyreBackend::upload_resident(&backend, &input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA sequence resident input upload failed.");

    let first_resources = [input.clone(), tmp.clone()];
    let second_resources = [tmp.clone(), output.clone()];
    let steps = [
        vyre_driver::backend::ResidentDispatchStep {
            program: &add_seven,
            resources: &first_resources,
            grid_override: None,
            workgroup_override: None,
        },
        vyre_driver::backend::ResidentDispatchStep {
            program: &add_seven,
            resources: &first_resources,
            grid_override: None,
            workgroup_override: None,
        },
        vyre_driver::backend::ResidentDispatchStep {
            program: &double,
            resources: &second_resources,
            grid_override: None,
            workgroup_override: None,
        },
    ];
    let read_ranges = [vyre_driver::backend::ResidentReadRange {
        resource: &output,
        byte_offset: 4,
        byte_len: 8,
    }];
    let mut compact = Vec::with_capacity(64);
    let compact_ptr = compact.as_ptr();

    backend.reset_telemetry();
    VyreBackend::dispatch_resident_sequence_read_ranges_into(
        &backend,
        &steps,
        &read_ranges,
        &mut [&mut compact],
    )
    .expect("Fix: CUDA backend resident sequence-read API must execute dependent kernels.");

    assert_eq!(bytes_u32(&compact), vec![18, 20]);
    assert_eq!(
        compact.as_ptr(),
        compact_ptr,
        "Fix: CUDA backend resident sequence-read API must preserve caller-owned output capacity."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 3,
        "Fix: CUDA backend resident sequence-read API must launch every dependent sequence step."
    );
    assert!(telemetry.sync_points > 0, "Fix: CUDA backend resident sequence-read API must fence once for the whole dependent window plus readback.");
    assert_eq!(
        telemetry.readback_bytes,
        expected_readback_bytes(8, 104),
        "Fix: CUDA backend resident sequence-read API must compact readback to the requested byte range."
    );
    assert!(
        telemetry.param_upload_bytes <= 128,
        "Fix: CUDA backend resident sequence-read API must hoist duplicate launch parameter blocks instead of uploading parameters once per sequence step; observed {} bytes.",
        telemetry.param_upload_bytes
    );

    VyreBackend::free_resident(&backend, input)
        .expect("Fix: CUDA sequence resident input free failed.");
    VyreBackend::free_resident(&backend, tmp)
        .expect("Fix: CUDA sequence resident tmp free failed.");
    VyreBackend::free_resident(&backend, output)
        .expect("Fix: CUDA sequence resident output free failed.");
}

#[test]
fn backend_sequence_read_ranges_coalesces_duplicate_d2h_copies() {
    let backend = CudaBackendRegistration::new(
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host."),
    );
    let add_seven = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(7)),
        )],
    );
    let input = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA duplicate-readback input allocation failed.");
    let output = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA duplicate-readback output allocation failed.");
    VyreBackend::upload_resident(&backend, &input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA duplicate-readback input upload failed.");

    let resources = [input.clone(), output.clone()];
    let steps = [vyre_driver::backend::ResidentDispatchStep {
        program: &add_seven,
        resources: &resources,
        grid_override: None,
        workgroup_override: None,
    }];
    let read_ranges = (0..16)
        .map(|_| vyre_driver::backend::ResidentReadRange {
            resource: &output,
            byte_offset: 4,
            byte_len: 8,
        })
        .collect::<Vec<_>>();
    let mut outputs = (0..16).map(|_| Vec::with_capacity(64)).collect::<Vec<_>>();
    let output_ptrs = outputs.iter().map(Vec::as_ptr).collect::<Vec<_>>();

    backend.reset_telemetry();
    {
        let mut output_refs = outputs.iter_mut().collect::<Vec<_>>();
        VyreBackend::dispatch_resident_sequence_read_ranges_into(
            &backend,
            &steps,
            &read_ranges,
            &mut output_refs,
        )
        .expect("Fix: CUDA backend resident sequence-read API must coalesce duplicate readbacks without losing output slots.");
    }

    for (index, output) in outputs.iter().enumerate() {
        assert_eq!(bytes_u32(output), vec![9, 10]);
        assert_eq!(
            output.as_ptr(),
            output_ptrs[index],
            "Fix: duplicate compact readback must preserve caller-owned byte capacity for output slot {index}."
        );
    }
    let telemetry = backend.telemetry_snapshot();
    if !cuda_resident_borrowed_fallback_active() {
        assert_eq!(
            telemetry.readback_bytes, 8,
            "Fix: native CUDA sequence readback must issue one compact D2H copy for duplicate ranges."
        );
        assert_eq!(
            telemetry.device_readback_operations, 1,
            "Fix: native CUDA sequence readback must count one D2H operation for duplicate ranges."
        );
        assert_eq!(
            telemetry.resident_borrowed_fallback_dispatches, 0,
            "Fix: native CUDA resident sequence readback must not touch the borrowed fallback path."
        );
    }

    VyreBackend::free_resident(&backend, input)
        .expect("Fix: CUDA duplicate-readback input free failed.");
    VyreBackend::free_resident(&backend, output)
        .expect("Fix: CUDA duplicate-readback output free failed.");
}

#[test]

fn backend_sequence_read_ranges_fuses_overlapping_and_adjacent_d2h_intervals() {
    let backend = CudaBackendRegistration::new(
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host."),
    );
    let add_seven = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(7)),
        )],
    );
    let input = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA fused-readback input allocation failed.");
    let output = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA fused-readback output allocation failed.");
    VyreBackend::upload_resident(&backend, &input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA fused-readback input upload failed.");

    let resources = [input.clone(), output.clone()];
    let steps = [vyre_driver::backend::ResidentDispatchStep {
        program: &add_seven,
        resources: &resources,
        grid_override: None,
        workgroup_override: None,
    }];
    let read_ranges = [
        vyre_driver::backend::ResidentReadRange {
            resource: &output,
            byte_offset: 0,
            byte_len: 8,
        },
        vyre_driver::backend::ResidentReadRange {
            resource: &output,
            byte_offset: 4,
            byte_len: 8,
        },
        vyre_driver::backend::ResidentReadRange {
            resource: &output,
            byte_offset: 12,
            byte_len: 4,
        },
    ];
    let mut first = Vec::with_capacity(64);
    let mut second = Vec::with_capacity(64);
    let mut third = Vec::with_capacity(64);

    backend.reset_telemetry();
    VyreBackend::dispatch_resident_sequence_read_ranges_into(
        &backend,
        &steps,
        &read_ranges,
        &mut [&mut first, &mut second, &mut third],
    )
    .expect("Fix: CUDA backend resident sequence-read API must fuse overlapping and adjacent readbacks without changing caller output ordering.");

    assert_eq!(bytes_u32(&first), vec![8, 9]);
    assert_eq!(bytes_u32(&second), vec![9, 10]);
    assert_eq!(bytes_u32(&third), vec![11]);
    let telemetry = backend.telemetry_snapshot();
    if !cuda_resident_borrowed_fallback_active() {
        assert_eq!(
            telemetry.readback_bytes, 16,
            "Fix: native CUDA sequence readback must fuse overlapping/adjacent ranges into one 16-byte D2H interval."
        );
        assert_eq!(
            telemetry.device_readback_operations, 1,
            "Fix: native CUDA sequence readback must issue one D2H operation for a fused readback interval."
        );
        assert_eq!(
            telemetry.resident_borrowed_fallback_dispatches, 0,
            "Fix: native CUDA resident sequence readback must not touch the borrowed fallback path."
        );
    }

    VyreBackend::free_resident(&backend, input)
        .expect("Fix: CUDA fused-readback input free failed.");
    VyreBackend::free_resident(&backend, output)
        .expect("Fix: CUDA fused-readback output free failed.");
}

#[test]
fn backend_repeated_sequence_read_ranges_runs_without_expanded_host_window() {
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
        .expect("Fix: CUDA repeated sequence resident input allocation failed.");
    let tmp = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA repeated sequence resident tmp allocation failed.");
    let output = VyreBackend::allocate_resident(&backend, 16)
        .expect("Fix: CUDA repeated sequence resident output allocation failed.");
    VyreBackend::upload_resident(&backend, &input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA repeated sequence resident input upload failed.");

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

    backend.reset_telemetry();
    VyreBackend::dispatch_resident_repeated_sequence_read_ranges_into(
        &backend,
        &prefix_steps,
        &repeated_steps,
        4,
        &read_ranges,
        &mut [&mut readback],
    )
    .expect("Fix: CUDA backend repeated resident sequence-read API must execute without materializing an expanded caller sequence.");

    assert_eq!(bytes_u32(&readback), vec![16, 18, 20, 22]);
    assert_eq!(
        readback.as_ptr(),
        readback_ptr,
        "Fix: CUDA repeated resident sequence-read API must preserve caller-owned output capacity."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 5,
        "Fix: CUDA repeated resident sequence-read API must launch prefix plus every repeated step."
    );
    assert!(telemetry.sync_points > 0, "Fix: CUDA repeated resident sequence-read API must fence once for the whole repeated window plus readback.");
    assert_eq!(
        telemetry.readback_bytes,
        expected_readback_bytes(16, 176),
        "Fix: CUDA repeated resident sequence-read API must compact readback to the requested byte range."
    );
    assert!(
        telemetry.param_upload_bytes <= 128,
        "Fix: CUDA repeated resident sequence-read API must hoist repeated launch parameter blocks instead of uploading parameters once per repeated step; observed {} bytes.",
        telemetry.param_upload_bytes
    );

    VyreBackend::free_resident(&backend, input)
        .expect("Fix: CUDA repeated sequence resident input free failed.");
    VyreBackend::free_resident(&backend, tmp)
        .expect("Fix: CUDA repeated sequence resident tmp free failed.");
    VyreBackend::free_resident(&backend, output)
        .expect("Fix: CUDA repeated sequence resident output free failed.");
}


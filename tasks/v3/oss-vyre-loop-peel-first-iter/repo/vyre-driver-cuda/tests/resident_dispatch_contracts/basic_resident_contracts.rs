use super::*;

#[test]
fn resident_dispatch_runs_without_host_buffer_arguments() {
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
            Expr::mul(Expr::load("input", Expr::gid_x()), Expr::u32(3)),
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

    backend
        .dispatch_resident(&program, &[input, output], &DispatchConfig::default())
        .expect("Fix: CUDA resident dispatch must execute the scalar trainer-safe subset.");

    let output_bytes = backend
        .download_resident(output)
        .expect("Fix: CUDA resident output download failed.");
    assert_eq!(bytes_u32(&output_bytes), vec![3, 6, 9, 12]);

    backend
        .free_resident(input)
        .expect("Fix: CUDA resident input free failed.");
    backend
        .free_resident(output)
        .expect("Fix: CUDA resident output free failed.");
}

#[test]
fn resident_dispatch_preserves_plain_read_write_state() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![BufferDecl::storage("state", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::store(
            "state",
            Expr::gid_x(),
            Expr::add(Expr::load("state", Expr::gid_x()), Expr::u32(7)),
        )],
    );

    let state = backend
        .allocate_resident(16)
        .expect("Fix: CUDA resident state allocation failed.");
    backend
        .upload_resident(state, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA resident state upload failed.");

    backend
        .dispatch_resident(&program, &[state], &DispatchConfig::default())
        .expect("Fix: CUDA resident dispatch must update plain read-write state in place.");

    let output_bytes = backend
        .download_resident(state)
        .expect("Fix: CUDA resident state download failed.");
    assert_eq!(bytes_u32(&output_bytes), vec![8, 9, 10, 11]);

    backend
        .free_resident(state)
        .expect("Fix: CUDA resident state free failed.");
}

#[test]
fn async_resident_dispatch_holds_handles_until_awaited() {
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
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(5)),
        )],
    );

    let input = backend
        .allocate_resident(16)
        .expect("Fix: CUDA resident input allocation failed.");
    let output = backend
        .allocate_resident(16)
        .expect("Fix: CUDA resident output allocation failed.");
    backend
        .upload_resident(input, &u32_bytes(&[10, 20, 30, 40]))
        .expect("Fix: CUDA resident input upload failed.");

    let pending = backend
        .dispatch_resident_async(&program, &[input, output], &DispatchConfig::default())
        .expect("Fix: CUDA resident async dispatch must enqueue without host buffer arguments.");
    pending
        .await_result()
        .expect("Fix: CUDA resident async dispatch must complete successfully.");

    let output_bytes = backend
        .download_resident(output)
        .expect("Fix: CUDA resident output download failed.");
    assert_eq!(bytes_u32(&output_bytes), vec![15, 25, 35, 45]);

    backend
        .free_resident(input)
        .expect("Fix: CUDA resident input free failed after await.");
    backend
        .free_resident(output)
        .expect("Fix: CUDA resident output free failed after await.");
}

#[test]
fn timed_resident_dispatch_reports_device_time_and_outputs() {
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
        .upload_resident(input, &u32_bytes(&[2, 4, 6, 8]))
        .expect("Fix: CUDA resident input upload failed.");

    let timed = backend
        .dispatch_resident_timed(&program, &[input, output], &DispatchConfig::default())
        .expect("Fix: timed CUDA resident dispatch must complete successfully.");
    assert_eq!(bytes_u32(&timed.outputs[0]), vec![4, 8, 12, 16]);
    assert!(
        timed.wall_ns > 0,
        "Fix: CUDA resident timing fallback must return wall-clock timing."
    );

    backend
        .free_resident(input)
        .expect("Fix: CUDA resident input free failed after timed dispatch.");
    backend
        .free_resident(output)
        .expect("Fix: CUDA resident output free failed after timed dispatch.");
}

#[test]
fn compiled_resident_dispatch_into_reuses_output_slot() {
    let backend = Arc::new(CudaBackendRegistration::new(
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host."),
    ));
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(11)),
        )],
    );
    let config = DispatchConfig::default();
    let pipeline = vyre_driver::pipeline::compile(backend.clone(), &program, &config)
        .expect("Fix: CUDA compiled pipeline creation failed for resident dispatch.");
    let input = VyreBackend::allocate_resident(backend.as_ref(), 16)
        .expect("Fix: CUDA trait resident input allocation failed.");
    let output = VyreBackend::allocate_resident(backend.as_ref(), 16)
        .expect("Fix: CUDA trait resident output allocation failed.");
    VyreBackend::upload_resident(backend.as_ref(), &input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA trait resident input upload failed.");

    let mut outputs = vec![Vec::with_capacity(64)];
    let outer_ptr = outputs.as_ptr();
    let first_slot_ptr = outputs[0].as_ptr();

    backend.reset_telemetry();
    pipeline
        .dispatch_persistent_handles_into(&[input.clone(), output.clone()], &config, &mut outputs)
        .expect("Fix: CUDA compiled resident dispatch must support caller-owned output slots.");

    assert_eq!(bytes_u32(&outputs[0]), vec![12, 13, 14, 15]);
    assert_eq!(outputs.as_ptr(), outer_ptr);
    assert_eq!(outputs[0].as_ptr(), first_slot_ptr);
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.param_upload_bytes, 0,
        "Fix: same-shape compiled resident dispatch must reuse static CUDA launch params instead of re-uploading params through the borrowed fallback."
    );
    assert_eq!(
        telemetry.readback_bytes, 16,
        "Fix: same-shape compiled resident dispatch must read back only requested output bytes, not resident inputs."
    );

    VyreBackend::free_resident(backend.as_ref(), input)
        .expect("Fix: CUDA trait resident input free failed.");
    VyreBackend::free_resident(backend.as_ref(), output)
        .expect("Fix: CUDA trait resident output free failed.");
}

#[test]
fn compiled_resident_dispatch_skips_zero_length_output_writeback() {
    let backend = Arc::new(CudaBackendRegistration::new(
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host."),
    ));
    let program = Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::U32)
            .with_count(4)
            .with_output_byte_range(0..0)],
        [1, 1, 1],
        vec![Node::store("state", Expr::gid_x(), Expr::u32(99))],
    );
    let config = DispatchConfig::default();
    let pipeline = vyre_driver::pipeline::compile(backend.clone(), &program, &config)
        .expect("Fix: CUDA compiled pipeline creation failed for zero-readback resident dispatch.");
    let state = VyreBackend::allocate_resident(backend.as_ref(), 16)
        .expect("Fix: CUDA trait resident state allocation failed.");
    VyreBackend::upload_resident(backend.as_ref(), &state, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA trait resident state upload failed.");

    let mut outputs = Vec::new();
    pipeline
        .dispatch_persistent_handles_into(&[state.clone()], &config, &mut outputs)
        .expect(
            "Fix: CUDA compiled resident dispatch must skip writeback for output_byte_range=0..0.",
        );

    assert_eq!(outputs, vec![Vec::<u8>::new()]);
    VyreBackend::free_resident(backend.as_ref(), state)
        .expect("Fix: CUDA trait resident state free failed.");
}

#[test]
fn compiled_resource_output_dispatch_reuses_static_launch_params() {
    let backend = Arc::new(CudaBackendRegistration::new(
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host."),
    ));
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(17)),
        )],
    );
    let config = DispatchConfig::default();
    let pipeline = vyre_driver::pipeline::compile(backend.clone(), &program, &config)
        .expect("Fix: CUDA compiled pipeline creation failed for resident resource outputs.");
    let input = VyreBackend::allocate_resident(backend.as_ref(), 16)
        .expect("Fix: CUDA trait resident input allocation failed.");
    let output = VyreBackend::allocate_resident(backend.as_ref(), 16)
        .expect("Fix: CUDA trait resident output allocation failed.");
    VyreBackend::upload_resident(backend.as_ref(), &input, &u32_bytes(&[1, 2, 3, 4]))
        .expect("Fix: CUDA trait resident input upload failed.");

    backend.reset_telemetry();
    let resources = pipeline
        .dispatch_persistent_resource_outputs(&[input.clone(), output.clone()], &config)
        .expect("Fix: CUDA compiled persistent resource-output dispatch must stay resident.");

    assert_eq!(
        resources.len(),
        1,
        "Fix: CUDA compiled persistent resource-output dispatch must return resident output resources only."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.param_upload_bytes, 0,
        "Fix: compiled persistent resource-output dispatch must reuse static CUDA launch params instead of re-uploading params through a fallback."
    );
    assert_eq!(
        telemetry.readback_bytes, 0,
        "Fix: resource-output dispatch must stay resident and avoid host readback before the caller asks for bytes."
    );
    let output_bytes = VyreBackend::download_resident(backend.as_ref(), &output)
        .expect("Fix: CUDA compiled resource-output dispatch must leave computed bytes resident.");
    assert_eq!(bytes_u32(&output_bytes), vec![18, 19, 20, 21]);

    VyreBackend::free_resident(backend.as_ref(), input)
        .expect("Fix: CUDA trait resident input free failed.");
    VyreBackend::free_resident(backend.as_ref(), output)
        .expect("Fix: CUDA trait resident output free failed.");
}


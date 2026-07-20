use super::*;

#[test]
fn cuda_large_storage_atomic_sum_crosses_workgroup_boundary() {
    let count = 4096u32;
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("sum", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::read("values", 1, DataType::U32).with_count(count),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(count)),
                vec![Node::let_bind(
                    "old_sum",
                    Expr::atomic_add("sum", Expr::u32(0), Expr::load("values", Expr::var("idx"))),
                )],
            ),
        ],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[u32_bytes(&[0]), u32_bytes(&vec![1; count as usize])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA storage atomic reduction must launch every inferred workgroup.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![count],
        "Fix: CUDA storage atomics must produce a full multi-workgroup sum, not only the first block."
    );
}

#[test]
fn cuda_compiled_pipeline_matches_direct_dispatch_for_multi_block_atomics() {
    let count = 4096u32;
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("sum", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::read("values", 1, DataType::U32).with_count(count),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(count)),
                vec![Node::let_bind(
                    "old_sum",
                    Expr::atomic_add("sum", Expr::u32(0), Expr::load("values", Expr::var("idx"))),
                )],
            ),
        ],
    );
    let backend: Arc<dyn VyreBackend> = cuda_factory()
        .expect("Fix: CUDA factory must acquire the live GPU.")
        .into();
    let compiled = pipeline::compile(Arc::clone(&backend), &program, &DispatchConfig::default())
        .expect("Fix: CUDA native pipeline compile must accept storage atomic programs.");
    let values = u32_bytes(&vec![1; count as usize]);
    let initial_sum = u32_bytes(&[0]);
    let grid = vyre_driver::program_walks::infer_dispatch_grid(
        &program,
        &[initial_sum.clone(), values.clone()],
        &DispatchConfig::default(),
    )
    .expect("Fix: shared dispatch-grid inference must handle CUDA storage atomic programs.");
    let mut config = DispatchConfig::default();
    config.grid_override = Some(grid);
    let outputs = compiled
        .dispatch_borrowed(&[initial_sum.as_slice(), values.as_slice()], &config)
        .expect("Fix: CUDA compiled pipeline dispatch must match direct backend dispatch.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![count],
        "Fix: CUDA compiled pipeline must honor caller launch config across all workgroups."
    );
}

#[test]
fn cuda_subgroup_add_reports_full_warp_sum() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(32)],
        [32, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::subgroup_add(Expr::u32(1)),
        )],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect("Fix: CUDA subgroup add must execute through warp-sync PTX.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![32; 32],
        "Fix: CUDA subgroup add must reduce across every lane in the warp."
    );
}

#[test]
fn cuda_shared_memory_round_trips_with_barrier() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(32),
            BufferDecl::workgroup("scratch", 32, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32).with_count(32),
        ],
        [32, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::let_bind("lane", Expr::local_x()),
            Node::store(
                "scratch",
                Expr::var("lane"),
                Expr::load("input", Expr::var("idx")),
            ),
            Node::barrier(),
            Node::store(
                "out",
                Expr::var("idx"),
                Expr::add(Expr::load("scratch", Expr::var("lane")), Expr::u32(7)),
            ),
        ],
    );
    let input = (0..32).collect::<Vec<u32>>();
    let expected = input.iter().map(|value| value + 7).collect::<Vec<_>>();
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(&program, &[u32_bytes(&input)], &DispatchConfig::default())
        .expect("Fix: CUDA shared-memory program must lower and execute.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        expected,
        "Fix: CUDA shared memory must use shared address space and respect workgroup barriers."
    );
}

#[test]
fn cuda_dispatch_rejects_per_axis_block_overflow_before_launch() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let max_block_dim = backend.max_block_dim();
    let program = Program::wrapped(
        vec![BufferDecl::output("dst", 0, DataType::U32).with_count(1)],
        [1, 1, max_block_dim[2].saturating_add(1)],
        vec![Node::store("dst", Expr::u32(0), Expr::u32(1))],
    );

    let err = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect_err("Fix: CUDA must reject per-axis block overflow before cuLaunchKernel.");
    let msg = err.to_string();
    assert!(
        msg.contains("axis 2") && msg.contains("Fix:"),
        "Fix: CUDA block-axis validation must produce an actionable axis-specific error; got: {msg}"
    );
}

#[test]
fn cuda_tanh_matches_reference_softcap_fixture() {
    let i = Expr::var("i");
    let value = Expr::mul(
        Expr::UnOp {
            op: UnOp::Tanh,
            operand: Box::new(Expr::div(Expr::load("input", i.clone()), Expr::f32(30.0))),
        },
        Expr::f32(30.0),
    );
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(4),
            BufferDecl::output("output", 1, DataType::F32).with_count(4),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("i", Expr::gid_x()),
            Node::if_then(
                Expr::lt(i.clone(), Expr::buf_len("input")),
                vec![Node::store("output", i, value)],
            ),
        ],
    );
    let input = [0.0_f32, 15.0, -60.0, 100.0];
    let expected = input
        .iter()
        .map(|x| (x / 30.0).tanh() * 30.0)
        .collect::<Vec<_>>();
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let mut config = DispatchConfig::default();
    config.ulp_budget = Some(128);
    let outputs = backend
        .dispatch(
            &program,
            &[input.iter().flat_map(|x| x.to_le_bytes()).collect()],
            &config,
        )
        .expect("Fix: CUDA tanh dispatch must complete.");

    let got = outputs[0]
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect::<Vec<_>>();
    for (lane, (actual, expected)) in got.iter().zip(expected.iter()).enumerate() {
        let diff = ordered_f32_bits(*actual).abs_diff(ordered_f32_bits(*expected));
        assert!(
            diff <= 128,
            "Fix: CUDA tanh lane {lane} exceeded the 128-ULP native transcendental budget: actual={actual:e} expected={expected:e} diff={diff}."
        );
    }
}

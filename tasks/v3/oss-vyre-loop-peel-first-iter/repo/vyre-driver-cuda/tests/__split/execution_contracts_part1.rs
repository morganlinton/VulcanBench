use super::*;

#[test]
fn cuda_dispatch_writes_every_output_lane_for_identity() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("src", 0, DataType::U32).with_count(4),
            BufferDecl::output("dst", 1, DataType::U32).with_count(4),
        ],
        [64, 1, 1],
        vec![Node::store(
            "dst",
            Expr::gid_x(),
            Expr::load("src", Expr::gid_x()),
        )],
    );

    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[u32_bytes(&[10, 20, 30, 40])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA identity dispatch must complete.");

    assert_eq!(
        outputs,
        vec![u32_bytes(&[10, 20, 30, 40])],
        "Fix: CUDA launch bounds must cover every writable output lane."
    );
}

#[test]
fn cuda_dispatch_conv2d_identity_box_matches_fixture() {
    let program = vyre_libs::math::conv::conv2d_3x3_direct("input", "kernel", "output", 4, 4)
        .expect("Fix: conv2d fixture program must build.");
    let input = f32_bytes(&[
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]);
    let kernel = f32_bytes(&[1.0; 9]);
    let expected = vec![
        2.0, 2.0, 1.0, 0.0,
        2.0, 3.0, 2.0, 1.0,
        1.0, 2.0, 3.0, 2.0,
        0.0, 1.0, 2.0, 2.0,
    ];

    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[input, kernel],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA conv2d dispatch must complete.");
    let actual = bytes_to_f32(&outputs[0]);

    assert_eq!(
        actual, expected,
        "Fix: CUDA conv2d must preserve zero-padded select semantics."
    );
}

#[test]
fn cuda_dispatch_fft_circular_convolution_matches_fixture() {
    let program = vyre_libs::math::fft::fft_convolve_circular_complex(
        "signal",
        "kernel",
        "signal_freq",
        "kernel_freq",
        "product_freq",
        "output",
        4,
    )
    .expect("Fix: FFT convolution fixture program must build.");
    let signal = f32_bytes(&[1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0]);
    let kernel = f32_bytes(&[1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let expected = vec![5.0, 0.0, 3.0, 0.0, 5.0, 0.0, 7.0, 0.0];

    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[signal, kernel],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA FFT convolution dispatch must complete.");
    let actual = bytes_to_f32(&outputs[0]);

    assert_eq!(
        actual, expected,
        "Fix: CUDA FFT circular convolution must match the fixture oracle."
    );
}

#[test]
fn cuda_dispatch_broadcasts_scalar_to_every_output_lane() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("src", 0, DataType::U32).with_count(1),
            BufferDecl::output("dst", 1, DataType::U32).with_count(4),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(4)),
                vec![Node::store(
                    "dst",
                    Expr::var("idx"),
                    Expr::load("src", Expr::u32(0)),
                )],
            ),
        ],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(&program, &[u32_bytes(&[42])], &DispatchConfig::default())
        .expect("Fix: CUDA broadcast dispatch must complete.");

    assert_eq!(
        outputs,
        vec![u32_bytes(&[42, 42, 42, 42])],
        "Fix: CUDA scalar broadcast must execute all in-bounds lanes."
    );
}

#[test]
fn cuda_dispatch_lowers_f16_buffer_load_add_store() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::F16).with_count(2),
            BufferDecl::read("b", 1, DataType::F16).with_count(2),
            BufferDecl::output("out", 2, DataType::F16).with_count(2),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(
                Expr::load("a", Expr::gid_x()),
                Expr::load("b", Expr::gid_x()),
            ),
        )],
    );

    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[u16_bytes(&[0x3c00, 0x4000]), u16_bytes(&[0x4000, 0x4000])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA f16 buffer load/add/store dispatch must complete.");

    assert_eq!(
        outputs,
        vec![u16_bytes(&[0x4200, 0x4400])],
        "Fix: CUDA f16 buffers must use 2-byte addressing and f16<->f32 PTX conversion."
    );
}

#[test]
fn cuda_dispatch_lowers_bf16_buffer_load_add_store() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::BF16).with_count(2),
            BufferDecl::read("b", 1, DataType::BF16).with_count(2),
            BufferDecl::output("out", 2, DataType::BF16).with_count(2),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(
                Expr::load("a", Expr::gid_x()),
                Expr::load("b", Expr::gid_x()),
            ),
        )],
    );

    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[u16_bytes(&[0x3f80, 0x4000]), u16_bytes(&[0x4000, 0x4000])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA bf16 buffer load/add/store dispatch must complete.");

    assert_eq!(
        outputs,
        vec![u16_bytes(&[0x4040, 0x4080])],
        "Fix: CUDA bf16 buffers must use 2-byte addressing and round-to-nearest-even bf16 store conversion."
    );
}

#[test]
fn cuda_unsigned_div_mod_zero_matches_reference_total_contract() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(1),
            BufferDecl::read("b", 1, DataType::U32).with_count(1),
            BufferDecl::output("out", 2, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::div(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            ),
            Node::store(
                "out",
                Expr::u32(1),
                Expr::rem(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            ),
        ],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[u32_bytes(&[123]), u32_bytes(&[0])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA unsigned div/mod zero dispatch must complete deterministically.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![u32::MAX, 0],
        "Fix: CUDA unsigned div/mod by zero must match the reference total semantics."
    );
}

#[test]
fn cuda_signed_div_edge_cases_match_reference_total_contract() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::I32).with_count(2),
            BufferDecl::read("b", 1, DataType::I32).with_count(2),
            BufferDecl::output("out", 2, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::Cast {
                    target: DataType::U32,
                    value: Box::new(Expr::div(
                        Expr::load("a", Expr::u32(0)),
                        Expr::load("b", Expr::u32(0)),
                    )),
                },
            ),
            Node::store(
                "out",
                Expr::u32(1),
                Expr::Cast {
                    target: DataType::U32,
                    value: Box::new(Expr::div(
                        Expr::load("a", Expr::u32(1)),
                        Expr::load("b", Expr::u32(1)),
                    )),
                },
            ),
        ],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[i32_bytes(&[7, i32::MIN]), i32_bytes(&[0, -1])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA signed div/mod edge-case dispatch must complete deterministically.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![0, i32::MIN as u32],
        "Fix: CUDA signed div edge cases must match the reference total semantics at the bit level."
    );
}

#[test]
fn cuda_signed_mod_round_trips_after_foundation_started_allowing_it() {
    // Original contract said "signed Mod must be rejected before
    // CUDA lowering while foundation rejects it." Foundation now
    // accepts signed Mod, so the test inverts: dispatch must succeed
    // and produce a defined output (we use a non-zero divisor to
    // avoid divide-by-zero UB and pin the expected result).
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::I32).with_count(1),
            BufferDecl::read("b", 1, DataType::I32).with_count(1),
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::rem(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
        )],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[i32_bytes(&[7]), i32_bytes(&[3])],
            &DispatchConfig::default(),
        )
        .expect(
            "Fix: signed modulo must dispatch cleanly now that foundation allows signed Mod.",
        );
    // 7 % 3 == 1 (signed mod, host and device agree on positive operands).
    assert_eq!(
        outputs,
        vec![vec![1u8, 0, 0, 0]],
        "Fix: signed modulo of 7 % 3 must equal 1 (little-endian u32 store of an i32 result)."
    );
}

#[test]
fn cuda_grid_override_drives_logical_lane_count_for_output_small_kernels() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("sum", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_output_byte_range(0..4),
            BufferDecl::read("values", 1, DataType::U32).with_count(256),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(256)),
                vec![Node::let_bind(
                    "old_sum",
                    Expr::atomic_add("sum", Expr::u32(0), Expr::load("values", Expr::var("idx"))),
                )],
            ),
        ],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = backend
        .dispatch(&program, &[u32_bytes(&[0]), u32_bytes(&[1; 256])], &config)
        .expect("Fix: CUDA grid_override dispatch must complete for output-small kernels.");

    assert_eq!(
        outputs,
        vec![u32_bytes(&[256])],
        "Fix: CUDA grid_override must update launch metadata so every logical lane executes."
    );
}

#[test]
fn cuda_honors_zero_length_output_byte_range() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("state", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(4)
                .with_output_byte_range(0..0),
        ],
        [1, 1, 1],
        vec![Node::store("state", Expr::u32(0), Expr::u32(7))],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(&program, &[u32_bytes(&[0, 0, 0, 0])], &DispatchConfig::default())
        .expect("Fix: CUDA dispatch must allow output_byte_range=0..0 without readback.");

    assert_eq!(
        outputs,
        vec![Vec::<u8>::new()],
        "Fix: CUDA output_byte_range=0..0 must produce an empty output buffer, not a full readback."
    );
}

#[test]
fn cuda_honors_nonzero_output_byte_range_offset() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("state", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(4)
                .with_output_byte_range(4..12),
        ],
        [1, 1, 1],
        vec![Node::store("state", Expr::u32(3), Expr::u32(99))],
    );
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let outputs = backend
        .dispatch(
            &program,
            &[u32_bytes(&[11, 22, 33, 44])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA dispatch must read back the requested byte-range slice.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![22, 33],
        "Fix: CUDA output_byte_range=4..12 must return only the requested middle words."
    );
}


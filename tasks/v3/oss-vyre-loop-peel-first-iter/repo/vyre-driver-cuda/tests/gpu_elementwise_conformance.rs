//! Live CUDA elementwise dispatch conformance.

mod common;
use common::{bytes_u32, u32_bytes};
use vyre_driver::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferDecl, CommGroup, DataType, Expr, Node, Program};

#[test]
fn cuda_runs_u32_add_one_program_end_to_end() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(8),
            BufferDecl::output("out", 1, DataType::U32).with_count(8),
        ],
        [128, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(1)),
        )],
    );

    backend.reset_telemetry();
    let outputs = backend
        .dispatch(
            &program,
            &[u32_bytes(&[0, 1, 2, 3, 9, 10, 99, u32::MAX - 1])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA backend must execute the minimal u32 add-one program end-to-end.");

    assert_eq!(outputs.len(), 1);
    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![1, 2, 3, 4, 10, 11, 100, u32::MAX],
        "Fix: CUDA dispatch output must be byte-exact for u32 add-one."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.param_upload_bytes, 12,
        "Fix: CUDA host dispatch must report the exact non-empty launch parameter bytes."
    );
    assert_eq!(
        telemetry.host_upload_operations, 2,
        "Fix: CUDA host dispatch must count one input upload plus one non-empty parameter upload."
    );
    assert_eq!(
        telemetry.transient_allocation_bytes_requested, 80,
        "Fix: CUDA host dispatch transient allocation telemetry must include input, output, and the rounded non-empty parameter buffer only."
    );
}

#[test]
fn cuda_runs_simple_if_select_program_end_to_end() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::select(
                Expr::gt(Expr::load("input", Expr::gid_x()), Expr::u32(10)),
                Expr::u32(1),
                Expr::u32(0),
            ),
        )],
    );

    let outputs = backend
        .dispatch(
            &program,
            &[u32_bytes(&[1, 10, 11, 99])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA backend must execute select/comparison subset end-to-end.");

    assert_eq!(bytes_u32(&outputs[0]), vec![0, 0, 1, 1]);
}

#[test]
fn cuda_async_dispatch_returns_pending_gpu_work() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(9)),
        )],
    );

    let pending = backend
        .dispatch_async(
            &program,
            &[u32_bytes(&[1, 2, 3, 4])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA async dispatch must enqueue kernel work and return a pending handle.");
    let outputs = pending
        .await_result()
        .expect("Fix: CUDA async pending dispatch must return completed readback bytes.");

    assert_eq!(bytes_u32(&outputs[0]), vec![10, 11, 12, 13]);
}

#[test]
fn cuda_executes_world_allgather_as_single_rank_copy() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(8),
            BufferDecl::output("out", 1, DataType::U32).with_count(8),
        ],
        [64, 1, 1],
        vec![Node::AllGather {
            input: "input".into(),
            output: "out".into(),
            group: CommGroup::WORLD,
        }],
    );

    let outputs = backend
        .dispatch(
            &program,
            &[u32_bytes(&[3, 1, 4, 1, 5, 9, 2, 6])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA must lower WORLD AllGather into a real single-rank device copy.");

    assert_eq!(bytes_u32(&outputs[0]), vec![3, 1, 4, 1, 5, 9, 2, 6]);
}

#[test]
fn cuda_compiled_pipeline_executes_world_reduce_scatter_as_single_rank_copy() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [64, 1, 1],
        vec![Node::ReduceScatter {
            input: "input".into(),
            output: "out".into(),
            op: vyre_foundation::ir::CollectiveOp::Sum,
            group: CommGroup::WORLD,
        }],
    );
    let pipeline = backend
        .compile_native(&program, &DispatchConfig::default())
        .expect("Fix: CUDA native compile must pre-lower WORLD ReduceScatter before PTX emission.");

    let outputs = pipeline
        .dispatch(&[u32_bytes(&[8, 6, 7, 5])], &DispatchConfig::default())
        .expect("Fix: compiled CUDA pipeline must execute single-rank ReduceScatter.");

    assert_eq!(bytes_u32(&outputs[0]), vec![8, 6, 7, 5]);
}

#[test]
fn cuda_executes_world_allreduce_as_single_rank_identity() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(8),
            BufferDecl::output("out", 1, DataType::U32).with_count(8),
        ],
        [8, 1, 1],
        vec![
            Node::store("out", Expr::gid_x(), Expr::load("input", Expr::gid_x())),
            Node::AllReduce {
                buffer: "out".into(),
                op: vyre_foundation::ir::CollectiveOp::Sum,
                group: CommGroup::WORLD,
            },
        ],
    );

    let outputs = backend
        .dispatch(
            &program,
            &[u32_bytes(&[13, 21, 34, 55, 89, 144, 233, 377])],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA must lower WORLD AllReduce into a single-rank identity.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![13, 21, 34, 55, 89, 144, 233, 377]
    );
}

#[test]
fn cuda_compiled_pipeline_executes_world_broadcast_root_zero_as_identity() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        [4, 1, 1],
        vec![
            Node::store("out", Expr::gid_x(), Expr::load("input", Expr::gid_x())),
            Node::Broadcast {
                buffer: "out".into(),
                root: 0,
                group: CommGroup::WORLD,
            },
        ],
    );
    let pipeline = backend
        .compile_native(&program, &DispatchConfig::default())
        .expect(
            "Fix: CUDA native compile must pre-lower WORLD Broadcast root 0 before PTX emission.",
        );

    let outputs = pipeline
        .dispatch(
            &[u32_bytes(&[0, 1, u32::MAX - 1, u32::MAX])],
            &DispatchConfig::default(),
        )
        .expect("Fix: compiled CUDA pipeline must execute single-rank Broadcast root 0.");

    assert_eq!(bytes_u32(&outputs[0]), vec![0, 1, u32::MAX - 1, u32::MAX]);
}

#[test]
fn generated_cuda_world_copy_collectives_cover_boundary_shapes() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    for &count in &[1u32, 7, 8, 63, 64, 65, 127, 128, 255, 256, 257, 1024] {
        let input = (0..count)
            .map(|index| {
                index.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
                    ^ count.rotate_left(index & 15)
            })
            .collect::<Vec<_>>();
        for reduce in [false, true] {
            let node = if reduce {
                Node::ReduceScatter {
                    input: "input".into(),
                    output: "out".into(),
                    op: vyre_foundation::ir::CollectiveOp::Sum,
                    group: CommGroup::WORLD,
                }
            } else {
                Node::AllGather {
                    input: "input".into(),
                    output: "out".into(),
                    group: CommGroup::WORLD,
                }
            };
            let program = Program::wrapped(
                vec![
                    BufferDecl::read("input", 0, DataType::U32).with_count(count),
                    BufferDecl::output("out", 1, DataType::U32).with_count(count),
                ],
                [64, 1, 1],
                vec![node],
            );

            let outputs = backend
                .dispatch(&program, &[u32_bytes(&input)], &DispatchConfig::default())
                .unwrap_or_else(|error| {
                    panic!(
                        "Fix: CUDA WORLD copy collective must dispatch at boundary count={count} reduce={reduce}: {error}"
                    )
                });

            assert_eq!(
                bytes_u32(&outputs[0]),
                input,
                "Fix: CUDA WORLD copy collective mismatch at count={count} reduce={reduce}"
            );
        }
    }
}

#[test]
fn cuda_rejects_nonzero_single_rank_broadcast_root() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [64, 1, 1],
        vec![Node::Broadcast {
            buffer: "out".into(),
            root: 1,
            group: CommGroup::WORLD,
        }],
    );

    let error = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect_err("Fix: CUDA must reject a single-rank broadcast root other than rank 0.");
    assert!(
        error.to_string().contains("root 0"),
        "Fix: CUDA single-rank broadcast rejection must explain the root invariant: {error}"
    );
}

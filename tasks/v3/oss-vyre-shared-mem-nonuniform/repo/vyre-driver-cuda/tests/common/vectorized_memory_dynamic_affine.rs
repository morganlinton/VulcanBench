use crate::common::{assert_u32_output_lanes, cuda_reference_outputs, live_backend, u32_bytes};
use vyre::DispatchConfig;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

const DYNAMIC_AFFINE_GROUP_COUNT: usize = 256;
const DYNAMIC_AFFINE_STRIDE: usize = 5;
const DYNAMIC_AFFINE_SOURCE_LANES: usize = DYNAMIC_AFFINE_GROUP_COUNT * DYNAMIC_AFFINE_STRIDE;
const DYNAMIC_AFFINE_OUTPUT_LANES: usize = DYNAMIC_AFFINE_GROUP_COUNT * 4;
const WORKGROUP_SIZE_X: u32 = 128;

#[test]
fn dynamic_affine_sparse_gather_scalarizes_misaligned_loads_and_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let program = dynamic_affine_sparse_gather_program();
    let ptx = vyre_driver_cuda::codegen::program_to_ptx(&program, &DispatchConfig::default())
        .expect("Fix: CUDA PTX emission must support dynamic affine sparse gather vectorization.");
    assert!(
        !ptx.contains("ld.global.v2.u32")
            && !ptx.contains("ld.global.nc.v2.u32")
            && !ptx.contains("ld.global.v4.u32")
            && !ptx.contains("ld.global.nc.v4.u32"),
        "Fix: misaligned dynamic affine sparse gathers must not emit packed global loads.\n{ptx}"
    );
    assert!(
        !ptx.contains("st.global.v2.u32") && !ptx.contains("st.global.v4.u32"),
        "Fix: values from misaligned dynamic affine sparse gathers must not be repacked into unsafe global vector stores.\n{ptx}"
    );

    let input = generated_dynamic_u32_values(DYNAMIC_AFFINE_SOURCE_LANES, 0x3141_5926);
    let outputs = cuda_reference_outputs(
        &backend,
        &program,
        &[u32_bytes(&input)],
        "dynamic_affine_sparse_gather",
    );
    let checked = assert_u32_output_lanes(
        "dynamic_affine_sparse_gather direct",
        DYNAMIC_AFFINE_OUTPUT_LANES,
        &outputs.direct_cuda,
        &outputs.reference,
    ) + assert_u32_output_lanes(
        "dynamic_affine_sparse_gather compiled",
        DYNAMIC_AFFINE_OUTPUT_LANES,
        &outputs.compiled_cuda,
        &outputs.reference,
    );

    assert_eq!(
        checked,
        DYNAMIC_AFFINE_OUTPUT_LANES * 2,
        "Fix: live CUDA dynamic affine sparse gather must compare every output lane."
    );
}

#[test]
fn vectorized_dynamic_affine_sparse_scatter_emits_packed_v4_ptx_and_matches_reference_on_live_cuda()
{
    let backend = live_backend();
    let program = dynamic_affine_sparse_scatter_program();
    let ptx = vyre_driver_cuda::codegen::program_to_ptx(&program, &DispatchConfig::default())
        .expect("Fix: CUDA PTX emission must support dynamic affine sparse scatter vectorization.");
    assert!(
        ptx.contains("ld.global.v4.u32") || ptx.contains("ld.global.nc.v4.u32"),
        "Fix: dynamic affine sparse scatter input must emit a packed v4 global load.\n{ptx}"
    );
    assert!(
        !ptx.contains("st.global.v2.u32") && !ptx.contains("st.global.v4.u32"),
        "Fix: misaligned dynamic affine sparse scatters must not emit packed global stores.\n{ptx}"
    );

    let input = generated_dynamic_u32_values(DYNAMIC_AFFINE_OUTPUT_LANES, 0x2718_2818);
    let outputs = cuda_reference_outputs(
        &backend,
        &program,
        &[u32_bytes(&input)],
        "dynamic_affine_sparse_scatter",
    );
    let checked = assert_u32_output_lanes(
        "dynamic_affine_sparse_scatter direct",
        DYNAMIC_AFFINE_SOURCE_LANES,
        &outputs.direct_cuda,
        &outputs.reference,
    ) + assert_u32_output_lanes(
        "dynamic_affine_sparse_scatter compiled",
        DYNAMIC_AFFINE_SOURCE_LANES,
        &outputs.compiled_cuda,
        &outputs.reference,
    );

    assert_eq!(
        checked,
        DYNAMIC_AFFINE_SOURCE_LANES * 2,
        "Fix: live CUDA dynamic affine sparse scatter must compare every output lane."
    );
}

fn dynamic_affine_sparse_gather_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32)
                .with_count(DYNAMIC_AFFINE_SOURCE_LANES as u32),
            BufferDecl::output("out", 1, DataType::U32)
                .with_count(DYNAMIC_AFFINE_OUTPUT_LANES as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(DYNAMIC_AFFINE_GROUP_COUNT as u32)),
            vec![
                Node::let_bind(
                    "src_base",
                    Expr::mul(Expr::gid_x(), Expr::u32(DYNAMIC_AFFINE_STRIDE as u32)),
                ),
                Node::let_bind("dst_base", Expr::mul(Expr::gid_x(), Expr::u32(4))),
                Node::let_bind("s0", Expr::var("src_base")),
                Node::let_bind("s1", Expr::add(Expr::var("src_base"), Expr::u32(1))),
                Node::let_bind("s2", Expr::add(Expr::var("src_base"), Expr::u32(2))),
                Node::let_bind("s3", Expr::add(Expr::var("src_base"), Expr::u32(3))),
                Node::let_bind("d0", Expr::var("dst_base")),
                Node::let_bind("d1", Expr::add(Expr::var("dst_base"), Expr::u32(1))),
                Node::let_bind("d2", Expr::add(Expr::var("dst_base"), Expr::u32(2))),
                Node::let_bind("d3", Expr::add(Expr::var("dst_base"), Expr::u32(3))),
                Node::let_bind("v0", Expr::load("input", Expr::var("s0"))),
                Node::let_bind("v1", Expr::load("input", Expr::var("s1"))),
                Node::let_bind("v2", Expr::load("input", Expr::var("s2"))),
                Node::let_bind("v3", Expr::load("input", Expr::var("s3"))),
                Node::store("out", Expr::var("d0"), Expr::var("v0")),
                Node::store("out", Expr::var("d1"), Expr::var("v1")),
                Node::store("out", Expr::var("d2"), Expr::var("v2")),
                Node::store("out", Expr::var("d3"), Expr::var("v3")),
            ],
        )],
    )
}

fn dynamic_affine_sparse_scatter_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32)
                .with_count(DYNAMIC_AFFINE_OUTPUT_LANES as u32),
            BufferDecl::output("out", 1, DataType::U32)
                .with_count(DYNAMIC_AFFINE_SOURCE_LANES as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(DYNAMIC_AFFINE_GROUP_COUNT as u32)),
            vec![
                Node::let_bind("src_base", Expr::mul(Expr::gid_x(), Expr::u32(4))),
                Node::let_bind(
                    "dst_base",
                    Expr::mul(Expr::gid_x(), Expr::u32(DYNAMIC_AFFINE_STRIDE as u32)),
                ),
                Node::let_bind("s0", Expr::var("src_base")),
                Node::let_bind("s1", Expr::add(Expr::var("src_base"), Expr::u32(1))),
                Node::let_bind("s2", Expr::add(Expr::var("src_base"), Expr::u32(2))),
                Node::let_bind("s3", Expr::add(Expr::var("src_base"), Expr::u32(3))),
                Node::let_bind("d0", Expr::var("dst_base")),
                Node::let_bind("d1", Expr::add(Expr::var("dst_base"), Expr::u32(1))),
                Node::let_bind("d2", Expr::add(Expr::var("dst_base"), Expr::u32(2))),
                Node::let_bind("d3", Expr::add(Expr::var("dst_base"), Expr::u32(3))),
                Node::let_bind("v0", Expr::load("input", Expr::var("s0"))),
                Node::let_bind("v1", Expr::load("input", Expr::var("s1"))),
                Node::let_bind("v2", Expr::load("input", Expr::var("s2"))),
                Node::let_bind("v3", Expr::load("input", Expr::var("s3"))),
                Node::store("out", Expr::var("d0"), Expr::var("v0")),
                Node::store("out", Expr::var("d1"), Expr::var("v1")),
                Node::store("out", Expr::var("d2"), Expr::var("v2")),
                Node::store("out", Expr::var("d3"), Expr::var("v3")),
            ],
        )],
    )
}

fn generated_dynamic_u32_values(len: usize, salt: u32) -> Vec<u32> {
    (0..len)
        .map(|lane| {
            let lane = lane as u32;
            lane.wrapping_mul(0x9e37_79b9).rotate_left((lane & 31) + 1)
                ^ salt.rotate_right(lane & 31)
        })
        .collect()
}

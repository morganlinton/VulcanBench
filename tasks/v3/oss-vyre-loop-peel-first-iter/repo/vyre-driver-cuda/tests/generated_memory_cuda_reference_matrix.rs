//! Generated live CUDA/reference differential matrix for indexed memory semantics.

mod common;

use common::{
    assert_f32_output_lanes, assert_u32_output_lanes, bool_bytes, cuda_reference_outputs,
    f32_bytes, i32_bytes, live_backend, u32_bytes, GENERATED_LANE_COUNT as LANE_COUNT,
    GENERATED_WORKGROUP_SIZE_X as WORKGROUP_SIZE_X,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

const INDEX_MASK: u32 = LANE_COUNT as u32 - 1;
const MAX_F32_ULP: u32 = 0;

#[derive(Clone)]
struct MemoryCase {
    name: &'static str,
    ty: DataType,
    build_value: fn(Expr, Expr) -> Expr,
    build_src: fn(Expr) -> Expr,
    build_dst: fn(Expr) -> Expr,
}

fn identity_index(idx: Expr) -> Expr {
    idx
}

fn reverse_index(idx: Expr) -> Expr {
    Expr::sub(Expr::u32(INDEX_MASK), idx)
}

fn stride37_index(idx: Expr) -> Expr {
    Expr::bitand(Expr::mul(idx, Expr::u32(37)), Expr::u32(INDEX_MASK))
}

fn stride73_index(idx: Expr) -> Expr {
    Expr::bitand(Expr::mul(idx, Expr::u32(73)), Expr::u32(INDEX_MASK))
}

fn load_value(src: Expr, _idx: Expr) -> Expr {
    Expr::load("input", src)
}

fn u32_xor_lane(src: Expr, idx: Expr) -> Expr {
    Expr::bitxor(Expr::load("input", src), idx)
}

fn i32_add_lane(src: Expr, idx: Expr) -> Expr {
    Expr::add(Expr::load("input", src), Expr::cast(DataType::I32, idx))
}

fn f32_identity(src: Expr, _idx: Expr) -> Expr {
    Expr::load("input", src)
}

const U32_MEMORY_CASES: &[MemoryCase] = &[
    MemoryCase {
        name: "u32_reverse_store",
        ty: DataType::U32,
        build_value: load_value,
        build_src: identity_index,
        build_dst: reverse_index,
    },
    MemoryCase {
        name: "u32_reverse_load",
        ty: DataType::U32,
        build_value: load_value,
        build_src: reverse_index,
        build_dst: identity_index,
    },
    MemoryCase {
        name: "u32_stride_load_xor_lane",
        ty: DataType::U32,
        build_value: u32_xor_lane,
        build_src: stride73_index,
        build_dst: identity_index,
    },
    MemoryCase {
        name: "u32_stride_scatter",
        ty: DataType::U32,
        build_value: load_value,
        build_src: identity_index,
        build_dst: stride37_index,
    },
    MemoryCase {
        name: "u32_dual_permutation",
        ty: DataType::U32,
        build_value: u32_xor_lane,
        build_src: stride73_index,
        build_dst: stride37_index,
    },
];

const I32_MEMORY_CASES: &[MemoryCase] = &[
    MemoryCase {
        name: "i32_reverse_store",
        ty: DataType::I32,
        build_value: load_value,
        build_src: identity_index,
        build_dst: reverse_index,
    },
    MemoryCase {
        name: "i32_stride_load_add_lane",
        ty: DataType::I32,
        build_value: i32_add_lane,
        build_src: stride73_index,
        build_dst: identity_index,
    },
    MemoryCase {
        name: "i32_dual_permutation",
        ty: DataType::I32,
        build_value: i32_add_lane,
        build_src: stride73_index,
        build_dst: stride37_index,
    },
];

const F32_MEMORY_CASES: &[MemoryCase] = &[
    MemoryCase {
        name: "f32_reverse_store",
        ty: DataType::F32,
        build_value: f32_identity,
        build_src: identity_index,
        build_dst: reverse_index,
    },
    MemoryCase {
        name: "f32_reverse_load",
        ty: DataType::F32,
        build_value: f32_identity,
        build_src: reverse_index,
        build_dst: identity_index,
    },
    MemoryCase {
        name: "f32_dual_permutation",
        ty: DataType::F32,
        build_value: f32_identity,
        build_src: stride73_index,
        build_dst: stride37_index,
    },
];

const BOOL_MEMORY_CASES: &[MemoryCase] = &[
    MemoryCase {
        name: "bool_reverse_store",
        ty: DataType::Bool,
        build_value: load_value,
        build_src: identity_index,
        build_dst: reverse_index,
    },
    MemoryCase {
        name: "bool_reverse_load",
        ty: DataType::Bool,
        build_value: load_value,
        build_src: reverse_index,
        build_dst: identity_index,
    },
    MemoryCase {
        name: "bool_dual_permutation",
        ty: DataType::Bool,
        build_value: load_value,
        build_src: stride73_index,
        build_dst: stride37_index,
    },
];

#[test]
fn generated_u32_memory_permutation_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let input = generated_u32_values(0x3141_5926);
    let mut checked_lanes = 0usize;

    for case in U32_MEMORY_CASES {
        let program = memory_program(case);
        let inputs = vec![u32_bytes(&input)];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_lanes += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_lanes += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_lanes,
        U32_MEMORY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA u32 memory permutation matrix must keep every lane active across direct and compiled paths."
    );
}

#[test]
fn generated_i32_memory_permutation_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let input = generated_i32_values(0x2718_2818);
    let mut checked_lanes = 0usize;

    for case in I32_MEMORY_CASES {
        let program = memory_program(case);
        let inputs = vec![i32_bytes(&input)];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_lanes += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_lanes += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_lanes,
        I32_MEMORY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA i32 memory permutation matrix must keep every lane active across direct and compiled paths."
    );
}

#[test]
fn generated_f32_memory_permutation_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let input = generated_f32_values();
    let mut checked_lanes = 0usize;

    for case in F32_MEMORY_CASES {
        let program = memory_program(case);
        let inputs = vec![f32_bytes(&input)];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_lanes += assert_f32_output_lanes(
            case.name,
            LANE_COUNT,
            MAX_F32_ULP,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_lanes += assert_f32_output_lanes(
            case.name,
            LANE_COUNT,
            MAX_F32_ULP,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_lanes,
        F32_MEMORY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA f32 memory permutation matrix must keep every lane active across direct and compiled paths."
    );
}

#[test]
fn generated_bool_memory_permutation_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let input = generated_bool_values();
    let mut checked_lanes = 0usize;

    for case in BOOL_MEMORY_CASES {
        let program = memory_program(case);
        let inputs = vec![bool_bytes(&input)];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_lanes += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_lanes += assert_u32_output_lanes(
            case.name,
            LANE_COUNT,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_lanes,
        BOOL_MEMORY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA bool memory permutation matrix must keep every lane active across direct and compiled paths."
    );
}

fn memory_program(case: &MemoryCase) -> Program {
    let idx = Expr::var("idx");
    let src = (case.build_src)(idx.clone());
    let dst = (case.build_dst)(idx.clone());
    let value = (case.build_value)(src, idx);
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, case.ty.clone()).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, case.ty.clone()).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(LANE_COUNT as u32)),
                vec![Node::store("out", dst, value)],
            ),
        ],
    )
}

fn generated_u32_values(salt: u32) -> Vec<u32> {
    (0..LANE_COUNT)
        .map(|lane| {
            let lane = lane as u32;
            let mixed = lane.wrapping_mul(0x9e37_79b9).rotate_left((lane & 31) + 1)
                ^ salt.rotate_right(lane & 31);
            match lane % 12 {
                0 => 0,
                1 => 1,
                2 => u32::MAX,
                3 => 0x8000_0000,
                4 => 0x7fff_ffff,
                5 => 0x5555_5555,
                6 => 0xaaaa_aaaa,
                _ => mixed,
            }
        })
        .collect()
}

fn generated_i32_values(salt: u32) -> Vec<i32> {
    generated_u32_values(salt)
        .into_iter()
        .enumerate()
        .map(|(lane, word)| match lane % 10 {
            0 => i32::MIN,
            1 => i32::MAX,
            2 => -1,
            3 => 1,
            4 => -1024,
            5 => 1024,
            _ => word as i32,
        })
        .collect()
}

fn generated_f32_values() -> Vec<f32> {
    const SEEDS: &[u32] = &[
        0x0000_0000,
        0x8000_0000,
        0x3f80_0000,
        0xbf80_0000,
        0x4000_0000,
        0xc000_0000,
        0x3f00_0000,
        0xbf00_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
    ];
    (0..LANE_COUNT)
        .map(|lane| {
            let seed = SEEDS[lane % SEEDS.len()];
            let mixed = seed ^ (lane as u32).wrapping_mul(0x0101_0101);
            f32::from_bits(if lane % 7 == 0 {
                seed
            } else {
                mixed & 0x7f7f_ffff
            })
        })
        .collect()
}

fn generated_bool_values() -> Vec<bool> {
    (0..LANE_COUNT)
        .map(|lane| {
            let lane = lane as u32;
            ((lane.wrapping_mul(0x9e37_79b9) ^ lane.rotate_left(5)) & 0x11) != 0
        })
        .collect()
}

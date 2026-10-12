//! Generated live CUDA/reference differential matrix for cast and fused arithmetic semantics.

mod common;

use common::{
    assert_f32_output_lanes, assert_u32_output_lanes, bool_bytes, cuda_reference_outputs,
    f32_bytes, generated_bool_cast_values, generated_f32_cast_values, generated_f32_fma_values,
    generated_i32_cast_values, generated_u32_cast_values, i32_bytes, live_backend, u32_bytes,
    GENERATED_LANE_COUNT as LANE_COUNT, GENERATED_WORKGROUP_SIZE_X as WORKGROUP_SIZE_X,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

const MAX_F32_ULP: u32 = 1;

#[derive(Clone)]
struct CastCase {
    name: &'static str,
    input_type: DataType,
    output_type: DataType,
    input: CastInput,
    build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
enum CastInput {
    U32,
    I32,
    F32,
    Bool,
}

fn cast_to_u32(value: Expr) -> Expr {
    Expr::cast(DataType::U32, value)
}

fn cast_to_i32(value: Expr) -> Expr {
    Expr::cast(DataType::I32, value)
}

fn cast_to_f32(value: Expr) -> Expr {
    Expr::cast(DataType::F32, value)
}

fn cast_to_bool_word(value: Expr) -> Expr {
    Expr::select(
        Expr::cast(DataType::Bool, value),
        Expr::u32(1),
        Expr::u32(0),
    )
}

const CAST_CASES: &[CastCase] = &[
    CastCase {
        name: "cast_u32_to_i32",
        input_type: DataType::U32,
        output_type: DataType::I32,
        input: CastInput::U32,
        build: cast_to_i32,
    },
    CastCase {
        name: "cast_i32_to_u32",
        input_type: DataType::I32,
        output_type: DataType::U32,
        input: CastInput::I32,
        build: cast_to_u32,
    },
    CastCase {
        name: "cast_u32_to_f32_numeric",
        input_type: DataType::U32,
        output_type: DataType::F32,
        input: CastInput::U32,
        build: cast_to_f32,
    },
    CastCase {
        name: "cast_i32_to_f32_numeric",
        input_type: DataType::I32,
        output_type: DataType::F32,
        input: CastInput::I32,
        build: cast_to_f32,
    },
    CastCase {
        name: "cast_bool_to_u32",
        input_type: DataType::Bool,
        output_type: DataType::U32,
        input: CastInput::Bool,
        build: cast_to_u32,
    },
    CastCase {
        name: "cast_bool_to_i32",
        input_type: DataType::Bool,
        output_type: DataType::I32,
        input: CastInput::Bool,
        build: cast_to_i32,
    },
    CastCase {
        name: "cast_bool_to_f32_numeric",
        input_type: DataType::Bool,
        output_type: DataType::F32,
        input: CastInput::Bool,
        build: cast_to_f32,
    },
    CastCase {
        name: "cast_f32_to_u32",
        input_type: DataType::F32,
        output_type: DataType::U32,
        input: CastInput::F32,
        build: cast_to_u32,
    },
    CastCase {
        name: "cast_f32_to_i32",
        input_type: DataType::F32,
        output_type: DataType::I32,
        input: CastInput::F32,
        build: cast_to_i32,
    },
    CastCase {
        name: "cast_u32_to_bool_word",
        input_type: DataType::U32,
        output_type: DataType::U32,
        input: CastInput::U32,
        build: cast_to_bool_word,
    },
    CastCase {
        name: "cast_i32_to_bool_word",
        input_type: DataType::I32,
        output_type: DataType::U32,
        input: CastInput::I32,
        build: cast_to_bool_word,
    },
    CastCase {
        name: "cast_f32_to_bool_word",
        input_type: DataType::F32,
        output_type: DataType::U32,
        input: CastInput::F32,
        build: cast_to_bool_word,
    },
];

#[test]
fn generated_cast_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let u32_input = generated_u32_cast_values(LANE_COUNT);
    let i32_input = generated_i32_cast_values(LANE_COUNT);
    let f32_input = generated_f32_cast_values(LANE_COUNT);
    let bool_input = generated_bool_cast_values(LANE_COUNT);
    let mut checked_lanes = 0usize;

    for case in CAST_CASES {
        let program = cast_program(case);
        let inputs = vec![match case.input {
            CastInput::U32 => u32_bytes(&u32_input),
            CastInput::I32 => i32_bytes(&i32_input),
            CastInput::F32 => f32_bytes(&f32_input),
            CastInput::Bool => bool_bytes(&bool_input),
        }];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_lanes += assert_outputs(case, &outputs.direct_cuda, &outputs.reference);
        checked_lanes += assert_outputs(case, &outputs.compiled_cuda, &outputs.reference);
    }

    assert_eq!(
        checked_lanes,
        CAST_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA cast matrix must keep every lane active across direct and compiled paths."
    );
}

#[test]
fn generated_f32_fma_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let a = generated_f32_fma_values(LANE_COUNT, 0x1234_5678);
    let b = generated_f32_fma_values(LANE_COUNT, 0x9abc_def0);
    let c = generated_f32_fma_values(LANE_COUNT, 0x0fed_cba9);
    let program = f32_fma_program();
    let inputs = vec![f32_bytes(&a), f32_bytes(&b), f32_bytes(&c)];
    let outputs = cuda_reference_outputs(&backend, &program, &inputs, "f32_fma");
    let checked_lanes = assert_f32_output_lanes(
        "f32_fma",
        LANE_COUNT,
        MAX_F32_ULP,
        &outputs.direct_cuda,
        &outputs.reference,
    ) + assert_f32_output_lanes(
        "f32_fma",
        LANE_COUNT,
        MAX_F32_ULP,
        &outputs.compiled_cuda,
        &outputs.reference,
    );

    assert_eq!(
        checked_lanes,
        LANE_COUNT * 2,
        "Fix: generated CUDA f32 fma matrix must keep every lane active across direct and compiled paths."
    );
}

fn assert_outputs(case: &CastCase, actual: &[Vec<u8>], expected: &[Vec<u8>]) -> usize {
    if matches!(case.output_type, DataType::F32) {
        assert_f32_output_lanes(case.name, LANE_COUNT, MAX_F32_ULP, actual, expected)
    } else {
        assert_u32_output_lanes(case.name, LANE_COUNT, actual, expected)
    }
}

fn cast_program(case: &CastCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(Expr::load("input", idx));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, case.input_type.clone()).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, case.output_type.clone()).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn f32_fma_program() -> Program {
    let idx = Expr::var("idx");
    let value = Expr::fma(
        Expr::load("a", idx.clone()),
        Expr::load("b", idx.clone()),
        Expr::load("c", idx),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("b", 1, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("c", 2, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 3, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn guarded_store(value: Expr) -> Vec<Node> {
    vec![
        Node::let_bind("idx", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("idx"), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store("out", Expr::var("idx"), value)],
        ),
    ]
}

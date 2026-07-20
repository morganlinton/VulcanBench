//! Generated live CUDA/reference differential matrix for data-dependent control semantics.

mod common;
#[path = "common/generated_control_values.rs"]
mod generated_control_values;

use common::{
    assert_f32_output_lanes, assert_u32_output_lanes, bool_bytes, cuda_reference_outputs,
    f32_bytes, generated_mixed_bool_values as generated_bool_values, i32_bytes, live_backend,
    u32_bytes, GENERATED_LANE_COUNT as LANE_COUNT, GENERATED_WORKGROUP_SIZE_X as WORKGROUP_SIZE_X,
};
use generated_control_values::{
    generated_f32_values, generated_i32_values, generated_u32_values, MAX_F32_ULP,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

#[derive(Clone, Copy)]
struct U32SelectCase {
    name: &'static str,
    build: fn(Expr, Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct I32SelectCase {
    name: &'static str,
    build: fn(Expr, Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct F32SelectCase {
    name: &'static str,
    build: fn(Expr, Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct BoolSelectCase {
    name: &'static str,
    build: fn(Expr, Expr, Expr) -> Expr,
}

fn u32_select_eq_flag(flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(Expr::eq(flag, Expr::u32(0)), lhs, rhs)
}

fn u32_select_lt_min(_flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(Expr::lt(lhs.clone(), rhs.clone()), lhs, rhs)
}

fn u32_select_bit_mixed(flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(
        Expr::ne(Expr::bitand(flag, Expr::u32(1)), Expr::u32(0)),
        Expr::bitxor(lhs.clone(), rhs.clone()),
        Expr::add(lhs, rhs),
    )
}

fn u32_select_nested(flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    let low_flag = Expr::lt(flag.clone(), Expr::u32(0x8000_0000));
    let low_value = Expr::select(Expr::lt(lhs.clone(), rhs.clone()), lhs.clone(), rhs.clone());
    let high_value = Expr::select(
        Expr::gt(lhs.clone(), rhs.clone()),
        Expr::sub(lhs, rhs),
        flag,
    );
    Expr::select(low_flag, low_value, high_value)
}

fn i32_select_lt_min(_flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(Expr::lt(lhs.clone(), rhs.clone()), lhs, rhs)
}

fn i32_select_ge_delta(_flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(
        Expr::ge(lhs.clone(), rhs.clone()),
        Expr::sub(lhs.clone(), rhs.clone()),
        Expr::add(lhs, rhs),
    )
}

fn i32_select_flag_sign(flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(
        Expr::lt(flag, Expr::i32(0)),
        Expr::sub(Expr::i32(0), lhs),
        rhs,
    )
}

fn f32_select_nan(_flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(Expr::is_nan(lhs.clone()), rhs, lhs)
}

fn f32_select_finite(_flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(
        Expr::is_finite(lhs.clone()),
        Expr::add(lhs, rhs.clone()),
        rhs,
    )
}

fn f32_select_ordered(_flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(
        Expr::lt(lhs.clone(), rhs.clone()),
        Expr::mul(lhs.clone(), rhs.clone()),
        Expr::sub(lhs, rhs),
    )
}

fn bool_select_flag(flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(flag, lhs, rhs)
}

fn bool_select_eq(flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    Expr::select(Expr::eq(lhs.clone(), rhs), flag, lhs)
}

fn bool_select_nested(flag: Expr, lhs: Expr, rhs: Expr) -> Expr {
    let then_value = Expr::select(lhs.clone(), flag.clone(), rhs.clone());
    let else_value = Expr::select(rhs, lhs, flag.clone());
    Expr::select(flag, then_value, else_value)
}

const U32_SELECT_CASES: &[U32SelectCase] = &[
    U32SelectCase {
        name: "u32_select_eq_flag",
        build: u32_select_eq_flag,
    },
    U32SelectCase {
        name: "u32_select_lt_min",
        build: u32_select_lt_min,
    },
    U32SelectCase {
        name: "u32_select_bit_mixed",
        build: u32_select_bit_mixed,
    },
    U32SelectCase {
        name: "u32_select_nested",
        build: u32_select_nested,
    },
];

const I32_SELECT_CASES: &[I32SelectCase] = &[
    I32SelectCase {
        name: "i32_select_lt_min",
        build: i32_select_lt_min,
    },
    I32SelectCase {
        name: "i32_select_ge_delta",
        build: i32_select_ge_delta,
    },
    I32SelectCase {
        name: "i32_select_flag_sign",
        build: i32_select_flag_sign,
    },
];

const F32_SELECT_CASES: &[F32SelectCase] = &[
    F32SelectCase {
        name: "f32_select_nan",
        build: f32_select_nan,
    },
    F32SelectCase {
        name: "f32_select_finite",
        build: f32_select_finite,
    },
    F32SelectCase {
        name: "f32_select_ordered",
        build: f32_select_ordered,
    },
];

const BOOL_SELECT_CASES: &[BoolSelectCase] = &[
    BoolSelectCase {
        name: "bool_select_flag",
        build: bool_select_flag,
    },
    BoolSelectCase {
        name: "bool_select_eq",
        build: bool_select_eq,
    },
    BoolSelectCase {
        name: "bool_select_nested",
        build: bool_select_nested,
    },
];

#[test]
fn generated_u32_select_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let flag = generated_u32_values(0x1020_3040);
    let lhs = generated_u32_values(0xa5a5_5a5a);
    let rhs = generated_u32_values(0x5a5a_a5a5);
    let mut checked_lanes = 0usize;

    for case in U32_SELECT_CASES {
        let program = u32_select_program(case);
        let inputs = vec![u32_bytes(&flag), u32_bytes(&lhs), u32_bytes(&rhs)];
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
        U32_SELECT_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA u32 select matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_i32_select_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let flag = generated_i32_values(0x1020_3040);
    let lhs = generated_i32_values(0x1357_9bdf);
    let rhs = generated_i32_values(0xfdb9_7531);
    let mut checked_lanes = 0usize;

    for case in I32_SELECT_CASES {
        let program = i32_select_program(case);
        let inputs = vec![i32_bytes(&flag), i32_bytes(&lhs), i32_bytes(&rhs)];
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
        I32_SELECT_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA i32 select matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_f32_select_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let flag = generated_u32_values(0x3333_cccc);
    let lhs = generated_f32_values(0x1234_abcd);
    let rhs = generated_f32_values(0xdcba_4321);
    let mut checked_lanes = 0usize;

    for case in F32_SELECT_CASES {
        let program = f32_select_program(case);
        let inputs = vec![u32_bytes(&flag), f32_bytes(&lhs), f32_bytes(&rhs)];
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
        F32_SELECT_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA f32 select matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_bool_select_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let flag = generated_bool_values(0x3333_cccc);
    let lhs = generated_bool_values(0x1234_abcd);
    let rhs = generated_bool_values(0xdcba_4321);
    let mut checked_lanes = 0usize;

    for case in BOOL_SELECT_CASES {
        let program = bool_select_program(case);
        let inputs = vec![bool_bytes(&flag), bool_bytes(&lhs), bool_bytes(&rhs)];
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
        BOOL_SELECT_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA bool select matrix must keep predicate select and bool output storage active across direct and compiled paths."
    );
}

#[test]
fn generated_data_dependent_if_then_overwrite_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let flag = generated_u32_values(0xface_cafe);
    let lhs = generated_u32_values(0x0123_4567);
    let rhs = generated_u32_values(0x89ab_cdef);
    let program = u32_if_then_overwrite_program();
    let inputs = vec![u32_bytes(&flag), u32_bytes(&lhs), u32_bytes(&rhs)];
    let outputs = cuda_reference_outputs(&backend, &program, &inputs, "u32_if_then_overwrite");
    let checked_lanes = assert_u32_output_lanes(
        "u32_if_then_overwrite",
        LANE_COUNT,
        &outputs.direct_cuda,
        &outputs.reference,
    ) + assert_u32_output_lanes(
        "u32_if_then_overwrite",
        LANE_COUNT,
        &outputs.compiled_cuda,
        &outputs.reference,
    );

    assert_eq!(
        checked_lanes,
        LANE_COUNT * 2,
        "Fix: generated CUDA if_then matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

fn u32_select_program(case: &U32SelectCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("flag", idx.clone()),
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("flag", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::read("lhs", 1, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 2, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 3, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn i32_select_program(case: &I32SelectCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("flag", idx.clone()),
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("flag", 0, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::read("lhs", 1, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 2, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 3, DataType::I32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn f32_select_program(case: &F32SelectCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("flag", idx.clone()),
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("flag", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::read("lhs", 1, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 2, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 3, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn bool_select_program(case: &BoolSelectCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("flag", idx.clone()),
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("flag", 0, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::read("lhs", 1, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 2, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 3, DataType::Bool).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn u32_if_then_overwrite_program() -> Program {
    let idx = Expr::var("idx");
    Program::wrapped(
        vec![
            BufferDecl::read("flag", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::read("lhs", 1, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 2, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 3, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(LANE_COUNT as u32)),
                vec![
                    Node::store("out", Expr::var("idx"), Expr::load("rhs", Expr::var("idx"))),
                    Node::if_then(
                        Expr::ne(
                            Expr::bitand(Expr::load("flag", Expr::var("idx")), Expr::u32(1)),
                            Expr::u32(0),
                        ),
                        vec![Node::store(
                            "out",
                            Expr::var("idx"),
                            Expr::load("lhs", Expr::var("idx")),
                        )],
                    ),
                ],
            ),
        ],
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

//! Generated live CUDA/reference differential matrix for signed i32 IR semantics.

mod common;

use common::{
    assert_u32_output_lanes, cuda_reference_outputs, i32_bytes, live_backend,
    GENERATED_LANE_COUNT as LANE_COUNT, GENERATED_WORKGROUP_SIZE_X as WORKGROUP_SIZE_X,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

const ADVERSARIAL_I32_SEEDS: &[i32] = &[
    0,
    1,
    -1,
    2,
    -2,
    3,
    -3,
    7,
    -7,
    31,
    -31,
    32,
    -32,
    127,
    -127,
    128,
    -128,
    255,
    -255,
    1024,
    -1024,
    i16::MAX as i32,
    i16::MIN as i32,
    i32::MAX,
    i32::MIN,
    0x5555_5555,
    0x2aaa_aaaa,
    0x0123_4567,
    -0x0123_4567,
];

#[derive(Clone)]
struct I32BinaryCase {
    name: &'static str,
    rhs: I32RhsKind,
    output: DataType,
    build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone)]
struct I32UnaryCase {
    name: &'static str,
    output: DataType,
    build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
enum I32RhsKind {
    Mixed,
    DefinedDivisor,
}

fn eq_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::eq(left, right), Expr::u32(1), Expr::u32(0))
}

fn ne_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::ne(left, right), Expr::u32(1), Expr::u32(0))
}

fn lt_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::lt(left, right), Expr::u32(1), Expr::u32(0))
}

fn le_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::le(left, right), Expr::u32(1), Expr::u32(0))
}

fn gt_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::gt(left, right), Expr::u32(1), Expr::u32(0))
}

fn ge_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::ge(left, right), Expr::u32(1), Expr::u32(0))
}

fn wrapping_negate_i32(value: Expr) -> Expr {
    Expr::sub(Expr::i32(0), value)
}

const I32_BINARY_CASES: &[I32BinaryCase] = &[
    I32BinaryCase {
        name: "i32_add",
        rhs: I32RhsKind::Mixed,
        output: DataType::I32,
        build: Expr::add,
    },
    I32BinaryCase {
        name: "i32_sub",
        rhs: I32RhsKind::Mixed,
        output: DataType::I32,
        build: Expr::sub,
    },
    I32BinaryCase {
        name: "i32_mul",
        rhs: I32RhsKind::Mixed,
        output: DataType::I32,
        build: Expr::mul,
    },
    I32BinaryCase {
        name: "i32_div_defined",
        rhs: I32RhsKind::DefinedDivisor,
        output: DataType::I32,
        build: Expr::div,
    },
    I32BinaryCase {
        name: "i32_mod_defined",
        rhs: I32RhsKind::DefinedDivisor,
        output: DataType::U32,
        build: Expr::rem,
    },
    I32BinaryCase {
        name: "i32_bitand",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: Expr::bitand,
    },
    I32BinaryCase {
        name: "i32_bitor",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: Expr::bitor,
    },
    I32BinaryCase {
        name: "i32_bitxor",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: Expr::bitxor,
    },
    I32BinaryCase {
        name: "i32_min",
        rhs: I32RhsKind::Mixed,
        output: DataType::I32,
        build: Expr::min,
    },
    I32BinaryCase {
        name: "i32_max",
        rhs: I32RhsKind::Mixed,
        output: DataType::I32,
        build: Expr::max,
    },
    I32BinaryCase {
        name: "i32_eq",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: eq_word,
    },
    I32BinaryCase {
        name: "i32_ne",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: ne_word,
    },
    I32BinaryCase {
        name: "i32_lt",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: lt_word,
    },
    I32BinaryCase {
        name: "i32_le",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: le_word,
    },
    I32BinaryCase {
        name: "i32_gt",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: gt_word,
    },
    I32BinaryCase {
        name: "i32_ge",
        rhs: I32RhsKind::Mixed,
        output: DataType::U32,
        build: ge_word,
    },
];

const I32_UNARY_CASES: &[I32UnaryCase] = &[
    I32UnaryCase {
        name: "i32_negate",
        output: DataType::I32,
        build: wrapping_negate_i32,
    },
    I32UnaryCase {
        name: "i32_bitnot",
        output: DataType::I32,
        build: Expr::bitnot,
    },
    I32UnaryCase {
        name: "i32_popcount",
        output: DataType::I32,
        build: Expr::popcount,
    },
    I32UnaryCase {
        name: "i32_clz",
        output: DataType::I32,
        build: Expr::clz,
    },
    I32UnaryCase {
        name: "i32_ctz",
        output: DataType::I32,
        build: Expr::ctz,
    },
    I32UnaryCase {
        name: "i32_reverse_bits",
        output: DataType::I32,
        build: Expr::reverse_bits,
    },
];

#[test]
fn generated_i32_binary_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = adversarial_i32_values(0x3141_5926);

    let mut checked_lanes = 0usize;
    for case in I32_BINARY_CASES {
        let rhs = adversarial_i32_rhs(case.rhs, &lhs, 0x2718_2818);
        let program = i32_binary_program(case);
        let inputs = vec![i32_bytes(&lhs), i32_bytes(&rhs)];
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
        I32_BINARY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA i32 binary matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_i32_unary_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let input = adversarial_i32_values(0x1618_0339);

    let mut checked_lanes = 0usize;
    for case in I32_UNARY_CASES {
        let program = i32_unary_program(case);
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
        I32_UNARY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA i32 unary matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

fn i32_binary_program(case: &I32BinaryCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, case.output.clone()).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn i32_unary_program(case: &I32UnaryCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(Expr::load("input", idx));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, case.output.clone()).with_count(LANE_COUNT as u32),
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

fn adversarial_i32_values(salt: u32) -> Vec<i32> {
    (0..LANE_COUNT)
        .map(|lane| {
            let seed = ADVERSARIAL_I32_SEEDS[lane % ADVERSARIAL_I32_SEEDS.len()] as u32;
            let lane_word = lane as u32;
            let mixed = lane_word
                .wrapping_mul(0x9e37_79b9)
                .rotate_left((lane_word & 31) + 1)
                ^ salt.rotate_right(lane_word & 31);
            (seed ^ mixed) as i32
        })
        .collect()
}

fn adversarial_i32_rhs(kind: I32RhsKind, lhs: &[i32], salt: u32) -> Vec<i32> {
    adversarial_i32_values(salt)
        .into_iter()
        .enumerate()
        .map(|(lane, value)| match kind {
            I32RhsKind::Mixed if lane % 11 == 0 => lhs[lane],
            I32RhsKind::Mixed => value,
            I32RhsKind::DefinedDivisor => defined_i32_divisor(lhs[lane], value, lane),
        })
        .collect()
}

fn defined_i32_divisor(lhs: i32, value: i32, lane: usize) -> i32 {
    let candidate = if value == 0 {
        (lane as i32 % 31) + 1
    } else {
        value
    };
    if lhs == i32::MIN && candidate == -1 {
        1
    } else {
        candidate
    }
}

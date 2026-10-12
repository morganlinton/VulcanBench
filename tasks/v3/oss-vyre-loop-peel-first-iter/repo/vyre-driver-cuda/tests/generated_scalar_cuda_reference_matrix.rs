//! Generated live CUDA/reference differential matrix for scalar IR semantics.

mod common;

use common::{
    assert_u32_output_lanes, bool_bytes, cuda_reference_outputs, live_backend, u32_bytes,
    GENERATED_LANE_COUNT as LANE_COUNT, GENERATED_WORKGROUP_SIZE_X as WORKGROUP_SIZE_X,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

const ADVERSARIAL_SEEDS: &[u32] = &[
    0,
    1,
    2,
    3,
    7,
    31,
    32,
    63,
    127,
    128,
    255,
    256,
    1023,
    1024,
    0x7fff,
    0x8000,
    0xffff,
    0x1_0000,
    0x7fff_ffff,
    0x8000_0000,
    0xffff_fffe,
    0xffff_ffff,
    0x5555_5555,
    0xaaaa_aaaa,
    0x0123_4567,
    0x89ab_cdef,
    0xfedc_ba98,
];

#[derive(Clone, Copy)]
struct BinaryCase {
    name: &'static str,
    rhs: RhsKind,
    build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct UnaryCase {
    name: &'static str,
    build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct BoolBinaryCase {
    name: &'static str,
    build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct BoolUnaryCase {
    name: &'static str,
    build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
enum RhsKind {
    Mixed,
    Divisor,
    Shift,
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

fn bool_and_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::and(left, right), Expr::u32(1), Expr::u32(0))
}

fn bool_or_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::or(left, right), Expr::u32(1), Expr::u32(0))
}

fn bool_eq_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::eq(left, right), Expr::u32(1), Expr::u32(0))
}

fn bool_ne_word(left: Expr, right: Expr) -> Expr {
    Expr::select(Expr::ne(left, right), Expr::u32(1), Expr::u32(0))
}

fn bool_not_word(value: Expr) -> Expr {
    Expr::select(Expr::not(value), Expr::u32(1), Expr::u32(0))
}

const BINARY_CASES: &[BinaryCase] = &[
    BinaryCase {
        name: "add",
        rhs: RhsKind::Mixed,
        build: Expr::add,
    },
    BinaryCase {
        name: "sub",
        rhs: RhsKind::Mixed,
        build: Expr::sub,
    },
    BinaryCase {
        name: "mul",
        rhs: RhsKind::Mixed,
        build: Expr::mul,
    },
    BinaryCase {
        name: "div_total",
        rhs: RhsKind::Divisor,
        build: Expr::div,
    },
    BinaryCase {
        name: "mod_total",
        rhs: RhsKind::Divisor,
        build: Expr::rem,
    },
    BinaryCase {
        name: "mulhi",
        rhs: RhsKind::Mixed,
        build: Expr::mulhi,
    },
    BinaryCase {
        name: "abs_diff",
        rhs: RhsKind::Mixed,
        build: Expr::abs_diff,
    },
    BinaryCase {
        name: "bitand",
        rhs: RhsKind::Mixed,
        build: Expr::bitand,
    },
    BinaryCase {
        name: "bitor",
        rhs: RhsKind::Mixed,
        build: Expr::bitor,
    },
    BinaryCase {
        name: "bitxor",
        rhs: RhsKind::Mixed,
        build: Expr::bitxor,
    },
    BinaryCase {
        name: "shl_masked",
        rhs: RhsKind::Shift,
        build: Expr::shl,
    },
    BinaryCase {
        name: "shr_masked",
        rhs: RhsKind::Shift,
        build: Expr::shr,
    },
    BinaryCase {
        name: "min",
        rhs: RhsKind::Mixed,
        build: Expr::min,
    },
    BinaryCase {
        name: "max",
        rhs: RhsKind::Mixed,
        build: Expr::max,
    },
    BinaryCase {
        name: "eq",
        rhs: RhsKind::Mixed,
        build: eq_word,
    },
    BinaryCase {
        name: "ne",
        rhs: RhsKind::Mixed,
        build: ne_word,
    },
    BinaryCase {
        name: "lt",
        rhs: RhsKind::Mixed,
        build: lt_word,
    },
    BinaryCase {
        name: "le",
        rhs: RhsKind::Mixed,
        build: le_word,
    },
    BinaryCase {
        name: "gt",
        rhs: RhsKind::Mixed,
        build: gt_word,
    },
    BinaryCase {
        name: "ge",
        rhs: RhsKind::Mixed,
        build: ge_word,
    },
];

const UNARY_CASES: &[UnaryCase] = &[
    UnaryCase {
        name: "bitnot",
        build: Expr::bitnot,
    },
    UnaryCase {
        name: "popcount",
        build: Expr::popcount,
    },
    UnaryCase {
        name: "clz",
        build: Expr::clz,
    },
    UnaryCase {
        name: "ctz",
        build: Expr::ctz,
    },
    UnaryCase {
        name: "reverse_bits",
        build: Expr::reverse_bits,
    },
];

const BOOL_BINARY_CASES: &[BoolBinaryCase] = &[
    BoolBinaryCase {
        name: "bool_and",
        build: bool_and_word,
    },
    BoolBinaryCase {
        name: "bool_or",
        build: bool_or_word,
    },
    BoolBinaryCase {
        name: "bool_eq",
        build: bool_eq_word,
    },
    BoolBinaryCase {
        name: "bool_ne",
        build: bool_ne_word,
    },
];

const BOOL_UNARY_CASES: &[BoolUnaryCase] = &[BoolUnaryCase {
    name: "bool_not",
    build: bool_not_word,
}];

#[test]
fn generated_binary_scalar_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = adversarial_values(0x1357_2468);

    let mut checked_lanes = 0usize;
    for case in BINARY_CASES {
        let rhs = adversarial_rhs(case.rhs, &lhs, 0x9e37_79b9);
        let program = binary_program(case);
        let inputs = vec![u32_bytes(&lhs), u32_bytes(&rhs)];
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
        BINARY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA binary matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_unary_scalar_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let input = adversarial_values(0xfeed_babe);

    let mut checked_lanes = 0usize;
    for case in UNARY_CASES {
        let program = unary_program(case);
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
        UNARY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA unary matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_bool_scalar_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = adversarial_bool_values(0x1357_2468);
    let rhs = adversarial_bool_values(0x9e37_79b9);
    let mut checked_lanes = 0usize;

    for case in BOOL_BINARY_CASES {
        let program = bool_binary_program(case);
        let inputs = vec![bool_bytes(&lhs), bool_bytes(&rhs)];
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

    for case in BOOL_UNARY_CASES {
        let program = bool_unary_program(case);
        let inputs = vec![bool_bytes(&lhs)];
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
        (BOOL_BINARY_CASES.len() + BOOL_UNARY_CASES.len()) * LANE_COUNT * 2,
        "Fix: generated CUDA bool scalar matrix must keep predicate ALU and bool memory active across direct and compiled paths."
    );
}

fn binary_program(case: &BinaryCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn bool_binary_program(case: &BoolBinaryCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn bool_unary_program(case: &BoolUnaryCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(Expr::load("input", idx));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn unary_program(case: &UnaryCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(Expr::load("input", idx));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::U32).with_count(LANE_COUNT as u32),
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

fn adversarial_values(salt: u32) -> Vec<u32> {
    (0..LANE_COUNT)
        .map(|lane| {
            let seed = ADVERSARIAL_SEEDS[lane % ADVERSARIAL_SEEDS.len()];
            let lane_word = lane as u32;
            let mixed = lane_word
                .wrapping_mul(0x9e37_79b9)
                .rotate_left((lane_word & 31) + 1)
                ^ salt.rotate_right(lane_word & 31);
            seed ^ mixed
        })
        .collect()
}

fn adversarial_rhs(kind: RhsKind, lhs: &[u32], salt: u32) -> Vec<u32> {
    let mixed = adversarial_values(salt);
    mixed
        .into_iter()
        .enumerate()
        .map(|(lane, value)| match kind {
            RhsKind::Mixed if lane % 11 == 0 => lhs[lane],
            RhsKind::Mixed => value,
            RhsKind::Divisor if lane % 13 == 0 => 0,
            RhsKind::Divisor if lane % 17 == 0 => 1,
            RhsKind::Divisor => value,
            RhsKind::Shift => value & 31,
        })
        .collect()
}

fn adversarial_bool_values(salt: u32) -> Vec<bool> {
    (0..LANE_COUNT)
        .map(|lane| {
            let lane = lane as u32;
            let mixed = lane.wrapping_mul(0x45d9_f3b).rotate_left((lane & 7) + 1)
                ^ salt.rotate_right(lane & 31);
            (mixed & 0b1011) == 0b0001 || lane % 17 == 0
        })
        .collect()
}

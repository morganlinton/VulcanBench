//! Generated live CUDA/reference differential matrix for f32 IR semantics.

mod common;

use common::{
    assert_f32_output_lanes, assert_u32_output_lanes, cuda_reference_outputs, eq_word, f32_bytes,
    ge_word, gt_word, le_word, live_backend, lt_word, ne_word, GENERATED_LANE_COUNT as LANE_COUNT,
    GENERATED_WORKGROUP_SIZE_X as WORKGROUP_SIZE_X,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

const MAX_ARITH_ULP: u32 = 1;

const F32_ARITH_BITS: &[u32] = &[
    0x0000_0000, // +0
    0x8000_0000, // -0
    0x3f80_0000, // +1
    0xbf80_0000, // -1
    0x4000_0000, // +2
    0xc000_0000, // -2
    0x3f00_0000, // +0.5
    0xbf00_0000, // -0.5
    0x0080_0000, // smallest positive normal
    0x8080_0000, // largest-magnitude negative just-normal boundary
    0x7f7f_ffff, // max finite
    0xff7f_ffff, // min finite
    0x7f80_0000, // +inf
    0xff80_0000, // -inf
    0x7fc0_0000, // canonical quiet NaN
    0xffc0_0000, // negative quiet NaN
    0x7fa0_0001, // payload NaN
    0x3eaa_aaab, // 1/3 rounded
    0xbeaa_aaab, // -1/3 rounded
    0x4120_0000, // 10
    0xc120_0000, // -10
    0x447a_0000, // 1000
    0xc47a_0000, // -1000
];

const F32_CLASSIFY_BITS: &[u32] = &[
    0x0000_0000,
    0x8000_0000,
    0x0000_0001, // positive subnormal
    0x8000_0001, // negative subnormal
    0x007f_ffff, // largest positive subnormal
    0x807f_ffff, // largest negative subnormal
    0x0080_0000,
    0x8080_0000,
    0x3f80_0000,
    0xbf80_0000,
    0x7f7f_ffff,
    0xff7f_ffff,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7fa0_0001,
    0x7fff_ffff,
    0xffff_ffff,
];

#[derive(Clone, Copy)]
struct F32BinaryCase {
    name: &'static str,
    rhs: F32RhsKind,
    build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct F32UnaryCase {
    name: &'static str,
    inputs: F32InputKind,
    build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct F32ClassifyCase {
    name: &'static str,
    build: fn(Expr) -> Expr,
}

#[derive(Clone, Copy)]
struct F32CompareCase {
    name: &'static str,
    build: fn(Expr, Expr) -> Expr,
}

#[derive(Clone, Copy)]
enum F32RhsKind {
    Mixed,
    NonZero,
}

#[derive(Clone, Copy)]
enum F32InputKind {
    Mixed,
    SqrtDomain,
}

fn isnan_word(value: Expr) -> Expr {
    Expr::select(Expr::is_nan(value), Expr::u32(1), Expr::u32(0))
}

fn isinf_word(value: Expr) -> Expr {
    Expr::select(Expr::is_inf(value), Expr::u32(1), Expr::u32(0))
}

fn isfinite_word(value: Expr) -> Expr {
    Expr::select(Expr::is_finite(value), Expr::u32(1), Expr::u32(0))
}

const F32_BINARY_CASES: &[F32BinaryCase] = &[
    F32BinaryCase {
        name: "f32_add",
        rhs: F32RhsKind::Mixed,
        build: Expr::add,
    },
    F32BinaryCase {
        name: "f32_sub",
        rhs: F32RhsKind::Mixed,
        build: Expr::sub,
    },
    F32BinaryCase {
        name: "f32_mul",
        rhs: F32RhsKind::Mixed,
        build: Expr::mul,
    },
    F32BinaryCase {
        name: "f32_div_nonzero",
        rhs: F32RhsKind::NonZero,
        build: Expr::div,
    },
    F32BinaryCase {
        name: "f32_min",
        rhs: F32RhsKind::Mixed,
        build: Expr::min,
    },
    F32BinaryCase {
        name: "f32_max",
        rhs: F32RhsKind::Mixed,
        build: Expr::max,
    },
];

const F32_COMPARE_CASES: &[F32CompareCase] = &[
    F32CompareCase {
        name: "f32_eq",
        build: eq_word,
    },
    F32CompareCase {
        name: "f32_ne",
        build: ne_word,
    },
    F32CompareCase {
        name: "f32_lt",
        build: lt_word,
    },
    F32CompareCase {
        name: "f32_le",
        build: le_word,
    },
    F32CompareCase {
        name: "f32_gt",
        build: gt_word,
    },
    F32CompareCase {
        name: "f32_ge",
        build: ge_word,
    },
];

const F32_UNARY_CASES: &[F32UnaryCase] = &[
    F32UnaryCase {
        name: "f32_negate",
        inputs: F32InputKind::Mixed,
        build: Expr::negate,
    },
    F32UnaryCase {
        name: "f32_abs",
        inputs: F32InputKind::Mixed,
        build: Expr::abs,
    },
    F32UnaryCase {
        name: "f32_sqrt",
        inputs: F32InputKind::SqrtDomain,
        build: Expr::sqrt,
    },
    F32UnaryCase {
        name: "f32_reciprocal",
        inputs: F32InputKind::Mixed,
        build: Expr::reciprocal,
    },
    F32UnaryCase {
        name: "f32_floor",
        inputs: F32InputKind::Mixed,
        build: Expr::floor,
    },
    F32UnaryCase {
        name: "f32_ceil",
        inputs: F32InputKind::Mixed,
        build: Expr::ceil,
    },
    F32UnaryCase {
        name: "f32_trunc",
        inputs: F32InputKind::Mixed,
        build: Expr::trunc,
    },
];

const F32_CLASSIFY_CASES: &[F32ClassifyCase] = &[
    F32ClassifyCase {
        name: "f32_is_nan",
        build: isnan_word,
    },
    F32ClassifyCase {
        name: "f32_is_inf",
        build: isinf_word,
    },
    F32ClassifyCase {
        name: "f32_is_finite",
        build: isfinite_word,
    },
];

#[test]
fn generated_f32_binary_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = generated_f32_values(F32InputKind::Mixed, 0x1357_9bdf);
    let mut checked_lanes = 0usize;

    for case in F32_BINARY_CASES {
        let rhs = generated_f32_rhs(case.rhs, 0xf00d_cafe);
        let program = f32_binary_program(case);
        let inputs = vec![f32_bytes(&lhs), f32_bytes(&rhs)];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_lanes += assert_f32_output_lanes(
            case.name,
            LANE_COUNT,
            MAX_ARITH_ULP,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_lanes += assert_f32_output_lanes(
            case.name,
            LANE_COUNT,
            MAX_ARITH_ULP,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_lanes,
        F32_BINARY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA f32 binary matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_f32_unary_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let mut checked_lanes = 0usize;

    for case in F32_UNARY_CASES {
        let input = generated_f32_values(case.inputs, 0x2468_ace0);
        let program = f32_unary_program(case);
        let inputs = vec![f32_bytes(&input)];
        let outputs = cuda_reference_outputs(&backend, &program, &inputs, case.name);
        checked_lanes += assert_f32_output_lanes(
            case.name,
            LANE_COUNT,
            MAX_ARITH_ULP,
            &outputs.direct_cuda,
            &outputs.reference,
        );
        checked_lanes += assert_f32_output_lanes(
            case.name,
            LANE_COUNT,
            MAX_ARITH_ULP,
            &outputs.compiled_cuda,
            &outputs.reference,
        );
    }

    assert_eq!(
        checked_lanes,
        F32_UNARY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA f32 unary matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_f32_classification_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let input = generated_f32_classification_values();
    let mut checked_lanes = 0usize;

    for case in F32_CLASSIFY_CASES {
        let program = f32_classify_program(case);
        let inputs = vec![f32_bytes(&input)];
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
        F32_CLASSIFY_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA f32 classification matrix must keep every adversarial lane active across direct and compiled paths."
    );
}

#[test]
fn generated_f32_comparison_matrix_matches_reference_on_live_cuda() {
    let backend = live_backend();
    let lhs = generated_f32_values(F32InputKind::Mixed, 0x55aa_1234);
    let rhs = generated_f32_rhs(F32RhsKind::Mixed, 0xaa55_4321);
    let mut checked_lanes = 0usize;

    for case in F32_COMPARE_CASES {
        let program = f32_compare_program(case);
        let inputs = vec![f32_bytes(&lhs), f32_bytes(&rhs)];
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
        F32_COMPARE_CASES.len() * LANE_COUNT * 2,
        "Fix: generated CUDA f32 comparison matrix must keep NaN/Inf edge lanes active across direct and compiled paths."
    );
}

fn f32_binary_program(case: &F32BinaryCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn f32_compare_program(case: &F32CompareCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(
        Expr::load("lhs", idx.clone()),
        Expr::load("rhs", idx.clone()),
    );
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn f32_unary_program(case: &F32UnaryCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(Expr::load("input", idx));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        guarded_store(value),
    )
}

fn f32_classify_program(case: &F32ClassifyCase) -> Program {
    let idx = Expr::var("idx");
    let value = (case.build)(Expr::load("input", idx));
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(LANE_COUNT as u32),
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

fn generated_f32_values(kind: F32InputKind, salt: u32) -> Vec<f32> {
    (0..LANE_COUNT)
        .map(|lane| {
            let seed = F32_ARITH_BITS[lane % F32_ARITH_BITS.len()];
            let lane_word = lane as u32;
            let mixed = lane_word
                .wrapping_mul(0x45d9_f3b)
                .rotate_left((lane_word & 15) + 1)
                ^ salt.rotate_right(lane_word & 31);
            let bits = match kind {
                F32InputKind::Mixed => seed,
                F32InputKind::SqrtDomain => seed & 0x7fff_ffff,
            };
            f32::from_bits(if lane % 5 == 0 {
                bits
            } else {
                bits ^ (mixed & 0x007f_ffff)
            })
        })
        .collect()
}

fn generated_f32_rhs(kind: F32RhsKind, salt: u32) -> Vec<f32> {
    generated_f32_values(F32InputKind::Mixed, salt)
        .into_iter()
        .enumerate()
        .map(|(lane, value)| match kind {
            F32RhsKind::Mixed => value,
            F32RhsKind::NonZero if value == 0.0 || lane % 17 == 0 => {
                f32::from_bits(0x3f80_0000 ^ ((lane as u32) << 12))
            }
            F32RhsKind::NonZero => value,
        })
        .collect()
}

fn generated_f32_classification_values() -> Vec<f32> {
    (0..LANE_COUNT)
        .map(|lane| f32::from_bits(F32_CLASSIFY_BITS[lane % F32_CLASSIFY_BITS.len()]))
        .collect()
}

//! Variance reduction: `y = variance(x)` using Welford's parallel pair-combination algorithm.
//!
//! Category-A composition with a workgroup-tiled Welford reduction.

use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const OP_ID: &str = "vyre-libs::math::reduce_variance";
#[cfg(test)]
const REFERENCE_OP_ID: &str = "vyre-libs::math::reduce_variance_reference";
const REDUCE_VARIANCE_TILE: u32 = 256;
const EMPTY_REDUCTION_FIX: &str =
    "Fix: reduce_variance n=0 is invalid; pass at least one input element or route empty reductions to a caller-defined identity.";

/// Build a Program that computes the population variance of `input` into `output[0]`.
#[must_use]
pub fn reduce_variance(input: &str, output: &str, n: u32) -> Program {
    if n == 0 {
        return reduce_variance_invalid_program(input, output);
    }
    reduce_variance_tiled_program(input, output, n, false)
}

/// Fallible builder for variance reduction.
///
/// # Errors
///
/// Returns an actionable error for empty reductions.
pub fn try_reduce_variance(
    input: &str,
    output: &str,
    n: u32,
    bessel: bool,
) -> Result<Program, &'static str> {
    if n == 0 {
        return Err(EMPTY_REDUCTION_FIX);
    }
    Ok(reduce_variance_tiled_program(input, output, n, bessel))
}

fn reduce_variance_invalid_program(input: &str, output: &str) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(1),
            BufferDecl::output(output, 1, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            OP_ID,
            vec![Node::trap(Expr::u32(0), EMPTY_REDUCTION_FIX)],
        )],
    )
}

fn reduce_variance_tiled_program(input: &str, output: &str, n: u32, bessel: bool) -> Program {
    let tile = REDUCE_VARIANCE_TILE;
    let chunks = n.div_ceil(tile);
    let local = Expr::var("local");
    let idx = Expr::var("idx");

    // Per-lane Welford accumulation over grid-stride chunks.
    let mut body = vec![
        Node::let_bind("local", Expr::LocalId { axis: 0 }),
        Node::if_then(
            Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("n_i", Expr::u32(0)),
                Node::let_bind("M1_i", Expr::f32(0.0)),
                Node::let_bind("M2_i", Expr::f32(0.0)),
                Node::loop_for(
                    "chunk",
                    Expr::u32(0),
                    Expr::u32(chunks),
                    vec![
                        Node::let_bind(
                            "idx",
                            Expr::add(
                                Expr::mul(Expr::var("chunk"), Expr::u32(tile)),
                                local.clone(),
                            ),
                        ),
                        Node::if_then(
                            Expr::lt(idx.clone(), Expr::u32(n)),
                            vec![
                                Node::let_bind("x", Expr::load(input, idx.clone())),
                                Node::assign("n_i", Expr::add(Expr::var("n_i"), Expr::u32(1))),
                                Node::let_bind(
                                    "delta",
                                    Expr::sub(Expr::var("x"), Expr::var("M1_i")),
                                ),
                                Node::assign(
                                    "M1_i",
                                    Expr::add(
                                        Expr::var("M1_i"),
                                        Expr::div(
                                            Expr::var("delta"),
                                            Expr::cast(DataType::F32, Expr::var("n_i")),
                                        ),
                                    ),
                                ),
                                Node::let_bind(
                                    "delta2",
                                    Expr::sub(Expr::var("x"), Expr::var("M1_i")),
                                ),
                                Node::assign(
                                    "M2_i",
                                    Expr::add(
                                        Expr::var("M2_i"),
                                        Expr::mul(Expr::var("delta"), Expr::var("delta2")),
                                    ),
                                ),
                            ],
                        ),
                    ],
                ),
                Node::store("var_n_scratch", local.clone(), Expr::var("n_i")),
                Node::store("var_m1_scratch", local.clone(), Expr::var("M1_i")),
                Node::store("var_m2_scratch", local.clone(), Expr::var("M2_i")),
            ],
        ),
        Node::barrier(),
    ];

    // Workgroup-local tree reduction for Welford triples.
    let wg0_guard = Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0));
    let mut stride = tile.next_power_of_two() / 2;
    while stride > 0 {
        body.push(Node::if_then(
            Expr::and(
                wg0_guard.clone(),
                Expr::lt(Expr::var("local"), Expr::u32(stride)),
            ),
            vec![Node::if_then(
                Expr::lt(
                    Expr::add(Expr::var("local"), Expr::u32(stride)),
                    Expr::u32(tile),
                ),
                vec![
                    Node::let_bind("n_a", Expr::load("var_n_scratch", Expr::var("local"))),
                    Node::let_bind("M1_a", Expr::load("var_m1_scratch", Expr::var("local"))),
                    Node::let_bind("M2_a", Expr::load("var_m2_scratch", Expr::var("local"))),
                    Node::let_bind(
                        "n_b",
                        Expr::load(
                            "var_n_scratch",
                            Expr::add(Expr::var("local"), Expr::u32(stride)),
                        ),
                    ),
                    Node::let_bind(
                        "M1_b",
                        Expr::load(
                            "var_m1_scratch",
                            Expr::add(Expr::var("local"), Expr::u32(stride)),
                        ),
                    ),
                    Node::let_bind(
                        "M2_b",
                        Expr::load(
                            "var_m2_scratch",
                            Expr::add(Expr::var("local"), Expr::u32(stride)),
                        ),
                    ),
                    Node::let_bind("combined_n", Expr::add(Expr::var("n_a"), Expr::var("n_b"))),
                    Node::let_bind("n_a_f", Expr::cast(DataType::F32, Expr::var("n_a"))),
                    Node::let_bind("n_b_f", Expr::cast(DataType::F32, Expr::var("n_b"))),
                    Node::let_bind(
                        "combined_n_f",
                        Expr::cast(DataType::F32, Expr::var("combined_n")),
                    ),
                    Node::let_bind("delta", Expr::sub(Expr::var("M1_b"), Expr::var("M1_a"))),
                    Node::let_bind(
                        "combined_M1",
                        Expr::select(
                            Expr::eq(Expr::var("combined_n"), Expr::u32(0)),
                            Expr::f32(0.0),
                            Expr::div(
                                Expr::add(
                                    Expr::mul(Expr::var("n_a_f"), Expr::var("M1_a")),
                                    Expr::mul(Expr::var("n_b_f"), Expr::var("M1_b")),
                                ),
                                Expr::var("combined_n_f"),
                            ),
                        ),
                    ),
                    Node::let_bind(
                        "term",
                        Expr::mul(
                            Expr::mul(Expr::var("delta"), Expr::var("delta")),
                            Expr::select(
                                Expr::eq(Expr::var("combined_n"), Expr::u32(0)),
                                Expr::f32(0.0),
                                Expr::div(
                                    Expr::mul(Expr::var("n_a_f"), Expr::var("n_b_f")),
                                    Expr::var("combined_n_f"),
                                ),
                            ),
                        ),
                    ),
                    Node::let_bind(
                        "combined_M2",
                        Expr::add(
                            Expr::add(Expr::var("M2_a"), Expr::var("M2_b")),
                            Expr::var("term"),
                        ),
                    ),
                    Node::store("var_n_scratch", Expr::var("local"), Expr::var("combined_n")),
                    Node::store(
                        "var_m1_scratch",
                        Expr::var("local"),
                        Expr::var("combined_M1"),
                    ),
                    Node::store(
                        "var_m2_scratch",
                        Expr::var("local"),
                        Expr::var("combined_M2"),
                    ),
                ],
            )],
        ));
        body.push(Node::barrier());
        stride /= 2;
    }

    let total_n = Expr::load("var_n_scratch", Expr::u32(0));
    let denom = if bessel {
        Expr::cast(DataType::F32, Expr::sub(total_n.clone(), Expr::u32(1)))
    } else {
        Expr::cast(DataType::F32, total_n.clone())
    };

    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
            Expr::eq(local.clone(), Expr::u32(0)),
        ),
        vec![Node::Store {
            buffer: output.into(),
            index: Expr::u32(0),
            value: Expr::div(Expr::load("var_m2_scratch", Expr::u32(0)), denom),
        }],
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::workgroup("var_n_scratch", tile, DataType::U32),
            BufferDecl::workgroup("var_m1_scratch", tile, DataType::F32),
            BufferDecl::workgroup("var_m2_scratch", tile, DataType::F32),
            BufferDecl::output(output, 1, DataType::F32).with_count(1),
        ],
        [tile, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

#[cfg(test)]
fn reduce_variance_reference_program(input: &str, output: &str, n: u32, bessel: bool) -> Program {
    let body = vec![
        Node::let_bind("sum", Expr::f32(0.0)),
        Node::let_bind("sum_sq", Expr::f32(0.0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(n),
            vec![
                Node::let_bind("x", Expr::load(input, Expr::var("i"))),
                Node::assign("sum", Expr::add(Expr::var("sum"), Expr::var("x"))),
                Node::assign(
                    "sum_sq",
                    Expr::add(
                        Expr::var("sum_sq"),
                        Expr::mul(Expr::var("x"), Expr::var("x")),
                    ),
                ),
            ],
        ),
        Node::let_bind("mean", Expr::div(Expr::var("sum"), Expr::f32(n as f32))),
        Node::let_bind(
            "variance",
            Expr::div(
                Expr::sub(
                    Expr::var("sum_sq"),
                    Expr::mul(Expr::var("mean"), Expr::var("sum")),
                ),
                Expr::f32(if bessel { (n - 1) as f32 } else { n as f32 }),
            ),
        ),
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(0),
            value: Expr::var("variance"),
        },
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 1, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(REFERENCE_OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::math::reduce_variance",
        build: || reduce_variance("input", "output", 256),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_f32_slice;
            vec![vec![
                to_bytes(&[2.0_f32; 256]), // input
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_f32_slice;
            vec![vec![to_bytes(&[0.0_f32])]]
        }),
        category: Some("math"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32_one as decode_one;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn tiled_reduce_variance_matches_scalar_reference_across_multiple_tiles() {
        let n = 777_u32;
        let input = (0..n)
            .map(|i| ((i as f32) * 0.019).sin() * 4.0 + (i % 7) as f32)
            .collect::<Vec<_>>();
        let run = |program: Program| {
            let outputs = vyre_reference::reference_eval(
                &program,
                &[
                    Value::from(f32_bytes(&input)),
                    Value::from(vec![0u8; core::mem::size_of::<f32>()]),
                ],
            )
            .expect("Fix: reduce_variance program must execute in the reference interpreter.");
            decode_one(&outputs[0].to_bytes())
        };
        let actual = run(reduce_variance("input", "output", n));
        let expected = run(reduce_variance_reference_program(
            "input", "output", n, false,
        ));
        assert!(
            (actual - expected).abs() <= 1.0e-4,
            "reduce_variance mismatch: tiled={actual:?} reference={expected:?}"
        );
    }

    #[test]
    fn bessel_correction_changes_result_by_expected_ratio() {
        let n = 777_u32;
        let input = (0..n)
            .map(|i| ((i as f32) * 0.019).sin() * 4.0 + (i % 7) as f32)
            .collect::<Vec<_>>();
        let run = |program: Program| {
            let outputs = vyre_reference::reference_eval(
                &program,
                &[
                    Value::from(f32_bytes(&input)),
                    Value::from(vec![0u8; core::mem::size_of::<f32>()]),
                ],
            )
            .expect("Fix: reduce_variance program must execute in the reference interpreter.");
            decode_one(&outputs[0].to_bytes())
        };
        let pop = run(try_reduce_variance("input", "output", n, false).unwrap());
        let sample = run(try_reduce_variance("input", "output", n, true).unwrap());
        let expected_ratio = n as f32 / (n - 1) as f32;
        let actual_ratio = sample / pop;
        assert!(
            (actual_ratio - expected_ratio).abs() <= 1.0e-4,
            "Bessel correction ratio mismatch: sample={sample:?} pop={pop:?} expected_ratio={expected_ratio:?} actual_ratio={actual_ratio:?}"
        );
    }

    #[test]
    fn reduce_variance_rejects_empty_reduction_without_panicking() {
        let program = reduce_variance("input", "output", 0);
        let err = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vec![0u8; core::mem::size_of::<f32>()]),
                Value::from(vec![0u8; core::mem::size_of::<f32>()]),
            ],
        )
        .expect_err("empty reduction must trap instead of constructing a fake variance");
        assert!(
            err.to_string().contains(EMPTY_REDUCTION_FIX),
            "wrong error: {err}"
        );
        assert_eq!(
            try_reduce_variance("input", "output", 0, false),
            Err(EMPTY_REDUCTION_FIX)
        );
    }

    #[test]
    fn try_reduce_variance_returns_err_for_zero_count() {
        assert_eq!(
            try_reduce_variance("input", "output", 0, false),
            Err(EMPTY_REDUCTION_FIX)
        );
        assert_eq!(
            try_reduce_variance("input", "output", 0, true),
            Err(EMPTY_REDUCTION_FIX)
        );
    }
}

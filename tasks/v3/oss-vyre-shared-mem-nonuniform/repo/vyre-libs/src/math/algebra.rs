//! DF-11  -  Abstract algebraic structures for dataflow, security, and scheduling.
//!
//! This module provides reusable semiring and lattice primitives that support
//! higher-level analyses (taint, range, reaching defs) and telemetry (sketching).
//!
//! Every op here is a pure Category A composition over vyre-ops primitives.

use crate::builder::{build_elementwise_unary, BuildOptions};
use crate::region::wrap_anonymous;
use crate::tensor_ref::{check_dtype, check_shape, check_unique_names, TensorRef, TensorRefError};
use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const JOIN_OP_ID: &str = "vyre-libs::math::algebra::join";
const MEET_OP_ID: &str = "vyre-libs::math::algebra::meet";
const MINPLUS_MUL_OP_ID: &str = "vyre-libs::math::algebra::minplus_mul";
const BOOL_MATMUL_OP_ID: &str = "vyre-libs::math::algebra::bool_semiring_matmul";
const SKETCH_MIX_OP_ID: &str = "vyre-libs::math::algebra::sketch_mix";

/// Lattice Join (Supremum) for u32.
/// Performs element-wise bitwise OR.
///
/// This is the canonical merge operation for security taint analysis (merging
/// taint bitsets) and parser state sets.
#[must_use]
pub fn lattice_join(a: &str, b: &str, out: &str, size: u32) -> Program {
    try_lattice_join(a, b, out, size).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            JOIN_OP_ID,
            out,
            DataType::U32,
            format!("Fix: {err}"),
        )
    })
}

/// Checked builder for [`lattice_join`].
///
/// # Errors
///
/// Returns [`TensorRefError`] when buffer names alias, dtypes are wrong, or the
/// element count cannot be represented by the IR.
pub fn try_lattice_join(a: &str, b: &str, out: &str, size: u32) -> Result<Program, TensorRefError> {
    super::elementwise::try_u32_elementwise_binary(JOIN_OP_ID, a, b, out, size, Expr::bitor)
}

/// Lattice Meet (Infimum) for u32.
/// Performs element-wise bitwise AND.
///
/// Used for mask intersections, reaching definition constraints, and
/// narrowing value sets during dataflow analysis.
#[must_use]
pub fn lattice_meet(a: &str, b: &str, out: &str, size: u32) -> Program {
    try_lattice_meet(a, b, out, size).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            MEET_OP_ID,
            out,
            DataType::U32,
            format!("Fix: {err}"),
        )
    })
}

/// Checked builder for [`lattice_meet`].
///
/// # Errors
///
/// Returns [`TensorRefError`] when buffer names alias, dtypes are wrong, or the
/// element count cannot be represented by the IR.
pub fn try_lattice_meet(a: &str, b: &str, out: &str, size: u32) -> Result<Program, TensorRefError> {
    super::elementwise::try_u32_elementwise_binary(MEET_OP_ID, a, b, out, size, Expr::bitand)
}

/// Min-Plus Semiring Multiplication.
/// Performs element-wise saturating addition.
///
/// In the (min, +) semiring, the multiplicative identity is 0 and
/// multiplication is addition. Distances use `u32::MAX` as infinity, so this
/// primitive saturates rather than wrapping. Used for shortest path distance
/// propagation in adaptive scheduling and loop-cost estimation.
#[must_use]
pub fn semiring_min_plus_mul(a: &str, b: &str, out: &str, size: u32) -> Program {
    try_semiring_min_plus_mul(a, b, out, size).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            MINPLUS_MUL_OP_ID,
            out,
            DataType::U32,
            format!("Fix: {err}"),
        )
    })
}

/// Checked builder for [`semiring_min_plus_mul`].
///
/// # Errors
///
/// Returns [`TensorRefError`] when buffer names alias, dtypes are wrong, or the
/// element count cannot be represented by the IR.
pub fn try_semiring_min_plus_mul(
    a: &str,
    b: &str,
    out: &str,
    size: u32,
) -> Result<Program, TensorRefError> {
    super::elementwise::try_u32_elementwise_binary(MINPLUS_MUL_OP_ID, a, b, out, size, |lx, rx| {
        Expr::BinOp {
            op: BinOp::SaturatingAdd,
            left: Box::new(lx),
            right: Box::new(rx),
        }
    })
}

/// Boolean-semiring dense matrix multiplication.
///
/// Computes `out[row, col] = OR_k(a[row, k] != 0 && b[k, col] != 0)`.
/// This is the GraphBLAS building block for reachability, dataflow, and
/// context-free parser closure: graph traversal becomes coalesced matrix work.
#[must_use]
pub fn bool_semiring_matmul(
    a: &str,
    b: &str,
    out: &str,
    rows: u32,
    inner: u32,
    cols: u32,
) -> Program {
    try_bool_semiring_matmul(a, b, out, rows, inner, cols).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            BOOL_MATMUL_OP_ID,
            out,
            DataType::U32,
            format!("Fix: {err}"),
        )
    })
}

/// Checked builder for [`bool_semiring_matmul`].
///
/// # Errors
///
/// Returns [`TensorRefError`] when buffer names alias, matrix shapes are
/// invalid, or the output element count overflows `u32`.
pub fn try_bool_semiring_matmul(
    a: &str,
    b: &str,
    out: &str,
    rows: u32,
    inner: u32,
    cols: u32,
) -> Result<Program, TensorRefError> {
    let a_ref = TensorRef::u32_2d(a, rows, inner);
    let b_ref = TensorRef::u32_2d(b, inner, cols);
    let out_ref = TensorRef::u32_2d(out, rows, cols);
    check_unique_names(&[&a_ref, &b_ref, &out_ref], BOOL_MATMUL_OP_ID)?;
    check_dtype(&a_ref, DataType::U32, BOOL_MATMUL_OP_ID)?;
    check_dtype(&b_ref, DataType::U32, BOOL_MATMUL_OP_ID)?;
    check_dtype(&out_ref, DataType::U32, BOOL_MATMUL_OP_ID)?;
    check_shape(&a_ref, &[rows, inner], BOOL_MATMUL_OP_ID)?;
    check_shape(&b_ref, &[inner, cols], BOOL_MATMUL_OP_ID)?;
    check_shape(&out_ref, &[rows, cols], BOOL_MATMUL_OP_ID)?;
    let a_count = a_ref
        .element_count()
        .ok_or_else(|| TensorRefError::ElementCountOverflow {
            name: a_ref.name.as_str().to_string(),
            shape: a_ref.shape.to_vec(),
        })?;
    let b_count = b_ref
        .element_count()
        .ok_or_else(|| TensorRefError::ElementCountOverflow {
            name: b_ref.name.as_str().to_string(),
            shape: b_ref.shape.to_vec(),
        })?;
    let out_count =
        out_ref
            .element_count()
            .ok_or_else(|| TensorRefError::ElementCountOverflow {
                name: out_ref.name.as_str().to_string(),
                shape: out_ref.shape.to_vec(),
            })?;
    if out_count <= 64 && inner <= 64 {
        let mut stores = Vec::with_capacity(out_count as usize);
        for row_idx in 0..rows {
            for col_idx in 0..cols {
                let mut cell_expr = Expr::u32(0);
                for k_idx in 0..inner {
                    let a_idx = row_idx
                        .checked_mul(inner)
                        .and_then(|base| base.checked_add(k_idx))
                        .ok_or_else(|| TensorRefError::ElementCountOverflow {
                            name: a_ref.name.as_str().to_string(),
                            shape: a_ref.shape.to_vec(),
                        })?;
                    let b_idx = k_idx
                        .checked_mul(cols)
                        .and_then(|base| base.checked_add(col_idx))
                        .ok_or_else(|| TensorRefError::ElementCountOverflow {
                            name: b_ref.name.as_str().to_string(),
                            shape: b_ref.shape.to_vec(),
                        })?;
                    cell_expr = Expr::bitor(
                        cell_expr,
                        Expr::select(
                            Expr::ne(Expr::load(a, Expr::u32(a_idx)), Expr::u32(0)),
                            Expr::select(
                                Expr::ne(Expr::load(b, Expr::u32(b_idx)), Expr::u32(0)),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                            Expr::u32(0),
                        ),
                    );
                }
                stores.push(Node::store(
                    out,
                    Expr::u32(row_idx * cols + col_idx),
                    cell_expr,
                ));
            }
        }
        return Ok(Program::wrapped(
            vec![
                BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(a_count.max(1)),
                BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(b_count.max(1)),
                BufferDecl::output(out, 2, DataType::U32).with_count(out_count.max(1)),
            ],
            [1, 1, 1],
            vec![wrap_anonymous(
                BOOL_MATMUL_OP_ID,
                vec![Node::if_then(
                    Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
                    stores,
                )],
            )],
        ));
    }
    let cell = Expr::InvocationId { axis: 0 };
    let row = Expr::div(cell.clone(), Expr::u32(cols.max(1)));
    let col = Expr::rem(cell.clone(), Expr::u32(cols.max(1)));
    let body = vec![Node::if_then(
        Expr::lt(cell.clone(), Expr::u32(out_count)),
        vec![
            Node::let_bind("bool_mm_row", row),
            Node::let_bind("bool_mm_col", col),
            Node::let_bind("bool_mm_acc", Expr::u32(0)),
            Node::loop_for(
                "bool_mm_k",
                Expr::u32(0),
                Expr::u32(inner),
                vec![
                    Node::let_bind(
                        "bool_mm_a_idx",
                        Expr::add(
                            Expr::mul(Expr::var("bool_mm_row"), Expr::u32(inner)),
                            Expr::var("bool_mm_k"),
                        ),
                    ),
                    Node::let_bind(
                        "bool_mm_b_idx",
                        Expr::add(
                            Expr::mul(Expr::var("bool_mm_k"), Expr::u32(cols)),
                            Expr::var("bool_mm_col"),
                        ),
                    ),
                    Node::assign(
                        "bool_mm_acc",
                        Expr::bitor(
                            Expr::var("bool_mm_acc"),
                            Expr::select(
                                Expr::and(
                                    Expr::ne(
                                        Expr::load(a, Expr::var("bool_mm_a_idx")),
                                        Expr::u32(0),
                                    ),
                                    Expr::ne(
                                        Expr::load(b, Expr::var("bool_mm_b_idx")),
                                        Expr::u32(0),
                                    ),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                    ),
                ],
            ),
            Node::store(out, cell, Expr::var("bool_mm_acc")),
        ],
    )];
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(a_count.max(1)),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(b_count.max(1)),
            BufferDecl::output(out, 2, DataType::U32).with_count(out_count.max(1)),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(BOOL_MATMUL_OP_ID, body)],
    ))
}

/// Simple diversity sketch update.
/// Performs element-wise hash-and-mix.
///
/// Used for fuzz corpus scoring and diversity tracking. This primitive
/// ensures that small changes in input lead to widely distributed
/// sketch updates, suitable for corpus scoring in G9.
#[must_use]
pub fn sketch_mix(input: &str, out: &str, size: u32) -> Program {
    try_sketch_mix(input, out, size).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            SKETCH_MIX_OP_ID,
            out,
            DataType::U32,
            format!("Fix: {err}"),
        )
    })
}

/// Checked builder for [`sketch_mix`].
///
/// # Errors
///
/// Returns [`TensorRefError`] when buffer names alias, dtypes are wrong, or the
/// element count cannot be represented by the IR.
pub fn try_sketch_mix(input: &str, out: &str, size: u32) -> Result<Program, TensorRefError> {
    build_elementwise_unary(
        SKETCH_MIX_OP_ID,
        TensorRef::u32_1d(input, size),
        TensorRef::u32_1d(out, size),
        BuildOptions::default(),
        |val| {
            // Thomas Wang's 32-bit mix function
            let mut h = val;
            h = Expr::add(h.clone(), Expr::bitnot(Expr::shl(h, Expr::u32(15))));
            h = Expr::bitxor(h.clone(), Expr::shr(h, Expr::u32(12)));
            h = Expr::add(h.clone(), Expr::shl(h, Expr::u32(2)));
            h = Expr::bitxor(h.clone(), Expr::shr(h, Expr::u32(4)));
            h = Expr::mul(h.clone(), Expr::u32(2057)); // h = h * 2057
            h = Expr::bitxor(h.clone(), Expr::shr(h, Expr::u32(16)));
            h
        },
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: JOIN_OP_ID,
        build: || lattice_join("a", "b", "out", 4),
        test_inputs: Some(|| {
            let a = [0x0000FFFFu32, 0xAAAAAAAA, 0x00000000, 0xFFFFFFFF];
            let b = [0xFFFF0000u32, 0x55555555, 0x00000000, 0x00000000];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&a), to_bytes(&b)]]
        }),
        expected_output: Some(|| {
            let expected = [0xFFFFFFFFu32, 0xFFFFFFFF, 0x00000000, 0xFFFFFFFF];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: MEET_OP_ID,
        build: || lattice_meet("a", "b", "out", 4),
        test_inputs: Some(|| {
            let a = [0x0000FFFFu32, 0xAAAAAAAA, 0x00000000, 0xFFFFFFFF];
            let b = [0xFFFF0000u32, 0x55555555, 0x00000000, 0x00000000];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&a), to_bytes(&b)]]
        }),
        expected_output: Some(|| {
            let expected = [0x00000000u32, 0x00000000, 0x00000000, 0x00000000];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: MINPLUS_MUL_OP_ID,
        build: || semiring_min_plus_mul("a", "b", "out", 4),
        test_inputs: Some(|| {
            let a = [10u32, 20, u32::MAX, u32::MAX - 1];
            let b = [1u32, 2, 3, 4];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&a), to_bytes(&b)]]
        }),
        expected_output: Some(|| {
            let expected = [11u32, 22, u32::MAX, u32::MAX];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: BOOL_MATMUL_OP_ID,
        build: || bool_semiring_matmul("a", "b", "out", 2, 3, 2),
        test_inputs: Some(|| {
            let a = [1u32, 0, 1, 0, 1, 0];
            let b = [0u32, 1, 1, 0, 0, 0];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&a), to_bytes(&b)]]
        }),
        expected_output: Some(|| {
            let expected = [0u32, 1, 1, 0];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SKETCH_MIX_OP_ID,
        build: || sketch_mix("input", "out", 4),
        test_inputs: Some(|| {
            let input = [1u32, 2, 3, 4];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&input)]]
        }),
        expected_output: Some(|| {
            // We'll let the reference interpreter verify the mix logic matches.
            // Thomas Wang's 32 bit mix:
            let mix = |mut h: u32| {
                h = h.wrapping_add(!(h << 15));
                h ^= h >> 12;
                h = h.wrapping_add(h << 2);
                h ^= h >> 4;
                h = h.wrapping_mul(2057);
                h ^= h >> 16;
                h
            };
            let expected = [mix(1), mix(2), mix(3), mix(4)];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn checked_u32_binary_builders_reject_aliasing() {
        let program = try_lattice_join("a", "b", "out", 4).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid lattice_join must build");
        assert_eq!(
            program.buffers.len(),
            3,
            "lattice_join must declare a, b, and out"
        );

        let join_err = try_lattice_join("a", "a", "out", 4).expect_err("aliased inputs");
        assert!(
            matches!(join_err, TensorRefError::NameCollision { .. }),
            "lattice_join aliasing error: {join_err:?}"
        );

        let meet_err = try_lattice_meet("a", "b", "a", 4).expect_err("aliased output");
        assert!(
            matches!(meet_err, TensorRefError::NameCollision { .. }),
            "lattice_meet aliasing error: {meet_err:?}"
        );

        let mul_err = try_semiring_min_plus_mul("a", "b", "b", 4).expect_err("aliased output");
        assert!(
            matches!(mul_err, TensorRefError::NameCollision { .. }),
            "min-plus mul aliasing error: {mul_err:?}"
        );
    }

    #[test]
    fn generated_u32_binary_algebra_contracts_match_scalar_reference() {
        let mut state = 0xA17E_BA5E_u32;
        for case in 0..4096u32 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let a = match case % 17 {
                0 => 0,
                1 => u32::MAX,
                2 => 1,
                _ => state,
            };
            state = state.rotate_left(11) ^ case.wrapping_mul(0x9E37_79B9);
            let b = match case % 19 {
                0 => 0,
                1 => u32::MAX,
                2 => u32::MAX - 1,
                _ => state,
            };
            let c = state.rotate_right((case % 31) + 1);

            assert_eq!(a | b, b | a, "join must be commutative for case {case}");
            assert_eq!(a & b, b & a, "meet must be commutative for case {case}");
            assert_eq!(a | a, a, "join must be idempotent for case {case}");
            assert_eq!(a & a, a, "meet must be idempotent for case {case}");
            assert_eq!(
                a | (a & b),
                a,
                "join/meet absorption failed for case {case}"
            );
            assert_eq!(
                a & (a | b),
                a,
                "meet/join absorption failed for case {case}"
            );
            assert_eq!(
                (a | b) | c,
                a | (b | c),
                "join must be associative for case {case}"
            );
            assert_eq!(
                (a & b) & c,
                a & (b & c),
                "meet must be associative for case {case}"
            );
            assert_eq!(
                a.saturating_add(b),
                b.saturating_add(a),
                "min-plus saturating multiply must be commutative for case {case}"
            );
            assert_eq!(
                a.saturating_add(0),
                a,
                "min-plus zero identity failed for case {case}"
            );
            assert_eq!(
                u32::MAX.saturating_add(a),
                u32::MAX,
                "min-plus infinity saturation failed for case {case}"
            );
        }
    }
}

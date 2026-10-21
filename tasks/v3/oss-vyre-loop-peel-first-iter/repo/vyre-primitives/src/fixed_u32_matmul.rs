//! Shared fixed-point u32 matrix-contraction IR builder.
//!
//! Several primitive domains expose matrix multiplication under different
//! semantics: tensor-network contraction and categorical monoidal composition
//! are intentionally separate public ops, but their GPU kernel body is the same
//! 16.16 fixed-point row/column contraction. This module keeps that kernel in
//! one place while callers retain their own validation language and op ids.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Return `rows * cols` as a u32 cell count with an actionable overflow error.
pub(crate) fn checked_cells(label: &'static str, rows: u32, cols: u32) -> Result<u32, String> {
    rows.checked_mul(cols).ok_or_else(|| {
        format!(
            "{label} rows*cols overflows cell count for rows={rows}, cols={cols}. Fix: shard the contraction before GPU dispatch."
        )
    })
}

/// Build a customizable u32 matrix contraction.
///
/// The caller supplies the scalar combine operation for each
/// `lhs[i, kk]`/`rhs[kk, j]` pair and the accumulator operation that folds it
/// into `acc`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn u32_matmul_program<C, A>(
    op_id: &'static str,
    lhs: &str,
    rhs: &str,
    out: &str,
    _rows: u32,
    shared: u32,
    cols: u32,
    lhs_cells: u32,
    rhs_cells: u32,
    out_cells: u32,
    identity: u32,
    combine: C,
    accumulate: A,
) -> Program
where
    C: Fn(Expr, Expr) -> Expr,
    A: Fn(Expr, Expr) -> Expr,
{
    let t = Expr::InvocationId { axis: 0 };
    let i_expr = Expr::div(t.clone(), Expr::u32(cols));
    let j_expr = Expr::rem(t.clone(), Expr::u32(cols));
    let lhs_value = Expr::load(
        lhs,
        Expr::add(
            Expr::mul(Expr::var("i"), Expr::u32(shared)),
            Expr::var("kk"),
        ),
    );
    let rhs_value = Expr::load(
        rhs,
        Expr::add(Expr::mul(Expr::var("kk"), Expr::u32(cols)), Expr::var("j")),
    );
    let combined = combine(lhs_value, rhs_value);
    let folded = accumulate(Expr::var("acc"), combined);

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(out_cells)),
        vec![
            Node::let_bind("acc", Expr::u32(identity)),
            Node::let_bind("i", i_expr),
            Node::let_bind("j", j_expr),
            Node::loop_for(
                "kk",
                Expr::u32(0),
                Expr::u32(shared),
                vec![Node::assign("acc", folded)],
            ),
            Node::store(out, t, Expr::var("acc")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(lhs, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lhs_cells),
            BufferDecl::storage(rhs, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(rhs_cells),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build `out[rows x cols] = lhs[rows x shared] * rhs[shared x cols]`.
///
/// Inputs and output use unsigned 16.16 fixed-point lanes packed as u32. The
/// caller owns all semantic naming and validation; this function only owns the
/// common kernel shape and buffer layout.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn fixed_u32_matmul_program(
    op_id: &'static str,
    lhs: &str,
    rhs: &str,
    out: &str,
    _rows: u32,
    shared: u32,
    cols: u32,
    lhs_cells: u32,
    rhs_cells: u32,
    out_cells: u32,
) -> Program {
    u32_matmul_program(
        op_id,
        lhs,
        rhs,
        out,
        _rows,
        shared,
        cols,
        lhs_cells,
        rhs_cells,
        out_cells,
        0,
        crate::fixed_mul_16_16_expr,
        Expr::add,
    )
}

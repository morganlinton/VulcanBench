//! Reverse-mode autodiff: forward locals used in nonlinear ops.
//!
//! The adjoint of a nonlinear op carries the forward operand's VALUE
//! (`d(a*a)` accumulates `adjoint * a`). The generated backward Program
//! re-declares forward BUFFERS as ReadOnly but never re-materializes forward
//! LOCALS, so an adjoint that embeds `Var(a)` for a forward `let a = ...`
//! dangles. Before the fix, `grad()` returned `Ok` with a backward Program
//! that failed validation only when the caller tried to run it
//! ("reference to undeclared variable `a`"). Now `grad()` fails closed with a
//! clear `NotDifferentiable` error naming the local.
//!
//! The boundary is precise: a local used only LINEARLY (its value never enters
//! an adjoint expression) still differentiates; only nonlinear use is refused.
//! The buffer-load form of the same math differentiates AND the oracle
//! confirms the gradient value.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::transform::autodiff::error::AutodiffError;
use vyre_foundation::transform::autodiff::grad::grad;
use vyre_reference::value::Value;

/// Forward: `let a = x[0]; out[0] = a * a`. The adjoint needs `a`'s forward
/// value, which the backward Program cannot recompute -> fail closed.
#[test]
fn grad_fails_closed_on_forward_local_used_nonlinearly() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(1),
            BufferDecl::output("out", 1, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::load("x", Expr::u32(0))),
            Node::store("out", Expr::u32(0), Expr::mul(Expr::var("a"), Expr::var("a"))),
        ],
    );

    let err = grad(&program, &["out"], &["x"]).expect_err(
        "grad must refuse a forward local used nonlinearly instead of emitting an \
         invalid backward Program",
    );
    match err {
        AutodiffError::NotDifferentiable { op, .. } => assert!(
            op.contains("forward local `a`"),
            "error must name the offending forward local `a`; got op={op}",
        ),
        other => panic!("expected NotDifferentiable naming the local, got {other:?}"),
    }
}

/// The buffer-load form of the same math: `out[0] = x[0] * x[0]`. No forward
/// local, so the backward is self-contained. The oracle confirms
/// `grad_x[0] == 2 * x[0]`.
#[test]
fn grad_buffer_load_square_oracle_confirms_two_x() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(1),
            BufferDecl::output("out", 1, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::mul(Expr::load("x", Expr::u32(0)), Expr::load("x", Expr::u32(0))),
        )],
    );

    let backward = grad(&program, &["out"], &["x"]).expect("buffer-load square must differentiate");

    // Backward buffers: x(ro), out(ro), grad_out(rw), grad_x(rw) -- all
    // non-output, so the reference wants one input Value each. x = 3.0; the
    // grad buffers are cleared by the backward itself so their inputs are
    // placeholders; out is unused by the square's backward.
    let inputs = [
        Value::from(3.0f32.to_le_bytes().to_vec()),  // x
        Value::from(0.0f32.to_le_bytes().to_vec()),  // out (unused)
        Value::from(0.0f32.to_le_bytes().to_vec()),  // grad_out (cleared+seeded)
        Value::from(0.0f32.to_le_bytes().to_vec()),  // grad_x (cleared)
    ];
    let results = vyre_reference::reference_eval(&backward, &inputs)
        .expect("buffer-load square backward must validate and run");

    // d(x^2)/dx = 2x = 6.0 at x = 3.0.
    assert!(
        results.contains(&Value::from(6.0f32.to_le_bytes().to_vec())),
        "grad_x[0] must equal 2*x[0] == 6.0; results = {results:?}",
    );
}

/// A forward local used only LINEARLY: `let xw = x*w; out = xw + x`. The adjoint
/// of `+` never multiplies in `xw`'s value, so `Var(xw)` never enters an emitted
/// adjoint expression. This must still differentiate (the fail-closed guard is
/// not over-broad).
#[test]
fn grad_still_supports_linearly_used_local() {
    let i = Expr::InvocationId { axis: 0 };
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(4),
            BufferDecl::storage("w", 1, BufferAccess::ReadOnly, DataType::F32).with_count(4),
            BufferDecl::output("out", 2, DataType::F32).with_count(4),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind(
                "xw",
                Expr::mul(Expr::load("x", i.clone()), Expr::load("w", i.clone())),
            ),
            Node::store("out", i.clone(), Expr::add(Expr::var("xw"), Expr::load("x", i))),
        ],
    );

    grad(&program, &["out"], &["x", "w"])
        .expect("a linearly-used forward local must still differentiate");
}

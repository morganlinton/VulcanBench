//! Symbolic differentiation rules for Vyre IR expressions.
//!
//! Each rule takes the forward expression + the adjoint flowing into its
//! output and returns a list of `(child_expr, child_adjoint)` pairs that
//! propagate the chain rule down.

use crate::ir::{BinOp, Expr, UnOp};

use super::AutodiffError;

/// One adjoint contribution: the adjoint value to add to a child expression.
#[derive(Debug, Clone)]
pub struct AdjointContrib {
    /// The child expression this adjoint flows to.
    pub child: Expr,
    /// The adjoint value: `parent_adjoint * local_jacobian`.
    pub adjoint: Expr,
}

/// Compute the adjoint contributions for a `BinOp` expression.
///
/// Given `z = left ⊕ right` and `dz` (adjoint on z), returns `(d_left, d_right)`.
///
/// # Errors
///
/// Returns `NotDifferentiable` for integer/bitwise/comparison ops.
#[expect(
    clippy::too_many_lines,
    reason = "binary autodiff rules are a compact mathematical dispatch table and must remain auditable as a complete BinOp surface"
)]
pub fn binop_adjoints(
    op: BinOp,
    left: &Expr,
    right: &Expr,
    adjoint: &Expr,
) -> Result<Vec<AdjointContrib>, AutodiffError> {
    match op {
        // d(a+b)/da = 1, d(a+b)/db = 1
        BinOp::Add => Ok(vec![
            AdjointContrib {
                child: left.clone(),
                adjoint: adjoint.clone(),
            },
            AdjointContrib {
                child: right.clone(),
                adjoint: adjoint.clone(),
            },
        ]),
        // d(a-b)/da = 1, d(a-b)/db = -1
        BinOp::Sub => Ok(vec![
            AdjointContrib {
                child: left.clone(),
                adjoint: adjoint.clone(),
            },
            AdjointContrib {
                child: right.clone(),
                adjoint: Expr::UnOp {
                    op: UnOp::Negate,
                    operand: Box::new(adjoint.clone()),
                },
            },
        ]),
        // d(a*b)/da = b, d(a*b)/db = a
        BinOp::Mul => Ok(vec![
            AdjointContrib {
                child: left.clone(),
                adjoint: Expr::mul(adjoint.clone(), right.clone()),
            },
            AdjointContrib {
                child: right.clone(),
                adjoint: Expr::mul(adjoint.clone(), left.clone()),
            },
        ]),
        // d(a/b)/da = 1/b, d(a/b)/db = -a/b²
        BinOp::Div => Ok(vec![
            AdjointContrib {
                child: left.clone(),
                adjoint: Expr::div(adjoint.clone(), right.clone()),
            },
            AdjointContrib {
                child: right.clone(),
                adjoint: Expr::UnOp {
                    op: UnOp::Negate,
                    operand: Box::new(Expr::div(
                        Expr::mul(adjoint.clone(), left.clone()),
                        Expr::mul(right.clone(), right.clone()),
                    )),
                },
            },
        ]),
        // d(min(a,b))/da = (a < b) ? dz : 0, d(min(a,b))/db = (b <= a) ? dz : 0
        // Subgradient: route adjoint to the arg that was selected.
        BinOp::Min => Ok(vec![
            AdjointContrib {
                child: left.clone(),
                adjoint: Expr::Select {
                    cond: Box::new(Expr::lt(left.clone(), right.clone())),
                    true_val: Box::new(adjoint.clone()),
                    false_val: Box::new(Expr::f32(0.0)),
                },
            },
            AdjointContrib {
                child: right.clone(),
                adjoint: Expr::Select {
                    cond: Box::new(Expr::le(left.clone(), right.clone())),
                    true_val: Box::new(Expr::f32(0.0)),
                    false_val: Box::new(adjoint.clone()),
                },
            },
        ]),
        // max: mirror of min
        BinOp::Max => Ok(vec![
            AdjointContrib {
                child: left.clone(),
                adjoint: Expr::Select {
                    cond: Box::new(Expr::gt(left.clone(), right.clone())),
                    true_val: Box::new(adjoint.clone()),
                    false_val: Box::new(Expr::f32(0.0)),
                },
            },
            AdjointContrib {
                child: right.clone(),
                adjoint: Expr::Select {
                    cond: Box::new(Expr::gt(left.clone(), right.clone())),
                    true_val: Box::new(Expr::f32(0.0)),
                    false_val: Box::new(adjoint.clone()),
                },
            },
        ]),
        // Non-differentiable
        BinOp::Mod
        | BinOp::BitAnd
        | BinOp::BitOr
        | BinOp::BitXor
        | BinOp::Shl
        | BinOp::Shr
        | BinOp::WrappingAdd
        | BinOp::WrappingSub
        | BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Gt
        | BinOp::Le
        | BinOp::Ge
        | BinOp::And
        | BinOp::Or
        | BinOp::AbsDiff
        | BinOp::SaturatingAdd
        | BinOp::SaturatingSub
        | BinOp::SaturatingMul
        | BinOp::Shuffle
        | BinOp::Ballot
        | BinOp::WaveReduce
        | BinOp::WaveBroadcast
        | BinOp::RotateLeft
        | BinOp::RotateRight
        | BinOp::Opaque(_)
        | _ => Err(AutodiffError::NotDifferentiable {
            op: format!("BinOp::{op:?}"),
            fix: "replace with a differentiable equivalent or gate behind a stop-gradient barrier"
                .into(),
        }),
    }
}

/// Compute the adjoint contribution for a `UnOp` expression.
///
/// Given `z = op(x)` and `dz`, returns `dx = dz * dz/dx`.
///
/// # Errors
///
/// Returns `NotDifferentiable` for integer/bitwise ops.
#[expect(
    clippy::too_many_lines,
    reason = "unary autodiff rules are a compact mathematical dispatch table and must remain auditable as a complete UnOp surface"
)]
pub fn unop_adjoint(
    op: &UnOp,
    operand: &Expr,
    adjoint: &Expr,
) -> Result<AdjointContrib, AutodiffError> {
    let dx = match op {
        // d(-x)/dx = -1
        UnOp::Negate => Expr::UnOp {
            op: UnOp::Negate,
            operand: Box::new(adjoint.clone()),
        },
        // d(exp(x))/dx = exp(x)
        UnOp::Exp => Expr::mul(
            adjoint.clone(),
            Expr::UnOp {
                op: UnOp::Exp,
                operand: Box::new(operand.clone()),
            },
        ),
        // d(log(x))/dx = 1/x
        UnOp::Log => Expr::div(adjoint.clone(), operand.clone()),
        // d(sqrt(x))/dx = 1 / (2*sqrt(x))
        UnOp::Sqrt => Expr::div(
            adjoint.clone(),
            Expr::mul(
                Expr::f32(2.0),
                Expr::UnOp {
                    op: UnOp::Sqrt,
                    operand: Box::new(operand.clone()),
                },
            ),
        ),
        // d(tanh(x))/dx = 1 - tanh²(x)
        UnOp::Tanh => {
            let t = Expr::UnOp {
                op: UnOp::Tanh,
                operand: Box::new(operand.clone()),
            };
            Expr::mul(
                adjoint.clone(),
                Expr::sub(Expr::f32(1.0), Expr::mul(t.clone(), t)),
            )
        }
        // d(sin(x))/dx = cos(x)
        UnOp::Sin => Expr::mul(
            adjoint.clone(),
            Expr::UnOp {
                op: UnOp::Cos,
                operand: Box::new(operand.clone()),
            },
        ),
        // d(cos(x))/dx = -sin(x)
        UnOp::Cos => Expr::UnOp {
            op: UnOp::Negate,
            operand: Box::new(Expr::mul(
                adjoint.clone(),
                Expr::UnOp {
                    op: UnOp::Sin,
                    operand: Box::new(operand.clone()),
                },
            )),
        },
        // d(abs(x))/dx = sign(x)
        UnOp::Abs => Expr::mul(
            adjoint.clone(),
            Expr::UnOp {
                op: UnOp::Sign,
                operand: Box::new(operand.clone()),
            },
        ),
        // d(exp2(x))/dx = exp2(x) * ln(2)
        UnOp::Exp2 => Expr::mul(
            Expr::mul(
                adjoint.clone(),
                Expr::UnOp {
                    op: UnOp::Exp2,
                    operand: Box::new(operand.clone()),
                },
            ),
            Expr::f32(core::f32::consts::LN_2),
        ),
        // d(log2(x))/dx = 1 / (x * ln(2))
        UnOp::Log2 => Expr::div(
            adjoint.clone(),
            Expr::mul(operand.clone(), Expr::f32(core::f32::consts::LN_2)),
        ),
        // d(tan(x))/dx = 1 + tan²(x) = sec²(x)
        UnOp::Tan => {
            let t = Expr::UnOp {
                op: UnOp::Tan,
                operand: Box::new(operand.clone()),
            };
            Expr::mul(
                adjoint.clone(),
                Expr::add(Expr::f32(1.0), Expr::mul(t.clone(), t)),
            )
        }
        // d(sinh(x))/dx = cosh(x)
        UnOp::Sinh => Expr::mul(
            adjoint.clone(),
            Expr::UnOp {
                op: UnOp::Cosh,
                operand: Box::new(operand.clone()),
            },
        ),
        // d(cosh(x))/dx = sinh(x)
        UnOp::Cosh => Expr::mul(
            adjoint.clone(),
            Expr::UnOp {
                op: UnOp::Sinh,
                operand: Box::new(operand.clone()),
            },
        ),
        // d(asin(x))/dx = 1/sqrt(1-x²)
        UnOp::Asin => Expr::div(
            adjoint.clone(),
            Expr::UnOp {
                op: UnOp::Sqrt,
                operand: Box::new(Expr::sub(
                    Expr::f32(1.0),
                    Expr::mul(operand.clone(), operand.clone()),
                )),
            },
        ),
        // d(acos(x))/dx = -1/sqrt(1-x²)
        UnOp::Acos => Expr::UnOp {
            op: UnOp::Negate,
            operand: Box::new(Expr::div(
                adjoint.clone(),
                Expr::UnOp {
                    op: UnOp::Sqrt,
                    operand: Box::new(Expr::sub(
                        Expr::f32(1.0),
                        Expr::mul(operand.clone(), operand.clone()),
                    )),
                },
            )),
        },
        // d(atan(x))/dx = 1/(1+x²)
        UnOp::Atan => Expr::div(
            adjoint.clone(),
            Expr::add(Expr::f32(1.0), Expr::mul(operand.clone(), operand.clone())),
        ),
        // d(1/sqrt(x))/dx = -1/(2 * x^(3/2))
        UnOp::InverseSqrt => {
            let sqrt_x = Expr::UnOp {
                op: UnOp::Sqrt,
                operand: Box::new(operand.clone()),
            };
            Expr::UnOp {
                op: UnOp::Negate,
                operand: Box::new(Expr::div(
                    adjoint.clone(),
                    Expr::mul(Expr::f32(2.0), Expr::mul(operand.clone(), sqrt_x)),
                )),
            }
        }
        // d(1/x)/dx = -1/x²
        UnOp::Reciprocal => Expr::UnOp {
            op: UnOp::Negate,
            operand: Box::new(Expr::div(
                adjoint.clone(),
                Expr::mul(operand.clone(), operand.clone()),
            )),
        },
        // Non-differentiable
        UnOp::BitNot
        | UnOp::LogicalNot
        | UnOp::Popcount
        | UnOp::Clz
        | UnOp::Ctz
        | UnOp::ReverseBits
        | UnOp::Floor
        | UnOp::Ceil
        | UnOp::Round
        | UnOp::Trunc
        | UnOp::Sign
        | UnOp::IsNan
        | UnOp::IsInf
        | UnOp::IsFinite
        | UnOp::Unpack4Low
        | UnOp::Unpack4High
        | UnOp::Unpack8Low
        | UnOp::Unpack8High
        | UnOp::Opaque(_)
        | _ => {
            return Err(AutodiffError::NotDifferentiable {
                op: format!("UnOp::{op:?}"),
                fix: "replace with a differentiable equivalent or gate behind a stop-gradient barrier".into(),
            });
        }
    };
    Ok(AdjointContrib {
        child: operand.clone(),
        adjoint: dx,
    })
}

/// Adjoint for `Expr::Fma { a, b, c }` = `a * b + c`.
///
/// `d/da = b * dz`, `d/db = a * dz`, `d/dc = dz`.
#[must_use]
pub fn fma_adjoints(a: &Expr, b: &Expr, c: &Expr, adjoint: &Expr) -> Vec<AdjointContrib> {
    vec![
        AdjointContrib {
            child: a.clone(),
            adjoint: Expr::mul(adjoint.clone(), b.clone()),
        },
        AdjointContrib {
            child: b.clone(),
            adjoint: Expr::mul(adjoint.clone(), a.clone()),
        },
        AdjointContrib {
            child: c.clone(),
            adjoint: adjoint.clone(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BinOp, Expr, UnOp};

    #[test]
    fn test_binop_add_adjoint() {
        let l = Expr::var("l");
        let r = Expr::var("r");
        let adj = Expr::var("adj");
        let contribs = binop_adjoints(BinOp::Add, &l, &r, &adj).unwrap();
        assert_eq!(contribs.len(), 2);
        assert_eq!(contribs[0].child, l);
        assert_eq!(contribs[0].adjoint, adj);
        assert_eq!(contribs[1].child, r);
        assert_eq!(contribs[1].adjoint, adj);
    }

    #[test]
    fn test_binop_mul_adjoint() {
        let l = Expr::var("l");
        let r = Expr::var("r");
        let adj = Expr::var("adj");
        let contribs = binop_adjoints(BinOp::Mul, &l, &r, &adj).unwrap();
        assert_eq!(contribs.len(), 2);
        assert_eq!(contribs[0].adjoint, Expr::mul(adj.clone(), r));
        assert_eq!(contribs[1].adjoint, Expr::mul(adj, l));
    }

    #[test]
    fn test_binop_not_differentiable() {
        let l = Expr::var("l");
        let r = Expr::var("r");
        let adj = Expr::var("adj");
        let err =
            binop_adjoints(BinOp::BitAnd, &l, &r, &adj).expect_err("BitAnd is not differentiable");
        assert!(
            matches!(err, AutodiffError::NotDifferentiable { ref op, .. } if op == "BinOp::BitAnd"),
            "non-differentiable binop error: {err:?}"
        );
    }

    #[test]
    fn test_unop_negate_adjoint() {
        let op = Expr::var("op");
        let adj = Expr::var("adj");
        let contrib = unop_adjoint(&UnOp::Negate, &op, &adj).unwrap();
        assert_eq!(contrib.child, op);
        assert!(matches!(
            contrib.adjoint,
            Expr::UnOp {
                op: UnOp::Negate,
                ..
            }
        ));
    }

    #[test]
    fn test_unop_exp_adjoint() {
        let op = Expr::var("op");
        let adj = Expr::var("adj");
        let contrib = unop_adjoint(&UnOp::Exp, &op, &adj).unwrap();
        assert!(matches!(
            contrib.adjoint,
            Expr::BinOp { op: BinOp::Mul, .. }
        ));
    }

    #[test]
    fn test_unop_not_differentiable() {
        let op = Expr::var("op");
        let adj = Expr::var("adj");
        let err = unop_adjoint(&UnOp::Floor, &op, &adj).expect_err("Floor is not differentiable");
        assert!(
            matches!(err, AutodiffError::NotDifferentiable { .. }),
            "non-differentiable unop error: {err:?}"
        );
    }

    #[test]
    fn test_fma_adjoint() {
        let a = Expr::var("a");
        let b = Expr::var("b");
        let c = Expr::var("c");
        let adj = Expr::var("adj");
        let contribs = fma_adjoints(&a, &b, &c, &adj);
        assert_eq!(contribs.len(), 3);
        assert_eq!(contribs[0].adjoint, Expr::mul(adj.clone(), b));
        assert_eq!(contribs[1].adjoint, Expr::mul(adj.clone(), a));
        assert_eq!(contribs[2].adjoint, adj);
    }
}

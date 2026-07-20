// Cast folding rules.
//
// Fold a Cast expression when the inner value is a compile-time literal.

use crate::ir::eval::fold_cast_literal;
use crate::ir::Expr;

/// Fold a Cast expression when the inner value is a compile-time literal.
pub(super) fn fold_cast(target: &crate::ir::DataType, value: &Expr) -> Option<Expr> {
    if let Some(folded) = fold_cast_literal(target, value) {
        return Some(folded);
    }
    match value {
        Expr::Cast {
            target: inner_target,
            value: inner,
        } if inner_target == target => Some(Expr::Cast {
            target: target.clone(),
            value: inner.clone(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::fold_cast;
    use crate::ir::{DataType, Expr};

    #[test]
    fn nested_cast_to_same_target_elides_redundant_inner_cast() {
        let expr = Expr::Cast {
            target: DataType::U32,
            value: Box::new(Expr::var("x")),
        };

        assert_eq!(
            fold_cast(&DataType::U32, &expr),
            Some(Expr::Cast {
                target: DataType::U32,
                value: Box::new(Expr::var("x")),
            })
        );
    }

    #[test]
    fn nested_cast_through_bool_preserves_canonicalization_boundary() {
        let expr = Expr::Cast {
            target: DataType::Bool,
            value: Box::new(Expr::var("x")),
        };

        assert_eq!(
            fold_cast(&DataType::U32, &expr),
            None,
            "Cast(U32, Cast(Bool, x)) must not become Cast(U32, x); the Bool cast canonicalizes arbitrary non-zero values to 1."
        );
    }
}

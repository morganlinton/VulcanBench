use crate::ir::Expr;

#[inline]
pub(crate) fn const_loop_empty(from: &Expr, to: &Expr) -> bool {
    match (from, to) {
        (Expr::LitU32(from), Expr::LitU32(to)) => from >= to,
        (Expr::LitI32(from), Expr::LitI32(to)) => from >= to,
        _ => false,
    }
}

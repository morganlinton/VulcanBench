use crate::ir::Expr;

#[inline]
pub(crate) fn const_truth(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::LitBool(value) => Some(*value),
        Expr::LitU32(value) => Some(*value != 0),
        Expr::LitI32(value) => Some(*value != 0),
        _ => None,
    }
}

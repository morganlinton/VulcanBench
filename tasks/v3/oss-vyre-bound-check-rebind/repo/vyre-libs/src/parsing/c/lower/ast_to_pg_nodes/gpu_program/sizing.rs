use super::*;

pub(super) fn infer_node_count_words(node_count: &Expr) -> u32 {
    match node_count {
        Expr::LitU32(n) => *n,
        _ => 1,
    }
}

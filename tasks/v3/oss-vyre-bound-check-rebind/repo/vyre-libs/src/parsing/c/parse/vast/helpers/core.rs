use vyre::ir::Expr;

pub(crate) fn node_count(num_tokens: &Expr) -> u32 {
    match num_tokens {
        Expr::LitU32(n) => *n,
        _ => 1,
    }
}

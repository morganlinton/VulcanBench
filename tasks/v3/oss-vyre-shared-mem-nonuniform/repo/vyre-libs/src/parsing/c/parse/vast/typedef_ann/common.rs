use super::*;

pub(super) fn typedef_symbol_bucket(hash: Expr, buckets: u32) -> Expr {
    let mixed = Expr::bitxor(hash.clone(), Expr::shr(hash, Expr::u32(16)));
    Expr::bitand(mixed, Expr::u32(buckets - 1))
}

pub(super) fn haystack_word_count(haystack_len: &Expr, packed_haystack: bool) -> u32 {
    match haystack_len {
        Expr::LitU32(n) => source_haystack_words(*n, packed_haystack),
        _ => 1,
    }
}

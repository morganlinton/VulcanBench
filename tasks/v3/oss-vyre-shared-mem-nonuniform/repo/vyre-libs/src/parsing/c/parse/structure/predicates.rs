use super::*;

pub(super) fn expr_is_any(token: Expr, candidates: &[u32]) -> Expr {
    let ranges = merged_token_ranges(candidates);
    let mut iter = ranges.into_iter();
    let Some((first_lo, first_hi)) = iter.next() else {
        return Expr::u32(0);
    };
    iter.fold(
        token_range_expr(&token, first_lo, first_hi),
        |acc, (lo, hi)| Expr::or(acc, token_range_expr(&token, lo, hi)),
    )
}

pub(super) fn token_range_expr(token: &Expr, lo: u32, hi: u32) -> Expr {
    if lo == hi {
        Expr::eq(token.clone(), Expr::u32(lo))
    } else {
        Expr::and(
            Expr::ge(token.clone(), Expr::u32(lo)),
            Expr::le(token.clone(), Expr::u32(hi)),
        )
    }
}

pub(super) fn merged_token_ranges(candidates: &[u32]) -> Vec<(u32, u32)> {
    let mut values = candidates.to_vec();
    values.sort_unstable();
    values.dedup();

    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for value in values {
        match ranges.last_mut() {
            Some((_, hi)) if hi.checked_add(1) == Some(value) => *hi = value,
            _ => ranges.push((value, value)),
        }
    }
    ranges
}

pub(super) fn function_prefix_token(token: Expr) -> Expr {
    expr_is_any(
        token,
        &[
            TOK_AUTO,
            TOK_ATOMIC,
            TOK_BOOL,
            TOK_CHAR_KW,
            TOK_COMPLEX,
            TOK_CONST,
            TOK_DOUBLE,
            TOK_ENUM,
            TOK_EXTERN,
            TOK_FLOAT_KW,
            TOK_GNU_TYPEOF,
            TOK_GNU_TYPEOF_UNQUAL,
            TOK_IDENTIFIER,
            TOK_IMAGINARY,
            TOK_INLINE,
            TOK_INT,
            TOK_GNU_INT128,
            TOK_LONG,
            TOK_REGISTER,
            TOK_RESTRICT,
            TOK_SHORT,
            TOK_SIGNED,
            TOK_STATIC,
            TOK_STAR,
            TOK_STRUCT,
            TOK_THREAD_LOCAL,
            TOK_TYPEDEF,
            TOK_UNION,
            TOK_UNSIGNED,
            TOK_VOID,
            TOK_VOLATILE,
            // C23 / TS 18661-2 scalar types and clang/GCC half-precision
            // spellings. Mirror of structure_tokens.rs::function_prefix_token.
            TOK_BITINT_KW,
            TOK_FLOAT16_KW,
            TOK_FLOAT32_KW,
            TOK_FLOAT64_KW,
            TOK_FLOAT128_KW,
            TOK_GNU_FLOAT128_KW,
            TOK_GNU_BF16_KW,
            TOK_GNU_FP16_KW,
            TOK_DECIMAL32_KW,
            TOK_DECIMAL64_KW,
            TOK_DECIMAL128_KW,
            TOK_FORCEINLINE_KW,
        ],
    )
}

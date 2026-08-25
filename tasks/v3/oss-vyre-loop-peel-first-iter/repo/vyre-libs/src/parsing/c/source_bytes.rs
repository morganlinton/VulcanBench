use vyre::ir::Expr;

/// Return an expression that loads one source byte from either resident-expanded
/// or packed-byte haystack storage.
#[must_use]
pub fn load_source_byte(haystack: &str, byte_index: Expr, packed_haystack: bool) -> Expr {
    if packed_haystack {
        Expr::bitand(
            Expr::shr(
                Expr::load(haystack, Expr::shr(byte_index.clone(), Expr::u32(2))),
                Expr::shl(Expr::bitand(byte_index, Expr::u32(3)), Expr::u32(3)),
            ),
            Expr::u32(0xff),
        )
    } else {
        Expr::load(haystack, byte_index)
    }
}

/// Number of `u32` storage elements required for a haystack of `source_len`
/// logical source bytes.
#[must_use]
pub fn source_haystack_words(source_len: u32, packed_haystack: bool) -> u32 {
    if packed_haystack {
        source_len.max(1).div_ceil(4).max(1)
    } else {
        source_len.max(1)
    }
}

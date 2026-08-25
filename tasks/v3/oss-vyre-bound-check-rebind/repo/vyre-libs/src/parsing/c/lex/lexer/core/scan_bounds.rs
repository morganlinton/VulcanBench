use super::*;

/// Per-token scan loop bound. Each `scan_*` loop walks bytes from
/// the current position looking for the token's terminator (newline,
/// closing quote, end of identifier, etc.). The previous bound was
/// `Expr::buf_len("haystack")`  -  meaning the GPU loop counter ran
/// 524288 iterations per token on a 524288-byte preprocessed TU even
/// when the token was 4 bytes long. Combined with the fact that the
/// outer `token_iter` is a single-thread (`InvocationId == 0`) walk
/// over n_tokens, this gave O(n_tokens * haystack_len) loop trip
/// count on the single GPU thread that runs the lexer body  -  ~157B
/// iterations / ~20s wall clock on a typical glibc-bearing TU.
///
/// All real C tokens fit far below this cap; the only host-visible
/// effect of the cap is that a single token cannot span more than
/// 8 KiB without being split  -  well above the practical limit for
/// preprocessor lines, string literals, identifiers, and numeric
/// literals in real source. Block comments could exceed this in
/// pathological cases, but the post-preprocess byte stream the
/// lexer sees has comments already stripped by `gpu_filter_source_bytes`,
/// so `scan_comment` / `scan_block_comment` never fire on production
/// input  -  the cap there is purely a defense-in-depth bound.
///
/// The dense lexer body runs as a single thread (`InvocationId == 0`)
/// over n_tokens. The previous bound, `Expr::buf_len(\"haystack\")`,
/// made the GPU loop counter run `haystack_len` iterations per token
/// even when the actual ident / number / literal was only a few
/// bytes long. Combined with the single-thread outer loop the total
/// iteration count scaled as O(n_tokens * haystack_len)  -  ~157B
/// iterations / ~20s wall clock per file on glibc-bearing TUs.
/// Per-loop bounds. Identifier and numeric literal scans dominate
/// the per-token loop iteration count (most tokens in real C are
/// idents or numbers, both < 64 chars), so a tight 256-byte bound
/// captures the savings without affecting correctness. String
/// literals and comments can legitimately be longer (multiline
/// docblocks, license headers, embedded SQL); 65 536 matches
/// `MAX_SPARSE_TOKEN_SCAN` and is the practical upper limit on a
/// single token.
pub(crate) const MAX_IDENT_SCAN: u32 = 256;

pub(crate) const MAX_NUMBER_SCAN: u32 = 256;

pub(crate) const MAX_LITERAL_SCAN: u32 = 4_096;

pub(crate) const MAX_PREPROC_SCAN: u32 = 8_192;

pub(crate) const MAX_COMMENT_SCAN: u32 = 8_192;

pub(crate) const MAX_BLOCK_COMMENT_SCAN: u32 = 16_384;

/// Compute a per-thread upper bound for a `scan_*` loop:
/// `min(start + cap, buf_len(haystack))`.
pub(crate) fn scan_upper_bound_with_cap(haystack: &str, start: Expr, cap: u32) -> Expr {
    Expr::min(Expr::add(start, Expr::u32(cap)), Expr::buf_len(haystack))
}

use crate::parsing::c::lex::tokens::*;
use crate::parsing::c::source_bytes::{load_source_byte, source_haystack_words};
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
pub use vyre_primitives::hash::fnv1a::fnv1a32;

/// C11 keyword table consumed by the GPU keyword promotion pass.
pub const C_KEYWORDS: &[(&str, u32)] = &[
    ("auto", TOK_AUTO),
    ("break", TOK_BREAK),
    ("case", TOK_CASE),
    ("char", TOK_CHAR_KW),
    ("const", TOK_CONST),
    ("__const", TOK_CONST),
    ("__const__", TOK_CONST),
    ("continue", TOK_CONTINUE),
    ("default", TOK_DEFAULT),
    ("do", TOK_DO),
    ("double", TOK_DOUBLE),
    ("else", TOK_ELSE),
    ("enum", TOK_ENUM),
    ("extern", TOK_EXTERN),
    ("float", TOK_FLOAT_KW),
    ("for", TOK_FOR),
    ("goto", TOK_GOTO),
    ("if", TOK_IF),
    ("inline", TOK_INLINE),
    ("int", TOK_INT),
    ("long", TOK_LONG),
    ("register", TOK_REGISTER),
    ("restrict", TOK_RESTRICT),
    ("__restrict", TOK_RESTRICT),
    ("__restrict__", TOK_RESTRICT),
    ("return", TOK_RETURN),
    ("short", TOK_SHORT),
    ("signed", TOK_SIGNED),
    ("__signed", TOK_SIGNED),
    ("__signed__", TOK_SIGNED),
    ("sizeof", TOK_SIZEOF),
    ("static", TOK_STATIC),
    ("struct", TOK_STRUCT),
    ("switch", TOK_SWITCH),
    ("typedef", TOK_TYPEDEF),
    ("union", TOK_UNION),
    ("unsigned", TOK_UNSIGNED),
    ("void", TOK_VOID),
    ("volatile", TOK_VOLATILE),
    ("__volatile", TOK_VOLATILE),
    ("while", TOK_WHILE),
    ("_Alignas", TOK_ALIGNAS),
    ("_Alignof", TOK_ALIGNOF),
    ("_Atomic", TOK_ATOMIC),
    ("_Bool", TOK_BOOL),
    ("_Complex", TOK_COMPLEX),
    ("_Generic", TOK_GENERIC),
    ("_Imaginary", TOK_IMAGINARY),
    ("_Noreturn", TOK_NORETURN),
    ("_Static_assert", TOK_STATIC_ASSERT),
    ("_Thread_local", TOK_THREAD_LOCAL),
    ("__thread", TOK_THREAD_LOCAL),
    ("asm", TOK_GNU_ASM),
    ("__asm", TOK_GNU_ASM),
    ("__asm__", TOK_GNU_ASM),
    ("__attribute", TOK_GNU_ATTRIBUTE),
    ("__attribute__", TOK_GNU_ATTRIBUTE),
    ("typeof", TOK_GNU_TYPEOF),
    ("__typeof", TOK_GNU_TYPEOF),
    ("__typeof__", TOK_GNU_TYPEOF),
    ("typeof_unqual", TOK_GNU_TYPEOF_UNQUAL),
    ("__typeof_unqual", TOK_GNU_TYPEOF_UNQUAL),
    ("__typeof_unqual__", TOK_GNU_TYPEOF_UNQUAL),
    ("__extension__", TOK_GNU_EXTENSION),
    ("__alignof", TOK_ALIGNOF),
    ("__alignof__", TOK_ALIGNOF),
    ("__inline", TOK_INLINE),
    ("__inline__", TOK_INLINE),
    ("__complex__", TOK_COMPLEX),
    ("__real__", TOK_GNU_REAL),
    ("__imag__", TOK_GNU_IMAG),
    ("__volatile__", TOK_VOLATILE),
    ("__builtin_constant_p", TOK_BUILTIN_CONSTANT_P),
    ("__builtin_choose_expr", TOK_BUILTIN_CHOOSE_EXPR),
    (
        "__builtin_types_compatible_p",
        TOK_BUILTIN_TYPES_COMPATIBLE_P,
    ),
    ("__auto_type", TOK_GNU_AUTO_TYPE),
    ("__int128", TOK_GNU_INT128),
    ("__int128_t", TOK_GNU_INT128),
    ("__uint128_t", TOK_GNU_INT128),
    ("__builtin_va_list", TOK_GNU_BUILTIN_VA_LIST),
    ("__seg_gs", TOK_GNU_ADDRESS_SPACE),
    ("__seg_fs", TOK_GNU_ADDRESS_SPACE),
    ("__label__", TOK_GNU_LABEL),
    // C23 + TS-extension scalar type keywords.
    ("_BitInt", TOK_BITINT_KW),
    ("_Float16", TOK_FLOAT16_KW),
    ("_Float32", TOK_FLOAT32_KW),
    ("_Float32x", TOK_FLOAT32_KW),
    ("_Float64", TOK_FLOAT64_KW),
    ("_Float64x", TOK_FLOAT64_KW),
    ("_Float128", TOK_FLOAT128_KW),
    ("_Float128x", TOK_FLOAT128_KW),
    ("__float128", TOK_GNU_FLOAT128_KW),
    ("__bf16", TOK_GNU_BF16_KW),
    ("__fp16", TOK_GNU_FP16_KW),
    ("_Decimal32", TOK_DECIMAL32_KW),
    ("_Decimal64", TOK_DECIMAL64_KW),
    ("_Decimal128", TOK_DECIMAL128_KW),
    ("__forceinline", TOK_FORCEINLINE_KW),
    // clang nullability qualifiers  -  folded onto one token id; the
    // identifier text disambiguates downstream.
    ("_Nonnull", TOK_NULLABILITY_KW),
    ("_Nullable", TOK_NULLABILITY_KW),
    ("_Nullable_result", TOK_NULLABILITY_KW),
    ("_Null_unspecified", TOK_NULLABILITY_KW),
];

/// Packed `[hash, token_id]` table for the GPU keyword pass.
#[must_use]
pub fn c_keyword_map_words() -> Vec<u32> {
    C_KEYWORDS
        .iter()
        .flat_map(|(keyword, token)| [fnv1a32(keyword.as_bytes()), *token])
        .collect()
}

/// Explicit CPU oracle for keyword promotion over extracted token streams.
#[must_use]
#[deprecated(
    note = "CPU oracle only; production C keyword promotion must dispatch the GPU keyword pass"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_c_keyword_types(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    haystack: &[u8],
) -> Vec<u32> {
    assert_eq!(
        tok_starts.len(),
        tok_types.len(),
        "vyre-libs C keyword CPU oracle received {} token starts for {} token types. Fix: pass one start per token.",
        tok_starts.len(),
        tok_types.len()
    );
    assert_eq!(
        tok_lens.len(),
        tok_types.len(),
        "vyre-libs C keyword CPU oracle received {} token lengths for {} token types. Fix: pass one length per token.",
        tok_lens.len(),
        tok_types.len()
    );
    let mut out = tok_types.to_vec();
    for (idx, tok_type) in out.iter_mut().enumerate() {
        if *tok_type != TOK_IDENTIFIER {
            continue;
        }
        let start = tok_starts[idx] as usize;
        let len = tok_lens[idx] as usize;
        let end = start.checked_add(len).unwrap_or_else(|| {
            panic!("vyre-libs C keyword CPU oracle token {idx} span overflows usize: start={start}, len={len}. Fix: validate lexer span emission.")
        });
        let lexeme = haystack.get(start..end).unwrap_or_else(|| {
            panic!(
                "vyre-libs C keyword CPU oracle token {idx} span is outside haystack: start={start}, end={end}, haystack_len={}. Fix: validate lexer span emission.",
                haystack.len()
            )
        });
        if let Some((_, keyword_token)) = C_KEYWORDS
            .iter()
            .find(|(keyword, _)| keyword.as_bytes() == lexeme)
        {
            *tok_type = *keyword_token;
        }
    }
    out
}

/// GPU keyword reclassification pass
///
/// Runs sequentially or in parallel over the extracted token stream (`out_tokens`).
/// For every `TOK_IDENTIFIER` (type == 1), hashes its bytes via FNV-1a32 and checks
/// a keyword hash table. If a match is found, the token type is updated.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn c_keyword(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    counts: &str,
    haystack: &str,
    keyword_map: &str,
    max_tokens: u32,
    num_keywords: u32,
    haystack_len: u32,
) -> Program {
    c_keyword_impl(
        tok_types,
        tok_starts,
        tok_lens,
        counts,
        haystack,
        keyword_map,
        max_tokens,
        num_keywords,
        haystack_len,
        false,
        "vyre-libs::parsing::c_keyword",
    )
}

/// GPU keyword reclassification pass over a packed-byte source haystack.
///
/// This is the CUDA megakernel companion to [`c_keyword`]. Token starts/lens
/// remain byte offsets, but the source buffer is resident as packed `u32`
/// words containing four source bytes per element.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn c_keyword_packed_haystack(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    counts: &str,
    haystack: &str,
    keyword_map: &str,
    max_tokens: u32,
    num_keywords: u32,
    haystack_len: u32,
) -> Program {
    c_keyword_impl(
        tok_types,
        tok_starts,
        tok_lens,
        counts,
        haystack,
        keyword_map,
        max_tokens,
        num_keywords,
        haystack_len,
        true,
        "vyre-libs::parsing::c_keyword_packed_haystack",
    )
}

#[allow(clippy::too_many_arguments)]
fn c_keyword_impl(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    counts: &str,
    haystack: &str,
    keyword_map: &str,
    max_tokens: u32,
    num_keywords: u32,
    haystack_len: u32,
    packed_haystack: bool,
    entry_op_id: &'static str,
) -> Program {
    let t = Expr::var("t");
    let num_tokens = Expr::load(counts, Expr::u32(0));

    let loop_body = vec![
        Node::let_bind("tok_type", Expr::load(tok_types, t.clone())),
        Node::store(tok_types, t.clone(), Expr::var("tok_type")),
        Node::if_then(
            Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
            vec![
                Node::let_bind("start", Expr::load(tok_starts, t.clone())),
                Node::let_bind("len", Expr::load(tok_lens, t.clone())),
                // inline fnv1a32
                Node::let_bind("hash", Expr::u32(0x811c9dc5)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::var("len"),
                    vec![
                        Node::let_bind(
                            "byte",
                            load_source_byte(
                                haystack,
                                Expr::add(Expr::var("start"), Expr::var("i")),
                                packed_haystack,
                            ),
                        ),
                        Node::assign("hash", Expr::bitxor(Expr::var("hash"), Expr::var("byte"))),
                        Node::assign("hash", Expr::mul(Expr::var("hash"), Expr::u32(0x01000193))),
                        // Node::assign("i", Expr::add(Expr::var("i"), Expr::u32(1))), // loop_for auto-increments
                    ],
                ),
                // keyword_map is [hash0, tok_id0, hash1, tok_id1, ...].
                // `done_kw` is the soft-break flag  -  once a keyword
                // match fires, subsequent iterations are no-ops.
                Node::let_bind("done_kw", Expr::u32(0)),
                Node::loop_for(
                    "k",
                    Expr::u32(0),
                    Expr::u32(num_keywords),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("done_kw"), Expr::u32(0)),
                        vec![
                            Node::let_bind(
                                "kw_hash",
                                Expr::load(keyword_map, Expr::mul(Expr::var("k"), Expr::u32(2))),
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("kw_hash"), Expr::var("hash")),
                                vec![
                                    Node::store(
                                        tok_types,
                                        t.clone(),
                                        Expr::load(
                                            keyword_map,
                                            Expr::add(
                                                Expr::mul(Expr::var("k"), Expr::u32(2)),
                                                Expr::u32(1),
                                            ),
                                        ),
                                    ),
                                    Node::assign("done_kw", Expr::u32(1)),
                                ],
                            ),
                        ],
                    )],
                ),
            ],
        ),
    ];

    let body = vec![
        Node::let_bind("lane", Expr::LocalId { axis: 0 }),
        Node::let_bind("block", Expr::WorkgroupId { axis: 0 }),
        Node::let_bind(
            "t",
            Expr::add(
                Expr::mul(Expr::var("block"), Expr::u32(256)),
                Expr::var("lane"),
            ),
        ),
        Node::if_then(Expr::lt(t.clone(), num_tokens), loop_body),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(max_tokens),
            BufferDecl::storage(tok_starts, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(max_tokens),
            BufferDecl::storage(tok_lens, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(max_tokens),
            BufferDecl::storage(counts, 3, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(haystack, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(source_haystack_words(haystack_len, packed_haystack)),
            BufferDecl::storage(keyword_map, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(num_keywords.saturating_mul(2)),
        ],
        [256, 1, 1], // Launch configuration
        vec![wrap_anonymous(entry_op_id, body)],
    )
    .with_entry_op_id(entry_op_id)
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c_keyword",
        build: || {
            c_keyword(
                "tok_types",
                "tok_starts",
                "tok_lens",
                "counts",
                "haystack",
                "keyword_map",
                1024,
                C_KEYWORDS.len() as u32,
                4096,
            )
        },
        test_inputs: Some(keyword_fixture_inputs),
        expected_output: Some(|| {
            let mut tok_types = vec![0u8; 1024 * 4];
            for (idx, tok) in [TOK_INT, TOK_IDENTIFIER, TOK_RETURN, TOK_GNU_ASM]
                .into_iter()
                .enumerate()
            {
                tok_types[idx * 4..idx * 4 + 4].copy_from_slice(&tok.to_le_bytes());
            }
            vec![vec![tok_types]]
        }),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c_keyword_packed_haystack",
        build: || {
            c_keyword_packed_haystack(
                "tok_types",
                "tok_starts",
                "tok_lens",
                "counts",
                "haystack",
                "keyword_map",
                1024,
                C_KEYWORDS.len() as u32,
                4096,
            )
        },
        test_inputs: Some(keyword_packed_fixture_inputs),
        expected_output: Some(|| {
            let mut tok_types = vec![0u8; 1024 * 4];
            for (idx, tok) in [TOK_INT, TOK_IDENTIFIER, TOK_RETURN, TOK_GNU_ASM]
                .into_iter()
                .enumerate()
            {
                tok_types[idx * 4..idx * 4 + 4].copy_from_slice(&tok.to_le_bytes());
            }
            vec![vec![tok_types]]
        }),
        category: Some("parsing"),
    }
}

fn keyword_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let lexemes = b"int foo return __asm__";
    let starts = [0u32, 4, 8, 15];
    let lens = [3u32, 3, 6, 7];
    let mut tok_types = vec![0u8; 1024 * 4];
    let mut tok_starts = vec![0u8; 1024 * 4];
    let mut tok_lens = vec![0u8; 1024 * 4];
    for idx in 0..starts.len() {
        tok_types[idx * 4..idx * 4 + 4].copy_from_slice(&TOK_IDENTIFIER.to_le_bytes());
        tok_starts[idx * 4..idx * 4 + 4].copy_from_slice(&starts[idx].to_le_bytes());
        tok_lens[idx * 4..idx * 4 + 4].copy_from_slice(&lens[idx].to_le_bytes());
    }

    let mut counts = vec![0u8; 4];
    counts.copy_from_slice(&(starts.len() as u32).to_le_bytes());

    let mut haystack = vec![0u8; 4_096 * 4];
    for (idx, byte) in lexemes.iter().enumerate() {
        haystack[idx * 4..idx * 4 + 4].copy_from_slice(&u32::from(*byte).to_le_bytes());
    }

    let mut keyword_map = vec![0u8; C_KEYWORDS.len() * 2 * 4];
    for (idx, (keyword, tok)) in C_KEYWORDS.iter().enumerate() {
        let hash = fnv1a32(keyword.as_bytes());
        let base = idx * 8;
        keyword_map[base..base + 4].copy_from_slice(&hash.to_le_bytes());
        keyword_map[base + 4..base + 8].copy_from_slice(&tok.to_le_bytes());
    }

    vec![vec![
        tok_types,
        tok_starts,
        tok_lens,
        counts,
        haystack,
        keyword_map,
    ]]
}

fn keyword_packed_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let mut inputs = keyword_fixture_inputs();
    let Some(first_case) = inputs.first_mut() else {
        return inputs;
    };
    let lexemes = b"int foo return __asm__";
    let mut packed_haystack = vec![0u8; 4096];
    packed_haystack[..lexemes.len()].copy_from_slice(lexemes);
    first_case[4] = packed_haystack;
    inputs
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)]

    use super::*;

    #[test]
    #[should_panic(expected = "one start per token")]
    fn reference_keyword_oracle_rejects_short_start_stream() {
        let _ = reference_c_keyword_types(&[TOK_IDENTIFIER], &[], &[3], b"int");
    }

    #[test]
    #[should_panic(expected = "outside haystack")]
    fn reference_keyword_oracle_rejects_out_of_bounds_span() {
        let _ = reference_c_keyword_types(&[TOK_IDENTIFIER], &[10], &[3], b"int");
    }

    #[test]
    fn reference_keyword_oracle_promotes_valid_identifier() {
        let out = reference_c_keyword_types(&[TOK_IDENTIFIER], &[0], &[3], b"int");
        assert_eq!(out, [TOK_INT]);
    }
}

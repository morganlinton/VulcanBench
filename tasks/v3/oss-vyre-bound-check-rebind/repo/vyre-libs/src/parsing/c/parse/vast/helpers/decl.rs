use super::super::*;
use super::*;
use crate::parsing::c::lex::tokens::*;
use vyre::ir::Expr;

pub(crate) fn is_type_name_start_token(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_CONST,
            TOK_RESTRICT,
            TOK_VOLATILE,
            TOK_STRUCT,
            TOK_UNION,
            TOK_ENUM,
            TOK_VOID,
            TOK_CHAR_KW,
            TOK_INT,
            TOK_LONG,
            TOK_SHORT,
            TOK_SIGNED,
            TOK_UNSIGNED,
            TOK_FLOAT_KW,
            TOK_DOUBLE,
            TOK_BOOL,
            TOK_COMPLEX,
            TOK_IMAGINARY,
            TOK_ATOMIC,
            TOK_GNU_TYPEOF,
            TOK_GNU_TYPEOF_UNQUAL,
            TOK_GNU_INT128,
            TOK_GNU_BUILTIN_VA_LIST,
            // C23 / TS 18661-2 scalar types and clang/GCC half-precision
            // spellings  -  every keyword that can begin a type-name must
            // be in this list, otherwise typeof / sizeof / casts using
            // these scalars fail the type-name predicate.
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
        ],
    )
}

pub(crate) fn is_decl_prefix_token(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_TYPEDEF,
            TOK_EXTERN,
            TOK_STATIC,
            TOK_INLINE,
            TOK_CONST,
            TOK_RESTRICT,
            TOK_VOLATILE,
            TOK_STRUCT,
            TOK_UNION,
            TOK_ENUM,
            TOK_VOID,
            TOK_CHAR_KW,
            TOK_INT,
            TOK_LONG,
            TOK_SHORT,
            TOK_SIGNED,
            TOK_UNSIGNED,
            TOK_FLOAT_KW,
            TOK_DOUBLE,
            TOK_BOOL,
            TOK_COMPLEX,
            TOK_IMAGINARY,
            TOK_ALIGNAS,
            TOK_ATOMIC,
            TOK_NORETURN,
            TOK_STATIC_ASSERT,
            TOK_THREAD_LOCAL,
            TOK_GNU_TYPEOF,
            TOK_GNU_TYPEOF_UNQUAL,
            TOK_GNU_AUTO_TYPE,
            TOK_GNU_INT128,
            TOK_GNU_BUILTIN_VA_LIST,
            TOK_GNU_ADDRESS_SPACE,
            TOK_GNU_EXTENSION,
            // C23 / TS 18661-2 scalar types and clang/GCC half-precision.
            // A `_BitInt(N)` declaration (used in real C code by SQLite,
            // Postgres, and any sub-word-aligned protocol struct) must be
            // recognised here as starting a decl prefix.
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

pub(crate) fn is_decl_prefix_token_or_gnu_type_hash(token: Expr, symbol_hash: Expr) -> Expr {
    Expr::or(
        is_decl_prefix_token(token.clone()),
        Expr::or(
            is_typeof_operator_token(token.clone(), symbol_hash.clone()),
            Expr::and(
                Expr::eq(token, Expr::u32(TOK_IDENTIFIER)),
                is_gnu_auto_type_symbol_hash(symbol_hash),
            ),
        ),
    )
}

pub(crate) fn is_decl_prefix_reset_token(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[TOK_SEMICOLON, TOK_LBRACE, TOK_RBRACE, TOK_ASSIGN, TOK_COLON],
    )
}

pub(crate) fn is_declarator_follower_token(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_SEMICOLON,
            TOK_COMMA,
            TOK_ASSIGN,
            TOK_LBRACKET,
            TOK_LPAREN,
            TOK_RPAREN,
            TOK_GNU_ATTRIBUTE,
        ],
    )
}

pub(crate) fn is_declaration_candidate_follower_token(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_SEMICOLON,
            TOK_COMMA,
            TOK_ASSIGN,
            TOK_LPAREN,
            TOK_LBRACKET,
            TOK_COLON,
            TOK_RPAREN,
            TOK_RBRACKET,
            TOK_GNU_ATTRIBUTE,
        ],
    )
}

pub(crate) fn is_declaration_previous_disqualifier_token(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[TOK_STRUCT, TOK_UNION, TOK_ENUM, TOK_DOT, TOK_ARROW],
    )
}

pub(crate) fn is_precomputed_declaration_previous_disqualifier_token(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_STRUCT, TOK_UNION, TOK_ENUM, TOK_DOT, TOK_ARROW, TOK_GOTO,
        ],
    )
}

pub(crate) fn is_typedef_name_annotation(flags: Expr) -> Expr {
    Expr::ne(
        Expr::bitand(flags, Expr::u32(C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME)),
        Expr::u32(0),
    )
}

pub(crate) fn is_typedef_declarator_annotation(flags: Expr) -> Expr {
    Expr::ne(
        Expr::bitand(flags, Expr::u32(C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR)),
        Expr::u32(0),
    )
}

pub(crate) fn is_ordinary_declarator_annotation(flags: Expr) -> Expr {
    Expr::ne(
        Expr::bitand(flags, Expr::u32(C_TYPEDEF_FLAG_ORDINARY_DECLARATOR)),
        Expr::u32(0),
    )
}

pub(crate) fn is_type_name_identifier(flags: Expr, fallback_has_prior_typedef: Expr) -> Expr {
    Expr::or(
        is_typedef_name_annotation(flags),
        fallback_has_prior_typedef,
    )
}

pub(crate) fn is_aggregate_specifier_body_open(
    open_kind: Expr,
    prev_kind: Expr,
    prev_prev_kind: Expr,
) -> Expr {
    Expr::and(
        Expr::eq(open_kind, Expr::u32(TOK_LBRACE)),
        Expr::or(
            any_token_eq(prev_kind.clone(), &[TOK_STRUCT, TOK_UNION, TOK_ENUM]),
            Expr::and(
                Expr::eq(prev_kind, Expr::u32(TOK_IDENTIFIER)),
                any_token_eq(prev_prev_kind, &[TOK_STRUCT, TOK_UNION, TOK_ENUM]),
            ),
        ),
    )
}

/// Returns true when a typedef symbol occurrence may participate in declaration linking.
pub(crate) fn is_typedef_symbol_link_follower_token(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_SEMICOLON,
            TOK_COMMA,
            TOK_ASSIGN,
            TOK_LPAREN,
            TOK_LBRACKET,
            TOK_COLON,
            TOK_RPAREN,
            TOK_RBRACKET,
        ],
    )
}

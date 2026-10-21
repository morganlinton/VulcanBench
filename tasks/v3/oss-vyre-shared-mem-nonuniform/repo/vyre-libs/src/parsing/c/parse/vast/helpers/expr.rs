use super::super::*;
use super::*;
use crate::parsing::c::lex::tokens::*;
use crate::parsing::c::parse::gnu_builtin_catalog::GNU_BUILTIN_NAME_KINDS;
use vyre::ir::Expr;

pub(crate) fn c_effective_expression_prev_kind(prev_kind: Expr, prev_prev_kind: Expr) -> Expr {
    let parenthesized_type_operand = Expr::and(
        Expr::eq(prev_kind.clone(), Expr::u32(TOK_LPAREN)),
        any_token_eq(
            prev_prev_kind,
            &[
                TOK_SIZEOF,
                TOK_ALIGNOF,
                TOK_GNU_TYPEOF,
                TOK_GNU_TYPEOF_UNQUAL,
            ],
        ),
    );
    Expr::select(parenthesized_type_operand, Expr::u32(TOK_RPAREN), prev_kind)
}

pub(crate) fn c_expression_operator_kind(
    token: Expr,
    prev_kind: Expr,
    prev_prev_kind: Expr,
) -> Expr {
    let effective_prev_kind = c_effective_expression_prev_kind(prev_kind, prev_prev_kind);
    let is_assignment_operator = any_token_eq(
        token.clone(),
        &[
            TOK_ASSIGN,
            TOK_PLUS_EQ,
            TOK_MINUS_EQ,
            TOK_STAR_EQ,
            TOK_SLASH_EQ,
            TOK_PERCENT_EQ,
            TOK_AMP_EQ,
            TOK_PIPE_EQ,
            TOK_CARET_EQ,
            TOK_LSHIFT_EQ,
            TOK_RSHIFT_EQ,
        ],
    );
    let unary_context = c_unary_context(effective_prev_kind.clone());
    let is_unary_operator = Expr::or(
        Expr::and(
            unary_context.clone(),
            Expr::or(
                Expr::eq(token.clone(), Expr::u32(TOK_INC)),
                Expr::eq(token.clone(), Expr::u32(TOK_DEC)),
            ),
        ),
        Expr::and(
            unary_context.clone(),
            any_token_eq(
                token.clone(),
                &[
                    TOK_STAR,
                    TOK_AMP,
                    TOK_PLUS,
                    TOK_MINUS,
                    TOK_BANG,
                    TOK_TILDE,
                    TOK_GNU_REAL,
                    TOK_GNU_IMAG,
                ],
            ),
        ),
    );
    let is_array_subscript = Expr::and(
        Expr::eq(token.clone(), Expr::u32(TOK_LBRACKET)),
        c_can_end_expression(effective_prev_kind.clone()),
    );

    Expr::select(
        is_assignment_operator,
        Expr::u32(C_AST_KIND_ASSIGN_EXPR),
        Expr::select(
            Expr::or(
                Expr::eq(token.clone(), Expr::u32(TOK_DOT)),
                Expr::eq(token.clone(), Expr::u32(TOK_ARROW)),
            ),
            Expr::u32(C_AST_KIND_MEMBER_ACCESS_EXPR),
            Expr::select(
                is_array_subscript,
                Expr::u32(C_AST_KIND_ARRAY_SUBSCRIPT_EXPR),
                Expr::select(
                    Expr::or(
                        Expr::eq(token.clone(), Expr::u32(TOK_SIZEOF)),
                        Expr::or(
                            Expr::eq(token.clone(), Expr::u32(TOK_GNU_TYPEOF)),
                            Expr::eq(token.clone(), Expr::u32(TOK_GNU_TYPEOF_UNQUAL)),
                        ),
                    ),
                    Expr::u32(C_AST_KIND_SIZEOF_EXPR),
                    Expr::select(
                        Expr::eq(token.clone(), Expr::u32(TOK_ALIGNOF)),
                        Expr::u32(C_AST_KIND_ALIGNOF_EXPR),
                        Expr::select(
                            Expr::eq(token.clone(), Expr::u32(TOK_QUESTION)),
                            Expr::u32(C_AST_KIND_CONDITIONAL_EXPR),
                            Expr::select(
                                is_unary_operator,
                                Expr::u32(C_AST_KIND_UNARY_EXPR),
                                Expr::select(
                                    Expr::and(
                                        Expr::not(unary_context),
                                        any_token_eq(
                                            token,
                                            &[
                                                TOK_PLUS,
                                                TOK_MINUS,
                                                TOK_STAR,
                                                TOK_SLASH,
                                                TOK_PERCENT,
                                                TOK_AMP,
                                                TOK_PIPE,
                                                TOK_CARET,
                                                TOK_EQ,
                                                TOK_NE,
                                                TOK_LE,
                                                TOK_GE,
                                                TOK_AND,
                                                TOK_OR,
                                                TOK_LSHIFT,
                                                TOK_RSHIFT,
                                                TOK_LT,
                                                TOK_GT,
                                            ],
                                        ),
                                    ),
                                    Expr::u32(node_kind::BINARY),
                                    Expr::u32(0),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    )
}

pub(crate) fn c_builtin_expression_kind(token: Expr) -> Expr {
    Expr::select(
        Expr::eq(token.clone(), Expr::u32(TOK_BUILTIN_CONSTANT_P)),
        Expr::u32(C_AST_KIND_BUILTIN_CONSTANT_P_EXPR),
        Expr::select(
            Expr::eq(token.clone(), Expr::u32(TOK_BUILTIN_CHOOSE_EXPR)),
            Expr::u32(C_AST_KIND_BUILTIN_CHOOSE_EXPR),
            Expr::select(
                Expr::eq(token.clone(), Expr::u32(TOK_BUILTIN_TYPES_COMPATIBLE_P)),
                Expr::u32(C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR),
                Expr::select(
                    Expr::eq(token.clone(), Expr::u32(TOK_GENERIC)),
                    Expr::u32(C_AST_KIND_GENERIC_SELECTION_EXPR),
                    Expr::select(
                        Expr::eq(token, Expr::u32(TOK_ELLIPSIS)),
                        Expr::u32(C_AST_KIND_RANGE_DESIGNATOR_EXPR),
                        Expr::u32(0),
                    ),
                ),
            ),
        ),
    )
}

pub(crate) fn c_builtin_identifier_expression_kind(
    raw_kind: Expr,
    symbol_hash: Expr,
    next_kind: Expr,
) -> Expr {
    let hash_kind = |hash: u32, kind: u32, fallback: Expr| {
        Expr::select(
            Expr::eq(symbol_hash.clone(), Expr::u32(hash)),
            Expr::u32(kind),
            fallback,
        )
    };
    let legacy_kind = hash_kind(
        0x749d_f71e,
        C_AST_KIND_BUILTIN_EXPECT_EXPR,
        hash_kind(
            0xdcec_13f5,
            C_AST_KIND_BUILTIN_OFFSETOF_EXPR,
            hash_kind(
                0x7900_03c8,
                C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR,
                hash_kind(
                    0x21a7_53f0,
                    C_AST_KIND_BUILTIN_PREFETCH_EXPR,
                    hash_kind(
                        0x4a9a_c967,
                        C_AST_KIND_BUILTIN_UNREACHABLE_STMT,
                        hash_kind(
                            0x7f55_6bd5,
                            C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
                            hash_kind(
                                0xb0bc_f282,
                                C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
                                hash_kind(
                                    0x8cc7_b276,
                                    C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
                                    hash_kind(
                                        0x3909_1622,
                                        C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR,
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    );
    Expr::select(
        Expr::and(
            Expr::eq(raw_kind, Expr::u32(TOK_IDENTIFIER)),
            Expr::eq(next_kind, Expr::u32(TOK_LPAREN)),
        ),
        Expr::select(
            is_gnu_typeof_symbol_hash(symbol_hash.clone()),
            Expr::u32(C_AST_KIND_SIZEOF_EXPR),
            c_gnu_builtin_catalog_kind_from_hash(symbol_hash.clone(), legacy_kind),
        ),
        Expr::u32(0),
    )
}

pub(crate) fn c_gnu_builtin_catalog_kind_from_hash(symbol_hash: Expr, fallback: Expr) -> Expr {
    let mut groups: Vec<(u32, Vec<u32>)> = Vec::new();
    for entry in GNU_BUILTIN_NAME_KINDS {
        match groups.iter_mut().find(|(kind, _)| *kind == entry.kind) {
            Some((_, hashes)) => hashes.push(entry.hash),
            None => groups.push((entry.kind, vec![entry.hash])),
        }
    }

    let mut out = fallback;
    for (kind, hashes) in groups.into_iter().rev() {
        out = Expr::select(
            any_hash_eq(symbol_hash.clone(), &hashes),
            Expr::u32(kind),
            out,
        );
    }
    out
}

fn any_hash_eq(hash: Expr, values: &[u32]) -> Expr {
    balanced_or(
        values
            .iter()
            .copied()
            .map(|value| Expr::eq(hash.clone(), Expr::u32(value)))
            .collect(),
    )
}

pub(crate) fn c_unary_context(prev_kind: Expr) -> Expr {
    Expr::or(
        Expr::eq(prev_kind.clone(), Expr::u32(SENTINEL)),
        any_token_eq(
            prev_kind,
            &[
                TOK_LPAREN,
                TOK_LBRACKET,
                TOK_LBRACE,
                TOK_SEMICOLON,
                TOK_COMMA,
                TOK_ASSIGN,
                TOK_PLUS_EQ,
                TOK_MINUS_EQ,
                TOK_STAR_EQ,
                TOK_SLASH_EQ,
                TOK_PERCENT_EQ,
                TOK_AMP_EQ,
                TOK_PIPE_EQ,
                TOK_CARET_EQ,
                TOK_LSHIFT_EQ,
                TOK_RSHIFT_EQ,
                TOK_QUESTION,
                TOK_COLON,
                TOK_RETURN,
                TOK_CASE,
                TOK_SIZEOF,
                TOK_GNU_TYPEOF,
                TOK_GNU_TYPEOF_UNQUAL,
                TOK_ALIGNOF,
                TOK_PLUS,
                TOK_MINUS,
                TOK_STAR,
                TOK_SLASH,
                TOK_PERCENT,
                TOK_AMP,
                TOK_PIPE,
                TOK_CARET,
                TOK_BANG,
                TOK_TILDE,
                TOK_EQ,
                TOK_NE,
                TOK_LE,
                TOK_GE,
                TOK_AND,
                TOK_OR,
                TOK_LSHIFT,
                TOK_RSHIFT,
                TOK_LT,
                TOK_GT,
            ],
        ),
    )
}

pub(crate) fn c_can_end_expression(prev_kind: Expr) -> Expr {
    Expr::or(
        Expr::or(
            Expr::eq(prev_kind.clone(), Expr::u32(TOK_IDENTIFIER)),
            is_c_literal_token(prev_kind.clone()),
        ),
        any_token_eq(prev_kind, &[TOK_RPAREN, TOK_RBRACKET, TOK_INC, TOK_DEC]),
    )
}

pub(crate) fn c_expr_shape_kind(raw_kind: Expr, typed_kind: Expr) -> Expr {
    Expr::select(
        Expr::eq(typed_kind.clone(), Expr::u32(C_AST_KIND_CONDITIONAL_EXPR)),
        Expr::u32(C_EXPR_SHAPE_CONDITIONAL),
        Expr::select(
            Expr::or(
                Expr::eq(typed_kind.clone(), Expr::u32(node_kind::BINARY)),
                Expr::eq(typed_kind, Expr::u32(C_AST_KIND_ASSIGN_EXPR)),
            ),
            Expr::u32(C_EXPR_SHAPE_BINARY),
            Expr::select(
                Expr::eq(raw_kind, Expr::u32(TOK_QUESTION)),
                Expr::u32(C_EXPR_SHAPE_CONDITIONAL),
                Expr::u32(C_EXPR_SHAPE_NONE),
            ),
        ),
    )
}

pub(crate) fn c_expr_operator_precedence(raw_kind: Expr, typed_kind: Expr) -> Expr {
    Expr::select(
        Expr::and(
            Expr::ne(typed_kind.clone(), Expr::u32(node_kind::BINARY)),
            Expr::and(
                Expr::ne(typed_kind.clone(), Expr::u32(C_AST_KIND_ASSIGN_EXPR)),
                Expr::and(
                    Expr::ne(typed_kind.clone(), Expr::u32(C_AST_KIND_CONDITIONAL_EXPR)),
                    Expr::ne(raw_kind.clone(), Expr::u32(TOK_QUESTION)),
                ),
            ),
        ),
        Expr::u32(0),
        Expr::select(
            Expr::eq(typed_kind.clone(), Expr::u32(C_AST_KIND_ASSIGN_EXPR)),
            Expr::u32(2),
            Expr::select(
                Expr::eq(typed_kind.clone(), Expr::u32(C_AST_KIND_CONDITIONAL_EXPR)),
                Expr::u32(3),
                Expr::select(
                    Expr::eq(raw_kind.clone(), Expr::u32(TOK_OR)),
                    Expr::u32(VAST_PREVIOUS_SIBLING_FIELD),
                    Expr::select(
                        Expr::eq(raw_kind.clone(), Expr::u32(TOK_AND)),
                        Expr::u32(5),
                        Expr::select(
                            Expr::eq(raw_kind.clone(), Expr::u32(TOK_PIPE)),
                            Expr::u32(6),
                            Expr::select(
                                Expr::eq(raw_kind.clone(), Expr::u32(TOK_CARET)),
                                Expr::u32(7),
                                Expr::select(
                                    Expr::eq(raw_kind.clone(), Expr::u32(TOK_AMP)),
                                    Expr::u32(8),
                                    Expr::select(
                                        Expr::or(
                                            Expr::eq(raw_kind.clone(), Expr::u32(TOK_EQ)),
                                            Expr::eq(raw_kind.clone(), Expr::u32(TOK_NE)),
                                        ),
                                        Expr::u32(9),
                                        Expr::select(
                                            any_token_eq(
                                                raw_kind.clone(),
                                                &[TOK_LT, TOK_GT, TOK_LE, TOK_GE],
                                            ),
                                            Expr::u32(10),
                                            Expr::select(
                                                Expr::or(
                                                    Expr::eq(
                                                        raw_kind.clone(),
                                                        Expr::u32(TOK_LSHIFT),
                                                    ),
                                                    Expr::eq(
                                                        raw_kind.clone(),
                                                        Expr::u32(TOK_RSHIFT),
                                                    ),
                                                ),
                                                Expr::u32(11),
                                                Expr::select(
                                                    Expr::or(
                                                        Expr::eq(
                                                            raw_kind.clone(),
                                                            Expr::u32(TOK_PLUS),
                                                        ),
                                                        Expr::eq(
                                                            raw_kind.clone(),
                                                            Expr::u32(TOK_MINUS),
                                                        ),
                                                    ),
                                                    Expr::u32(12),
                                                    Expr::select(
                                                        any_token_eq(
                                                            raw_kind,
                                                            &[TOK_STAR, TOK_SLASH, TOK_PERCENT],
                                                        ),
                                                        Expr::u32(13),
                                                        Expr::u32(0),
                                                    ),
                                                ),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    )
}

pub(crate) fn c_expr_operator_associativity(typed_kind: Expr) -> Expr {
    Expr::select(
        Expr::or(
            Expr::eq(typed_kind.clone(), Expr::u32(C_AST_KIND_ASSIGN_EXPR)),
            Expr::eq(typed_kind.clone(), Expr::u32(C_AST_KIND_CONDITIONAL_EXPR)),
        ),
        Expr::u32(C_EXPR_ASSOC_RIGHT),
        Expr::select(
            Expr::eq(typed_kind, Expr::u32(node_kind::BINARY)),
            Expr::u32(C_EXPR_ASSOC_LEFT),
            Expr::u32(C_EXPR_ASSOC_NONE),
        ),
    )
}

pub(crate) fn is_expr_shape_boundary(raw_kind: Expr, include_ternary_parts: bool) -> Expr {
    let common = Expr::or(
        Expr::eq(raw_kind.clone(), Expr::u32(TOK_SEMICOLON)),
        Expr::eq(raw_kind.clone(), Expr::u32(TOK_COMMA)),
    );
    if include_ternary_parts {
        Expr::or(
            common,
            Expr::or(
                Expr::eq(raw_kind.clone(), Expr::u32(TOK_QUESTION)),
                Expr::eq(raw_kind, Expr::u32(TOK_COLON)),
            ),
        )
    } else {
        common
    }
}

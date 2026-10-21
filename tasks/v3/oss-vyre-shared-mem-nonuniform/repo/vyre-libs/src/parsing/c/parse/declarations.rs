use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// LEGO Block 3: Declaration-local type-specifier propagation.
///
/// Each lane walks backward inside its declaration segment and records the
/// nearest visible C base type specifier. The scan crosses nested declarator
/// punctuation, array/function suffixes, and aggregate specifier bodies such as
/// `struct s { ... } x`, but stops at statement/directive boundaries so a stale
/// declaration cannot leak into later code.
#[must_use]
pub fn opt_propagate_type_specifiers(
    tok_types: &str,
    tok_depths: &str,
    node_out: &str,
    num_tokens: Expr,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let loop_body = vec![
        Node::let_bind("tok", Expr::load(tok_types, t.clone())),
        Node::let_bind("depth", Expr::load(tok_depths, t.clone())),
        Node::let_bind("active_type", Expr::u32(0)),
        Node::let_bind("blocked", Expr::u32(0)),
        Node::let_bind("brace_skip_depth", Expr::u32(0)),
        Node::loop_for(
            "scan_delta",
            Expr::u32(0),
            Expr::add(t.clone(), Expr::u32(1)),
            vec![
                Node::let_bind("scan_i", Expr::sub(t.clone(), Expr::var("scan_delta"))),
                Node::let_bind("scan_tok", Expr::load(tok_types, Expr::var("scan_i"))),
                Node::let_bind("scan_depth", Expr::load(tok_depths, Expr::var("scan_i"))),
                Node::let_bind("scan_has_prev", Expr::gt(Expr::var("scan_i"), Expr::u32(0))),
                Node::let_bind(
                    "scan_has_prev_prev",
                    Expr::gt(Expr::var("scan_i"), Expr::u32(1)),
                ),
                Node::let_bind(
                    "prev_tok",
                    Expr::select(
                        Expr::var("scan_has_prev"),
                        Expr::load(tok_types, Expr::sub(Expr::var("scan_i"), Expr::u32(1))),
                        Expr::u32(TOK_EOF),
                    ),
                ),
                Node::let_bind(
                    "prev_prev_tok",
                    Expr::select(
                        Expr::var("scan_has_prev_prev"),
                        Expr::load(tok_types, Expr::sub(Expr::var("scan_i"), Expr::u32(2))),
                        Expr::u32(TOK_EOF),
                    ),
                ),
                Node::let_bind(
                    "same_or_outer_depth",
                    Expr::le(Expr::var("scan_depth"), Expr::var("depth")),
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("blocked"), Expr::u32(0)),
                        Expr::and(
                            Expr::var("same_or_outer_depth"),
                            Expr::gt(Expr::var("brace_skip_depth"), Expr::u32(0)),
                        ),
                    ),
                    vec![
                        Node::if_then(
                            Expr::eq(Expr::var("scan_tok"), Expr::u32(TOK_RBRACE)),
                            vec![Node::assign(
                                "brace_skip_depth",
                                Expr::add(Expr::var("brace_skip_depth"), Expr::u32(1)),
                            )],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("scan_tok"), Expr::u32(TOK_LBRACE)),
                            vec![
                                Node::assign(
                                    "brace_skip_depth",
                                    Expr::sub(Expr::var("brace_skip_depth"), Expr::u32(1)),
                                ),
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("brace_skip_depth"), Expr::u32(0)),
                                        Expr::not(is_aggregate_body_open(
                                            Expr::var("prev_tok"),
                                            Expr::var("prev_prev_tok"),
                                        )),
                                    ),
                                    vec![Node::assign("blocked", Expr::u32(1))],
                                ),
                            ],
                        ),
                    ],
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("blocked"), Expr::u32(0)),
                        Expr::and(
                            Expr::eq(Expr::var("brace_skip_depth"), Expr::u32(0)),
                            Expr::and(
                                Expr::var("same_or_outer_depth"),
                                Expr::eq(Expr::var("scan_tok"), Expr::u32(TOK_RBRACE)),
                            ),
                        ),
                    ),
                    vec![Node::assign("brace_skip_depth", Expr::u32(1))],
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("blocked"), Expr::u32(0)),
                        Expr::and(
                            Expr::eq(Expr::var("brace_skip_depth"), Expr::u32(0)),
                            Expr::and(
                                Expr::var("same_or_outer_depth"),
                                is_declaration_boundary(
                                    Expr::var("scan_tok"),
                                    Expr::var("prev_tok"),
                                    Expr::var("prev_prev_tok"),
                                ),
                            ),
                        ),
                    ),
                    vec![Node::assign("blocked", Expr::u32(1))],
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("blocked"), Expr::u32(0)),
                        Expr::and(
                            Expr::eq(Expr::var("active_type"), Expr::u32(0)),
                            Expr::and(
                                Expr::and(
                                    Expr::eq(Expr::var("brace_skip_depth"), Expr::u32(0)),
                                    Expr::var("same_or_outer_depth"),
                                ),
                                is_propagatable_type_token(
                                    Expr::var("scan_tok"),
                                    Expr::var("prev_tok"),
                                    Expr::var("prev_prev_tok"),
                                    Expr::gt(Expr::var("scan_delta"), Expr::u32(0)),
                                ),
                            ),
                        ),
                    ),
                    vec![Node::assign("active_type", Expr::var("scan_tok"))],
                ),
            ],
        ),
        Node::store(node_out, t.clone(), Expr::var("active_type")),
    ];

    let tok_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        other => panic!(
            "opt_propagate_type_specifiers requires a literal num_tokens for buffer sizing, \
             got {other:?}. Fix: pass Expr::u32(N)."
        ),
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count),
            BufferDecl::storage(tok_depths, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count),
            BufferDecl::output(node_out, 2, DataType::U32).with_count(tok_count),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::opt_propagate_type_specifiers",
            vec![child_phase(
                "vyre-libs::parsing::opt_propagate_type_specifiers",
                vyre_primitives::parsing::ssa_dominance_scan::OP_ID,
                vec![Node::if_then(Expr::lt(t.clone(), num_tokens), loop_body)],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::opt_propagate_type_specifiers")
    .with_non_composable_with_self(true)
}

fn any_token_eq(token: Expr, values: &[u32]) -> Expr {
    values
        .iter()
        .copied()
        .fold(Expr::bool(false), |acc, value| {
            Expr::or(acc, Expr::eq(token.clone(), Expr::u32(value)))
        })
}

fn is_propagatable_type_token(
    token: Expr,
    prev: Expr,
    prev_prev: Expr,
    allow_identifier_typedef: Expr,
) -> Expr {
    Expr::or(
        is_type_specifier(token.clone()),
        Expr::and(
            allow_identifier_typedef,
            Expr::and(
                Expr::eq(token, Expr::u32(TOK_IDENTIFIER)),
                is_typedef_name_position(prev, prev_prev),
            ),
        ),
    )
}

fn is_type_specifier(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_INT,
            TOK_CHAR_KW,
            TOK_VOID,
            TOK_STRUCT,
            TOK_UNION,
            TOK_ENUM,
            TOK_FLOAT_KW,
            TOK_DOUBLE,
            TOK_SHORT,
            TOK_LONG,
            TOK_SIGNED,
            TOK_UNSIGNED,
            TOK_BOOL,
            TOK_COMPLEX,
            TOK_IMAGINARY,
            TOK_ATOMIC,
            TOK_GNU_TYPEOF,
            TOK_GNU_TYPEOF_UNQUAL,
            TOK_GNU_AUTO_TYPE,
            TOK_GNU_INT128,
            TOK_GNU_BUILTIN_VA_LIST,
            // C23 / TS 18661-2 scalar types and clang/GCC half-precision.
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

fn is_typedef_name_position(prev: Expr, prev_prev: Expr) -> Expr {
    Expr::and(
        Expr::not(is_aggregate_tag_name(prev.clone(), prev_prev)),
        Expr::or(
            is_declaration_start(prev.clone()),
            is_declaration_prefix(prev),
        ),
    )
}

fn is_declaration_start(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_EOF,
            TOK_SEMICOLON,
            TOK_COMMA,
            TOK_LBRACE,
            TOK_RBRACE,
            TOK_LPAREN,
        ],
    )
}

fn is_declaration_prefix(token: Expr) -> Expr {
    any_token_eq(
        token,
        &[
            TOK_TYPEDEF,
            TOK_EXTERN,
            TOK_STATIC,
            TOK_AUTO,
            TOK_REGISTER,
            TOK_INLINE,
            TOK_CONST,
            TOK_VOLATILE,
            TOK_RESTRICT,
            TOK_ALIGNAS,
            TOK_NORETURN,
            TOK_THREAD_LOCAL,
            TOK_GNU_EXTENSION,
            TOK_GNU_TYPEOF,
            TOK_GNU_TYPEOF_UNQUAL,
            TOK_GNU_AUTO_TYPE,
            TOK_GNU_INT128,
            TOK_GNU_BUILTIN_VA_LIST,
            TOK_GNU_ADDRESS_SPACE,
        ],
    )
}

fn is_aggregate_body_open(prev: Expr, prev_prev: Expr) -> Expr {
    Expr::or(
        any_token_eq(prev.clone(), &[TOK_STRUCT, TOK_UNION, TOK_ENUM]),
        is_aggregate_tag_name(prev, prev_prev),
    )
}

fn is_aggregate_tag_name(prev: Expr, prev_prev: Expr) -> Expr {
    Expr::and(
        Expr::eq(prev, Expr::u32(TOK_IDENTIFIER)),
        any_token_eq(prev_prev, &[TOK_STRUCT, TOK_UNION, TOK_ENUM]),
    )
}

fn is_declaration_boundary(token: Expr, prev: Expr, prev_prev: Expr) -> Expr {
    Expr::or(
        any_token_eq(token.clone(), &[TOK_SEMICOLON, TOK_PREPROC]),
        Expr::and(
            Expr::eq(token, Expr::u32(TOK_LBRACE)),
            Expr::not(is_aggregate_body_open(prev, prev_prev)),
        ),
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::opt_propagate_type_specifiers",
        build: || opt_propagate_type_specifiers("tok_types", "tok_depths", "node_out", Expr::u32(1024)),
        // Buffers: tok_types (read-only u32), tok_depths (read-only u32),
        // node_out (read-write u32). The witness asserts propagation across
        // `int a, b;` and termination before `char c;`.
        test_inputs: Some(|| {
            let mut tok_types = vec![0u8; 4 * 1024];
            let mut tok_depths = vec![0u8; 4 * 1024];
            for (i, tok) in [
                TOK_INT,
                TOK_IDENTIFIER,
                TOK_COMMA,
                TOK_IDENTIFIER,
                TOK_SEMICOLON,
                TOK_CHAR_KW,
                TOK_IDENTIFIER,
                TOK_SEMICOLON,
            ]
            .into_iter()
            .enumerate()
            {
                tok_types[i * 4..i * 4 + 4].copy_from_slice(&tok.to_le_bytes());
                tok_depths[i * 4..i * 4 + 4].copy_from_slice(&0u32.to_le_bytes());
            }
            vec![vec![tok_types, tok_depths, vec![0u8; 4 * 1024]]]
        }),
        expected_output: Some(|| {
            let mut out = vec![0u8; 4 * 1024];
            for (i, tok) in [
                TOK_INT, TOK_INT, TOK_INT, TOK_INT, 0, TOK_CHAR_KW, TOK_CHAR_KW, 0,
            ]
            .into_iter()
            .enumerate()
            {
                out[i * 4..i * 4 + 4].copy_from_slice(&tok.to_le_bytes());
            }
            vec![vec![out]]
        }),
        category: Some("parsing"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VL-004: passing a non-literal num_tokens must panic rather than silently
    /// sizing all three GPU buffers to 1. Before the fix, `_ => 1` produced a
    /// Program where tok_types, tok_depths, and node_out each had count=1 even
    /// though the loop guard used the original non-literal expression, causing
    /// OOB GPU accesses for any input with more than 1 token.
    #[test]
    #[should_panic(
        expected = "opt_propagate_type_specifiers requires a literal num_tokens for buffer sizing"
    )]
    fn non_literal_num_tokens_panics_at_build_time() {
        let non_literal = Expr::InvocationId { axis: 0 };
        let _ = opt_propagate_type_specifiers("tok_types", "tok_depths", "node_out", non_literal);
    }

    /// VL-004 negative twin: a literal num_tokens must build without panic and
    /// produce buffers sized exactly to that count (not 1).
    #[test]
    fn literal_num_tokens_builds_with_correct_buffer_size() {
        let program = opt_propagate_type_specifiers(
            "tok_types",
            "tok_depths",
            "node_out",
            Expr::u32(128),
        );
        let buffers = program.buffers();
        let tok_types_count = buffers
            .iter()
            .find(|b| b.name() == "tok_types")
            .expect("tok_types buffer must be declared")
            .count();
        let tok_depths_count = buffers
            .iter()
            .find(|b| b.name() == "tok_depths")
            .expect("tok_depths buffer must be declared")
            .count();
        let node_out_count = buffers
            .iter()
            .find(|b| b.name() == "node_out")
            .expect("node_out buffer must be declared")
            .count();
        assert_eq!(
            tok_types_count, 128,
            "tok_types must be sized to num_tokens=128; got {tok_types_count}"
        );
        assert_eq!(
            tok_depths_count, 128,
            "tok_depths must be sized to num_tokens=128; got {tok_depths_count}"
        );
        assert_eq!(
            node_out_count, 128,
            "node_out must be sized to num_tokens=128; got {node_out_count}"
        );
    }
}

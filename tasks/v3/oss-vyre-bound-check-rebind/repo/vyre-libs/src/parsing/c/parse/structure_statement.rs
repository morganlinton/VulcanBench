use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::memory_model::MemoryOrdering;

/// Compact C11 statement spans for AST construction.
#[must_use]
pub fn c11_statement_bounds(
    tok_types: &str,
    num_tokens: Expr,
    out_statements: &str,
    out_counts: &str,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let tok_count = match &num_tokens {
        Expr::LitU32(0) => 1,
        Expr::LitU32(n) => *n,
        // The statement output buffers are sized at build time from this count
        // (`tok_count.saturating_mul(2)` below). Silently defaulting a
        // runtime-dynamic count to 1 would mis-size those buffers and drop
        // statements with no signal — fail fast instead.
        other => panic!(
            "c11_statement_bounds requires a literal token-window count for build-time output \
             buffer sizing, got a non-literal expression {other:?}. Fix: pass Expr::u32(N)."
        ),
    };
    let scan_count = Expr::buf_len(tok_types);
    const MAX_STMT_THREADS: u32 = 256;

    let loop_body = vec![
        Node::let_bind("token", Expr::load(tok_types, Expr::var("stmt_scan"))),
        Node::if_then(
            Expr::eq(Expr::var("token"), Expr::u32(TOK_LPAREN)),
            vec![Node::assign(
                "paren_depth",
                Expr::add(Expr::var("paren_depth"), Expr::u32(1)),
            )],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("token"), Expr::u32(TOK_RPAREN)),
                Expr::gt(Expr::var("paren_depth"), Expr::u32(0)),
            ),
            vec![Node::assign(
                "paren_depth",
                Expr::sub(Expr::var("paren_depth"), Expr::u32(1)),
            )],
        ),
        Node::if_then(
            Expr::eq(Expr::var("token"), Expr::u32(TOK_LBRACKET)),
            vec![Node::assign(
                "bracket_depth",
                Expr::add(Expr::var("bracket_depth"), Expr::u32(1)),
            )],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("token"), Expr::u32(TOK_RBRACKET)),
                Expr::gt(Expr::var("bracket_depth"), Expr::u32(0)),
            ),
            vec![Node::assign(
                "bracket_depth",
                Expr::sub(Expr::var("bracket_depth"), Expr::u32(1)),
            )],
        ),
        Node::let_bind(
            "at_top_level_expr",
            Expr::and(
                Expr::eq(Expr::var("paren_depth"), Expr::u32(0)),
                Expr::eq(Expr::var("bracket_depth"), Expr::u32(0)),
            ),
        ),
        Node::let_bind(
            "is_brace_boundary",
            Expr::and(
                Expr::var("at_top_level_expr"),
                Expr::or(
                    Expr::eq(Expr::var("token"), Expr::u32(TOK_LBRACE)),
                    Expr::eq(Expr::var("token"), Expr::u32(TOK_RBRACE)),
                ),
            ),
        ),
        Node::let_bind(
            "is_statement_boundary",
            Expr::or(
                Expr::eq(Expr::var("token"), Expr::u32(TOK_SEMICOLON)),
                Expr::var("is_brace_boundary"),
            ),
        ),
        Node::let_bind("stmt_end", Expr::add(Expr::var("stmt_scan"), Expr::u32(1))),
        Node::if_then(
            Expr::and(
                Expr::and(
                    Expr::var("is_statement_boundary"),
                    Expr::eq(Expr::var("found_boundary"), Expr::u32(0)),
                ),
                Expr::gt(Expr::var("stmt_end"), t.clone()),
            ),
            vec![
                Node::assign("found_boundary", Expr::u32(1)),
                Node::assign("stmt_bound_end", Expr::var("stmt_end")),
            ],
        ),
    ];

    let stmt_body = vec![
        Node::let_bind("found_boundary", Expr::u32(0)),
        Node::let_bind("stmt_bound_end", Expr::add(t.clone(), Expr::u32(1))),
        Node::let_bind("paren_depth", Expr::u32(0)),
        Node::let_bind("bracket_depth", Expr::u32(0)),
        Node::loop_for(
            "stmt_scan",
            t.clone(),
            Expr::min(
                Expr::add(t.clone(), Expr::u32(MAX_STMT_THREADS)),
                scan_count.clone(),
            ),
            loop_body,
        ),
        Node::let_bind(
            "stmt_idx",
            Expr::atomic_add(out_counts, Expr::u32(0), Expr::u32(2)),
        ),
        Node::store(out_statements, Expr::var("stmt_idx"), t.clone()),
        Node::store(
            out_statements,
            Expr::add(Expr::var("stmt_idx"), Expr::u32(1)),
            Expr::var("stmt_bound_end"),
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(out_statements, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_count.saturating_mul(2)),
            BufferDecl::storage(out_counts, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c11_statement_bounds",
            vec![
                Node::if_then(
                    Expr::eq(t.clone(), Expr::u32(0)),
                    vec![Node::store(out_counts, Expr::u32(0), Expr::u32(0))],
                ),
                Node::Barrier {
                    ordering: MemoryOrdering::SeqCst,
                },
                child_phase(
                    "vyre-libs::parsing::c11_statement_bounds",
                    vyre_primitives::bitset::select::OP_ID,
                    vec![Node::if_then(Expr::lt(t.clone(), scan_count), stmt_body)],
                ),
            ],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c11_statement_bounds")
    .with_non_composable_with_self(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_bounds_sizes_outputs_to_full_literal_window_without_fixed_clamp() {
        let token_window = crate::parsing::c::pipeline::stages::C11_AST_MAX_TOK_SCAN + 1;
        let program = c11_statement_bounds(
            "tok_types",
            Expr::u32(token_window),
            "out_statements",
            "out_counts",
        );
        let out_statements = program
            .buffers
            .iter()
            .find(|buffer| buffer.name() == "out_statements")
            .expect("Fix: out_statements buffer must exist");
        assert_eq!(out_statements.count, token_window.saturating_mul(2));
    }

    #[test]
    fn statement_bounds_rejects_non_literal_token_count_for_buffer_sizing() {
        let panic = std::panic::catch_unwind(|| {
            let _ = c11_statement_bounds(
                "tok_types",
                Expr::var("dynamic_tokens"),
                "out_statements",
                "out_counts",
            );
        })
        .expect_err("non-literal statement-bound count must fail");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&'static str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("requires a literal token-window count"),
            "{message}"
        );
    }

    #[test]
    fn statement_bounds_initializes_atomic_count_inside_kernel() {
        let source = include_str!("structure_statement.rs");
        assert!(
            source.contains("Node::store(out_counts, Expr::u32(0), Expr::u32(0))"),
            "Fix: statement bounds must zero out_counts in-kernel before atomic_add."
        );
        assert!(
            source.contains("MemoryOrdering::SeqCst"),
            "Fix: statement bounds must synchronize after zeroing out_counts before worker lanes append records."
        );
    }
}

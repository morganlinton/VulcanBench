use crate::parsing::c::lex::tokens::*;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Topological CFG Assembly (Agent A - Frontend)
///
/// Organizes flattened statements into a control flow hierarchy without CPU trees.
/// Every statement computes its Control Flow Header token index (`if`, `while`, etc.)
/// and its target conditional expression.
///
/// Uses purely spatial boundaries computed during the Structure pass.
///
/// `out_scope_parents` is part of the stable public signature. The
/// current body derives header positions from `tok_types` +
/// `statements`; callers may pass the scope-parent buffer produced by
/// the surrounding C pipeline without changing this contract.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn ast_cfg_blocks(
    tok_types: &str,
    _out_scope_parents: &str, // Inherited logical boundaries  -  reserved for V2.
    statements: &str,         // Array of statement [start_tok, end_tok]
    num_statements: Expr,
    out_block_headers: &str, // Maps stmt -> enclosing control flow keyword token index
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    // We assume `statements` provides [start_tok, end_tok].
    // If start_tok is immediately preceded by an `if (expr)` or `while (expr)`,
    // we bind it. Since scopes.rs handles boundaries, the block inherently wraps.
    // For V1, we simulate a scan backward to the nearest keyword.

    let loop_body = vec![
        Node::let_bind(
            "stmt_start",
            Expr::load(statements, Expr::mul(t.clone(), Expr::u32(2))),
        ),
        Node::let_bind("header_tok", Expr::u32(u32::MAX)),
        // `found` replaces the previous `assign("i", 32)` early-break:
        // the reference interpreter rejects assignment to a loop
        // variable, and the bounded 32-iteration scan is cheap enough
        // to run to completion anyway. Further iterations become
        // no-ops once `found` flips to 1.
        Node::let_bind("found", Expr::u32(0)),
        Node::loop_for(
            "i",
            Expr::u32(1),
            Expr::u32(32),
            vec![Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var("found"), Expr::u32(0)),
                    Expr::ge(Expr::var("stmt_start"), Expr::var("i")),
                ),
                vec![
                    Node::let_bind(
                        "check_idx",
                        Expr::sub(Expr::var("stmt_start"), Expr::var("i")),
                    ),
                    Node::let_bind("prev_tok", Expr::load(tok_types, Expr::var("check_idx"))),
                    Node::if_then(
                        Expr::or(
                            Expr::eq(Expr::var("prev_tok"), Expr::u32(TOK_IF)),
                            Expr::eq(Expr::var("prev_tok"), Expr::u32(TOK_WHILE)),
                        ),
                        vec![
                            Node::assign("header_tok", Expr::var("check_idx")),
                            Node::assign("found", Expr::u32(1)),
                        ],
                    ),
                ],
            )],
        ),
        Node::store(out_block_headers, t.clone(), Expr::var("header_tok")),
    ];

    let stmt_count = match &num_statements {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(stmt_count.saturating_mul(4)),
            BufferDecl::storage(statements, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(stmt_count.saturating_mul(2)),
            BufferDecl::storage(out_block_headers, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(stmt_count),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::ast_cfg_blocks",
            vec![Node::if_then(
                Expr::lt(t.clone(), num_statements),
                loop_body,
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::ast_cfg_blocks")
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::ast_cfg_blocks",
        build: || ast_cfg_blocks(
            "tok_types", "out_scope_parents", "statements",
            Expr::u32(2), "out_block_headers"
        ),
        // 2-statement fixture. tok_types[0] = TOK_IF, followed by
        // a body token at index 1; statements = [(1, 1), (0, 0)].
        // Statement 0 starts at token 1; the backward lookback finds
        // TOK_IF at check_idx=0 and writes header_tok=0. Statement 1
        // starts at 0 so the `stmt_start >= i` guard never fires and
        // header_tok stays u32::MAX.
        test_inputs: Some(|| {
            let mut tok_types = vec![0u32; 8];
            tok_types[0] = TOK_IF;
            // statements is [start_tok, end_tok] per statement:
            //   stmt 0 → (1, 1), stmt 1 → (0, 0)
            let statements: [u32; 4] = [1, 1, 0, 0];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&tok_types), to_bytes(&statements), vec![0u8; 4 * 2]]]
        }),
        expected_output: Some(|| {
            let headers: [u32; 2] = [0, u32::MAX];
            let bytes = vyre_primitives::wire::pack_u32_slice(&headers);
            vec![vec![bytes]]
        }),
        category: Some("parsing"),
    }
}

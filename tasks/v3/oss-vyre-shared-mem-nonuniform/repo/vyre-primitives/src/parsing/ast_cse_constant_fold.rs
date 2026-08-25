//! Constant-folding wave for AST optimizer passes.

use vyre_foundation::ir::{Expr, Node};

use super::ast_ops::{AST_ADD, AST_CONST_INT, AST_MUL};

/// Stable op id for the constant-fold child region.
pub const OP_ID: &str = "vyre-primitives::parsing::ast_cse_constant_fold";

/// Emit the constant-folding phase for add/mul AST nodes.
#[must_use]
pub fn ast_cse_constant_fold(
    ast_opcodes: &str,
    ast_lefts: &str,
    ast_rights: &str,
    ast_vals: &str,
    out_modified_flag: &str,
    t: Expr,
) -> Vec<Node> {
    vec![Node::if_then(
        Expr::or(
            Expr::eq(Expr::var("op"), Expr::u32(AST_ADD)),
            Expr::eq(Expr::var("op"), Expr::u32(AST_MUL)),
        ),
        vec![
            Node::let_bind("l_idx", Expr::load(ast_lefts, t.clone())),
            Node::let_bind("r_idx", Expr::load(ast_rights, t.clone())),
            Node::let_bind("l_op", Expr::load(ast_opcodes, Expr::var("l_idx"))),
            Node::let_bind("r_op", Expr::load(ast_opcodes, Expr::var("r_idx"))),
            Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var("l_op"), Expr::u32(AST_CONST_INT)),
                    Expr::eq(Expr::var("r_op"), Expr::u32(AST_CONST_INT)),
                ),
                vec![
                    Node::let_bind("l_v", Expr::load(ast_vals, Expr::var("l_idx"))),
                    Node::let_bind("r_v", Expr::load(ast_vals, Expr::var("r_idx"))),
                    Node::let_bind("new_val", Expr::u32(0)),
                    Node::if_then(
                        Expr::eq(Expr::var("op"), Expr::u32(AST_ADD)),
                        vec![Node::assign(
                            "new_val",
                            Expr::add(Expr::var("l_v"), Expr::var("r_v")),
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("op"), Expr::u32(AST_MUL)),
                        vec![Node::assign(
                            "new_val",
                            Expr::mul(Expr::var("l_v"), Expr::var("r_v")),
                        )],
                    ),
                    Node::store(ast_opcodes, t.clone(), Expr::u32(AST_CONST_INT)),
                    Node::store(ast_vals, t.clone(), Expr::var("new_val")),
                    Node::let_bind(
                        "_",
                        Expr::atomic_add(out_modified_flag, Expr::u32(0), Expr::u32(1)),
                    ),
                ],
            ),
        ],
    )]
}

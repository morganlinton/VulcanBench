use crate::parsing::c::lex::tokens::{TOK_INTEGER, TOK_LPAREN};
use crate::parsing::core::ast::node::{AST_CONST_INT, AST_VAR};
use vyre::ir::{Expr, Node};

use super::operator::{ast_opcode, precedence, should_pop_cached};
use super::STACK_SLOTS_PER_STATEMENT;

pub(super) fn emit_value_leaf(
    out_ast_nodes: &str,
    out_ast_count: &str,
    scratch_val_stack: &str,
    val_stack_base: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind(
            "ast_idx",
            Expr::atomic_add(out_ast_count, Expr::u32(0), Expr::u32(4)),
        ),
        Node::let_bind(
            "opcode",
            Expr::select(
                Expr::eq(Expr::var("tok"), Expr::u32(TOK_INTEGER)),
                Expr::u32(AST_CONST_INT),
                Expr::u32(AST_VAR),
            ),
        ),
        Node::store(out_ast_nodes, Expr::var("ast_idx"), Expr::var("opcode")),
        Node::store(
            out_ast_nodes,
            Expr::add(Expr::var("ast_idx"), Expr::u32(1)),
            Expr::u32(u32::MAX),
        ),
        Node::store(
            out_ast_nodes,
            Expr::add(Expr::var("ast_idx"), Expr::u32(2)),
            Expr::u32(u32::MAX),
        ),
        Node::store(
            out_ast_nodes,
            Expr::add(Expr::var("ast_idx"), Expr::u32(3)),
            Expr::var("tok_idx"),
        ),
        Node::store(
            scratch_val_stack,
            Expr::add(val_stack_base, Expr::var("v_sp")),
            Expr::var("ast_idx"),
        ),
        Node::assign("v_sp", Expr::add(Expr::var("v_sp"), Expr::u32(1))),
    ]
}

fn reduce_loaded_operator(
    out_ast_nodes: &str,
    out_ast_count: &str,
    scratch_val_stack: &str,
    val_stack_base: Expr,
    opcode: Expr,
) -> Vec<Node> {
    vec![
        Node::assign("v_sp", Expr::sub(Expr::var("v_sp"), Expr::u32(1))),
        Node::let_bind(
            "right_child",
            Expr::load(
                scratch_val_stack,
                Expr::add(val_stack_base.clone(), Expr::var("v_sp")),
            ),
        ),
        Node::assign("v_sp", Expr::sub(Expr::var("v_sp"), Expr::u32(1))),
        Node::let_bind(
            "left_child",
            Expr::load(
                scratch_val_stack,
                Expr::add(val_stack_base.clone(), Expr::var("v_sp")),
            ),
        ),
        Node::let_bind(
            "ast_idx",
            Expr::atomic_add(out_ast_count, Expr::u32(0), Expr::u32(4)),
        ),
        Node::store(out_ast_nodes, Expr::var("ast_idx"), opcode),
        Node::store(
            out_ast_nodes,
            Expr::add(Expr::var("ast_idx"), Expr::u32(1)),
            Expr::var("left_child"),
        ),
        Node::store(
            out_ast_nodes,
            Expr::add(Expr::var("ast_idx"), Expr::u32(2)),
            Expr::var("right_child"),
        ),
        Node::store(
            out_ast_nodes,
            Expr::add(Expr::var("ast_idx"), Expr::u32(3)),
            Expr::u32(u32::MAX),
        ),
        Node::store(
            scratch_val_stack,
            Expr::add(val_stack_base, Expr::var("v_sp")),
            Expr::var("ast_idx"),
        ),
        Node::assign("v_sp", Expr::add(Expr::var("v_sp"), Expr::u32(1))),
    ]
}

fn reduce_if_allowed(
    scratch_op_stack: &str,
    out_ast_nodes: &str,
    out_ast_count: &str,
    scratch_val_stack: &str,
    val_stack_base: Expr,
    op_stack_base: Expr,
) -> Vec<Node> {
    let mut body = vec![Node::assign(
        "o_sp",
        Expr::sub(Expr::var("o_sp"), Expr::u32(1)),
    )];
    body.extend(reduce_loaded_operator(
        out_ast_nodes,
        out_ast_count,
        scratch_val_stack,
        val_stack_base,
        Expr::var("top_ast_opcode"),
    ));

    vec![
        Node::let_bind(
            "top_op",
            Expr::load(
                scratch_op_stack,
                Expr::add(op_stack_base, Expr::sub(Expr::var("o_sp"), Expr::u32(1))),
            ),
        ),
        Node::let_bind("top_op_prec", precedence(Expr::var("top_op"))),
        Node::let_bind("top_ast_opcode", ast_opcode(Expr::var("top_op"))),
        Node::let_bind(
            "reduce_now",
            Expr::and(
                should_pop_cached(
                    Expr::var("top_op"),
                    Expr::var("top_op_prec"),
                    Expr::var("tok_prec"),
                    Expr::var("tok_is_assignment"),
                ),
                Expr::ge(Expr::var("v_sp"), Expr::u32(2)),
            ),
        ),
        Node::if_then(Expr::var("reduce_now"), body),
        Node::if_then(
            Expr::not(Expr::var("reduce_now")),
            vec![Node::assign("done_bin", Expr::u32(1))],
        ),
    ]
}

pub(super) fn binary_token_body(
    scratch_op_stack: &str,
    out_ast_nodes: &str,
    out_ast_count: &str,
    scratch_val_stack: &str,
    val_stack_base: Expr,
    op_stack_base: Expr,
) -> Vec<Node> {
    let reduce_one = reduce_if_allowed(
        scratch_op_stack,
        out_ast_nodes,
        out_ast_count,
        scratch_val_stack,
        val_stack_base,
        op_stack_base.clone(),
    );

    vec![
        Node::let_bind("done_bin", Expr::u32(0)),
        Node::loop_for(
            "pop",
            Expr::u32(0),
            Expr::u32(STACK_SLOTS_PER_STATEMENT),
            vec![Node::if_then(
                Expr::eq(Expr::var("done_bin"), Expr::u32(0)),
                vec![
                    Node::if_then(
                        Expr::eq(Expr::var("o_sp"), Expr::u32(0)),
                        vec![Node::assign("done_bin", Expr::u32(1))],
                    ),
                    Node::if_then(Expr::ne(Expr::var("o_sp"), Expr::u32(0)), reduce_one),
                ],
            )],
        ),
        Node::store(
            scratch_op_stack,
            Expr::add(op_stack_base, Expr::var("o_sp")),
            Expr::var("tok"),
        ),
        Node::assign("o_sp", Expr::add(Expr::var("o_sp"), Expr::u32(1))),
    ]
}

pub(super) fn rparen_body(
    scratch_op_stack: &str,
    out_ast_nodes: &str,
    out_ast_count: &str,
    scratch_val_stack: &str,
    val_stack_base: Expr,
    op_stack_base: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind("done_rp", Expr::u32(0)),
        Node::loop_for(
            "pop",
            Expr::u32(0),
            Expr::u32(STACK_SLOTS_PER_STATEMENT),
            vec![Node::if_then(
                Expr::eq(Expr::var("done_rp"), Expr::u32(0)),
                vec![
                    Node::if_then(
                        Expr::eq(Expr::var("o_sp"), Expr::u32(0)),
                        vec![Node::assign("done_rp", Expr::u32(1))],
                    ),
                    Node::if_then(Expr::ne(Expr::var("o_sp"), Expr::u32(0)), {
                        let mut body = vec![
                            Node::assign("o_sp", Expr::sub(Expr::var("o_sp"), Expr::u32(1))),
                            Node::let_bind(
                                "top_op",
                                Expr::load(
                                    scratch_op_stack,
                                    Expr::add(op_stack_base.clone(), Expr::var("o_sp")),
                                ),
                            ),
                            Node::let_bind("top_ast_opcode", ast_opcode(Expr::var("top_op"))),
                            Node::if_then(
                                Expr::eq(Expr::var("top_op"), Expr::u32(TOK_LPAREN)),
                                vec![Node::assign("done_rp", Expr::u32(1))],
                            ),
                        ];
                        body.push(Node::if_then(
                            Expr::and(
                                Expr::ne(Expr::var("top_op"), Expr::u32(TOK_LPAREN)),
                                Expr::ge(Expr::var("v_sp"), Expr::u32(2)),
                            ),
                            reduce_loaded_operator(
                                out_ast_nodes,
                                out_ast_count,
                                scratch_val_stack,
                                val_stack_base.clone(),
                                Expr::var("top_ast_opcode"),
                            ),
                        ));
                        body
                    }),
                ],
            )],
        ),
    ]
}

pub(super) fn final_sweep_body(
    scratch_op_stack: &str,
    out_ast_nodes: &str,
    out_ast_count: &str,
    scratch_val_stack: &str,
    val_stack_base: Expr,
    op_stack_base: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind("done_fs", Expr::u32(0)),
        Node::loop_for(
            "pop",
            Expr::u32(0),
            Expr::u32(STACK_SLOTS_PER_STATEMENT),
            vec![Node::if_then(
                Expr::eq(Expr::var("done_fs"), Expr::u32(0)),
                vec![
                    Node::if_then(
                        Expr::eq(Expr::var("o_sp"), Expr::u32(0)),
                        vec![Node::assign("done_fs", Expr::u32(1))],
                    ),
                    Node::if_then(Expr::ne(Expr::var("o_sp"), Expr::u32(0)), {
                        let mut body = vec![
                            Node::assign("o_sp", Expr::sub(Expr::var("o_sp"), Expr::u32(1))),
                            Node::let_bind(
                                "top_op",
                                Expr::load(
                                    scratch_op_stack,
                                    Expr::add(op_stack_base, Expr::var("o_sp")),
                                ),
                            ),
                            Node::let_bind("top_ast_opcode", ast_opcode(Expr::var("top_op"))),
                        ];
                        body.push(Node::if_then(
                            Expr::and(
                                Expr::ne(Expr::var("top_op"), Expr::u32(TOK_LPAREN)),
                                Expr::ge(Expr::var("v_sp"), Expr::u32(2)),
                            ),
                            reduce_loaded_operator(
                                out_ast_nodes,
                                out_ast_count,
                                scratch_val_stack,
                                val_stack_base,
                                Expr::var("top_ast_opcode"),
                            ),
                        ));
                        body
                    }),
                ],
            )],
        ),
    ]
}

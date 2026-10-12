use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::parsing::core::ast::node::*;
use crate::region::wrap_anonymous;
use emit::{binary_token_body, emit_value_leaf, final_sweep_body, rparen_body};
use operator::{is_assignment_token, is_value_token, precedence};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

mod emit;
mod operator;

const OP_ID: &str = "vyre-libs::parsing::ast_shunting_yard";
const STATEMENT_PASS_OP_ID: &str = "vyre-libs::parsing::ast_shunting_yard::statement_pass";
const MAX_TOK_SCAN: u32 = 65_536;
const STACK_SLOTS_PER_STATEMENT: u32 = 64;

use crate::scan::dispatch_io::pack_u32_slice as pack_u32;

/// Data-parallel shunting-yard AST builder.
///
/// Each invocation owns one statement boundary and emits a flat node stream
/// where every AST node is four `u32` words: `(opcode, left, right, value_ref)`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn ast_shunting_yard(
    tok_types: &str,
    statements: &str,
    num_statements: Expr,
    out_ast_nodes: &str,
    out_ast_count: &str,
    out_statement_roots: &str,
    scratch_val_stack: &str,
    scratch_op_stack: &str,
) -> Program {
    ast_shunting_yard_program(
        tok_types,
        statements,
        num_statements,
        out_ast_nodes,
        out_ast_count,
        out_statement_roots,
        scratch_val_stack,
        scratch_op_stack,
        MAX_TOK_SCAN,
        None,
    )
}

/// Data-parallel shunting-yard AST builder with caller-bounded capacities.
///
/// This preserves [`ast_shunting_yard`]'s semantics while avoiding the
/// release-path cost of uploading and allocating fixed 65k-token buffers for
/// small translation units. `token_capacity` sizes the four-word AST-node
/// stream, while `statement_capacity` sizes the per-statement root table.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn ast_shunting_yard_with_capacity(
    tok_types: &str,
    statements: &str,
    num_statements: Expr,
    out_ast_nodes: &str,
    out_ast_count: &str,
    out_statement_roots: &str,
    scratch_val_stack: &str,
    scratch_op_stack: &str,
    token_capacity: u32,
    statement_capacity: u32,
) -> Program {
    ast_shunting_yard_program(
        tok_types,
        statements,
        num_statements,
        out_ast_nodes,
        out_ast_count,
        out_statement_roots,
        scratch_val_stack,
        scratch_op_stack,
        token_capacity,
        Some(statement_capacity),
    )
}

#[allow(clippy::too_many_arguments)]
fn ast_shunting_yard_program(
    tok_types: &str,
    statements: &str,
    num_statements: Expr,
    out_ast_nodes: &str,
    out_ast_count: &str,
    out_statement_roots: &str,
    scratch_val_stack: &str,
    scratch_op_stack: &str,
    token_capacity: u32,
    statement_capacity: Option<u32>,
) -> Program {
    let token_capacity = token_capacity.clamp(1, MAX_TOK_SCAN);
    let statement_capacity = statement_capacity.map(|capacity| capacity.clamp(1, MAX_TOK_SCAN));
    let t = Expr::InvocationId { axis: 0 };
    let val_stack_base = Expr::mul(t.clone(), Expr::u32(STACK_SLOTS_PER_STATEMENT));
    let op_stack_base = Expr::mul(t.clone(), Expr::u32(STACK_SLOTS_PER_STATEMENT));

    let loop_body = vec![
        Node::let_bind(
            "stmt_start",
            Expr::load(statements, Expr::mul(t.clone(), Expr::u32(2))),
        ),
        Node::let_bind(
            "stmt_end",
            Expr::load(
                statements,
                Expr::add(Expr::mul(t.clone(), Expr::u32(2)), Expr::u32(1)),
            ),
        ),
        Node::let_bind("v_sp", Expr::u32(0)),
        Node::let_bind("o_sp", Expr::u32(0)),
        Node::loop_for(
            "tok_idx",
            Expr::var("stmt_start"),
            Expr::var("stmt_end"),
            vec![
                Node::let_bind("tok", Expr::load(tok_types, Expr::var("tok_idx"))),
                Node::let_bind("tok_prec", precedence(Expr::var("tok"))),
                Node::let_bind("tok_is_assignment", is_assignment_token(Expr::var("tok"))),
                Node::if_then(
                    is_value_token(Expr::var("tok")),
                    emit_value_leaf(
                        out_ast_nodes,
                        out_ast_count,
                        scratch_val_stack,
                        val_stack_base.clone(),
                    ),
                ),
                Node::if_then(
                    Expr::ne(Expr::var("tok_prec"), Expr::u32(0)),
                    binary_token_body(
                        scratch_op_stack,
                        out_ast_nodes,
                        out_ast_count,
                        scratch_val_stack,
                        val_stack_base.clone(),
                        op_stack_base.clone(),
                    ),
                ),
                Node::if_then(
                    Expr::eq(Expr::var("tok"), Expr::u32(TOK_LPAREN)),
                    vec![
                        Node::store(
                            scratch_op_stack,
                            Expr::add(op_stack_base.clone(), Expr::var("o_sp")),
                            Expr::var("tok"),
                        ),
                        Node::assign("o_sp", Expr::add(Expr::var("o_sp"), Expr::u32(1))),
                    ],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("tok"), Expr::u32(TOK_RPAREN)),
                    rparen_body(
                        scratch_op_stack,
                        out_ast_nodes,
                        out_ast_count,
                        scratch_val_stack,
                        val_stack_base.clone(),
                        op_stack_base.clone(),
                    ),
                ),
            ],
        ),
        Node::Block(final_sweep_body(
            scratch_op_stack,
            out_ast_nodes,
            out_ast_count,
            scratch_val_stack,
            val_stack_base.clone(),
            op_stack_base,
        )),
        Node::if_then(
            Expr::gt(Expr::var("v_sp"), Expr::u32(0)),
            vec![Node::store(
                out_statement_roots,
                t.clone(),
                Expr::load(
                    scratch_val_stack,
                    Expr::add(val_stack_base, Expr::sub(Expr::var("v_sp"), Expr::u32(1))),
                ),
            )],
        ),
        Node::if_then(
            Expr::eq(Expr::var("v_sp"), Expr::u32(0)),
            vec![Node::store(
                out_statement_roots,
                t.clone(),
                Expr::u32(u32::MAX),
            )],
        ),
    ];

    let statement_limit = statement_capacity
        .map(Expr::u32)
        .unwrap_or_else(|| num_statements.clone());
    let out_statement_roots_decl = {
        let decl = BufferDecl::storage(
            out_statement_roots,
            4,
            BufferAccess::ReadWrite,
            DataType::U32,
        );
        if let Some(statement_capacity) = statement_capacity {
            decl.with_count(statement_capacity)
        } else {
            decl
        }
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(statements, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(out_ast_nodes, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(token_capacity.saturating_mul(4)),
            BufferDecl::storage(out_ast_count, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            out_statement_roots_decl,
            BufferDecl::storage(scratch_val_stack, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(
                    statement_capacity
                        .unwrap_or(MAX_TOK_SCAN)
                        .saturating_mul(STACK_SLOTS_PER_STATEMENT),
                ),
            BufferDecl::storage(scratch_op_stack, 6, BufferAccess::ReadWrite, DataType::U32)
                .with_count(
                    statement_capacity
                        .unwrap_or(MAX_TOK_SCAN)
                        .saturating_mul(STACK_SLOTS_PER_STATEMENT),
                ),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            OP_ID,
            vec![Node::if_then(
                Expr::lt(
                    t.clone(),
                    Expr::min(
                        Expr::div(Expr::buf_len(statements), Expr::u32(2)),
                        statement_limit,
                    ),
                ),
                vec![child_phase(OP_ID, STATEMENT_PASS_OP_ID, loop_body)],
            )],
        )],
    )
    .with_entry_op_id(OP_ID)
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || ast_shunting_yard_with_capacity(
            "tok_types", "statements", Expr::u32(100),
            "out_ast_nodes", "out_ast_count", "out_statement_roots",
            "scratch_val_stack", "scratch_op_stack",
            MAX_TOK_SCAN, 100
        ),
        test_inputs: Some(|| vec![vec![
            shunting_token_fixture(),
            shunting_statement_fixture(),
            vec![0u8; MAX_TOK_SCAN as usize * 4 * 4],
            vec![0u8; 4],
            vec![0u8; 100 * 4],
            vec![0u8; 6_400 * 4],
            vec![0u8; 6_400 * 4],
        ]]),
        expected_output: Some(shunting_expected_output),
        category: Some("parsing"),
    }
}

fn shunting_token_fixture() -> Vec<u8> {
    let mut tokens = vec![0u32; MAX_TOK_SCAN as usize];
    tokens[0] = TOK_IDENTIFIER;
    pack_u32(&tokens)
}

fn shunting_statement_fixture() -> Vec<u8> {
    let mut statements = vec![0u32; 200];
    statements[1] = 1;
    pack_u32(&statements)
}

fn shunting_expected_output() -> Vec<Vec<Vec<u8>>> {
    let mut ast_nodes = vec![0u32; MAX_TOK_SCAN as usize * 4];
    ast_nodes[0..4].copy_from_slice(&[AST_VAR, u32::MAX, u32::MAX, 0]);
    let mut roots = vec![u32::MAX; 100];
    roots[0] = 0;
    vec![vec![
        pack_u32(&ast_nodes),
        pack_u32(&[4]),
        pack_u32(&roots),
        vec![0u8; 6_400 * 4],
        vec![0u8; 6_400 * 4],
    ]]
}

//! Oracle-differential probe: software-pipelining a Load-then-Store loop whose
//! STORE VALUE references the loop variable (beyond the index) leaks the loop
//! variable into the epilogue, which runs OUTSIDE the steady loop.
//!
//! `expr_reads_only` gates the store value to "reads the loaded `x` and is
//! observably free" -- but it returns `true` for ANY `Var`, so a value like
//! `x + i` (i = the loop var) passes. `apply_pipeline` emits the epilogue store
//! with the same value template (`x := pipe`), leaving `Var(i)` in place. After
//! the steady `Loop(i, lo, hi-1, ...)` exits, `i` is out of scope, so the
//! epilogue `Store(buf_out, hi-1, pipe + i)` references an undeclared variable.
//!
//! The epilogue is conceptually the last iteration (`i == hi-1`), so the fix
//! substitutes the loop var with the literal `hi-1` in the epilogue value.

use vyre_foundation::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_software_pipeline::LoopSoftwarePipeline;
use vyre_reference::value::Value;

/// ```text
/// loop i in 0..8 { let x = buf_in[i]; buf_out[i] = x + i; }
/// ```
/// buf_out[i] == buf_in[i] + i.
fn program_value_uses_loop_var() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("buf_in", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::output("buf_out", 1, DataType::U32).with_count(8),
        ],
        [1, 1, 1],
        vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::let_bind("x", Expr::load("buf_in", Expr::var("i"))),
                Node::store(
                    "buf_out",
                    Expr::var("i"),
                    Expr::BinOp {
                        op: BinOp::Add,
                        left: Box::new(Expr::var("x")),
                        right: Box::new(Expr::var("i")),
                    },
                ),
            ],
        }],
    )
}

#[test]
fn software_pipeline_does_not_leak_loop_var_into_epilogue() {
    let program = program_value_uses_loop_var();
    // buf_in is the only ReadOnly non-output buffer.
    let buf_in: Vec<u8> = (10u32..18).flat_map(u32::to_le_bytes).collect();
    let inputs = [Value::from(buf_in)];

    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original loop program is valid and must run");
    // buf_out[i] = buf_in[i] + i = (10+0, 11+1, ... 17+7).
    let expected: Vec<u8> = (0u32..8).flat_map(|i| (10 + i + i).to_le_bytes()).collect();
    assert_eq!(
        original,
        vec![Value::from(expected)],
        "buf_out[i] == buf_in[i] + i",
    );

    let result = LoopSoftwarePipeline::transform(program);
    assert!(result.changed, "the Load-then-Store loop must pipeline");

    let after = vyre_reference::reference_eval(&result.program, &inputs).expect(
        "pipelined program must still validate and run -- the epilogue must not \
         reference the loop variable after the steady loop has exited",
    );
    assert_eq!(
        after, original,
        "software pipelining must preserve semantics when the store value uses \
         the loop variable; the epilogue (iteration hi-1) must substitute the \
         loop var with the literal hi-1",
    );
}

//! Oracle-differential regression: dead_buffer_elim must not remove a buffer
//! that is still *referenced* by a node which survives filtering.
//!
//! The pass computes OUTPUT-liveness (which buffers transitively feed an
//! output) and uses that single set for two distinct jobs: deciding which
//! stores are dead, AND deciding which buffer declarations to keep. Those are
//! not the same question. A buffer read only in a *control* position -- an
//! `If` condition or `Loop` bound whose guarded body has no live store -- does
//! not feed any output, so output-liveness marks it dead and drops its
//! declaration. But `filter_nodes` never removes the `If` itself: it only
//! drops stores to dead buffers. The surviving `if (load(gate) != 0) { }`
//! still loads `gate`, whose declaration is now gone -> a dangling load that
//! the reference interpreter's validator rejects ("load from unknown buffer").
//!
//! The keep-set must be REFERENCE-liveness over the *filtered* program (every
//! buffer still read or written by a surviving node), not output-liveness.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::memory::dead_buffer_elim::DeadBufferElim;
use vyre_reference::value::Value;

/// `store(out, 0, 7); if (load(gate, 0) != 0) { }`. `gate` is a read-only
/// input buffer referenced only by the (side-effect-free) guard, so it
/// contributes nothing to the output `out` -- but the `If` that loads it
/// survives filtering, so its declaration must be kept to avoid a dangling
/// load.
fn program_with_control_only_buffer_read() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
            BufferDecl::read("gate", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(7)),
            Node::if_then_else(
                Expr::ne(Expr::load("gate", Expr::u32(0)), Expr::u32(0)),
                vec![],
                vec![],
            ),
        ],
    )
}

#[test]
fn dead_buffer_elim_keeps_control_only_referenced_buffer() {
    let program = program_with_control_only_buffer_read();
    // One Value for the single non-output buffer `gate` (= 0, so the guard is
    // not taken); `out` is backend-allocated and zero-initialized.
    let inputs = [Value::U32(0)];

    // The original is well-scoped: `gate` is declared, the guard loads it,
    // and the program stores 7 to `out`.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![Value::from(7u32.to_le_bytes().to_vec())],
        "original must store 7 into the single-element output buffer `out`"
    );

    let optimized = DeadBufferElim::transform(program).program;

    // The optimized program must STILL run. Pre-fix this FAILS: dead_buffer_elim
    // removed `gate` (it feeds no output) yet left the `If` that loads it, so
    // the reference interpreter's validator rejects the dangling load
    // ("load from unknown buffer `gate`") and reference_eval errors.
    let transformed = vyre_reference::reference_eval(&optimized, &inputs).expect(
        "dead_buffer_elim must not remove a buffer still referenced by a \
         surviving node (control-only load left a dangling reference)",
    );

    assert_eq!(
        transformed, original,
        "dead_buffer_elim must preserve observable semantics: out == 7"
    );
}

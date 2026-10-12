//! Oracle-differential regression: dead_store_elim must not drop a store when
//! the OVERWRITING store reads the same buffer in its own value/index.
//!
//! The pass drops `Store(b,i,V1)` when a later sibling `Store(b,i,V2)`
//! overwrites the same slot with nothing BETWEEN them observing `b`. But the
//! second store evaluates `V2` (and its index) BEFORE the overwrite, and if
//! `V2` reads `b[i]` it observes the first store's value. The matcher only
//! scanned the nodes strictly between the two stores -- never the overwriting
//! store's own subexpressions -- so
//!
//!   store(b,0,1); store(b,0, load(b,0) + 5)
//!
//! dropped the first store, making `load(b,0)` read the buffer's initial value
//! instead of 1: a read-modify-write miscompile.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::memory::dead_store_elim::DeadStoreElim;
use vyre_reference::value::Value;

/// `store(b,0,1); store(b,0, load(b,0)+5); store(out,0, load(b,0))`. The middle
/// store reads `b[0]` (= 1) to compute `6`; the first store is observed and
/// must survive.
fn program_with_self_reading_overwriter() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
            BufferDecl::storage("b", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::store("b", Expr::u32(0), Expr::u32(1)),
            Node::store(
                "b",
                Expr::u32(0),
                Expr::add(Expr::load("b", Expr::u32(0)), Expr::u32(5)),
            ),
            Node::store("out", Expr::u32(0), Expr::load("b", Expr::u32(0))),
        ],
    )
}

#[test]
fn dead_store_elim_keeps_store_read_by_its_overwriter() {
    let program = program_with_self_reading_overwriter();
    let inputs = [Value::U32(0)]; // b initial = 0 (overwritten before any observable read)

    // Original: b[0]=1; b[0]=load(b,0)+5=6; out=load(b,0)=6.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![
            Value::from(6u32.to_le_bytes().to_vec()), // out
            Value::from(6u32.to_le_bytes().to_vec()), // b
        ],
        "original: the overwriter reads the stored 1, yielding 6"
    );

    let optimized = DeadStoreElim::transform(program).program;

    // The optimized program must produce the SAME observable output. Pre-fix
    // the pass dropped `store(b,0,1)`, so `load(b,0)` in the overwriter reads
    // b's initial 0 -> writes 5 -> out = 5, not 6: a miscompile.
    let transformed = vyre_reference::reference_eval(&optimized, &inputs)
        .expect("optimized program must still run");

    assert_eq!(
        transformed, original,
        "dead_store_elim must not drop a store whose value is read by the \
         overwriting store (out must stay 6, not 5)"
    );
}

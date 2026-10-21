//! Oracle-differential regression: store-to-load forwarding must not forward a
//! stored value whose free variable is REASSIGNED between the store and the
//! load.
//!
//! The pass turns `store(b, i, V); ...; let x = load(b, i)` into
//! `store(b, i, V); ...; let x = V.clone()`, sound only when the bytes the
//! load observes equal `V` evaluated AT THE LOAD POINT. It guards the load
//! target `b[i]` against intervening writes, but `V` itself is cloned forward
//! and re-evaluated later -- so any input `V` depends on must also be
//! invariant across the gap. vyre scalars are mutable via `Node::Assign`
//! (only loop variables are immutable), so
//!
//!   let t = 5; store(b, 0, t); assign t = 99; let x = load(b, 0)
//!
//! forwards `x = t`, which is 99 at the load point, while `b[0]` still holds 5.
//! The reassignment of `t` does not touch buffer `b`, so the old
//! buffer-only invalidation check missed it.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::memory::store_to_load_forward::StoreToLoadForward;
use vyre_reference::value::Value;

/// `let t = 5; store(b, 0, t); assign t = 99; let x = load(b, 0);
/// store(out, 0, x)`. The load observes `b[0] == 5`; forwarding `t` (now 99)
/// would write 99 to `out` instead.
fn program_with_reassigned_forwarded_var() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
            BufferDecl::storage("b", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("t", Expr::u32(5)),
            Node::store("b", Expr::u32(0), Expr::var("t")),
            Node::assign("t", Expr::u32(99)),
            Node::let_bind("x", Expr::load("b", Expr::u32(0))),
            Node::store("out", Expr::u32(0), Expr::var("x")),
        ],
    )
}

#[test]
fn store_to_load_forward_declines_when_forwarded_var_is_reassigned() {
    let program = program_with_reassigned_forwarded_var();
    // One Value for the single non-output buffer `b` (overwritten by the
    // store before it is read, so its initial value is irrelevant); `out` is
    // backend-allocated and zero-initialized.
    let inputs = [Value::U32(0)];

    // Original: store b[0] = 5, reassign t = 99, x = load(b,0) = 5, out[0] = 5.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![
            Value::from(5u32.to_le_bytes().to_vec()), // out
            Value::from(5u32.to_le_bytes().to_vec()), // b
        ],
        "original must load the stored 5, not the reassigned t (99)"
    );

    let optimized = StoreToLoadForward::transform(program).program;

    // The optimized program must produce the SAME observable output. Pre-fix
    // the pass forwarded `x = t`, and `t` is 99 at the load point, so `out`
    // would be 99 -- a miscompile. Post-fix the pass declines (the forwarded
    // value's variable `t` is reassigned in the gap), leaving the load intact.
    let transformed = vyre_reference::reference_eval(&optimized, &inputs)
        .expect("optimized program must still run");

    assert_eq!(
        transformed, original,
        "store-to-load forwarding must not forward a value whose variable is \
         reassigned between the store and the load (out must stay 5, not 99)"
    );
}

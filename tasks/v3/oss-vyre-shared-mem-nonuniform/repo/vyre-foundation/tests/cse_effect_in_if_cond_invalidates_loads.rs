//! Soundness-coupling lock: CSE value-numbering relies on the fusion-hazard
//! validator to keep a non-atomic `Load` and an `Atomic` to the same buffer
//! barrier-separated -- including when the atomic is HIDDEN inside a subgroup
//! operand.
//!
//! Why this matters for CSE
//! ------------------------
//! `CseCtx::expr` does NOT descend into a subgroup op's operand (the op interns
//! to a unique key and is never deduplicated), so an `Atomic` buried inside a
//! subgroup operand never reaches the `Expr::Atomic` arm that calls
//! `clear_observed_state`. `CseCtx::expr` also merely early-returns on an
//! effectful expression without clearing observed state. So if a program could
//! legally place
//!
//! ```text
//! let a = load(ctr, 0);                        // memoize Load(ctr,0) -> "a"
//! if subgroup_add(atomic_add(ctr, 0, 1)) > 0 { let b = load(ctr, 0); ... }
//! ```
//!
//! in one barrier region, CSE would alias `let b = a` across the atomic's
//! mutation of `ctr` -- a value-motion miscompile (reusing a load whose buffer
//! changed). CSE never has to clear for this case because such a program is
//! INVALID: the fusion-hazard validator rejects a non-atomic read and an
//! atomic on the same buffer without a barrier between them, and the barrier
//! independently clears CSE's observed state.
//!
//! The whole protection hinges on `validate::fusion_safety::collect_expr_accesses`
//! descending into subgroup operands (so it sees the *hidden* atomic, not just
//! bare ones). This test locks that: the subgroup-hidden form is rejected, and
//! inserting the barrier both makes it valid AND makes CSE preserve semantics.
//! If the access collector ever stops looking inside subgroup operands, CSE
//! would silently miscompile -- this test fails first.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::fusion_cse::cse::cse;
use vyre_reference::value::Value;

fn buffers() -> Vec<BufferDecl> {
    // `out` is the single result buffer (V022: at most one is_output). `ctr` is
    // a read_write in-out buffer; it takes one input Value (overwritten by the
    // initial store) and the load+atomic hazard on it drives this test.
    vec![
        BufferDecl::output("out", 0, DataType::U32).with_count(1),
        BufferDecl::read_write("ctr", 1, DataType::U32).with_count(1),
    ]
}

/// One input Value for the non-output `ctr` buffer (overwritten by the store).
fn inputs() -> [Value; 1] {
    [Value::from(0u32.to_le_bytes().to_vec())]
}

/// The hazardous, barrier-less form: a load of `ctr` followed by an atomic on
/// `ctr` whose access is hidden inside a `subgroup_add` operand.
fn program_without_barrier() -> Program {
    Program::wrapped(
        buffers(),
        [1, 1, 1],
        vec![
            Node::store("ctr", Expr::u32(0), Expr::u32(5)),
            Node::let_bind("a", Expr::load("ctr", Expr::u32(0))),
            Node::if_then_else(
                Expr::gt(
                    Expr::subgroup_add(Expr::atomic_add("ctr", Expr::u32(0), Expr::u32(1))),
                    Expr::u32(0),
                ),
                vec![
                    Node::let_bind("b", Expr::load("ctr", Expr::u32(0))),
                    Node::store("out", Expr::u32(0), Expr::var("b")),
                ],
                vec![],
            ),
        ],
    )
}

/// Same program with an explicit `Node::barrier()` separating the read path
/// from the atomic path -- the only legal way to express this access pattern.
fn program_with_barrier() -> Program {
    Program::wrapped(
        buffers(),
        [1, 1, 1],
        vec![
            Node::store("ctr", Expr::u32(0), Expr::u32(5)),
            Node::let_bind("a", Expr::load("ctr", Expr::u32(0))),
            Node::barrier(),
            Node::if_then_else(
                Expr::gt(
                    Expr::subgroup_add(Expr::atomic_add("ctr", Expr::u32(0), Expr::u32(1))),
                    Expr::u32(0),
                ),
                vec![
                    Node::let_bind("b", Expr::load("ctr", Expr::u32(0))),
                    Node::store("out", Expr::u32(0), Expr::var("b")),
                ],
                vec![],
            ),
        ],
    )
}

#[test]
fn subgroup_hidden_atomic_after_load_is_rejected_without_a_barrier() {
    let program = program_without_barrier();

    let err = vyre_reference::reference_eval(&program, &inputs())
        .expect_err("a non-atomic load + subgroup-hidden atomic on the same buffer must be rejected");
    let message = format!("{err:?}");
    assert!(
        message.contains("fusion hazard on buffer `ctr`"),
        "the validator must see the atomic HIDDEN inside the subgroup operand \
         and reject the missing barrier; got: {message}",
    );
}

#[test]
fn barrier_makes_the_form_valid_and_cse_preserves_semantics() {
    let program = program_with_barrier();

    // With the barrier the program is valid. The barrier clears CSE's observed
    // state, so the post-atomic `load(ctr,0)` is NOT aliased to the stale `a`.
    let original = vyre_reference::reference_eval(&program, &inputs())
        .expect("with a barrier the access pattern is valid and must run");
    // `out` (the single result buffer) captures the post-increment load == 6.
    assert!(
        original.contains(&Value::from(6u32.to_le_bytes().to_vec())),
        "out == load(ctr) after the atomic increment == 6; got {original:?}",
    );

    let optimized = cse(program);
    let after = vyre_reference::reference_eval(&optimized, &inputs())
        .expect("CSE-optimized program must still validate and run");
    assert_eq!(
        after, original,
        "the barrier clears CSE observed state, so CSE must not alias the \
         post-atomic load to the pre-atomic `a`",
    );
}

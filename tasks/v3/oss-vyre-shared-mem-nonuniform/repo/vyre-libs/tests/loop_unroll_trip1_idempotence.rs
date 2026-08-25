//! Regression for FINDING-OPT-IDEM-1: `pre_lowering::optimize` was non-idempotent
//! for `scallop_join_wide::consumer_b`. The op's write-back `__sjw_chunk` loop has
//! a build-time-constant trip count of 1, but its body cost exceeded
//! `loop_unroll`'s size cap, so the trip-1 Loop->Block promotion only fired on the
//! *second* optimize() (after phase-3 CSE/DCE shrank the body). The fix lifts the
//! size cap for trip_count == 1 (no duplication => no blowup), so the promotion
//! happens on the first pass and optimize() is idempotent.

use vyre::ir::{Expr, Node};
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::harness::all_entries;

fn is_trip1(from: &Expr, to: &Expr) -> bool {
    matches!((from, to), (Expr::LitU32(0), Expr::LitU32(1)))
}

fn body_has_assign(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Assign { .. } => true,
        Node::If { then, otherwise, .. } => body_has_assign(then) || body_has_assign(otherwise),
        Node::Loop { body, .. } | Node::Block(body) => body_has_assign(body),
        Node::Region { body, .. } => body_has_assign(body),
        _ => false,
    })
}

/// (assign-free trip-1 loops, assign-bearing trip-1 loops).
fn count_trip1_loops(nodes: &[Node], free: &mut u32, with_assign: &mut u32) {
    for node in nodes {
        match node {
            Node::Loop { from, to, body, .. } => {
                if is_trip1(from, to) {
                    if body_has_assign(body) {
                        *with_assign += 1;
                    } else {
                        *free += 1;
                    }
                }
                count_trip1_loops(body, free, with_assign);
            }
            Node::If { then, otherwise, .. } => {
                count_trip1_loops(then, free, with_assign);
                count_trip1_loops(otherwise, free, with_assign);
            }
            Node::Block(body) => count_trip1_loops(body, free, with_assign),
            Node::Region { body, .. } => count_trip1_loops(body, free, with_assign),
            _ => {}
        }
    }
}

#[test]
fn scallop_join_wide_consumer_b_optimize_is_idempotent() {
    let entry = all_entries()
        .find(|e| e.id == "vyre-libs::catalog::math::scallop_join_wide::consumer_b")
        .expect("consumer_b entry must be registered");
    let program = (entry.build)();

    let once = optimize(program.clone());
    let twice = optimize(once.clone());

    // The harness invariant: a single optimize() must reach a fixpoint.
    assert_eq!(
        once, twice,
        "optimize(optimize(p)) must equal optimize(p) for scallop_join_wide::consumer_b"
    );

    // Pin the fix's effect: after ONE optimize, the unroll-eligible (assign-free)
    // trip-count-1 write-back loop must already be promoted to a Block; the
    // assign-bearing transfer loop is correctly left as a Loop (unrolling it would
    // duplicate a mutating accumulator).
    let mut free = 0u32;
    let mut with_assign = 0u32;
    count_trip1_loops(once.entry(), &mut free, &mut with_assign);
    assert_eq!(
        free, 0,
        "every assign-free trip-count-1 loop must be promoted to a Block on the first optimize pass"
    );
    assert_eq!(
        with_assign, 1,
        "the assign-bearing transfer __sjw_chunk loop must remain a Loop (not unrolled)"
    );
}

//! Integration test crate for the containing Vyre package.

use crate::ir::{Expr, Node};
use crate::optimizer::passes::fusion_cse::dce::const_loop_empty;
use crate::optimizer::passes::fusion_cse::dce::const_truth;
use crate::optimizer::passes::fusion_cse::dce::eliminate_unreachable;
use crate::optimizer::passes::fusion_cse::dce::reachable_prefix;

#[test]
fn test_const_truth_evaluation() {
    assert_eq!(const_truth(&Expr::bool(true)), Some(true));
    assert_eq!(const_truth(&Expr::bool(false)), Some(false));
    assert_eq!(const_truth(&Expr::u32(1)), Some(true));
    assert_eq!(const_truth(&Expr::u32(0)), Some(false));
    assert_eq!(const_truth(&Expr::i32(-1)), Some(true));
    assert_eq!(const_truth(&Expr::i32(0)), Some(false));
    assert_eq!(const_truth(&Expr::var("x")), None);
}

#[test]
fn test_const_loop_empty_evaluation() {
    assert!(const_loop_empty(&Expr::u32(10), &Expr::u32(10)));
    assert!(const_loop_empty(&Expr::u32(10), &Expr::u32(0)));
    assert!(!const_loop_empty(&Expr::u32(0), &Expr::u32(10)));
    assert!(!const_loop_empty(&Expr::var("a"), &Expr::u32(10)));
}

#[test]
fn test_reachable_prefix_truncates_after_return() {
    let nodes = vec![
        Node::let_bind("a", Expr::u32(1)),
        Node::Return,
        Node::let_bind("b", Expr::u32(2)),
    ];
    let prefix = reachable_prefix(&nodes);
    assert_eq!(prefix.len(), 2);
    assert!(matches!(prefix[1], Node::Return));
}

#[test]
fn test_reachable_prefix_no_return() {
    let nodes = vec![
        Node::let_bind("a", Expr::u32(1)),
        Node::let_bind("b", Expr::u32(2)),
    ];
    let prefix = reachable_prefix(&nodes);
    assert_eq!(prefix.len(), 2);
}

#[test]
fn test_eliminate_unreachable_folds_if_true() {
    let nodes = vec![Node::if_then_else(
        Expr::bool(true),
        vec![Node::let_bind("a", Expr::u32(1))],
        vec![Node::let_bind("b", Expr::u32(2))],
    )];
    let folded = eliminate_unreachable(nodes);
    assert_eq!(folded.len(), 1);
    assert!(matches!(&folded[0], Node::Let { name, .. } if name == "a"));
}

#[test]
fn test_eliminate_unreachable_folds_if_false() {
    let nodes = vec![Node::if_then_else(
        Expr::bool(false),
        vec![Node::let_bind("a", Expr::u32(1))],
        vec![Node::let_bind("b", Expr::u32(2))],
    )];
    let folded = eliminate_unreachable(nodes);
    assert_eq!(folded.len(), 1);
    assert!(matches!(&folded[0], Node::Let { name, .. } if name == "b"));
}

#[test]
fn test_eliminate_unreachable_preserves_unknown_if() {
    let nodes = vec![Node::if_then_else(
        Expr::var("cond"),
        vec![Node::let_bind("a", Expr::u32(1))],
        vec![Node::let_bind("b", Expr::u32(2))],
    )];
    let folded = eliminate_unreachable(nodes);
    assert_eq!(folded.len(), 1);
    assert!(matches!(&folded[0], Node::If { .. }));
}

#[test]
fn test_eliminate_unreachable_drops_empty_loop() {
    let nodes = vec![
        Node::let_bind("start", Expr::u32(1)),
        Node::loop_for(
            "i",
            Expr::u32(10),
            Expr::u32(0),
            vec![Node::let_bind("x", Expr::u32(1))],
        ),
        Node::let_bind("end", Expr::u32(2)),
    ];
    let folded = eliminate_unreachable(nodes);
    assert_eq!(folded.len(), 2);
    assert!(matches!(&folded[0], Node::Let { name, .. } if name == "start"));
    assert!(matches!(&folded[1], Node::Let { name, .. } if name == "end"));
}

#[test]
fn test_eliminate_unreachable_preserves_valid_loop() {
    let nodes = vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(10),
        vec![Node::let_bind("x", Expr::u32(1))],
    )];
    let folded = eliminate_unreachable(nodes);
    assert_eq!(folded.len(), 1);
    assert!(matches!(&folded[0], Node::Loop { .. }));
}

#[test]
fn test_eliminate_unreachable_truncates_after_return() {
    let nodes = vec![
        Node::let_bind("a", Expr::u32(1)),
        Node::Return,
        Node::let_bind("b", Expr::u32(2)),
    ];
    let folded = eliminate_unreachable(nodes);
    assert_eq!(folded.len(), 2);
    assert!(matches!(&folded[0], Node::Let { name, .. } if name == "a"));
    assert!(matches!(&folded[1], Node::Return));
}

#[test]
fn test_eliminate_unreachable_removes_empty_blocks() {
    let nodes = vec![Node::block(vec![]), Node::let_bind("a", Expr::u32(1))];
    let folded = eliminate_unreachable(nodes);
    assert_eq!(folded.len(), 1);
    assert!(matches!(&folded[0], Node::Let { name, .. } if name == "a"));
}

#[test]
fn test_eliminate_dead_lets_removes_dead_let() {
    let nodes = vec![
        Node::let_bind("dead", Expr::u32(1)),
        Node::let_bind("alive", Expr::u32(2)),
        Node::store("out", Expr::u32(0), Expr::var("alive")),
    ];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(result.nodes.len(), 2);
    assert!(matches!(&result.nodes[0], Node::Let { name, .. } if name == "alive"));
}

#[test]
fn test_eliminate_dead_lets_preserves_effectful_let() {
    let nodes = vec![Node::let_bind(
        "effectful",
        Expr::Atomic {
            op: crate::ir::AtomicOp::Add,
            buffer: "buf".into(),
            index: Box::new(Expr::u32(0)),
            value: Box::new(Expr::u32(1)),
            expected: None,
            ordering: crate::ir::MemoryOrdering::Relaxed,
        },
    )];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(result.nodes.len(), 1);
    assert!(matches!(&result.nodes[0], Node::Let { name, .. } if name == "effectful"));
}

#[test]
fn test_eliminate_dead_lets_computes_live_in() {
    let nodes = vec![Node::store("out", Expr::var("idx"), Expr::var("val"))];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert!(result.live_in.contains("idx"));
    assert!(result.live_in.contains("val"));
}

#[test]
fn test_async_load_offset_keeps_let_alive() {
    // Reproducer for the vfs::resolve cat_a_gpu_differential panic on
    // 2026-05-02: AsyncLoad's `offset` Expr was not walked for live
    // refs, so the upstream `Let("file_hash", ...)` was eliminated as
    // dead, leaving the AsyncLoad referencing an unbound name during
    // backend lowering.
    let nodes = vec![
        Node::let_bind("file_hash", Expr::load("hashes", Expr::u32(0))),
        Node::AsyncLoad {
            source: "src".into(),
            destination: "dst".into(),
            offset: Box::new(Expr::var("file_hash")),
            size: Box::new(Expr::u32(4096)),
            tag: "vfs_req".into(),
        },
    ];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(
        result.nodes.len(),
        2,
        "let_bind(file_hash) must survive DCE because AsyncLoad.offset reads it"
    );
    assert!(matches!(&result.nodes[0], Node::Let { name, .. } if name == "file_hash"));
    assert!(matches!(&result.nodes[1], Node::AsyncLoad { .. }));
}

#[test]
fn test_async_load_size_keeps_let_alive() {
    // Adversarial twin: same gap could exist on `size` independently
    // of `offset`  -  exercise size-only liveness so a future caller
    // who computes size dynamically (e.g. `Let("nbytes", ...); AsyncLoad
    // { size: var("nbytes") }`) also survives DCE.
    let nodes = vec![
        Node::let_bind("nbytes", Expr::u32(4096)),
        Node::AsyncLoad {
            source: "src".into(),
            destination: "dst".into(),
            offset: Box::new(Expr::u32(0)),
            size: Box::new(Expr::var("nbytes")),
            tag: "vfs_req".into(),
        },
    ];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(result.nodes.len(), 2);
    assert!(matches!(&result.nodes[0], Node::Let { name, .. } if name == "nbytes"));
}

#[test]
fn test_async_store_offset_and_size_keep_lets_alive() {
    let nodes = vec![
        Node::let_bind("dst_off", Expr::u32(64)),
        Node::let_bind("nbytes", Expr::u32(4096)),
        Node::AsyncStore {
            source: "src".into(),
            destination: "dst".into(),
            offset: Box::new(Expr::var("dst_off")),
            size: Box::new(Expr::var("nbytes")),
            tag: "store_req".into(),
        },
    ];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(result.nodes.len(), 3);
    assert!(matches!(&result.nodes[0], Node::Let { name, .. } if name == "dst_off"));
    assert!(matches!(&result.nodes[1], Node::Let { name, .. } if name == "nbytes"));
}

#[test]
fn test_trap_address_keeps_let_alive() {
    // Same omission existed for Node::Trap, whose `address: Box<Expr>`
    // was not walked. A fault-handler that builds the trap address
    // dynamically (`Let("trap_addr", ...)`) would have lost its bind.
    let nodes = vec![
        Node::let_bind("trap_addr", Expr::u32(0xDEAD_BEEF)),
        Node::Trap {
            address: Box::new(Expr::var("trap_addr")),
            tag: "div_by_zero".into(),
        },
    ];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(result.nodes.len(), 2);
    assert!(matches!(&result.nodes[0], Node::Let { name, .. } if name == "trap_addr"));
}

// ──── ROADMAP A21: dead-load elimination ────────────────────

#[test]
fn dead_let_bound_to_load_is_eliminated() {
    // ROADMAP A21: a `Let` whose value is `Expr::Load { buffer, lit }`
    // and whose name is never read must be dropped by DCE. The Load
    // is treated as effect-free at the IR level (the buffer's contents
    // do not change between binding and use within the same dispatch
    // unless an Atomic / Async* / Store / Barrier intervenes), so the
    // bind+load are removed together. Proves A21 holds via the
    // existing DCE liveness machinery with no extra pass.
    let nodes = vec![
        Node::let_bind("dead_load", Expr::load("input", Expr::u32(0))),
        Node::let_bind("alive", Expr::u32(7)),
        Node::store("out", Expr::u32(0), Expr::var("alive")),
    ];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(
        result.nodes.len(),
        2,
        "dead Load-bound Let must be dropped; only `alive` and the Store remain"
    );
    assert!(matches!(&result.nodes[0], Node::Let { name, .. } if name == "alive"));
    assert!(matches!(&result.nodes[1], Node::Store { .. }));
}

#[test]
fn dead_let_bound_to_load_with_load_index_is_eliminated() {
    // Adversarial: the Load's index is itself a Load  -  neither Load
    // is read by anything live. DCE must drop both the bind and the
    // outer Load (and not panic on the nested Load). This exercises
    // the recursive `collect_expr_refs` walk through the index.
    let nodes = vec![
        Node::let_bind(
            "dead_chained_load",
            Expr::load("input", Expr::load("indirect", Expr::u32(0))),
        ),
        Node::store("out", Expr::u32(0), Expr::u32(7)),
    ];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(
        result.nodes.len(),
        1,
        "the chained dead Load + the bind both die; only the Store survives"
    );
}

#[test]
fn live_let_bound_to_load_is_kept() {
    // Negative twin: the Load is used by a downstream Store, so the
    // Let stays alive.
    let nodes = vec![
        Node::let_bind("live_load", Expr::load("input", Expr::u32(0))),
        Node::store("out", Expr::u32(0), Expr::var("live_load")),
    ];
    let result =
        crate::optimizer::passes::fusion_cse::dce::eliminate_dead_lets::eliminate_dead_lets(
            nodes,
            im::HashSet::new(),
        );
    assert_eq!(result.nodes.len(), 2);
    assert!(matches!(&result.nodes[0], Node::Let { name, .. } if name == "live_load"));
}

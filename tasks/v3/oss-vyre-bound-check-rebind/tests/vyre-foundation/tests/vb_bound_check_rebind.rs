//! Cross-pass adversarial coverage for induction-variable rebinds.
//!
//! A `Let`/`Assign` that shadows the loop var breaks every pass that treats
//! `Var(i)` as the `[from, to)` induction value. These tests drive the shared
//! helper through each consumer pass so a local-only fix to one pass cannot
//! leave the others green.

use vyre_foundation::ir::{Expr, Ident, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_lower_bound_normalize::LoopLowerBoundNormalize;
use vyre_foundation::optimizer::passes::loops::loop_redundant_bound_check_elide::LoopRedundantBoundCheckElidePass;
use vyre_foundation::optimizer::passes::loops::loop_strip_mine::LoopStripMine;
use vyre_foundation::optimizer::passes::loops::loop_unroll::LoopUnroll;
use vyre_foundation::optimizer::passes::loops::loop_var_range_fold::LoopVarRangeFoldPass;
use vyre_foundation::optimizer::{PassAnalysis, ProgramPass};

fn program_with_entry(entry: Vec<Node>) -> Program {
    Program::wrapped(vec![], [1, 1, 1], entry)
}

fn entry_body(program: &Program) -> &[Node] {
    match program.entry() {
        [Node::Region { body, .. }] => body.as_slice(),
        other => other,
    }
}

fn count_ifs(nodes: &[Node]) -> usize {
    let mut n = 0;
    for node in nodes {
        match node {
            Node::If { then, otherwise, .. } => {
                n += 1;
                n += count_ifs(then);
                n += count_ifs(otherwise);
            }
            Node::Block(b) | Node::Loop { body: b, .. } => n += count_ifs(b),
            Node::Region { body, .. } => n += count_ifs(body),
            _ => {}
        }
    }
    n
}

fn rebind_via_let_load() -> Vec<Node> {
    vec![
        Node::let_bind("i", Expr::load("buf", Expr::var("i"))),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(4)),
            vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
        ),
    ]
}

fn rebind_via_assign() -> Vec<Node> {
    vec![
        Node::assign("i", Expr::load("buf", Expr::var("i"))),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(4)),
            vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
        ),
    ]
}

#[test]
fn elide_keeps_guard_after_let_rebind() {
    let program = program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(4),
        body: rebind_via_let_load(),
    }]);
    let result = LoopRedundantBoundCheckElidePass::transform(program.clone());
    assert!(!result.changed);
    assert_eq!(count_ifs(entry_body(&result.program)), 1);
    assert_eq!(
        ProgramPass::analyze(&LoopRedundantBoundCheckElidePass, &program),
        PassAnalysis::SKIP
    );
}

#[test]
fn elide_keeps_guard_after_assign_rebind() {
    let program = program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(4),
        body: rebind_via_assign(),
    }]);
    let result = LoopRedundantBoundCheckElidePass::transform(program);
    assert!(
        !result.changed,
        "Assign rebind is as real as Let rebind — guard must stay"
    );
    assert_eq!(count_ifs(entry_body(&result.program)), 1);
}

#[test]
fn range_fold_skips_when_induction_rebound() {
    // Without the rebind, `i < 4` inside Loop(i,0,4) is always-true and folds.
    // With a Let rebind first, the comparison is no longer about the induction
    // range and must survive.
    let program = program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(4),
        body: rebind_via_let_load(),
    }]);
    let result = LoopVarRangeFoldPass::transform(program);
    assert!(
        !result.changed,
        "range fold must not treat a rebound Var(i) as the loop range"
    );
    assert_eq!(count_ifs(entry_body(&result.program)), 1);
}

#[test]
fn lower_bound_normalize_skips_when_induction_rebound() {
    // Loop(i, 4, 12, [let i = load; store(i)]) must NOT rewrite from→0: the
    // subsequent uses of i are the loaded value, not induction+offset.
    let program = program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(4),
        to: Expr::u32(12),
        body: vec![
            Node::let_bind("i", Expr::load("buf", Expr::var("i"))),
            Node::store("buf", Expr::var("i"), Expr::u32(1)),
        ],
    }]);
    let result = LoopLowerBoundNormalize::transform(program);
    assert!(
        !result.changed,
        "lower-bound normalize must decline when the body rebinds the loop var"
    );
}

#[test]
fn strip_mine_skips_when_induction_rebound() {
    let program = program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(32),
        body: vec![Node::let_bind("i", Expr::u32(7))],
    }]);
    let result = LoopStripMine::transform(program);
    assert!(!result.changed);
}

#[test]
fn unroll_skips_when_induction_written() {
    // Small trip count would otherwise unroll; a Let to `i` must block it.
    let program = program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(3),
        body: vec![
            Node::let_bind("i", Expr::u32(0)),
            Node::store("buf", Expr::var("i"), Expr::u32(1)),
        ],
    }]);
    let before = program.clone();
    let result = LoopUnroll::transform(program);
    assert!(
        !result.changed || result.program == before,
        "unroll must not expand a loop whose body writes the induction var"
    );
    // Stronger: the Loop node must still exist (not be replaced by 3 stores).
    let still_has_loop = entry_body(&result.program)
        .iter()
        .any(|n| matches!(n, Node::Loop { .. }) || {
            if let Node::Block(b) = n {
                b.iter().any(|c| matches!(c, Node::Loop { .. }))
            } else {
                false
            }
        });
    assert!(
        still_has_loop
            || entry_body(&result.program)
                .iter()
                .any(|n| matches!(n, Node::Loop { .. })),
        "loop with induction write must remain a Loop, not an unrolled splat; got {:?}",
        entry_body(&result.program)
    );
}

#[test]
fn nested_same_name_loop_blocks_range_fold_on_outer() {
    // Stricter body_rebinds_var: nested Loop(i, ...) counts as a rebind for
    // range_fold even though the inner writes are scoped.
    let inner = Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(2),
        body: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
    };
    let program = program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(8),
        body: vec![
            inner,
            Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(8)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(2))],
            ),
        ],
    }]);
    let result = LoopVarRangeFoldPass::transform(program);
    // Outer `i < 8` must not fold away while a nested same-name loop exists.
    assert_eq!(
        count_ifs(entry_body(&result.program)),
        1,
        "nested same-name loop must keep outer range fold conservative"
    );
}

fn loop_with_body(var: &str, to: u32, body: Vec<Node>) -> Node {
    Node::Loop {
        var: Ident::from(var),
        from: Expr::u32(0),
        to: Expr::u32(to),
        body,
    }
}

#[test]
fn still_elides_true_redundant_guard_on_stable_induction() {
    // No rebind: `if i < 4` inside Loop(i,0,4) is a redundant range re-check.
    let entry = vec![loop_with_body(
        "i",
        4,
        vec![Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(4)),
            vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
        )],
    )];
    let program = program_with_entry(entry);
    let result = LoopRedundantBoundCheckElidePass::transform(program);
    assert!(
        result.changed,
        "stable induction + matching upper bound should still elide"
    );
    assert_eq!(
        count_ifs(entry_body(&result.program)),
        0,
        "redundant guard should be gone when induction is stable"
    );
}


#[test]
fn rebind_keeps_store_under_if() {
    let entry = vec![loop_with_body(
        "i",
        4,
        vec![
            Node::let_bind("i", Expr::load("buf", Expr::var("i"))),
            Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(4)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            ),
        ],
    )];
    let result = LoopRedundantBoundCheckElidePass::transform(program_with_entry(entry));
    // Walk: every Store must be nested under an If (not floated to the loop body).
    fn stores_outside_if(nodes: &[Node]) -> usize {
        let mut bad = 0;
        for n in nodes {
            match n {
                Node::Store { .. } => bad += 1,
                Node::If { .. } => {}
                Node::Block(b) | Node::Loop { body: b, .. } => {
                    bad += stores_outside_if(b);
                }
                Node::Region { body, .. } => {
                    bad += stores_outside_if(body);
                }
                _ => {}
            }
        }
        bad
    }
    assert_eq!(
        stores_outside_if(entry_body(&result.program)),
        0,
        "store must remain under the bounds-check If, not become unconditional"
    );
}

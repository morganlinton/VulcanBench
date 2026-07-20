//! Hidden tests: bound-check elision must not drop guards after induction rebind.
use vyre_foundation::ir::{Expr, Ident, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_redundant_bound_check_elide::LoopRedundantBoundCheckElidePass;
use vyre_foundation::optimizer::{PassAnalysis, ProgramPass};

fn program_with_entry(entry: Vec<Node>) -> Program {
    Program::wrapped(vec![], [1, 1, 1], entry)
}

fn loop_with_body(var: &str, to: u32, body: Vec<Node>) -> Node {
    Node::Loop {
        var: Ident::from(var),
        from: Expr::u32(0),
        to: Expr::u32(to),
        body,
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

#[test]
fn does_not_elide_guard_after_rebind() {
    let entry = vec![loop_with_body(
        "i",
        4,
        vec![
            Node::let_bind(
                "i",
                Expr::Load {
                    buffer: Ident::from("buf"),
                    index: Box::new(Expr::var("i")),
                },
            ),
            Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(4)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            ),
        ],
    )];
    let program = program_with_entry(entry);
    let result = LoopRedundantBoundCheckElidePass::transform(program.clone());
    assert!(
        !result.changed,
        "guard after rebinding the loop var is a real bounds check"
    );
    assert_eq!(
        count_ifs(result.program.entry()),
        1,
        "the bounds-check If must remain"
    );
}

#[test]
fn analyze_skips_when_body_rebinds() {
    let entry = vec![loop_with_body(
        "i",
        4,
        vec![
            Node::let_bind(
                "i",
                Expr::Load {
                    buffer: Ident::from("buf"),
                    index: Box::new(Expr::var("i")),
                },
            ),
            Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(4)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            ),
        ],
    )];
    let program = program_with_entry(entry);
    assert_eq!(
        ProgramPass::analyze(&LoopRedundantBoundCheckElidePass, &program),
        PassAnalysis::SKIP,
        "analyze must agree there is nothing redundant to elide"
    );
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
        count_ifs(result.program.entry()),
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
            Node::let_bind(
                "i",
                Expr::Load {
                    buffer: Ident::from("buf"),
                    index: Box::new(Expr::var("i")),
                },
            ),
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
        stores_outside_if(result.program.entry()),
        0,
        "store must remain under the bounds-check If, not become unconditional"
    );
}

//! Hidden fail_to_pass / pass_to_pass for loop peel first-iteration materialization.
use vyre_foundation::ir::{Expr, Ident, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_peel::LoopPeelPass;

fn program_with_entry(entry: Vec<Node>) -> Program {
    Program::wrapped(vec![], [1, 1, 1], entry)
}

fn store_pairs(nodes: &[Node]) -> Vec<(Expr, Expr)> {
    let mut out = Vec::new();
    for n in nodes {
        match n {
            Node::Store { index, value, .. } => out.push(((*index).clone(), (*value).clone())),
            Node::Block(b) => out.extend(store_pairs(b)),
            Node::Region { body, .. } => out.extend(store_pairs(body)),
            Node::If { then, otherwise, .. } => {
                out.extend(store_pairs(then));
                out.extend(store_pairs(otherwise));
            }
            Node::Loop { body, .. } => out.extend(store_pairs(body)),
            _ => {}
        }
    }
    out
}

fn find_loop(nodes: &[Node]) -> Option<(&Expr, &[Node])> {
    for n in nodes {
        match n {
            Node::Loop { from, body, .. } => return Some((from, body)),
            Node::Block(b) => {
                if let Some(x) = find_loop(b) {
                    return Some(x);
                }
            }
            Node::Region { body, .. } => {
                if let Some(x) = find_loop(body) {
                    return Some(x);
                }
            }
            _ => {}
        }
    }
    None
}

fn entry_body(program: &Program) -> Vec<Node> {
    program.entry().to_vec()
}

fn peel_fixture() -> Program {
    let guard = Node::If {
        cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
        then: vec![Node::store("buf", Expr::var("i"), Expr::u32(99))],
        otherwise: vec![],
    };
    let rest = Node::store("buf", Expr::var("i"), Expr::u32(7));
    let entry = vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(10),
        body: vec![guard, rest],
    }];
    program_with_entry(entry)
}

#[test]
fn peel_prologue_includes_rest_at_i_zero() {
    let result = LoopPeelPass::transform(peel_fixture());
    assert!(result.changed, "peeling must fire");
    let body = entry_body(&result.program);
    let pairs = store_pairs(&body);
    assert!(
        pairs.len() >= 2,
        "prologue must include then and rest stores; got {pairs:?}"
    );
    assert_eq!(
        &pairs[0],
        &(Expr::u32(0), Expr::u32(99)),
        "first-iteration then-arm must store with i substituted to 0"
    );
    assert_eq!(
        &pairs[1],
        &(Expr::u32(0), Expr::u32(7)),
        "first-iteration rest must also run at i = 0, not be dropped"
    );
}

#[test]
fn peel_substitutes_induction_var_in_prologue() {
    let result = LoopPeelPass::transform(peel_fixture());
    let body = entry_body(&result.program);
    let pairs = store_pairs(&body);
    assert!(
        !matches!(pairs[0].0, Expr::Var(_)),
        "prologue must not leave a stale Var(i) as the store index"
    );
    assert!(
        !matches!(pairs[1].0, Expr::Var(_)),
        "prologue rest store must use the peeled constant index, not Var(i)"
    );
}

#[test]
fn peel_remainder_starts_at_one_with_rest() {
    let result = LoopPeelPass::transform(peel_fixture());
    let body = entry_body(&result.program);
    let (from, lbody) = find_loop(&body).expect("remainder loop present");
    assert_eq!(from, &Expr::u32(1), "remainder loop starts at i = 1");
    assert_eq!(
        store_pairs(lbody),
        vec![(Expr::var("i"), Expr::u32(7))],
        "remainder keeps rest with the induction variable"
    );
}

#[test]
fn peel_still_skips_non_zero_from() {
    let entry = vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(2),
        to: Expr::u32(10),
        body: vec![
            Node::If {
                cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
                then: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
                otherwise: vec![],
            },
            Node::store("buf", Expr::var("i"), Expr::u32(2)),
        ],
    }];
    let result = LoopPeelPass::transform(program_with_entry(entry));
    assert!(!result.changed, "non-zero from must not peel");
}

#[test]
fn peel_full_first_iteration_store_sequence() {
    let result = LoopPeelPass::transform(peel_fixture());
    assert!(result.changed);
    let body = entry_body(&result.program);
    assert_eq!(
        store_pairs(&body),
        vec![
            (Expr::u32(0), Expr::u32(99)),
            (Expr::u32(0), Expr::u32(7)),
            (Expr::var("i"), Expr::u32(7)),
        ],
        "prologue then++rest at i=0, then remainder rest with Var(i)"
    );
}

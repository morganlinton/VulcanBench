//! Adversarial coverage for loop peel first-iteration materialization.
//!
//! Locks down shapes beyond the single then+rest fixture: multiple trailing
//! statements, nested control flow in the remainder, and induction uses inside
//! non-store expressions in the peeled then-arm.

use vyre_foundation::ir::{Expr, Ident, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_peel::LoopPeelPass;

fn program_with_entry(entry: Vec<Node>) -> Program {
    Program::wrapped(vec![], [1, 1, 1], entry)
}

fn entry_body(program: &Program) -> &[Node] {
    // Program::wrapped places the body under a Region.
    match program.entry() {
        [Node::Region { body, .. }] => body.as_slice(),
        other => other,
    }
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

fn peel_then_plus_rest(rest: Vec<Node>) -> Program {
    let guard = Node::If {
        cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
        then: vec![Node::store("buf", Expr::var("i"), Expr::u32(99))],
        otherwise: vec![],
    };
    let mut body = vec![guard];
    body.extend(rest);
    program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(10),
        body,
    }])
}

#[test]
fn peel_materializes_multiple_trailing_rest_stores_at_i_zero() {
    let program = peel_then_plus_rest(vec![
        Node::store("buf", Expr::var("i"), Expr::u32(7)),
        Node::store("buf", Expr::var("i"), Expr::u32(8)),
    ]);
    let result = LoopPeelPass::transform(program);
    assert!(result.changed, "peeling must fire");
    assert_eq!(
        store_pairs(entry_body(&result.program)),
        vec![
            (Expr::u32(0), Expr::u32(99)),
            (Expr::u32(0), Expr::u32(7)),
            (Expr::u32(0), Expr::u32(8)),
            (Expr::var("i"), Expr::u32(7)),
            (Expr::var("i"), Expr::u32(8)),
        ],
        "every trailing rest stmt must run once in the prologue at i=0 and stay in the remainder"
    );
}

#[test]
fn peel_substitutes_induction_inside_nested_if_in_rest() {
    // rest contains If(true, [store(i, 3)], []) — the nested store still uses Var(i)
    // and must become LitU32(0) in the prologue.
    let nested = Node::If {
        cond: Expr::bool(true),
        then: vec![Node::store("buf", Expr::var("i"), Expr::u32(3))],
        otherwise: vec![],
    };
    let program = peel_then_plus_rest(vec![nested]);
    let result = LoopPeelPass::transform(program);
    assert!(result.changed);
    let pairs = store_pairs(entry_body(&result.program));
    assert!(
        pairs
            .iter()
            .any(|(idx, val)| *idx == Expr::u32(0) && *val == Expr::u32(3)),
        "nested rest store must appear in prologue with i substituted to 0; got {pairs:?}"
    );
    assert!(
        pairs
            .iter()
            .any(|(idx, val)| matches!(idx, Expr::Var(_)) && *val == Expr::u32(3)),
        "remainder must keep the nested rest store under Var(i); got {pairs:?}"
    );
}

#[test]
fn peel_substitutes_induction_in_then_arm_arithmetic_index() {
    // then stores at (i + 1). Peel substitutes only; it does not const-fold, so
    // the prologue index is Add(0, 1) — never a leftover Var(i).
    let guard = Node::If {
        cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
        then: vec![Node::store(
            "buf",
            Expr::add(Expr::var("i"), Expr::u32(1)),
            Expr::u32(99),
        )],
        otherwise: vec![],
    };
    let rest = Node::store("buf", Expr::var("i"), Expr::u32(7));
    let program = program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(4),
        body: vec![guard, rest],
    }]);
    let result = LoopPeelPass::transform(program);
    assert!(result.changed);
    let pairs = store_pairs(entry_body(&result.program));
    assert_eq!(
        &pairs[0],
        &(Expr::add(Expr::u32(0), Expr::u32(1)), Expr::u32(99)),
        "then-arm index must substitute i:=0 inside Add; got {pairs:?}"
    );
    assert!(
        !matches!(pairs[0].0, Expr::Var(_))
            && !matches!(&pairs[0].0, Expr::BinOp { left, .. } if matches!(left.as_ref(), Expr::Var(_))),
        "prologue must not leave a stale Var(i) in the arithmetic index"
    );
    assert_eq!(&pairs[1], &(Expr::u32(0), Expr::u32(7)));
}

#[test]
fn peel_remainder_keeps_from_one_after_multi_rest() {
    let program = peel_then_plus_rest(vec![
        Node::store("buf", Expr::var("i"), Expr::u32(7)),
        Node::store("buf", Expr::var("i"), Expr::u32(8)),
    ]);
    let result = LoopPeelPass::transform(program);
    let (from, lbody) = find_loop(entry_body(&result.program)).expect("remainder loop");
    assert_eq!(from, &Expr::u32(1));
    assert_eq!(
        store_pairs(lbody),
        vec![
            (Expr::var("i"), Expr::u32(7)),
            (Expr::var("i"), Expr::u32(8)),
        ]
    );
}

#[test]
fn peel_accepts_swapped_eq_operands() {
    // Eq(LitU32(0), Var(i)) must peel the same as Eq(Var(i), LitU32(0)).
    let guard = Node::If {
        cond: Expr::eq(Expr::u32(0), Expr::var("i")),
        then: vec![Node::store("buf", Expr::var("i"), Expr::u32(99))],
        otherwise: vec![],
    };
    let rest = Node::store("buf", Expr::var("i"), Expr::u32(7));
    let result = LoopPeelPass::transform(program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(5),
        body: vec![guard, rest],
    }]));
    assert!(result.changed);
    assert_eq!(
        store_pairs(entry_body(&result.program)),
        vec![
            (Expr::u32(0), Expr::u32(99)),
            (Expr::u32(0), Expr::u32(7)),
            (Expr::var("i"), Expr::u32(7)),
        ]
    );
}

#[test]
fn peel_skips_when_otherwise_arm_is_nonempty() {
    let guard = Node::If {
        cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
        then: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
        otherwise: vec![Node::store("buf", Expr::var("i"), Expr::u32(2))],
    };
    let result = LoopPeelPass::transform(program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(4),
        body: vec![guard, Node::store("buf", Expr::var("i"), Expr::u32(3))],
    }]));
    assert!(!result.changed, "nonempty else-arm is not the peel pattern");
}

#[test]
fn peel_skips_when_rest_rebinds_induction() {
    let result = LoopPeelPass::transform(peel_then_plus_rest(vec![
        Node::let_bind("i", Expr::u32(0)),
        Node::store("buf", Expr::var("i"), Expr::u32(7)),
    ]));
    assert!(
        !result.changed,
        "rest that rebinds the loop var must refuse peel (substitution would corrupt)"
    );
}

#[test]
fn peel_skips_trip_count_one() {
    let guard = Node::If {
        cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
        then: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
        otherwise: vec![],
    };
    let result = LoopPeelPass::transform(program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(1),
        body: vec![guard],
    }]));
    assert!(!result.changed, "N<=1 has no remainder to peel into");
}

#[test]
fn peel_skips_guard_on_nonzero_literal() {
    let guard = Node::If {
        cond: Expr::eq(Expr::var("i"), Expr::u32(1)),
        then: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
        otherwise: vec![],
    };
    let result = LoopPeelPass::transform(program_with_entry(vec![Node::Loop {
        var: Ident::from("i"),
        from: Expr::u32(0),
        to: Expr::u32(4),
        body: vec![guard, Node::store("buf", Expr::var("i"), Expr::u32(2))],
    }]));
    assert!(!result.changed, "first-iter guard must be Eq(i, 0), not Eq(i, 1)");
}

#[test]
fn peel_preserves_reference_eval_on_buffer_stores() {
    use vyre_foundation::ir::{BufferDecl, DataType};
    use vyre_reference::value::Value;

    // out[i] = 10 on first iter via guard, out[i] = 1 on every iter via rest.
    // For N=3 expected: out = [10, 1, 1] wait — both then and rest store on i=0,
    // so out[0] ends as rest's value 1, then iters 1,2 store 1. So [1,1,1].
    // Stronger observable: then stores 99 at i, rest stores 7 at i.
    // Final: out[0]=7 (rest overwrites then), out[1]=7, out[2]=7.
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(3)],
        [1, 1, 1],
        vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(3),
            body: vec![
                Node::If {
                    cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
                    then: vec![Node::store("out", Expr::var("i"), Expr::u32(99))],
                    otherwise: vec![],
                },
                Node::store("out", Expr::var("i"), Expr::u32(7)),
            ],
        }],
    );
    let inputs: [Value; 0] = [];
    let before = vyre_reference::reference_eval(&program, &inputs)
        .expect("original peel fixture must evaluate");
    let peeled = LoopPeelPass::transform(program);
    assert!(peeled.changed);
    let after = vyre_reference::reference_eval(&peeled.program, &inputs)
        .expect("peeled program must evaluate");
    assert_eq!(
        before, after,
        "peel must be observationally identical under reference_eval"
    );
    assert_eq!(
        before,
        vec![Value::from(7u32.to_le_bytes().to_vec().repeat(3))],
        "sanity: final buffer is three 7s (rest overwrites then on i=0)"
    );
}

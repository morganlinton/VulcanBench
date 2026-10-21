use crate::ir::{Expr, Ident, Node};
use crate::visit::bound_names::{collect_bound_names, count_bound_names};
use rustc_hash::{FxHashMap, FxHashSet};

#[test]
fn counts_let_and_loop_var_bindings_recursively() {
    // let a; loop b { let c }; if cond { let a } else { let d }
    let nodes = vec![
        Node::let_bind("a", Expr::u32(0)),
        Node::loop_for(
            "b",
            Expr::u32(0),
            Expr::u32(2),
            vec![Node::let_bind("c", Expr::u32(1))],
        ),
        Node::if_then_else(
            Expr::bool(true),
            vec![Node::let_bind("a", Expr::u32(2))],
            vec![Node::let_bind("d", Expr::u32(3))],
        ),
    ];

    let mut counts = FxHashMap::default();
    count_bound_names(&nodes, &mut counts);
    assert_eq!(
        counts.get("a").copied(),
        Some(2),
        "`a` is bound at top level AND in the then-arm"
    );
    assert_eq!(
        counts.get("b").copied(),
        Some(1),
        "loop variable counts as a binding"
    );
    assert_eq!(
        counts.get("c").copied(),
        Some(1),
        "binding inside the loop body counts"
    );
    assert_eq!(counts.get("d").copied(), Some(1));
    assert_eq!(counts.get("missing").copied(), None);

    let mut set = FxHashSet::default();
    collect_bound_names(&nodes, &mut set);
    assert!(
        ["a", "b", "c", "d"]
            .iter()
            .all(|n| set.contains(&Ident::from(*n))),
        "collect_bound_names captures every bound name"
    );
    assert_eq!(set.len(), 4, "collect dedups the two `a` bindings");
}

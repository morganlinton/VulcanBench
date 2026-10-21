use super::{eliminate_dead_lets, eliminate_unreachable};
use crate::ir::{Ident, Program};
use im::HashSet;

/// Remove unreachable statements and unused pure `let` bindings.
#[must_use]
#[inline]
pub fn dce(program: Program) -> Program {
    program.map_entry(|entry| {
        let entry = eliminate_dead_lets(entry, HashSet::<Ident>::new()).nodes;
        let entry = eliminate_unreachable(entry);
        eliminate_dead_lets(entry, HashSet::<Ident>::new()).nodes
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Expr, Node};
    use std::sync::Arc;

    #[test]
    fn dce_descends_into_region_bodies() {
        let program = Program::wrapped(
            Vec::new(),
            [1, 1, 1],
            vec![Node::Region {
                generator: "test".into(),
                source_region: None,
                body: Arc::new(vec![
                    Node::let_bind("dead", Expr::u32(1)),
                    Node::Return,
                    Node::let_bind("unreachable", Expr::u32(2)),
                ]),
            }],
        );

        let optimized = dce(program);
        let [Node::Region { body, .. }] = optimized.entry() else {
            panic!("Fix: DCE must preserve the Region wrapper while optimizing its body");
        };
        assert_eq!(body.as_slice(), &[Node::Return]);
    }

    #[test]
    fn dce_region_live_ins_propagate_to_outer_scope() {
        let program = Program::wrapped(
            Vec::new(),
            [1, 1, 1],
            vec![
                Node::let_bind("live", Expr::u32(7)),
                Node::Region {
                    generator: "test".into(),
                    source_region: None,
                    body: Arc::new(vec![Node::store("out", Expr::u32(0), Expr::var("live"))]),
                },
            ],
        );

        let optimized = dce(program);
        let [Node::Region { body, .. }] = optimized.entry() else {
            panic!("Fix: Program::wrapped must keep the root Region");
        };
        assert!(
            matches!(body.first(), Some(Node::Let { name, .. }) if name.as_str() == "live"),
            "Fix: variables read inside a Region must keep their outer definitions live: {:?}",
            body
        );
    }
}

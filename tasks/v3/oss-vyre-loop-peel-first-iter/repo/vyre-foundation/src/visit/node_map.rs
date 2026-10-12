//! Owning child-recursive `Node` map: walks one level into `If`, `Loop`,
//! `Block`, `Region` bodies and rebuilds the node with each child mapped
//! through the caller's transform.
//!
//! Audit cleanup A5 (2026-04-30): hoisted from per-pass copies. The
//! cleanup catalog (`empty_block_collapse`, `region_promote_singleton_block`,
//! `loop_trip_zero_eliminate`, `if_constant_branch_eliminate`,
//! `noop_assign_eliminate`, `loop_redundant_bound_check_elide`) used to
//! carry near-identical 4-arm `If/Loop/Block/Region` walkers  -  all those
//! files now compose this helper plus a per-pass rewrite predicate.
//!
//! ## Why an owning map (not a `&mut Node` mutator)
//!
//! Vyre's IR uses `Arc<Vec<Node>>` for `Region::body`. Mutating an
//! existing tree requires `Arc::make_mut` or full clone-on-write at every
//! step; the owning-by-value path lets each pass produce a structurally
//! new tree (pre-existing share-state is preserved when the rewrite
//! predicate returns the input unchanged).
//!
//! ## Recursion contract
//!
//! `map_children(node, &mut f)` calls `f` once per immediate child of
//! `node` (not on `node` itself; the caller is in charge of deciding
//! whether to apply its rewrite at the current level). The function
//! reconstructs `node` with each child replaced by `f`'s output.
//!
//! For non-container nodes (`Store`, `Assign`, `Let`, `Barrier`, `Return`,
//! `IndirectDispatch`, `AsyncLoad`, `AsyncStore`, `AsyncWait`, `Trap`,
//! `Resume`, `Opaque`) `map_children` returns the node unchanged.

use std::sync::Arc;

use crate::ir::Node;

/// Recurse one level into `node`'s child sequences and apply `f` to each
/// immediate child node. Returns the rebuilt node.
///
/// The closure may itself call `map_children` to recurse further; the
/// helper does not do deep recursion on its own.
#[must_use]
pub fn map_children<F>(node: Node, f: &mut F) -> Node
where
    F: FnMut(Node) -> Node,
{
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond,
            then: then.into_iter().map(&mut *f).collect(),
            otherwise: otherwise.into_iter().map(&mut *f).collect(),
        },
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::Loop {
            var,
            from,
            to,
            body: body.into_iter().map(&mut *f).collect(),
        },
        Node::Block(body) => Node::Block(body.into_iter().map(&mut *f).collect()),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            Node::Region {
                generator,
                source_region,
                body: Arc::new(body_vec.into_iter().map(f).collect()),
            }
        }
        other => other,
    }
}

/// Rewrite the body sequence of a container node (`If::then`,
/// `If::otherwise`, `Loop::body`, `Block::body`, `Region::body`) through
/// `f`, then rebuild the node. Non-container nodes are returned
/// unchanged (no body to rewrite).
///
/// Used by the cleanup catalog to filter or transform a node's
/// immediate child sequence after recursion. Callers typically compose
/// `map_body(map_children(node, &mut recurse), &mut filter_step)`.
#[must_use]
pub fn map_body<F>(node: Node, f: &mut F) -> Node
where
    F: FnMut(Vec<Node>) -> Vec<Node>,
{
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond,
            then: f(then),
            otherwise: f(otherwise),
        },
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::Loop {
            var,
            from,
            to,
            body: f(body),
        },
        Node::Block(body) => Node::Block(f(body)),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            Node::Region {
                generator,
                source_region,
                body: Arc::new(f(body_vec)),
            }
        }
        other => other,
    }
}

/// True iff `pred` matches `node` itself or any descendant. Linear-time
/// preorder scan preserving the same visitation order as the prior
/// recursive implementation. Used by passes to short-circuit when
/// `analyze` can prove there's nothing to rewrite.
///
/// Implemented iteratively with a `SmallVec<&Node>` worklist to avoid
/// stack overflow on deeply nested trees (e.g. 1000+ nested `If`
/// bodies) AND to avoid the heap allocation entirely on the typical
/// small-tree case.
///
/// `VYRE_IR_HOTSPOTS` HIGH: every `analyze_impl` in cleanup/algebraic/loops
/// calls this once per top-level entry node. The 64-slot inline
/// `SmallVec` covers the vast majority of program trees in zero
/// allocations; deeper trees spill to the heap and pay only a couple
/// of doublings.
#[must_use]
pub fn any_descendant<P>(node: &Node, pred: &mut P) -> bool
where
    P: FnMut(&Node) -> bool,
{
    let mut stack: smallvec::SmallVec<[&Node; 64]> = smallvec::SmallVec::new();
    stack.push(node);
    while let Some(current) = stack.pop() {
        if pred(current) {
            return true;
        }
        match current {
            Node::If {
                then, otherwise, ..
            } => {
                for child in otherwise.iter().rev().chain(then.iter().rev()) {
                    stack.push(child);
                }
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                for child in body.iter().rev() {
                    stack.push(child);
                }
            }
            Node::Region { body, .. } => {
                for child in body.iter().rev() {
                    stack.push(child);
                }
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::model::expr::Ident;
    use crate::ir::Expr;

    fn store_at(idx: u32, value: u32) -> Node {
        Node::store("buf", Expr::u32(idx), Expr::u32(value))
    }

    #[test]
    fn map_children_recurses_into_if_branches() {
        // `if true { store(0,1) } else { store(0,2) }` → callback applied
        // to both inner stores; callback rewrites Store → Block(empty)
        // for proof.
        let input =
            Node::if_then_else(Expr::bool(true), vec![store_at(0, 1)], vec![store_at(0, 2)]);
        let mut count = 0;
        let mapped = map_children(input, &mut |n| {
            count += 1;
            match n {
                Node::Store { .. } => Node::Block(Vec::new()),
                other => other,
            }
        });
        assert_eq!(count, 2, "callback must fire once per branch's store");
        match mapped {
            Node::If {
                then, otherwise, ..
            } => {
                assert!(matches!(then[0], Node::Block(_)));
                assert!(matches!(otherwise[0], Node::Block(_)));
            }
            other => panic!("expected Node::If; got {other:?}"),
        }
    }

    #[test]
    fn map_children_recurses_into_loop_body() {
        let input = Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(4),
            body: vec![store_at(0, 7), store_at(1, 8)],
        };
        let mut count = 0;
        let _mapped = map_children(input, &mut |n| {
            count += 1;
            n
        });
        assert_eq!(count, 2);
    }

    #[test]
    fn map_children_recurses_into_block() {
        let input = Node::Block(vec![store_at(0, 1), store_at(1, 2), store_at(2, 3)]);
        let mut count = 0;
        let _mapped = map_children(input, &mut |n| {
            count += 1;
            n
        });
        assert_eq!(count, 3);
    }

    #[test]
    fn map_children_recurses_into_region_body() {
        let input = Node::Region {
            generator: Ident::from("test_op"),
            source_region: None,
            body: Arc::new(vec![store_at(0, 1)]),
        };
        let mut count = 0;
        let mapped = map_children(input, &mut |n| {
            count += 1;
            n
        });
        assert_eq!(count, 1);
        assert!(matches!(mapped, Node::Region { .. }));
    }

    #[test]
    fn map_children_preserves_op_id_through_region_unwrap_clone_path() {
        // Two strong refs to the same Arc<Vec<Node>> body force the
        // Arc::try_unwrap → clone branch.
        let body = Arc::new(vec![store_at(0, 1)]);
        let _keepalive = Arc::clone(&body);
        let input = Node::Region {
            generator: Ident::from("test_op_with_clone"),
            source_region: None,
            body,
        };
        let mapped = map_children(input, &mut |n| n);
        match mapped {
            Node::Region { generator, .. } => {
                assert_eq!(generator.as_str(), "test_op_with_clone");
            }
            other => panic!("expected Region; got {other:?}"),
        }
    }

    #[test]
    fn map_children_preserves_loop_metadata() {
        let input = Node::Loop {
            var: Ident::from("ix"),
            from: Expr::u32(2),
            to: Expr::u32(9),
            body: Vec::new(),
        };
        let mapped = map_children(input, &mut |n| n);
        match mapped {
            Node::Loop { var, from, to, .. } => {
                assert_eq!(var.as_str(), "ix");
                assert!(matches!(from, Expr::LitU32(2)));
                assert!(matches!(to, Expr::LitU32(9)));
            }
            other => panic!("expected Loop; got {other:?}"),
        }
    }

    #[test]
    fn map_children_returns_non_container_nodes_unchanged() {
        let input = store_at(0, 7);
        let mut fired = false;
        let mapped = map_children(input, &mut |_n| {
            fired = true;
            unreachable!("non-container nodes must not invoke the callback")
        });
        assert!(!fired, "no children = no callback invocations");
        assert!(matches!(mapped, Node::Store { .. }));
    }

    #[test]
    fn any_descendant_finds_match_at_root() {
        let node = store_at(0, 7);
        assert!(any_descendant(&node, &mut |n| matches!(
            n,
            Node::Store { .. }
        )));
    }

    #[test]
    fn any_descendant_recurses_into_nested_region() {
        // store nested 3 levels deep: Block > If > Region > Store.
        let node = Node::Block(vec![Node::if_then(
            Expr::bool(true),
            vec![Node::Region {
                generator: Ident::from("nested"),
                source_region: None,
                body: Arc::new(vec![store_at(0, 1)]),
            }],
        )]);
        assert!(any_descendant(&node, &mut |n| matches!(
            n,
            Node::Store { .. }
        )));
    }

    #[test]
    fn any_descendant_returns_false_when_no_match() {
        let node = Node::Block(vec![Node::if_then_else(
            Expr::bool(true),
            vec![Node::Block(Vec::new())],
            vec![Node::Block(Vec::new())],
        )]);
        assert!(!any_descendant(&node, &mut |n| matches!(
            n,
            Node::Store { .. }
        )));
    }

    #[test]
    fn any_descendant_short_circuits() {
        // Confirmed indirectly: counter must stay <= the position of the
        // first matching node in pre-order traversal.
        let node = Node::Block(vec![
            store_at(0, 1),
            store_at(1, 2),
            store_at(2, 3),
            store_at(3, 4),
        ]);
        let mut visited = 0;
        let found = any_descendant(&node, &mut |n| {
            visited += 1;
            matches!(n, Node::Store { .. })
        });
        assert!(found);
        // First node visited is the Block itself (no match), then the
        // first Store (match). So visited <= 2.
        assert!(
            visited <= 2,
            "any_descendant must short-circuit; visited {visited}"
        );
    }

    // --- O4: iterative worklist correctness & stack-safety tests ---

    /// Build a 1000-deep nested `If` tree with a single `Store` leaf.
    /// Construction is iterative so the test harness itself never recurses.
    fn deep_if_tree(depth: usize) -> Node {
        let mut node = store_at(0, 1);
        for _ in 0..depth {
            node = Node::if_then(Expr::bool(true), vec![node]);
        }
        node
    }

    /// Reference recursive walker used only for correctness comparison on
    /// small trees where recursion depth is safe.
    fn any_descendant_recursive<P>(node: &Node, pred: &mut P) -> bool
    where
        P: FnMut(&Node) -> bool,
    {
        if pred(node) {
            return true;
        }
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                then.iter().any(|n| any_descendant_recursive(n, pred))
                    || otherwise.iter().any(|n| any_descendant_recursive(n, pred))
            }
            Node::Loop { body, .. } => body.iter().any(|n| any_descendant_recursive(n, pred)),
            Node::Block(body) => body.iter().any(|n| any_descendant_recursive(n, pred)),
            Node::Region { body, .. } => body.iter().any(|n| any_descendant_recursive(n, pred)),
            _ => false,
        }
    }

    #[test]
    fn any_descendant_iterative_no_stack_overflow_on_deep_tree() {
        let deep = deep_if_tree(1000);
        // The iterative worklist must survive a depth that would overflow
        // a naive recursive implementation.
        assert!(
            any_descendant(&deep, &mut |n| matches!(n, Node::Store { .. })),
            "must find the Store leaf at depth 1000 without stack overflow"
        );
    }

    #[test]
    fn any_descendant_iterative_matches_recursive_traversal() {
        // Diverse small tree: Block > If(then: Loop(body: Store), else: Region(body: Store))
        let tree = Node::Block(vec![Node::if_then_else(
            Expr::bool(true),
            vec![Node::Loop {
                var: Ident::from("i"),
                from: Expr::u32(0),
                to: Expr::u32(2),
                body: vec![store_at(1, 2)],
            }],
            vec![Node::Region {
                generator: Ident::from("r"),
                source_region: None,
                body: Arc::new(vec![store_at(3, 4)]),
            }],
        )]);

        let mut recursive_ptrs = Vec::new();
        let recursive_found = any_descendant_recursive(&tree, &mut |n| {
            recursive_ptrs.push(n as *const Node);
            false
        });
        assert!(!recursive_found);

        let mut iterative_ptrs = Vec::new();
        let iterative_found = any_descendant(&tree, &mut |n| {
            iterative_ptrs.push(n as *const Node);
            false
        });
        assert!(!iterative_found);

        assert_eq!(
            recursive_ptrs, iterative_ptrs,
            "iterative walker must visit the exact same nodes in the exact same pre-order as the recursive reference"
        );
    }
}

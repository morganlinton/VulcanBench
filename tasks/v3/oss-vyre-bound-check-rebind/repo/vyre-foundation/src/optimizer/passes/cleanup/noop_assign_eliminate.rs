//! `noop_assign_eliminate`  -  drop `Node::Assign { name, value: Var(name) }`.
//!
//! Op id: `vyre-foundation::optimizer::passes::noop_assign_eliminate`.
//! Soundness: `Exact`  -  assigning a variable to itself is a no-op. Cost
//! direction: monotone-down on node_count + control_flow_count.
//! Preserves: every analysis. Invalidates: nothing.
//!
//! ## Rule
//!
//! ```text
//! Node::Assign { name: x, value: Expr::Var(x) }  →  drop
//! ```
//!
//! Comes up after value-numbering / CSE rewrites a value's RHS to its own
//! variable, leaving the syntactic self-assignment as residue. Without this
//! pass, the wire format and downstream codegen carry the no-op forward.

use crate::ir::{Expr, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Drop `Node::Assign` whose RHS is the name being assigned.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "noop_assign_eliminate",
    requires = [],
    invalidates = []
)]
pub struct NoopAssignEliminatePass;

impl NoopAssignEliminatePass {
    /// Skip programs without any self-assigning Assign.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_ASSIGN)
        {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_noop_assign))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree; drop noop self-assignments from sibling sequences.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            drop_noop_assigns(
                entry
                    .into_iter()
                    .map(|n| rewrite_node(n, &mut changed))
                    .collect(),
                &mut changed,
            )
        });
        PassResult { program, changed }
    }
}

/// Recurse into `node`'s descendants and drop noop self-assignments
/// from each container's body sequence.
fn rewrite_node(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| rewrite_node(child, changed));
    node_map::map_body(recursed, &mut |body| drop_noop_assigns(body, changed))
}

/// Drop `Node::Assign { name, value: Var(name) }` from a body sequence,
/// flipping `changed` when at least one is dropped.
fn drop_noop_assigns(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let mut out = Vec::with_capacity(body.len());
    for node in body {
        if is_noop_assign(&node) {
            *changed = true;
        } else {
            out.push(node);
        }
    }
    out
}

/// True iff `node` is `Assign { name, value: Var(name) }`.
fn is_noop_assign(node: &Node) -> bool {
    matches!(node, Node::Assign { name, value: Expr::Var(v) } if v == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn count_assigns(node: &Node) -> usize {
        match node {
            Node::Assign { .. } => 1,
            Node::If {
                then, otherwise, ..
            } => {
                then.iter().map(count_assigns).sum::<usize>()
                    + otherwise.iter().map(count_assigns).sum::<usize>()
            }
            Node::Loop { body, .. } => body.iter().map(count_assigns).sum(),
            Node::Block(body) => body.iter().map(count_assigns).sum(),
            Node::Region { body, .. } => body.iter().map(count_assigns).sum(),
            _ => 0,
        }
    }

    #[test]
    fn drops_self_assign() {
        let entry = vec![Node::Assign {
            name: Ident::from("x"),
            value: Expr::var("x"),
        }];
        let program = program_with_entry(entry);
        let result = NoopAssignEliminatePass::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_assigns).sum();
        assert_eq!(total, 0, "self-assign must be dropped");
    }

    #[test]
    fn keeps_real_assign() {
        // x = y is a real value-flow; must not be dropped.
        let entry = vec![Node::Assign {
            name: Ident::from("x"),
            value: Expr::var("y"),
        }];
        let program = program_with_entry(entry);
        let result = NoopAssignEliminatePass::transform(program);
        assert!(
            !result.changed,
            "x = y is a real assign and must be preserved"
        );
    }

    #[test]
    fn drops_self_assign_inside_if_branch() {
        let entry = vec![Node::if_then(
            Expr::var("c"),
            vec![Node::Assign {
                name: Ident::from("x"),
                value: Expr::var("x"),
            }],
        )];
        let program = program_with_entry(entry);
        let result = NoopAssignEliminatePass::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_assigns).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn drops_multiple_self_assigns() {
        let entry = vec![
            Node::Assign {
                name: Ident::from("a"),
                value: Expr::var("a"),
            },
            Node::Assign {
                name: Ident::from("b"),
                value: Expr::var("b"),
            },
            Node::Assign {
                name: Ident::from("c"),
                value: Expr::var("c"),
            },
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
        ];
        let program = program_with_entry(entry);
        let result = NoopAssignEliminatePass::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_assigns).sum();
        assert_eq!(total, 0, "all three self-assigns must be dropped");
    }

    #[test]
    fn analyze_skips_program_with_no_self_assigns() {
        let entry = vec![Node::Assign {
            name: Ident::from("x"),
            value: Expr::var("y"),
        }];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&NoopAssignEliminatePass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_self_assign_present() {
        let entry = vec![Node::Assign {
            name: Ident::from("x"),
            value: Expr::var("x"),
        }];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&NoopAssignEliminatePass, &program),
            PassAnalysis::RUN
        );
    }
}

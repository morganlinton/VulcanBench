//! `branch_value_hoist`  -  hoist a common prefix out of a divergent
//! `Node::If`. Cross-branch GVN entry point under ROADMAP A18.
//!
//! Soundness: `Exact`. When both arms of an `If` begin with the same
//! observably-side-effect-free `Let` (same name, same value expression),
//! that `Let` produces the same binding regardless of which arm executes,
//! so executing it once *before* the `If` is observably equivalent. The
//! hoisted name is in scope for the surviving sibling tail under both
//! arms (the subsequent IR already references it from inside each arm,
//! so no rename is needed). This is the prefix counterpart of A32
//! `tail_duplication`'s suffix hoist and is a value-numbering primitive
//! over the join-point at an `If`.
//!
//! Cost-direction: monotone-down on code_size (collapses one duplicated
//! `Let` per iteration). Preserves: every analysis. Invalidates: nothing
//! (the duplicated bindings were already in scope after the If).
//!
//! ## Pattern
//!
//! ```text
//! If(c, [Let(x, e), a, b, ...], [Let(x, e), a', b', ...])
//!   where e is observably side-effect-free (Let-eligible only)
//!   → Let(x, e); If(c, [a, b, ...], [a', b', ...])
//! ```
//!
//! The pass repeats the extraction so a chain of common prefix `Let`s
//! collapses to a sequence before a single `If`.
//!
//! ## ROADMAP
//!
//! A18  -  GVN across control flow. The fact-driven full-CFG GVN over
//! arbitrary join points lands beside the downstream reaching-def pass; this
//! row implements the structural prefix slice that is provably correct
//! without needing the alias substrate.

use crate::ir::{Expr, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Hoist a common prefix of side-effect-free `Let` bindings out of
/// every `Node::If` in the program.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "branch_value_hoist",
    requires = [],
    invalidates = [],
    phase = "cleanup",
    boundary_class = "abi_preserving",
    cost_model_family = "fusion"
)]
pub struct BranchValueHoistPass;

impl BranchValueHoistPass {
    /// Skip programs with no candidate `If`.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_IF)
        {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_prefix_candidate))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and hoist common prefixes.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .flat_map(|node| hoist_prefix(node, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

/// Recurse into descendants then hoist this node's prefix.
fn hoist_prefix(node: Node, changed: &mut bool) -> Vec<Node> {
    let recursed = node_map::map_children(node, &mut |child| {
        let hoisted = hoist_prefix(child, changed);
        if hoisted.len() == 1 {
            hoisted
                .into_iter()
                .next()
                .unwrap_or(Node::Block(Vec::new()))
        } else {
            Node::Block(hoisted)
        }
    });

    if let Node::If {
        cond,
        then,
        otherwise,
    } = recursed
    {
        let (prefix, new_then, new_otherwise) = extract_common_prefix(then, otherwise);
        if !prefix.is_empty() {
            *changed = true;
            let mut out = prefix;
            out.push(Node::If {
                cond,
                then: new_then,
                otherwise: new_otherwise,
            });
            return out;
        }
        return vec![Node::If {
            cond,
            then: new_then,
            otherwise: new_otherwise,
        }];
    }

    vec![recursed]
}

/// Pull the longest run of leading nodes that are identical, observably
/// free `Let` bindings out of both arms.
fn extract_common_prefix(
    mut then: Vec<Node>,
    mut otherwise: Vec<Node>,
) -> (Vec<Node>, Vec<Node>, Vec<Node>) {
    // Count the prefix length first, then drain in one pass  -  the
    // previous loop did Vec::remove(0) per matched Let which is O(n)
    // each (every remaining element shifts left). For an If with a
    // long body and a 5-deep common prefix that's 5 * 2 * (n - 5)
    // shifts; the count-then-drain version is one shift per arm.
    let mut prefix_len = 0;
    let pair_limit = then.len().min(otherwise.len());
    while prefix_len < pair_limit
        && is_hoistable_let_pair(&then[prefix_len], &otherwise[prefix_len])
    {
        prefix_len += 1;
    }
    if prefix_len == 0 {
        return (Vec::new(), then, otherwise);
    }
    let prefix: Vec<Node> = then.drain(0..prefix_len).collect();
    otherwise.drain(0..prefix_len);
    (prefix, then, otherwise)
}

/// True iff both nodes are the same `Let` with an observably-free value.
fn is_hoistable_let_pair(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (
            Node::Let {
                name: name_a,
                value: value_a,
            },
            Node::Let {
                name: name_b,
                value: value_b,
            },
        ) => name_a == name_b && value_a == value_b && expr_is_observably_free(value_a),
        _ => false,
    }
}

/// True iff `expr` cannot observe or mutate program-visible state.
///
/// Hoisting an observable expression across a branch boundary would
/// duplicate the observation in the unconditional path, so the gate is
/// strict: any read from memory, any atomic, any opaque or extension
/// call, any lane-correlated subgroup intrinsic blocks the hoist.
fn expr_is_observably_free(expr: &Expr) -> bool {
    match expr {
        Expr::Load { .. }
        | Expr::Atomic { .. }
        | Expr::Call { .. }
        | Expr::Opaque(_)
        | Expr::SubgroupBallot { .. }
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => false,
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. } => true,
        Expr::BinOp { left, right, .. } => {
            expr_is_observably_free(left) && expr_is_observably_free(right)
        }
        Expr::UnOp { operand, .. } => expr_is_observably_free(operand),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_is_observably_free(cond)
                && expr_is_observably_free(true_val)
                && expr_is_observably_free(false_val)
        }
        Expr::Cast { value, .. } => expr_is_observably_free(value),
        Expr::Fma { a, b, c } => {
            expr_is_observably_free(a) && expr_is_observably_free(b) && expr_is_observably_free(c)
        }
    }
}

/// True iff `node` is an `If` with a hoistable common prefix.
fn is_prefix_candidate(node: &Node) -> bool {
    if let Node::If {
        then, otherwise, ..
    } = node
    {
        match (then.first(), otherwise.first()) {
            (Some(t), Some(o)) => is_hoistable_let_pair(t, o),
            _ => false,
        }
    } else {
        false
    }
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

    /// Walk the program's entry tree and find the first sibling sequence
    /// containing the targeted `If`. `wrapped` programs nest the entry
    /// inside a Region wrapper, and the pass leaves a hoisted prefix as
    /// `[Let..., If]` siblings inside whichever container held the
    /// original `If`. This helper unwraps Region/Block layers so a test
    /// can reason about the pass's local rewrite shape.
    fn find_if_with_siblings(nodes: &[Node]) -> Option<&[Node]> {
        if nodes.iter().any(|n| matches!(n, Node::If { .. })) {
            return Some(nodes);
        }
        for node in nodes {
            let body = match node {
                Node::Block(body) => body.as_slice(),
                Node::Region { body, .. } => body.as_ref().as_slice(),
                _ => continue,
            };
            if let Some(found) = find_if_with_siblings(body) {
                return Some(found);
            }
        }
        None
    }

    /// Positive: a single common-prefix `Let` is hoisted out.
    #[test]
    fn hoists_single_common_let_prefix() {
        let common = Node::let_bind("x", Expr::u32(42));
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                common.clone(),
                Node::store("buf", Expr::u32(0), Expr::var("x")),
            ],
            otherwise: vec![common, Node::store("buf", Expr::u32(0), Expr::var("x"))],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(result.changed, "common Let prefix must be hoisted");
        let siblings = find_if_with_siblings(result.program.entry())
            .expect("Fix: hoisted Let + If must live as siblings somewhere in the entry tree");
        assert_eq!(siblings.len(), 2, "prefix Let then surviving If");
        assert!(matches!(&siblings[0], Node::Let { name, .. } if name.as_str() == "x"));
        assert!(matches!(&siblings[1], Node::If { .. }));
    }

    /// Positive: a chain of common-prefix `Let`s collapses in one pass.
    #[test]
    fn hoists_chain_of_common_lets() {
        let a = Node::let_bind("x", Expr::u32(1));
        let b = Node::let_bind(
            "y",
            Expr::BinOp {
                op: crate::ir::BinOp::Add,
                left: Box::new(Expr::var("x")),
                right: Box::new(Expr::u32(2)),
            },
        );
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                a.clone(),
                b.clone(),
                Node::store("buf", Expr::u32(0), Expr::var("y")),
            ],
            otherwise: vec![a, b, Node::store("buf", Expr::u32(1), Expr::var("y"))],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(result.changed, "two-Let prefix must be hoisted in one pass");
        let siblings = find_if_with_siblings(result.program.entry())
            .expect("Fix: hoisted Lets + If must live as siblings somewhere in the entry tree");
        assert_eq!(siblings.len(), 3, "two Let prefix nodes then surviving If");
        assert!(matches!(&siblings[0], Node::Let { name, .. } if name.as_str() == "x"));
        assert!(matches!(&siblings[1], Node::Let { name, .. } if name.as_str() == "y"));
        assert!(matches!(&siblings[2], Node::If { .. }));
    }

    /// Negative: differing names block the hoist.
    #[test]
    fn keeps_when_names_differ() {
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", Expr::u32(1))],
            otherwise: vec![Node::let_bind("y", Expr::u32(1))],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(!result.changed, "differing names must not hoist");
    }

    /// Negative: differing values block the hoist.
    #[test]
    fn keeps_when_values_differ() {
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", Expr::u32(1))],
            otherwise: vec![Node::let_bind("x", Expr::u32(2))],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(!result.changed, "differing values must not hoist");
    }

    /// Negative: a `Let` whose value reads memory must not be hoisted  -
    /// the `Load` would observe state that may not have been initialised
    /// on the unconditional path.
    #[test]
    fn keeps_when_value_reads_memory() {
        let common = Node::let_bind(
            "x",
            Expr::Load {
                buffer: Ident::from("buf"),
                index: Box::new(Expr::u32(0)),
            },
        );
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![common.clone()],
            otherwise: vec![common],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(!result.changed, "Load-bearing prefix must not be hoisted");
    }

    /// Negative: an `Atomic` value may have observable ordering
    /// implications and must not move across the branch boundary.
    #[test]
    fn keeps_when_value_is_atomic() {
        let common = Node::let_bind(
            "x",
            Expr::Atomic {
                op: crate::ir::AtomicOp::Add,
                buffer: Ident::from("buf"),
                index: Box::new(Expr::u32(0)),
                expected: None,
                value: Box::new(Expr::u32(1)),
                ordering: crate::ir::MemoryOrdering::Relaxed,
            },
        );
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![common.clone()],
            otherwise: vec![common],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(!result.changed, "Atomic prefix must not be hoisted");
    }

    /// Negative: prefix nodes must be `Let`  -  a leading `Store`
    /// observable on both arms must not be hoisted either, because the
    /// hoist would change observed memory ordering relative to the
    /// surrounding code outside the `If`.
    #[test]
    fn keeps_when_prefix_is_store() {
        let common = Node::store("buf", Expr::u32(0), Expr::u32(7));
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![common.clone()],
            otherwise: vec![common],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(!result.changed, "Store prefix must not be hoisted");
    }

    /// Negative: only the matching prefix is extracted  -  non-matching
    /// trailing nodes stay in their respective arms.
    #[test]
    fn extracts_only_the_common_prefix() {
        let common = Node::let_bind("x", Expr::u32(7));
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                common.clone(),
                Node::store("buf", Expr::u32(0), Expr::u32(1)),
            ],
            otherwise: vec![common, Node::store("buf", Expr::u32(0), Expr::u32(2))],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(result.changed, "leading common prefix must be hoisted");
        let siblings = find_if_with_siblings(result.program.entry())
            .expect("Fix: hoisted Let + If must live as siblings somewhere in the entry tree");
        let surviving_if = siblings
            .iter()
            .find(|n| matches!(n, Node::If { .. }))
            .expect("Fix: surviving If must remain after the hoist");
        match surviving_if {
            Node::If {
                then, otherwise, ..
            } => {
                assert_eq!(then.len(), 1, "non-prefix tail stays in then");
                assert_eq!(otherwise.len(), 1, "non-prefix tail stays in otherwise");
                assert!(matches!(&then[0], Node::Store { .. }));
                assert!(matches!(&otherwise[0], Node::Store { .. }));
            }
            other => panic!("expected If, got {other:?}"),
        }
    }

    /// Negative: an empty arm cannot share a prefix.
    #[test]
    fn keeps_when_one_arm_is_empty() {
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", Expr::u32(1))],
            otherwise: vec![],
        }];
        let program = program_with_entry(entry);
        let result = BranchValueHoistPass::transform(program);
        assert!(!result.changed, "empty otherwise has nothing to share");
    }

    /// `analyze` short-circuits on programs with no candidate `If`.
    #[test]
    fn analyze_skips_programs_with_no_branch() {
        let entry = vec![Node::store("buf", Expr::u32(0), Expr::u32(1))];
        let program = program_with_entry(entry);
        match crate::optimizer::ProgramPass::analyze(&BranchValueHoistPass, &program) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP, got {other:?}"),
        }
    }
}

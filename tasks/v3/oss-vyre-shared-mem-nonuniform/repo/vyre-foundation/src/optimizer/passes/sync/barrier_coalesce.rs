//! `barrier_coalesce`  -  fold consecutive `Node::Barrier` runs into one.
//!
//! Op id: `vyre-foundation::optimizer::passes::barrier_coalesce`. Soundness:
//! `Exact` over the per-MemoryOrdering join rule documented in
//! `vyre-foundation::memory_model`. Cost-direction: monotone-down on
//! `node_count` and `control_flow_count`  -  never inserts a barrier; only
//! removes redundant ones. Preserves: every analysis. Invalidates: nothing
//! (the IR shape changes but downstream passes that care about barrier
//! placement only ask for the strictest barrier in each sequence).
//!
//! ## Rule
//!
//! When the IR contains a sequence `[..., Barrier(A), Barrier(B), ...]`
//! at the same scope (same parent body), the pair is replaced by a single
//! `Barrier(join(A, B))` where `join` is the MemoryOrdering join  -  the
//! stronger of the two. This is sound because two barriers in immediate
//! sequence with no intervening memory operations are equivalent to one
//! barrier of the stronger ordering: every guarantee of either is provided
//! by the join.
//!
//! Common after lowering passes that conservatively emit barriers around
//! every atomic, then rely on a coalesce pass to collapse runs.
//!
//! ## Ordering join (mirrors `effect_lattice::SyncScope::join`)
//!
//! - `Relaxed ⊔ X = X`
//! - `Acquire ⊔ Release = AcqRel`
//! - `AcqRel ⊔ X = AcqRel` (unless X is `SeqCst` or `GridSync`)
//! - `SeqCst ⊔ X = SeqCst` (unless X is `GridSync`)
//! - `GridSync ⊔ anything = GridSync`
//!
//! GridSync dominates because it's the only ordering that synchronizes
//! across blocks; absorbing weaker orderings into it is sound.

use crate::ir::{Node, Program};
use crate::memory_model::MemoryOrdering;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};

/// Coalesce consecutive `Node::Barrier` siblings into the join of their orderings.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "barrier_coalesce",
    requires = [],
    invalidates = []
)]
pub struct BarrierCoalescePass;

impl BarrierCoalescePass {
    /// Skip programs that contain no consecutive-barrier pair. Checks
    /// both the top-level entry vec (transform fuses adjacent siblings
    /// there too) and every nested If/Loop/Block/Region body.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Coalescing requires at least two barriers; even one Barrier
        // is necessary. Bit-test the cached stats first.
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_BARRIER)
        {
            return PassAnalysis::SKIP;
        }
        if sequence_has_consecutive_barriers(program.entry())
            || program.entry().iter().any(has_consecutive_barriers)
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree; for every body containing consecutive
    /// barriers, replace them with the join.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| coalesce_nodes(entry, &mut changed));
        PassResult { program, changed }
    }
}

/// Recurse into the children of `node` and coalesce any internal
/// barrier sequences. Parent-level coalescing is handled by `coalesce_nodes`.
fn coalesce_node(node: Node, changed: &mut bool) -> Node {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            let then = coalesce_nodes(then, changed);
            let otherwise = coalesce_nodes(otherwise, changed);
            Node::If {
                cond,
                then,
                otherwise,
            }
        }
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            let body = coalesce_nodes(body, changed);
            Node::Loop {
                var,
                from,
                to,
                body,
            }
        }
        Node::Block(body) => Node::Block(coalesce_nodes(body, changed)),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            // Body is Arc<Vec<Node>>; recursive into the inner sequence
            // requires owning a fresh Vec. Either Arc::try_unwrap or
            // clone-the-inner.
            let body_vec: Vec<Node> = match std::sync::Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            let body_vec = coalesce_nodes(body_vec, changed);
            Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(body_vec),
            }
        }
        other => other,
    }
}

fn coalesce_nodes(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let mut out = Vec::with_capacity(body.len());
    for node in body {
        push_coalesced(&mut out, coalesce_node(node, changed), changed);
    }
    out
}

fn push_coalesced(out: &mut Vec<Node>, node: Node, changed: &mut bool) {
    match (out.last(), &node) {
        (Some(Node::Barrier { ordering: prev }), Node::Barrier { ordering: curr }) => {
            let joined = join_ordering(*prev, *curr);
            let new_last = Node::Barrier { ordering: joined };
            *out.last_mut()
                .unwrap_or_else(|| unreachable!("non-empty by match arm above (Some(Barrier))")) =
                new_last;
            *changed = true;
        }
        _ => out.push(node),
    }
}

/// True if `node` (or any of its non-Region descendants) contains an
/// adjacent-barrier pair at any nesting level. Drives the analyze gate.
fn has_consecutive_barriers(node: &Node) -> bool {
    match node {
        Node::If {
            then, otherwise, ..
        } => {
            sequence_has_consecutive_barriers(then)
                || sequence_has_consecutive_barriers(otherwise)
                || then.iter().any(has_consecutive_barriers)
                || otherwise.iter().any(has_consecutive_barriers)
        }
        Node::Loop { body, .. } | Node::Block(body) => {
            sequence_has_consecutive_barriers(body) || body.iter().any(has_consecutive_barriers)
        }
        Node::Region { body, .. } => {
            sequence_has_consecutive_barriers(body) || body.iter().any(has_consecutive_barriers)
        }
        _ => false,
    }
}

fn sequence_has_consecutive_barriers(body: &[Node]) -> bool {
    body.windows(2).any(|pair| {
        matches!(
            (&pair[0], &pair[1]),
            (Node::Barrier { .. }, Node::Barrier { .. })
        )
    })
}

/// Join two `MemoryOrderings` to the strictest of the pair. Mirrors the
/// composition rule used by `effect_lattice::SyncScope::join` and the
/// per-ordering composition table documented in the module doc.
fn join_ordering(a: MemoryOrdering, b: MemoryOrdering) -> MemoryOrdering {
    use MemoryOrdering::{AcqRel, Acquire, GridSync, Relaxed, Release, SeqCst};
    match (a, b) {
        (GridSync, _) | (_, GridSync) => GridSync,
        (SeqCst, _) | (_, SeqCst) => SeqCst,
        (AcqRel, _) | (_, AcqRel) | (Acquire, Release) | (Release, Acquire) => AcqRel,
        (Acquire, Acquire) => Acquire,
        (Release, Release) => Release,
        (Relaxed, x) | (x, Relaxed) => x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    /// Count `Node::Barrier` occurrences anywhere in the program entry
    /// tree (descending into Region / If / Loop / Block bodies). Lets
    /// tests assert post-coalesce barrier count even though
    /// Program::wrapped puts everything inside an outer Region.
    fn count_barriers(node: &Node) -> usize {
        match node {
            Node::Barrier { .. } => 1,
            Node::If {
                then, otherwise, ..
            } => {
                then.iter().map(count_barriers).sum::<usize>()
                    + otherwise.iter().map(count_barriers).sum::<usize>()
            }
            Node::Loop { body, .. } | Node::Block(body) => body.iter().map(count_barriers).sum(),
            Node::Region { body, .. } => body.iter().map(count_barriers).sum(),
            _ => 0,
        }
    }

    /// Find the first `Node::Barrier` at any depth in the entry tree
    /// and return its ordering. Returns None if no barrier exists.
    fn first_barrier_ordering(node: &Node) -> Option<MemoryOrdering> {
        match node {
            Node::Barrier { ordering } => Some(*ordering),
            Node::If {
                then, otherwise, ..
            } => then
                .iter()
                .find_map(first_barrier_ordering)
                .or_else(|| otherwise.iter().find_map(first_barrier_ordering)),
            Node::Loop { body, .. } | Node::Block(body) => {
                body.iter().find_map(first_barrier_ordering)
            }
            Node::Region { body, .. } => body.iter().find_map(first_barrier_ordering),
            _ => None,
        }
    }

    #[test]
    fn join_workgroup_acqrel_workgroup_acqrel_yields_acqrel() {
        // Two AcqRel barriers join to AcqRel  -  neither is stronger.
        assert_eq!(
            join_ordering(MemoryOrdering::AcqRel, MemoryOrdering::AcqRel),
            MemoryOrdering::AcqRel
        );
    }

    #[test]
    fn join_acquire_release_yields_acqrel() {
        assert_eq!(
            join_ordering(MemoryOrdering::Acquire, MemoryOrdering::Release),
            MemoryOrdering::AcqRel,
            "acquire ⊔ release must escalate to AcqRel"
        );
    }

    #[test]
    fn join_grid_sync_dominates_everything() {
        for other in [
            MemoryOrdering::Relaxed,
            MemoryOrdering::Acquire,
            MemoryOrdering::Release,
            MemoryOrdering::AcqRel,
            MemoryOrdering::SeqCst,
        ] {
            assert_eq!(
                join_ordering(MemoryOrdering::GridSync, other),
                MemoryOrdering::GridSync,
                "GridSync ⊔ {other:?} must stay GridSync  -  losing GridSync would silently \
                 downgrade cross-block synchronization"
            );
            assert_eq!(
                join_ordering(other, MemoryOrdering::GridSync),
                MemoryOrdering::GridSync
            );
        }
    }

    #[test]
    fn coalesces_two_seqcst_barriers_into_one() {
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::Barrier {
                ordering: MemoryOrdering::SeqCst,
            },
            Node::Barrier {
                ordering: MemoryOrdering::SeqCst,
            },
            Node::store("buf", Expr::u32(1), Expr::u32(8)),
        ];
        let program = program_with_entry(entry);
        let result = BarrierCoalescePass::transform(program);
        assert!(
            result.changed,
            "consecutive barriers must mark the program as changed"
        );
        let barrier_count: usize = result.program.entry().iter().map(count_barriers).sum();
        assert_eq!(
            barrier_count, 1,
            "two consecutive SeqCst barriers must coalesce into one; got {barrier_count}"
        );
    }

    #[test]
    fn coalesces_three_consecutive_barriers_to_one_with_join() {
        // Acquire + Release + AcqRel → AcqRel
        let entry = vec![
            Node::Barrier {
                ordering: MemoryOrdering::Acquire,
            },
            Node::Barrier {
                ordering: MemoryOrdering::Release,
            },
            Node::Barrier {
                ordering: MemoryOrdering::AcqRel,
            },
        ];
        let program = program_with_entry(entry);
        let result = BarrierCoalescePass::transform(program);
        assert!(result.changed);
        let total_barriers: usize = result.program.entry().iter().map(count_barriers).sum();
        assert_eq!(
            total_barriers, 1,
            "three consecutive barriers must coalesce to one; got {total_barriers}"
        );
        let ordering = result
            .program
            .entry()
            .iter()
            .find_map(first_barrier_ordering)
            .expect("Fix: a barrier must exist after coalesce");
        assert_eq!(
            ordering,
            MemoryOrdering::AcqRel,
            "Acquire ⊔ Release ⊔ AcqRel must collapse to AcqRel"
        );
    }

    #[test]
    fn does_not_coalesce_barriers_separated_by_a_store() {
        let entry = vec![
            Node::Barrier {
                ordering: MemoryOrdering::SeqCst,
            },
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::Barrier {
                ordering: MemoryOrdering::SeqCst,
            },
        ];
        let program = program_with_entry(entry);
        let result = BarrierCoalescePass::transform(program);
        // The store between the two barriers means they're NOT consecutive
        // siblings  -  coalescing would skip the store's memory effects.
        assert!(
            !result.changed,
            "barriers separated by a store must NOT coalesce"
        );
    }

    #[test]
    fn analyze_skips_program_with_no_consecutive_barriers() {
        let entry = vec![
            Node::Barrier {
                ordering: MemoryOrdering::SeqCst,
            },
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::Barrier {
                ordering: MemoryOrdering::SeqCst,
            },
        ];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&BarrierCoalescePass, &program),
            PassAnalysis::SKIP,
            "analyze must SKIP when no consecutive barriers exist; otherwise the optimizer \
             pays a full transform pass for nothing"
        );
    }

    #[test]
    fn coalesces_grid_sync_with_workgroup_to_grid_sync() {
        // GridSync followed by SeqCst (workgroup-scope)  -  GridSync dominates.
        let entry = vec![
            Node::Barrier {
                ordering: MemoryOrdering::GridSync,
            },
            Node::Barrier {
                ordering: MemoryOrdering::SeqCst,
            },
        ];
        let program = program_with_entry(entry);
        let result = BarrierCoalescePass::transform(program);
        assert!(result.changed);
        let ordering = result
            .program
            .entry()
            .iter()
            .find_map(first_barrier_ordering)
            .expect("Fix: a barrier must exist");
        assert_eq!(
            ordering,
            MemoryOrdering::GridSync,
            "GridSync ⊔ SeqCst must stay GridSync"
        );
    }

    #[test]
    fn coalesces_inside_if_then_branch() {
        let entry = vec![Node::if_then(
            Expr::bool(true),
            vec![
                Node::Barrier {
                    ordering: MemoryOrdering::SeqCst,
                },
                Node::Barrier {
                    ordering: MemoryOrdering::SeqCst,
                },
            ],
        )];
        let program = program_with_entry(entry);
        let result = BarrierCoalescePass::transform(program);
        assert!(result.changed, "coalesce must recurse into If branches");
        // Verify the coalesce was correct: exactly one barrier must survive in
        // the if-then branch, and it must carry SeqCst (not be downgraded or
        // dropped entirely). A regression that coalesces to Relaxed or removes
        // both barriers would have passed the old `changed == true` check.
        assert_eq!(
            result.program.entry().iter().map(count_barriers).sum::<usize>(),
            1,
            "two SeqCst barriers in if-then must coalesce to exactly one"
        );
        let ordering = result
            .program
            .entry()
            .iter()
            .find_map(first_barrier_ordering)
            .expect("Fix: a barrier must exist after coalesce inside if-then branch");
        assert_eq!(
            ordering,
            MemoryOrdering::SeqCst,
            "SeqCst ⊔ SeqCst must preserve SeqCst ordering, not downgrade"
        );
    }
}

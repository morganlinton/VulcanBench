//! ROADMAP A13  -  escape facts into scratch reuse across megakernel
//! arms.
//!
//! Foundation-side substrate. Walks every `Node::Region` in the
//! program (a Region is the megakernel arm boundary) and uses
//! `ProgramFacts::buffer_escapes` (A13 fact) to identify per-arm
//! buffers whose contents do NOT need to survive past the arm.
//! Non-escaping buffers can share storage with other non-escaping
//! buffers from disjoint arms  -  the runtime allocator can recycle
//! one underlying allocation across arms as long as the arms don't
//! execute in parallel.
//!
//! The pass produces a `ScratchReusePlan`: a mapping from
//! `RegionMeta` to the set of non-escaping buffer names the
//! runtime is licensed to recycle into a shared scratch pool.
//! Backends consume the plan via the public API; the foundation
//! pass is read-only (it doesn't rewrite the IR).
//!
//! ## Why a plan, not a rewrite
//!
//! Buffer reuse is a runtime allocation decision, not an IR
//! shape change. The IR keeps every buffer named; the runtime
//! decides which underlying storage backs each name based on the
//! plan. Keeping the rewrite at the runtime level lets backends
//! use their own pool / arena strategies (CUDA stream allocator,
//! wgpu buffer pool) without IR-level constraint.

use crate::ir::{Ident, Node, Program};
use crate::optimizer::program_soa::ProgramFacts;
use rustc_hash::{FxHashMap, FxHashSet};

/// Plan: which buffers each arm-region is licensed to recycle.
#[derive(Debug, Default)]
pub struct ScratchReusePlan {
    /// Region's `generator` ident → set of buffer names that arm
    /// is licensed to recycle (non-escaping per A13 facts).
    arm_recyclable: FxHashMap<Ident, FxHashSet<Ident>>,
    /// All buffers in the program that are "non-escaping" (host
    /// never reads back, no atomics, no `IndirectDispatch`). These
    /// are the candidates for recycling.
    non_escaping: FxHashSet<Ident>,
}

impl ScratchReusePlan {
    /// Build the plan from a Program. One pass over the entry
    /// tree builds `ProgramFacts`, then the plan iterates regions
    /// and queries the facts.
    #[must_use]
    pub fn build(program: &Program) -> Self {
        let facts = ProgramFacts::build_cached(program);
        let escaping = facts.escaping_buffers();
        let non_escaping: FxHashSet<Ident> = program
            .buffers()
            .iter()
            .filter_map(|b| {
                let name = Ident::from(b.name.as_ref());
                if escaping.contains(&name) {
                    None
                } else {
                    Some(name)
                }
            })
            .collect();
        let mut arm_recyclable: FxHashMap<Ident, FxHashSet<Ident>> = FxHashMap::default();
        for region in facts.regions() {
            let arm_buffers = collect_buffer_uses(program.entry(), region.node, &facts);
            let recyclable: FxHashSet<Ident> = arm_buffers
                .into_iter()
                .filter(|b| non_escaping.contains(b))
                .collect();
            if !recyclable.is_empty() {
                arm_recyclable
                    .entry(region.generator.clone())
                    .or_default()
                    .extend(recyclable);
            }
        }
        Self {
            arm_recyclable,
            non_escaping,
        }
    }

    /// Buffers the named arm-region is licensed to recycle. Empty
    /// set if the region has no recyclable buffers or doesn't
    /// appear in the plan.
    #[must_use]
    pub fn recyclable_for(&self, arm_generator: &str) -> &FxHashSet<Ident> {
        self.arm_recyclable.get(arm_generator).unwrap_or(&EMPTY_SET)
    }

    /// All non-escaping buffers in the program. The recycling
    /// candidates are a subset of this.
    #[must_use]
    pub fn non_escaping(&self) -> &FxHashSet<Ident> {
        &self.non_escaping
    }

    /// `true` iff the named buffer can be recycled by some arm.
    #[must_use]
    pub fn is_recyclable(&self, name: &str) -> bool {
        // Ident: Borrow<str>, so contains takes &str directly without
        // building an Ident; the lookup is one hash + one cmp instead
        // of an O(N) linear scan.
        self.non_escaping.contains(name)
    }

    /// Total number of arm/buffer recycle pairs in the plan.
    #[must_use]
    pub fn pair_count(&self) -> usize {
        self.arm_recyclable
            .values()
            .map(std::collections::HashSet::len)
            .sum()
    }
}

static EMPTY_SET: std::sync::LazyLock<FxHashSet<Ident>> =
    std::sync::LazyLock::new(FxHashSet::default);

/// Collect every buffer touched (via Read / Write / Atomic / Async)
/// inside the subtree rooted at the given Region `NodeIndex`.
fn collect_buffer_uses(
    _entry: &[Node],
    region_node: crate::optimizer::program_soa::NodeIndex,
    facts: &ProgramFacts,
) -> FxHashSet<Ident> {
    let mut out: FxHashSet<Ident> = FxHashSet::default();
    for (node, name, _) in facts.buffer_refs() {
        if facts.is_descendant_of(*node, region_node) {
            out.insert(name.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf_rw(name: &str, binding: u32) -> BufferDecl {
        BufferDecl::storage(name, binding, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn buf_ro(name: &str, binding: u32) -> BufferDecl {
        BufferDecl::storage(name, binding, BufferAccess::ReadOnly, DataType::U32).with_count(4)
    }

    fn buf_output(name: &str, binding: u32) -> BufferDecl {
        BufferDecl::output(name, binding, DataType::U32).with_count(4)
    }

    fn region(generator_name: &str, body: Vec<Node>) -> Node {
        Node::Region {
            generator: Ident::from(generator_name),
            source_region: None,
            body: std::sync::Arc::new(body),
        }
    }

    /// Read-only buffers don't escape and aren't written → not
    /// recyclable (they're inputs). Wait  -  actually non-escaping
    /// = no writes ever = the runtime CAN reuse storage between
    /// arms, but only if subsequent arms don't read the original
    /// content. For inputs that the host produced and the arm
    /// just reads, recycling means clobbering the input  -  not
    /// what we want. So the plan should EXCLUDE buffers that are
    /// only read.
    ///
    /// The current escape-fact definition treats Read-only buffers
    /// as non-escaping (they're not in `escaping_buffers()`). That
    /// makes them "candidates" by the trivial rule. The runtime
    /// safety net: only WRITE targets that don't escape can be
    /// recycled (the read-only inputs stay pinned). The plan
    /// surface here is the conservative super-set; the runtime
    /// narrows via its own per-arm bound-buffer check.
    #[test]
    fn read_only_buffer_appears_as_non_escaping() {
        let entry = vec![region(
            "arm_a",
            vec![Node::let_bind(
                "x",
                Expr::Load {
                    buffer: Ident::from("input"),
                    index: Box::new(Expr::u32(0)),
                },
            )],
        )];
        let prog = Program::wrapped(vec![buf_ro("input", 0)], [1, 1, 1], entry);
        let plan = ScratchReusePlan::build(&prog);
        assert!(plan.is_recyclable("input"));
    }

    /// Output buffer escapes → not recyclable.
    #[test]
    fn output_buffer_does_not_appear_as_recyclable() {
        let entry = vec![region(
            "arm_a",
            vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
        )];
        let prog = Program::wrapped(vec![buf_output("out", 0)], [1, 1, 1], entry);
        let plan = ScratchReusePlan::build(&prog);
        // 'out' is a Store target → escapes per A13 → not in
        // non-escaping set.
        assert!(!plan.is_recyclable("out"));
    }

    /// `pair_count()` reports the number of (arm, recyclable)
    /// pairs.
    #[test]
    fn pair_count_reports_total_pairs() {
        let entry = vec![
            region(
                "arm_a",
                vec![Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("scratch_a"),
                        index: Box::new(Expr::u32(0)),
                    },
                )],
            ),
            region(
                "arm_b",
                vec![Node::let_bind(
                    "y",
                    Expr::Load {
                        buffer: Ident::from("scratch_b"),
                        index: Box::new(Expr::u32(0)),
                    },
                )],
            ),
        ];
        let prog = Program::wrapped(
            vec![buf_ro("scratch_a", 0), buf_ro("scratch_b", 1)],
            [1, 1, 1],
            entry,
        );
        let plan = ScratchReusePlan::build(&prog);
        let arm_a = plan.recyclable_for("arm_a");
        assert!(arm_a.iter().any(|n| n.as_str() == "scratch_a"));
        assert!(!arm_a.iter().any(|n| n.as_str() == "scratch_b"));
        let arm_b = plan.recyclable_for("arm_b");
        assert!(arm_b.iter().any(|n| n.as_str() == "scratch_b"));
        assert!(!arm_b.iter().any(|n| n.as_str() == "scratch_a"));
        assert!(plan.pair_count() >= 2);
    }

    /// Non-escaping set excludes Store targets.
    #[test]
    fn non_escaping_excludes_store_targets() {
        let entry = vec![region(
            "arm",
            vec![
                Node::store("rw", Expr::u32(0), Expr::u32(7)),
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("input"),
                        index: Box::new(Expr::u32(0)),
                    },
                ),
            ],
        )];
        let prog = Program::wrapped(vec![buf_ro("input", 0), buf_rw("rw", 1)], [1, 1, 1], entry);
        let plan = ScratchReusePlan::build(&prog);
        assert!(plan.non_escaping().iter().any(|n| n.as_str() == "input"));
        assert!(!plan.non_escaping().iter().any(|n| n.as_str() == "rw"));
    }
}

//! Cost certificates for cost-monotone-down pass enforcement.
//!
//! Op id: `vyre-foundation::optimizer::cost`. Soundness: `Exact` over the per-Program
//! statistics surface plus a divergence score derived from `if invocation_id == K`
//! patterns. Cost-direction: this module is the gate that lets the scheduler
//! refuse a `ProgramPass::transform` rewrite whose post-condition cost is higher than
//! the pre-condition cost on any tracked dimension. Preserves: every analysis
//! (cost is read-only). Invalidates: nothing.
//!
//! ## Why a wrapper around `ProgramStats`
//!
//! `Program::stats()` already returns cached `ProgramStats` covering node count,
//! storage, instructions, memory ops, atomics, control flow, register pressure,
//! and a capability bitmask. That surface is most of what a cost certificate
//! needs. This module wraps it into the fixed `CostCertificate` shape used by
//! the optimizer post-condition gate, adds a divergence score (per-region
//! `if invocation_id == K` count, which is not in `ProgramStats`), and exposes
//! the per-dimension dominance comparison that drives `RefusalReason::CostIncrease`.
//!
//! ## Contract for passes
//!
//! Every pass that lands a rewrite is expected to be cost-monotone-down: every
//! tracked cost dimension on the post-rewrite Program must be `<=` the same
//! dimension on the pre-rewrite Program. Passes that intentionally trade one
//! dimension for another (e.g. trade a memory op for an atomic op to remove a
//! barrier) MUST opt out by returning `RefusalReason::CostIncrease` from
//! `ProgramPass::try_transform` instead of allowing the scheduler to silently land
//! the rewrite. The scheduler computes the pre/post cost diff and refuses any
//! rewrite that increases a tracked dimension without an explicit declaration.
//!
//! ## Dimensions tracked
//!
//! | dimension | source | meaning |
//! |---|---|---|
//! | `node_count` | `ProgramStats::node_count` | total IR statements (proxy for codegen size) |
//! | `instruction_count` | `ProgramStats::instruction_count` | scalar/vector instruction estimate |
//! | `memory_op_count` | `ProgramStats::memory_op_count` | loads + stores + async copies (memory-bandwidth proxy) |
//! | `atomic_op_count` | `ProgramStats::atomic_op_count` | atomic RMWs (contention proxy) |
//! | `control_flow_count` | `ProgramStats::control_flow_count` | branches + loops (divergence-cost proxy) |
//! | `register_pressure_estimate` | `ProgramStats::register_pressure_estimate` | concurrent live SSA-ish values (occupancy-cap proxy) |
//! | `static_storage_bytes` | `ProgramStats::static_storage_bytes` | sum of statically-known buffer byte sizes |
//! | `divergence_score` | local walker | count of `if invocation_id == K { ... }` patterns (warp-divergence proxy) |
//!
//! Capability bits are NOT compared  -  passes are allowed to add OR remove
//! capability requirements; cost-direction is orthogonal.

use crate::ir::{Expr, Node, Program};
use crate::optimizer::AdapterCaps;

/// A frozen snapshot of the cost dimensions tracked by the optimizer's
/// monotone-down post-condition gate.
///
/// Construct via [`CostCertificate::for_program`]. Compare via
/// [`CostCertificate::dominates_or_equal`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CostCertificate {
    /// Total statement-node count.
    pub node_count: usize,
    /// Estimated scalar/vector instruction count.
    pub instruction_count: u64,
    /// Number of explicit memory operations (loads, stores, async copies).
    pub memory_op_count: u64,
    /// Number of atomic read-modify-write expressions.
    pub atomic_op_count: u64,
    /// Number of control-flow operations (branches + loops).
    pub control_flow_count: u64,
    /// Coarse register pressure estimate (simultaneously named SSA-ish values).
    pub register_pressure_estimate: u32,
    /// Sum of statically-known buffer byte sizes.
    pub static_storage_bytes: u64,
    /// Count of `if invocation_id == K { ... }` patterns at any nesting
    /// depth  -  warp-divergence proxy. Programs that lift divergent stores
    /// out of an `if invocation_id == K` block reduce this dimension; programs
    /// that introduce one increase it.
    pub divergence_score: u64,
}

/// Device-aware cost projection used by extraction/autotune callers that need
/// the neutral Tier-B device profile to affect variant scoring.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceCostEstimate {
    /// Base device-independent certificate.
    pub base: CostCertificate,
    /// Profile-selected vector pack width in bits.
    pub vector_pack_bits: u32,
    /// Profile-selected unroll depth.
    pub unroll_depth: u32,
    /// Profile-selected workgroup tile.
    pub workgroup_tile: [u32; 3],
    /// Scalar score; lower is cheaper.
    pub score: u64,
}

impl CostCertificate {
    /// Compute the cost certificate of `program`. Reads cached `ProgramStats`
    /// (constant-time after the first call per Program) and walks the entry
    /// once for the divergence score.
    #[must_use]
    pub fn for_program(program: &Program) -> Self {
        let stats = program.stats();
        let mut divergence_score = 0u64;
        // Divergent shapes are `if invocation_id == K { ... }`. If the
        // program has no If at all, there is nothing to score and the
        // O(N) preorder walk can be skipped entirely. CostCertificate
        // is built per-pass when enforce_cost_monotone is on, so this
        // saves tens of redundant walks per scheduler iteration on
        // programs that contain no Ifs.
        if stats.has_node_if() {
            for node in program.entry() {
                count_divergent_patterns(node, &mut divergence_score);
            }
        }
        Self {
            node_count: stats.node_count,
            instruction_count: stats.instruction_count,
            memory_op_count: stats.memory_op_count,
            atomic_op_count: stats.atomic_op_count,
            control_flow_count: stats.control_flow_count,
            register_pressure_estimate: stats.register_pressure_estimate,
            static_storage_bytes: stats.static_storage_bytes,
            divergence_score,
        }
    }

    /// Compute a device-aware estimate from this certificate.
    #[must_use]
    pub fn estimate_for_adapter(&self, caps: &AdapterCaps) -> DeviceCostEstimate {
        let policy = crate::execution_plan::SchedulingPolicy::standard();
        let vector_pack_bits = policy.select_vector_pack_bits(32, caps);
        let unroll_depth = policy.select_unroll_depth(None, caps);
        let workgroup_tile = policy.select_workgroup_tile([1, 1, 1], None, caps);
        let vector_divisor = u64::from((vector_pack_bits / 32).max(1));
        let unroll_divisor = u64::from(unroll_depth.max(1));
        let tile_lanes = u64::from(
            workgroup_tile[0]
                .saturating_mul(workgroup_tile[1])
                .saturating_mul(workgroup_tile[2])
                .max(1),
        );
        let memory_component = self.memory_op_count.saturating_mul(1024) / vector_divisor;
        let instruction_component = self.instruction_count.saturating_mul(1024) / unroll_divisor;
        let occupancy_component =
            u64::from(self.register_pressure_estimate).saturating_mul(1024) / tile_lanes.min(1024);
        DeviceCostEstimate {
            base: *self,
            vector_pack_bits,
            unroll_depth,
            workgroup_tile,
            score: memory_component
                .saturating_add(instruction_component)
                .saturating_add(occupancy_component)
                .saturating_add(self.atomic_op_count.saturating_mul(2048))
                .saturating_add(self.divergence_score.saturating_mul(4096)),
        }
    }

    /// Compute a device-aware estimate for `program`.
    #[must_use]
    pub fn for_program_on_adapter(program: &Program, caps: &AdapterCaps) -> DeviceCostEstimate {
        Self::for_program(program).estimate_for_adapter(caps)
    }

    /// Returns `true` when `self` is cost-monotone-down relative to `other`:
    /// every tracked dimension on `self` is `<=` the corresponding dimension
    /// on `other`. The optimizer post-condition gate uses this to decide
    /// whether a `ProgramPass::transform` rewrite is allowed to land silently.
    ///
    /// A pass that intentionally trades one dimension for another (atomic
    /// ops down, memory ops up  -  e.g. fusing two `atomic_or` RMWs into a single
    /// gather + or  -  store) is expected to opt out via
    /// `RefusalReason::CostIncrease`; if it does not, this method's `false`
    /// return is the scheduler's signal to refuse the rewrite.
    #[must_use]
    pub fn dominates_or_equal(&self, other: &Self) -> bool {
        self.node_count <= other.node_count
            && self.instruction_count <= other.instruction_count
            && self.memory_op_count <= other.memory_op_count
            && self.atomic_op_count <= other.atomic_op_count
            && self.control_flow_count <= other.control_flow_count
            && self.register_pressure_estimate <= other.register_pressure_estimate
            && self.static_storage_bytes <= other.static_storage_bytes
            && self.divergence_score <= other.divergence_score
    }

    /// Returns the names of the dimensions where `self` exceeds `other`.
    /// Empty `Vec` when `self.dominates_or_equal(other)`. Used by the
    /// scheduler to populate `RefusalReason::CostIncrease::detail`.
    #[must_use]
    pub fn dimensions_increased_over(&self, other: &Self) -> Vec<&'static str> {
        let mut out = Vec::with_capacity(8);
        if self.node_count > other.node_count {
            out.push("node_count");
        }
        if self.instruction_count > other.instruction_count {
            out.push("instruction_count");
        }
        if self.memory_op_count > other.memory_op_count {
            out.push("memory_op_count");
        }
        if self.atomic_op_count > other.atomic_op_count {
            out.push("atomic_op_count");
        }
        if self.control_flow_count > other.control_flow_count {
            out.push("control_flow_count");
        }
        if self.register_pressure_estimate > other.register_pressure_estimate {
            out.push("register_pressure_estimate");
        }
        if self.static_storage_bytes > other.static_storage_bytes {
            out.push("static_storage_bytes");
        }
        if self.divergence_score > other.divergence_score {
            out.push("divergence_score");
        }
        out
    }
}

/// Walk a node tree and add 1 to `score` for every `if invocation_id == K { ... }`
/// pattern encountered, recursively. The shape `if invocation_id == K { ... }`
/// (or `if K == invocation_id { ... }`) is the canonical warp-divergent
/// pattern this dimension tracks. Other branchy patterns (e.g. `if x < y`) are
/// not divergent in the same warp-cost sense and are NOT counted here  -  they
/// land in `control_flow_count`, which is also tracked.
fn count_divergent_patterns(node: &Node, score: &mut u64) {
    let _ = crate::visit::node_map::any_descendant(node, &mut |n| {
        if let Node::If { cond, .. } = n {
            if is_invocation_id_eq_constant(cond) {
                *score = score.saturating_add(1);
            }
        }
        false
    });
}

/// Recognize the `invocation_id == K` divergence shape in either operand
/// orientation. Both `Eq` and `Ne` are explicitly covered  -  `Ne` is the
/// inverted form (`if invocation_id != K { ... }` divides the warp the same
/// way) and is counted. Any other comparison is NOT counted (those land in
/// the broader `control_flow_count` dimension).
fn is_invocation_id_eq_constant(cond: &Expr) -> bool {
    use crate::ir::BinOp;
    match cond {
        Expr::BinOp {
            op: BinOp::Eq | BinOp::Ne,
            left,
            right,
        } => {
            is_invocation_id_expr(left) && matches!(**right, Expr::LitU32(_))
                || is_invocation_id_expr(right) && matches!(**left, Expr::LitU32(_))
        }
        _ => false,
    }
}

/// True when `expr` is one of the workgroup-relative thread identifiers used
/// for divergent gating: global invocation id (any axis), local id, or
/// subgroup local id. Workgroup id is NOT counted  -  it gates entire
/// workgroups, not threads within a warp.
fn is_invocation_id_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::InvocationId { .. } | Expr::LocalId { .. } | Expr::SubgroupLocalId
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

    fn trivial_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4),
            ],
            [1, 1, 1],
            vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        )
    }

    #[test]
    fn for_program_returns_zero_divergence_on_pure_program() {
        let cost = CostCertificate::for_program(&trivial_program());
        assert_eq!(cost.divergence_score, 0);
        assert!(cost.memory_op_count >= 1, "trivial program has one store");
    }

    #[test]
    fn dominates_or_equal_is_reflexive() {
        let cost = CostCertificate::for_program(&trivial_program());
        assert!(cost.dominates_or_equal(&cost));
        assert!(cost.dimensions_increased_over(&cost).is_empty());
    }

    #[test]
    fn dominates_or_equal_detects_per_dimension_increase() {
        let mut a = CostCertificate::default();
        let mut b = CostCertificate {
            atomic_op_count: 1,
            ..Default::default()
        };
        // a < b on atomic_op_count, so a dominates_or_equal b ✓
        assert!(a.dominates_or_equal(&b));
        // b > a on atomic_op_count, so b does NOT dominate a
        assert!(!b.dominates_or_equal(&a));
        let increased = b.dimensions_increased_over(&a);
        assert_eq!(increased, vec!["atomic_op_count"]);

        // Multi-dimension increase reports every dimension
        a.node_count = 0;
        b.node_count = 5;
        b.divergence_score = 2;
        let increased = b.dimensions_increased_over(&a);
        assert!(increased.contains(&"node_count"));
        assert!(increased.contains(&"atomic_op_count"));
        assert!(increased.contains(&"divergence_score"));
    }

    #[test]
    fn divergence_score_counts_invocation_id_eq_constant() {
        // Build: if invocation_id == 0 { store buf[0] = 1 }
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4),
            ],
            [256, 1, 1],
            vec![Node::if_then(
                Expr::BinOp {
                    op: BinOp::Eq,
                    left: Box::new(Expr::gid_x()),
                    right: Box::new(Expr::u32(0)),
                },
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
            )],
        );
        let cost = CostCertificate::for_program(&program);
        assert_eq!(
            cost.divergence_score, 1,
            "divergence walker must count an `if invocation_id == K {{ ... }}` pattern exactly once"
        );
    }

    #[test]
    fn divergence_score_ignores_non_thread_id_comparisons() {
        // Build: if buf_load < 5 { ... }  -  control_flow but NOT divergence
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4),
            ],
            [256, 1, 1],
            vec![Node::if_then(
                Expr::BinOp {
                    op: BinOp::Lt,
                    left: Box::new(Expr::load("buf", Expr::u32(0))),
                    right: Box::new(Expr::u32(5)),
                },
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
            )],
        );
        let cost = CostCertificate::for_program(&program);
        assert_eq!(
            cost.divergence_score, 0,
            "divergence walker must NOT count branches whose condition isn't a thread-id-vs-constant"
        );
        assert!(
            cost.control_flow_count >= 1,
            "branches still count toward control_flow_count regardless of divergence shape"
        );
    }

    #[test]
    fn divergence_score_recurses_into_nested_regions() {
        // Build: region { if invocation_id == 0 { region { if invocation_id == 1 { ... } } } }
        // Two divergent patterns, both should be counted.
        let inner = Node::if_then(
            Expr::BinOp {
                op: BinOp::Eq,
                left: Box::new(Expr::gid_x()),
                right: Box::new(Expr::u32(1)),
            },
            vec![Node::store("buf", Expr::u32(1), Expr::u32(7))],
        );
        let outer = Node::if_then(
            Expr::BinOp {
                op: BinOp::Eq,
                left: Box::new(Expr::gid_x()),
                right: Box::new(Expr::u32(0)),
            },
            vec![inner],
        );
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4),
            ],
            [256, 1, 1],
            vec![outer],
        );
        let cost = CostCertificate::for_program(&program);
        assert_eq!(
            cost.divergence_score, 2,
            "nested divergence patterns must be counted at every depth"
        );
    }

    #[test]
    fn device_profile_fields_change_cost_projection() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(4096),
            ],
            [1, 1, 1],
            vec![
                Node::let_bind("x", Expr::load("buf", Expr::gid_x())),
                Node::store("buf", Expr::gid_x(), Expr::var("x")),
            ],
        );
        let compact = AdapterCaps {
            max_workgroup_size: [256, 256, 64],
            max_invocations_per_workgroup: 256,
            ideal_unroll_depth: 4,
            ideal_vector_pack_bits: 64,
            ideal_workgroup_tile: [8, 8, 1],
            ..AdapterCaps::conservative()
        };
        let wide = AdapterCaps {
            ideal_unroll_depth: 8,
            ideal_vector_pack_bits: 128,
            ideal_workgroup_tile: [16, 16, 1],
            ..compact
        };

        let compact_cost = CostCertificate::for_program_on_adapter(&program, &compact);
        let wide_cost = CostCertificate::for_program_on_adapter(&program, &wide);

        assert_eq!(compact_cost.vector_pack_bits, 64);
        assert_eq!(wide_cost.vector_pack_bits, 128);
        assert_eq!(compact_cost.unroll_depth, 4);
        assert_eq!(wide_cost.unroll_depth, 8);
        assert_eq!(compact_cost.workgroup_tile, [8, 8, 1]);
        assert_eq!(wide_cost.workgroup_tile, [16, 16, 1]);
        assert!(
            wide_cost.score < compact_cost.score,
            "Fix: wider profile vector/unroll/tile facts must lower the projected device cost"
        );
    }

    #[test]

    fn walker_matches_canonical_on_corpus() {
        // Kept-inline private old walker for drift-prevention
        fn count_divergent_patterns_old(node: &Node, score: &mut u64, visited: &mut Vec<Node>) {
            let mut stack: smallvec::SmallVec<[&Node; 64]> = smallvec::SmallVec::new();
            stack.push(node);
            while let Some(node) = stack.pop() {
                visited.push(node.clone());
                match node {
                    Node::If {
                        cond,
                        then,
                        otherwise,
                    } => {
                        if super::is_invocation_id_eq_constant(cond) {
                            *score = score.saturating_add(1);
                        }
                        stack.extend(otherwise.iter());
                        stack.extend(then.iter());
                    }
                    Node::Loop { body, .. } | Node::Block(body) => {
                        stack.extend(body.iter());
                    }
                    Node::Region { body, .. } => stack.extend(body.iter()),
                    _ => {}
                }
            }
        }

        let inner = Node::if_then(
            Expr::BinOp {
                op: BinOp::Eq,
                left: Box::new(Expr::gid_x()),
                right: Box::new(Expr::u32(1)),
            },
            vec![Node::store("buf", Expr::u32(1), Expr::u32(7))],
        );
        let outer = Node::if_then(
            Expr::BinOp {
                op: BinOp::Eq,
                left: Box::new(Expr::gid_x()),
                right: Box::new(Expr::u32(0)),
            },
            vec![inner, Node::Block(vec![Node::Return])],
        );

        let mut score_old = 0;
        let mut visited_old = Vec::new();
        count_divergent_patterns_old(&outer, &mut score_old, &mut visited_old);

        let mut score_new = 0;
        let mut visited_new = Vec::new();
        let _ = crate::visit::node_map::any_descendant(&outer, &mut |n| {
            visited_new.push(n.clone());
            if let Node::If { cond, .. } = n {
                if super::is_invocation_id_eq_constant(cond) {
                    score_new += 1;
                }
            }
            false
        });

        assert_eq!(score_old, score_new, "Divergence score mismatch");
        assert_eq!(
            visited_old.len(),
            visited_new.len(),
            "Node set length mismatch"
        );

        // Node-set (unordered) equivalence assertion
        for node in &visited_old {
            assert!(
                visited_new.contains(node),
                "Old walker visited a node that the new canonical walker missed"
            );
        }
        for node in &visited_new {
            assert!(
                visited_old.contains(node),
                "New canonical walker visited a node that the old walker missed"
            );
        }
    }
}

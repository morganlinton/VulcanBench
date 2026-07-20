//! Warp-Aggregated Hierarchical Atomics using Subgroup Primitives
//!
//! Replaces $O(N)$ sequential `atomic_add` congestion with $O(1)$ batch
//! allocation using `Expr::SubgroupBallot` and `Expr::SubgroupAdd`.
//!
//! Important execution constraint: Subgroup operations MUST be evaluated in
//! uniform control flow. They cannot be placed inside an `if_then` that
//! diverges across the subgroup.

use vyre_foundation::ir::{Expr, Node};

mod queue_state_word {
    pub(super) const HIT_HEAD: usize = 2;
    pub(super) const HIT_CAPACITY: usize = 3;
}

/// Host-independent binding names used by the hierarchical hit writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitRingBindings {
    /// Queue-state buffer name.
    pub queue_state: &'static str,
    /// Sparse hit-ring buffer name.
    pub hit_ring: &'static str,
    /// Variable holding the file table index.
    pub file_idx: &'static str,
    /// Variable holding the rule table index.
    pub rule_idx: &'static str,
    /// Variable holding the decoded layer index.
    pub layer_idx: &'static str,
    /// Variable holding the byte position within the file.
    pub byte_pos: &'static str,
    /// Variable holding the first byte position of the file.
    pub file_start: &'static str,
}

impl Default for HitRingBindings {
    fn default() -> Self {
        Self {
            queue_state: "queue_state",
            hit_ring: "hit_ring",
            file_idx: "file_idx",
            rule_idx: "rule_idx",
            layer_idx: "layer_idx",
            byte_pos: "byte_pos",
            file_start: "file_start",
        }
    }
}

/// Generates the AST for a mathematically equivalent, 32x faster hit-recording
/// routine that does a single atomic global operation per subgroup.
#[must_use]
pub fn record_hit_to_ring_hierarchical(is_hit_var: &str) -> Vec<Node> {
    record_hit_to_ring_hierarchical_with(is_hit_var, &HitRingBindings::default())
}

/// Generate a subgroup-aggregated sparse-hit writer for custom buffer names.
///
/// The returned fragment is safe to place only in uniform control flow. Each
/// lane passes a boolean/int predicate via `is_hit_var`; lanes with a false
/// predicate reserve no slot and perform no writes.
#[must_use]
pub fn record_hit_to_ring_hierarchical_with(
    is_hit_var: &str,
    bindings: &HitRingBindings,
) -> Vec<Node> {
    vec![
        Node::let_bind(
            "subgroup_hit_mask",
            Expr::subgroup_ballot(Expr::var(is_hit_var)),
        ),
        Node::let_bind(
            "subgroup_hit_count",
            Expr::subgroup_add(Expr::select(
                Expr::var(is_hit_var),
                Expr::u32(1),
                Expr::u32(0),
            )),
        ),
        Node::let_bind("leader_base_slot", Expr::u32(0)),
        Node::if_then(
            Expr::eq(Expr::subgroup_local_id(), Expr::u32(0)),
            vec![Node::assign(
                "leader_base_slot",
                Expr::atomic_add(
                    bindings.queue_state,
                    Expr::u32(queue_state_word::HIT_HEAD as u32),
                    Expr::var("subgroup_hit_count"),
                ),
            )],
        ),
        Node::let_bind(
            "global_hit_base",
            Expr::subgroup_shuffle(Expr::var("leader_base_slot"), Expr::u32(0)),
        ),
        Node::let_bind(
            "lane_lower_mask",
            Expr::sub(
                Expr::shl(Expr::u32(1), Expr::subgroup_local_id()),
                Expr::u32(1),
            ),
        ),
        Node::let_bind(
            "lane_hit_offset",
            Expr::popcount(Expr::bitand(
                Expr::var("subgroup_hit_mask"),
                Expr::var("lane_lower_mask"),
            )),
        ),
        Node::let_bind(
            "hit_slot",
            Expr::add(Expr::var("global_hit_base"), Expr::var("lane_hit_offset")),
        ),
        Node::if_then(
            Expr::and(
                Expr::var(is_hit_var),
                Expr::lt(
                    Expr::var("hit_slot"),
                    Expr::load(
                        bindings.queue_state,
                        Expr::u32(queue_state_word::HIT_CAPACITY as u32),
                    ),
                ),
            ),
            write_hit_element(bindings),
        ),
    ]
}

fn write_hit_element(bindings: &HitRingBindings) -> Vec<Node> {
    vec![
        Node::let_bind("hit_base", Expr::mul(Expr::var("hit_slot"), Expr::u32(4))),
        Node::store(
            bindings.hit_ring,
            Expr::var("hit_base"),
            Expr::var(bindings.file_idx),
        ),
        Node::store(
            bindings.hit_ring,
            Expr::add(Expr::var("hit_base"), Expr::u32(1)),
            Expr::var(bindings.rule_idx),
        ),
        Node::store(
            bindings.hit_ring,
            Expr::add(Expr::var("hit_base"), Expr::u32(2)),
            Expr::var(bindings.layer_idx),
        ),
        Node::store(
            bindings.hit_ring,
            Expr::add(Expr::var("hit_base"), Expr::u32(3)),
            Expr::sub(Expr::var(bindings.byte_pos), Expr::var(bindings.file_start)),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchical_hit_writer_emits_real_ring_stores() {
        let nodes = record_hit_to_ring_hierarchical("is_hit");
        let store_count = count_stores(&nodes);
        assert_eq!(store_count, 4);
        assert!(contains_subgroup(&nodes));
        assert!(
            contains_subgroup_local_id(&nodes),
            "subgroup aggregation must elect one leader per subgroup, not only workgroup lane 0"
        );
    }

    fn count_stores(nodes: &[Node]) -> usize {
        nodes.iter().fold(0, |count, node| {
            count
                + match node {
                    Node::Store { .. } => 1,
                    Node::Block(body) => count_stores(body),
                    Node::If {
                        then, otherwise, ..
                    } => count_stores(then) + count_stores(otherwise),
                    Node::Loop { body, .. } => count_stores(body),
                    Node::Region { body, .. } => count_stores(body),
                    _ => 0,
                }
        })
    }

    fn contains_subgroup(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| {
            matches!(
                node,
                Node::Let {
                    value: Expr::SubgroupBallot { .. }
                        | Expr::SubgroupReduce { .. }
                        | Expr::SubgroupShuffle { .. },
                    ..
                }
            )
        })
    }

    fn contains_subgroup_local_id(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::Let { value, .. } => expr_contains_subgroup_local_id(value),
            Node::Store { index, value, .. } => {
                expr_contains_subgroup_local_id(index) || expr_contains_subgroup_local_id(value)
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                expr_contains_subgroup_local_id(cond)
                    || contains_subgroup_local_id(then)
                    || contains_subgroup_local_id(otherwise)
            }
            Node::Block(body) | Node::Loop { body, .. } => contains_subgroup_local_id(body),
            Node::Region { body, .. } => contains_subgroup_local_id(body),
            _ => false,
        })
    }

    fn expr_contains_subgroup_local_id(expr: &Expr) -> bool {
        match expr {
            Expr::SubgroupLocalId => true,
            Expr::BinOp { left, right, .. } => {
                expr_contains_subgroup_local_id(left) || expr_contains_subgroup_local_id(right)
            }
            Expr::UnOp { operand, .. } => expr_contains_subgroup_local_id(operand),
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                expr_contains_subgroup_local_id(cond)
                    || expr_contains_subgroup_local_id(true_val)
                    || expr_contains_subgroup_local_id(false_val)
            }
            Expr::Cast { value, .. } | Expr::SubgroupBallot { cond: value } => {
                expr_contains_subgroup_local_id(value)
            }
            Expr::SubgroupShuffle { value, lane } => {
                expr_contains_subgroup_local_id(value) || expr_contains_subgroup_local_id(lane)
            }
            Expr::SubgroupReduce { value, .. } => expr_contains_subgroup_local_id(value),
            Expr::Load { index, .. } => expr_contains_subgroup_local_id(index),
            Expr::Atomic {
                index,
                expected,
                value,
                ..
            } => {
                expr_contains_subgroup_local_id(index)
                    || expected
                        .as_deref()
                        .is_some_and(expr_contains_subgroup_local_id)
                    || expr_contains_subgroup_local_id(value)
            }
            _ => false,
        }
    }
}

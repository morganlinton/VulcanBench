//! Shared recursive load-site counting for memory-placement analyses.
//!
//! Texture promotion, AoS-to-SoA layout rewrites, and later cache-placement
//! analyses all need the same conservative traversal over structured kernel
//! bodies. Keeping the traversal here prevents each analysis from guessing
//! which operands are child-body IDs.

use rustc_hash::FxHashMap;

use crate::{KernelBody, KernelOpKind};

pub(crate) fn count_global_loads_by_slot<F>(
    body: &KernelBody,
    is_eligible_slot: &F,
    counts: &mut FxHashMap<u32, u32>,
) where
    F: Fn(u32) -> bool,
{
    for op in &body.ops {
        if matches!(op.kind, KernelOpKind::LoadGlobal) {
            if let Some(slot) = op.operands.first() {
                if is_eligible_slot(*slot) {
                    *counts.entry(*slot).or_insert(0) += 1;
                }
            }
        }
        for child_id in child_body_operands(&op.kind, &op.operands) {
            if let Some(child) = body.child_bodies.get(child_id as usize) {
                count_global_loads_by_slot(child, is_eligible_slot, counts);
            }
        }
    }
}

fn child_body_operands<'a>(
    kind: &KernelOpKind,
    operands: &'a [u32],
) -> impl Iterator<Item = u32> + 'a {
    let start = match kind {
        KernelOpKind::StructuredIfThen | KernelOpKind::StructuredIfThenElse => 1,
        KernelOpKind::StructuredForLoop { .. } => 2,
        KernelOpKind::StructuredBlock | KernelOpKind::Region { .. } => 0,
        _ => operands.len(),
    };
    operands.iter().skip(start).copied()
}

#[cfg(test)]
mod tests {
    use rustc_hash::FxHashMap;

    use super::*;
    use crate::{KernelOp, LiteralValue};

    fn body_with_load(slot: u32) -> KernelBody {
        KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![slot, 0],
                result: Some(slot),
            }],
            child_bodies: vec![],
            literals: vec![],
        }
    }

    #[test]
    fn counts_if_else_children_and_ignores_for_loop_bound_operands() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "i".into(),
                    },
                    operands: vec![0, 1, 2],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThenElse,
                    operands: vec![99, 3, 4],
                    result: None,
                },
            ],
            child_bodies: vec![
                body_with_load(7),
                body_with_load(7),
                body_with_load(7),
                body_with_load(7),
                body_with_load(7),
            ],
            literals: vec![LiteralValue::U32(0)],
        };

        let mut counts = FxHashMap::default();
        count_global_loads_by_slot(&body, &|slot| slot == 7, &mut counts);

        assert_eq!(
            counts.get(&7).copied(),
            Some(3),
            "Fix: load counting must include real structured child bodies without treating loop bound operands as child indices."
        );
    }
}

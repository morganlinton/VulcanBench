//! Shared literal helpers for lowered-IR rewrite passes.
//!
//! Arithmetic, bitwise, and loop rewrites all need the same question:
//! "does this SSA id come from a U32 literal op?" Keeping that helper
//! single-sourced prevents nine local copies from drifting.

use crate::{KernelBody, KernelOp, KernelOpKind, LiteralValue};
use rustc_hash::FxHashMap;

pub(super) fn intern_literal(literals: &mut Vec<LiteralValue>, value: LiteralValue) -> u32 {
    if let Some(idx) = literals.iter().position(|lit| lit == &value) {
        return idx as u32;
    }
    let idx = literals.len() as u32;
    literals.push(value);
    idx
}

pub(super) struct ResultAllocator {
    next: u32,
}

impl ResultAllocator {
    pub(super) fn for_body_tree(body: &KernelBody) -> Self {
        fn walk(body: &KernelBody, next: &mut u32) {
            for op in &body.ops {
                for result in op.result_ids() {
                    *next = (*next).max(result.saturating_add(1));
                }
            }
            for child in &body.child_bodies {
                walk(child, next);
            }
        }

        let mut next = 0;
        walk(body, &mut next);
        Self { next }
    }

    pub(super) fn fresh(&mut self) -> u32 {
        let result = self.next;
        self.next = self.next.saturating_add(1);
        result
    }

    pub(super) fn fresh_block(&mut self, count: u32) -> u32 {
        let result = self.next;
        self.next = self.next.saturating_add(count);
        result
    }

    pub(super) fn push_result(
        &mut self,
        ops: &mut Vec<KernelOp>,
        kind: KernelOpKind,
        operands: Vec<u32>,
    ) -> u32 {
        let result = self.fresh();
        ops.push(KernelOp {
            kind,
            operands,
            result: Some(result),
        });
        result
    }

    pub(super) fn push_literal(
        &mut self,
        ops: &mut Vec<KernelOp>,
        literals: &mut Vec<LiteralValue>,
        value: LiteralValue,
    ) -> u32 {
        let pool_index = intern_literal(literals, value);
        self.push_result(ops, KernelOpKind::Literal, vec![pool_index])
    }
}

pub(super) fn u32_literals_by_result(body: &KernelBody) -> FxHashMap<u32, u32> {
    let mut out = FxHashMap::default();
    for op in &body.ops {
        if !matches!(op.kind, KernelOpKind::Literal) {
            continue;
        }
        let Some(result) = op.result else {
            continue;
        };
        let Some(pool_index) = op.operands.first() else {
            continue;
        };
        let Some(LiteralValue::U32(value)) = body.literals.get(*pool_index as usize) else {
            continue;
        };
        out.insert(result, *value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KernelOp, LiteralValue};

    #[test]
    fn u32_literals_by_result_collects_only_well_formed_u32_literals() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(11),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![99],
                    result: Some(12),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::GlobalInvocationId,
                    operands: vec![0],
                    result: Some(13),
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::U32(7), LiteralValue::Bool(true)],
        };

        let literals = u32_literals_by_result(&body);
        assert_eq!(literals.len(), 1);
        assert_eq!(literals.get(&10), Some(&7));
        assert!(!literals.contains_key(&11));
        assert!(!literals.contains_key(&12));
        assert!(!literals.contains_key(&13));
    }

    #[test]
    fn generated_literal_interning_reuses_pool_slots_and_allocates_new_values() {
        let mut checked = 0_u32;
        let mut literals = vec![LiteralValue::Bool(false), LiteralValue::U32(7)];
        for value in 0_u32..=2_048 {
            let before = literals.len();
            let idx = intern_literal(&mut literals, LiteralValue::U32(value));
            if value == 7 {
                assert_eq!(idx, 1);
                assert_eq!(literals.len(), before);
            } else {
                assert_eq!(idx as usize, before);
                assert_eq!(literals[idx as usize], LiteralValue::U32(value));
            }
            checked += 1;
        }
        assert_eq!(checked, 2_049);
    }

    #[test]
    fn result_allocator_tracks_sparse_descriptor_results() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::GlobalInvocationId,
                    operands: vec![0],
                    result: Some(99),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: None,
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::U32(1)],
        };

        let mut allocator = ResultAllocator::for_body_tree(&body);
        assert_eq!(allocator.fresh(), 100);
    }

    #[test]
    fn result_allocator_walks_child_bodies_and_pushes_interned_literals() {
        let body = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::GlobalInvocationId,
                operands: vec![0],
                result: Some(9),
            }],
            child_bodies: vec![KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(99),
                }],
                child_bodies: Vec::new(),
                literals: vec![LiteralValue::U32(7)],
            }],
            literals: vec![LiteralValue::U32(7)],
        };
        let mut allocator = ResultAllocator::for_body_tree(&body);
        let mut ops = Vec::new();
        let mut literals = vec![LiteralValue::U32(7)];

        let literal_id = allocator.push_literal(&mut ops, &mut literals, LiteralValue::U32(7));
        let copy_id = allocator.push_result(&mut ops, KernelOpKind::Copy, vec![literal_id]);

        assert_eq!(literal_id, 100);
        assert_eq!(copy_id, 101);
        assert_eq!(literals, vec![LiteralValue::U32(7)]);
        assert_eq!(ops[0].operands, vec![0]);
    }

    #[test]
    fn result_allocator_reserves_contiguous_result_blocks() {
        let body = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(4),
            }],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::U32(1)],
        };
        let mut allocator = ResultAllocator::for_body_tree(&body);

        let base = allocator.fresh_block(4);
        let after = allocator.fresh();

        assert_eq!(base, 5);
        assert_eq!(after, 9);
    }
}

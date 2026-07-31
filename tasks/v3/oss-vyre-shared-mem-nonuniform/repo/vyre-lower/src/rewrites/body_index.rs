//! Shared SSA producer/use index for lowered-IR rewrite passes.

use rustc_hash::FxHashMap;

use crate::{KernelBody, KernelOp, KernelOpKind, LiteralValue};

pub(super) struct BodyIndex {
    result_to_idx: FxHashMap<u32, usize>,
    use_count: FxHashMap<u32, u32>,
}

impl BodyIndex {
    pub(super) fn new(body: &KernelBody) -> Self {
        let result_to_idx = body
            .ops
            .iter()
            .enumerate()
            .filter_map(|(idx, op)| op.result.map(|result| (result, idx)))
            .collect();

        let mut use_count = FxHashMap::default();
        for op in &body.ops {
            for operand in &op.operands {
                *use_count.entry(*operand).or_insert(0) += 1;
            }
        }

        Self {
            result_to_idx,
            use_count,
        }
    }

    pub(super) fn producer<'a>(
        &self,
        body: &'a KernelBody,
        result_id: u32,
    ) -> Option<&'a KernelOp> {
        body.ops.get(*self.result_to_idx.get(&result_id)?)
    }

    pub(super) fn producer_index(&self, result_id: u32) -> Option<usize> {
        self.result_to_idx.get(&result_id).copied()
    }

    pub(super) fn literal_value<'a>(
        &self,
        body: &'a KernelBody,
        result_id: u32,
    ) -> Option<&'a LiteralValue> {
        let producer = self.producer(body, result_id)?;
        if !matches!(producer.kind, KernelOpKind::Literal) {
            return None;
        }
        let pool_idx = *producer.operands.first()? as usize;
        body.literals.get(pool_idx)
    }

    pub(super) fn u32_lit(&self, body: &KernelBody, result_id: u32) -> Option<u32> {
        match self.literal_value(body, result_id)? {
            LiteralValue::U32(value) => Some(*value),
            _ => None,
        }
    }

    pub(super) fn bool_lit(&self, body: &KernelBody, result_id: u32) -> Option<bool> {
        match self.literal_value(body, result_id)? {
            LiteralValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub(super) fn use_count_of(&self, result_id: u32) -> u32 {
        self.use_count.get(&result_id).copied().unwrap_or(0)
    }

    pub(super) fn has_single_consumer(&self, result_id: u32) -> bool {
        self.use_count_of(result_id) == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KernelOpKind, LiteralValue};

    #[test]
    fn body_index_maps_sparse_results_and_counts_operand_uses() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::Copy,
                    operands: vec![10],
                    result: Some(20),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 4, 20],
                    result: None,
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::U32(7)],
        };

        let index = BodyIndex::new(&body);
        assert!(matches!(
            index.producer(&body, 10).map(|op| &op.kind),
            Some(KernelOpKind::Literal)
        ));
        assert!(matches!(
            index.producer(&body, 20).map(|op| &op.kind),
            Some(KernelOpKind::Copy)
        ));
        assert_eq!(index.use_count_of(10), 1);
        assert_eq!(index.use_count_of(20), 1);
        assert_eq!(index.use_count_of(99), 0);
        assert_eq!(index.u32_lit(&body, 10), Some(7));
        assert_eq!(index.bool_lit(&body, 10), None);
    }

    #[test]
    fn generated_body_index_detects_single_consumer_chains() {
        let mut ops = Vec::new();
        for id in 0..=2_048u32 {
            ops.push(KernelOp {
                kind: KernelOpKind::Copy,
                operands: if id == 0 { Vec::new() } else { vec![id - 1] },
                result: Some(id),
            });
        }
        let body = KernelBody {
            ops,
            child_bodies: Vec::new(),
            literals: Vec::new(),
        };

        let index = BodyIndex::new(&body);
        for id in 0..2_048u32 {
            assert!(
                index.has_single_consumer(id),
                "result {id} must feed exactly one chain consumer"
            );
        }
        assert_eq!(index.use_count_of(2_048), 0);
    }

    #[test]
    fn body_index_literal_accessors_reject_malformed_and_nonliteral_results() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![99],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Copy,
                    operands: vec![2],
                    result: Some(3),
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::Bool(true)],
        };
        let index = BodyIndex::new(&body);

        assert_eq!(index.literal_value(&body, 1), None);
        assert_eq!(index.bool_lit(&body, 2), Some(true));
        assert_eq!(index.u32_lit(&body, 2), None);
        assert_eq!(index.literal_value(&body, 3), None);
    }
}

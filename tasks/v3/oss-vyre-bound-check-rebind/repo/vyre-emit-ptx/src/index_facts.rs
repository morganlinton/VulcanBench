use rustc_hash::FxHashMap;
use vyre_foundation::ir::BinOp;
use vyre_lower::{KernelBody, KernelOp, KernelOpKind, LiteralValue};

pub(crate) struct IndexFacts {
    producer: FxHashMap<u32, usize>,
    consumer_indices: FxHashMap<u32, Vec<usize>>,
    lit_u32: FxHashMap<u32, u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NormalizedIndex {
    root: Option<u32>,
    offset: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AffineModulo {
    root: Option<u32>,
    coeff: u32,
    offset: u32,
}

const AFFINE_ROOT_GLOBAL_INVOCATION: u32 = u32::MAX - 2;
const AFFINE_ROOT_LOCAL_INVOCATION: u32 = u32::MAX - 5;
const AFFINE_ROOT_WORKGROUP: u32 = u32::MAX - 8;

impl IndexFacts {
    pub(crate) fn new(body: &KernelBody) -> Self {
        let mut producer = FxHashMap::with_capacity_and_hasher(body.ops.len(), Default::default());
        let mut consumer_indices =
            FxHashMap::with_capacity_and_hasher(body.ops.len(), Default::default());
        let mut lit_u32 = FxHashMap::with_capacity_and_hasher(body.ops.len(), Default::default());
        for (idx, op) in body.ops.iter().enumerate() {
            let Some(result_id) = op.result else {
                continue;
            };
            producer.insert(result_id, idx);
            if matches!(op.kind, KernelOpKind::MatrixMma { .. }) {
                for offset in 1..4 {
                    if let Some(fragment_id) = result_id.checked_add(offset) {
                        producer.insert(fragment_id, idx);
                    }
                }
            }
            if !matches!(op.kind, KernelOpKind::Literal) {
                continue;
            }
            let Some(&pool_idx) = op.operands.first() else {
                continue;
            };
            let Some(literal) = body.literals.get(pool_idx as usize) else {
                continue;
            };
            let value = match literal {
                LiteralValue::U32(value) => Some(*value),
                LiteralValue::I32(value) => Some(*value as u32),
                _ => None,
            };
            if let Some(value) = value {
                lit_u32.insert(result_id, value);
            }
        }
        for (op_idx, op) in body.ops.iter().enumerate() {
            visit_value_operands(op, |operand| {
                if !producer.contains_key(&operand) {
                    return;
                }
                consumer_indices
                    .entry(operand)
                    .or_insert_with(Vec::new)
                    .push(op_idx);
            });
        }
        Self {
            producer,
            consumer_indices,
            lit_u32,
        }
    }

    pub(crate) fn is_index_plus_one(
        &self,
        body: &KernelBody,
        candidate_id: u32,
        prev_id: u32,
    ) -> bool {
        if let (Some(candidate), Some(prev)) =
            (self.lit_u32.get(&candidate_id), self.lit_u32.get(&prev_id))
        {
            return prev.checked_add(1) == Some(*candidate);
        }
        if let (Some(candidate), Some(prev)) = (
            self.normalized_index(body, candidate_id),
            self.normalized_index(body, prev_id),
        ) {
            if candidate.root == prev.root && prev.offset.checked_add(1) == Some(candidate.offset) {
                return true;
            }
        }
        let Some(&op_idx) = self.producer.get(&candidate_id) else {
            return false;
        };
        let op = &body.ops[op_idx];
        let KernelOpKind::BinOpKind(BinOp::Add) = op.kind else {
            return false;
        };
        if op.operands.len() != 2 {
            return false;
        }
        let lhs = op.operands[0];
        let rhs = op.operands[1];
        let is_one = |id: u32| self.lit_u32.get(&id) == Some(&1);
        (lhs == prev_id && is_one(rhs)) || (rhs == prev_id && is_one(lhs))
    }

    #[cfg(test)]
    pub(crate) fn index_is_multiple_of(
        &self,
        body: &KernelBody,
        result_id: u32,
        modulus: u32,
    ) -> bool {
        if modulus <= 1 {
            return true;
        }
        self.index_mod(body, result_id, modulus, 0) == Some(0)
    }

    pub(crate) fn index_modulo(
        &self,
        body: &KernelBody,
        result_id: u32,
        modulus: u32,
    ) -> Option<u32> {
        self.index_mod(body, result_id, modulus, 0)
    }

    fn index_mod(&self, body: &KernelBody, result_id: u32, modulus: u32, depth: u8) -> Option<u32> {
        if modulus == 0 || depth > 8 {
            return None;
        }
        let affine = self.affine_mod(body, result_id, modulus, depth)?;
        return (affine.coeff == 0).then_some(affine.offset % modulus);
    }

    fn affine_mod(
        &self,
        body: &KernelBody,
        result_id: u32,
        modulus: u32,
        depth: u8,
    ) -> Option<AffineModulo> {
        if modulus == 0 || depth > 8 {
            return None;
        }
        if let Some(value) = self.lit_u32.get(&result_id).copied() {
            return Some(AffineModulo {
                root: None,
                coeff: 0,
                offset: value % modulus,
            });
        }
        let Some(&op_idx) = self.producer.get(&result_id) else {
            return Some(AffineModulo {
                root: Some(result_id),
                coeff: 1 % modulus,
                offset: 0,
            });
        };
        let op = &body.ops[op_idx];
        if op.operands.len() != 2 {
            return Some(AffineModulo {
                root: Some(symbolic_affine_root(op, result_id)),
                coeff: 1 % modulus,
                offset: 0,
            });
        }
        let lhs = op.operands[0];
        let rhs = op.operands[1];
        match op.kind {
            KernelOpKind::BinOpKind(BinOp::Add | BinOp::WrappingAdd) => {
                let lhs_mod = self.affine_mod(body, lhs, modulus, depth + 1)?;
                let rhs_mod = self.affine_mod(body, rhs, modulus, depth + 1)?;
                combine_affine_add(lhs_mod, rhs_mod, modulus)
            }
            KernelOpKind::BinOpKind(BinOp::Mul) => {
                if let Some(value) = self.lit_u32.get(&lhs).copied() {
                    let rhs_mod = self.affine_mod(body, rhs, modulus, depth + 1)?;
                    return Some(scale_affine(rhs_mod, value, modulus));
                }
                if let Some(value) = self.lit_u32.get(&rhs).copied() {
                    let lhs_mod = self.affine_mod(body, lhs, modulus, depth + 1)?;
                    return Some(scale_affine(lhs_mod, value, modulus));
                }
                Some(AffineModulo {
                    root: Some(symbolic_affine_root(op, result_id)),
                    coeff: 1 % modulus,
                    offset: 0,
                })
            }
            KernelOpKind::BinOpKind(BinOp::Shl) => {
                let shift = self.lit_u32.get(&rhs).copied()? & 31;
                let factor = 1u32 << shift;
                let lhs_mod = self.affine_mod(body, lhs, modulus, depth + 1)?;
                Some(scale_affine(lhs_mod, factor, modulus))
            }
            _ => Some(AffineModulo {
                root: Some(symbolic_affine_root(op, result_id)),
                coeff: 1 % modulus,
                offset: 0,
            }),
        }
    }

    fn normalized_index(&self, body: &KernelBody, result_id: u32) -> Option<NormalizedIndex> {
        self.normalized_index_inner(body, result_id, 0)
    }

    fn normalized_index_inner(
        &self,
        body: &KernelBody,
        result_id: u32,
        depth: u8,
    ) -> Option<NormalizedIndex> {
        if depth > 8 {
            return Some(NormalizedIndex {
                root: Some(result_id),
                offset: 0,
            });
        }
        if let Some(value) = self.lit_u32.get(&result_id).copied() {
            return Some(NormalizedIndex {
                root: None,
                offset: value,
            });
        }
        let Some(&op_idx) = self.producer.get(&result_id) else {
            return Some(NormalizedIndex {
                root: Some(result_id),
                offset: 0,
            });
        };
        let op = &body.ops[op_idx];
        if !matches!(
            op.kind,
            KernelOpKind::BinOpKind(BinOp::Add | BinOp::WrappingAdd)
        ) || op.operands.len() != 2
        {
            return Some(NormalizedIndex {
                root: Some(result_id),
                offset: 0,
            });
        }

        let lhs = op.operands[0];
        let rhs = op.operands[1];
        if let Some(delta) = self.lit_u32.get(&rhs).copied() {
            let base = self.normalized_index_inner(body, lhs, depth + 1)?;
            return Some(NormalizedIndex {
                root: base.root,
                offset: base.offset.checked_add(delta)?,
            });
        }
        if let Some(delta) = self.lit_u32.get(&lhs).copied() {
            let base = self.normalized_index_inner(body, rhs, depth + 1)?;
            return Some(NormalizedIndex {
                root: base.root,
                offset: base.offset.checked_add(delta)?,
            });
        }

        Some(NormalizedIndex {
            root: Some(result_id),
            offset: 0,
        })
    }

    pub(crate) fn producer_idx(&self, result_id: u32) -> Option<usize> {
        self.producer.get(&result_id).copied()
    }

    pub(crate) fn result_use_count(&self, result_id: u32) -> usize {
        self.consumer_indices
            .get(&result_id)
            .map(Vec::len)
            .unwrap_or(0)
    }

    pub(crate) fn consumer_indices(&self, result_id: u32) -> Option<&[usize]> {
        self.consumer_indices.get(&result_id).map(Vec::as_slice)
    }

    #[cfg(test)]
    pub(crate) fn single_consumer_idx(&self, result_id: u32) -> Option<usize> {
        match self.consumer_indices(result_id)? {
            [index] => Some(*index),
            _ => None,
        }
    }
}

fn visit_value_operands(op: &KernelOp, mut visit: impl FnMut(u32)) {
    match &op.kind {
        KernelOpKind::Literal
        | KernelOpKind::LocalInvocationId
        | KernelOpKind::GlobalInvocationId
        | KernelOpKind::WorkgroupId
        | KernelOpKind::SubgroupLocalId
        | KernelOpKind::SubgroupSize
        | KernelOpKind::LoopIndex { .. }
        | KernelOpKind::LoopCarrier { .. }
        | KernelOpKind::BufferLength
        | KernelOpKind::StructuredBlock
        | KernelOpKind::Return
        | KernelOpKind::Barrier { .. }
        | KernelOpKind::Region { .. }
        | KernelOpKind::AsyncWait { .. }
        | KernelOpKind::Resume { .. }
        | KernelOpKind::IndirectDispatch { .. } => {}
        KernelOpKind::LoadGlobal | KernelOpKind::LoadShared | KernelOpKind::LoadConstant => {
            if let Some(&index_id) = op.operands.get(1) {
                visit(index_id);
            }
        }
        KernelOpKind::StoreGlobal | KernelOpKind::StoreShared | KernelOpKind::Atomic { .. } => {
            for &operand in op.operands.iter().skip(1) {
                visit(operand);
            }
        }
        KernelOpKind::StructuredIfThen | KernelOpKind::StructuredIfThenElse => {
            if let Some(&condition_id) = op.operands.first() {
                visit(condition_id);
            }
        }
        KernelOpKind::StructuredForLoop { .. } => {
            for &operand in op.operands.iter().take(2) {
                visit(operand);
            }
        }
        KernelOpKind::AsyncLoad { .. } | KernelOpKind::AsyncStore { .. } => {
            for &operand in op.operands.iter().skip(2) {
                visit(operand);
            }
        }
        KernelOpKind::Copy
        | KernelOpKind::LoopCarrierInit { .. }
        | KernelOpKind::LoopCarrierEnd { .. }
        | KernelOpKind::BinOpKind(_)
        | KernelOpKind::UnOpKind(_)
        | KernelOpKind::Fma
        | KernelOpKind::MatrixMma { .. }
        | KernelOpKind::Select
        | KernelOpKind::Cast { .. }
        | KernelOpKind::SubgroupBallot
        | KernelOpKind::SubgroupShuffle
        | KernelOpKind::SubgroupBroadcast
        | KernelOpKind::SubgroupReduce { .. }
        | KernelOpKind::Trap { .. }
        | KernelOpKind::Call { .. }
        | KernelOpKind::OpaqueExpr(_)
        | KernelOpKind::OpaqueNode(_) => {
            for &operand in &op.operands {
                visit(operand);
            }
        }
    }
}

fn combine_affine_add(lhs: AffineModulo, rhs: AffineModulo, modulus: u32) -> Option<AffineModulo> {
    let root = match (lhs.root, rhs.root) {
        (None, root) | (root, None) => root,
        (Some(left), Some(right)) if left == right => Some(left),
        _ => return None,
    };
    Some(AffineModulo {
        root,
        coeff: ((u64::from(lhs.coeff) + u64::from(rhs.coeff)) % u64::from(modulus)) as u32,
        offset: ((u64::from(lhs.offset) + u64::from(rhs.offset)) % u64::from(modulus)) as u32,
    })
}

fn scale_affine(value: AffineModulo, factor: u32, modulus: u32) -> AffineModulo {
    AffineModulo {
        root: value.root,
        coeff: ((u64::from(value.coeff) * u64::from(factor % modulus)) % u64::from(modulus)) as u32,
        offset: ((u64::from(value.offset) * u64::from(factor % modulus)) % u64::from(modulus))
            as u32,
    }
}

fn symbolic_affine_root(op: &KernelOp, result_id: u32) -> u32 {
    let axis = op.operands.first().copied().unwrap_or(0).min(2);
    match op.kind {
        KernelOpKind::GlobalInvocationId => AFFINE_ROOT_GLOBAL_INVOCATION + axis,
        KernelOpKind::LocalInvocationId => AFFINE_ROOT_LOCAL_INVOCATION + axis,
        KernelOpKind::WorkgroupId => AFFINE_ROOT_WORKGROUP + axis,
        _ => result_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_lower::{
        KernelBody, KernelOp, LiteralValue, MatrixMmaElement, MatrixMmaLayout, MatrixMmaShape,
    };

    fn body_with_add(
        operands: Vec<u32>,
        result: Option<u32>,
        literals: Vec<LiteralValue>,
    ) -> KernelBody {
        KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands,
                    result,
                },
            ],
            child_bodies: Vec::new(),
            literals,
        }
    }

    #[test]
    fn detects_unit_stride_add_in_either_operand_order() {
        let body = body_with_add(vec![7, 1], Some(9), vec![LiteralValue::U32(1)]);
        let facts = IndexFacts::new(&body);
        assert!(facts.is_index_plus_one(&body, 9, 7));

        let body = body_with_add(vec![1, 7], Some(9), vec![LiteralValue::I32(1)]);
        let facts = IndexFacts::new(&body);
        assert!(facts.is_index_plus_one(&body, 9, 7));
    }

    #[test]
    fn detects_adjacent_folded_literal_indices() {
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
            ],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::U32(8), LiteralValue::U32(9)],
        };
        let facts = IndexFacts::new(&body);
        assert!(facts.is_index_plus_one(&body, 11, 10));
        assert!(!facts.is_index_plus_one(&body, 10, 11));
    }

    #[test]
    fn detects_adjacent_dynamic_indices_after_affine_reassociation() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![7, 1],
                    result: Some(9),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![7, 2],
                    result: Some(10),
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::U32(1), LiteralValue::U32(2)],
        };
        let facts = IndexFacts::new(&body);
        assert!(facts.is_index_plus_one(&body, 10, 9));
        assert!(!facts.is_index_plus_one(&body, 9, 10));
    }

    #[test]
    fn detects_adjacent_dynamic_indices_after_chained_reassociation() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![7, 1],
                    result: Some(9),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::WrappingAdd),
                    operands: vec![9, 1],
                    result: Some(10),
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::U32(1)],
        };
        let facts = IndexFacts::new(&body);
        assert!(facts.is_index_plus_one(&body, 10, 9));
    }

    #[test]
    fn rejects_missing_producer_non_add_and_non_one_literals() {
        let body = body_with_add(vec![7, 1], Some(9), vec![LiteralValue::U32(2)]);
        let facts = IndexFacts::new(&body);
        assert!(!facts.is_index_plus_one(&body, 9, 7));
        assert!(!facts.is_index_plus_one(&body, 99, 7));
    }

    #[test]
    fn literal_pool_indices_do_not_count_as_result_consumers() {
        let body = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            child_bodies: Vec::new(),
            literals: vec![LiteralValue::U32(9)],
        };
        let facts = IndexFacts::new(&body);
        assert_eq!(facts.result_use_count(0), 0);
        assert_eq!(facts.single_consumer_idx(0), None);
    }

    #[test]
    fn store_binding_slots_do_not_count_as_result_consumers() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 1, 2],
                    result: None,
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
            ],
        };
        let facts = IndexFacts::new(&body);
        assert_eq!(facts.result_use_count(0), 0);
        assert_eq!(facts.result_use_count(1), 1);
        assert_eq!(facts.result_use_count(2), 1);
    }

    #[test]
    fn matrix_mma_consecutive_fragment_results_are_producers() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::MatrixMma {
                        shape: MatrixMmaShape::M16N8K16,
                        a_layout: MatrixMmaLayout::RowMajor,
                        b_layout: MatrixMmaLayout::ColMajor,
                        a_type: MatrixMmaElement::F16,
                        b_type: MatrixMmaElement::F16,
                        accum_type: MatrixMmaElement::F32,
                    },
                    operands: vec![0; 10],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::Copy,
                    operands: vec![12],
                    result: Some(20),
                },
            ],
            child_bodies: Vec::new(),
            literals: Vec::new(),
        };
        let facts = IndexFacts::new(&body);
        assert_eq!(facts.producer_idx(12), Some(0));
        assert_eq!(facts.result_use_count(12), 1);
        assert_eq!(facts.single_consumer_idx(12), Some(1));
    }

    #[test]
    fn index_multiple_detects_dynamic_constant_stride_alignment() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![99, 1],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![99, 2],
                    result: Some(11),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![10, 3],
                    result: Some(12),
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![
                LiteralValue::U32(4),
                LiteralValue::U32(10),
                LiteralValue::U32(1),
            ],
        };
        let facts = IndexFacts::new(&body);
        assert!(facts.index_is_multiple_of(&body, 10, 4));
        assert!(facts.index_is_multiple_of(&body, 11, 2));
        assert!(!facts.index_is_multiple_of(&body, 11, 4));
        assert!(!facts.index_is_multiple_of(&body, 12, 2));
    }

    #[test]
    fn index_multiple_detects_strength_reduced_shift_alignment() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Shl),
                    operands: vec![99, 1],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Shl),
                    operands: vec![99, 2],
                    result: Some(11),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![10, 3],
                    result: Some(12),
                },
            ],
            child_bodies: Vec::new(),
            literals: vec![
                LiteralValue::U32(2),
                LiteralValue::U32(1),
                LiteralValue::U32(1),
            ],
        };
        let facts = IndexFacts::new(&body);
        assert!(facts.index_is_multiple_of(&body, 10, 4));
        assert!(facts.index_is_multiple_of(&body, 11, 2));
        assert!(!facts.index_is_multiple_of(&body, 11, 4));
        assert!(!facts.index_is_multiple_of(&body, 12, 2));
    }
}

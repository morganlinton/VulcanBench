//! B1 substrate: adjacent-load packing analysis.
//!
//! Detects chains of `LoadGlobal` ops on the same binding slot whose
//! normalized indices share one base and have consecutive offsets
//! (`base+i`, `base+i+1`, `base+i+2`, `base+i+3`). Such chains are
//! candidates for vec2/vec4 packed loads  -  one wide
//! transaction instead of N narrow ones, saving (N-1) memory
//! request slots and improving coalescing.
//!
//! Pure analysis on a [`KernelDescriptor`]. The actual rewrite
//! (collapse N adjacent Loads into one wide Load + N AccessIndex
//! projections) is downstream work in `vyre-lower::rewrites`. This
//! substrate just produces the per-body chain inventory.

use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue};
use rustc_hash::FxHashMap;
use vyre_foundation::ir::BinOp;

/// One detected adjacent-load chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VecPackChain {
    /// The binding slot all loads in the chain target.
    pub slot: u32,
    /// Op indices in the body, in chain order. Length is the
    /// chain length (always >= 2  -  single loads are not a chain).
    pub op_indices: Vec<usize>,
    /// Starting literal index or constant offset. Subsequent loads target
    /// `start_index + 1`, `+ 2`, ...
    pub start_index: u32,
}

impl VecPackChain {
    /// The width of the packed load this chain enables (2, 3, or
    /// 4 depending on chain length, capped at 4).
    #[must_use]
    pub fn pack_width(&self) -> u32 {
        let len = self.op_indices.len() as u32;
        len.min(4)
    }
}

/// Per-body inventory of vec-pack chains.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VecPackReport {
    /// All chains in the body, sorted by `(slot, start_index)`.
    pub chains: Vec<VecPackChain>,
    /// Total ops eliminated if every chain were packed at its
    /// maximum width.
    pub total_ops_eliminated: u32,
}

impl VecPackReport {
    /// True iff at least one chain was detected.
    #[must_use]
    pub fn has_chains(&self) -> bool {
        !self.chains.is_empty()
    }
}

/// Analyse `desc.body` and return the vec-pack chain inventory.
///
/// O(ops + candidates log candidates) per body. Pure: no allocation outside
/// analysis-local tables and the returned report.
#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> VecPackReport {
    let mut report = VecPackReport::default();
    walk_body(&desc.body, &mut report);
    report.chains.sort_by_key(|c| (c.slot, c.start_index));
    report.total_ops_eliminated = report
        .chains
        .iter()
        .map(|c| (c.op_indices.len() as u32).saturating_sub(1))
        .sum();
    report
}

fn walk_body(body: &KernelBody, report: &mut VecPackReport) {
    detect_chains_in_body(body, report);
    for child in &body.child_bodies {
        walk_body(child, report);
    }
}

fn detect_chains_in_body(body: &KernelBody, report: &mut VecPackReport) {
    let indices = index_expr_by_result(body);
    let mut by_slot_and_base: FxHashMap<(u32, Option<u32>), Vec<(u32, usize)>> =
        FxHashMap::with_capacity_and_hasher(body.ops.len(), Default::default());

    for (op_idx, op) in body.ops.iter().enumerate() {
        let Some((slot, index)) = load_with_index_expr(op, &indices) else {
            continue;
        };
        by_slot_and_base
            .entry((slot, index.base_result))
            .or_default()
            .push((index.offset, op_idx));
    }

    for ((slot, _base_result), mut candidates) in by_slot_and_base {
        candidates.sort_unstable_by_key(|(offset, op_idx)| (*offset, *op_idx));
        let mut run_start = 0usize;
        while run_start < candidates.len() {
            let mut run_end = run_start + 1;
            while run_end < candidates.len()
                && candidates[run_end].0 == candidates[run_end - 1].0.saturating_add(1)
            {
                run_end += 1;
            }

            if run_end - run_start >= 2 {
                report.chains.push(VecPackChain {
                    slot,
                    op_indices: candidates[run_start..run_end]
                        .iter()
                        .map(|(_, op_idx)| *op_idx)
                        .collect(),
                    start_index: candidates[run_start].0,
                });
            }
            run_start = run_end;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexExpr {
    base_result: Option<u32>,
    offset: u32,
}

fn index_expr_by_result(body: &KernelBody) -> FxHashMap<u32, IndexExpr> {
    let mut out = FxHashMap::with_capacity_and_hasher(body.ops.len(), Default::default());
    for op in &body.ops {
        let Some(result) = op.result else {
            continue;
        };
        if let Some(expr) = literal_index_expr(op, body).or_else(|| add_index_expr(op, &out)) {
            out.insert(result, expr);
        } else {
            out.insert(
                result,
                IndexExpr {
                    base_result: Some(result),
                    offset: 0,
                },
            );
        }
    }
    out
}

fn literal_index_expr(op: &KernelOp, body: &KernelBody) -> Option<IndexExpr> {
    if !matches!(op.kind, KernelOpKind::Literal) {
        return None;
    }
    let pool_idx = *op.operands.first()?;
    let LiteralValue::U32(value) = body.literals.get(pool_idx as usize)? else {
        return None;
    };
    Some(IndexExpr {
        base_result: None,
        offset: *value,
    })
}

fn add_index_expr(op: &KernelOp, indices: &FxHashMap<u32, IndexExpr>) -> Option<IndexExpr> {
    if !matches!(op.kind, KernelOpKind::BinOpKind(BinOp::Add)) {
        return None;
    }
    let lhs = indices.get(op.operands.first()?)?;
    let rhs = indices.get(op.operands.get(1)?)?;
    let base_result = match (lhs.base_result, rhs.base_result) {
        (None, None) => None,
        (Some(base), None) | (None, Some(base)) => Some(base),
        (Some(lhs_base), Some(rhs_base)) if lhs_base == rhs_base => Some(lhs_base),
        (Some(_), Some(_)) => return None,
    };
    Some(IndexExpr {
        base_result,
        offset: lhs.offset.checked_add(rhs.offset)?,
    })
}

/// Returns `Some((slot, index))` when `op` is a `LoadGlobal` whose index
/// operand resolves to a normalized expression.
fn load_with_index_expr(
    op: &KernelOp,
    indices: &FxHashMap<u32, IndexExpr>,
) -> Option<(u32, IndexExpr)> {
    if !matches!(op.kind, KernelOpKind::LoadGlobal) {
        return None;
    }
    let slot = *op.operands.first()?;
    let index_op_id = *op.operands.get(1)?;
    indices.get(&index_op_id).map(|index| (slot, *index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, MemoryClass,
    };
    use vyre_foundation::ir::DataType;

    fn input_slot(id: u32, name: &str) -> BindingSlot {
        BindingSlot {
            slot: id,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadOnly,
            name: name.into(),
        }
    }

    /// Build a body with N consecutive LoadGlobal(slot, Literal(i)) ops.
    fn linear_load_body(slot: u32, n: u32) -> KernelBody {
        let mut ops = Vec::new();
        let mut literals = Vec::new();
        for i in 0..n {
            literals.push(LiteralValue::U32(i));
            ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![i],
                result: Some(i),
            });
        }
        for i in 0..n {
            ops.push(KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![slot, i],
                result: Some(n + i),
            });
        }
        KernelBody {
            ops,
            child_bodies: vec![],
            literals,
        }
    }

    fn desc_with_body(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![input_slot(0, "in")],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
    }

    #[test]
    fn empty_body_has_no_chains() {
        let desc = desc_with_body(KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        });
        let report = analyze(&desc);
        assert!(!report.has_chains());
        assert_eq!(report.total_ops_eliminated, 0);
    }

    #[test]
    fn single_load_is_not_a_chain() {
        let desc = desc_with_body(linear_load_body(0, 1));
        assert!(!analyze(&desc).has_chains());
    }

    #[test]
    fn two_adjacent_loads_form_a_chain() {
        let desc = desc_with_body(linear_load_body(0, 2));
        let report = analyze(&desc);
        assert_eq!(report.chains.len(), 1);
        assert_eq!(report.chains[0].op_indices.len(), 2);
        assert_eq!(report.chains[0].slot, 0);
        assert_eq!(report.chains[0].start_index, 0);
        assert_eq!(report.chains[0].pack_width(), 2);
        assert_eq!(report.total_ops_eliminated, 1);
    }

    #[test]
    fn four_adjacent_loads_form_one_chain_at_pack_width_4() {
        let desc = desc_with_body(linear_load_body(0, 4));
        let report = analyze(&desc);
        assert_eq!(report.chains.len(), 1);
        assert_eq!(report.chains[0].pack_width(), 4);
        assert_eq!(report.total_ops_eliminated, 3);
    }

    #[test]
    fn five_adjacent_loads_pack_width_capped_at_4() {
        // 5 consecutive loads still form one chain of length 5;
        // pack_width caps at 4 (vec4 is the widest WGSL/PTX
        // primitive load). Total ops eliminated = 4 (5 → 1 wide
        // load gives 4 saved transactions).
        let desc = desc_with_body(linear_load_body(0, 5));
        let report = analyze(&desc);
        assert_eq!(report.chains.len(), 1);
        assert_eq!(report.chains[0].op_indices.len(), 5);
        assert_eq!(report.chains[0].pack_width(), 4);
        assert_eq!(report.total_ops_eliminated, 4);
    }

    #[test]
    fn loads_on_different_slots_form_separate_chains() {
        let mut body = linear_load_body(0, 3);
        // Append two more loads on slot 1 with consecutive indices.
        body.literals.push(LiteralValue::U32(0));
        body.literals.push(LiteralValue::U32(1));
        let lit_a = body.literals.len() as u32 - 2;
        let lit_b = body.literals.len() as u32 - 1;
        let result_a = body.ops.len() as u32 + 100;
        let result_b = result_a + 1;
        body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![lit_a],
            result: Some(result_a),
        });
        body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![lit_b],
            result: Some(result_b),
        });
        body.ops.push(KernelOp {
            kind: KernelOpKind::LoadGlobal,
            operands: vec![1, result_a],
            result: Some(200),
        });
        body.ops.push(KernelOp {
            kind: KernelOpKind::LoadGlobal,
            operands: vec![1, result_b],
            result: Some(201),
        });
        let mut desc = desc_with_body(body);
        desc.bindings.slots.push(input_slot(1, "in2"));
        let report = analyze(&desc);
        // Chain on slot 0 (length 3) + chain on slot 1 (length 2).
        assert_eq!(report.chains.len(), 2);
        assert_eq!(report.chains[0].slot, 0);
        assert_eq!(report.chains[0].op_indices.len(), 3);
        assert_eq!(report.chains[1].slot, 1);
        assert_eq!(report.chains[1].op_indices.len(), 2);
    }

    #[test]
    fn non_consecutive_indices_break_the_chain() {
        // Loads at indices 0, 1, 3 (skip 2) → only 0,1 chain;
        // index 3 is a singleton.
        let mut body = KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(3),
            ],
        };
        for (i, _) in [0, 1, 3].iter().enumerate() {
            body.ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![i as u32],
                result: Some(i as u32),
            });
        }
        for (offset, lit_id) in [0, 1, 2].iter().enumerate() {
            body.ops.push(KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, *lit_id as u32],
                result: Some(10 + offset as u32),
            });
        }
        let report = analyze(&desc_with_body(body));
        // Only 0, 1 form a chain (length 2). Index 3 is a singleton.
        assert_eq!(report.chains.len(), 1);
        assert_eq!(report.chains[0].op_indices.len(), 2);
        assert_eq!(report.total_ops_eliminated, 1);
    }

    #[test]
    fn dynamic_base_plus_adjacent_offsets_forms_chain() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![3],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![4],
                    result: Some(6),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 3],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 4],
                    result: Some(8),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 5],
                    result: Some(9),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![2, 6],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 7],
                    result: Some(11),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 8],
                    result: Some(12),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 9],
                    result: Some(13),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 10],
                    result: Some(14),
                },
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(4),
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(3),
            ],
        };
        let report = analyze(&desc_with_body(body));
        assert_eq!(report.chains.len(), 1);
        assert_eq!(report.chains[0].op_indices, vec![11, 12, 13, 14]);
        assert_eq!(report.chains[0].start_index, 0);
        assert_eq!(report.chains[0].pack_width(), 4);
        assert_eq!(report.total_ops_eliminated, 3);
    }

    #[test]
    fn adjacent_offsets_from_different_dynamic_bases_do_not_chain() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
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
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 2],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 1],
                    result: Some(5),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![4, 2],
                    result: Some(6),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 5],
                    result: Some(7),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 6],
                    result: Some(8),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        };
        let report = analyze(&desc_with_body(body));
        assert!(!report.has_chains());
        assert_eq!(report.total_ops_eliminated, 0);
    }

    #[test]
    fn release_a16_fixture_cases_trigger_vec_pack_analysis() {
        let cases = crate::optimization_corpus::generate_release_corpus();
        let a16_cases = cases
            .iter()
            .filter(|case| case.family == "A16-vec-pack-fixture")
            .collect::<Vec<_>>();
        assert_eq!(a16_cases.len(), 256);

        let mut total_chains = 0usize;
        let mut total_ops_eliminated = 0u32;
        for case in a16_cases {
            let report = analyze(&case.descriptor);
            assert!(
                report.has_chains(),
                "case `{}` produced no vec-pack chain",
                case.id
            );
            total_chains += report.chains.len();
            total_ops_eliminated = total_ops_eliminated.saturating_add(report.total_ops_eliminated);
        }
        assert_eq!(total_chains, 256);
        assert_eq!(total_ops_eliminated, 768);
    }

    #[test]
    fn singleton_computed_index_is_not_chainable() {
        let body = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 2],
                    result: Some(3),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        };
        let report = analyze(&desc_with_body(body));
        assert!(!report.has_chains());
    }

    #[test]
    fn chains_in_child_bodies_are_detected_too() {
        let child = linear_load_body(0, 3);
        let parent = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::StructuredBlock,
                operands: vec![0],
                result: None,
            }],
            child_bodies: vec![child],
            literals: vec![],
        };
        let report = analyze(&desc_with_body(parent));
        assert_eq!(report.chains.len(), 1);
        assert_eq!(report.chains[0].op_indices.len(), 3);
    }
}

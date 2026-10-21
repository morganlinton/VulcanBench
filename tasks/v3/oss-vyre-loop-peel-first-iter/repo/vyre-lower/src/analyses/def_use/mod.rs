//! Def-use chains over a `KernelDescriptor`.
//!
//! For every result-id produced by an op anywhere in the descriptor,
//! collects the list of `UseSite`s (body_path, op_index, operand_pos)
//! that reference that id. Recurses into child bodies; chains are
//! per-body (since each body has its own id space).
//!
//! ## Use cases
//!
//! - Building DCE more efficient than its current "scan everything"
//!   pass: drop ops whose def_use chain is empty.
//! - Replacing-uses-with: substitute references to id A with id B
//!   without re-walking the whole body.
//! - Live-range analysis (combine with op order to derive intervals).
//! - Soundness checks: assert no use precedes its def.

use rustc_hash::FxHashMap;

use serde::{Deserialize, Serialize};

use crate::operand_semantics::operand_is_result_reference;
use crate::{KernelBody, KernelDescriptor};

/// Where in the descriptor a result-id is referenced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UseSite {
    /// Path of `child_bodies` indices to reach the body containing the
    /// use. Empty for the top-level body.
    pub body_path: Vec<usize>,
    /// Index of the using op within its body.
    pub op_index: usize,
    /// Operand position within the using op.
    pub operand_pos: usize,
}

/// Per-body chains. The map's key is a result-id; the value is every
/// site within THIS body where that id is used. (Cross-body uses are
/// not modeled  -  bodies have isolated id spaces in vyre's structured
/// IR.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PerBodyChains {
    pub uses: FxHashMap<u32, Vec<UseSite>>,
    /// Path to this body (empty = top-level).
    pub body_path: Vec<usize>,
    /// Total number of result-ids produced by this body's ops.
    pub def_count: usize,
    /// Total number of operand references (across all ops, all positions
    /// classified as ResultRef). Useful for sanity checking & stats.
    pub use_count: usize,
}

/// Top-level report: one `PerBodyChains` per body in the descriptor,
/// produced in pre-order (top body first, then each subtree).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DefUseReport {
    pub bodies: Vec<PerBodyChains>,
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> DefUseReport {
    let mut report = DefUseReport::default();
    walk(&desc.body, &mut Vec::new(), &mut report);
    report
}

/// Convenience: collect every result-id with NO uses across the whole
/// descriptor. These are dead-by-no-use candidates.
#[must_use]
pub fn dead_by_no_use(desc: &KernelDescriptor) -> Vec<(Vec<usize>, u32)> {
    let report = analyze(desc);
    let mut out = Vec::new();
    for body_chains in &report.bodies {
        for (id, uses) in &body_chains.uses {
            if uses.is_empty() {
                out.push((body_chains.body_path.clone(), *id));
            }
        }
    }
    out
}

fn walk(body: &KernelBody, path: &mut Vec<usize>, report: &mut DefUseReport) {
    let mut chains = PerBodyChains {
        uses: FxHashMap::default(),
        body_path: path.clone(),
        def_count: 0,
        use_count: 0,
    };

    // Seed the map with every produced id (so dead defs appear with
    // empty Vec<UseSite>).
    for op in &body.ops {
        if let Some(rid) = op.result {
            chains.uses.entry(rid).or_default();
            chains.def_count += 1;
        }
    }

    // Now walk again to collect uses.
    for (op_idx, op) in body.ops.iter().enumerate() {
        for (pos, &val) in op.operands.iter().enumerate() {
            if operand_is_result_reference(&op.kind, pos) {
                chains.uses.entry(val).or_default().push(UseSite {
                    body_path: path.clone(),
                    op_index: op_idx,
                    operand_pos: pos,
                });
                chains.use_count += 1;
            }
        }
    }

    report.bodies.push(chains);

    for (idx, child) in body.child_bodies.iter().enumerate() {
        path.push(idx);
        walk(child, path, report);
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };
    use vyre_foundation::ir::BinOp;

    fn empty_desc(ops: Vec<KernelOp>, literals: Vec<LiteralValue>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals,
            },
        }
    }

    #[test]
    fn empty_kernel_yields_one_body_no_chains() {
        let r = analyze(&empty_desc(vec![], vec![]));
        assert_eq!(r.bodies.len(), 1);
        assert!(r.bodies[0].uses.is_empty());
        assert_eq!(r.bodies[0].def_count, 0);
        assert_eq!(r.bodies[0].use_count, 0);
    }

    #[test]
    fn linear_chain_traces_correctly() {
        // r0 = Lit, r1 = Lit, r2 = Add(r0, r1), r3 = Mul(r2, r0)
        let desc = empty_desc(
            vec![
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
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![2, 0],
                    result: Some(3),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        );
        let r = analyze(&desc);
        let chains = &r.bodies[0];
        assert_eq!(chains.def_count, 4);
        // r0 used by Add (op 2 pos 0) and Mul (op 3 pos 1).
        assert_eq!(chains.uses[&0].len(), 2);
        // r1 used by Add (op 2 pos 1).
        assert_eq!(chains.uses[&1].len(), 1);
        // r2 used by Mul (op 3 pos 0).
        assert_eq!(chains.uses[&2].len(), 1);
        // r3 used by no one (dead).
        assert_eq!(chains.uses[&3].len(), 0);
    }

    #[test]
    fn dead_def_visible_with_empty_chain() {
        // r0 = Lit, r1 = Lit (unused).
        let desc = empty_desc(
            vec![
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
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        );
        let r = analyze(&desc);
        assert_eq!(r.bodies[0].uses[&0].len(), 0);
        assert_eq!(r.bodies[0].uses[&1].len(), 0);

        let dead = dead_by_no_use(&desc);
        // Both are dead.
        assert_eq!(dead.len(), 2);
    }

    #[test]
    fn store_operands_classified_correctly() {
        // r0 = Lit, r1 = Lit, Store(0, 0, 1)
        // Store operand 0 = slot (not ref), operand 1 = idx (ref → r0),
        // operand 2 = val (ref → r1)
        let desc = empty_desc(
            vec![
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
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 0, 1],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(7)],
        );
        let r = analyze(&desc);
        // r0 used at op 2 pos 1.
        assert_eq!(r.bodies[0].uses[&0].len(), 1);
        assert_eq!(r.bodies[0].uses[&0][0].operand_pos, 1);
        // r1 used at op 2 pos 2.
        assert_eq!(r.bodies[0].uses[&1].len(), 1);
        assert_eq!(r.bodies[0].uses[&1][0].operand_pos, 2);
    }

    #[test]
    fn child_bodies_get_separate_chains() {
        // Parent body has just an If pointing at a child body that
        // does its own arithmetic.
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
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
                            kind: KernelOpKind::BinOpKind(BinOp::Add),
                            operands: vec![0, 1],
                            result: Some(2),
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
                }],
                literals: vec![LiteralValue::U32(1)],
            },
        };
        let r = analyze(&desc);
        assert_eq!(r.bodies.len(), 2);
        assert_eq!(r.bodies[0].body_path, Vec::<usize>::new());
        assert_eq!(r.bodies[1].body_path, vec![0]);
        // Parent body: r0 used by If (op 1 pos 0).
        assert_eq!(r.bodies[0].uses[&0].len(), 1);
        // Child body: r0 used by Add (op 2 pos 0); r1 by Add (op 2 pos 1); r2 dead.
        assert_eq!(r.bodies[1].uses[&0].len(), 1);
        assert_eq!(r.bodies[1].uses[&1].len(), 1);
        assert_eq!(r.bodies[1].uses[&2].len(), 0);
    }

    #[test]
    fn use_count_matches_sum_of_chain_lengths() {
        let desc = empty_desc(
            vec![
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
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![2, 0],
                    result: Some(3),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        );
        let r = analyze(&desc);
        let chains = &r.bodies[0];
        let total_uses: usize = chains.uses.values().map(|v| v.len()).sum();
        assert_eq!(total_uses, chains.use_count);
    }

    #[test]
    fn select_op_three_refs() {
        // Select takes 3 result-id operands.
        let desc = empty_desc(
            vec![
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
                    kind: KernelOpKind::Select,
                    operands: vec![0, 1, 2],
                    result: Some(3),
                },
            ],
            vec![
                LiteralValue::Bool(true),
                LiteralValue::U32(7),
                LiteralValue::U32(8),
            ],
        );
        let r = analyze(&desc);
        let chains = &r.bodies[0];
        assert_eq!(chains.uses[&0].len(), 1);
        assert_eq!(chains.uses[&1].len(), 1);
        assert_eq!(chains.uses[&2].len(), 1);
    }

    #[test]
    fn dce_replacement_could_use_chains() {
        // Demonstrates the analysis is enough to drive DCE: any def
        // with empty uses AND op produces no side effect IS dead.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // dead lit
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // used
                KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![0, 1, 1],
                    result: None,
                },
            ],
            vec![LiteralValue::U32(99), LiteralValue::U32(0)],
        );
        let dead = dead_by_no_use(&desc);
        // r0 is dead (the unused literal); r1 is used by the Store.
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].1, 0);
    }
}

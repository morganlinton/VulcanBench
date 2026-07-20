//! Descriptor invariant verifier.
//!
//! Walks a `KernelDescriptor` checking structural invariants that
//! every well-formed descriptor must satisfy:
//!
//! 1. Within each `KernelBody`, `op.result` ids are unique.
//! 2. Operand positions classified as result-id references must
//!    point at a result-id produced in the same body or lexically
//!    captured from a parent structured-control body.
//! 3. Operand positions classified as literal-pool indices must be in
//!    range of the body's `literals` vector.
//! 4. Operand positions classified as child-body indices must be in
//!    range of the body's `child_bodies` vector.
//! 5. Every `KernelOpKind::Literal` op must have at least one operand
//!    (the pool index).
//!
//! Bodies recurse with lexical scope and loop-carried visibility.
//! `vyre-lower` allocates result ids globally for the descriptor:
//! structured child bodies may reference values available before the
//! child was attached, and parent bodies may reference values assigned
//! by a completed child body.
//!
//! ## Wiring
//!
//! Useful as a debug-time check after every rewrite pass. Not yet
//! wired into `run_all` because the established invariant is "every
//! rewrite preserves verify()"  -  wire when the user asks. Direct
//! callers (rewrite tests, fuzz harnesses) can call `verify()` to
//! turn quiet bugs into loud ones.

use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

use crate::{KernelBody, KernelDescriptor, KernelOpKind};

/// Result type  -  `Ok(())` if every invariant holds; `Err(Vec)` lists
/// every violation found, not just the first.
pub type VerifyResult = Result<(), Vec<VerifyError>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyError {
    pub body_path: Vec<usize>,
    pub op_index: usize,
    pub kind: VerifyErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifyErrorKind {
    DuplicateResultId(u32),
    DanglingResultRef {
        operand_pos: usize,
        ref_id: u32,
    },
    LiteralPoolOutOfRange {
        operand_pos: usize,
        pool_idx: u32,
        pool_size: usize,
    },
    ChildBodyIndexOutOfRange {
        operand_pos: usize,
        body_idx: u32,
        child_count: usize,
    },
    LiteralOpMissingPoolOperand,
    OperandCountTooShort {
        expected_min: usize,
        got: usize,
    },
    /// `dispatch.workgroup_size[axis]` is zero. A kernel with a zero
    /// dim never runs  -  almost certainly a host-side bug.
    DispatchZeroDim {
        axis: u8,
    },
    /// Two `BindingSlot` entries share the same `.slot` field. The
    /// emitters look up bindings by `.slot`; duplicates make the
    /// lookup ambiguous.
    DuplicateBindingSlotId {
        slot: u32,
    },
    /// A host-bound binding (`Global` / `Constant` / `Uniform`) sits
    /// in the workgroup-reserved slot range (`>= 1<<24`). Backend
    /// bind-group layouts cap at 1000 bindings on wgpu and similar
    /// limits elsewhere; a host slot in the reserved range fails
    /// layout creation with a "binding index N greater than maximum"
    /// validator error. Earlier rewrites should have allocated the
    /// new slot in the host range.
    HostBindingInWorkgroupRange {
        slot: u32,
    },
    /// A workgroup binding (`Shared` / `Scratch`) sits in the
    /// host-bindable slot range (`< 1<<24`). The host dispatch path
    /// addresses host bindings by slot id; a workgroup binding in
    /// that range can collide with a Global binding's slot id and
    /// silently steer load/store ops to the wrong memory class.
    WorkgroupBindingInHostRange {
        slot: u32,
    },
}

#[must_use]
pub fn verify(desc: &KernelDescriptor) -> VerifyResult {
    use rustc_hash::FxHashSet;
    let mut errors = Vec::new();
    // Dispatch-level checks (don't have a body_path).
    for (axis, &dim) in desc.dispatch.workgroup_size.iter().enumerate() {
        if dim == 0 {
            errors.push(VerifyError {
                body_path: vec![],
                op_index: 0,
                kind: VerifyErrorKind::DispatchZeroDim { axis: axis as u8 },
            });
        }
    }
    // Binding-layout checks: no two slots share `.slot` field; host vs
    // workgroup ranges stay segregated.
    use crate::descriptor::MemoryClass;
    let mut seen_slots: FxHashSet<u32> = FxHashSet::default();
    for s in &desc.bindings.slots {
        if !seen_slots.insert(s.slot) {
            errors.push(VerifyError {
                body_path: vec![],
                op_index: 0,
                kind: VerifyErrorKind::DuplicateBindingSlotId { slot: s.slot },
            });
        }
        let in_workgroup_range = s.slot >= crate::lower::WORKGROUP_SLOT_BASE;
        let is_workgroup_class =
            matches!(s.memory_class, MemoryClass::Shared | MemoryClass::Scratch,);
        if in_workgroup_range && !is_workgroup_class {
            errors.push(VerifyError {
                body_path: vec![],
                op_index: 0,
                kind: VerifyErrorKind::HostBindingInWorkgroupRange { slot: s.slot },
            });
        }
        if !in_workgroup_range && is_workgroup_class {
            errors.push(VerifyError {
                body_path: vec![],
                op_index: 0,
                kind: VerifyErrorKind::WorkgroupBindingInHostRange { slot: s.slot },
            });
        }
    }
    verify_body(
        &desc.body,
        &mut Vec::new(),
        &FxHashSet::default(),
        &mut errors,
    );
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn verify_body(
    body: &KernelBody,
    path: &mut Vec<usize>,
    inherited_results: &FxHashSet<u32>,
    errors: &mut Vec<VerifyError>,
) {
    use rustc_hash::FxHashSet;

    // 1. Collect produced result-ids, flagging duplicates.
    let mut produced: FxHashSet<u32> = FxHashSet::default();
    for (i, op) in body.ops.iter().enumerate() {
        for r in op.result_ids() {
            if !produced.insert(r) {
                errors.push(VerifyError {
                    body_path: path.clone(),
                    op_index: i,
                    kind: VerifyErrorKind::DuplicateResultId(r),
                });
            }
        }
    }

    // 2 & 3 & 4 & 5: per-op operand checks.
    let mut produced_so_far: FxHashSet<u32> = FxHashSet::default();
    let child_results: Vec<FxHashSet<u32>> =
        body.child_bodies.iter().map(collect_body_results).collect();
    let mut completed_child_results: FxHashSet<u32> = FxHashSet::default();
    let mut child_scopes = vec![FxHashSet::default(); body.child_bodies.len()];
    for (i, op) in body.ops.iter().enumerate() {
        // Literal ops must have at least one operand (pool index).
        if matches!(op.kind, KernelOpKind::Literal) {
            if op.operands.is_empty() {
                errors.push(VerifyError {
                    body_path: path.clone(),
                    op_index: i,
                    kind: VerifyErrorKind::LiteralOpMissingPoolOperand,
                });
            } else {
                let pool_idx = op.operands[0];
                if (pool_idx as usize) >= body.literals.len() {
                    errors.push(VerifyError {
                        body_path: path.clone(),
                        op_index: i,
                        kind: VerifyErrorKind::LiteralPoolOutOfRange {
                            operand_pos: 0,
                            pool_idx,
                            pool_size: body.literals.len(),
                        },
                    });
                }
            }
        }

        // Per-position classification.
        for (pos, &val) in op.operands.iter().enumerate() {
            let cls = classify_operand(&op.kind, pos);
            match cls {
                OperandClass::ResultRef => {
                    if !produced_so_far.contains(&val)
                        && !produced.contains(&val)
                        && !inherited_results.contains(&val)
                        && !completed_child_results.contains(&val)
                    {
                        errors.push(VerifyError {
                            body_path: path.clone(),
                            op_index: i,
                            kind: VerifyErrorKind::DanglingResultRef {
                                operand_pos: pos,
                                ref_id: val,
                            },
                        });
                    }
                }
                OperandClass::ChildBodyIdx => {
                    if (val as usize) >= body.child_bodies.len() {
                        errors.push(VerifyError {
                            body_path: path.clone(),
                            op_index: i,
                            kind: VerifyErrorKind::ChildBodyIndexOutOfRange {
                                operand_pos: pos,
                                body_idx: val,
                                child_count: body.child_bodies.len(),
                            },
                        });
                    } else {
                        let child_scope = &mut child_scopes[val as usize];
                        child_scope.extend(inherited_results.iter().copied());
                        child_scope.extend(produced_so_far.iter().copied());
                        child_scope.extend(completed_child_results.iter().copied());
                    }
                }
                OperandClass::LiteralPoolIdx => {
                    if (val as usize) >= body.literals.len() {
                        errors.push(VerifyError {
                            body_path: path.clone(),
                            op_index: i,
                            kind: VerifyErrorKind::LiteralPoolOutOfRange {
                                operand_pos: pos,
                                pool_idx: val,
                                pool_size: body.literals.len(),
                            },
                        });
                    }
                }
                OperandClass::Other => {}
            }
        }

        // Minimum operand count per kind. Conservative  -  we just check
        // shapes the rewrites actually produce.
        let min_required = min_operand_count(&op.kind);
        if op.operands.len() < min_required {
            errors.push(VerifyError {
                body_path: path.clone(),
                op_index: i,
                kind: VerifyErrorKind::OperandCountTooShort {
                    expected_min: min_required,
                    got: op.operands.len(),
                },
            });
        }

        for r in op.result_ids() {
            produced_so_far.insert(r);
        }
        for child_idx in child_body_operands(op) {
            if let Some(results) = child_results.get(child_idx as usize) {
                completed_child_results.extend(results.iter().copied());
            }
        }
    }

    // Recurse.
    for (idx, child) in body.child_bodies.iter().enumerate() {
        path.push(idx);
        verify_body(child, path, &child_scopes[idx], errors);
        path.pop();
    }
}

fn collect_body_results(body: &KernelBody) -> FxHashSet<u32> {
    let mut results = FxHashSet::default();
    for op in &body.ops {
        for result in op.result_ids() {
            results.insert(result);
        }
    }
    for child in &body.child_bodies {
        results.extend(collect_body_results(child));
    }
    results
}

fn child_body_operands(op: &crate::KernelOp) -> impl Iterator<Item = u32> + '_ {
    op.operands
        .iter()
        .enumerate()
        .filter_map(|(pos, &operand)| {
            (classify_operand(&op.kind, pos) == OperandClass::ChildBodyIdx).then_some(operand)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandClass {
    ResultRef,
    ChildBodyIdx,
    LiteralPoolIdx,
    /// Binding-slot literal, opaque tag, etc.  -  not validated structurally.
    Other,
}

pub fn classify_operand(kind: &KernelOpKind, pos: usize) -> OperandClass {
    use KernelOpKind::*;
    match kind {
        Literal => {
            if pos == 0 {
                OperandClass::LiteralPoolIdx
            } else {
                OperandClass::Other
            }
        }
        LocalInvocationId | GlobalInvocationId | WorkgroupId => OperandClass::Other,
        SubgroupLocalId | SubgroupSize => OperandClass::Other,
        LoopIndex { .. } => OperandClass::Other,
        BufferLength => OperandClass::Other,
        LoadGlobal | LoadShared | LoadConstant => {
            if pos == 0 {
                OperandClass::Other
            } else {
                OperandClass::ResultRef
            }
        }
        StoreGlobal | StoreShared => {
            if pos == 0 {
                OperandClass::Other
            } else {
                OperandClass::ResultRef
            }
        }
        Copy | BinOpKind(_) | UnOpKind(_) | Fma | MatrixMma { .. } | Select | Cast { .. } => {
            OperandClass::ResultRef
        }
        Atomic { .. } => {
            if pos == 0 {
                OperandClass::Other
            } else {
                OperandClass::ResultRef
            }
        }
        SubgroupBallot | SubgroupShuffle | SubgroupBroadcast | SubgroupReduce { .. } => OperandClass::ResultRef,
        StructuredIfThen => {
            if pos == 0 {
                OperandClass::ResultRef
            } else if pos == 1 {
                OperandClass::ChildBodyIdx
            } else {
                OperandClass::Other
            }
        }
        StructuredIfThenElse => {
            if pos == 0 {
                OperandClass::ResultRef
            } else if pos == 1 || pos == 2 {
                OperandClass::ChildBodyIdx
            } else {
                OperandClass::Other
            }
        }
        StructuredForLoop { .. } => {
            if pos == 0 || pos == 1 {
                OperandClass::ResultRef
            } else if pos == 2 {
                OperandClass::ChildBodyIdx
            } else {
                OperandClass::Other
            }
        }
        StructuredBlock => {
            if pos == 0 {
                OperandClass::ChildBodyIdx
            } else {
                OperandClass::Other
            }
        }
        Region { .. } => {
            if pos == 0 {
                OperandClass::ChildBodyIdx
            } else {
                OperandClass::Other
            }
        }
        Return | Barrier { .. } => OperandClass::Other,
        AsyncLoad { .. } | AsyncStore { .. } => {
            if pos < 2 {
                OperandClass::Other
            } else {
                OperandClass::ResultRef
            }
        }
        AsyncWait { .. } => OperandClass::Other,
        Trap { .. } => {
            if pos == 0 {
                OperandClass::ResultRef
            } else {
                OperandClass::Other
            }
        }
        Resume { .. } => OperandClass::Other,
        IndirectDispatch { .. } => OperandClass::Other,
        Call { .. } => OperandClass::ResultRef,
        OpaqueExpr(..) | OpaqueNode(..) => OperandClass::ResultRef,
        LoopCarrierInit { .. } | LoopCarrier { .. } | LoopCarrierEnd { .. } => {
            OperandClass::ResultRef
        }
    }
}

fn min_operand_count(kind: &KernelOpKind) -> usize {
    use KernelOpKind::*;
    match kind {
        Literal => 1,
        Copy => 1,
        LocalInvocationId | GlobalInvocationId | WorkgroupId => 0,
        SubgroupLocalId | SubgroupSize => 0,
        LoopIndex { .. } => 0,
        BufferLength => 1,
        LoadGlobal | LoadShared | LoadConstant => 2,
        StoreGlobal | StoreShared => 3,
        BinOpKind(_) => 2,
        UnOpKind(_) | Cast { .. } => 1,
        Fma => 3,
        MatrixMma { .. } => 10,
        Select => 3,
        Atomic { .. } => 2,
        SubgroupBallot | SubgroupShuffle | SubgroupBroadcast | SubgroupReduce { .. } => 1,
        StructuredIfThen => 2,
        StructuredIfThenElse => 3,
        StructuredForLoop { .. } => 3,
        StructuredBlock => 1,
        Region { .. } => 1,
        Return => 0,
        Barrier { .. } => 0,
        AsyncLoad { .. } | AsyncStore { .. } => 2,
        AsyncWait { .. } => 0,
        Trap { .. } => 1,
        Resume { .. } => 0,
        IndirectDispatch { .. } => 0,
        Call { .. } => 0,
        OpaqueExpr(..) | OpaqueNode(..) => 0,
        LoopCarrier { .. } => 0,
        LoopCarrierInit { .. } | LoopCarrierEnd { .. } => 1,
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
    fn empty_kernel_verifies() {
        assert!(matches!(verify(&empty_desc(vec![], vec![])), Ok(_)));
    }

    #[test]
    fn well_formed_kernel_verifies() {
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
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        );
        assert_eq!(verify(&desc), Ok(()));
    }

    #[test]
    fn duplicate_result_id_detected() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // dup
            ],
            vec![LiteralValue::U32(1)],
        );
        let r = verify(&desc);
        assert!(r.is_err());
        let errs = r.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e.kind, VerifyErrorKind::DuplicateResultId(0))));
    }

    #[test]
    fn dangling_result_ref_detected() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 99], // 99 is not produced anywhere
                    result: Some(1),
                },
            ],
            vec![LiteralValue::U32(1)],
        );
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            e.kind,
            VerifyErrorKind::DanglingResultRef { ref_id: 99, .. }
        )));
    }

    #[test]
    fn literal_pool_out_of_range_detected() {
        let desc = empty_desc(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![5], // pool only has 1 entry
                result: Some(0),
            }],
            vec![LiteralValue::U32(1)],
        );
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            e.kind,
            VerifyErrorKind::LiteralPoolOutOfRange {
                pool_idx: 5,
                pool_size: 1,
                ..
            }
        )));
    }

    #[test]
    fn child_body_index_out_of_range_detected() {
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
                        operands: vec![0, 7], // child idx 7 with no children
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(1)],
            },
        };
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            e.kind,
            VerifyErrorKind::ChildBodyIndexOutOfRange {
                body_idx: 7,
                child_count: 0,
                ..
            }
        )));
    }

    #[test]
    fn literal_op_with_no_operands_detected() {
        let desc = empty_desc(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![],
                result: Some(0),
            }],
            vec![LiteralValue::U32(1)],
        );
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e.kind, VerifyErrorKind::LiteralOpMissingPoolOperand)));
    }

    #[test]
    fn operand_count_too_short_detected() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0], // only 1 operand, Add needs 2
                    result: Some(1),
                },
            ],
            vec![LiteralValue::U32(1)],
        );
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            e.kind,
            VerifyErrorKind::OperandCountTooShort {
                expected_min: 2,
                got: 1
            }
        )));
    }

    #[test]
    fn errors_are_collected_not_short_circuited() {
        // 3 distinct violations in one body.
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![99],
                    result: Some(0),
                }, // pool oor
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // dup
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![100, 200], // dangling refs
                    result: Some(1),
                },
            ],
            vec![LiteralValue::U32(1)],
        );
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs.len() >= 3);
    }

    #[test]
    fn child_body_violations_recurse() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![99],
                        result: Some(0),
                    }],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(1)],
                }],
                literals: vec![],
            },
        };
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs.iter().any(|e| e.body_path == vec![0]));
    }

    #[test]
    fn child_body_may_capture_parent_result_available_before_control_op() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 0],
                result: Some(1),
            }],
            child_bodies: vec![],
            literals: vec![],
        };
        let desc = KernelDescriptor {
            id: "captures".into(),
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
                        kind: KernelOpKind::StructuredBlock,
                        operands: vec![0],
                        result: None,
                    },
                ],
                child_bodies: vec![child],
                literals: vec![LiteralValue::U32(7)],
            },
        };

        assert_eq!(verify(&desc), Ok(()));
    }

    #[test]
    fn child_body_cannot_capture_parent_result_declared_after_control_op() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![1, 1],
                result: Some(2),
            }],
            child_bodies: vec![],
            literals: vec![],
        };
        let desc = KernelDescriptor {
            id: "future_capture".into(),
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
                        kind: KernelOpKind::StructuredBlock,
                        operands: vec![0],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![child],
                literals: vec![LiteralValue::U32(7), LiteralValue::U32(9)],
            },
        };

        let errors = verify(&desc).expect_err("future child capture must fail");
        assert!(errors.iter().any(|error| {
            error.body_path == vec![0]
                && matches!(
                    error.kind,
                    VerifyErrorKind::DanglingResultRef { ref_id: 1, .. }
                )
        }));
    }

    #[test]
    fn parent_body_may_read_result_assigned_by_completed_child_body() {
        let child = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 0],
                result: Some(1),
            }],
            child_bodies: vec![],
            literals: vec![],
        };
        let desc = KernelDescriptor {
            id: "loop_carried".into(),
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
                        kind: KernelOpKind::StructuredBlock,
                        operands: vec![0],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Mul),
                        operands: vec![1, 0],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![child],
                literals: vec![LiteralValue::U32(7)],
            },
        };

        assert_eq!(verify(&desc), Ok(()));
    }

    #[test]
    fn run_all_output_verifies() {
        // Full pipeline output must satisfy verify(). This is the
        // critical regression gate  -  any rewrite that produces an
        // invalid descriptor will fail this test.
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
                    operands: vec![1, 0],
                    result: Some(3),
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(99)],
        );
        let optimized = crate::rewrites::run_all(&desc);
        assert_eq!(verify(&optimized), Ok(()));
    }

    #[test]
    fn dispatch_zero_x_dim_detected() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(0, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e.kind, VerifyErrorKind::DispatchZeroDim { axis: 0 })));
    }

    #[test]
    fn dispatch_zero_z_dim_detected() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 0),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e.kind, VerifyErrorKind::DispatchZeroDim { axis: 2 })));
    }

    #[test]
    fn duplicate_binding_slot_detected() {
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let dup = BindingSlot {
            slot: 5,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "a".into(),
        };
        let mut second = dup.clone();
        second.name = "b".into();
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![dup, second],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = verify(&desc);
        let errs = r.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e.kind, VerifyErrorKind::DuplicateBindingSlotId { slot: 5 })));
    }

    #[test]
    fn dispatch_normal_no_error() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        assert_eq!(verify(&desc), Ok(()));
    }

    #[test]
    fn host_binding_in_workgroup_range_is_rejected() {
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let bad = BindingSlot {
            slot: crate::lower::WORKGROUP_SLOT_BASE + 7,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "host_in_high_range".into(),
        };
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![bad] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let errs = verify(&desc).unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e.kind, VerifyErrorKind::HostBindingInWorkgroupRange { .. })));
    }

    #[test]
    fn workgroup_binding_in_host_range_is_rejected() {
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let bad = BindingSlot {
            slot: 5,
            element_type: DataType::U32,
            element_count: Some(64),
            memory_class: MemoryClass::Shared,
            visibility: BindingVisibility::ReadWrite,
            name: "shared_in_low_range".into(),
        };
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![bad] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let errs = verify(&desc).unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e.kind, VerifyErrorKind::WorkgroupBindingInHostRange { .. })));
    }
}

//! Implementation: trace branch conditions back through the op stream
//! looking for any thread-id dependency.

use super::report::{BranchSite, BranchUniformity, WorkgroupUniformReport};
use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind};
use rustc_hash::{FxHashMap, FxHashSet};

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> WorkgroupUniformReport {
    let mut branches = Vec::new();
    walk_body(&desc.body, &mut branches, 0);
    WorkgroupUniformReport {
        kernel_id: desc.id.clone(),
        branches,
    }
}

fn walk_body(body: &KernelBody, branches: &mut Vec<BranchSite>, op_index_offset: usize) {
    let producers = producer_map(body);
    for (local_idx, op) in body.ops.iter().enumerate() {
        let op_index = op_index_offset + local_idx;
        match &op.kind {
            KernelOpKind::StructuredIfThen | KernelOpKind::StructuredIfThenElse => {
                if let Some(cond_id) = op.operands.first() {
                    let uniformity = classify(&producers, *cond_id);
                    branches.push(BranchSite {
                        op_index,
                        cond_operand_id: *cond_id,
                        uniformity,
                    });
                }
                // Recurse into child bodies.
                for child_id in op.operands.iter().skip(1) {
                    if let Some(child) = body.child_bodies.get(*child_id as usize) {
                        walk_body(child, branches, op_index_offset + body.ops.len());
                    }
                }
            }
            KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::Region { .. } => {
                if let Some(child_id) = op.operands.last() {
                    if let Some(child) = body.child_bodies.get(*child_id as usize) {
                        walk_body(child, branches, op_index_offset + body.ops.len());
                    }
                }
            }
            _ => {}
        }
    }
}

type ProducerMap<'a> = FxHashMap<u32, &'a KernelOp>;

fn producer_map(body: &KernelBody) -> ProducerMap<'_> {
    let mut producers = FxHashMap::with_capacity_and_hasher(body.ops.len(), Default::default());
    for op in &body.ops {
        for result in op.result_ids() {
            producers.insert(result, op);
        }
    }
    producers
}

/// Classify a condition operand by tracing its dependency closure.
fn classify(producers: &ProducerMap<'_>, cond_operand_id: u32) -> BranchUniformity {
    let mut visited = FxHashSet::default();
    let info = visit(producers, cond_operand_id, &mut visited);
    if info.contains_thread_id() {
        BranchUniformity::Divergent
    } else if info.has_unknown || visited.is_empty() {
        BranchUniformity::Unknown
    } else {
        BranchUniformity::Uniform
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct DepInfo {
    has_thread_id: bool,
    has_unknown: bool,
}

impl DepInfo {
    fn contains_thread_id(self) -> bool {
        self.has_thread_id
    }
}

fn visit(producers: &ProducerMap<'_>, operand_id: u32, visited: &mut FxHashSet<u32>) -> DepInfo {
    if !visited.insert(operand_id) {
        return DepInfo::default();
    }
    let producer = match producers.get(&operand_id).copied() {
        Some(p) => p,
        None => {
            return DepInfo {
                has_thread_id: false,
                has_unknown: true,
            }
        }
    };
    let mut info = DepInfo::default();
    match &producer.kind {
        KernelOpKind::LocalInvocationId
        | KernelOpKind::GlobalInvocationId
        | KernelOpKind::SubgroupLocalId => {
            info.has_thread_id = true;
        }
        KernelOpKind::Literal
        | KernelOpKind::WorkgroupId
        | KernelOpKind::SubgroupSize
        | KernelOpKind::BufferLength => {
            // Workgroup-scope constants  -  uniform across all threads in
            // the workgroup. No thread-id dependency.
        }
        KernelOpKind::LoadGlobal | KernelOpKind::LoadShared | KernelOpKind::LoadConstant => {
            // Loads MAY be uniform if every thread reads the same address.
            // Phase 1 conservatively treats loads as unknown unless all
            // index operands trace to non-thread-id producers.
            for op_id in producer.operands.iter().skip(1) {
                let sub = visit(producers, *op_id, visited);
                info.has_thread_id |= sub.has_thread_id;
                info.has_unknown |= sub.has_unknown;
            }
            // Even with non-thread-id index, the loaded VALUE could vary  -
            // loads aren't compile-time uniform. Mark unknown.
            info.has_unknown = true;
        }
        KernelOpKind::Atomic { .. } => {
            // Atomic results vary across threads → divergent.
            info.has_thread_id = true;
        }
        _ => {
            // Pure compute ops: recurse into operands.
            for op_id in &producer.operands {
                let sub = visit(producers, *op_id, visited);
                info.has_thread_id |= sub.has_thread_id;
                info.has_unknown |= sub.has_unknown;
            }
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };
    use vyre_foundation::ir::BinOp;

    fn empty_kernel() -> KernelDescriptor {
        KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        }
    }

    #[test]
    fn empty_kernel_has_no_branches() {
        let r = analyze(&empty_kernel());
        assert!(r.branches.is_empty());
    }

    #[test]
    fn if_with_constant_condition_is_uniform() {
        let kernel = KernelDescriptor {
            id: "uniform".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
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
                    ops: vec![],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let r = analyze(&kernel);
        assert_eq!(r.branches.len(), 1);
        assert_eq!(r.branches[0].uniformity, BranchUniformity::Uniform);
        assert_eq!(r.uniform_count(), 1);
    }

    #[test]
    fn if_with_local_invocation_id_condition_is_divergent() {
        let kernel = KernelDescriptor {
            id: "divergent".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
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
                        kind: KernelOpKind::BinOpKind(BinOp::Lt),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![2, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(32)],
            },
        };
        let r = analyze(&kernel);
        assert_eq!(r.branches.len(), 1);
        assert_eq!(r.branches[0].uniformity, BranchUniformity::Divergent);
        assert_eq!(r.divergent_count(), 1);
    }

    #[test]
    fn if_with_workgroup_id_only_is_uniform() {
        // workgroup_id is the same for every thread in the workgroup.
        let kernel = KernelDescriptor {
            id: "wid_uniform".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::WorkgroupId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Eq),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![2, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let r = analyze(&kernel);
        assert_eq!(r.branches[0].uniformity, BranchUniformity::Uniform);
    }

    #[test]
    fn nested_arithmetic_propagates_divergence() {
        // (tid + 5) > 0  -  divergent because tid is in the chain.
        let kernel = KernelDescriptor {
            id: "nested".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
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
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Gt),
                        operands: vec![2, 3],
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![4, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(5), LiteralValue::U32(0)],
            },
        };
        let r = analyze(&kernel);
        assert_eq!(r.branches[0].uniformity, BranchUniformity::Divergent);
    }

    #[test]
    fn no_branches_means_no_report_entries() {
        let kernel = KernelDescriptor {
            id: "no_branch".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
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
            },
        };
        let r = analyze(&kernel);
        assert!(r.branches.is_empty());
    }

    #[test]
    fn if_else_branch_classified_separately() {
        let kernel = KernelDescriptor {
            id: "if_else".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThenElse,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![
                    KernelBody {
                        ops: vec![],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                    KernelBody {
                        ops: vec![],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                ],
                literals: vec![LiteralValue::Bool(false)],
            },
        };
        let r = analyze(&kernel);
        assert_eq!(r.branches.len(), 1);
        assert_eq!(r.branches[0].uniformity, BranchUniformity::Uniform);
    }

    #[test]
    fn condition_from_load_is_unknown() {
        // We can't know at compile time whether two threads read the
        // same value from memory  -  phase 1 marks load-derived
        // conditions as Unknown.
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let kernel = KernelDescriptor {
            id: "load_cond".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::Bool,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "flag".into(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![1, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let r = analyze(&kernel);
        // Documented phase-1 behavior: Loads → Unknown rather than Uniform.
        assert_eq!(r.branches[0].uniformity, BranchUniformity::Unknown);
    }
}

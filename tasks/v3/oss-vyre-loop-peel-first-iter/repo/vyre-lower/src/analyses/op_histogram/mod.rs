//! Per-kind op histogram.
//!
//! Counts each `KernelOpKind` variant occurrence in the descriptor
//! (parent body + recursive child bodies). Useful for telemetry  -
//! answers "what does this kernel mostly do?" with a single struct.
//!
//! Group categories collapse related variants:
//! - `arithmetic` = BinOpKind + UnOpKind + Fma + Cast + Select
//! - `memory` = Load* + Store* + Atomic + AsyncLoad + AsyncStore
//! - `control_flow` = StructuredIfThen + StructuredIfThenElse +
//!   StructuredForLoop + StructuredBlock + Region + Barrier + Return +
//!   Trap + Resume
//! - `subgroup` = SubgroupBallot + SubgroupShuffle + SubgroupAdd
//! - `builtin` = LocalInvocationId + GlobalInvocationId + WorkgroupId +
//!   SubgroupLocalId + SubgroupSize + BufferLength
//! - `literal` = Literal
//! - `other` = Call + OpaqueExpr + OpaqueNode + AsyncWait +
//!   IndirectDispatch

use serde::{Deserialize, Serialize};

use crate::{KernelBody, KernelDescriptor, KernelOpKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpHistogram {
    pub literal: u32,
    pub arithmetic: u32,
    pub memory: u32,
    pub control_flow: u32,
    pub subgroup: u32,
    pub builtin: u32,
    pub other: u32,
}

impl OpHistogram {
    pub fn total(&self) -> u32 {
        self.literal
            + self.arithmetic
            + self.memory
            + self.control_flow
            + self.subgroup
            + self.builtin
            + self.other
    }

    /// True iff every category is zero. The kernel has no ops at all.
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// The dominant category and its count. Returns `None` if the
    /// histogram is empty. Useful for one-line "this kernel is mostly
    /// X" reporting.
    pub fn dominant(&self) -> Option<(&'static str, u32)> {
        let mut entries = [
            ("literal", self.literal),
            ("arithmetic", self.arithmetic),
            ("memory", self.memory),
            ("control_flow", self.control_flow),
            ("subgroup", self.subgroup),
            ("builtin", self.builtin),
            ("other", self.other),
        ];
        entries.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
        let (name, count) = entries[0];
        if count == 0 {
            None
        } else {
            Some((name, count))
        }
    }

    /// True iff the kernel is dominated by memory ops (memory > all
    /// other categories combined). Memory-bound kernels typically
    /// benefit most from coalesce / vec_pack / shared-mem promotion.
    pub fn is_memory_bound(&self) -> bool {
        let non_memory = self.total() - self.memory;
        self.memory > non_memory
    }

    /// True iff the kernel is dominated by arithmetic ops. ALU-bound
    /// kernels benefit from strength_reduce / fma fusion / tensor cores.
    pub fn is_arithmetic_bound(&self) -> bool {
        let non_arith = self.total() - self.arithmetic;
        self.arithmetic > non_arith
    }

    /// Merge another histogram into self (saturating-adds every
    /// category). Useful for corpus-level rollup.
    pub fn merge(&mut self, other: OpHistogram) {
        self.literal = self.literal.saturating_add(other.literal);
        self.arithmetic = self.arithmetic.saturating_add(other.arithmetic);
        self.memory = self.memory.saturating_add(other.memory);
        self.control_flow = self.control_flow.saturating_add(other.control_flow);
        self.subgroup = self.subgroup.saturating_add(other.subgroup);
        self.builtin = self.builtin.saturating_add(other.builtin);
        self.other = self.other.saturating_add(other.other);
    }

    /// Identity element for [`Self::merge`]  -  all zeros. `Default` produces
    /// the same value but `zero()` reads more naturally as a fold seed.
    pub fn zero() -> Self {
        Self::default()
    }

    /// One-line human-readable summary suitable for log lines.
    /// Format: `"N ops: lit=X arith=Y mem=Z cf=W sg=V bi=U other=T"`.
    pub fn format_short(&self) -> String {
        format!(
            "{} ops: lit={} arith={} mem={} cf={} sg={} bi={} other={}",
            self.total(),
            self.literal,
            self.arithmetic,
            self.memory,
            self.control_flow,
            self.subgroup,
            self.builtin,
            self.other,
        )
    }
}

impl std::fmt::Display for OpHistogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format_short())
    }
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> OpHistogram {
    let mut h = OpHistogram::default();
    walk(&desc.body, &mut h);
    h
}

fn walk(body: &KernelBody, h: &mut OpHistogram) {
    for op in &body.ops {
        bump(&op.kind, h);
    }
    for child in &body.child_bodies {
        walk(child, h);
    }
}

fn bump(kind: &KernelOpKind, h: &mut OpHistogram) {
    use KernelOpKind::*;
    match kind {
        Literal => h.literal += 1,
        Copy | BinOpKind(_) | UnOpKind(_) | Fma | MatrixMma { .. } | Select | Cast { .. } => {
            h.arithmetic += 1
        }
        LoadGlobal
        | LoadShared
        | LoadConstant
        | StoreGlobal
        | StoreShared
        | LoopCarrierInit { .. }
        | LoopCarrierEnd { .. }
        | Atomic { .. }
        | AsyncLoad { .. }
        | AsyncStore { .. } => h.memory += 1,
        StructuredIfThen
        | StructuredIfThenElse
        | StructuredForLoop { .. }
        | StructuredBlock
        | Region { .. }
        | Barrier { .. }
        | Return
        | Trap { .. }
        | Resume { .. } => h.control_flow += 1,
        SubgroupBallot | SubgroupShuffle | SubgroupBroadcast | SubgroupReduce { .. } => h.subgroup += 1,
        LocalInvocationId
        | GlobalInvocationId
        | WorkgroupId
        | SubgroupLocalId
        | SubgroupSize
        | LoopIndex { .. }
        | LoopCarrier { .. }
        | BufferLength => h.builtin += 1,
        Call { .. }
        | OpaqueExpr(..)
        | OpaqueNode(..)
        | AsyncWait { .. }
        | IndirectDispatch { .. } => h.other += 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };
    use vyre_foundation::ir::{BinOp, DataType};

    fn build(ops: Vec<KernelOp>, child_bodies: Vec<KernelBody>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops,
                child_bodies,
                literals: vec![LiteralValue::U32(7)],
            },
        }
    }

    #[test]
    fn histogram_merge_is_associative() {
        // (a + b) + c == a + (b + c)
        let a = OpHistogram {
            literal: 2,
            arithmetic: 3,
            memory: 1,
            control_flow: 0,
            subgroup: 0,
            builtin: 0,
            other: 0,
        };
        let b = OpHistogram {
            literal: 5,
            arithmetic: 0,
            memory: 4,
            control_flow: 1,
            subgroup: 0,
            builtin: 2,
            other: 0,
        };
        let c = OpHistogram {
            literal: 1,
            arithmetic: 1,
            memory: 1,
            control_flow: 1,
            subgroup: 1,
            builtin: 1,
            other: 1,
        };

        let mut left = a;
        left.merge(b);
        left.merge(c);

        let mut bc = b;
        bc.merge(c);
        let mut right = a;
        right.merge(bc);

        assert_eq!(left, right);
    }

    #[test]
    fn histogram_zero_is_identity() {
        let h = OpHistogram {
            literal: 7,
            arithmetic: 3,
            memory: 2,
            control_flow: 1,
            subgroup: 0,
            builtin: 0,
            other: 0,
        };
        let mut acc = OpHistogram::zero();
        acc.merge(h);
        assert_eq!(acc, h);

        let mut acc2 = h;
        acc2.merge(OpHistogram::zero());
        assert_eq!(acc2, h);
    }

    #[test]
    fn histogram_merge_aggregates() {
        let mut acc = OpHistogram::zero();
        acc.merge(OpHistogram {
            literal: 2,
            arithmetic: 3,
            memory: 1,
            control_flow: 0,
            subgroup: 0,
            builtin: 0,
            other: 0,
        });
        acc.merge(OpHistogram {
            literal: 5,
            arithmetic: 0,
            memory: 4,
            control_flow: 1,
            subgroup: 0,
            builtin: 2,
            other: 0,
        });
        assert_eq!(acc.literal, 7);
        assert_eq!(acc.arithmetic, 3);
        assert_eq!(acc.memory, 5);
        assert_eq!(acc.control_flow, 1);
        assert_eq!(acc.builtin, 2);
        assert_eq!(acc.total(), 18);
    }

    #[test]
    fn dominant_picks_largest_category() {
        let h = OpHistogram {
            literal: 2,
            arithmetic: 7,
            memory: 5,
            control_flow: 1,
            subgroup: 0,
            builtin: 0,
            other: 0,
        };
        let (name, count) = h.dominant().unwrap();
        assert_eq!(name, "arithmetic");
        assert_eq!(count, 7);
    }

    #[test]
    fn dominant_none_when_empty() {
        let h = OpHistogram::default();
        assert!(h.dominant().is_none());
        assert!(h.is_empty());
    }

    #[test]
    fn empty_kernel_has_all_zeros() {
        let h = analyze(&build(vec![], vec![]));
        assert_eq!(h.total(), 0);
        assert!(!h.is_memory_bound());
        assert!(!h.is_arithmetic_bound());
    }

    #[test]
    fn literal_and_binop_counted_separately() {
        let h = analyze(&build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
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
            ],
            vec![],
        ));
        assert_eq!(h.literal, 2);
        assert_eq!(h.arithmetic, 1);
        assert_eq!(h.total(), 3);
    }

    #[test]
    fn memory_bound_when_loads_dominate() {
        // 5 loads, 1 lit  -  memory > everything else (1 lit).
        let mut ops = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        }];
        for i in 0..5 {
            ops.push(KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 0],
                result: Some(1 + i),
            });
        }
        let h = analyze(&build(ops, vec![]));
        assert_eq!(h.memory, 5);
        assert_eq!(h.literal, 1);
        assert!(h.is_memory_bound());
        assert!(!h.is_arithmetic_bound());
    }

    #[test]
    fn arithmetic_bound_when_binops_dominate() {
        let mut ops = vec![
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(1),
            },
        ];
        for i in 0..5 {
            ops.push(KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 1],
                result: Some(2 + i),
            });
        }
        let h = analyze(&build(ops, vec![]));
        assert_eq!(h.arithmetic, 5);
        assert!(h.is_arithmetic_bound());
        assert!(!h.is_memory_bound());
    }

    #[test]
    fn control_flow_includes_barrier() {
        let h = analyze(&build(
            vec![KernelOp {
                kind: KernelOpKind::Barrier {
                    ordering: vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                },
                operands: vec![],
                result: None,
            }],
            vec![],
        ));
        assert_eq!(h.control_flow, 1);
    }

    #[test]
    fn subgroup_ops_categorized() {
        let h = analyze(&build(
            vec![KernelOp {
                kind: KernelOpKind::SubgroupBallot,
                operands: vec![0],
                result: Some(0),
            }],
            vec![],
        ));
        assert_eq!(h.subgroup, 1);
    }

    #[test]
    fn builtin_ops_categorized() {
        let h = analyze(&build(
            vec![
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::WorkgroupId,
                    operands: vec![0],
                    result: Some(1),
                },
            ],
            vec![],
        ));
        assert_eq!(h.builtin, 2);
    }

    #[test]
    fn child_body_ops_recursively_counted() {
        let child = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 0],
                    result: Some(1),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(3)],
        };
        let h = analyze(&build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![child],
        ));
        // 2 literals (1 parent + 1 child), 1 arithmetic (child Add).
        assert_eq!(h.literal, 2);
        assert_eq!(h.arithmetic, 1);
        assert_eq!(h.total(), 3);
    }

    #[test]
    fn format_short_includes_all_categories() {
        let h = OpHistogram {
            literal: 2,
            arithmetic: 5,
            memory: 3,
            control_flow: 1,
            subgroup: 0,
            builtin: 1,
            other: 0,
        };
        let s = h.format_short();
        assert!(s.contains("12 ops"));
        assert!(s.contains("lit=2"));
        assert!(s.contains("arith=5"));
        assert!(s.contains("mem=3"));
        // Display delegates to format_short.
        assert_eq!(format!("{h}"), s);
    }

    #[test]
    fn total_sums_all_categories() {
        let h = OpHistogram {
            literal: 3,
            arithmetic: 5,
            memory: 7,
            control_flow: 2,
            subgroup: 1,
            builtin: 4,
            other: 0,
        };
        assert_eq!(h.total(), 22);
    }
}

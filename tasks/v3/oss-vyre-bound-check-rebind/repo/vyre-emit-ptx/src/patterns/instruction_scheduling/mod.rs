//! PERF B9: PTX-level instruction scheduling hints.
//!
//! Modern NVIDIA GPUs reorder PTX instructions based on dependency
//! latencies. The driver does most of the work, but PTX exposes
//! `.pragma "nounroll"`, `__pipeline_depth`, and similar hints that
//! pin behavior when the compiler's reordering is suboptimal.
//!
//! This module computes a `SchedulingHints` for a kernel: detects
//! long dependency chains (where back-to-back instructions read what
//! the previous one wrote), reports them as latency-sensitive
//! sequences worth scheduling around.

use serde::{Deserialize, Serialize};
use vyre_lower::{KernelBody, KernelDescriptor};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DependencyChain {
    /// Op-index where the chain starts.
    pub start_op_index: usize,
    /// Length of the chain (number of dependent ops).
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchedulingHints {
    pub kernel_id: String,
    /// Long dependency chains. Each has `length ≥ 4`.
    pub long_chains: Vec<DependencyChain>,
    /// Total ops in the body (for context).
    pub total_op_count: u32,
}

impl SchedulingHints {
    #[must_use]
    pub fn long_chain_count(&self) -> usize {
        self.long_chains.len()
    }

    #[must_use]
    pub fn longest_chain(&self) -> u32 {
        self.long_chains.iter().map(|c| c.length).max().unwrap_or(0)
    }

    #[must_use]
    pub fn schedule_latency_pressure(&self) -> u32 {
        self.longest_chain()
            .saturating_mul(self.long_chain_count().min(u32::MAX as usize) as u32)
    }
}

pub const LONG_CHAIN_THRESHOLD: u32 = 4;

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> SchedulingHints {
    let mut long_chains = Vec::new();
    detect_chains(&desc.body, &mut long_chains, 0);
    SchedulingHints {
        kernel_id: desc.id.clone(),
        long_chains,
        total_op_count: count_ops(&desc.body),
    }
}

fn count_ops(body: &KernelBody) -> u32 {
    let mut total: u32 = body.ops.len() as u32;
    for child in &body.child_bodies {
        total = total.saturating_add(count_ops(child));
    }
    total
}

fn detect_chains(body: &KernelBody, chains: &mut Vec<DependencyChain>, op_index_offset: usize) {
    for start in 0..body.ops.len() {
        let mut len: u32 = 1;
        let mut current_index = start;
        let mut prev_result = body.ops[start].result;
        while let Some(result) = prev_result {
            let Some(next_index) = first_later_consumer(body, result, current_index + 1) else {
                break;
            };
            len = len.saturating_add(1);
            current_index = next_index;
            prev_result = body.ops[next_index].result;
        }
        if len >= LONG_CHAIN_THRESHOLD {
            chains.push(DependencyChain {
                start_op_index: op_index_offset + start,
                length: len,
            });
        }
    }
    for child in &body.child_bodies {
        detect_chains(child, chains, op_index_offset + body.ops.len());
    }
}

fn first_later_consumer(body: &KernelBody, value: u32, start: usize) -> Option<usize> {
    body.ops
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, op)| op.operands.contains(&value).then_some(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::BinOp;
    use vyre_lower::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };

    fn linear_chain(length: usize) -> KernelDescriptor {
        // x0 = literal; x1 = x0 + lit; x2 = x1 + lit; ...
        let mut ops = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        }];
        for i in 1..length {
            ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![1],
                result: Some(100 + i as u32),
            });
            ops.push(KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![(i - 1) as u32, 100 + i as u32],
                result: Some(i as u32),
            });
        }
        KernelDescriptor {
            id: "chain".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
            },
        }
    }

    #[test]
    fn empty_kernel_no_chains() {
        let desc = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let h = analyze(&desc);
        assert!(h.long_chains.is_empty());
        assert_eq!(h.total_op_count, 0);
    }

    #[test]
    fn short_independent_ops_no_long_chain() {
        let desc = KernelDescriptor {
            id: "indep".into(),
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
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let h = analyze(&desc);
        assert!(h.long_chains.is_empty());
        assert_eq!(h.total_op_count, 3);
    }

    #[test]
    fn long_dep_chain_detected() {
        // Build a chain of length 8 (1 literal + 7 add hops).
        let desc = linear_chain(8);
        let h = analyze(&desc);
        // Note: the linear_chain helper interleaves literals so the
        // chain detection sees: lit(r0); lit(r101); add(r0, r101)→r1;
        // lit(r102); add(r1, r102)→r2; ...  -  chain reads previous
        // hop's result through the Add op. The dependency-chain
        // detector should find at least one long chain.
        assert!(!h.long_chains.is_empty());
        assert!(h.longest_chain() >= LONG_CHAIN_THRESHOLD);
    }

    #[test]
    fn longest_chain_aggregates_correctly() {
        let h = SchedulingHints {
            kernel_id: "k".into(),
            long_chains: vec![
                DependencyChain {
                    start_op_index: 0,
                    length: 5,
                },
                DependencyChain {
                    start_op_index: 10,
                    length: 12,
                },
                DependencyChain {
                    start_op_index: 25,
                    length: 8,
                },
            ],
            total_op_count: 50,
        };
        assert_eq!(h.long_chain_count(), 3);
        assert_eq!(h.longest_chain(), 12);
        assert_eq!(h.schedule_latency_pressure(), 36);
    }

    #[test]
    fn longest_chain_zero_when_empty() {
        let h = SchedulingHints {
            kernel_id: "k".into(),
            long_chains: vec![],
            total_op_count: 0,
        };
        assert_eq!(h.longest_chain(), 0);
    }

    #[test]
    fn threshold_constant_is_documented() {
        assert_eq!(LONG_CHAIN_THRESHOLD, 4);
    }
}

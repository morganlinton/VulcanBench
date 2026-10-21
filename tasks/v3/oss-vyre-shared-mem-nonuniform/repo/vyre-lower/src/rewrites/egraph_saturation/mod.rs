//! Bounded descriptor rewrite saturation surface.
//!
//! The release architecture requires e-graph/egglog-family rewrites to
//! have a real integration point. This module provides the deterministic
//! bounded saturation contract used by release evidence and future
//! equality-saturation engines.

use vyre_foundation::ir::BinOp;

use super::body_index::BodyIndex;
use super::literal::ResultAllocator;
use crate::{rewrites, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue};

/// Saturation execution limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SaturationLimits {
    pub max_iterations: u32,
    pub max_nodes: u32,
}

impl Default for SaturationLimits {
    fn default() -> Self {
        Self {
            max_iterations: 8,
            max_nodes: 65_536,
        }
    }
}

/// Result of bounded saturation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SaturationReport {
    pub iterations: u32,
    pub input_ops: usize,
    pub output_ops: usize,
    pub equality_classes: usize,
    pub applied_rewrites: usize,
    pub hit_iteration_limit: bool,
    pub hit_node_limit: bool,
}

/// Run the current deterministic rewrite pipeline under a bounded
/// saturation contract.
///
/// This is intentionally conservative: until the equality-saturation
/// engine owns extraction, the release path gets a bounded fixed-point
/// driver over the descriptor rewrite pipeline. It never returns an
/// unchecked descriptor; callers still run verifier gates around it.
#[must_use]
pub fn saturate_descriptor(
    desc: &KernelDescriptor,
    limits: SaturationLimits,
) -> (KernelDescriptor, SaturationReport) {
    let mut current = desc.clone();
    let input_ops = current.body.ops.len();
    let mut equality_classes = 0usize;
    let mut applied_rewrites = 0usize;
    let mut iterations = 0;
    let mut hit_node_limit = false;
    for _ in 0..limits.max_iterations {
        let algebraic = saturate_local_algebra(&current);
        equality_classes = equality_classes.saturating_add(algebraic.equality_classes);
        applied_rewrites = applied_rewrites.saturating_add(algebraic.applied_rewrites);
        let (next, _) = rewrites::run_all_with_stats(&algebraic.descriptor);
        iterations += 1;
        if next.body.ops.len() as u32 > limits.max_nodes {
            hit_node_limit = true;
            break;
        }
        if next == current {
            current = next;
            break;
        }
        current = next;
    }
    let hit_iteration_limit = iterations == limits.max_iterations;
    let output_ops = current.body.ops.len();
    (
        current,
        SaturationReport {
            iterations,
            input_ops,
            output_ops,
            equality_classes,
            applied_rewrites,
            hit_iteration_limit,
            hit_node_limit,
        },
    )
}

/// Run the non-recursive local algebraic saturation step.
///
/// This is the canonical-pipeline integration point: it performs the
/// e-graph-family reassociation rewrites without calling back into
/// `run_all_with_stats`, so it is safe to compose inside the standard
/// pass sequence.
#[must_use]
pub fn saturate_algebraic_descriptor(
    desc: &KernelDescriptor,
) -> (KernelDescriptor, SaturationReport) {
    let input_ops = desc.body.ops.len();
    let algebraic = saturate_local_algebra(desc);
    let output_ops = algebraic.descriptor.body.ops.len();
    (
        algebraic.descriptor,
        SaturationReport {
            iterations: 1,
            input_ops,
            output_ops,
            equality_classes: algebraic.equality_classes,
            applied_rewrites: algebraic.applied_rewrites,
            hit_iteration_limit: false,
            hit_node_limit: false,
        },
    )
}

struct AlgebraicSaturation {
    descriptor: KernelDescriptor,
    equality_classes: usize,
    applied_rewrites: usize,
}

fn saturate_local_algebra(desc: &KernelDescriptor) -> AlgebraicSaturation {
    let mut descriptor = desc.clone();
    let mut stats = AlgebraicStats::default();
    descriptor.body = saturate_body(descriptor.body, &mut stats);
    AlgebraicSaturation {
        descriptor,
        equality_classes: stats.equality_classes,
        applied_rewrites: stats.applied_rewrites,
    }
}

#[derive(Default)]
struct AlgebraicStats {
    equality_classes: usize,
    applied_rewrites: usize,
}

fn saturate_body(mut body: KernelBody, stats: &mut AlgebraicStats) -> KernelBody {
    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| saturate_body(child, stats))
        .collect();

    let index = BodyIndex::new(&body);
    let mut allocator = ResultAllocator::for_body_tree(&body);
    let mut new_ops = Vec::with_capacity(body.ops.len());
    for current_op in &body.ops {
        let mut op = current_op.clone();
        if let Some(rewrite) = reassociate_constant_chain(&op, &body, &index) {
            let literal_result =
                allocator.push_literal(&mut new_ops, &mut body.literals, rewrite.literal);
            op.operands = vec![rewrite.base, literal_result];
            stats.equality_classes = stats.equality_classes.saturating_add(1);
            stats.applied_rewrites = stats.applied_rewrites.saturating_add(1);
        }
        new_ops.push(op);
    }
    body.ops = new_ops;
    body
}

struct Reassociated {
    base: u32,
    literal: LiteralValue,
}

fn reassociate_constant_chain(
    op: &KernelOp,
    body: &KernelBody,
    index: &BodyIndex,
) -> Option<Reassociated> {
    let KernelOpKind::BinOpKind(
        kind @ (BinOp::Add
        | BinOp::Mul
        | BinOp::BitAnd
        | BinOp::BitOr
        | BinOp::BitXor
        | BinOp::And
        | BinOp::Or),
    ) = &op.kind
    else {
        return None;
    };
    let kind = *kind;
    let left = *op.operands.first()?;
    let right = *op.operands.get(1)?;
    if matches!(kind, BinOp::And | BinOp::Or) {
        return reassociate_bool_constant_chain(kind, left, right, body, index);
    }
    let (inner_result, outer_const) = split_value_const(left, right, body, index)
        .or_else(|| split_value_const(right, left, body, index))?;
    let inner = index.producer(body, inner_result)?;
    let KernelOpKind::BinOpKind(inner_kind) = &inner.kind else {
        return None;
    };
    if *inner_kind != kind {
        return None;
    }
    let inner_left = *inner.operands.first()?;
    let inner_right = *inner.operands.get(1)?;
    let (base, inner_const) = split_value_const(inner_left, inner_right, body, index)
        .or_else(|| split_value_const(inner_right, inner_left, body, index))?;
    let constant = match kind {
        BinOp::Add => inner_const.checked_add(outer_const)?,
        BinOp::Mul => inner_const.checked_mul(outer_const)?,
        BinOp::BitAnd => inner_const & outer_const,
        BinOp::BitOr => inner_const | outer_const,
        BinOp::BitXor => inner_const ^ outer_const,
        _ => return None,
    };
    Some(Reassociated {
        base,
        literal: LiteralValue::U32(constant),
    })
}

fn split_value_const(
    value: u32,
    maybe_const: u32,
    body: &KernelBody,
    index: &BodyIndex,
) -> Option<(u32, u32)> {
    index
        .u32_lit(body, maybe_const)
        .map(|constant| (value, constant))
}

fn reassociate_bool_constant_chain(
    kind: BinOp,
    left: u32,
    right: u32,
    body: &KernelBody,
    index: &BodyIndex,
) -> Option<Reassociated> {
    let (inner_result, outer_const) = split_value_bool(left, right, body, index)
        .or_else(|| split_value_bool(right, left, body, index))?;
    let inner = index.producer(body, inner_result)?;
    let KernelOpKind::BinOpKind(inner_kind) = &inner.kind else {
        return None;
    };
    if *inner_kind != kind {
        return None;
    }
    let inner_left = *inner.operands.first()?;
    let inner_right = *inner.operands.get(1)?;
    let (base, inner_const) = split_value_bool(inner_left, inner_right, body, index)
        .or_else(|| split_value_bool(inner_right, inner_left, body, index))?;
    let literal = match kind {
        BinOp::And => LiteralValue::Bool(inner_const && outer_const),
        BinOp::Or => LiteralValue::Bool(inner_const || outer_const),
        _ => return None,
    };
    Some(Reassociated { base, literal })
}

fn split_value_bool(
    value: u32,
    maybe_const: u32,
    body: &KernelBody,
    index: &BodyIndex,
) -> Option<(u32, bool)> {
    index
        .bool_lit(body, maybe_const)
        .map(|constant| (value, constant))
}

#[cfg(test)]
mod tests {
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };

    use super::*;

    #[test]
    fn bounded_saturation_reports_progress_without_unbounded_loop() {
        let desc = KernelDescriptor {
            id: "sat".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(1)],
            },
        };
        let (_, report) = saturate_descriptor(
            &desc,
            SaturationLimits {
                max_iterations: 2,
                max_nodes: 16,
            },
        );
        assert!(report.iterations >= 1);
        assert_eq!(report.input_ops, 1);
    }

    #[test]
    fn saturation_reassociates_constant_add_chain() {
        let desc = KernelDescriptor {
            id: "sat_add_chain".into(),
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
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![3, 2],
                        result: Some(4),
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(10),
                    LiteralValue::U32(2),
                    LiteralValue::U32(3),
                ],
            },
        };
        let (_, report) = saturate_descriptor(
            &desc,
            SaturationLimits {
                max_iterations: 2,
                max_nodes: 64,
            },
        );
        assert!(report.equality_classes >= 1);
        assert!(report.applied_rewrites >= 1);
    }

    #[test]
    fn saturation_reassociates_constant_bitwise_chain() {
        let desc = KernelDescriptor {
            id: "sat_bitwise_chain".into(),
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
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::BitOr),
                        operands: vec![0, 1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::BitOr),
                        operands: vec![3, 2],
                        result: Some(4),
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0b0001),
                    LiteralValue::U32(0b0010),
                    LiteralValue::U32(0b0100),
                ],
            },
        };
        let (_, report) = saturate_descriptor(
            &desc,
            SaturationLimits {
                max_iterations: 2,
                max_nodes: 64,
            },
        );
        assert!(report.equality_classes >= 1);
        assert!(report.applied_rewrites >= 1);
    }

    #[test]
    fn saturation_reassociates_constant_boolean_chain() {
        let desc = KernelDescriptor {
            id: "sat_bool_chain".into(),
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
                        kind: KernelOpKind::BinOpKind(BinOp::And),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::And),
                        operands: vec![2, 3],
                        result: Some(4),
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::Bool(true),
                    LiteralValue::Bool(true),
                    LiteralValue::Bool(false),
                ],
            },
        };
        let (_, report) = saturate_descriptor(
            &desc,
            SaturationLimits {
                max_iterations: 2,
                max_nodes: 64,
            },
        );
        assert!(report.equality_classes >= 1);
        assert!(report.applied_rewrites >= 1);
    }
}

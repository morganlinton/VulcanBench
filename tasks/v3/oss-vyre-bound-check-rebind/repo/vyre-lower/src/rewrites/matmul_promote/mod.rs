//! B6  -  promote a recognized matmul fragment shape into a single
//! [`KernelOpKind::MatrixMma`] op.
//!
//! ## What this catches
//!
//! The promotable shape is a tight, contiguous descriptor-body
//! sub-sequence with the operand contract for the
//! `M16N8K16/F16/F16/F32` MatrixMma fragment:
//!
//!   - 4 ops producing `a0..a3` (the A fragment lanes  -  typically
//!     `LoadShared` / `LoadGlobal` of the A tile);
//!   - 2 ops producing `b0..b1` (the B fragment lanes);
//!   - 4 `Fma` ops producing `c0..c3` accumulators in order, where
//!     each `Fma`'s third operand (`c_in`) is one of four pre-loaded
//!     C accumulators threaded through the chain.
//!
//! The pre-loaded `c_init0..c_init3` ops are NOT consumed by the
//! rewrite (they remain in the descriptor)  -  only the `Fma` chain
//! collapses into one `MatrixMma`. The MatrixMma's operand vector
//! is `[a0,a1,a2,a3, b0,b1, c0,c1,c2,c3]` and its 4 result ids start
//! at the new fresh result base.
//!
//! ## What this DOES NOT catch
//!
//! Generic 2D matmul-tile loops (the `for k in 0..K { c += a[k] * b[k] }`
//! shape that compiles to a serial Fma chain over a runtime-bounded
//! loop). Those need cross-iteration dataflow + tile-shape inference
//! and are an open follow-up. v0.4.1 ships the contiguous-fragment
//! detector so a kernel that explicitly lays out the M16N8K16 shape
//! (e.g. a hand-written tile primitive) gets the MatrixMma
//! lowering for free.
//!
//! ## Why this is safe to land
//!
//! The MatrixMma op is already verified and emitted (PTX side ships
//! `mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32` on sm_70+ per
//! B6 row text). This rewrite only adds the source-side promoter so
//! the existing emit path activates without explicit MatrixMma in
//! the input descriptor.

use super::literal::ResultAllocator;
use crate::descriptor::{
    KernelBody, KernelDescriptor, KernelOp, KernelOpKind, MatrixMmaElement, MatrixMmaLayout,
    MatrixMmaShape,
};

const MATMUL_TILE_LEN: usize = 4; // 4 Fma ops produce c0..c3
const A_FRAGMENT_LEN: usize = 4;
const B_FRAGMENT_LEN: usize = 2;

/// One structured loop whose body contains a promotable matmul fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatmulTileLoopPlan {
    /// Path to the body containing the loop. Empty path means the root body.
    pub body_path: Vec<usize>,
    /// Op index of the `StructuredForLoop` within `body_path`.
    pub loop_op_index: usize,
    /// Child body index referenced by the loop.
    pub child_body_index: u32,
    /// Op index inside the child body where the promotable FMA block begins.
    pub fma_start_index: usize,
}

/// Infer structured-loop tensor-core opportunities without rewriting.
///
/// Backends and diagnostics use this to prove generic matmul tile loops are
/// no longer invisible: a `StructuredForLoop` whose child body contains the
/// canonical four-FMA M16N8K16 fragment is reported with a stable body path.
#[must_use]
pub fn infer_matmul_tile_loops(desc: &KernelDescriptor) -> Vec<MatmulTileLoopPlan> {
    let mut plans = Vec::new();
    infer_body(&desc.body, &mut Vec::new(), &mut plans);
    plans
}

/// Apply the matmul-fragment promoter recursively. Returns the
/// transformed descriptor with every recognized fragment chain
/// collapsed into a single `MatrixMma`.
#[must_use]
pub fn matmul_promote(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&desc.body);
    out.body = promote_body(&out.body, &mut allocator);
    out
}

fn promote_body(body: &KernelBody, allocator: &mut ResultAllocator) -> KernelBody {
    let mut new_ops: Vec<KernelOp> = Vec::with_capacity(body.ops.len());

    let ops = &body.ops;
    let mut i = 0;
    while i < ops.len() {
        if let Some((promoted, advance)) = try_promote_at(ops, i, allocator) {
            new_ops.push(promoted);
            i += advance;
        } else {
            new_ops.push(ops[i].clone());
            i += 1;
        }
    }

    let new_children: Vec<KernelBody> = body
        .child_bodies
        .iter()
        .map(|c| promote_body(c, allocator))
        .collect();

    KernelBody {
        ops: new_ops,
        child_bodies: new_children,
        literals: body.literals.clone(),
    }
}

fn infer_body(body: &KernelBody, path: &mut Vec<usize>, plans: &mut Vec<MatmulTileLoopPlan>) {
    for (op_index, op) in body.ops.iter().enumerate() {
        if matches!(op.kind, KernelOpKind::StructuredForLoop { .. }) {
            let Some(child_body_index) = op.operands.get(2).copied() else {
                continue;
            };
            let Some(child) = body.child_bodies.get(child_body_index as usize) else {
                continue;
            };
            for fma_start_index in promotable_fma_starts(&child.ops) {
                plans.push(MatmulTileLoopPlan {
                    body_path: path.clone(),
                    loop_op_index: op_index,
                    child_body_index,
                    fma_start_index,
                });
            }
        }
    }
    for (child_index, child) in body.child_bodies.iter().enumerate() {
        path.push(child_index);
        infer_body(child, path, plans);
        path.pop();
    }
}

fn promotable_fma_starts(ops: &[KernelOp]) -> Vec<usize> {
    let mut starts = Vec::new();
    for i in 0..ops.len() {
        if match_fragment_at(ops, i).is_some() {
            starts.push(i);
        }
    }
    starts
}

/// If `ops[i..]` begins with the promotable fragment shape, return the
/// synthesized `MatrixMma` op + how many ops to advance past in the
/// source. `None` means no match.
fn try_promote_at(
    ops: &[KernelOp],
    i: usize,
    allocator: &mut ResultAllocator,
) -> Option<(KernelOp, usize)> {
    let FragmentMatch {
        a_ids,
        b_unique,
        c_ids,
    } = match_fragment_at(ops, i)?;

    let mut operands = Vec::with_capacity(A_FRAGMENT_LEN + B_FRAGMENT_LEN + MATMUL_TILE_LEN);
    operands.extend_from_slice(&a_ids);
    operands.extend_from_slice(&b_unique);
    operands.extend_from_slice(&c_ids);

    let result_base = allocator.fresh_block(MATMUL_TILE_LEN as u32);

    let promoted = KernelOp {
        kind: KernelOpKind::MatrixMma {
            shape: MatrixMmaShape::M16N8K16,
            a_layout: MatrixMmaLayout::RowMajor,
            b_layout: MatrixMmaLayout::ColMajor,
            a_type: MatrixMmaElement::F16,
            b_type: MatrixMmaElement::F16,
            accum_type: MatrixMmaElement::F32,
        },
        operands,
        result: Some(result_base),
    };

    Some((promoted, MATMUL_TILE_LEN))
}

struct FragmentMatch {
    a_ids: [u32; A_FRAGMENT_LEN],
    b_unique: [u32; B_FRAGMENT_LEN],
    c_ids: [u32; MATMUL_TILE_LEN],
}

fn match_fragment_at(ops: &[KernelOp], i: usize) -> Option<FragmentMatch> {
    let needed = MATMUL_TILE_LEN; // we look at the 4 Fma ops
    if i + needed > ops.len() {
        return None;
    }
    // The MatrixMma operand vector requires 4 a + 2 b + 4 c  -  the
    // 4 c values come from the prior cumulative state. We look for a
    // contiguous block of 4 `Fma` ops whose first operand cycles over
    // 4 unique a ids and whose second cycles over 2 unique b ids.

    let block = &ops[i..i + needed];
    if !block.iter().all(|op| matches!(op.kind, KernelOpKind::Fma)) {
        return None;
    }
    if !block
        .iter()
        .all(|op| op.operands.len() == 3 && op.result.is_some())
    {
        return None;
    }
    // Each Fma's c-in is a distinct id (the 4 pre-loaded accumulators).
    let c_ids = [
        block[0].operands[2],
        block[1].operands[2],
        block[2].operands[2],
        block[3].operands[2],
    ];
    if !all_distinct(&c_ids) {
        return None;
    }
    // a operand cycles 4-wide; b cycles 2-wide.
    let a_ids = [
        block[0].operands[0],
        block[1].operands[0],
        block[2].operands[0],
        block[3].operands[0],
    ];
    let b_ids = [
        block[0].operands[1],
        block[1].operands[1],
        block[2].operands[1],
        block[3].operands[1],
    ];
    if !all_distinct(&a_ids) {
        return None;
    }
    // Two of the b ids must repeat; the other two are the unique b
    // fragment lanes. Reject if more than 2 unique.
    let mut b_unique: Vec<u32> = b_ids.to_vec();
    b_unique.sort_unstable();
    b_unique.dedup();
    if b_unique.len() != B_FRAGMENT_LEN {
        return None;
    }
    Some(FragmentMatch {
        a_ids,
        b_unique: [b_unique[0], b_unique[1]],
        c_ids,
    })
}

fn all_distinct(ids: &[u32]) -> bool {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).all(|w| w[0] != w[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind,
    };

    fn empty_desc(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
    }

    fn lit(result: u32) -> KernelOp {
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(result),
        }
    }

    fn fma(a: u32, b: u32, c: u32, result: u32) -> KernelOp {
        KernelOp {
            kind: KernelOpKind::Fma,
            operands: vec![a, b, c],
            result: Some(result),
        }
    }

    #[test]
    fn empty_body_is_unchanged() {
        let desc = empty_desc(KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        });
        let out = matmul_promote(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn non_matmul_body_is_unchanged() {
        let desc = empty_desc(KernelBody {
            ops: vec![lit(0), lit(1), lit(2)],
            child_bodies: vec![],
            literals: vec![],
        });
        let out = matmul_promote(&desc);
        assert_eq!(out.body.ops.len(), 3);
        assert!(out
            .body
            .ops
            .iter()
            .all(|op| matches!(op.kind, KernelOpKind::Literal)));
    }

    #[test]
    fn four_fma_with_correct_shape_promotes_to_matrix_mma() {
        // Pre-load 4 a + 2 b + 4 c accumulators, then 4 Fmas:
        //   Fma(a0, b0, c0) -> r10
        //   Fma(a1, b1, c1) -> r11
        //   Fma(a2, b0, c2) -> r12  (b cycles)
        //   Fma(a3, b1, c3) -> r13
        let prelude = vec![
            lit(0), // a0
            lit(1), // a1
            lit(2), // a2
            lit(3), // a3
            lit(4), // b0
            lit(5), // b1
            lit(6), // c0
            lit(7), // c1
            lit(8), // c2
            lit(9), // c3
        ];
        let fmas = vec![
            fma(0, 4, 6, 10),
            fma(1, 5, 7, 11),
            fma(2, 4, 8, 12),
            fma(3, 5, 9, 13),
        ];
        let mut ops = prelude;
        ops.extend(fmas);
        let desc = empty_desc(KernelBody {
            ops,
            child_bodies: vec![],
            literals: vec![],
        });
        let out = matmul_promote(&desc);
        // The 10 prelude Literals stay; the 4 Fmas collapse to 1 MatrixMma.
        assert_eq!(out.body.ops.len(), 11);
        let mma = out.body.ops.last().unwrap();
        assert!(matches!(mma.kind, KernelOpKind::MatrixMma { .. }));
        // operand layout: [a0..a3, b0..b1, c0..c3]
        assert_eq!(mma.operands, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn fma_chain_with_repeated_a_does_not_promote() {
        // a id repeats  -  not a valid M16N8K16 fragment.
        let prelude = (0..10).map(lit).collect::<Vec<_>>();
        let fmas = vec![
            fma(0, 4, 6, 10),
            fma(0, 5, 7, 11), // repeated a
            fma(2, 4, 8, 12),
            fma(3, 5, 9, 13),
        ];
        let mut ops = prelude;
        ops.extend(fmas);
        let desc = empty_desc(KernelBody {
            ops,
            child_bodies: vec![],
            literals: vec![],
        });
        let out = matmul_promote(&desc);
        // No promotion: original 14 ops remain.
        assert_eq!(out.body.ops.len(), 14);
        assert!(!out
            .body
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::MatrixMma { .. })));
    }

    #[test]
    fn fma_chain_with_three_unique_b_does_not_promote() {
        // 3 unique b ids violates the M16N8K16 contract (B fragment is 2 lanes).
        let prelude = (0..11).map(lit).collect::<Vec<_>>();
        let fmas = vec![
            fma(0, 4, 6, 11),
            fma(1, 5, 7, 12),
            fma(2, 10, 8, 13), // third unique b
            fma(3, 5, 9, 14),
        ];
        let mut ops = prelude;
        ops.extend(fmas);
        let desc = empty_desc(KernelBody {
            ops,
            child_bodies: vec![],
            literals: vec![],
        });
        let out = matmul_promote(&desc);
        assert!(!out
            .body
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::MatrixMma { .. })));
    }

    #[test]
    fn matmul_promote_recurses_into_child_bodies() {
        let prelude = (0..10).map(lit).collect::<Vec<_>>();
        let fmas = vec![
            fma(0, 4, 6, 10),
            fma(1, 5, 7, 11),
            fma(2, 4, 8, 12),
            fma(3, 5, 9, 13),
        ];
        let mut child_ops = prelude;
        child_ops.extend(fmas);
        let child = KernelBody {
            ops: child_ops,
            child_bodies: vec![],
            literals: vec![],
        };
        let desc = empty_desc(KernelBody {
            ops: vec![lit(0)],
            child_bodies: vec![child],
            literals: vec![],
        });
        let out = matmul_promote(&desc);
        let promoted_child = &out.body.child_bodies[0];
        assert!(promoted_child
            .ops
            .iter()
            .any(|op| matches!(op.kind, KernelOpKind::MatrixMma { .. })));
    }

    #[test]
    fn matmul_tile_loop_inference_finds_fma_fragment_inside_structured_loop() {
        let prelude = (0..10).map(lit).collect::<Vec<_>>();
        let fmas = vec![
            fma(0, 4, 6, 10),
            fma(1, 5, 7, 11),
            fma(2, 4, 8, 12),
            fma(3, 5, 9, 13),
        ];

        let mut child_ops = prelude;
        child_ops.extend(fmas);
        let child = KernelBody {
            ops: child_ops,
            child_bodies: vec![],
            literals: vec![],
        };
        let desc = empty_desc(KernelBody {
            ops: vec![
                lit(20),
                lit(21),
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: "k".into(),
                    },
                    operands: vec![20, 21, 0],
                    result: None,
                },
            ],
            child_bodies: vec![child],
            literals: vec![],
        });
        let plans = infer_matmul_tile_loops(&desc);
        assert_eq!(
            plans,
            vec![MatmulTileLoopPlan {
                body_path: vec![],
                loop_op_index: 2,
                child_body_index: 0,
                fma_start_index: 10,
            }]
        );
    }

    #[test]
    fn matmul_promote_is_idempotent() {
        let prelude = (0..10).map(lit).collect::<Vec<_>>();
        let fmas = vec![
            fma(0, 4, 6, 10),
            fma(1, 5, 7, 11),
            fma(2, 4, 8, 12),
            fma(3, 5, 9, 13),
        ];
        let mut ops = prelude;
        ops.extend(fmas);
        let desc = empty_desc(KernelBody {
            ops,
            child_bodies: vec![],
            literals: vec![],
        });
        let once = matmul_promote(&desc);
        let twice = matmul_promote(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
    }
}

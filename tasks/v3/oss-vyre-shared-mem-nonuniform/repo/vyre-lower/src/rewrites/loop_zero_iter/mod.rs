//! Zero-iteration loop elimination  -  drop `StructuredForLoop` ops
//! whose `lo` and `hi` are both U32 literals with `lo >= hi`.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4 (loop
//! peeling subset). Companion to `loop_unroll` which inlines small-
//! count loops; this pass handles the degenerate zero-count case
//! that loop_unroll's body-inline doesn't apply to.
//!
//! Patterns rewritten:
//! - `StructuredForLoop { lo: Lit(L), hi: Lit(H), body }` where
//!   `L >= H` → drop the op entirely (and dispose of the orphaned
//!   child body via `drop_unused_child_bodies` later in the pipeline)
//!
//! Out of scope:
//! - Loops with non-literal bounds  -  even when range analysis can
//!   prove `lo == hi` at runtime, that proof requires the consumer's range
//!   substrate; this is a pure constant-folding pass over literal
//!   operands.
//! - Loop-with-body-of-size-zero (`body: Vec::new()`)  -  a separate
//!   pattern that `descriptor_dce` and `drop_unused_child_bodies`
//!   already handle when the body never reads/writes.
//!
//! Recurses into nested control flow. Idempotent (a second pass
//! finds no zero-iter loops because the first one already dropped
//! them). Wired into `CANONICAL_REWRITE_PASSES` after
//! `loop_unroll` so any zero-count loops exposed by loop unrolling
//! itself drop in the same fixpoint phase.

use super::body_index::BodyIndex;
use crate::{KernelBody, KernelDescriptor, KernelOpKind};

#[must_use]
pub fn loop_zero_iter(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = loop_zero_iter_body(out.body);
    out
}

fn loop_zero_iter_body(mut body: KernelBody) -> KernelBody {
    let index = BodyIndex::new(&body);

    let mut drop_indices: Vec<usize> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        if !matches!(op.kind, KernelOpKind::StructuredForLoop { .. }) {
            continue;
        }
        if op.operands.len() < 2 {
            continue;
        }
        let lo_id = op.operands[0];
        let hi_id = op.operands[1];
        let lo = match index.u32_lit(&body, lo_id) {
            Some(v) => v,
            None => continue,
        };
        let hi = match index.u32_lit(&body, hi_id) {
            Some(v) => v,
            None => continue,
        };
        if lo >= hi {
            drop_indices.push(idx);
        }
    }

    // Drop in reverse so earlier indices stay valid through the
    // splice. Also clear each orphaned child body  -  the verifier
    // recurses into ALL child bodies with their lexically-computed
    // scopes, and an orphaned body that no control-flow op activates
    // receives an empty scope, causing any parent-ref operands inside
    // it to appear as dangling. Replacing with an empty body prevents
    // the false positive while leaving the positional slot intact so
    // other ops' child-body indices remain valid.
    for idx in drop_indices.into_iter().rev() {
        let child_idx = body.ops[idx].operands.get(2).copied().unwrap_or(u32::MAX);
        body.ops.remove(idx);
        if let Some(child) = body.child_bodies.get_mut(child_idx as usize) {
            *child = KernelBody {
                ops: Vec::new(),
                child_bodies: Vec::new(),
                literals: Vec::new(),
            };
        }
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(loop_zero_iter_body)
        .collect();
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelOp, LiteralValue};
    fn empty_body() -> KernelBody {
        KernelBody {
            ops: Vec::new(),
            child_bodies: Vec::new(),
            literals: Vec::new(),
        }
    }

    fn descriptor_with(body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "loop_zero_iter_test".into(),
            bindings: BindingLayout { slots: Vec::new() },
            dispatch: Dispatch::new(1, 1, 1),
            body,
        }
    }

    fn lit_u32(body: &mut KernelBody, value: u32, result: u32) {
        let pool_idx = body.literals.len() as u32;
        body.literals.push(LiteralValue::U32(value));
        body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![pool_idx],
            result: Some(result),
        });
    }

    fn for_loop(body: &mut KernelBody, lo_id: u32, hi_id: u32, child_idx: u32, var: &str) {
        body.ops.push(KernelOp {
            kind: KernelOpKind::StructuredForLoop {
                loop_var: std::sync::Arc::from(var),
            },
            operands: vec![lo_id, hi_id, child_idx],
            result: None,
        });
    }

    #[test]
    fn equal_bounds_drop_loop() {
        let mut body = empty_body();
        lit_u32(&mut body, 5, 0); // lo
        lit_u32(&mut body, 5, 1); // hi (== lo)
        for_loop(&mut body, 0, 1, 0, "i");
        body.child_bodies.push(empty_body());
        let desc = loop_zero_iter(&descriptor_with(body));
        assert!(
            !desc
                .body
                .ops
                .iter()
                .any(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. })),
            "Fix: loop with lo == hi must be dropped"
        );
    }

    #[test]
    fn lo_greater_than_hi_drops_loop() {
        let mut body = empty_body();
        lit_u32(&mut body, 10, 0);
        lit_u32(&mut body, 5, 1);
        for_loop(&mut body, 0, 1, 0, "i");
        body.child_bodies.push(empty_body());
        let desc = loop_zero_iter(&descriptor_with(body));
        assert!(
            !desc
                .body
                .ops
                .iter()
                .any(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. })),
            "Fix: loop with lo > hi must be dropped"
        );
    }

    #[test]
    fn live_loop_kept() {
        let mut body = empty_body();
        lit_u32(&mut body, 0, 0);
        lit_u32(&mut body, 4, 1);
        for_loop(&mut body, 0, 1, 0, "i");
        body.child_bodies.push(empty_body());
        let desc = loop_zero_iter(&descriptor_with(body));
        assert!(
            desc.body
                .ops
                .iter()
                .any(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. })),
            "Fix: live loop (lo < hi) must NOT be dropped"
        );
    }

    #[test]
    fn non_literal_bounds_kept() {
        // lo and hi resolve to non-Literal ops.
        let mut body = empty_body();
        body.ops.push(KernelOp {
            kind: KernelOpKind::GlobalInvocationId,
            operands: vec![0], // axis 0
            result: Some(0),
        });
        body.ops.push(KernelOp {
            kind: KernelOpKind::LocalInvocationId,
            operands: vec![1], // axis 1
            result: Some(1),
        });
        for_loop(&mut body, 0, 1, 0, "i");
        body.child_bodies.push(empty_body());
        let desc = loop_zero_iter(&descriptor_with(body));
        assert!(
            desc.body
                .ops
                .iter()
                .any(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. })),
            "Fix: non-literal-bound loops must be left alone"
        );
    }

    #[test]
    fn rewrite_is_idempotent() {
        let mut body = empty_body();
        lit_u32(&mut body, 3, 0);
        lit_u32(&mut body, 3, 1);
        for_loop(&mut body, 0, 1, 0, "i");
        body.child_bodies.push(empty_body());
        let desc = descriptor_with(body);
        let once = loop_zero_iter(&desc);
        let twice = loop_zero_iter(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn recurses_into_child_bodies() {
        let mut child = empty_body();
        lit_u32(&mut child, 7, 10);
        lit_u32(&mut child, 7, 11);
        for_loop(&mut child, 10, 11, 0, "j");
        child.child_bodies.push(empty_body());

        let mut body = empty_body();
        body.child_bodies.push(child);
        let desc = loop_zero_iter(&descriptor_with(body));
        let child_out = &desc.body.child_bodies[0];
        assert!(
            !child_out
                .ops
                .iter()
                .any(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. })),
            "Fix: zero-iter loops in child bodies must drop too"
        );
    }
}

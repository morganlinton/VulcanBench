//! Megakernel rule-fusion with cross-rule CSE (G2).
//!
//! # Idea
//!
//! Today every rule dispatches as its own Program. 14 launch rules
//! = 14 dispatches with no sharing. Most of them read the same
//! buffers (`input`, `output_slots`, the decoded-bytes view) and
//! run sibling ops on sibling lanes. Paying for 14 separate
//! pipeline creations, 14 launch barriers, and 14 DRAM round-trips
//! on shared inputs is pure waste.
//!
//! The fusion pass collapses N Programs into a single Program that
//! runs every rule's body back-to-back in one dispatch. Shared
//! buffers collapse to a single `BufferDecl` via the name-keyed
//! union in [`crate::execution_plan::fusion::fuse_programs_vec`], which
//! also handles the read-after-write hazard analysis and barrier
//! insertion.
//!
//! # What this adds on top of `fuse_programs_vec`
//!
//! 1. A multi-Program free function ([`fuse_cse`]) that callers
//!    who already own a `Vec<Program>` can invoke directly. It
//!    pre-validates workgroup-size compatibility (which the
//!    single-Program `ProgramPass` trait
//!    cannot express) before delegating to `fuse_programs_vec`.
//! 2. `cse_savings`  -  counts buffer-level sharing so the G12
//!    benchmark harness can assert fusion is actually happening
//!    and a regression (savings drop to 0) gets flagged.
//!
//! # Soundness
//!
//! Self-aliasing + `non_composable_with_self` enforcement is
//! delegated to `fuse_programs_vec` (which rejects self-aliasing
//! pairs with `FusionError::SelfAliasing` and inherits the
//! flag from inputs). Mixed workgroup sizes are rejected at this
//! layer (returns `None`)  -  the caller is expected to normalise
//! via the `autotune` pass first.

use rustc_hash::FxHashSet;

use crate::execution_plan::fusion::fuse_programs_vec;
use crate::ir::Program;

/// Fuse a set of Programs into one. Returns `None` when the inputs
/// cannot be unified (conflicting workgroup sizes, self-aliasing).
///
/// Semantics match `fuse_programs_vec`: entry bodies run in order,
/// barriers are inserted where a buffer is read by one arm and
/// written by a later arm. Shared buffers collapse to a single
/// declaration  -  that is the CSE.
#[must_use]
pub fn fuse_cse(mut programs: Vec<Program>) -> Option<Program> {
    if programs.is_empty() {
        return Some(Program::empty());
    }
    if programs.len() == 1 {
        return programs.pop();
    }

    // Reject conflicting workgroup sizes  -  the caller must run
    // autotune first to normalise.
    let wg0 = programs[0].workgroup_size;
    if programs.iter().any(|p| p.workgroup_size != wg0) {
        return None;
    }

    fuse_programs_vec(programs).ok()
}

/// CSE savings from fusion: how many buffer declarations the
/// fused Program omitted compared to the independent pre-fusion
/// set. A savings of 0 means every rule brought unique buffers  -
/// the pass is doing no buffer-level sharing and is a regression
/// signal. The G12 benchmark harness asserts this is > 0 for
/// realistic rule sets (which typically share `input` and
/// `output_slots`).
#[must_use]
pub fn cse_savings(before: &[Program], after: &Program) -> usize {
    let total_before: usize = before.iter().map(|p| p.buffers.len()).sum();
    let total_after = after.buffers.len();
    total_before.saturating_sub(total_after)
}

/// How many *distinct* buffers the fused Program touches.
///
/// Useful for benchmarks that want to log "N rules fused to K
/// unique buffers" alongside `cse_savings`.
#[must_use]
pub fn unique_buffer_count(after: &Program) -> usize {
    let mut seen: FxHashSet<&str> = FxHashSet::default();
    for buf in after.buffers.iter() {
        seen.insert(buf.name());
    }
    seen.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn rule(name: &str, extra_buf: Option<&str>) -> Program {
        let mut buffers = vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(32),
            BufferDecl::storage("output_slots", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(32),
        ];
        if let Some(b) = extra_buf {
            buffers.push(
                BufferDecl::storage(b, 2, BufferAccess::ReadOnly, DataType::U32).with_count(32),
            );
        }
        let d = Expr::InvocationId { axis: 0 };
        Program::wrapped(
            buffers,
            [64, 1, 1],
            vec![Node::let_bind(format!("{name}_val"), d)],
        )
    }

    #[test]
    fn empty_returns_empty_program() {
        let fused = fuse_cse(Vec::new())
            .expect("Fix: empty input is ok; restore this invariant before continuing.");
        assert_eq!(fused.buffers.len(), 0);
    }

    #[test]
    fn single_program_is_returned_unchanged() {
        let p = rule("r1", None);
        let before_bufs = p.buffers.len();
        let fused = fuse_cse(vec![p]).unwrap();
        assert_eq!(fused.buffers.len(), before_bufs);
    }

    #[test]
    fn two_rules_with_shared_input_collapse_buffers() {
        let r1 = rule("r1", None);
        let r2 = rule("r2", None);
        let before: Vec<_> = vec![r1.clone(), r2.clone()];
        let fused = fuse_cse(vec![r1, r2]).unwrap();
        // Two rules × 2 identical buffers = 4 declarations before,
        // 2 after fusion.
        assert_eq!(fused.buffers.len(), 2);
        assert_eq!(cse_savings(&before, &fused), 2);
        assert_eq!(unique_buffer_count(&fused), 2);
    }

    #[test]
    fn unique_buffers_are_preserved() {
        let r1 = rule("r1", Some("r1_private"));
        let r2 = rule("r2", Some("r2_private"));
        let before: Vec<_> = vec![r1.clone(), r2.clone()];
        let fused = fuse_cse(vec![r1, r2]).unwrap();
        // Shared: input, output_slots (2). Unique: r1_private,
        // r2_private (2). Total: 4.
        assert_eq!(fused.buffers.len(), 4);
        // Saved: 2 (the two duplicated buffers).
        assert_eq!(cse_savings(&before, &fused), 2);
    }

    #[test]
    fn conflicting_workgroup_sizes_are_rejected() {
        let r1 = Program::wrapped(
            vec![BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1)],
            [64, 1, 1],
            vec![Node::let_bind("x", Expr::u32(0))],
        );
        let r2 = Program::wrapped(
            vec![BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1)],
            [128, 1, 1],
            vec![Node::let_bind("x", Expr::u32(0))],
        );
        assert!(fuse_cse(vec![r1, r2]).is_none());
    }

    #[test]
    fn savings_are_zero_when_no_shared_buffers() {
        let r1 = Program::wrapped(
            vec![
                BufferDecl::storage("r1_a", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            ],
            [64, 1, 1],
            vec![Node::let_bind("x", Expr::u32(0))],
        );
        let r2 = Program::wrapped(
            vec![
                BufferDecl::storage("r2_a", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            ],
            [64, 1, 1],
            vec![Node::let_bind("x", Expr::u32(0))],
        );
        let before: Vec<_> = vec![r1.clone(), r2.clone()];
        let fused = fuse_cse(vec![r1, r2]).unwrap();
        assert_eq!(cse_savings(&before, &fused), 0);
    }

    #[test]
    fn savings_scale_with_rule_count() {
        let rules: Vec<Program> = (0..5).map(|_| rule("r", None)).collect();
        let before = rules.clone();
        let fused = fuse_cse(rules).unwrap();
        // 5 × 2 = 10 before; 2 after. Savings = 8.
        assert_eq!(cse_savings(&before, &fused), 8);
    }
}

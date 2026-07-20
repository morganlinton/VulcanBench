//! `buffer_decl_sort`  -  canonicalize `BufferDecl` order by `(binding, name)`.
//!
//! Op id: `vyre-foundation::optimizer::passes::buffer_decl_sort`. Soundness:
//! `Exact`  -  the IR semantics are reference-by-name, not reference-by-index,
//! so reordering BufferDecls is not observable in program behavior.
//! Cost-direction: identity  -  node_count, instruction_count, all 8 cost dimensions are
//! unchanged. Preserves: every analysis. Invalidates: nothing.
//!
//! ## Why
//!
//! Two surface motivations:
//!
//! 1. **Wire-content-hash cache hit rate.** The wire-content hash keys
//!    the differential-compilation cache. Two Programs
//!    that differ only in `BufferDecl` order serialize to different bytes
//!    and miss the cache, even though they emit byte-identical backend code.
//!    Canonicalizing the BufferDecl order at the start of the lowering
//!    pipeline collapses these aliases into one cache key, raising hit
//!    rate on the same workload across reorderings.
//!
//! 2. **Deterministic backend emission.** Target emitters walk
//!    `Program::buffers()` in slice order. Without a canonicalization
//!    pass, generated source changes whenever a frontend rebuilds
//!    the `Program` in a slightly different order  -  bad for shader cache,
//!    bad for reproducible builds, bad for diffing emitted code.
//!
//! ## Rule
//!
//! Sort BufferDecls by:
//!   - primary key: `binding` ascending,
//!   - tie-breaker: `name` ascending (lexicographic on the `Arc<str>`).
//!
//! All other fields are preserved verbatim (access, kind, element, count,
//! is_output, pipeline_live_out, output_byte_range, hints, bytes_extraction,
//! linear_type, shape_predicate). The IR entry body is untouched.
//!
//! ## Why this ordering
//!
//! Binding is the primary key because it is the canonical resource
//! addressing identifier  -  sorting by it makes the lowered output's
//! resource-table layout match the IR's BufferDecl table layout, which is
//! what every target emitter already wants. Name is the tie-breaker because two
//! BufferDecls SHOULD never share a binding (the validator catches this),
//! but if they ever do, deterministic ordering is still better than
//! whatever the source order happened to be.

use crate::ir::Program;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};

/// Sort `Program::buffers()` by `(binding, name)` for deterministic
/// downstream emission and cache-hit-rate maximization.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "buffer_decl_sort",
    requires = [],
    invalidates = []
)]
pub struct BufferDeclSortPass;

impl BufferDeclSortPass {
    /// Skip programs whose `BufferDecls` are already sorted.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if buffers_in_canonical_order(program) {
            PassAnalysis::SKIP
        } else {
            PassAnalysis::RUN
        }
    }

    /// Re-emit the program with `BufferDecls` sorted by `(binding, name)`.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        if buffers_in_canonical_order(&program) {
            return PassResult {
                program,
                changed: false,
            };
        }
        let mut buffers = program.buffers().to_vec();
        // (binding, name) is unique per buffer (the validator rejects
        // duplicates), so the order of equal keys is irrelevant  -
        // unstable sort is faster than the stable sort_by and produces
        // the same canonical output.
        buffers
            .sort_unstable_by(|a, b| a.binding.cmp(&b.binding).then_with(|| a.name.cmp(&b.name)));
        let new_program = program.with_rewritten_buffers(buffers);
        PassResult {
            program: new_program,
            changed: true,
        }
    }
}

/// True iff the `BufferDecl` slice is already sorted by `(binding, name)`.
fn buffers_in_canonical_order(program: &Program) -> bool {
    let buffers = program.buffers();
    if buffers.len() < 2 {
        return true;
    }
    buffers
        .windows(2)
        .all(|pair| match pair[0].binding.cmp(&pair[1].binding) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Equal => pair[0].name <= pair[1].name,
            std::cmp::Ordering::Greater => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf(name: &str, binding: u32) -> BufferDecl {
        BufferDecl::storage(name, binding, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn ro_buf(name: &str, binding: u32) -> BufferDecl {
        BufferDecl::storage(name, binding, BufferAccess::ReadOnly, DataType::U32).with_count(4)
    }

    fn entry() -> Vec<Node> {
        vec![Node::store("a", Expr::u32(0), Expr::u32(7))]
    }

    #[test]
    fn skip_analysis_on_already_sorted() {
        let program = Program::wrapped(vec![buf("a", 0), buf("b", 1)], [1, 1, 1], entry());
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&BufferDeclSortPass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn run_analysis_on_unsorted() {
        let program = Program::wrapped(vec![buf("a", 1), buf("b", 0)], [1, 1, 1], entry());
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&BufferDeclSortPass, &program),
            PassAnalysis::RUN
        );
    }

    #[test]
    fn transform_sorts_simple_two_buffer_swap() {
        let program = Program::wrapped(vec![buf("late", 5), buf("early", 0)], [1, 1, 1], entry());
        let result = BufferDeclSortPass::transform(program);
        assert!(result.changed);
        let bindings: Vec<u32> = result.program.buffers().iter().map(|b| b.binding).collect();
        assert_eq!(bindings, vec![0, 5]);
        let names: Vec<&str> = result
            .program
            .buffers()
            .iter()
            .map(|b| b.name.as_ref())
            .collect();
        assert_eq!(names, vec!["early", "late"]);
    }

    #[test]
    fn transform_preserves_already_sorted_program_unchanged() {
        let program = Program::wrapped(
            vec![buf("a", 0), buf("b", 3), buf("c", 7)],
            [1, 1, 1],
            entry(),
        );
        let result = BufferDeclSortPass::transform(program);
        assert!(
            !result.changed,
            "already-sorted Program must not report changed"
        );
    }

    #[test]
    fn transform_uses_name_tiebreaker_when_bindings_collide() {
        // Two buffers with binding 3; "alpha" < "beta" lexicographically, so
        // "alpha" wins the tie.
        let program = Program::wrapped(vec![buf("beta", 3), buf("alpha", 3)], [1, 1, 1], entry());
        let result = BufferDeclSortPass::transform(program);
        assert!(result.changed);
        let names: Vec<&str> = result
            .program
            .buffers()
            .iter()
            .map(|b| b.name.as_ref())
            .collect();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn transform_preserves_per_buffer_metadata() {
        let read_write = buf("rw", 5);
        let read_only = ro_buf("ro", 0);
        let program = Program::wrapped(
            vec![read_write.clone(), read_only.clone()],
            [1, 1, 1],
            entry(),
        );
        let result = BufferDeclSortPass::transform(program);
        let buffers = result.program.buffers();
        // The ro buffer (binding 0) should now be first; verify access mode
        // is preserved verbatim through the sort.
        assert_eq!(buffers[0].name.as_ref(), "ro");
        assert_eq!(buffers[0].access, BufferAccess::ReadOnly);
        assert_eq!(buffers[1].name.as_ref(), "rw");
        assert_eq!(buffers[1].access, BufferAccess::ReadWrite);
    }

    #[test]
    fn transform_preserves_entry_body_unchanged() {
        // The IR entry body must be untouched  -  the pass only touches the
        // BufferDecl table.
        let original_entry = entry();
        let program = Program::wrapped(
            vec![buf("late", 5), buf("early", 0)],
            [1, 1, 1],
            original_entry.clone(),
        );
        let result = BufferDeclSortPass::transform(program);
        assert_eq!(
            result.program.entry().len(),
            original_entry.len(),
            "entry body length must be preserved"
        );
    }

    #[test]
    fn transform_handles_empty_buffer_table() {
        // A Program with no buffers (rare but allowed for pure-compute
        // experiments) must not panic.
        let program = Program::wrapped(vec![], [1, 1, 1], vec![]);
        let result = BufferDeclSortPass::transform(program);
        assert!(!result.changed);
        assert_eq!(result.program.buffers().len(), 0);
    }

    #[test]
    fn transform_handles_single_buffer_no_op() {
        let program = Program::wrapped(vec![buf("only", 0)], [1, 1, 1], entry());
        let result = BufferDeclSortPass::transform(program);
        assert!(!result.changed);
    }

    #[test]
    fn transform_sorts_many_scrambled_bindings() {
        let bindings = [7, 2, 5, 0, 3, 9, 1, 8, 4, 6];
        let buffers: Vec<BufferDecl> = bindings
            .iter()
            .map(|b| buf(&format!("buf_{b}"), *b))
            .collect();
        let program = Program::wrapped(buffers, [1, 1, 1], entry());
        let result = BufferDeclSortPass::transform(program);
        assert!(result.changed);
        let sorted_bindings: Vec<u32> =
            result.program.buffers().iter().map(|b| b.binding).collect();
        assert_eq!(sorted_bindings, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn transform_is_idempotent() {
        // Running the pass twice must produce the same output as running
        // it once. This is the sort-stability invariant.
        let program = Program::wrapped(
            vec![buf("c", 5), buf("a", 1), buf("b", 3)],
            [1, 1, 1],
            entry(),
        );
        let once = BufferDeclSortPass::transform(program);
        let twice = BufferDeclSortPass::transform(Clone::clone(&once.program));
        assert!(once.changed);
        assert!(!twice.changed, "second run must report no change");
        let once_names: Vec<&str> = once
            .program
            .buffers()
            .iter()
            .map(|b| b.name.as_ref())
            .collect();
        let twice_names: Vec<&str> = twice
            .program
            .buffers()
            .iter()
            .map(|b| b.name.as_ref())
            .collect();
        assert_eq!(once_names, twice_names);
    }

    #[test]
    fn fingerprint_returns_stable_value() {
        let program = Program::wrapped(vec![buf("a", 0)], [1, 1, 1], entry());
        let fp1 = crate::optimizer::ProgramPass::fingerprint(&BufferDeclSortPass, &program);
        let fp2 = crate::optimizer::ProgramPass::fingerprint(&BufferDeclSortPass, &program);
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn already_sorted_with_tied_bindings_is_skipped() {
        // Tied bindings in name-sorted order must skip.
        let program = Program::wrapped(vec![buf("alpha", 3), buf("beta", 3)], [1, 1, 1], entry());
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&BufferDeclSortPass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn unsorted_with_tied_bindings_runs() {
        // Tied bindings in reverse name order must run.
        let program = Program::wrapped(vec![buf("beta", 3), buf("alpha", 3)], [1, 1, 1], entry());
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&BufferDeclSortPass, &program),
            PassAnalysis::RUN
        );
    }
}

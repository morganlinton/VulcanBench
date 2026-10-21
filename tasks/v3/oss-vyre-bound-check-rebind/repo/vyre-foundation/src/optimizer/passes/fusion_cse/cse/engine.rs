use super::CseCtx;
use crate::ir_inner::model::program::Program;

/// Run local value-numbering CSE over pure expressions, reusing an existing
/// scratchpad context.
///
/// The context is automatically [`clear`](CseCtx::clear)ed before use so that
/// callers can amortize allocation costs across multiple programs.
#[must_use]
#[inline]
#[expect(
    clippy::needless_pass_by_value,
    reason = "CSE consumes Program to reuse its allocation-preserving with_rewritten_entry path"
)]
pub fn cse_into(program: Program, ctx: &mut CseCtx) -> Program {
    ctx.clear();
    program.with_rewritten_entry(ctx.nodes(program.entry()))
}

// Thread-local CseCtx scratchpad. Each `cse(program)` call would otherwise
// alloc fresh hashmaps / vectors / arena that immediately get dropped on
// return; the context's `clear()` already preserves allocated capacity, so
// reusing one per thread converts the cost from O(N programs) → O(1)
// initial allocation.
thread_local! {
    static CSE_CTX: std::cell::RefCell<CseCtx> = std::cell::RefCell::new(CseCtx::default());
}

/// Run local value-numbering CSE over pure expressions.
#[must_use]
#[inline]
pub fn cse(program: Program) -> Program {
    CSE_CTX.with(|cell| cse_into(program, &mut cell.borrow_mut()))
}

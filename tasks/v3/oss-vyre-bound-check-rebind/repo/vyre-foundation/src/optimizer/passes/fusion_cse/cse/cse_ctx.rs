use super::expr_key::{ExprId, ExprKey};
use crate::ir::Ident;
use rustc_hash::FxHashMap;

#[derive(Clone)]
pub(crate) struct ScopedBinding {
    pub(super) name: Ident,
    pub(super) epoch: u64,
}

pub(crate) struct ScopeFrame {
    pub(super) undo_len: usize,
    pub(super) epoch: u64,
}

/// Mutable state for one common-subexpression-elimination traversal.
#[derive(Default)]
pub struct CseCtx {
    // PERF: uses `Ident` (Arc<str>) instead of `String`.
    // CSE runs on every expression in the program; switching from
    // String (heap alloc per entry) to Ident (atomic refcount bump)
    // eliminates O(n) allocations per CSE pass.
    pub(super) values: FxHashMap<ExprId, ScopedBinding>,
    pub(super) undo_log: Vec<(ExprId, Option<ScopedBinding>)>,
    pub(super) scope_stack: Vec<ScopeFrame>,
    /// Visibility epoch for side-effect invalidation inside a lexical scope.
    ///
    /// A store/barrier/atomic inside an `If`/`Block`/`Region` used to drain
    /// `values` into the undo log so leaving the scope could restore the
    /// parent table. That made every side effect O(number of visible CSE
    /// entries). Bumping this epoch hides all older bindings in O(1); actual
    /// map mutations are still restored by the undo log.
    pub(super) current_epoch: u64,
    pub(super) arena: Vec<ExprKey>,
    pub(super) deduplication: FxHashMap<ExprKey, ExprId>,
    /// Monotonic counter for uniquely keying subgroup-intrinsic expressions
    /// so CSE never merges two subgroup calls (they are lane-correlated
    /// and effectful). See `expr_key::ExprKey::Subgroup`.
    pub(super) subgroup_counter: u32,
    /// Test-only counter: number of times `intern_expr` actually built a key.
    pub(super) intern_calls: std::sync::atomic::AtomicUsize,
}

//! Lazy scope/name resolution for the C sema layer.
//!
//! The eager `c_sema_scope` builder walks the entire structural
//! parse tree and emits the Cat-A scope-resolution Program in one
//! pass. Every name lookup the program will perform is wired in
//! at build time. That's correct but pays the full scope-table
//! construction cost even when downstream stages only query a
//! handful of names (typedef-vs-identifier disambiguation often
//! needs only the names that appear in declarators).
//!
//! Lazy scope resolution defers the scope-table construction
//! until a name is queried. The first query for a name builds
//! its frame's scope table (one parent-to-root walk), caches the
//! result keyed by `(scope_frame_id, name)`, and serves later
//! queries from the cache.
//!
//! This is the host-side substrate. The actual GPU dispatch
//! still uses the eager `c_sema_scope` Program when the host
//! hasn't already resolved the name; the lazy host cache is the
//! short-circuit for the common case where the same name is
//! queried many times during semantic analysis.

use rustc_hash::FxHashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Stable id for a scope frame. The C parser issues fresh ids in
/// preorder as each block-introducing construct (function body,
/// compound statement, struct/union body) opens; the parent
/// chain is tracked separately by the parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ScopeFrameId(pub u32);

/// What kind of declaration a name resolves to. Mirrors the
/// `DECL_KIND_*` constants in `crate::parsing::c::sema::lookup`
/// but uses an enum so callers can `match` exhaustively.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DeclKind {
    /// Name has not been declared in any visible scope.
    Undeclared,
    /// Function definition (with body).
    Function,
    /// Function prototype (no body).
    FunctionDecl,
    /// Variable / parameter binding.
    Variable,
    /// Goto label.
    Label,
    /// Typedef alias.
    Typedef,
    /// Enum constant.
    EnumConstant,
}

/// Lazy scope/name resolution cache. Lookups are keyed by
/// `(ScopeFrameId, name)` and produce a `DeclKind`. Cheap to
/// clone (Arc-shared internal lock), Send + Sync.
#[derive(Clone)]
pub struct LazyScopeTable {
    inner: Arc<RwLock<LazyInner>>,
}

struct LazyInner {
    /// Parent-frame chain. Frame `i` has parent `parent[i]`
    /// (`None` at the translation-unit root).
    parent: Vec<Option<ScopeFrameId>>,
    /// Per-frame local declarations: `frames[i]` maps name → kind
    /// for declarations introduced *in that frame only* (no
    /// inherited entries).
    frames: Vec<FxHashMap<Arc<str>, DeclKind>>,
    /// Lookup cache: `name -> frame -> resolved DeclKind`.
    /// Populated on first query; cleared by `invalidate`.
    ///
    /// The name-major layout makes declaration invalidation proportional to
    /// the one declared name instead of the total number of cached frames.
    cache: FxHashMap<Arc<str>, FxHashMap<ScopeFrameId, DeclKind>>,
}

impl LazyScopeTable {
    /// Build an empty scope table with a single root frame
    /// (`ScopeFrameId(0)`, parent `None`). New frames are added
    /// via `push_frame`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(LazyInner {
                parent: vec![None],
                frames: vec![FxHashMap::default()],
                cache: FxHashMap::default(),
            })),
        }
    }

    /// Open a new scope frame whose parent is `parent`. Returns
    /// the new frame's id.
    #[must_use]
    pub fn push_frame(&self, parent: ScopeFrameId) -> ScopeFrameId {
        let mut inner = self.write_inner();
        let id = ScopeFrameId(inner.frames.len() as u32);
        inner.parent.push(Some(parent));
        inner.frames.push(FxHashMap::default());
        // Pushing a new frame does not invalidate the cache for
        // existing frames (their parent chain is unchanged); but
        // it could newly shadow a parent-chain name if `declare`
        // is called on this frame later. We do nothing here and
        // rely on `declare` to invalidate the cache for the
        // affected name when shadowing occurs.
        id
    }

    /// Declare `name` in `frame` with the given kind. Invalidates
    /// the cached entry for that `(frame, name)` and any
    /// descendant frame that previously inherited a different
    /// resolution for that name.
    pub fn declare(&self, frame: ScopeFrameId, name: &str, kind: DeclKind) -> bool {
        let mut inner = self.write_inner();
        let frame_idx = frame.0 as usize;
        if frame_idx >= inner.frames.len() {
            return false;
        }
        inner.frames[frame_idx].insert(Arc::from(name), kind);
        // Conservative cache invalidation: drop every cached entry for this
        // one name. A new declaration could shadow a previously-resolved
        // binding for any descendant frame, but unrelated names stay hot.
        inner.cache.remove(name);
        true
    }

    /// Look up `name` starting from `frame`. Walks the parent
    /// chain until a match is found or the root is reached.
    /// Cached after first lookup.
    #[must_use]
    pub fn lookup(&self, frame: ScopeFrameId, name: &str) -> DeclKind {
        {
            let inner = self.read_inner();
            if let Some(kind) = inner.cache.get(name).and_then(|cache| cache.get(&frame)) {
                return *kind;
            }
        }
        // Cache miss: walk parent chain.
        let kind = self.walk_chain(frame, name);
        let mut inner = self.write_inner();
        inner
            .cache
            .entry(Arc::from(name))
            .or_default()
            .insert(frame, kind);
        kind
    }

    fn walk_chain(&self, start: ScopeFrameId, name: &str) -> DeclKind {
        let inner = self.read_inner();
        let mut current = Some(start);
        while let Some(frame) = current {
            let frame_idx = frame.0 as usize;
            if frame_idx >= inner.frames.len() {
                break;
            }
            if let Some(&kind) = inner.frames[frame_idx].get(name) {
                return kind;
            }
            current = inner.parent[frame_idx];
        }
        DeclKind::Undeclared
    }

    /// Drop the entire lookup cache. Use when the underlying
    /// scope tree changes shape (frame parents reassigned, etc.).
    pub fn invalidate(&self) {
        let mut inner = self.write_inner();
        inner.cache.clear();
    }

    /// Number of frames in the table (including the root).
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.read_inner().frames.len()
    }

    /// Number of cached lookups. Useful for tests + telemetry.
    #[must_use]
    pub fn cache_size(&self) -> usize {
        self.read_inner().cache.values().map(FxHashMap::len).sum()
    }

    /// Root frame id (`ScopeFrameId(0)`).
    #[must_use]
    pub fn root() -> ScopeFrameId {
        ScopeFrameId(0)
    }

    fn read_inner(&self) -> RwLockReadGuard<'_, LazyInner> {
        // Fail closed on poison: `into_inner` would silently expose a scope
        // table left half-mutated by a panicking writer, so subsequent name
        // resolution would trust corrupt bindings with no signal. A poisoned
        // semantic scope table is unrecoverable; surface it loudly.
        self.inner
            .read()
            .unwrap_or_else(|_| panic!("lazy scope table read lock was poisoned"))
    }

    fn write_inner(&self) -> RwLockWriteGuard<'_, LazyInner> {
        self.inner
            .write()
            .unwrap_or_else(|_| panic!("lazy scope table write lock was poisoned"))
    }
}

impl Default for LazyScopeTable {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LazyScopeTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.read_inner();
        f.debug_struct("LazyScopeTable")
            .field("frame_count", &inner.frames.len())
            .field(
                "cache_size",
                &inner.cache.values().map(FxHashMap::len).sum::<usize>(),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty table: every lookup returns `Undeclared`.
    #[test]
    fn empty_table_undeclared() {
        let table = LazyScopeTable::new();
        assert_eq!(
            table.lookup(LazyScopeTable::root(), "x"),
            DeclKind::Undeclared
        );
        assert_eq!(table.cache_size(), 1, "miss is also cached");
    }

    /// Declare in root, lookup from root.
    #[test]
    fn root_declaration_resolves_from_root() {
        let table = LazyScopeTable::new();
        table.declare(LazyScopeTable::root(), "main", DeclKind::Function);
        assert_eq!(
            table.lookup(LazyScopeTable::root(), "main"),
            DeclKind::Function
        );
    }

    /// Lookup from child frame walks parent chain to root.
    #[test]
    fn child_frame_inherits_parent_declaration() {
        let table = LazyScopeTable::new();
        table.declare(LazyScopeTable::root(), "x", DeclKind::Variable);
        let child = table.push_frame(LazyScopeTable::root());
        assert_eq!(table.lookup(child, "x"), DeclKind::Variable);
    }

    /// Inner-frame declaration shadows outer-frame name.
    #[test]
    fn inner_frame_shadows_outer() {
        let table = LazyScopeTable::new();
        table.declare(LazyScopeTable::root(), "x", DeclKind::Function);
        let child = table.push_frame(LazyScopeTable::root());
        table.declare(child, "x", DeclKind::Variable);
        assert_eq!(table.lookup(child, "x"), DeclKind::Variable);
        assert_eq!(
            table.lookup(LazyScopeTable::root(), "x"),
            DeclKind::Function
        );
    }

    /// Cache hit on second lookup for the same `(frame, name)`.
    #[test]
    fn second_lookup_hits_cache() {
        let table = LazyScopeTable::new();
        table.declare(LazyScopeTable::root(), "x", DeclKind::Variable);
        assert_eq!(
            table.lookup(LazyScopeTable::root(), "x"),
            DeclKind::Variable
        );
        let initial = table.cache_size();
        assert_eq!(
            table.lookup(LazyScopeTable::root(), "x"),
            DeclKind::Variable
        );
        assert_eq!(
            table.cache_size(),
            initial,
            "second lookup must not grow cache"
        );
    }

    /// `declare` invalidates cached entries for the same name (so
    /// a shadow that arrives after a lookup re-resolves correctly
    /// on the next query).
    #[test]
    fn declare_invalidates_cached_entries_for_name() {
        let table = LazyScopeTable::new();
        table.declare(LazyScopeTable::root(), "x", DeclKind::Function);
        let child = table.push_frame(LazyScopeTable::root());
        // Prime the cache for the child's view of `x` (resolves
        // to Function via parent walk).
        assert_eq!(table.lookup(child, "x"), DeclKind::Function);
        // Add a shadowing declaration in the child.
        table.declare(child, "x", DeclKind::Variable);
        // Re-query: cache was invalidated for `x`, so we resolve
        // again and find the new shadow.
        assert_eq!(table.lookup(child, "x"), DeclKind::Variable);
    }

    /// Deep parent chain (10-deep) walks all the way to root for
    /// a name declared at the root.
    #[test]
    fn deep_chain_walks_to_root() {
        let table = LazyScopeTable::new();
        table.declare(LazyScopeTable::root(), "x", DeclKind::EnumConstant);
        let mut current = LazyScopeTable::root();
        for _ in 0..10 {
            current = table.push_frame(current);
        }
        assert_eq!(table.lookup(current, "x"), DeclKind::EnumConstant);
    }

    /// `invalidate` clears the entire cache.
    #[test]
    fn invalidate_clears_cache() {
        let table = LazyScopeTable::new();
        table.declare(LazyScopeTable::root(), "x", DeclKind::Variable);
        assert_eq!(
            table.lookup(LazyScopeTable::root(), "x"),
            DeclKind::Variable
        );
        assert!(table.cache_size() > 0);
        table.invalidate();
        assert_eq!(table.cache_size(), 0);
    }

    /// Poisoned semantic state is not recoverable: continuing after
    /// a panic can corrupt declaration classification.
    #[test]
    fn poisoned_scope_table_lock_is_not_silently_recovered() {
        let table = LazyScopeTable::new();
        let poisoned = table.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoned.write_inner();
            panic!("poison lazy scope table");
        })
        .join();

        let panic = std::panic::catch_unwind(|| {
            let _ = table.lookup(LazyScopeTable::root(), "x");
        })
        .expect_err("poisoned semantic scope table must panic instead of recovering");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&'static str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(message.contains("read lock was poisoned"), "{message}");
    }

    /// Send + Sync trait bounds (compile-time check).
    #[test]
    fn send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<LazyScopeTable>();
        assert_sync::<LazyScopeTable>();
    }
}

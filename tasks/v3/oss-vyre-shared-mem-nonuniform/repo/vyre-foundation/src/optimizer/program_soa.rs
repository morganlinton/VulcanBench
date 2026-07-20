//! ROADMAP A2  -  columnar / SoA fact view of a `Program` that hot
//! optimizer passes can opt into.
//!
//! This is the *additive* shape of the same A1 contract. The
//! existing `Node` enum tree stays as the canonical IR; this module
//! ships a parallel `ProgramFacts` representation that walks the
//! tree once and stores per-Node payload in flat `Vec` columns. A
//! pass that needs to ask repeated "where is name `x` bound?" or
//! "every site that touches buffer `b`?" or "every Let in preorder"
//! questions builds `ProgramFacts` once (one tree walk, O(N)) and
//! then answers each question in O(1) lookup or O(K) over the
//! reply, instead of paying a fresh tree walk per query.
//!
//! ## Why columnar
//!
//! The hot optimizer queries fall into a small fixed set:
//!   - "every Let target name in this scope" (DCE, A14, A18)
//!   - "every Var read site of name `x`" (DCE liveness, CSE)
//!   - "every site that reads / writes / RMW-atomics buffer `b`"
//!     (alias-aware load elision, atomic minimization, store
//!     forwarding, dead-store elimination)
//!   - "every Node of kind `K`" (any pass that wants to skip when
//!     no candidate node is present)
//!
//! Each of these is a sequential scan over a single column when the
//! IR is laid out as struct-of-arrays. The cache footprint of one
//! column is dramatically smaller than a tree walk that touches
//! every Node enum tag, every Box pointer, every Arc indirection,
//! and every recursive child sequence  -  and the SoA columns are
//! contiguous, so a SIMD-aware scan is straightforward.
//!
//! ## What this module is NOT
//!
//! - Not a replacement for the `Node` enum. The enum stays the
//!   ground truth; `ProgramFacts` is a derived view that gets
//!   rebuilt when the program shape changes.
//! - Not a mutation API. Hot passes still rewrite the `Node` tree.
//!   They only use `ProgramFacts` for fast read-side queries.
//! - Not the GPU-resident A10 representation. The columns live in
//!   host memory; a future GPU mirror is a separate module.

use crate::ir::{AtomicOp, Expr, Ident, Node, Program};
use crate::ir_inner::model::expr::GeneratorRef;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::OnceLock;

/// Stable preorder index into the `ProgramFacts` columns. Distinct
/// programs (or rebuilt fact tables for the same program) generally
/// hash to distinct sequences of indices; do not persist these
/// across `Program::with_rewritten_entry` calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct NodeIndex(pub u32);

/// Compact 1-byte tag mirroring every `Node` variant. The
/// discriminant order matches the order in
/// `vyre-foundation/src/ir_inner/model/generated.rs::Node`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum NodeKind {
    /// `Node::Let { name, value }`.
    Let,
    /// `Node::Assign { name, value }`.
    Assign,
    /// `Node::Store { buffer, index, value }`.
    Store,
    /// `Node::If { cond, then, otherwise }`.
    If,
    /// `Node::Loop { var, from, to, body }`.
    Loop,
    /// `Node::IndirectDispatch { count_buffer, .. }`.
    IndirectDispatch,
    /// `Node::AsyncLoad { source, destination, .. }`.
    AsyncLoad,
    /// `Node::AsyncStore { source, destination, .. }`.
    AsyncStore,
    /// `Node::AsyncWait { tag }`.
    AsyncWait,
    /// `Node::Trap { address, tag }`.
    Trap,
    /// `Node::Resume { tag }`.
    Resume,
    /// `Node::Return`.
    Return,
    /// `Node::Barrier { ordering }`.
    Barrier,
    /// `Node::Block(body)`.
    Block,
    /// `Node::Region { generator, source_region, body }`.
    Region,
    /// `Node::AllReduce { .. }`.
    AllReduce,
    /// `Node::AllGather { .. }`.
    AllGather,
    /// `Node::ReduceScatter { .. }`.
    ReduceScatter,
    /// `Node::Broadcast { .. }`.
    Broadcast,
    /// `Node::Opaque(extension)`.
    Opaque,
}

/// How a buffer was touched at a given node. Drives alias-aware
/// queries that need to distinguish reads from writes from atomics.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum BufferRefKind {
    /// `Expr::Load { buffer, .. }`, `Expr::BufLen { buffer }`, or
    /// any read-side reference inside another expression.
    Read,
    /// `Node::Store { buffer, .. }`, `Node::AsyncStore.destination`,
    /// or any write-side reference.
    Write,
    /// `Expr::Atomic { buffer, op, .. }`  -  both a read and a write
    /// in one operation, with explicit memory ordering.
    Atomic(AtomicOp),
    /// `Node::AsyncLoad.destination`  -  the destination of an async
    /// copy is treated as a write target.
    AsyncDestination,
    /// `Node::AsyncLoad.source` / `Node::AsyncStore.source`  -  async
    /// copy sources are read targets.
    AsyncSource,
    /// `Node::IndirectDispatch.count_buffer`  -  read-side reference
    /// to a dispatch-grid buffer.
    IndirectCount,
}

/// One row per `Node::Region` observed during the build walk  -
/// the diagnostic / source-correlation metadata that the
/// `Region` enum variant inlines. ROADMAP A3  -  passes that don't
/// care about source provenance can ignore this column entirely;
/// passes that do care (diagnostics, region-inlining, region
/// identity tracking) iterate the column once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionMeta {
    /// `NodeIndex` of the `Node::Region` within the `SoA` fact table.
    pub node: NodeIndex,
    /// `Region.generator`  -  the op id / pass / extension that
    /// produced this region, used by diagnostics to attribute
    /// errors back to their source.
    pub generator: Ident,
    /// `Region.source_region`  -  the optional generator ref that
    /// links a derived region back to the original source span.
    pub source_region: Option<GeneratorRef>,
}

/// Columnar fact view of a `Program`. Construct via
/// `ProgramFacts::build(&program)` and query through the helpers.
#[derive(Debug, Default)]
pub struct ProgramFacts {
    kinds: Vec<NodeKind>,
    parent: Vec<Option<NodeIndex>>,
    /// Bitset of every `NodeKind` discriminant observed during
    /// `build`. Populated alongside `kinds` so `has_kind` and
    /// `has_any_kind_in_mask` are O(1) bit tests instead of an O(N)
    /// scan of `kinds`. Pass-`analyze_impl` predicates (which run
    /// before every transform on every iteration) hit this in the
    /// hot pipeline.
    kinds_present: u32,
    lets: Vec<(NodeIndex, Ident)>,
    assigns: Vec<(NodeIndex, Ident)>,
    loop_vars: Vec<(NodeIndex, Ident)>,
    var_reads: Vec<(NodeIndex, Ident)>,
    buffer_refs: Vec<(NodeIndex, Ident, BufferRefKind)>,
    regions: Vec<RegionMeta>,
    let_index: OnceLock<FxHashMap<Ident, Vec<NodeIndex>>>,
    assign_index: OnceLock<FxHashMap<Ident, Vec<NodeIndex>>>,
    var_read_index: OnceLock<FxHashMap<Ident, Vec<NodeIndex>>>,
    buffer_index: OnceLock<FxHashMap<Ident, Vec<(NodeIndex, BufferRefKind)>>>,
    region_index_by_node: OnceLock<FxHashMap<NodeIndex, usize>>,
    region_index_by_generator: OnceLock<FxHashMap<Ident, Vec<usize>>>,
}

/// Bit position of a `NodeKind` inside the `ProgramFacts::kinds_present`
/// bitset. Returned as a `u32` so callers can `1 << kind_bit(k)` directly.
#[must_use]
#[inline]
pub const fn kind_bit(kind: NodeKind) -> u32 {
    kind as u32
}

/// `1 << kind_bit(k)` mask for one [`NodeKind`].
#[must_use]
#[inline]
pub const fn kind_mask(kind: NodeKind) -> u32 {
    1u32 << (kind as u32)
}

thread_local! {
    /// Last (program-fingerprint, ProgramFacts) pair the current thread
    /// computed. ProgramFacts builds are deterministic in `program` and
    /// the scheduler runs sequentially against the SAME program for a
    /// burst of passes (analyze + transform per pass, multiple passes
    /// per iteration). A one-entry thread-local cache keyed by the
    /// program's stable fingerprint collapses 6+ redundant rebuilds
    /// per scheduler iteration into a single build.
    ///
    /// Rc rather than Arc  -  the cache slot only ever hands references
    /// back to the same thread that owns it, so we don't need cross-
    /// thread synchronization for the cached payload.
    static FACTS_CACHE: std::cell::RefCell<Option<([u8; 32], std::rc::Rc<ProgramFacts>)>> =
        const { std::cell::RefCell::new(None) };
}

impl ProgramFacts {
    /// Return a thread-local cached [`ProgramFacts`] for `program`,
    /// rebuilding only when the program's stable fingerprint differs
    /// from the last build on this thread.
    ///
    /// Use this in pass `analyze_impl` / `transform` paths instead of
    /// calling [`ProgramFacts::build`] directly: the scheduler hits
    /// the same `Program` repeatedly within one iteration (analyze
    /// then transform; multiple consecutive passes that all need
    /// facts) and the cache turns those repeats into refcount bumps.
    ///
    /// First-call cost is identical to `build`. Subsequent same-program
    /// calls on the same thread cost one `program.fingerprint()` (already
    /// OnceLock-cached) plus an Rc clone.
    #[must_use]
    pub fn build_cached(program: &Program) -> std::rc::Rc<ProgramFacts> {
        let fp = program.fingerprint();
        FACTS_CACHE.with(|cell| {
            let mut slot = cell.borrow_mut();
            if let Some((cached_fp, cached)) = slot.as_ref() {
                if cached_fp == &fp {
                    return std::rc::Rc::clone(cached);
                }
            }
            let facts = std::rc::Rc::new(ProgramFacts::build(program));
            *slot = Some((fp, std::rc::Rc::clone(&facts)));
            facts
        })
    }
}

impl ProgramFacts {
    /// Walk the program's entry tree once in preorder and populate
    /// every column. The lookup indices are built lazily on the
    /// first call to `let_sites_of` / `var_read_sites_of` /
    /// `buffer_refs_of`.
    #[must_use]
    pub fn build(program: &Program) -> Self {
        match Self::try_build(program) {
            Ok(facts) => facts,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "ProgramFacts::build failed; use try_build on release paths to handle allocation pressure explicitly"
                );
                Self::default()
            }
        }
    }

    /// Fallible version of [`ProgramFacts::build`] for release paths that must
    /// surface allocation pressure instead of panicking during optimizer
    /// analysis.
    ///
    /// # Errors
    ///
    /// Returns an actionable message when a ProgramFacts column cannot reserve
    /// enough storage for the program's cached node/region counts.
    pub fn try_build(program: &Program) -> Result<Self, String> {
        // Pre-size the columnar Vec storage off the OnceLock-cached
        // node count so the build walk fills already-allocated
        // capacity instead of grow-by-doubling each column. The
        // counts (kinds, parent, lets, assigns, etc.) are bounded by
        // node_count; non-Let/Assign columns over-reserve, but the
        // single allocation is cheaper than 6+ doublings on a 1000-
        // node entry tree.
        let stats = program.stats();
        let node_count = stats.node_count;
        let mut facts = Self {
            kinds: Vec::default(),
            parent: Vec::default(),
            kinds_present: 0,
            lets: Vec::default(),
            assigns: Vec::default(),
            loop_vars: Vec::default(),
            var_reads: Vec::default(),
            buffer_refs: Vec::default(),
            regions: Vec::default(),
            let_index: OnceLock::new(),
            assign_index: OnceLock::new(),
            var_read_index: OnceLock::new(),
            buffer_index: OnceLock::new(),
            region_index_by_node: OnceLock::new(),
            region_index_by_generator: OnceLock::new(),
        };
        reserve_program_fact_columns(&mut facts, node_count, stats.region_count as usize)?;
        for node in program.entry() {
            walk_node(node, None, &mut facts);
        }
        Ok(facts)
    }

    /// Total number of nodes (preorder count) in the program tree.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.kinds.len()
    }

    /// `NodeKind` at index `idx`. Panics if `idx` is out of range  -
    /// callers should always pull indices from this same fact table.
    #[must_use]
    pub fn kind_at(&self, idx: NodeIndex) -> NodeKind {
        self.kinds[idx.0 as usize]
    }

    /// Parent node index, or `None` if `idx` is a root entry-level
    /// sibling.
    #[must_use]
    pub fn parent_of(&self, idx: NodeIndex) -> Option<NodeIndex> {
        self.parent[idx.0 as usize]
    }

    /// `true` iff `node` is inside the subtree rooted at `ancestor`.
    /// A node is considered inside itself. This is O(depth) and uses
    /// the parent column, avoiding a recursive tree walk for scoped
    /// optimizer queries.
    #[must_use]
    pub fn is_descendant_of(&self, node: NodeIndex, ancestor: NodeIndex) -> bool {
        let mut current = Some(node);
        while let Some(idx) = current {
            if idx == ancestor {
                return true;
            }
            current = self.parent_of(idx);
        }
        false
    }

    /// Iterate every `(NodeIndex, NodeKind)` in preorder.
    pub fn iter_nodes(&self) -> impl Iterator<Item = (NodeIndex, NodeKind)> + '_ {
        self.kinds.iter().copied().enumerate().map(|(i, kind)| {
            (
                NodeIndex(u32::try_from(i).map_or(u32::MAX, |value| value)),
                kind,
            )
        })
    }

    /// Iterate optimizer-semantic nodes in preorder, skipping
    /// `NodeKind::Region`. Region generator/source payload lives in
    /// the [`RegionMeta`] side table, so passes that only care about
    /// computation can scan this view without matching through debug
    /// wrappers.
    pub fn iter_regionless_nodes(&self) -> impl Iterator<Item = (NodeIndex, NodeKind)> + '_ {
        self.iter_nodes()
            .filter(|(_, kind)| *kind != NodeKind::Region)
    }

    /// Parent in the optimizer-semantic tree, skipping any enclosing
    /// `NodeKind::Region` wrappers. This lets passes treat Region as
    /// provenance metadata while preserving the canonical wire tree
    /// for diagnostics and serialization.
    #[must_use]
    pub fn regionless_parent_of(&self, idx: NodeIndex) -> Option<NodeIndex> {
        let mut parent = self.parent_of(idx);
        while let Some(candidate) = parent {
            if self.kind_at(candidate) != NodeKind::Region {
                return Some(candidate);
            }
            parent = self.parent_of(candidate);
        }
        None
    }

    /// `true` iff at least one node has the given kind. O(1) bit
    /// test against the cached `kinds_present` mask populated during
    /// `build`.
    #[must_use]
    #[inline]
    pub fn has_kind(&self, kind: NodeKind) -> bool {
        (self.kinds_present & kind_mask(kind)) != 0
    }

    /// `true` iff at least one node's kind is in `mask`. O(1).
    /// Compose with [`kind_mask`] when checking several kinds at
    /// once: `facts.has_any_kind_in_mask(kind_mask(NodeKind::Loop) | kind_mask(NodeKind::If))`.
    #[must_use]
    #[inline]
    pub fn has_any_kind_in_mask(&self, mask: u32) -> bool {
        (self.kinds_present & mask) != 0
    }

    /// Raw kind-presence bitset. Exposed so passes that need to
    /// short-circuit on multiple distinct kinds can grab the mask
    /// once and AND/OR/XOR locally without going through the
    /// helpers per-kind.
    #[must_use]
    #[inline]
    pub fn kinds_present(&self) -> u32 {
        self.kinds_present
    }

    /// Every `(NodeIndex, name)` where `Node::Let { name, .. }` was
    /// observed. The order is preorder.
    #[must_use]
    pub fn lets(&self) -> &[(NodeIndex, Ident)] {
        &self.lets
    }

    /// Every `(NodeIndex, name)` where `Node::Assign { name, .. }`
    /// was observed.
    #[must_use]
    pub fn assigns(&self) -> &[(NodeIndex, Ident)] {
        &self.assigns
    }

    /// Every `(NodeIndex, name)` where `Node::Loop { var, .. }`
    /// declared an induction variable.
    #[must_use]
    pub fn loop_vars(&self) -> &[(NodeIndex, Ident)] {
        &self.loop_vars
    }

    /// Every `(NodeIndex, name)` where `Expr::Var(name)` appears
    /// (including inside compound expressions).
    #[must_use]
    pub fn var_reads(&self) -> &[(NodeIndex, Ident)] {
        &self.var_reads
    }

    /// Every `(NodeIndex, buffer, kind)` where a buffer was touched.
    #[must_use]
    pub fn buffer_refs(&self) -> &[(NodeIndex, Ident, BufferRefKind)] {
        &self.buffer_refs
    }

    /// All node indices where `Let(name, _)` was observed.
    /// Builds the lookup index on first call; subsequent calls are
    /// O(1) hash lookup.
    #[must_use]
    pub fn let_sites_of(&self, name: &str) -> &[NodeIndex] {
        let map = self.let_index.get_or_init(|| build_index(&self.lets));
        map.get(name).map_or(&[], Vec::as_slice)
    }

    /// All node indices where `Assign(name, _)` was observed.
    #[must_use]
    pub fn assign_sites_of(&self, name: &str) -> &[NodeIndex] {
        let map = self.assign_index.get_or_init(|| build_index(&self.assigns));
        map.get(name).map_or(&[], Vec::as_slice)
    }

    /// All node indices that read `Expr::Var(name)`.
    #[must_use]
    pub fn var_read_sites_of(&self, name: &str) -> &[NodeIndex] {
        let map = self
            .var_read_index
            .get_or_init(|| build_index(&self.var_reads));
        map.get(name).map_or(&[], Vec::as_slice)
    }

    /// Every site that touches buffer `name`, paired with the kind
    /// of touch (Read / Write / Atomic / `AsyncDestination` /
    /// `AsyncSource` / `IndirectCount`).
    #[must_use]
    pub fn buffer_refs_of(&self, name: &str) -> &[(NodeIndex, BufferRefKind)] {
        let map = self.buffer_index.get_or_init(|| {
            let mut out: FxHashMap<Ident, Vec<(NodeIndex, BufferRefKind)>> = FxHashMap::default();
            for (idx, buffer, kind) in &self.buffer_refs {
                out.entry(buffer.duplicate_handle())
                    .or_default()
                    .push((*idx, *kind));
            }
            out
        });
        map.get(name).map_or(&[], Vec::as_slice)
    }

    /// Every `Node::Region` observed during the build walk, with
    /// its diagnostic `generator` ident and optional `source_region`
    /// ref. ROADMAP A3  -  the side-table half of "treat Region /
    /// source metadata as side tables during optimization, restore
    /// for diagnostics."
    #[must_use]
    pub fn regions(&self) -> &[RegionMeta] {
        &self.regions
    }

    /// Look up the `RegionMeta` for the `Node::Region` at `idx`,
    /// or `None` if `idx` is not a Region or no Region was recorded
    /// at that index. O(1) hash lookup once the index is built.
    #[must_use]
    pub fn region_at(&self, idx: NodeIndex) -> Option<&RegionMeta> {
        let map = self.region_index_by_node.get_or_init(|| {
            let mut out: FxHashMap<NodeIndex, usize> = FxHashMap::default();
            for (i, meta) in self.regions.iter().enumerate() {
                out.insert(meta.node, i);
            }
            out
        });
        map.get(&idx).and_then(|&i| self.regions.get(i))
    }

    /// All `Node::Region` sites whose `generator` ident equals the
    /// argument. O(1) hash lookup once the index is built.
    pub fn regions_by_generator(&self, generator: &str) -> impl Iterator<Item = &RegionMeta> + '_ {
        let map = self.region_index_by_generator.get_or_init(|| {
            let mut out: FxHashMap<Ident, Vec<usize>> = FxHashMap::default();
            for (i, meta) in self.regions.iter().enumerate() {
                out.entry(meta.generator.duplicate_handle())
                    .or_default()
                    .push(i);
            }
            out
        });
        map.get(generator)
            .map_or(&[] as &[usize], std::vec::Vec::as_slice)
            .iter()
            .filter_map(move |&i| self.regions.get(i))
    }

    /// Convenience: `true` iff `name` is rebound anywhere  -  either
    /// as a `Let` shadow, an `Assign`, or a `Loop` induction
    /// variable. Used by passes that want to check "is this name
    /// stable across the whole program?" without writing the same
    /// scan three times.
    #[must_use]
    pub fn is_name_rebound(&self, name: &str) -> bool {
        let lets = self.let_sites_of(name);
        if lets.len() > 1 {
            return true;
        }
        if !self.assign_sites_of(name).is_empty() {
            return true;
        }
        self.loop_vars.iter().any(|(_, var)| var.as_str() == name)
    }

    /// ROADMAP A12  -  points-to fact: `true` iff `buf_a` and `buf_b`
    /// can be proven to refer to disjoint memory.
    ///
    /// Soundness: in vyre's IR every `BufferDecl` is a distinct
    /// named allocation. Two distinct buffer names declared in the
    /// program's buffer table are guaranteed by construction not to
    /// alias (the runtime allocates a fresh region per `BufferDecl`).
    /// The same name aliases itself trivially. The fact lets
    /// alias-aware passes (load elision, store-to-load forwarding,
    /// dead-store elimination) assume non-aliasing without paying
    /// for the full downstream points-to analysis on the unique slice.
    ///
    /// Returns `true` iff `buf_a != buf_b` AND both names appear in
    /// the program's `buffer_refs` column (so they're real declared
    /// buffers, not phantom or extension-defined names).
    #[must_use]
    pub fn buffers_provably_distinct(&self, buf_a: &str, buf_b: &str) -> bool {
        if buf_a == buf_b {
            return false;
        }
        let a_seen = self.buffer_refs.iter().any(|(_, b, _)| b.as_str() == buf_a);
        let b_seen = self.buffer_refs.iter().any(|(_, b, _)| b.as_str() == buf_b);
        a_seen && b_seen
    }

    /// ROADMAP A13  -  escape fact: `true` iff `name`'s contents are
    /// observable outside this kernel's execution.
    ///
    /// A buffer escapes the kernel scope when it appears as:
    ///   - the destination of any `Node::Store`, `Node::AsyncStore`,
    ///     `Node::AsyncLoad` (the host reads back the destination),
    ///   - the index target of any `Expr::Atomic` (atomic results are
    ///     visible to other workgroups + the host),
    ///   - the count buffer of any `Node::IndirectDispatch` (the
    ///     value is consumed by the dispatch grid).
    ///
    /// Buffers that are READ ONLY (no Write / Atomic / `AsyncDestination`
    /// / `IndirectCount` in the `buffer_refs` column) do not escape  -  their
    /// contents are an input the host produced, not a kernel-local
    /// scratch the host needs to read back.
    ///
    /// Used by scratch-reuse passes (megakernel arms can recycle the
    /// storage of a non-escaping buffer for the next arm).
    #[must_use]
    pub fn buffer_escapes(&self, name: &str) -> bool {
        self.buffer_refs.iter().any(|(_, b, kind)| {
            b.as_str() == name
                && matches!(
                    kind,
                    BufferRefKind::Write
                        | BufferRefKind::Atomic(_)
                        | BufferRefKind::AsyncDestination
                        | BufferRefKind::IndirectCount
                )
        })
    }

    /// All buffer names that escape the kernel scope (helper for
    /// scratch-reuse passes that want to enumerate the escaping
    /// set in one go).
    #[must_use]
    pub fn escaping_buffers(&self) -> FxHashSet<Ident> {
        let mut out: FxHashSet<Ident> = FxHashSet::default();
        for (_, name, kind) in &self.buffer_refs {
            if matches!(
                kind,
                BufferRefKind::Write
                    | BufferRefKind::Atomic(_)
                    | BufferRefKind::AsyncDestination
                    | BufferRefKind::IndirectCount
            ) {
                out.insert(name.duplicate_handle());
            }
        }
        out
    }
}

fn build_index(rows: &[(NodeIndex, Ident)]) -> FxHashMap<Ident, Vec<NodeIndex>> {
    let mut out: FxHashMap<Ident, Vec<NodeIndex>> = FxHashMap::default();
    for (idx, name) in rows {
        out.entry(name.duplicate_handle()).or_default().push(*idx);
    }
    out
}

fn reserve_program_fact_columns(
    facts: &mut ProgramFacts,
    node_count: usize,
    region_count: usize,
) -> Result<(), String> {
    reserve_fact_vec(&mut facts.kinds, node_count, "kind column")?;
    reserve_fact_vec(&mut facts.parent, node_count, "parent column")?;
    reserve_fact_vec(&mut facts.lets, node_count / 4, "let column")?;
    reserve_fact_vec(&mut facts.assigns, node_count / 8, "assign column")?;
    reserve_fact_vec(&mut facts.loop_vars, node_count / 16, "loop-var column")?;
    reserve_fact_vec(&mut facts.var_reads, node_count, "var-read column")?;
    reserve_fact_vec(&mut facts.buffer_refs, node_count / 2, "buffer-ref column")?;
    reserve_fact_vec(&mut facts.regions, region_count, "region metadata column")?;
    Ok(())
}

fn reserve_fact_vec<T>(
    vec: &mut Vec<T>,
    target_capacity: usize,
    label: &'static str,
) -> Result<(), String> {
    crate::allocation::try_reserve_vec_to_capacity(vec, target_capacity).map_err(|source| {
        format!(
            "ProgramFacts {label} reservation failed for {target_capacity} item(s): {source}. Fix: shard the optimizer input or rebuild facts from a smaller program slice."
        )
    })
}

fn record_node(facts: &mut ProgramFacts, kind: NodeKind, parent: Option<NodeIndex>) -> NodeIndex {
    let idx = NodeIndex(u32::try_from(facts.kinds.len()).map_or(u32::MAX, |value| value));
    facts.kinds.push(kind);
    facts.parent.push(parent);
    // Set the kind-presence bit. `kind as u32` is the discriminant
    // (NodeKind has 16 variants, all fit in a u32). The optimizer
    // uses kinds_present for O(1) `has_kind` queries instead of
    // scanning the kinds column.
    facts.kinds_present |= kind_mask(kind);
    idx
}

#[expect(
    clippy::too_many_lines,
    reason = "SoA extraction keeps the Node variant-to-column mapping auditable in one walk"
)]
fn walk_node(node: &Node, parent: Option<NodeIndex>, facts: &mut ProgramFacts) {
    match node {
        Node::Let { name, value } => {
            let idx = record_node(facts, NodeKind::Let, parent);
            facts.lets.push((idx, name.duplicate_handle()));
            walk_expr(value, idx, facts);
        }
        Node::Assign { name, value } => {
            let idx = record_node(facts, NodeKind::Assign, parent);
            facts.assigns.push((idx, name.duplicate_handle()));
            walk_expr(value, idx, facts);
        }
        Node::Store {
            buffer,
            index,
            value,
        } => {
            let idx = record_node(facts, NodeKind::Store, parent);
            facts
                .buffer_refs
                .push((idx, buffer.duplicate_handle(), BufferRefKind::Write));
            walk_expr(index, idx, facts);
            walk_expr(value, idx, facts);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            let idx = record_node(facts, NodeKind::If, parent);
            walk_expr(cond, idx, facts);
            for n in then {
                walk_node(n, Some(idx), facts);
            }
            for n in otherwise {
                walk_node(n, Some(idx), facts);
            }
        }
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            let idx = record_node(facts, NodeKind::Loop, parent);
            facts.loop_vars.push((idx, var.duplicate_handle()));
            walk_expr(from, idx, facts);
            walk_expr(to, idx, facts);
            for n in body {
                walk_node(n, Some(idx), facts);
            }
        }
        Node::IndirectDispatch { count_buffer, .. } => {
            let idx = record_node(facts, NodeKind::IndirectDispatch, parent);
            facts.buffer_refs.push((
                idx,
                count_buffer.duplicate_handle(),
                BufferRefKind::IndirectCount,
            ));
        }
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            ..
        } => {
            let idx = record_node(facts, NodeKind::AsyncLoad, parent);
            facts
                .buffer_refs
                .push((idx, source.duplicate_handle(), BufferRefKind::AsyncSource));
            facts.buffer_refs.push((
                idx,
                destination.duplicate_handle(),
                BufferRefKind::AsyncDestination,
            ));
            walk_expr(offset, idx, facts);
            walk_expr(size, idx, facts);
        }
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            ..
        } => {
            let idx = record_node(facts, NodeKind::AsyncStore, parent);
            facts
                .buffer_refs
                .push((idx, source.duplicate_handle(), BufferRefKind::AsyncSource));
            facts
                .buffer_refs
                .push((idx, destination.duplicate_handle(), BufferRefKind::Write));
            walk_expr(offset, idx, facts);
            walk_expr(size, idx, facts);
        }
        Node::AsyncWait { .. } => {
            record_node(facts, NodeKind::AsyncWait, parent);
        }
        Node::Trap { address, .. } => {
            let idx = record_node(facts, NodeKind::Trap, parent);
            walk_expr(address, idx, facts);
        }
        Node::Resume { .. } => {
            record_node(facts, NodeKind::Resume, parent);
        }
        Node::Return => {
            record_node(facts, NodeKind::Return, parent);
        }
        Node::Barrier { .. } => {
            record_node(facts, NodeKind::Barrier, parent);
        }
        Node::Block(body) => {
            let idx = record_node(facts, NodeKind::Block, parent);
            for n in body {
                walk_node(n, Some(idx), facts);
            }
        }
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let idx = record_node(facts, NodeKind::Region, parent);
            facts.regions.push(RegionMeta {
                node: idx,
                generator: generator.duplicate_handle(),
                source_region: source_region.clone(),
            });
            for n in body.iter() {
                walk_node(n, Some(idx), facts);
            }
        }
        Node::AllReduce { buffer, .. } => {
            let idx = record_node(facts, NodeKind::AllReduce, parent);
            facts
                .buffer_refs
                .push((idx, buffer.duplicate_handle(), BufferRefKind::Write));
        }
        Node::AllGather { input, output, .. } => {
            let idx = record_node(facts, NodeKind::AllGather, parent);
            facts
                .buffer_refs
                .push((idx, input.duplicate_handle(), BufferRefKind::Read));
            facts
                .buffer_refs
                .push((idx, output.duplicate_handle(), BufferRefKind::Write));
        }
        Node::ReduceScatter { input, output, .. } => {
            let idx = record_node(facts, NodeKind::ReduceScatter, parent);
            facts
                .buffer_refs
                .push((idx, input.duplicate_handle(), BufferRefKind::Read));
            facts
                .buffer_refs
                .push((idx, output.duplicate_handle(), BufferRefKind::Write));
        }
        Node::Broadcast { buffer, .. } => {
            let idx = record_node(facts, NodeKind::Broadcast, parent);
            facts
                .buffer_refs
                .push((idx, buffer.duplicate_handle(), BufferRefKind::Write));
        }
        Node::Opaque(_) => {
            record_node(facts, NodeKind::Opaque, parent);
        }
    }
}

fn walk_expr(expr: &Expr, owning_node: NodeIndex, facts: &mut ProgramFacts) {
    match expr {
        Expr::Var(name) => {
            facts.var_reads.push((owning_node, name.duplicate_handle()));
        }
        Expr::Load { buffer, index } => {
            facts
                .buffer_refs
                .push((owning_node, buffer.duplicate_handle(), BufferRefKind::Read));
            walk_expr(index, owning_node, facts);
        }
        Expr::BufLen { buffer } => {
            facts
                .buffer_refs
                .push((owning_node, buffer.duplicate_handle(), BufferRefKind::Read));
        }
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ..
        } => {
            facts.buffer_refs.push((
                owning_node,
                buffer.duplicate_handle(),
                BufferRefKind::Atomic(*op),
            ));
            walk_expr(index, owning_node, facts);
            if let Some(e) = expected.as_deref() {
                walk_expr(e, owning_node, facts);
            }
            walk_expr(value, owning_node, facts);
        }
        Expr::BinOp { left, right, .. } => {
            walk_expr(left, owning_node, facts);
            walk_expr(right, owning_node, facts);
        }
        Expr::UnOp { operand, .. } => walk_expr(operand, owning_node, facts),
        Expr::Call { args, .. } => {
            for arg in args {
                walk_expr(arg, owning_node, facts);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            walk_expr(cond, owning_node, facts);
            walk_expr(true_val, owning_node, facts);
            walk_expr(false_val, owning_node, facts);
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => {
            walk_expr(value, owning_node, facts);
        }
        Expr::Fma { a, b, c } => {
            walk_expr(a, owning_node, facts);
            walk_expr(b, owning_node, facts);
            walk_expr(c, owning_node, facts);
        }
        Expr::SubgroupBallot { cond } => walk_expr(cond, owning_node, facts),
        Expr::SubgroupShuffle { value, lane } => {
            walk_expr(value, owning_node, facts);
            walk_expr(lane, owning_node, facts);
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};
    use crate::runtime::memory_model::MemoryOrdering;

    fn buf(name: &str) -> BufferDecl {
        BufferDecl::storage(name, 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf("a"), buf("b")], [1, 1, 1], entry)
    }

    #[test]
    fn program_facts_build_exposes_fallible_reservation_path() {
        let production = include_str!("program_soa.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: ProgramFacts production section should precede tests");

        assert!(
            production.contains("pub fn try_build"),
            "Fix: ProgramFacts must expose a fallible build path for release optimizers."
        );
        assert!(
            production.contains("reserve_program_fact_columns"),
            "Fix: ProgramFacts column storage should reserve through one shared helper."
        );
        assert!(
            !production.contains("Vec::with_capacity"),
            "Fix: ProgramFacts production code must not use infallible Vec capacity constructors."
        );
        assert!(
            !production.contains(".expect("),
            "Fix: ProgramFacts production compatibility builders must not panic on allocation pressure."
        );
        assert!(
            !production.contains("with_capacity_and_hasher"),
            "Fix: ProgramFacts production indexes must not use infallible hash-map capacity constructors."
        );

        let facts = ProgramFacts::try_build(&program(vec![Node::let_bind("x", Expr::u32(1))]))
            .expect("Fix: small ProgramFacts build should reserve successfully");
        assert_eq!(facts.let_sites_of("x").len(), 1);
    }

    /// `build` returns an empty fact table for an entry tree that
    /// has no user nodes (the wrapping Region itself counts as one
    /// node and is recorded).
    #[test]
    fn empty_program_has_only_region_node() {
        let facts = ProgramFacts::build(&program(Vec::default()));
        assert_eq!(facts.node_count(), 1);
        assert_eq!(facts.kind_at(NodeIndex(0)), NodeKind::Region);
        assert!(facts.lets().is_empty());
        assert!(facts.var_reads().is_empty());
        assert!(facts.buffer_refs().is_empty());
    }

    /// Empty entry tree has the wrapping Region in `kinds_present`
    /// and nothing else  -  no Lets, no Loops, no Stores.
    #[test]
    fn kinds_present_bitset_starts_empty_then_records_each_kind() {
        let facts = ProgramFacts::build(&program(Vec::default()));
        // Wrapping Region IS recorded by `build`.
        assert!(facts.has_kind(NodeKind::Region));
        // But nothing else is.
        assert!(!facts.has_kind(NodeKind::Let));
        assert!(!facts.has_kind(NodeKind::Loop));
        assert!(!facts.has_kind(NodeKind::Store));
        assert!(!facts.has_kind(NodeKind::If));
        assert!(!facts.has_kind(NodeKind::Barrier));
    }

    /// Each observed Node sets exactly its bit in `kinds_present`.
    #[test]
    fn kinds_present_records_every_observed_kind() {
        let facts = ProgramFacts::build(&program(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::if_then(Expr::var("x"), vec![Node::Return]),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(4),
                vec![Node::Block(Vec::default())],
            ),
            Node::Barrier {
                ordering: MemoryOrdering::SeqCst,
            },
        ]));
        assert!(facts.has_kind(NodeKind::Let));
        assert!(facts.has_kind(NodeKind::Store));
        assert!(facts.has_kind(NodeKind::If));
        assert!(facts.has_kind(NodeKind::Return));
        assert!(facts.has_kind(NodeKind::Loop));
        assert!(facts.has_kind(NodeKind::Block));
        assert!(facts.has_kind(NodeKind::Barrier));
        assert!(facts.has_kind(NodeKind::Region));
        // Kinds we never produced must remain false.
        assert!(!facts.has_kind(NodeKind::Assign));
        assert!(!facts.has_kind(NodeKind::AsyncLoad));
        assert!(!facts.has_kind(NodeKind::AsyncStore));
        assert!(!facts.has_kind(NodeKind::IndirectDispatch));
        assert!(!facts.has_kind(NodeKind::Trap));
    }

    /// `has_any_kind_in_mask` ORs across the kinds_present bitset:
    /// a program with a Let alone matches a (Let | Loop) mask and
    /// not a (Loop | Trap) mask.
    #[test]
    fn has_any_kind_in_mask_is_or_across_observed_kinds() {
        let facts = ProgramFacts::build(&program(vec![Node::let_bind("x", Expr::u32(1))]));
        assert!(facts.has_any_kind_in_mask(kind_mask(NodeKind::Let)));
        assert!(facts.has_any_kind_in_mask(kind_mask(NodeKind::Let) | kind_mask(NodeKind::Loop)));
        assert!(!facts.has_any_kind_in_mask(kind_mask(NodeKind::Loop) | kind_mask(NodeKind::Trap)));
        assert_eq!(facts.has_kind(NodeKind::Let), true);
        assert_eq!(facts.has_kind(NodeKind::Loop), false);
    }

    /// `kinds_present()` mask exposes the raw bitset for callers that
    /// want to short-circuit on multiple kinds with a single AND.
    #[test]
    fn kinds_present_mask_round_trips_through_kind_mask_helper() {
        let facts = ProgramFacts::build(&program(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::Return,
        ]));
        let mask = facts.kinds_present();
        // Exactly the bits we expect: Let, Return, Region (the
        // wrapping Region is always recorded).
        let expected =
            kind_mask(NodeKind::Let) | kind_mask(NodeKind::Return) | kind_mask(NodeKind::Region);
        assert_eq!(mask, expected);
    }

    /// Lets are recorded in preorder with the right name.
    #[test]
    fn let_sites_recorded_in_preorder() {
        let facts = ProgramFacts::build(&program(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::let_bind("y", Expr::u32(2)),
        ]));
        let lets = facts.lets();
        assert_eq!(lets.len(), 2);
        assert_eq!(lets[0].1.as_str(), "x");
        assert_eq!(lets[1].1.as_str(), "y");
    }

    /// Var reads and buffer touches are observed across nesting.
    #[test]
    fn nested_if_collects_var_reads_and_buffer_refs() {
        let facts = ProgramFacts::build(&program(vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::If {
                cond: Expr::var("c"),
                then: vec![Node::store("a", Expr::var("x"), Expr::u32(1))],
                otherwise: vec![Node::store("b", Expr::var("x"), Expr::u32(2))],
            },
        ]));
        let var_reads: Vec<&str> = facts.var_reads().iter().map(|(_, n)| n.as_str()).collect();
        assert!(var_reads.contains(&"c"));
        let x_count = var_reads.iter().filter(|n| **n == "x").count();
        assert_eq!(x_count, 2, "x read in both arms");
        let a_writes: Vec<_> = facts
            .buffer_refs_of("a")
            .iter()
            .filter(|(_, k)| *k == BufferRefKind::Write)
            .collect();
        assert_eq!(a_writes.len(), 1);
        let b_writes: Vec<_> = facts
            .buffer_refs_of("b")
            .iter()
            .filter(|(_, k)| *k == BufferRefKind::Write)
            .collect();
        assert_eq!(b_writes.len(), 1);
    }

    /// `let_sites_of` returns every Let-site for a name; lookup
    /// indices are built lazily and reused.
    #[test]
    fn let_sites_of_resolves_via_lookup_index() {
        let facts = ProgramFacts::build(&program(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::Block(vec![Node::let_bind("x", Expr::u32(2))]),
        ]));
        let sites = facts.let_sites_of("x");
        assert_eq!(sites.len(), 2, "both Let-sites of `x` are recorded");
        assert!(facts.let_sites_of("missing").is_empty());
    }

    #[test]
    fn descendant_query_uses_parent_column() {
        let facts = ProgramFacts::build(&program(vec![Node::Block(vec![Node::let_bind(
            "x",
            Expr::u32(1),
        )])]));
        let root = facts.regions()[0].node;
        let let_idx = facts.lets()[0].0;
        assert!(facts.is_descendant_of(root, root));
        assert!(facts.is_descendant_of(let_idx, root));
        assert!(!facts.is_descendant_of(root, let_idx));
    }

    /// Atomic touches are recorded with the AtomicOp.
    #[test]
    fn atomic_buffer_refs_record_op() {
        let facts = ProgramFacts::build(&program(vec![Node::let_bind(
            "x",
            Expr::Atomic {
                op: AtomicOp::Add,
                buffer: Ident::from("a"),
                index: Box::new(Expr::u32(0)),
                expected: None,
                value: Box::new(Expr::u32(1)),
                ordering: MemoryOrdering::Relaxed,
            },
        )]));
        let touches = facts.buffer_refs_of("a");
        assert_eq!(touches.len(), 1);
        assert_eq!(touches[0].1, BufferRefKind::Atomic(AtomicOp::Add));
    }

    /// `is_name_rebound` distinguishes single Let, multi Let, Assign,
    /// and Loop-var rebinding.
    #[test]
    fn is_name_rebound_detects_every_shape() {
        let facts_single = ProgramFacts::build(&program(vec![Node::let_bind("x", Expr::u32(1))]));
        assert!(!facts_single.is_name_rebound("x"));
        assert!(!facts_single.is_name_rebound("y"));

        let facts_assign = ProgramFacts::build(&program(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::Assign {
                name: Ident::from("x"),
                value: Expr::u32(2),
            },
        ]));
        assert!(facts_assign.is_name_rebound("x"));

        let facts_loop = ProgramFacts::build(&program(vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(4),
            body: vec![],
        }]));
        assert!(facts_loop.is_name_rebound("i"));

        let facts_double_let = ProgramFacts::build(&program(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::Block(vec![Node::let_bind("x", Expr::u32(2))]),
        ]));
        assert!(facts_double_let.is_name_rebound("x"));
    }

    /// Every Loop-var binding is recorded in `loop_vars`.
    #[test]
    fn loop_vars_recorded_for_every_loop() {
        let facts = ProgramFacts::build(&program(vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(4),
            body: vec![Node::Loop {
                var: Ident::from("j"),
                from: Expr::u32(0),
                to: Expr::u32(4),
                body: vec![],
            }],
        }]));
        let names: Vec<&str> = facts.loop_vars().iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, vec!["i", "j"]);
    }

    /// `parent_of` reports the enclosing container for nested
    /// nodes; the root entry's wrapping Region has no parent.
    #[test]
    fn parent_of_reports_immediate_container() {
        let facts = ProgramFacts::build(&program(vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", Expr::u32(1))],
            otherwise: vec![Node::let_bind("y", Expr::u32(2))],
        }]));
        let region = NodeIndex(0);
        assert_eq!(facts.kind_at(region), NodeKind::Region);
        assert_eq!(facts.parent_of(region), None);
        let if_idx = facts
            .iter_nodes()
            .find(|(_, k)| *k == NodeKind::If)
            .map(|(i, _)| i)
            .expect("Fix: If node present");
        assert_eq!(facts.parent_of(if_idx), Some(region));
        let let_idxs: Vec<_> = facts.lets().iter().map(|(i, _)| *i).collect();
        for let_idx in let_idxs {
            assert_eq!(facts.parent_of(let_idx), Some(if_idx));
        }
    }

    /// `buffer_refs_of` reports the Write site of a Store, the
    /// Read site of a load inside its value, and distinguishes the
    /// two by `BufferRefKind`.
    #[test]
    fn buffer_refs_of_separates_read_and_write() {
        let facts = ProgramFacts::build(&program(vec![Node::store(
            "a",
            Expr::u32(0),
            Expr::Load {
                buffer: Ident::from("b"),
                index: Box::new(Expr::u32(0)),
            },
        )]));
        let a_touches = facts.buffer_refs_of("a");
        assert_eq!(a_touches.len(), 1);
        assert_eq!(a_touches[0].1, BufferRefKind::Write);
        let b_touches = facts.buffer_refs_of("b");
        assert_eq!(b_touches.len(), 1);
        assert_eq!(b_touches[0].1, BufferRefKind::Read);
    }

    /// `has_kind` short-circuits passes that have no candidate
    /// nodes.
    #[test]
    fn has_kind_short_circuits_missing_variants() {
        let facts = ProgramFacts::build(&program(vec![Node::let_bind("x", Expr::u32(1))]));
        assert!(facts.has_kind(NodeKind::Let));
        assert!(!facts.has_kind(NodeKind::Loop));
        assert!(!facts.has_kind(NodeKind::Trap));
    }

    /// `iter_nodes` yields every node in preorder with its kind.
    #[test]
    fn iter_nodes_yields_preorder() {
        let facts = ProgramFacts::build(&program(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::let_bind("y", Expr::u32(2)),
        ]));
        let kinds: Vec<NodeKind> = facts.iter_nodes().map(|(_, k)| k).collect();
        assert_eq!(kinds, vec![NodeKind::Region, NodeKind::Let, NodeKind::Let]);
    }

    // ──── ROADMAP A3: region/source metadata side-table ────

    /// `regions()` records every Region in the entry tree, including
    /// the wrapping Region that `Program::wrapped` injects when the
    /// entry contains non-Region top-level nodes.
    #[test]
    fn regions_records_wrapping_and_nested() {
        // Mixing a non-Region top-level node forces `Program::wrapped`
        // to inject the root Region, so the fact table sees exactly
        // two regions: the wrapper plus the explicit inner one.
        let inner = Node::Region {
            generator: Ident::from("inner_pass"),
            source_region: None,
            body: std::sync::Arc::new(vec![Node::let_bind("x", Expr::u32(1))]),
        };
        let facts = ProgramFacts::build(&program(vec![Node::let_bind("z", Expr::u32(0)), inner]));
        let regions = facts.regions();
        assert_eq!(regions.len(), 2, "wrapping Region + inner Region");
        assert!(regions.iter().any(|r| r.generator.as_str() == "inner_pass"));
    }

    /// `region_at(idx)` looks up the Region metadata for a Region
    /// node by its `NodeIndex`. Returns None for non-Region nodes.
    #[test]
    fn region_at_resolves_by_node_index() {
        let inner = Node::Region {
            generator: Ident::from("custom"),
            source_region: None,
            body: std::sync::Arc::new(vec![]),
        };
        let facts = ProgramFacts::build(&program(vec![inner]));
        let region_idx = facts
            .iter_nodes()
            .filter(|(_, k)| *k == NodeKind::Region)
            .map(|(i, _)| i)
            .find(|i| {
                facts
                    .region_at(*i)
                    .map(|m| m.generator.as_str() == "custom")
                    .unwrap_or(false)
            })
            .expect("Fix: custom-generator Region present");
        let meta = facts.region_at(region_idx).expect("Fix: region recorded");
        assert_eq!(meta.generator.as_str(), "custom");
        assert_eq!(meta.source_region, None);
        let let_idx = facts.lets().get(0).map(|(i, _)| *i);
        if let Some(let_idx) = let_idx {
            assert!(facts.region_at(let_idx).is_none());
        }
    }

    /// `regions_by_generator(name)` returns every Region whose
    /// generator matches.
    #[test]
    fn regions_by_generator_filters_by_ident() {
        let entry = vec![
            Node::Region {
                generator: Ident::from("vec"),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::let_bind("x", Expr::u32(1))]),
            },
            Node::Region {
                generator: Ident::from("dce"),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::let_bind("y", Expr::u32(2))]),
            },
            Node::Region {
                generator: Ident::from("vec"),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::let_bind("z", Expr::u32(3))]),
            },
        ];
        let facts = ProgramFacts::build(&program(entry));
        let vec_count = facts.regions_by_generator("vec").count();
        assert_eq!(vec_count, 2);
        let dce_count = facts.regions_by_generator("dce").count();
        assert_eq!(dce_count, 1);
        let missing = facts.regions_by_generator("missing").count();
        assert_eq!(missing, 0);
    }

    /// Region wrappers are provenance rows, not semantic optimizer
    /// nodes. The regionless view keeps preorder for real work while
    /// skipping both the wrapper inserted by Program::wrapped and any
    /// explicit nested Region.
    #[test]
    fn regionless_nodes_skip_provenance_wrappers() {
        let facts = ProgramFacts::build(&program(vec![
            Node::let_bind("root", Expr::u32(0)),
            Node::Region {
                generator: Ident::from("inner"),
                source_region: None,
                body: std::sync::Arc::new(vec![Node::let_bind("nested", Expr::u32(1))]),
            },
        ]));
        let kinds: Vec<NodeKind> = facts
            .iter_regionless_nodes()
            .map(|(_, kind)| kind)
            .collect();
        assert_eq!(kinds, vec![NodeKind::Let, NodeKind::Let]);
    }

    /// `regionless_parent_of` skips Region ancestors but preserves
    /// real structural parents such as Block. Optimizer passes can
    /// use this for scope queries without treating provenance
    /// wrappers as part of the compute tree.
    #[test]
    fn regionless_parent_skips_only_region_ancestors() {
        let facts = ProgramFacts::build(&program(vec![Node::Block(vec![Node::Region {
            generator: Ident::from("inner"),
            source_region: None,
            body: std::sync::Arc::new(vec![Node::let_bind("x", Expr::u32(1))]),
        }])]));
        let block = facts
            .iter_nodes()
            .find(|(_, kind)| *kind == NodeKind::Block)
            .map(|(idx, _)| idx)
            .expect("Fix: Block node present");
        let let_idx = facts.lets()[0].0;
        assert_eq!(facts.regionless_parent_of(block), None);
        assert_eq!(facts.regionless_parent_of(let_idx), Some(block));
    }

    // ──── ROADMAP A12: points-to facts (buffers_provably_distinct) ────

    /// Two distinct named buffers in the program both touched at
    /// least once → provably distinct.
    #[test]
    fn buffers_provably_distinct_for_distinct_names() {
        let facts = ProgramFacts::build(&program(vec![
            Node::store("a", Expr::u32(0), Expr::u32(1)),
            Node::store("b", Expr::u32(0), Expr::u32(2)),
        ]));
        assert!(facts.buffers_provably_distinct("a", "b"));
        assert!(facts.buffers_provably_distinct("b", "a"));
    }

    /// A buffer trivially aliases itself.
    #[test]
    fn buffers_provably_distinct_rejects_same_name() {
        let facts =
            ProgramFacts::build(&program(vec![Node::store("a", Expr::u32(0), Expr::u32(1))]));
        assert!(!facts.buffers_provably_distinct("a", "a"));
    }

    /// A name that doesn't appear in the buffer_refs column is not
    /// a real buffer  -  the fact returns false to keep the contract
    /// honest.
    #[test]
    fn buffers_provably_distinct_rejects_phantom_name() {
        let facts =
            ProgramFacts::build(&program(vec![Node::store("a", Expr::u32(0), Expr::u32(1))]));
        assert!(!facts.buffers_provably_distinct("a", "phantom"));
    }

    // ──── ROADMAP A13: escape facts (buffer_escapes) ────

    /// A buffer that's only read (Load) does NOT escape  -  its
    /// contents are an input the host produced.
    #[test]
    fn buffer_does_not_escape_when_read_only() {
        let facts = ProgramFacts::build(&program(vec![Node::let_bind(
            "x",
            Expr::Load {
                buffer: Ident::from("a"),
                index: Box::new(Expr::u32(0)),
            },
        )]));
        assert!(!facts.buffer_escapes("a"));
    }

    /// A buffer that's stored to escapes (host reads back).
    #[test]
    fn buffer_escapes_when_stored_to() {
        let facts =
            ProgramFacts::build(&program(vec![Node::store("a", Expr::u32(0), Expr::u32(1))]));
        assert!(facts.buffer_escapes("a"));
    }

    /// A buffer touched atomically escapes (atomic results are
    /// observable across workgroups + the host).
    #[test]
    fn buffer_escapes_when_atomically_touched() {
        let facts = ProgramFacts::build(&program(vec![Node::let_bind(
            "x",
            Expr::Atomic {
                op: AtomicOp::Add,
                buffer: Ident::from("a"),
                index: Box::new(Expr::u32(0)),
                expected: None,
                value: Box::new(Expr::u32(1)),
                ordering: MemoryOrdering::Relaxed,
            },
        )]));
        assert!(facts.buffer_escapes("a"));
    }

    /// `escaping_buffers()` enumerates the set in one go.
    #[test]
    fn escaping_buffers_enumerates_set() {
        let facts = ProgramFacts::build(&program(vec![
            Node::store("a", Expr::u32(0), Expr::u32(1)),
            Node::let_bind(
                "x",
                Expr::Load {
                    buffer: Ident::from("b"),
                    index: Box::new(Expr::u32(0)),
                },
            ),
        ]));
        let escaping = facts.escaping_buffers();
        assert_eq!(escaping.len(), 1);
        assert!(escaping.iter().any(|k| k.as_str() == "a"));
    }
}

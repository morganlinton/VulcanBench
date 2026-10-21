//! Equality-saturation engine  -  minimal `EGraph` substrate for vyre IR
//! algebraic rewrite families.
//!
//! Op id: `vyre-foundation::optimizer::eqsat`. Soundness: every equivalence
//! added to the `EGraph` must be a true semantic equality of the underlying
//! IR. Cost-direction: extraction phase picks the lowest-cost equivalent
//! representative under a caller-supplied cost function  -  guaranteed
//! cost-monotone-down by construction.
//!
//! ## Why
//!
//! Pass-by-pass rewriting commits to a single rewrite at every step. When
//! two passes both want to fire on the same expression, one wins
//! (whichever is scheduled first), even if the other would have unlocked
//! a much better optimization downstream. Equality saturation sidesteps
//! this by accumulating all known equivalences into one `EGraph`, running
//! every rewrite rule to a fixed point, and then extracting the
//! lowest-cost equivalent at the end.
//!
//! This module ships the substrate: a minimal but sound `EGraph` with
//! hashcons, union-find, rebuild, saturation, and a `Family` trait
//! that wraps a set of related rewrite rules.
//!
//! ## `ENode`
//!
//! `ENodes` are domain-specific: each family defines its own `ENode` enum.
//! The substrate is generic over `Lang: ENodeLang` which provides the
//! children-iteration API the `EGraph` needs to canonicalize and rebuild.
//!
//! ## Why not import egg
//!
//! This implementation is intentionally minimal so it lives entirely
//! within `vyre-foundation` with no external dep, no proc-macro, and
//! no per-rule code generation. The egg crate is more featureful but
//! adds a dependency tree that conflicts with vyre's "every dep is a
//! supply-chain risk" stance.

use std::error::Error as StdError;
use std::fmt;
use std::hash::{Hash, Hasher};

use rustc_hash::FxHashMap;
use rustc_hash::FxHasher;
use smallvec::SmallVec;

/// Default extraction fixed-point iteration budget.
pub const DEFAULT_EXTRACTION_ITER_BUDGET: usize = 1024;

/// Stack-backed child list used by `EGraph` node APIs. Most IR algebra nodes
/// have 0-3 children; keeping that path inline avoids allocator traffic during
/// saturation.
pub type EChildren = SmallVec<[EClassId; 4]>;

/// Identifier of an `EClass` in the `EGraph`. `EClasses` are dense u32-indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EClassId(pub u32);

/// Domain-specific `ENode` language. Implementations describe how to
/// iterate the children of a node (for canonicalization) and how to
/// rebuild a node with replacement child ids (for rebuild).
pub trait ENodeLang: Clone + Eq + Hash {
    /// Iterate the `EClass`-child ids referenced by this node, in order.
    fn children(&self) -> EChildren;

    /// Rebuild this node with replacement `EClass` children. The returned
    /// node has the same shape as `self` but with each child replaced by
    /// the corresponding entry in `children`. `children.len()` must equal
    /// `self.children().len()`.
    #[must_use]
    fn with_children(&self, children: &[EClassId]) -> Self;
}

/// One equivalence class  -  the set of all `ENodes` proven equal so far.
#[derive(Debug, Clone)]
pub struct EClass<L: ENodeLang> {
    /// Every `ENode` that lives in this class (canonicalized form).
    pub nodes: Vec<L>,
    /// `EClasses` that have THIS one as a child  -  used during rebuild to
    /// propagate canonicalization.
    pub parents: Vec<EClassId>,
}

/// The `EGraph`: a union-find of `EClasses` + a hashcons mapping
/// canonicalized `ENodes` to their `EClass`.
#[derive(Debug, Clone)]
pub struct EGraph<L: ENodeLang> {
    /// Class storage (dense). The class at index `i` is `EClass(i)`.
    classes: Vec<EClass<L>>,
    /// Hashcons: canonicalized `ENode` → `EClassId`. Maintained incrementally
    /// by `add()` and rebuilt after `union()` operations.
    hashcons: FxHashMap<L, EClassId>,
    /// Union-find parent pointers for path-compression find.
    parent: Vec<EClassId>,
    /// Set of `EClasses` that need rebuild after a union  -  drained by
    /// `rebuild()`.
    pending: Vec<EClassId>,
}

/// E-graph construction, indexing, and staging failure.
///
/// Equality saturation is optimizer infrastructure, so allocator pressure and
/// class-id overflow must be explicit errors on the fallible APIs rather than
/// latent panics or poisoned sentinel ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EGraphError {
    /// A fallible staging/allocation reservation failed.
    Capacity {
        /// Operation reserving memory.
        context: &'static str,
        /// Additional elements/slots requested.
        requested: usize,
        /// Allocator error rendered with platform-specific detail.
        source: String,
    },
    /// Dense class storage exceeded the public `u32` id space.
    ClassIdOverflow {
        /// Dense class index that could not be represented as [`EClassId`].
        index: usize,
    },
    /// A caller supplied an `EClassId` outside the current dense tables.
    ClassIdOutOfBounds {
        /// Operation resolving the id.
        context: &'static str,
        /// Invalid id.
        id: EClassId,
        /// Current table length.
        len: usize,
    },
}

impl fmt::Display for EGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity {
                context,
                requested,
                source,
            } => write!(
                f,
                "{context} could not reserve {requested} additional slots: {source}. Fix: lower the saturation batch size or split the optimizer workload."
            ),
            Self::ClassIdOverflow { index } => write!(
                f,
                "egraph class index {index} exceeds the u32 EClassId space. Fix: split the egraph or extract before adding more classes."
            ),
            Self::ClassIdOutOfBounds { context, id, len } => write!(
                f,
                "{context} referenced eclass id {} but only {len} class slots exist. Fix: pass ids returned by this EGraph instance.",
                id.0
            ),
        }
    }
}

impl StdError for EGraphError {}

fn log_egraph_compat_error(context: &'static str, error: &EGraphError) {
    tracing::error!(
        context,
        error = %error,
        "legacy infallible egraph API failed; use the matching try_* API to handle this condition explicitly"
    );
}

impl<L: ENodeLang> Default for EGraph<L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L: ENodeLang> EGraph<L> {
    /// Create an empty `EGraph`.
    #[must_use]
    pub fn new() -> Self {
        Self::empty_unreserved()
    }

    /// Create an `EGraph` with capacity for an expected number of `EClasses`.
    #[must_use]
    pub fn with_capacity(class_capacity: usize) -> Self {
        match Self::try_with_capacity(class_capacity) {
            Ok(egraph) => egraph,
            Err(error) => {
                log_egraph_compat_error("egraph with_capacity", &error);
                Self::empty_unreserved()
            }
        }
    }

    /// Fallible variant of [`Self::with_capacity`].
    pub fn try_with_capacity(class_capacity: usize) -> Result<Self, EGraphError> {
        let mut classes = Vec::new();
        reserve_vec_exact(&mut classes, class_capacity, "egraph class storage")?;
        let mut hashcons = FxHashMap::default();
        reserve_hashcons(&mut hashcons, class_capacity, "egraph hashcons storage")?;
        let mut parent = Vec::new();
        reserve_vec_exact(
            &mut parent,
            class_capacity,
            "egraph union-find parent storage",
        )?;
        let mut pending = Vec::new();
        reserve_vec_exact(&mut pending, class_capacity, "egraph rebuild queue storage")?;
        Ok(Self {
            classes,
            hashcons,
            parent,
            pending,
        })
    }

    fn empty_unreserved() -> Self {
        Self {
            classes: Vec::new(),
            hashcons: FxHashMap::default(),
            parent: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// Number of `EClasses` currently in the graph.
    #[must_use]
    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    /// Find the canonical class representative via path compression.
    pub fn find(&mut self, id: EClassId) -> EClassId {
        match self.try_find(id) {
            Ok(found) => found,
            Err(error) => {
                log_egraph_compat_error("egraph find", &error);
                id
            }
        }
    }

    /// Fallible variant of [`Self::find`].
    pub fn try_find(&mut self, id: EClassId) -> Result<EClassId, EGraphError> {
        let mut cur = id;
        loop {
            let cur_idx = eclass_index(cur, self.parent.len(), "egraph find")?;
            let parent = self.parent[cur_idx];
            if parent == cur {
                break;
            }
            cur = parent;
        }
        // Path compression.
        let mut walk = id;
        loop {
            let walk_idx = eclass_index(walk, self.parent.len(), "egraph path compression")?;
            let next = self.parent[walk_idx];
            if next == cur {
                break;
            }
            self.parent[walk_idx] = cur;
            walk = next;
        }
        Ok(cur)
    }

    /// Find a canonical class without path compression  -  for read-only
    /// use during iteration.
    #[must_use]
    pub fn find_immut(&self, id: EClassId) -> EClassId {
        match self.try_find_immut(id) {
            Ok(found) => found,
            Err(error) => {
                log_egraph_compat_error("egraph immutable find", &error);
                id
            }
        }
    }

    /// Fallible variant of [`Self::find_immut`].
    pub fn try_find_immut(&self, id: EClassId) -> Result<EClassId, EGraphError> {
        let mut cur = id;
        loop {
            let cur_idx = eclass_index(cur, self.parent.len(), "egraph immutable find")?;
            let parent = self.parent[cur_idx];
            if parent == cur {
                break;
            }
            cur = parent;
        }
        Ok(cur)
    }

    /// Canonicalize a node by replacing each child with its current
    /// canonical `EClass`.
    fn canonicalize(&self, node: &L) -> L {
        match self.try_canonicalize(node) {
            Ok(canonical) => canonical,
            Err(error) => {
                log_egraph_compat_error("egraph canonicalize", &error);
                node.clone()
            }
        }
    }

    fn try_canonicalize(&self, node: &L) -> Result<L, EGraphError> {
        let canon_children: EChildren = node
            .children()
            .into_iter()
            .map(|c| self.try_find_immut(c))
            .collect::<Result<_, _>>()?;
        Ok(node.with_children(&canon_children))
    }

    /// Add a node to the `EGraph`. If an equivalent node already exists,
    /// return its `EClassId`; otherwise create a new `EClass`.
    pub fn add(&mut self, node: L) -> EClassId {
        match self.try_add(node) {
            Ok(id) => id,
            Err(error) => {
                log_egraph_compat_error("egraph add", &error);
                EClassId(0)
            }
        }
    }

    /// Fallible variant of [`Self::add`].
    #[expect(
        clippy::needless_pass_by_value,
        reason = "public insertion API consumes language nodes; canonicalized misses store an owned node"
    )]
    pub fn try_add(&mut self, node: L) -> Result<EClassId, EGraphError> {
        let canon = self.try_canonicalize(&node)?;
        if let Some(&existing) = self.hashcons.get(&canon) {
            return self.try_find(existing);
        }
        let new_id = try_eclass_id_from_index(self.classes.len())?;
        let canon_children = canon.children();
        reserve_vec_exact(&mut self.parent, 1, "egraph parent insertion")?;
        reserve_vec_exact(&mut self.classes, 1, "egraph class insertion")?;
        reserve_hashcons(&mut self.hashcons, 1, "egraph hashcons insertion")?;
        let mut child_indices: SmallVec<[(usize, EClassId); 4]> = SmallVec::new();
        for child in &canon_children {
            let child_canon = self.try_find(*child)?;
            let child_idx = eclass_index(
                child_canon,
                self.classes.len(),
                "egraph child parent registration",
            )?;
            child_indices.push((child_idx, child_canon));
        }
        for (position, (child_idx, _)) in child_indices.iter().enumerate() {
            if child_indices[..position]
                .iter()
                .any(|(seen_idx, _)| seen_idx == child_idx)
            {
                continue;
            }
            let occurrences = child_indices
                .iter()
                .filter(|(seen_idx, _)| seen_idx == child_idx)
                .count();
            reserve_vec_exact(
                &mut self.classes[*child_idx].parents,
                occurrences,
                "egraph child parent registration",
            )?;
        }
        let mut nodes = Vec::new();
        reserve_vec_exact(&mut nodes, 1, "egraph singleton enode storage")?;
        nodes.push(canon.clone());
        self.parent.push(new_id);
        // Register `new_id` as a parent of each child class.
        for (child_idx, _) in child_indices {
            self.classes[child_idx].parents.push(new_id);
        }
        self.classes.push(EClass {
            nodes,
            parents: Vec::new(),
        });
        self.hashcons.insert(canon, new_id);
        Ok(new_id)
    }

    /// Equate two `EClasses`. The returned id is the canonical class for
    /// both inputs after the union. Calls to `add()` on equivalent nodes
    /// will return this same id.
    ///
    /// Caller must invoke `rebuild()` after a batch of `union()` calls
    /// to re-canonicalize the hashcons + propagate equivalences upward
    /// through parent pointers.
    pub fn union(&mut self, a: EClassId, b: EClassId) -> EClassId {
        match self.try_union(a, b) {
            Ok(id) => id,
            Err(error) => {
                log_egraph_compat_error("egraph union", &error);
                a
            }
        }
    }

    /// Fallible variant of [`Self::union`].
    pub fn try_union(&mut self, a: EClassId, b: EClassId) -> Result<EClassId, EGraphError> {
        let a_root = self.try_find(a)?;
        let b_root = self.try_find(b)?;
        if a_root == b_root {
            return Ok(a_root);
        }
        // Union with the smaller-id-as-root convention for determinism.
        let (winner, loser) = if a_root.0 < b_root.0 {
            (a_root, b_root)
        } else {
            (b_root, a_root)
        };
        let winner_idx = eclass_index(winner, self.classes.len(), "egraph union winner")?;
        let loser_idx = eclass_index(loser, self.classes.len(), "egraph union loser")?;
        let loser_nodes_len = self.classes[loser_idx].nodes.len();
        let loser_parents_len = self.classes[loser_idx].parents.len();
        reserve_vec_exact(
            &mut self.classes[winner_idx].nodes,
            loser_nodes_len,
            "egraph union node merge",
        )?;
        reserve_vec_exact(
            &mut self.classes[winner_idx].parents,
            loser_parents_len,
            "egraph union parent merge",
        )?;
        reserve_vec_exact(&mut self.pending, 1, "egraph rebuild queue push")?;
        self.parent[loser_idx] = winner;
        // Merge nodes + parent lists into the winning class.
        let loser_class = std::mem::replace(
            &mut self.classes[loser_idx],
            EClass {
                nodes: Vec::new(),
                parents: Vec::new(),
            },
        );
        self.classes[winner_idx].nodes.extend(loser_class.nodes);
        self.classes[winner_idx].parents.extend(loser_class.parents);
        // Schedule the winner for rebuild  -  its parents may now be
        // canonicalizable.
        self.pending.push(winner);
        Ok(winner)
    }

    /// Re-canonicalize the hashcons after a batch of `union()` calls.
    /// Returns the number of additional unions discovered transitively.
    pub fn rebuild(&mut self) -> usize {
        match self.try_rebuild() {
            Ok(count) => count,
            Err(error) => {
                log_egraph_compat_error("egraph rebuild", &error);
                0
            }
        }
    }

    /// Fallible variant of [`Self::rebuild`].
    pub fn try_rebuild(&mut self) -> Result<usize, EGraphError> {
        let mut new_unions = 0;
        while let Some(class_id) = self.pending.pop() {
            let canonical = self.try_find(class_id)?;
            let canonical_idx = eclass_index(canonical, self.classes.len(), "egraph rebuild")?;
            let nodes_len = self.classes[canonical_idx].nodes.len();
            let mut canon_nodes = Vec::new();
            reserve_vec_exact(
                &mut canon_nodes,
                nodes_len,
                "egraph rebuild canonical node staging",
            )?;
            reserve_hashcons(
                &mut self.hashcons,
                nodes_len,
                "egraph rebuild hashcons staging",
            )?;
            // Re-canonicalize every node in the canonical class.
            let nodes = std::mem::take(&mut self.classes[canonical_idx].nodes);
            for node in nodes {
                let new_canon = self.try_canonicalize(&node)?;
                // Re-insert into hashcons; collisions trigger more unions.
                if let Some(&existing) = self.hashcons.get(&new_canon) {
                    let existing_canon = self.try_find(existing)?;
                    if existing_canon != canonical {
                        let unified = self.try_union(existing_canon, canonical)?;
                        new_unions += 1;
                        if unified != canonical {
                            // The winner changed  -  re-find at top of loop.
                            reserve_vec_exact(
                                &mut self.pending,
                                1,
                                "egraph rebuild winner reschedule",
                            )?;
                            self.pending.push(unified);
                        }
                    }
                }
                self.hashcons.insert(new_canon.clone(), canonical);
                canon_nodes.push(new_canon);
            }
            try_dedup_enodes_by_hash(&mut canon_nodes)?;
            self.classes[canonical_idx].nodes = canon_nodes;
        }
        Ok(new_unions)
    }

    /// Iterate every (`EClassId`, `ENode`) pair currently in the graph.
    /// Useful for rule application and extraction.
    pub fn iter_nodes(&self) -> impl Iterator<Item = (EClassId, &L)> {
        self.classes
            .iter()
            .enumerate()
            .filter_map(|(idx, class)| {
                let class_id = match try_eclass_id_from_index(idx) {
                    Ok(class_id) => class_id,
                    Err(error) => {
                        log_egraph_compat_error("egraph iter_nodes class id", &error);
                        return None;
                    }
                };
                (self.parent[idx] == class_id).then_some((class_id, class))
            })
            .flat_map(|(class_id, class)| class.nodes.iter().map(move |n| (class_id, n)))
    }

    /// Read-only access to a class by id.
    #[must_use]
    pub fn class(&self, id: EClassId) -> Option<&EClass<L>> {
        match self.try_class(id) {
            Ok(class) => class,
            Err(error) => {
                log_egraph_compat_error("egraph class lookup", &error);
                None
            }
        }
    }

    /// Fallible variant of [`Self::class`].
    pub fn try_class(&self, id: EClassId) -> Result<Option<&EClass<L>>, EGraphError> {
        let canon = self.try_find_immut(id)?;
        let idx = eclass_index(canon, self.classes.len(), "egraph class lookup")?;
        Ok(self.classes.get(idx))
    }
}

fn eclass_id_from_index(index: usize) -> EClassId {
    match try_eclass_id_from_index(index) {
        Ok(id) => id,
        Err(error) => {
            log_egraph_compat_error("egraph class id conversion", &error);
            EClassId(0)
        }
    }
}

fn try_eclass_id_from_index(index: usize) -> Result<EClassId, EGraphError> {
    u32::try_from(index)
        .map(EClassId)
        .map_err(|_| EGraphError::ClassIdOverflow { index })
}

fn eclass_index(id: EClassId, len: usize, context: &'static str) -> Result<usize, EGraphError> {
    let index =
        usize::try_from(id.0).map_err(|_| EGraphError::ClassIdOutOfBounds { context, id, len })?;
    if index < len {
        Ok(index)
    } else {
        Err(EGraphError::ClassIdOutOfBounds { context, id, len })
    }
}

fn reserve_vec_exact<T>(
    vec: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), EGraphError> {
    vec.try_reserve_exact(additional)
        .map_err(|source| EGraphError::Capacity {
            context,
            requested: additional,
            source: source.to_string(),
        })
}

fn reserve_hashcons<L: Eq + Hash>(
    hashcons: &mut FxHashMap<L, EClassId>,
    additional: usize,
    context: &'static str,
) -> Result<(), EGraphError> {
    hashcons
        .try_reserve(additional)
        .map_err(|source| EGraphError::Capacity {
            context,
            requested: additional,
            source: source.to_string(),
        })
}

fn dedup_enodes_by_hash<L: ENodeLang>(nodes: &mut Vec<L>) {
    if let Err(error) = try_dedup_enodes_by_hash(nodes) {
        log_egraph_compat_error("egraph dedup", &error);
    }
}

fn try_dedup_enodes_by_hash<L: ENodeLang>(nodes: &mut Vec<L>) -> Result<(), EGraphError> {
    if nodes.len() <= 1 {
        return Ok(());
    }
    let mut keyed = Vec::new();
    reserve_vec_exact(&mut keyed, nodes.len(), "egraph dedup hash staging")?;
    keyed.extend(nodes.drain(..).map(|node| (stable_enode_hash(&node), node)));
    keyed.sort_unstable_by_key(|(hash, _)| *hash);
    let mut deduped: Vec<(u64, L)> = Vec::new();
    reserve_vec_exact(&mut deduped, keyed.len(), "egraph dedup output staging")?;
    for (hash, node) in keyed {
        let duplicate_in_hash_bucket = deduped
            .iter()
            .rev()
            .take_while(|(existing_hash, _)| *existing_hash == hash)
            .any(|(_, existing)| existing == &node);
        if !duplicate_in_hash_bucket {
            deduped.push((hash, node));
        }
    }
    reserve_vec_exact(nodes, deduped.len(), "egraph dedup node restoration")?;
    nodes.extend(deduped.into_iter().map(|(_, node)| node));
    Ok(())
}

fn stable_enode_hash<L: ENodeLang>(node: &L) -> u64 {
    let mut hasher = FxHasher::default();
    node.hash(&mut hasher);
    hasher.finish()
}

/// One equality-saturation rewrite rule. Returns a list of `(left, right)`
/// `EClass` pairs that should be unioned after the rule fires.
///
/// Implementations walk the `EGraph` (via `iter_nodes`), pattern-match on
/// shapes they recognize, and return the equivalences they want to add.
pub trait Rule<L: ENodeLang> {
    /// Human-readable rule name for telemetry + tests.
    fn name(&self) -> &'static str;

    /// Find every match of this rule's LHS pattern in `egraph` and return
    /// the (a, b) pairs that should be equated.
    fn matches(&self, egraph: &EGraph<L>) -> Vec<(EClassId, EClassId)>;
}

/// A family of related rewrite rules.
pub trait Family<L: ENodeLang> {
    /// Family name (e.g. "`commutative_arith`").
    fn name(&self) -> &'static str;

    /// Vec of rules in this family. Stored as boxed trait objects so a
    /// single family can mix rule shapes (literal-matching, pattern-
    /// matching, conditional rewrites).
    fn rules(&self) -> Vec<Box<dyn Rule<L>>>;
}

/// Reason an equality-saturation run stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturationStopReason {
    /// No rule set was supplied.
    EmptyRuleSet,
    /// The caller supplied a zero-iteration cap.
    ZeroBudget,
    /// A rule scan produced no more equivalences.
    FixedPoint,
    /// The run consumed the supplied iteration cap while matches were still
    /// being produced.
    IterationBudget,
}

/// Executable telemetry for one equality-saturation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaturationReport {
    /// Rewrite family label for this run. Raw rule-slice calls use `global`;
    /// per-family calls use [`Family::name`].
    pub rewrite_family: &'static str,
    /// Number of rules scanned each iteration.
    pub rule_count: usize,
    /// Iterations actually executed.
    pub iters_used: usize,
    /// Caller-supplied iteration budget.
    pub budget: usize,
    /// Why the run stopped.
    pub stop_reason: SaturationStopReason,
    /// Dense class slots before rule application.
    pub class_count_before: usize,
    /// Dense class slots after rule application.
    pub class_count_after: usize,
    /// Equivalence pairs returned by rules and handed to union.
    pub applied_equivalences: usize,
    /// Extra unions discovered by rebuild propagation.
    pub rebuild_unions: usize,
}

impl SaturationReport {
    /// Signed dense-class delta from before to after.
    #[must_use]
    pub fn class_count_delta(&self) -> isize {
        self.class_count_after as isize - self.class_count_before as isize
    }
}

/// Run rules to fixed point or `max_iters`, whichever comes first.
/// Returns the iteration count actually used.
pub fn saturate<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    rules: &[Box<dyn Rule<L>>],
    max_iters: usize,
) -> usize {
    match try_saturate(egraph, rules, max_iters) {
        Ok(iters) => iters,
        Err(error) => {
            log_egraph_compat_error("egraph saturate", &error);
            0
        }
    }
}

/// Fallible variant of [`saturate`].
pub fn try_saturate<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    rules: &[Box<dyn Rule<L>>],
    max_iters: usize,
) -> Result<usize, EGraphError> {
    try_saturate_with_report(egraph, rules, max_iters).map(|report| report.iters_used)
}

/// Run a raw rule set and return saturation telemetry.
pub fn saturate_with_report<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    rules: &[Box<dyn Rule<L>>],
    max_iters: usize,
) -> SaturationReport {
    match try_saturate_with_report(egraph, rules, max_iters) {
        Ok(report) => report,
        Err(error) => {
            log_egraph_compat_error("egraph saturate_with_report", &error);
            finalize_saturation_report(
                egraph,
                saturation_report(
                    "global",
                    rules.len(),
                    0,
                    max_iters,
                    SaturationStopReason::IterationBudget,
                    egraph.class_count(),
                    0,
                    0,
                ),
            )
        }
    }
}

/// Fallible variant of [`saturate_with_report`].
pub fn try_saturate_with_report<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    rules: &[Box<dyn Rule<L>>],
    max_iters: usize,
) -> Result<SaturationReport, EGraphError> {
    try_saturate_named(egraph, "global", rules, max_iters)
}

/// Run a named rewrite family and return saturation telemetry.
pub fn try_saturate_named<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    rewrite_family: &'static str,
    rules: &[Box<dyn Rule<L>>],
    max_iters: usize,
) -> Result<SaturationReport, EGraphError> {
    let class_count_before = egraph.class_count();
    if rules.is_empty() {
        return Ok(finalize_saturation_report(
            egraph,
            saturation_report(
                rewrite_family,
                0,
                0,
                max_iters,
                SaturationStopReason::EmptyRuleSet,
                class_count_before,
                0,
                0,
            ),
        ));
    }
    if max_iters == 0 {
        return Ok(finalize_saturation_report(
            egraph,
            saturation_report(
                rewrite_family,
                rules.len(),
                0,
                0,
                SaturationStopReason::ZeroBudget,
                class_count_before,
                0,
                0,
            ),
        ));
    }
    let mut equivalences = Vec::new();
    reserve_vec_exact(
        &mut equivalences,
        egraph.class_count(),
        "egraph saturation equivalence staging",
    )?;
    let mut applied_equivalences = 0usize;
    let mut rebuild_unions = 0usize;
    for iter in 0..max_iters {
        equivalences.clear();
        for rule in rules {
            let matches = rule.matches(egraph);
            reserve_vec_exact(
                &mut equivalences,
                matches.len(),
                "egraph saturation rule-match staging",
            )?;
            equivalences.extend(matches);
        }
        if equivalences.is_empty() {
            return Ok(finalize_saturation_report(
                egraph,
                saturation_report(
                    rewrite_family,
                    rules.len(),
                    iter,
                    max_iters,
                    SaturationStopReason::FixedPoint,
                    class_count_before,
                    applied_equivalences,
                    rebuild_unions,
                ),
            ));
        }
        applied_equivalences = applied_equivalences.saturating_add(equivalences.len());
        for (a, b) in equivalences.drain(..) {
            egraph.try_union(a, b)?;
        }
        let extra = egraph.try_rebuild()?;
        rebuild_unions = rebuild_unions.saturating_add(extra);
        if extra == 0 && egraph.pending.is_empty() {
            // Nothing else to propagate; still need to check if rules find
            // anything new on the next iter.
        }
    }
    Ok(finalize_saturation_report(
        egraph,
        saturation_report(
            rewrite_family,
            rules.len(),
            max_iters,
            max_iters,
            SaturationStopReason::IterationBudget,
            class_count_before,
            applied_equivalences,
            rebuild_unions,
        ),
    ))
}

fn saturation_report(
    rewrite_family: &'static str,
    rule_count: usize,
    iters_used: usize,
    budget: usize,
    stop_reason: SaturationStopReason,
    class_count_before: usize,
    applied_equivalences: usize,
    rebuild_unions: usize,
) -> SaturationReport {
    SaturationReport {
        rewrite_family,
        rule_count,
        iters_used,
        budget,
        stop_reason,
        class_count_before,
        class_count_after: class_count_before,
        applied_equivalences,
        rebuild_unions,
    }
}

fn finalize_saturation_report<L: ENodeLang>(
    egraph: &EGraph<L>,
    mut report: SaturationReport,
) -> SaturationReport {
    report.class_count_after = egraph.class_count();
    report
}

/// Adapter that gates a base [`Rule`] on a device-fact predicate.
///
/// ROADMAP A9. The "should this rule fire on this hardware?" check
/// recurs across every device-aware Rule (FP16 only on `supports_f16`,
/// tensor-core fusion only on `supports_tensor_cores`, subgroup
/// shuffle only on `has_subgroup_shuffle`). Without a shared adapter,
/// every Rule re-implements the same `if !facts.feature { return
/// vec![] }` preamble. This wrapper centralises it.
///
/// `DeviceFacts` is a free-form caller-owned object so the foundation
/// crate does not pull `DeviceProfile` (which lives in `vyre-driver`)
/// into its dependency graph. Callers either pass a borrowed
/// `&DeviceProfile` directly via the `predicate` closure capture, or
/// thread a snapshot through their own type.
///
/// When `predicate` returns `false` the wrapped rule's [`matches`]
/// short-circuits to an empty vector  -  the saturation loop sees no
/// equivalences and the rule contributes nothing. When `true`, the
/// wrapped rule fires unchanged.
pub struct DeviceAwareRule<L: ENodeLang, F: Fn() -> bool> {
    inner: Box<dyn Rule<L>>,
    predicate: F,
}

impl<L: ENodeLang, F: Fn() -> bool> DeviceAwareRule<L, F> {
    /// Wrap `inner` so it only fires when `predicate()` returns true.
    pub fn new(inner: Box<dyn Rule<L>>, predicate: F) -> Self {
        Self { inner, predicate }
    }
}

impl<L: ENodeLang, F: Fn() -> bool> Rule<L> for DeviceAwareRule<L, F> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn matches(&self, egraph: &EGraph<L>) -> Vec<(EClassId, EClassId)> {
        if (self.predicate)() {
            self.inner.matches(egraph)
        } else {
            Vec::new()
        }
    }
}

/// One family's saturation result: how many iterations were spent in
/// that family's [`saturate`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilySaturationReport {
    /// Family name as returned by [`Family::name`].
    pub family: &'static str,
    /// Iterations the family actually used (≤ `budget`). 0 when the
    /// budget was 0 or when the rule set converged immediately.
    pub iters_used: usize,
    /// Budget the family was given. Echoed back so callers can compare
    /// against `iters_used` without re-querying the budget function.
    pub budget: usize,
}

/// Detailed per-family saturation telemetry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilySaturationTelemetry {
    /// Family name as returned by [`Family::name`].
    pub family: &'static str,
    /// Full saturation report for this family run.
    pub saturation: SaturationReport,
}

/// Run each family with its own iteration budget.
///
/// Saturate-per-family is the prerequisite for ROADMAP A8: a global
/// `max_iters` punishes algebraic families (which converge in 2-3 iters)
/// for sharing a budget with slow rewrite families (which may need 50+).
/// The fix is to give each family its own cap  -  algebraic gets the small
/// cap it needs, structural rewrite gets the larger one, and neither
/// starves the other.
///
/// Order: families run in the order they appear in `families`. Earlier
/// families' merges are visible to later families (the `EGraph` carries
/// state across calls). Re-running this wrapper after a third-party
/// pass mutates the `EGraph` is safe  -  each call is independent.
///
/// `budget_for` is queried once per family to allow callers to pull
/// per-family caps from a TOML config or cost model. Returning 0 skips
/// the family without running it.
pub fn saturate_per_family<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    families: &[&dyn Family<L>],
    budget_for: impl Fn(&str) -> usize,
) -> Vec<FamilySaturationReport> {
    match try_saturate_per_family(egraph, families, budget_for) {
        Ok(report) => report,
        Err(error) => {
            log_egraph_compat_error("egraph saturate_per_family", &error);
            Vec::new()
        }
    }
}

/// Fallible variant of [`saturate_per_family`].
pub fn try_saturate_per_family<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    families: &[&dyn Family<L>],
    budget_for: impl Fn(&str) -> usize,
) -> Result<Vec<FamilySaturationReport>, EGraphError> {
    let detailed = try_saturate_per_family_detailed(egraph, families, budget_for)?;
    let mut out = Vec::new();
    reserve_vec_exact(
        &mut out,
        detailed.len(),
        "egraph family saturation legacy report staging",
    )?;
    out.extend(detailed.into_iter().map(|entry| FamilySaturationReport {
        family: entry.family,
        iters_used: entry.saturation.iters_used,
        budget: entry.saturation.budget,
    }));
    Ok(out)
}

/// Run each family with its own iteration budget and return full telemetry.
pub fn saturate_per_family_detailed<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    families: &[&dyn Family<L>],
    budget_for: impl Fn(&str) -> usize,
) -> Vec<FamilySaturationTelemetry> {
    match try_saturate_per_family_detailed(egraph, families, budget_for) {
        Ok(report) => report,
        Err(error) => {
            log_egraph_compat_error("egraph saturate_per_family_detailed", &error);
            Vec::new()
        }
    }
}

/// Fallible variant of [`saturate_per_family_detailed`].
pub fn try_saturate_per_family_detailed<L: ENodeLang>(
    egraph: &mut EGraph<L>,
    families: &[&dyn Family<L>],
    budget_for: impl Fn(&str) -> usize,
) -> Result<Vec<FamilySaturationTelemetry>, EGraphError> {
    let mut out = Vec::new();
    reserve_vec_exact(
        &mut out,
        families.len(),
        "egraph family saturation telemetry staging",
    )?;
    for family in families {
        let name = family.name();
        let budget = budget_for(name);
        let rules = family.rules();
        let saturation = try_saturate_named(egraph, name, &rules, budget)?;
        out.push(FamilySaturationTelemetry {
            family: name,
            saturation,
        });
    }
    Ok(out)
}

/// Reason an extraction run stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionStopReason {
    /// The extraction cost table reached a fixed point.
    FixedPoint,
    /// The extraction loop consumed the supplied iteration cap.
    IterationBudget,
    /// The root class remained uncosted, usually because the represented term
    /// is cyclic or depends on an uncosted child class.
    MissingCost,
}

/// Executable telemetry for one extraction run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionReport<L: ENodeLang> {
    /// Root class requested by the caller.
    pub class_id: EClassId,
    /// Best node and computed cost when extraction produced a candidate.
    pub best: Option<(L, u64)>,
    /// Iterations actually executed.
    pub iters_used: usize,
    /// Caller-supplied extraction iteration budget.
    pub budget: usize,
    /// Why extraction stopped.
    pub stop_reason: ExtractionStopReason,
    /// Dense class slots visible to extraction.
    pub class_count: usize,
}

/// Extract the lowest-cost equivalent representation of `class_id` under
/// `cost_fn`. Returns the chosen `ENode` and its computed cost.
///
/// Greedy bottom-up extraction: cost of each `EClass` is the min over its
/// nodes of `cost_fn(node) + sum(cost_of_child_classes)`. Iterates to
/// fixed point on the cost map.
pub fn extract_best<L: ENodeLang>(
    egraph: &EGraph<L>,
    class_id: EClassId,
    cost_fn: impl Fn(&L) -> u64,
) -> Option<(L, u64)> {
    match try_extract_best(egraph, class_id, cost_fn) {
        Ok(best) => best,
        Err(error) => {
            log_egraph_compat_error("egraph extract_best", &error);
            None
        }
    }
}

/// Fallible variant of [`extract_best`].
pub fn try_extract_best<L: ENodeLang>(
    egraph: &EGraph<L>,
    class_id: EClassId,
    cost_fn: impl Fn(&L) -> u64,
) -> Result<Option<(L, u64)>, EGraphError> {
    try_extract_best_with_budget(egraph, class_id, cost_fn, DEFAULT_EXTRACTION_ITER_BUDGET)
        .map(|report| report.best)
}

/// Extract with a caller-supplied fixed-point iteration budget and return
/// telemetry.
pub fn try_extract_best_with_budget<L: ENodeLang>(
    egraph: &EGraph<L>,
    class_id: EClassId,
    cost_fn: impl Fn(&L) -> u64,
    iter_budget: usize,
) -> Result<ExtractionReport<L>, EGraphError> {
    // VYRE_IR_HOTSPOTS HIGH: extract_best is the inner loop of every
    // optimizer extraction (called per device per root by
    // device_extraction). The previous FxHashMap<EClassId, (L,u64)>
    // hashed-lookup'd costs three times per node per iteration
    // (canon_cid, every child, and the insert check). Class ids are
    // dense u32s in [0, class_count); a direct Vec<Option<(L,u64)>>
    // cuts every lookup to a u32 deref. Plus iter_nodes already
    // filters for canonical (parent[idx] == idx), so the find_immut
    // on `cid` was redundant work  -  drop it.
    let class_count = egraph.class_count();
    let mut costs: Vec<Option<(L, u64)>> = Vec::new();
    reserve_vec_exact(&mut costs, class_count, "egraph extraction cost table")?;
    costs.resize_with(class_count, || None);
    let mut changed = true;
    let mut iters = 0;
    while changed && iters < iter_budget {
        changed = false;
        iters += 1;
        for (cid, node) in egraph.iter_nodes() {
            // cid is already canonical  -  iter_nodes filters parent[idx] == idx.
            let canon_cid_idx = eclass_index(cid, class_count, "egraph extraction class")?;
            let mut node_cost = cost_fn(node);
            let mut child_overflow = false;
            for child in node.children() {
                let canon_child = egraph.try_find_immut(child)?;
                let canon_child_idx =
                    eclass_index(canon_child, class_count, "egraph extraction child class")?;
                if let Some((_, c)) = costs.get(canon_child_idx).and_then(Option::as_ref) {
                    node_cost = node_cost.saturating_add(*c);
                } else {
                    child_overflow = true;
                    break;
                }
            }
            if child_overflow {
                continue;
            }
            let Some(slot) = costs.get_mut(canon_cid_idx) else {
                continue;
            };
            match slot {
                Some((_, existing_cost)) if *existing_cost <= node_cost => {}
                _ => {
                    *slot = Some((node.clone(), node_cost));
                    changed = true;
                }
            }
        }
    }
    let canon = egraph.try_find_immut(class_id)?;
    let canon_idx = eclass_index(canon, class_count, "egraph extraction root class")?;
    let best = costs.get(canon_idx).and_then(Clone::clone);
    let stop_reason = if changed {
        ExtractionStopReason::IterationBudget
    } else if best.is_some() {
        ExtractionStopReason::FixedPoint
    } else {
        ExtractionStopReason::MissingCost
    };
    Ok(ExtractionReport {
        class_id,
        best,
        iters_used: iters,
        budget: iter_budget,
        stop_reason,
        class_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::FxHashSet;
    use smallvec::smallvec;

    /// A minimal arithmetic `ENode` language for engine tests.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum Arith {
        Const(u32),
        Add(EClassId, EClassId),
        Mul(EClassId, EClassId),
    }

    impl ENodeLang for Arith {
        fn children(&self) -> EChildren {
            match self {
                Self::Const(_) => EChildren::new(),
                Self::Add(a, b) | Self::Mul(a, b) => smallvec![*a, *b],
            }
        }

        fn with_children(&self, children: &[EClassId]) -> Self {
            match self {
                Self::Const(n) => Self::Const(*n),
                Self::Add(_, _) => Self::Add(children[0], children[1]),
                Self::Mul(_, _) => Self::Mul(children[0], children[1]),
            }
        }
    }

    /// Simple cost: 1 per Const, 2 per Add, 3 per Mul.
    fn arith_cost(node: &Arith) -> u64 {
        match node {
            Arith::Const(_) => 1,
            Arith::Add(_, _) => 2,
            Arith::Mul(_, _) => 3,
        }
    }

    #[test]
    fn empty_egraph_has_zero_classes() {
        let egraph: EGraph<Arith> = EGraph::new();
        assert_eq!(egraph.class_count(), 0);
    }

    #[test]
    fn add_const_creates_one_class() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _ = egraph.add(Arith::Const(7));
        assert_eq!(egraph.class_count(), 1);
    }

    #[test]
    fn add_same_const_twice_returns_same_class() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(7));
        let b = egraph.add(Arith::Const(7));
        assert_eq!(a, b);
        assert_eq!(egraph.class_count(), 1);
    }

    #[test]
    fn add_distinct_consts_creates_distinct_classes() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(7));
        let b = egraph.add(Arith::Const(8));
        assert_ne!(a, b);
        assert_eq!(egraph.class_count(), 2);
    }

    #[test]
    fn add_compound_node_creates_proper_class() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(1));
        let b = egraph.add(Arith::Const(2));
        let sum = egraph.add(Arith::Add(a, b));
        assert_eq!(egraph.class_count(), 3);
        assert_ne!(sum, a);
        assert_ne!(sum, b);
    }

    #[test]
    fn union_merges_two_classes() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(1));
        let b = egraph.add(Arith::Const(2));
        let unified = egraph.union(a, b);
        assert_eq!(egraph.find(a), unified);
        assert_eq!(egraph.find(b), unified);
    }

    #[test]
    fn union_is_idempotent() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(1));
        let b = egraph.add(Arith::Const(2));
        let first = egraph.union(a, b);
        let second = egraph.union(a, b);
        assert_eq!(first, second);
    }

    #[test]
    fn rebuild_canonicalizes_compound_nodes_after_union() {
        // Build (1 + 2). Union 1 and 2. After rebuild, two adds that look
        // structurally different should canonicalize to the same form.
        let mut egraph: EGraph<Arith> = EGraph::new();
        let one = egraph.add(Arith::Const(1));
        let two = egraph.add(Arith::Const(2));
        let _add_12 = egraph.add(Arith::Add(one, two));
        let _add_22 = egraph.add(Arith::Add(two, two));
        egraph.union(one, two);
        let _ = egraph.rebuild();
        // After rebuild, Add(1,2) and Add(2,2) canonicalize to the same
        // pair of children → same EClass.
        let post_one = egraph.find(one);
        let post_two = egraph.find(two);
        assert_eq!(post_one, post_two, "1 and 2 must be in the same class");
    }

    #[test]
    fn extract_best_picks_cheapest_equivalent() {
        // Build two equivalent representations: Add(1, 2) and Const(3).
        // Equate them. Extract should pick Const(3) (cost 1) over Add (cost 4).
        let mut egraph: EGraph<Arith> = EGraph::new();
        let one = egraph.add(Arith::Const(1));
        let two = egraph.add(Arith::Const(2));
        let three = egraph.add(Arith::Const(3));
        let add_12 = egraph.add(Arith::Add(one, two));
        egraph.union(add_12, three);
        let _ = egraph.rebuild();
        let (best, cost) = extract_best(&egraph, add_12, arith_cost).expect("Fix: must extract");
        assert_eq!(best, Arith::Const(3));
        assert_eq!(cost, 1);
    }

    #[test]
    fn eqsat_extraction_report_records_budget_stop_reason_class_count_and_cost() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let one = egraph.add(Arith::Const(1));
        let two = egraph.add(Arith::Const(2));
        let three = egraph.add(Arith::Const(3));
        let add_12 = egraph.add(Arith::Add(one, two));
        egraph.union(add_12, three);
        let _ = egraph.rebuild();
        let report = try_extract_best_with_budget(&egraph, add_12, arith_cost, 16)
            .expect("Fix: valid extraction report must be produced");
        let (best, cost) = report
            .best
            .expect("Fix: equivalent constant must be extractable");
        assert_eq!(best, Arith::Const(3));
        assert_eq!(cost, 1);
        assert_eq!(report.class_id, add_12);
        assert_eq!(report.budget, 16);
        assert!(report.iters_used > 0 && report.iters_used <= 16);
        assert_eq!(report.stop_reason, ExtractionStopReason::FixedPoint);
        assert_eq!(report.class_count, egraph.class_count());
    }

    #[test]
    fn eqsat_extraction_zero_budget_reports_budget_stop_without_best() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let class_id = egraph.add(Arith::Const(42));
        let report = try_extract_best_with_budget(&egraph, class_id, arith_cost, 0)
            .expect("Fix: valid zero-budget extraction report must be produced");
        assert_eq!(report.class_id, class_id);
        assert_eq!(report.best, None);
        assert_eq!(report.iters_used, 0);
        assert_eq!(report.budget, 0);
        assert_eq!(report.stop_reason, ExtractionStopReason::IterationBudget);
        assert_eq!(report.class_count, egraph.class_count());
    }

    #[test]
    fn extract_best_returns_only_node_when_no_alternatives() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(42));
        let (best, cost) = extract_best(&egraph, a, arith_cost).expect("Fix: must extract");
        assert_eq!(best, Arith::Const(42));
        assert_eq!(cost, 1);
    }

    /// Simpler test rule: union every two `Const(a)` with `Const(a)` (idempotent).
    /// Used to verify `saturate` calls `matches` and `rebuild` correctly.
    struct UnionEqualConstsRule;

    impl Rule<Arith> for UnionEqualConstsRule {
        fn name(&self) -> &'static str {
            "union_equal_consts"
        }

        fn matches(&self, egraph: &EGraph<Arith>) -> Vec<(EClassId, EClassId)> {
            let mut by_value: FxHashMap<u32, Vec<EClassId>> = FxHashMap::default();
            for (cid, node) in egraph.iter_nodes() {
                if let Arith::Const(v) = node {
                    by_value.entry(*v).or_default().push(cid);
                }
            }
            let mut out = Vec::new();
            for ids in by_value.values() {
                for window in ids.windows(2) {
                    out.push((window[0], window[1]));
                }
            }
            out
        }
    }

    #[test]
    fn saturate_runs_to_fixed_point() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        // Hashcons should already prevent two Const(7)s, but this exercises
        // the saturate loop end-to-end with a real rule.
        let _a = egraph.add(Arith::Const(7));
        let _b = egraph.add(Arith::Const(8));
        let rules: Vec<Box<dyn Rule<Arith>>> = vec![Box::new(UnionEqualConstsRule)];
        let iters = saturate(&mut egraph, &rules, 10);
        assert!(iters <= 10);
        // No new equivalences past the first iter (hashcons already
        // dedupes), so saturate returns 0 or 1.
        assert!(iters <= 1);
    }

    #[test]
    fn eqsat_saturation_report_records_fixed_point_class_count_and_family() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _a = egraph.add(Arith::Const(7));
        let _b = egraph.add(Arith::Const(8));
        let rules: Vec<Box<dyn Rule<Arith>>> = vec![Box::new(UnionEqualConstsRule)];
        let report = try_saturate_named(&mut egraph, "arith_identities", &rules, 10)
            .expect("Fix: valid saturation report must be produced");
        assert_eq!(report.rewrite_family, "arith_identities");
        assert_eq!(report.rule_count, 1);
        assert_eq!(report.stop_reason, SaturationStopReason::FixedPoint);
        assert_eq!(report.class_count_before, 2);
        assert_eq!(report.class_count_after, egraph.class_count());
        assert_eq!(report.class_count_delta(), 0);
        assert_eq!(report.applied_equivalences, 0);
        assert_eq!(report.rebuild_unions, 0);
    }

    #[test]
    fn find_immut_returns_canonical_after_union() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(1));
        let b = egraph.add(Arith::Const(2));
        egraph.union(a, b);
        // find_immut must agree with find.
        let canon_a = egraph.find_immut(a);
        let canon_b = egraph.find_immut(b);
        assert_eq!(canon_a, canon_b);
    }

    #[test]
    fn class_lookup_returns_canonical_class() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(7));
        let class = egraph.class(a).expect("Fix: class must exist");
        assert!(matches!(class.nodes[0], Arith::Const(7)));
    }

    #[test]
    fn rebuild_propagates_through_parents() {
        // Build Add(1, 2). Union 1 and 2. After rebuild, Add(1,2) should
        // canonicalize to Add(1,1) (or whichever survived).
        let mut egraph: EGraph<Arith> = EGraph::new();
        let one = egraph.add(Arith::Const(1));
        let two = egraph.add(Arith::Const(2));
        let add_12 = egraph.add(Arith::Add(one, two));
        egraph.union(one, two);
        let _ = egraph.rebuild();
        // The Add(1,2) class should still be findable, and its node should
        // now reference the unified child class.
        let class = egraph.class(add_12).expect("Fix: class must still exist");
        match &class.nodes[0] {
            Arith::Add(a, b) => {
                let canon_a = egraph.find_immut(*a);
                let canon_b = egraph.find_immut(*b);
                assert_eq!(
                    canon_a, canon_b,
                    "Add(1,2)'s children must canonicalize to the same class after union"
                );
            }
            other => panic!("expected Add; got {other:?}"),
        }
    }

    /// Rule that pairs every Const id with itself  -  guaranteed to
    /// produce at least one match whenever the egraph holds any Const.
    /// Used purely as a forwarding-test fixture.
    struct PairConstSelfRule;

    impl Rule<Arith> for PairConstSelfRule {
        fn name(&self) -> &'static str {
            "pair_const_self"
        }

        fn matches(&self, egraph: &EGraph<Arith>) -> Vec<(EClassId, EClassId)> {
            let mut out = Vec::new();
            for (cid, node) in egraph.iter_nodes() {
                if let Arith::Const(_) = node {
                    out.push((cid, cid));
                }
            }
            out
        }
    }

    struct ForeignClassRule;

    impl Rule<Arith> for ForeignClassRule {
        fn name(&self) -> &'static str {
            "foreign_class"
        }

        fn matches(&self, _egraph: &EGraph<Arith>) -> Vec<(EClassId, EClassId)> {
            vec![(EClassId(999), EClassId(999))]
        }
    }

    #[test]
    fn eqsat_saturation_report_exposes_budget_stop_without_silent_fallback() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _ = egraph.add(Arith::Const(7));
        let rules: Vec<Box<dyn Rule<Arith>>> = vec![Box::new(PairConstSelfRule)];
        let report = try_saturate_named(&mut egraph, "self_pairs", &rules, 2)
            .expect("Fix: valid budgeted saturation report must be produced");
        assert_eq!(report.rewrite_family, "self_pairs");
        assert_eq!(report.iters_used, 2);
        assert_eq!(report.budget, 2);
        assert_eq!(report.stop_reason, SaturationStopReason::IterationBudget);
        assert_eq!(report.class_count_before, 1);
        assert_eq!(report.class_count_after, egraph.class_count());
        assert_eq!(report.applied_equivalences, 2);

        let invalid_rules: Vec<Box<dyn Rule<Arith>>> = vec![Box::new(ForeignClassRule)];
        let err = try_saturate_named(&mut egraph, "invalid_foreign", &invalid_rules, 1)
            .expect_err("Fix: fallible saturation must reject invalid equivalence ids");
        assert!(
            matches!(
                err,
                EGraphError::ClassIdOutOfBounds {
                    context: "egraph find",
                    id: EClassId(999),
                    len: 1
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn device_aware_rule_predicate_true_forwards_matches() {
        // First half: with no Consts, even the always-on inner rule
        // produces no matches. The forwarder must propagate that.
        let egraph: EGraph<Arith> = EGraph::new();
        let inner: Box<dyn Rule<Arith>> = Box::new(PairConstSelfRule);
        let rule = DeviceAwareRule::new(inner, || true);
        assert!(
            rule.matches(&egraph).is_empty(),
            "empty egraph must yield empty matches even with predicate true"
        );

        // Second half: add a Const and confirm the predicate-true
        // forwarder surfaces the inner rule's hits.
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _a = egraph.add(Arith::Const(7));
        let inner: Box<dyn Rule<Arith>> = Box::new(PairConstSelfRule);
        let rule = DeviceAwareRule::new(inner, || true);
        assert!(
            !rule.matches(&egraph).is_empty(),
            "predicate true must forward the inner rule's matches"
        );
    }

    #[test]
    fn device_aware_rule_predicate_false_returns_empty() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _ = egraph.add(Arith::Const(7));
        let _ = egraph.add(Arith::Const(7)); // hashcons collapses, but rule loop still scans
        let inner: Box<dyn Rule<Arith>> = Box::new(UnionEqualConstsRule);
        let rule = DeviceAwareRule::new(inner, || false);
        let matches = rule.matches(&egraph);
        assert!(
            matches.is_empty(),
            "predicate false must short-circuit to empty"
        );
    }

    #[test]
    fn device_aware_rule_forwards_inner_name() {
        let inner: Box<dyn Rule<Arith>> = Box::new(UnionEqualConstsRule);
        let rule = DeviceAwareRule::new(inner, || true);
        assert_eq!(rule.name(), "union_equal_consts");
    }

    /// A toy family with one rule, used for the per-family budget tests.
    struct ConstUnionFamily {
        name: &'static str,
    }

    impl Family<Arith> for ConstUnionFamily {
        fn name(&self) -> &'static str {
            self.name
        }
        fn rules(&self) -> Vec<Box<dyn Rule<Arith>>> {
            vec![Box::new(UnionEqualConstsRule)]
        }
    }

    #[test]
    fn saturate_per_family_skips_zero_budget() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _ = egraph.add(Arith::Const(7));
        let fam = ConstUnionFamily { name: "f0" };
        let report = saturate_per_family(&mut egraph, &[&fam], |_| 0);
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].family, "f0");
        assert_eq!(report[0].iters_used, 0);
        assert_eq!(report[0].budget, 0);
    }

    #[test]
    fn saturate_per_family_runs_each_family_independently() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _ = egraph.add(Arith::Const(1));
        let _ = egraph.add(Arith::Const(2));
        let fam_a = ConstUnionFamily { name: "alpha" };
        let fam_b = ConstUnionFamily { name: "beta" };
        let report = saturate_per_family(&mut egraph, &[&fam_a, &fam_b], |name| match name {
            "alpha" => 3,
            "beta" => 5,
            _ => 0,
        });
        assert_eq!(report.len(), 2);
        assert_eq!(report[0].family, "alpha");
        assert_eq!(report[0].budget, 3);
        assert!(report[0].iters_used <= 3);
        assert_eq!(report[1].family, "beta");
        assert_eq!(report[1].budget, 5);
        assert!(report[1].iters_used <= 5);
    }

    #[test]
    fn eqsat_per_family_detailed_report_keeps_family_identity_and_stop_reason() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _ = egraph.add(Arith::Const(1));
        let fam = ConstUnionFamily { name: "alpha" };
        let report = try_saturate_per_family_detailed(&mut egraph, &[&fam], |_| 0)
            .expect("Fix: detailed per-family report must be produced");
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].family, "alpha");
        assert_eq!(report[0].saturation.rewrite_family, "alpha");
        assert_eq!(report[0].saturation.budget, 0);
        assert_eq!(
            report[0].saturation.stop_reason,
            SaturationStopReason::ZeroBudget
        );
        assert_eq!(report[0].saturation.class_count_after, egraph.class_count());
    }

    #[test]
    fn saturate_per_family_empty_input_returns_empty() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let report = saturate_per_family(&mut egraph, &[], |_| 10);
        assert!(report.is_empty());
    }

    #[test]
    fn saturate_per_family_reports_iters_used_le_budget() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let _ = egraph.add(Arith::Const(1));
        let _ = egraph.add(Arith::Const(2));
        let fam = ConstUnionFamily { name: "single" };
        let report = saturate_per_family(&mut egraph, &[&fam], |_| 100);
        assert_eq!(report.len(), 1);
        assert!(
            report[0].iters_used <= report[0].budget,
            "iters_used ({}) must not exceed budget ({})",
            report[0].iters_used,
            report[0].budget
        );
    }

    #[test]
    fn iter_nodes_visits_only_canonical_classes() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let a = egraph.add(Arith::Const(1));
        let b = egraph.add(Arith::Const(2));
        egraph.union(a, b);
        let _ = egraph.rebuild();
        // iter_nodes yields one entry per (class, node) pair. After union,
        // the loser class is filtered out (its parent points elsewhere),
        // but the merged winner class holds both Const(1) and Const(2)
        // nodes. So the canonical-class set has size 1, but the (class,
        // node) entry count is 2.
        let unique_classes: FxHashSet<EClassId> = egraph.iter_nodes().map(|(cid, _)| cid).collect();
        assert_eq!(
            unique_classes.len(),
            1,
            "post-union iter must visit exactly one canonical class id"
        );
        let total_nodes = egraph.iter_nodes().count();
        assert_eq!(
            total_nodes, 2,
            "the merged class still holds both original nodes (Const(1) + Const(2))"
        );
    }

    #[test]
    fn fallible_find_reports_foreign_class_id() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let err = egraph
            .try_find(EClassId(0))
            .expect_err("empty graph must reject foreign class id 0");
        assert!(
            matches!(
                err,
                EGraphError::ClassIdOutOfBounds {
                    context: "egraph find",
                    id: EClassId(0),
                    len: 0
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fallible_add_rejects_foreign_child_ids() {
        let mut egraph: EGraph<Arith> = EGraph::new();
        let err = egraph
            .try_add(Arith::Add(EClassId(0), EClassId(0)))
            .expect_err("foreign children must be rejected before insertion");
        assert!(
            matches!(
                err,
                EGraphError::ClassIdOutOfBounds {
                    context: "egraph immutable find",
                    id: EClassId(0),
                    len: 0
                }
            ),
            "unexpected error: {err}"
        );
        assert_eq!(
            egraph.class_count(),
            0,
            "failed fallible insertion must not allocate a partial class"
        );
    }

    #[test]
    fn fallible_add_handles_duplicate_children_without_late_allocation_path() {
        let mut egraph: EGraph<Arith> = EGraph::try_with_capacity(2)
            .expect("Fix: unit-test oracle precondition - small egraph reservation must succeed");
        let one = egraph
            .try_add(Arith::Const(1))
            .expect("Fix: unit-test oracle precondition - const insert must succeed");
        let add = egraph
            .try_add(Arith::Add(one, one))
            .expect("Fix: unit-test oracle precondition - duplicate child registration must be pre-reserved");
        let class = egraph
            .try_class(add)
            .expect("Fix: unit-test oracle precondition - class lookup must be valid")
            .expect("Fix: unit-test oracle precondition - class must exist");
        assert!(matches!(class.nodes[0], Arith::Add(_, _)));
    }

    #[test]
    fn try_class_id_from_index_rejects_overflow() {
        if usize::BITS <= u32::BITS {
            return;
        }
        let overflow_index = (u32::MAX as usize) + 1;
        let err = try_eclass_id_from_index(overflow_index)
            .expect_err("overflowing class index must be rejected");
        assert_eq!(
            err,
            EGraphError::ClassIdOverflow {
                index: overflow_index
            }
        );
    }

    #[test]
    fn fallible_saturate_and_extract_match_infallible_contracts() {
        let mut egraph: EGraph<Arith> = EGraph::try_with_capacity(4)
            .expect("Fix: unit-test oracle precondition - small egraph reservation must succeed");
        let one = egraph
            .try_add(Arith::Const(1))
            .expect("Fix: unit-test oracle precondition - insert one");
        let two = egraph
            .try_add(Arith::Const(2))
            .expect("Fix: unit-test oracle precondition - insert two");
        let three = egraph
            .try_add(Arith::Const(3))
            .expect("Fix: unit-test oracle precondition - insert three");
        let add_12 = egraph
            .try_add(Arith::Add(one, two))
            .expect("Fix: unit-test oracle precondition - insert add");
        egraph
            .try_union(add_12, three)
            .expect("Fix: unit-test oracle precondition - union equivalent nodes");
        egraph
            .try_rebuild()
            .expect("Fix: unit-test oracle precondition - rebuild equivalent nodes");
        let rules: Vec<Box<dyn Rule<Arith>>> = vec![Box::new(UnionEqualConstsRule)];
        let iters = try_saturate(&mut egraph, &rules, 10)
            .expect("Fix: unit-test oracle precondition - fallible saturation");
        assert!(iters <= 10);
        let (best, cost) = try_extract_best(&egraph, add_12, arith_cost)
            .expect("Fix: unit-test oracle precondition - fallible extraction")
            .expect("Fix: unit-test oracle precondition - best node must exist");
        assert_eq!(best, Arith::Const(3));
        assert_eq!(cost, 1);
    }

    #[test]
    fn eqsat_production_uses_fallible_staging_and_checked_ids() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/optimizer/eqsat.rs"
        ))
        .expect("Fix: eqsat source must be readable");
        let production = src
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: unit-test oracle precondition - production section must exist");
        assert!(
            !production.contains("unwrap_or(u32::MAX)"),
            "class ids must never saturate to a poisoned sentinel"
        );
        assert!(
            !production.contains("Vec::with_capacity("),
            "optimizer staging must use fallible reservations"
        );
        assert!(
            !production.contains("with_capacity_and_hasher("),
            "hashcons staging must use fallible reservations"
        );
        assert!(
            !production.contains(" as usize"),
            "EClassId indexing must go through checked conversion helpers"
        );
        assert!(production.contains("try_with_capacity"));
        assert!(production.contains("try_saturate"));
        assert!(production.contains("try_extract_best"));
        assert!(
            !production.contains(".expect("),
            "legacy egraph compatibility APIs must not panic in production; callers that need hard errors use try_*"
        );
    }
}

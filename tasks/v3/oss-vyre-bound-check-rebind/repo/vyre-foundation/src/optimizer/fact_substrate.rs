//! Shared incremental/cached fact substrate for optimizer passes.
//!
//! Replaces ad-hoc per-pass re-computation (use-count tables, shape-fact
//! walks, type-inference maps) with a single derived structure that is
//! invalidated when the program changes.
//!
//! # Design
//!
//! * **Shape facts**  -  [`ProgramShapeFacts`] per buffer (already existed).
//! * **Use facts**  -  variable-use counts and buffer read/write sets.
//! * **Type facts**  -  best-effort expression-type map for float/int
//!   discrimination (used by FMA synthesis and vectorization).
//!
//! The substrate is keyed by the canonical 256-bit program fingerprint so
//! stale entries are never reused across fixpoint iterations.

use crate::ir::{Ident, Program};
use crate::optimizer::program_shape_facts::ProgramShapeFacts;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

mod type_facts;
mod use_facts;

use self::use_facts::derive_use_facts;

/// Typed fact partitions exposed by the optimizer fact substrate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum FactPartition {
    /// Graph/topology facts derived from shape and use dependency data.
    Graph,
    /// Scalar and expression type facts.
    Type,
    /// Static per-buffer shape facts.
    Shape,
    /// Variable and buffer use facts.
    Use,
    /// Side-effect and write-dependency facts backed by use analysis.
    Effects,
    /// Dataflow frontier dependencies backed by use analysis.
    DataflowFrontier,
}

impl FactPartition {
    pub(crate) const ALL: [Self; 6] = [
        Self::Graph,
        Self::Type,
        Self::Shape,
        Self::Use,
        Self::Effects,
        Self::DataflowFrontier,
    ];
}

/// Unified fact cache for a single program revision.
///
/// Passes that need shape, use, or type information call
/// [`FactSubstrate::derive`] once, then read the cached fields.  When a
/// pass mutates the program the scheduler calls [`FactSubstrate::invalidate`]
/// so the next reader re-derives.
#[derive(Default, Clone, Debug)]
pub struct FactSubstrate {
    /// Canonical fingerprint of the program these facts describe.
    fingerprint: [u8; 32],
    /// Per-buffer static shape facts.
    pub shape: Option<Arc<ProgramShapeFacts>>,
    /// Shared use facts derived in one walk over the program body.
    pub use_facts: Option<Arc<UseFacts>>,
    /// Per-variable use counts across the whole program entry.
    pub use_counts: Option<Arc<FxHashMap<Ident, usize>>>,
    /// Inferred scalar types for variables and expressions.
    pub type_map: Option<Arc<TypeFacts>>,
}

/// Best-effort type-inference results.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct TypeFacts {
    /// Inferred type for a variable binding.
    pub var_types: FxHashMap<Ident, crate::ir::DataType>,
    /// Inferred type for an expression (keyed by structural hash).
    pub expr_types: FxHashMap<u64, crate::ir::DataType>,
}

/// Optimizer facts derived from value uses and buffer accesses.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct UseFacts {
    /// Number of times each variable is referenced.
    pub var_counts: Arc<FxHashMap<Ident, usize>>,
    /// Number of read-side references for each buffer.
    pub buffer_reads: FxHashMap<Ident, usize>,
    /// Number of write-side references for each buffer.
    pub buffer_writes: FxHashMap<Ident, usize>,
    /// Index-expression axis usage per buffer: `[x, y, z]`.
    pub buffer_index_axes: FxHashMap<Ident, [usize; 3]>,
    /// Conservative transitive source-buffer dependencies for scalar bindings.
    pub var_buffer_deps: FxHashMap<Ident, FxHashSet<Ident>>,
    /// Conservative direct source-buffer dependencies for each written buffer.
    pub buffer_write_deps: FxHashMap<Ident, FxHashSet<Ident>>,
    /// Buffers used as indirect-dispatch count inputs.
    pub indirect_dispatch_buffers: FxHashSet<Ident>,
    /// True when opaque IR prevents complete static dependency recovery.
    pub has_opaque: bool,
}

impl UseFacts {
    /// Most common invocation/local axis used to index `buffer`.
    #[must_use]
    pub fn dominant_index_axis(&self, buffer: &Ident) -> Option<u8> {
        let axes = self.buffer_index_axes.get(buffer)?;
        axes.iter()
            .enumerate()
            .max_by_key(|&(axis, count)| (*count, std::cmp::Reverse(axis)))
            .and_then(|(axis, count)| (*count > 0).then(|| u8::try_from(axis).ok()))
            .flatten()
    }

    /// Total observed read/write references for `buffer`.
    #[must_use]
    pub fn access_count(&self, buffer: &Ident) -> usize {
        self.buffer_reads.get(buffer).copied().unwrap_or(0)
            + self.buffer_writes.get(buffer).copied().unwrap_or(0)
    }
}

// Thread-local cache of the most recently derived FactSubstrate, keyed by
// the program's 32-byte canonical fingerprint. Three slots so a pass that
// needs shape+use facts on iteration N does not invalidate the use-only
// cache that the next pass on iteration N+1 reuses. Each slot stores the
// fully-populated FactSubstrate; the `_*` variants narrow on read by
// returning a clone with the unrequested fields cleared. All FactSubstrate
// fields are `Arc`, so the clone is a handful of refcount bumps  -  never a
// deep copy.
thread_local! {
    static FACT_SUBSTRATE_CACHE_FULL: std::cell::RefCell<Option<([u8; 32], FactSubstrate)>> =
        const { std::cell::RefCell::new(None) };
    static FACT_SUBSTRATE_CACHE_SHAPE_USE: std::cell::RefCell<Option<([u8; 32], FactSubstrate)>> =
        const { std::cell::RefCell::new(None) };
    static FACT_SUBSTRATE_CACHE_USE_ONLY: std::cell::RefCell<Option<([u8; 32], FactSubstrate)>> =
        const { std::cell::RefCell::new(None) };
}

impl FactSubstrate {
    /// Derive all facts for `program`.
    #[must_use]
    pub fn derive(program: &Program) -> Self {
        let fp = program.fingerprint();
        let use_facts = derive_use_facts(program);
        Self {
            fingerprint: fp,
            shape: Some(Arc::new(ProgramShapeFacts::derive(program))),
            use_counts: Some(Arc::clone(&use_facts.var_counts)),
            use_facts: Some(Arc::new(use_facts)),
            type_map: Some(Arc::new(type_facts::derive(program))),
        }
    }

    /// Cached counterpart of [`Self::derive`]. Returns the previous result
    /// for the same program fingerprint when available, otherwise computes
    /// once and stashes the result in a thread-local cache. Same logical
    /// payload as `derive`  -  the cache is purely a redundant-walk avoider.
    #[must_use]
    pub fn derive_cached(program: &Program) -> Self {
        let fp = program.fingerprint();
        FACT_SUBSTRATE_CACHE_FULL.with(|cell| {
            if let Some((cached_fp, ref cached)) = *cell.borrow() {
                if cached_fp == fp {
                    return cached.clone();
                }
            }
            let fresh = Self::derive(program);
            *cell.borrow_mut() = Some((fp, fresh.clone()));
            fresh
        })
    }

    /// Derive shape and use facts without running type inference.
    #[must_use]
    pub fn derive_shape_and_use(program: &Program) -> Self {
        let fp = program.fingerprint();
        let use_facts = derive_use_facts(program);
        Self {
            fingerprint: fp,
            shape: Some(Arc::new(ProgramShapeFacts::derive(program))),
            use_counts: Some(Arc::clone(&use_facts.var_counts)),
            use_facts: Some(Arc::new(use_facts)),
            type_map: None,
        }
    }

    /// Cached counterpart of [`Self::derive_shape_and_use`]. See
    /// [`Self::derive_cached`] for the caching contract.
    #[must_use]
    pub fn derive_shape_and_use_cached(program: &Program) -> Self {
        let fp = program.fingerprint();
        FACT_SUBSTRATE_CACHE_SHAPE_USE.with(|cell| {
            if let Some((cached_fp, ref cached)) = *cell.borrow() {
                if cached_fp == fp {
                    return cached.clone();
                }
            }
            let fresh = Self::derive_shape_and_use(program);
            *cell.borrow_mut() = Some((fp, fresh.clone()));
            fresh
        })
    }

    /// Derive only use facts for passes that do not need shape or type maps.
    #[must_use]
    pub fn derive_use_only(program: &Program) -> Self {
        let use_facts = derive_use_facts(program);
        Self {
            fingerprint: program.fingerprint(),
            shape: None,
            use_counts: Some(Arc::clone(&use_facts.var_counts)),
            use_facts: Some(Arc::new(use_facts)),
            type_map: None,
        }
    }

    /// Cached counterpart of [`Self::derive_use_only`]. See
    /// [`Self::derive_cached`] for the caching contract.
    #[must_use]
    pub fn derive_use_only_cached(program: &Program) -> Self {
        let fp = program.fingerprint();
        FACT_SUBSTRATE_CACHE_USE_ONLY.with(|cell| {
            if let Some((cached_fp, ref cached)) = *cell.borrow() {
                if cached_fp == fp {
                    return cached.clone();
                }
            }
            let fresh = Self::derive_use_only(program);
            *cell.borrow_mut() = Some((fp, fresh.clone()));
            fresh
        })
    }

    /// Drop every cached fact. Called by the scheduler after a pass
    /// changes the program.
    pub fn invalidate(&mut self) {
        self.invalidate_partition(FactPartition::Shape);
        self.invalidate_partition(FactPartition::Use);
        self.invalidate_partition(FactPartition::Type);
    }

    /// Drop one typed partition and every cached view that depends on it.
    pub(crate) fn invalidate_partition(&mut self, partition: FactPartition) {
        match partition {
            FactPartition::Graph => {
                self.invalidate_shape();
                self.invalidate_use_facts();
            }
            FactPartition::Type => self.invalidate_type_map(),
            FactPartition::Shape => self.invalidate_shape(),
            FactPartition::Use | FactPartition::Effects | FactPartition::DataflowFrontier => {
                self.invalidate_use_facts();
            }
        }
    }

    /// Drop only shape facts.
    pub fn invalidate_shape(&mut self) {
        self.shape = None;
    }

    /// Drop only use facts.
    pub fn invalidate_use_facts(&mut self) {
        self.use_facts = None;
        self.use_counts = None;
    }

    /// Drop only type facts.
    pub fn invalidate_type_map(&mut self) {
        self.type_map = None;
    }

    /// True when the cached facts are known to match `program`.
    #[must_use]
    pub fn is_fresh_for(&self, program: &Program) -> bool {
        self.fresh_partitions_for(program).len() == FactPartition::ALL.len()
    }

    /// True when cached use facts match `program`.
    #[must_use]
    pub fn has_fresh_use_facts_for(&self, program: &Program) -> bool {
        self.has_fresh_partition_for(program, FactPartition::Use)
    }

    /// True when cached shape and use facts match `program`.
    #[must_use]
    pub fn has_fresh_shape_and_use_for(&self, program: &Program) -> bool {
        self.fingerprint == program.fingerprint()
            && self.has_partition(FactPartition::Shape)
            && self.has_partition(FactPartition::Use)
    }

    /// True when a typed fact partition is present, independent of fingerprint.
    #[must_use]
    pub(crate) fn has_partition(&self, partition: FactPartition) -> bool {
        match partition {
            FactPartition::Graph => self.shape.is_some() && self.use_facts.is_some(),
            FactPartition::Type => self.type_map.is_some(),
            FactPartition::Shape => self.shape.is_some(),
            FactPartition::Use => self.use_facts.is_some() && self.use_counts.is_some(),
            FactPartition::Effects | FactPartition::DataflowFrontier => self.use_facts.is_some(),
        }
    }

    /// True when a typed fact partition is present and matches `program`.
    #[must_use]
    pub(crate) fn has_fresh_partition_for(
        &self,
        program: &Program,
        partition: FactPartition,
    ) -> bool {
        self.fingerprint == program.fingerprint() && self.has_partition(partition)
    }

    /// All typed fact partitions currently fresh for `program`.
    #[must_use]
    pub(crate) fn fresh_partitions_for(&self, program: &Program) -> Vec<FactPartition> {
        FactPartition::ALL
            .iter()
            .copied()
            .filter(|partition| self.has_fresh_partition_for(program, *partition))
            .collect()
    }

    /// Shared use-fact lookup.
    #[must_use]
    pub fn use_facts(&self) -> Option<&UseFacts> {
        self.use_facts.as_deref()
    }

    /// Shared use-count lookup.
    #[must_use]
    pub fn use_counts(&self) -> Option<&FxHashMap<Ident, usize>> {
        self.use_counts.as_deref()
    }

    /// Number of uses for a variable, defaulting to `0`.
    #[must_use]
    pub fn use_count_of(&self, name: &Ident) -> usize {
        self.use_facts()
            .and_then(|facts| facts.var_counts.get(name))
            .copied()
            .or_else(|| self.use_counts().and_then(|m| m.get(name)).copied())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Use-count derivation
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;

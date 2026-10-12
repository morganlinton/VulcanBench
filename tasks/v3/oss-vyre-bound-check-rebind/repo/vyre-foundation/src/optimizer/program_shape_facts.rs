//! Program-level static shape facts derived from `BufferDecl`s.
//!
//! Audit P0 #38: replaces ad-hoc shape recomputations across optimizer,
//! lowering, and validation with one derived analysis. Every `Program`
//! produces exactly one `ProgramShapeFacts`; passes consume it instead of
//! walking `BufferDecl`s themselves.
//!
//! This module composes the predicate-level query API in
//! [`crate::optimizer::shape_facts`] with the structural data on each
//! `BufferDecl` (declared `count`, `DataType::size_bytes()`, the optional
//! `output_byte_range`, and the `bytes_extraction` opt-in) so a pass can
//! ask one analysis "what bytes can flow through buffer X?" without
//! re-implementing the math.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::ir_inner::model::expr::Ident;
use crate::ir_inner::model::program::Program;
use rustc_hash::FxHashMap;
use vyre_spec::data_type::DataType;

use super::shape_facts;

/// Cache key for stashing the derived facts on
/// [`crate::optimizer::AnalysisCache`]. Passes look up the analysis by
/// this key so every consumer reads the same derived map.
pub const ANALYSIS_KEY: &str = "program_shape_facts";
const SHAPE_FACT_CACHE_CAP: usize = 64;

/// Static facts the optimizer trusts about a single buffer.
///
/// Every `BufferDecl` in a program has a `BufferShapeFacts`. Passes can
/// `match` on the contents to make typed decisions; the fields are
/// monotonically derived (no source of nondeterminism) so two
/// derivations of the same `Program` always produce the same facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferShapeFacts {
    /// Buffer name (matches `Expr::Load.buffer`, `Node::Store.buffer`, etc.).
    pub name: Ident,
    /// `@binding(N)` slot.
    pub binding: u32,
    /// Element data type.
    pub dtype: DataType,
    /// Element count declared on the `BufferDecl`. `0` means runtime-sized.
    pub declared_count: u32,
    /// Fixed scalar element size in bytes, or `None` for variable-size types
    /// (`DataType::Tensor`, sparse families, opaque).
    pub element_size_bytes: Option<usize>,
    /// Tightest min count proved by the `BufferDecl`'s count + `ShapePredicate`.
    /// `0` when no positive lower bound is provable.
    pub min_count: u32,
    /// Tightest max count proved by the `BufferDecl`'s count + `ShapePredicate`.
    /// `None` when no upper bound is provable.
    pub max_count: Option<u32>,
    /// True when both min and max bound to the same value (equivalent to a
    /// `ShapePredicate::Exactly(n)` or a static non-zero `count`).
    pub is_fixed_count: bool,
    /// Min byte capacity provable for this buffer. `0` for runtime-sized
    /// variable-element buffers with no positive predicate lower bound.
    pub min_bytes: u64,
    /// Max byte capacity provable for this buffer. `None` when either the
    /// element type or the count is unbounded.
    pub max_bytes: Option<u64>,
    /// Byte alignment proved by the `BufferDecl`'s `MultipleOf(n)` predicate
    /// scaled by element size. `1` when no alignment is provable.
    pub byte_alignment: u32,
}

impl BufferShapeFacts {
    /// Whether the buffer is provably non-empty at runtime.
    #[must_use]
    #[inline]
    pub fn is_non_empty(&self) -> bool {
        self.min_count > 0
    }

    /// Whether the buffer's static byte size exceeds `bytes`.
    #[must_use]
    #[inline]
    pub fn min_bytes_at_least(&self, bytes: u64) -> bool {
        self.min_bytes >= bytes
    }

    /// Whether the buffer can be vectorized at `lane_count` lanes
    /// without a tail loop (count is a multiple of `lane_count`).
    #[must_use]
    pub fn vectorizable_at(&self, lane_count: u32) -> bool {
        if lane_count == 0 {
            return false;
        }
        if self.is_fixed_count {
            return self.max_count.is_some_and(|count| count % lane_count == 0);
        }
        let Some(element_size_bytes) = self.element_size_bytes else {
            return false;
        };
        let element_size = u32::try_from(element_size_bytes).unwrap_or(u32::MAX).max(1);
        self.byte_alignment
            .checked_div(element_size)
            .is_some_and(|elt_align| elt_align % lane_count == 0)
    }
}

/// Map of buffer-name → static facts. Built once per `Program`; immutable.
///
/// Build with [`ProgramShapeFacts::derive`]. Pass it through optimizer
/// `PassCtx`, lowering, and validation as a typed input  -  they all
/// consume the same facts so a contradiction can never arise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramShapeFacts {
    by_name: FxHashMap<Ident, BufferShapeFacts>,
}

// Thread-local fingerprint-keyed cache so consumers that call derive
// directly (e.g. `Autotune::transform`) reuse the previous result on the
// same program instead of re-walking every BufferDecl. The
// FactSubstrate caches own a separate `Arc<ProgramShapeFacts>` for
// shape+use bundles; this slot serves direct `derive()` callers.
thread_local! {
    static SHAPE_FACTS_CACHE: std::cell::RefCell<Option<([u8; 32], std::rc::Rc<ProgramShapeFacts>)>> =
        const { std::cell::RefCell::new(None) };
}

impl ProgramShapeFacts {
    /// Derive shape facts for every `BufferDecl` in the program.
    #[must_use]
    pub fn derive(program: &Program) -> Self {
        Self::derive_uncached(program)
    }

    /// Cached counterpart of [`Self::derive`]. Returns the previously
    /// derived facts for the same program fingerprint via a thread-local
    /// slot. Use when the caller needs `ProgramShapeFacts` by value but
    /// is on the optimizer hot path where the same program may be queried
    /// multiple times in quick succession (e.g. autotune followed by
    /// shape-aware vectorization on the same fingerprint).
    #[must_use]
    pub fn derive_cached(program: &Program) -> Self {
        let fp = program.fingerprint();
        SHAPE_FACTS_CACHE.with(|cell| {
            if let Some((cached_fp, ref cached)) = *cell.borrow() {
                if cached_fp == fp {
                    return (**cached).clone();
                }
            }
            let fresh = Self::derive_uncached(program);
            *cell.borrow_mut() = Some((fp, std::rc::Rc::new(fresh.clone())));
            fresh
        })
    }

    /// Derive shape facts and return a shared cached instance.
    #[must_use]
    fn derive_arc(program: &Program, cache: &mut ShapeFactCache) -> Arc<Self> {
        let fingerprint = crate::optimizer::fingerprint_program(program);
        if let Some(facts) = cache.get(fingerprint) {
            return facts;
        }
        let facts = Arc::new(Self::derive_uncached(program));
        cache.insert(fingerprint, Arc::clone(&facts));
        facts
    }

    fn derive_uncached(program: &Program) -> Self {
        let mut by_name = FxHashMap::default();
        by_name.reserve(program.buffers().len());

        for decl in program.buffers() {
            let name = Ident::from(decl.name.as_ref());
            let dtype = decl.element.clone();
            let element_size_bytes = dtype.size_bytes();
            let declared_count = decl.count;

            let predicate = decl.shape_predicate();

            let predicate_min = predicate.map_or(0, shape_facts::min_count);
            let predicate_max = predicate.and_then(shape_facts::max_count);

            let static_min = if declared_count > 0 {
                declared_count
            } else {
                0
            };
            let static_max = if declared_count > 0 {
                Some(declared_count)
            } else {
                None
            };

            let min_count = predicate_min.max(static_min);
            let max_count = match (predicate_max, static_max) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            };

            let is_fixed_count = max_count.is_some() && Some(min_count) == max_count;

            let element_byte_alignment = element_size_bytes
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(1)
                .max(1);
            let count_alignment = predicate.map_or(1, count_alignment_from_predicate);
            let byte_alignment = element_byte_alignment.saturating_mul(count_alignment);

            let element_size_u64 = element_size_bytes.and_then(|n| u64::try_from(n).ok());

            let min_bytes = match element_size_u64 {
                Some(esz) => esz.saturating_mul(u64::from(min_count)),
                None => 0,
            };
            let max_bytes = match (element_size_u64, max_count) {
                (Some(esz), Some(count)) => Some(esz.saturating_mul(u64::from(count))),
                _ => None,
            };

            by_name.insert(
                name.clone(),
                BufferShapeFacts {
                    name,
                    binding: decl.binding,
                    dtype,
                    declared_count,
                    element_size_bytes,
                    min_count,
                    max_count,
                    is_fixed_count,
                    min_bytes,
                    max_bytes,
                    byte_alignment,
                },
            );
        }

        Self { by_name }
    }

    /// Look up facts for a named buffer.
    #[must_use]
    #[inline]
    pub fn get(&self, name: &Ident) -> Option<&BufferShapeFacts> {
        self.by_name.get(name)
    }

    /// Iterator over every (name, facts) pair.
    pub fn iter(&self) -> impl Iterator<Item = (&Ident, &BufferShapeFacts)> {
        self.by_name.iter()
    }

    /// Count of buffers covered by this analysis.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Whether the analysis covers zero buffers.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// Derive once and stash on the analysis cache under [`ANALYSIS_KEY`].
    ///
    /// Pass schedulers call this before each fixpoint iteration so every
    /// pass that opts into shape-aware decisions reads the same derived
    /// map. The cache is type-erased; consumers retrieve via
    /// [`ProgramShapeFacts::from_cache`].
    pub fn derive_into_cache(program: &Program, cache: &mut crate::optimizer::AnalysisCache) {
        let mut shape_cache = ShapeFactCache::default();
        cache.insert(
            ANALYSIS_KEY,
            Self::derive_arc(program, &mut shape_cache).as_ref().clone(),
        );
    }

    /// Look up a previously-derived analysis from the cache. Returns
    /// `None` when the scheduler did not derive it (older passes that
    /// did not opt into the analysis).
    #[must_use]
    pub fn from_cache(cache: &crate::optimizer::AnalysisCache) -> Option<&Self> {
        cache.get::<Self>(ANALYSIS_KEY)
    }
}

#[derive(Default)]
struct ShapeFactCache {
    by_fingerprint: FxHashMap<u64, Arc<ProgramShapeFacts>>,
    order: VecDeque<u64>,
}

impl ShapeFactCache {
    fn get(&self, fingerprint: u64) -> Option<Arc<ProgramShapeFacts>> {
        self.by_fingerprint.get(&fingerprint).cloned()
    }

    fn insert(&mut self, fingerprint: u64, facts: Arc<ProgramShapeFacts>) {
        if self.by_fingerprint.insert(fingerprint, facts).is_none() {
            self.order.push_back(fingerprint);
        }
        while self.order.len() > SHAPE_FACT_CACHE_CAP {
            if let Some(evicted) = self.order.pop_front() {
                self.by_fingerprint.remove(&evicted);
            }
        }
    }
}

/// Largest divisor `n` proved by `MultipleOf(n)` clauses in the predicate
/// tree. Used to compute byte alignment when combined with element size.
fn count_alignment_from_predicate(
    predicate: &crate::ir_inner::model::program::ShapePredicate,
) -> u32 {
    use crate::ir_inner::model::program::ShapePredicate;
    match predicate {
        ShapePredicate::MultipleOf(n) | ShapePredicate::Exactly(n) if *n > 0 => *n,
        ShapePredicate::ModEquals { modulus, remainder } if *modulus > 0 && *remainder == 0 => {
            *modulus
        }
        ShapePredicate::And(a, b) => {
            count_alignment_from_predicate(a).max(count_alignment_from_predicate(b))
        }
        ShapePredicate::Or(a, b) => {
            let left = count_alignment_from_predicate(a);
            let right = count_alignment_from_predicate(b);
            left.min(right)
        }
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir_inner::model::program::{BufferDecl, ShapePredicate};
    use vyre::ir::{DataType as VyreDataType, Node, Program as VyreProgram};

    fn one_buffer_program() -> VyreProgram {
        VyreProgram::wrapped(
            vec![
                BufferDecl::read("input", 0, VyreDataType::U32).with_count(64),
                BufferDecl::output("out", 1, VyreDataType::U32)
                    .with_count(64)
                    .with_output_byte_range(0..256),
            ],
            [64, 1, 1],
            vec![Node::return_()],
        )
    }

    #[test]
    fn derive_records_every_buffer() {
        let program = one_buffer_program();
        let facts = ProgramShapeFacts::derive(&program);
        assert_eq!(facts.len(), 2);
        assert!(facts.get(&Ident::from("input")).is_some());
        assert!(facts.get(&Ident::from("out")).is_some());
    }

    #[test]
    fn declared_count_pins_min_and_max() {
        let program = one_buffer_program();
        let facts = ProgramShapeFacts::derive(&program);
        let input = facts
            .get(&Ident::from("input"))
            .expect("Fix: input fact must exist");
        assert_eq!(input.min_count, 64);
        assert_eq!(input.max_count, Some(64));
        assert!(input.is_fixed_count);
        assert_eq!(input.element_size_bytes, Some(4));
        assert_eq!(input.min_bytes, 256);
        assert_eq!(input.max_bytes, Some(256));
        assert!(input.is_non_empty());
    }

    #[test]
    fn shape_predicate_at_least_widens_lower_bound_only() {
        let program = VyreProgram::wrapped(
            vec![BufferDecl::read("input", 0, VyreDataType::U32)
                .with_shape_predicate(ShapePredicate::AtLeast(32))],
            [64, 1, 1],
            vec![Node::return_()],
        );
        let facts = ProgramShapeFacts::derive(&program);
        let input = facts.get(&Ident::from("input")).unwrap();
        assert_eq!(input.min_count, 32);
        assert_eq!(input.max_count, None);
        assert!(!input.is_fixed_count);
        assert!(input.is_non_empty());
    }

    #[test]
    fn shape_predicate_multiple_of_proves_byte_alignment() {
        let program = VyreProgram::wrapped(
            vec![BufferDecl::read("input", 0, VyreDataType::U32)
                .with_shape_predicate(ShapePredicate::MultipleOf(16))],
            [64, 1, 1],
            vec![Node::return_()],
        );
        let facts = ProgramShapeFacts::derive(&program);
        let input = facts.get(&Ident::from("input")).unwrap();
        // U32 = 4 bytes, MultipleOf(16) elements → 64-byte alignment.
        assert_eq!(input.byte_alignment, 64);
        assert!(input.vectorizable_at(4));
        assert!(input.vectorizable_at(8));
        assert!(input.vectorizable_at(16));
    }

    #[test]
    fn vectorizable_at_uses_exact_runtime_predicate_count() {
        let program = VyreProgram::wrapped(
            vec![BufferDecl::read("input", 0, VyreDataType::U32)
                .with_shape_predicate(ShapePredicate::Exactly(96))],
            [64, 1, 1],
            vec![Node::return_()],
        );
        let facts = ProgramShapeFacts::derive(&program);
        let input = facts.get(&Ident::from("input")).unwrap();
        assert!(input.is_fixed_count);
        assert_eq!(input.declared_count, 0);
        assert!(input.vectorizable_at(32));
        assert!(!input.vectorizable_at(64));
    }

    #[test]
    fn variable_size_dtype_leaves_bytes_unbounded() {
        // Tensor has no fixed element size at the spec level.
        let program = VyreProgram::wrapped(
            vec![BufferDecl::read("input", 0, VyreDataType::Tensor)],
            [64, 1, 1],
            vec![Node::return_()],
        );
        let facts = ProgramShapeFacts::derive(&program);
        let input = facts.get(&Ident::from("input")).unwrap();
        assert_eq!(input.element_size_bytes, None);
        assert_eq!(input.min_bytes, 0);
        assert_eq!(input.max_bytes, None);
        assert!(!input.is_fixed_count);
    }

    #[test]
    fn fixed_count_program_proves_byte_capacity_exactly() {
        let program = one_buffer_program();
        let facts = ProgramShapeFacts::derive(&program);
        let out = facts.get(&Ident::from("out")).unwrap();
        assert_eq!(out.min_bytes, 256);
        assert_eq!(out.max_bytes, Some(256));
        assert!(out.min_bytes_at_least(128));
        assert!(!out.min_bytes_at_least(512));
    }

    #[test]
    fn cache_round_trip_returns_same_facts() {
        use crate::optimizer::AnalysisCache;
        let program = one_buffer_program();
        let mut cache = AnalysisCache::new();
        ProgramShapeFacts::derive_into_cache(&program, &mut cache);
        let cached = ProgramShapeFacts::from_cache(&cache).expect("Fix: facts must round-trip");
        assert_eq!(cached.len(), 2);
        let direct = ProgramShapeFacts::derive(&program);
        assert_eq!(cached, &direct);
    }

    #[test]
    fn shape_fact_cache_eviction_is_fifo_without_shifting() {
        let mut cache = ShapeFactCache::default();
        for fingerprint in 0..(SHAPE_FACT_CACHE_CAP as u64 + 2) {
            cache.insert(fingerprint, Arc::new(ProgramShapeFacts::default()));
        }

        assert_eq!(cache.order.len(), SHAPE_FACT_CACHE_CAP);
        assert!(!cache.by_fingerprint.contains_key(&0));
        assert!(!cache.by_fingerprint.contains_key(&1));
        assert!(cache
            .by_fingerprint
            .contains_key(&(SHAPE_FACT_CACHE_CAP as u64 + 1)));
        assert_eq!(cache.order.front().copied(), Some(2));
    }
}

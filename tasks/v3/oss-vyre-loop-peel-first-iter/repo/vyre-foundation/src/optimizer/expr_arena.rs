//! ROADMAP A1  -  hash-consed Expr arena.
//!
//! Op id: `vyre-foundation::optimizer::expr_arena`. Soundness: read-only
//! over the input `Expr`; produces an additive side-table that does not
//! mutate the program. Cost-direction: monotone-down on optimizer
//! clone/walk cost  -  every interned `Expr` becomes a 32-bit `ExprId`,
//! so passes that previously cloned subtrees now copy 4 bytes.
//! Preserves: every analysis. Invalidates: nothing.
//!
//! ## Why
//!
//! `Expr` lives in `Box<Expr>` slots throughout the IR  -  every
//! `BinOp { left, right }`, every `Select { cond, true_val, false_val }`,
//! every `Load { index }` allocates. Optimizer passes that re-walk
//! the same subtree (CSE, fusion, const-fold to fixpoint) clone the
//! whole tree on every visit. Hash-consing turns the recursive `Box`
//! tree into a flat array of small `FlatExpr` nodes indexed by
//! 32-bit `ExprId`. Equal subtrees collapse to the same `ExprId`, so
//! "are these two subexpressions equal?" becomes a u32 compare.
//!
//! ## Design
//!
//! - `ExprArena` owns a `Vec<FlatExpr>`. Indexes into it are `ExprId(u32)`.
//! - `FlatExpr` mirrors the `Expr` enum but every recursive position
//!   is replaced with `ExprId`. The `Hash` + `Eq` impls are derived
//!   structurally, so a plain `FxHashMap<FlatExpr, ExprId>` is the
//!   hash-cons table.
//! - `intern(&Expr) -> ExprId` recurses children-first, builds a
//!   `FlatExpr` for the current node, looks it up in the hash-cons
//!   table, and either returns the existing `ExprId` or pushes a new
//!   one and records it.
//! - `rebuild(ExprId) -> Expr` walks the arena and reconstructs an
//!   owned `Expr` tree. Cost: one `Box::new` per node, same as
//!   `Expr::clone`. Used at the optimizer→backend boundary where the
//!   IR has to look like `Expr` again.
//!
//! ## Migration
//!
//! Additive  -  no existing code changes shape. Passes that want the
//! speedup do `let arena = ExprArena::default(); let id =
//! arena.intern(&expr);` and operate on `ExprId`s. The CSE pass is
//! the obvious first consumer; the egglog `Family` substrate
//! (`eqsat.rs`) already takes a generic `Lang: ENodeLang` so a
//! `FlatExpr`-flavoured language wrapper is the bridge.
//!
//! ## What this does NOT do
//!
//! - Replace `Box<Expr>` in the IR. That is the multi-day
//!   migration that has to follow once every consumer is on `ExprId`.
//! - Hash-cons across `Program`s. The arena is per-program; cross-
//!   program interning would need the `vyre-foundation::diff_compile`
//!   subtree-hash side table.
//! - Intern `Expr::Opaque`. Opaque extension expressions are sealed by
//!   their `ExprNode` trait object; the arena stores them by `Arc`
//!   identity (pointer-equality), which means two distinct `Arc`s
//!   wrapping equal contents produce distinct `ExprId`s. Good enough
//!   for the optimizer use-case; full structural equality on opaque
//!   extensions needs an `ExprNode::content_hash` API that does not
//!   exist today.

use crate::ir::model::expr::ExprNode;
use crate::ir::{AtomicOp, BinOp, DataType, Expr, Ident, MemoryOrdering, SubgroupReduceOp, UnOp};
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// Compact 32-bit handle into an [`ExprArena`].
///
/// `Copy` so passes can pass it around without clones. Two
/// structurally-equal `Expr`s interned into the same arena produce
/// the same `ExprId`  -  that is the whole point of the hash-cons.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ExprId(pub u32);

impl ExprId {
    /// Numeric form (for trace/debug output).
    #[must_use]
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// Flat hash-cons-friendly mirror of [`Expr`]. Every recursive
/// `Box<Expr>` is replaced with `ExprId`, so the variant itself fits
/// in a small fixed budget.
///
/// `Eq` + `Hash` are derived structurally, so the hash-cons table is
/// `FxHashMap<FlatExpr, ExprId>`.
///
/// Variants mirror the [`Expr`] enum 1:1  -  see that type's documentation
/// for per-variant semantics.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum FlatExpr {
    LitU32(u32),
    LitI32(i32),
    /// f32 is hashed/compared by raw bit pattern so `NaN == NaN` and
    /// `+0.0 != -0.0` both hold (the optimizer treats them as distinct
    /// values).
    LitF32Bits(u32),
    LitBool(bool),
    Var(Ident),
    Load {
        buffer: Ident,
        index: ExprId,
    },
    BufLen {
        buffer: Ident,
    },
    InvocationId {
        axis: u8,
    },
    WorkgroupId {
        axis: u8,
    },
    LocalId {
        axis: u8,
    },
    BinOp {
        op: BinOp,
        left: ExprId,
        right: ExprId,
    },
    UnOp {
        op: UnOp,
        operand: ExprId,
    },
    Call {
        op_id: Ident,
        args: Vec<ExprId>,
    },
    Select {
        cond: ExprId,
        true_val: ExprId,
        false_val: ExprId,
    },
    Cast {
        target: DataType,
        value: ExprId,
    },
    Fma {
        a: ExprId,
        b: ExprId,
        c: ExprId,
    },
    Atomic {
        op: AtomicOp,
        buffer: Ident,
        index: ExprId,
        expected: Option<ExprId>,
        value: ExprId,
        ordering: MemoryOrdering,
    },
    SubgroupBallot {
        cond: ExprId,
    },
    SubgroupShuffle {
        value: ExprId,
        lane: ExprId,
    },
    SubgroupReduce {
        op: SubgroupReduceOp,
        value: ExprId,
    },
    SubgroupLocalId,
    SubgroupSize,
    /// Opaque extension expressions are interned by `Arc` identity, not
    /// structural equality (no `ExprNode::content_hash` API). Equal
    /// contents wrapped in distinct `Arc`s get distinct `ExprId`s.
    Opaque(OpaqueId),
}

/// Pointer-identity tag for opaque extension expressions. Wraps the
/// `Arc<dyn ExprNode>` raw pointer cast to `usize`  -  two `Arc`s
/// pointing to the same allocation produce the same `OpaqueId`; two
/// `Arc`s wrapping equal contents but with distinct allocations
/// produce different `OpaqueId`s.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OpaqueId(usize);

/// Hash-consed arena of [`FlatExpr`] nodes.
///
/// Build one per program (or share across passes within the same
/// fixpoint iteration). Repeated calls to [`Self::intern`] for the
/// same input shape return the same [`ExprId`] in O(1) amortised.
#[derive(Debug, Default)]
pub struct ExprArena {
    /// Interned nodes, addressed by `ExprId.0`. Stored as `Arc<FlatExpr>`
    /// so the hashcons key shares storage with the node  -  interning a
    /// fresh expression performs one heap allocation (the Arc) rather
    /// than two (one for the Vec push clone + one for the map insert).
    /// Public `get(id)` still returns `&FlatExpr` via `Arc::as_ref`.
    nodes: Vec<Arc<FlatExpr>>,
    /// Side-table keeping the original `Arc<dyn ExprNode>` alive for
    /// every `OpaqueId` recorded in `nodes`. Needed because
    /// `OpaqueId` is just a usize fingerprint; `rebuild` reconstructs
    /// the `Arc` by looking up here.
    opaques: Vec<Arc<dyn ExprNode>>,
    /// `OpaqueId` already seen → its index into `opaques`.
    opaque_lookup: FxHashMap<OpaqueId, usize>,
    hashcons: FxHashMap<Arc<FlatExpr>, ExprId>,
}

impl ExprArena {
    /// Number of distinct `FlatExpr` nodes currently interned.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True iff no `Expr` has been interned yet.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Borrow the [`FlatExpr`] previously interned at `id`.
    ///
    /// # Panics
    ///
    /// Panics when `id` was not produced by this arena. Cross-arena
    /// `ExprId`s are a programming error and are caught loud rather
    /// than producing silently wrong rewrites.
    #[must_use]
    pub fn get(&self, id: ExprId) -> &FlatExpr {
        self.nodes.get(id.0 as usize).map_or_else(
            || unreachable!("Fix: ExprId({}) not produced by this arena", id.0),
            Arc::as_ref,
        )
    }

    /// Walk `expr` children-first, intern every node, return the
    /// `ExprId` of the root. Equal subtrees collapse to the same
    /// `ExprId`.
    pub fn intern(&mut self, expr: &Expr) -> ExprId {
        let flat = self.flatten(expr);
        self.intern_flat(flat)
    }

    /// Reconstruct an owned `Expr` tree from `id`. Cost: one
    /// `Box::new` per node, same as `Expr::clone`. Used at the
    /// optimizer→backend boundary.
    #[must_use]
    pub fn rebuild(&self, id: ExprId) -> Expr {
        match self.get(id).clone() {
            FlatExpr::LitU32(v) => Expr::LitU32(v),
            FlatExpr::LitI32(v) => Expr::LitI32(v),
            FlatExpr::LitF32Bits(bits) => Expr::LitF32(f32::from_bits(bits)),
            FlatExpr::LitBool(v) => Expr::LitBool(v),
            FlatExpr::Var(name) => Expr::Var(name),
            FlatExpr::Load { buffer, index } => Expr::Load {
                buffer,
                index: Box::new(self.rebuild(index)),
            },
            FlatExpr::BufLen { buffer } => Expr::BufLen { buffer },
            FlatExpr::InvocationId { axis } => Expr::InvocationId { axis },
            FlatExpr::WorkgroupId { axis } => Expr::WorkgroupId { axis },
            FlatExpr::LocalId { axis } => Expr::LocalId { axis },
            FlatExpr::BinOp { op, left, right } => Expr::BinOp {
                op,
                left: Box::new(self.rebuild(left)),
                right: Box::new(self.rebuild(right)),
            },
            FlatExpr::UnOp { op, operand } => Expr::UnOp {
                op,
                operand: Box::new(self.rebuild(operand)),
            },
            FlatExpr::Call { op_id, args } => Expr::Call {
                op_id,
                args: args.into_iter().map(|a| self.rebuild(a)).collect(),
            },
            FlatExpr::Select {
                cond,
                true_val,
                false_val,
            } => Expr::Select {
                cond: Box::new(self.rebuild(cond)),
                true_val: Box::new(self.rebuild(true_val)),
                false_val: Box::new(self.rebuild(false_val)),
            },
            FlatExpr::Cast { target, value } => Expr::Cast {
                target,
                value: Box::new(self.rebuild(value)),
            },
            FlatExpr::Fma { a, b, c } => Expr::Fma {
                a: Box::new(self.rebuild(a)),
                b: Box::new(self.rebuild(b)),
                c: Box::new(self.rebuild(c)),
            },
            FlatExpr::Atomic {
                op,
                buffer,
                index,
                expected,
                value,
                ordering,
            } => Expr::Atomic {
                op,
                buffer,
                index: Box::new(self.rebuild(index)),
                expected: expected.map(|id| Box::new(self.rebuild(id))),
                value: Box::new(self.rebuild(value)),
                ordering,
            },
            FlatExpr::SubgroupBallot { cond } => Expr::SubgroupBallot {
                cond: Box::new(self.rebuild(cond)),
            },
            FlatExpr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
                value: Box::new(self.rebuild(value)),
                lane: Box::new(self.rebuild(lane)),
            },
            FlatExpr::SubgroupReduce { op, value } => Expr::SubgroupReduce {
                op,
                value: Box::new(self.rebuild(value)),
            },
            FlatExpr::SubgroupLocalId => Expr::SubgroupLocalId,
            FlatExpr::SubgroupSize => Expr::SubgroupSize,
            FlatExpr::Opaque(opaque_id) => {
                let idx = self
                    .opaque_lookup
                    .get(&opaque_id)
                    .copied()
                    .unwrap_or_else(|| {
                        unreachable!(
                            "rebuild only sees OpaqueIds produced by intern_flat (this arena)"
                        )
                    });
                Expr::Opaque(Arc::clone(&self.opaques[idx]))
            }
        }
    }

    fn intern_flat(&mut self, flat: FlatExpr) -> ExprId {
        // Lookup-by-borrow: `Arc<FlatExpr>: Borrow<FlatExpr>`, so the
        // hashcons hits without allocating an Arc on a cache hit.
        if let Some(existing) = self.hashcons.get(&flat) {
            return *existing;
        }
        let id = expr_id_from_len(self.nodes.len());
        let shared: Arc<FlatExpr> = Arc::new(flat);
        self.nodes.push(Arc::clone(&shared));
        self.hashcons.insert(shared, id);
        id
    }

    fn flatten(&mut self, expr: &Expr) -> FlatExpr {
        match expr {
            Expr::LitU32(v) => FlatExpr::LitU32(*v),
            Expr::LitI32(v) => FlatExpr::LitI32(*v),
            Expr::LitF32(v) => FlatExpr::LitF32Bits(v.to_bits()),
            Expr::LitBool(v) => FlatExpr::LitBool(*v),
            Expr::Var(name) => FlatExpr::Var(name.clone()),
            Expr::Load { buffer, index } => FlatExpr::Load {
                buffer: buffer.clone(),
                index: self.intern(index),
            },
            Expr::BufLen { buffer } => FlatExpr::BufLen {
                buffer: buffer.clone(),
            },
            Expr::InvocationId { axis } => FlatExpr::InvocationId { axis: *axis },
            Expr::WorkgroupId { axis } => FlatExpr::WorkgroupId { axis: *axis },
            Expr::LocalId { axis } => FlatExpr::LocalId { axis: *axis },
            Expr::BinOp { op, left, right } => FlatExpr::BinOp {
                op: *op,
                left: self.intern(left),
                right: self.intern(right),
            },
            Expr::UnOp { op, operand } => FlatExpr::UnOp {
                op: op.clone(),
                operand: self.intern(operand),
            },
            Expr::Call { op_id, args } => FlatExpr::Call {
                op_id: op_id.clone(),
                args: args.iter().map(|a| self.intern(a)).collect(),
            },
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => FlatExpr::Select {
                cond: self.intern(cond),
                true_val: self.intern(true_val),
                false_val: self.intern(false_val),
            },
            Expr::Cast { target, value } => FlatExpr::Cast {
                target: target.clone(),
                value: self.intern(value),
            },
            Expr::Fma { a, b, c } => FlatExpr::Fma {
                a: self.intern(a),
                b: self.intern(b),
                c: self.intern(c),
            },
            Expr::Atomic {
                op,
                buffer,
                index,
                expected,
                value,
                ordering,
            } => FlatExpr::Atomic {
                op: *op,
                buffer: buffer.clone(),
                index: self.intern(index),
                expected: expected.as_deref().map(|e| self.intern(e)),
                value: self.intern(value),
                ordering: *ordering,
            },
            Expr::SubgroupBallot { cond } => FlatExpr::SubgroupBallot {
                cond: self.intern(cond),
            },
            Expr::SubgroupShuffle { value, lane } => FlatExpr::SubgroupShuffle {
                value: self.intern(value),
                lane: self.intern(lane),
            },
            Expr::SubgroupReduce { op, value } => FlatExpr::SubgroupReduce {
                op: *op,
                value: self.intern(value),
            },
            Expr::SubgroupLocalId => FlatExpr::SubgroupLocalId,
            Expr::SubgroupSize => FlatExpr::SubgroupSize,
            Expr::Opaque(arc) => {
                let opaque_id = OpaqueId(Arc::as_ptr(arc).cast::<()>() as usize);
                self.opaque_lookup.entry(opaque_id).or_insert_with(|| {
                    let idx = self.opaques.len();
                    self.opaques.push(Arc::clone(arc));
                    idx
                });
                FlatExpr::Opaque(opaque_id)
            }
        }
    }
}

fn expr_id_from_len(len: usize) -> ExprId {
    ExprId(u32::try_from(len).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Expr;

    #[test]
    fn distinct_literals_get_distinct_ids() {
        let mut arena = ExprArena::default();
        let a = arena.intern(&Expr::u32(1));
        let b = arena.intern(&Expr::u32(2));
        assert_ne!(a, b);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn equal_literals_collapse_to_one_id() {
        let mut arena = ExprArena::default();
        let a = arena.intern(&Expr::u32(7));
        let b = arena.intern(&Expr::u32(7));
        assert_eq!(a, b);
        assert_eq!(
            arena.len(),
            1,
            "second intern of equal Expr must not grow the arena"
        );
    }

    #[test]
    fn equal_subtrees_collapse_at_every_level() {
        let mut arena = ExprArena::default();
        // BinOp::Add(LitU32(1), LitU32(2))  -  first interning produces
        // 3 nodes (Lit 1, Lit 2, BinOp).
        let lhs = Expr::add(Expr::u32(1), Expr::u32(2));
        let id_a = arena.intern(&lhs);
        assert_eq!(arena.len(), 3);
        // Second identical subtree shares all three ids.
        let id_b = arena.intern(&lhs);
        assert_eq!(id_a, id_b);
        assert_eq!(arena.len(), 3, "no new nodes for an identical subtree");
    }

    #[test]
    fn shared_leaves_dedup_across_distinct_parents() {
        let mut arena = ExprArena::default();
        // Add(1, 2) and Sub(1, 2)  -  distinct parents, but the two
        // literals 1 and 2 must share ids.
        let add_id = arena.intern(&Expr::add(Expr::u32(1), Expr::u32(2)));
        let sub_id = arena.intern(&Expr::sub(Expr::u32(1), Expr::u32(2)));
        assert_ne!(add_id, sub_id);
        // Lit(1), Lit(2), Add, Sub = 4 nodes.
        assert_eq!(arena.len(), 4);
    }

    #[test]
    fn rebuild_round_trip_for_simple_literal() {
        let mut arena = ExprArena::default();
        let id = arena.intern(&Expr::u32(42));
        assert_eq!(arena.rebuild(id), Expr::u32(42));
    }

    #[test]
    fn rebuild_round_trip_for_nested_expression() {
        let mut arena = ExprArena::default();
        let original = Expr::Select {
            cond: Box::new(Expr::lt(Expr::var("i"), Expr::u32(8))),
            true_val: Box::new(Expr::add(Expr::var("i"), Expr::u32(1))),
            false_val: Box::new(Expr::u32(0)),
        };
        let id = arena.intern(&original);
        assert_eq!(arena.rebuild(id), original);
    }

    #[test]
    fn rebuild_round_trip_for_load() {
        let mut arena = ExprArena::default();
        let original = Expr::load("buf", Expr::add(Expr::var("base"), Expr::u32(4)));
        let id = arena.intern(&original);
        assert_eq!(arena.rebuild(id), original);
    }

    #[test]
    fn nan_literals_intern_to_same_id() {
        let mut arena = ExprArena::default();
        let a = arena.intern(&Expr::f32(f32::NAN));
        let b = arena.intern(&Expr::f32(f32::NAN));
        // f32 NaN comparison with == is false in IEEE 754, but
        // hashing by bit pattern makes the arena treat them as equal.
        // This is the right behaviour for an optimizer: two
        // syntactically-identical NaN literals are the same node.
        assert_eq!(a, b);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn positive_zero_and_negative_zero_get_distinct_ids() {
        let mut arena = ExprArena::default();
        let pos = arena.intern(&Expr::f32(0.0));
        let neg = arena.intern(&Expr::f32(-0.0));
        // +0.0 and -0.0 have distinct bit patterns and are treated as
        // distinct expressions by the arena. This matches IEEE 754
        // signed-zero semantics; passes that want to fold them must
        // use the const-fold rule, not arena identity.
        assert_ne!(pos, neg);
    }

    #[test]
    fn expr_id_is_copy_and_eq() {
        // Compile-time check: ExprId implements Copy.
        let id = ExprId(0);
        let copy = id;
        assert_eq!(id, copy);
    }

    #[test]
    fn arena_intern_is_idempotent_under_rebuild_intern_cycle() {
        let mut arena = ExprArena::default();
        let original = Expr::Fma {
            a: Box::new(Expr::var("x")),
            b: Box::new(Expr::var("y")),
            c: Box::new(Expr::var("z")),
        };
        let id = arena.intern(&original);
        let rebuilt = arena.rebuild(id);
        let id2 = arena.intern(&rebuilt);
        assert_eq!(id, id2, "rebuild then re-intern must hit the same ExprId");
    }

    #[test]
    fn opaque_expr_interning_via_arc_identity() {
        // Build two `Arc`s pointing at the same allocation; their
        // OpaqueIds must collapse.
        use crate::ir::model::expr::ExprNode;
        use crate::ir::DataType;
        use std::any::Any;

        #[derive(Debug)]
        struct DummyOpaque;
        impl ExprNode for DummyOpaque {
            fn extension_kind(&self) -> &'static str {
                "dummy"
            }
            fn debug_identity(&self) -> &str {
                "dummy"
            }
            fn result_type(&self) -> Option<DataType> {
                Some(DataType::U32)
            }
            fn cse_safe(&self) -> bool {
                true
            }
            fn stable_fingerprint(&self) -> [u8; 32] {
                [0u8; 32]
            }
            fn validate_extension(&self) -> Result<(), String> {
                Ok(())
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
        }
        let arc: Arc<dyn ExprNode> = Arc::new(DummyOpaque);
        let mut arena = ExprArena::default();
        let id_a = arena.intern(&Expr::Opaque(Arc::clone(&arc)));
        let id_b = arena.intern(&Expr::Opaque(arc));
        assert_eq!(id_a, id_b, "two Arcs of the same allocation must collapse");
        assert_eq!(arena.len(), 1);
    }
}

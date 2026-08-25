//! Effect lattice  -  composition-aware refusal for fusion + dispatch.
//!
//! Op id: `vyre-foundation::optimizer::effect_lattice`. Soundness: `Exact` over
//! the closed lattice declared below. Cost-direction: read-only  -  lifts
//! `SideEffectClass` declarations into the lattice and reasons about
//! composition; never mutates the IR.
//!
//! ## Why
//!
//! Today's `SideEffectClass` is single-valued (Pure / ReadsMemory / WritesMemory /
//! Synchronizing / Atomic). A fusion pass that combines two ops can silently
//! compose effects that shouldn't compose  -  the canonical example is
//! `Pure ∘ Diverging` (a value computation followed by a `if invocation_id == K`-
//! gated store), where lifting one op into the other without an explicit
//! `MemoryOrdering::GridSync` produces a kernel that races on cross-block reads.
//! That class of bug shipped as the recall-zero-past-block-zero failure on
//! downstream consumers  -  silent miscompile, no error message.
//!
//! This module promotes effects into a checked lattice with composition rules.
//! When a fusion pass asks "may I fuse producer P into consumer C", the
//! lattice answers either:
//!   - `Ok(combined_effect)`  -  fusion is sound, here's the combined effect.
//!   - `Err(RefusalReason::EffectLatticeViolation)`  -  fusion is unsound, here's
//!     a structured reason naming both ops + a suggested fix.
//!
//! ## Lattice
//!
//! ```text
//! Pure  ⊑  ReadAtomic  ⊑  ReadWriteAtomic(Ordering)  ⊑  Synchronized(Scope)  ⊑  Diverging
//! ```
//!
//! - `Pure`  -  no memory effects, no synchronization, no thread-id-gated control flow.
//! - `ReadAtomic`  -  atomic loads only. Composes freely with anything weaker.
//! - `ReadWriteAtomic(Ordering)`  -  atomic RMW with a declared `MemoryOrdering`.
//!   Two `ReadWriteAtomic`s with mismatched orderings synthesize a barrier.
//! - `Synchronized(Scope)`  -  explicit barrier site (workgroup / subgroup / grid).
//!   Carries the scope so the composition rule knows which scope to honor.
//! - `Diverging`  -  the program reads from or writes to memory inside a
//!   `if invocation_id == K { ... }` block. Composing this into a consumer that
//!   reads the touched memory without a `GridSync` between is unsound.
//!
//! ## Composition rules (the moat)
//!
//! `compose(producer, consumer) -> Result<EffectLevel, RefusalReason>` returns:
//!
//! | producer / consumer | Pure | ReadAtomic | ReadWriteAtomic | Synchronized | Diverging |
//! |---|---|---|---|---|---|
//! | **Pure** | Pure | ReadAtomic | ReadWriteAtomic | Synchronized | REFUSE: needs GridSync |
//! | **ReadAtomic** | ReadAtomic | ReadAtomic | ReadWriteAtomic | Synchronized | REFUSE: needs GridSync |
//! | **ReadWriteAtomic(O1)** | ReadWriteAtomic(O1) | ReadWriteAtomic(O1) | ReadWriteAtomic(join(O1,O2)) or REFUSE | Synchronized | REFUSE: needs GridSync |
//! | **Synchronized(S1)** | Synchronized(S1) | Synchronized(S1) | Synchronized(S1) | Synchronized(join(S1,S2)) | REFUSE: needs GridSync to escalate scope |
//! | **Diverging** | REFUSE: producer's divergent stores not visible without GridSync | (same) | (same) | OK if Synchronized scope is Grid; else REFUSE | REFUSE: cannot fuse two diverging arms without a barrier between |
//!
//! The "REFUSE: needs GridSync" cells are the recall-zero-past-block-zero bug
//! caught at compile time. The fusion pass that would have produced a racy
//! kernel returns `RefusalReason::EffectLatticeViolation` instead.

use crate::ir::{Expr, Node, Program};
use crate::optimizer::RefusalReason;
use vyre_spec::op_contract::SideEffectClass;

/// Memory-ordering tag carried by `ReadWriteAtomic`. Mirrors the wire-frozen
/// `MemoryOrdering` in `vyre-foundation::memory_model` but reduced to the
/// orderings the lattice composition rules distinguish. `Relaxed` is treated
/// as `Acquire` in lattice composition (conservative  -  no rule allows weaker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AtomicOrdering {
    /// Acquire ordering  -  synchronizes with a Release on the same address.
    Acquire,
    /// Release ordering  -  synchronizes with an Acquire on the same address.
    Release,
    /// Acquire+Release combined.
    AcqRel,
    /// Sequentially consistent ordering  -  total order across all `SeqCst` ops.
    SeqCst,
}

impl AtomicOrdering {
    /// Join two orderings to the strongest of the pair. Used when composing
    /// two `ReadWriteAtomic` effects.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        use AtomicOrdering::{AcqRel, Acquire, Release, SeqCst};
        match (self, other) {
            (SeqCst, _) | (_, SeqCst) => SeqCst,
            (AcqRel, _) | (_, AcqRel) | (Acquire, Release) | (Release, Acquire) => AcqRel,
            (Acquire, Acquire) => Acquire,
            (Release, Release) => Release,
        }
    }
}

/// Synchronization scope carried by `Synchronized`. Mirrors the wire-frozen
/// barrier scope in `vyre-foundation::memory_model`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SyncScope {
    /// Subgroup-scope barrier (a single warp / wavefront).
    Subgroup,
    /// Workgroup-scope barrier (one hardware workgroup/thread block).
    Workgroup,
    /// Grid-scope barrier (every block in the dispatch). Only some hardware
    /// supports this natively; backends that don't lower it must split the
    /// kernel at the barrier (see `vyre-driver::grid_sync`).
    Grid,
}

impl SyncScope {
    /// Join two scopes to the larger one. Workgroup-then-Grid escalates to Grid.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }
}

/// Lattice point representing the cumulative effect of one op or one program
/// region. Composition uses [`compose`] which returns an
/// [`Err(RefusalReason::EffectLatticeViolation)`] when the pair cannot fuse
/// safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EffectLevel {
    /// No memory effects, no synchronization, no thread-id-gated control flow.
    Pure,
    /// Atomic loads only  -  composes freely with anything weaker.
    ReadAtomic,
    /// Atomic RMW with a declared `AtomicOrdering`.
    ReadWriteAtomic(AtomicOrdering),
    /// Explicit barrier site at a declared `SyncScope`.
    Synchronized(SyncScope),
    /// Program contains a `if invocation_id == K { ... }`-gated memory effect.
    /// Cannot fuse with a consumer that reads the touched memory without an
    /// explicit `GridSync` between them.
    Diverging,
}

impl EffectLevel {
    /// Lift a single-op `SideEffectClass` declaration into the lattice. Used
    /// when scanning `OpDef` metadata to derive an op's lattice point.
    #[must_use]
    pub fn from_class(class: SideEffectClass) -> Self {
        match class {
            SideEffectClass::Pure => Self::Pure,
            SideEffectClass::ReadsMemory => Self::ReadAtomic,
            SideEffectClass::WritesMemory | SideEffectClass::Atomic => {
                Self::ReadWriteAtomic(AtomicOrdering::SeqCst)
            }
            // `SideEffectClass` is #[non_exhaustive]; future variants default
            // to the strongest non-Diverging level so the lattice never
            // silently accepts an unknown class as Pure.
            _ => Self::Synchronized(SyncScope::Workgroup),
        }
    }

    /// Stable kind tag for diagnostics  -  drives the `RefusalReason` payload.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Pure => "pure",
            Self::ReadAtomic => "read_atomic",
            Self::ReadWriteAtomic(_) => "read_write_atomic",
            Self::Synchronized(_) => "synchronized",
            Self::Diverging => "diverging",
        }
    }
}

/// Compose `producer` followed by `consumer` and return the combined effect,
/// OR a structured refusal naming both sides + the fix the user must apply.
///
/// The producer's effect is observed by the consumer  -  that's the asymmetry
/// the rules encode. `compose(Pure, Diverging)` is fine in one direction
/// (consuming a divergent value into a pure computation is sound) but
/// `compose(Diverging, anything-that-reads-affected-memory)` is not.
///
/// # Errors
///
/// Returns `Err(RefusalReason::EffectLatticeViolation { producer, consumer,
/// suggested_fix })` whenever the composition rule forbids the pair. The
/// `producer` and `consumer` strings are stable kind tags from
/// `EffectLevel::kind`; the `suggested_fix` string names the IR rewrite the
/// user must apply (insert `Node::Barrier { GridSync }`, escalate scope, etc.).
pub fn compose(producer: EffectLevel, consumer: EffectLevel) -> Result<EffectLevel, RefusalReason> {
    use EffectLevel::{Diverging, Pure, ReadAtomic, ReadWriteAtomic, Synchronized};

    // Diverging on the PRODUCER side means a thread-id-gated store. The
    // consumer reading any touched memory without an intervening GridSync
    // races on cross-block visibility. Refuse every consumer except a
    // `Synchronized(Grid)` (which IS the GridSync the producer needs).
    if matches!(producer, Diverging) {
        if matches!(consumer, Synchronized(SyncScope::Grid)) {
            // Producer's divergent stores are flushed by the grid barrier
            // before the consumer runs. Combined effect: the grid sync.
            return Ok(Synchronized(SyncScope::Grid));
        }
        return Err(RefusalReason::EffectLatticeViolation {
            producer: "diverging",
            consumer: consumer.kind(),
            suggested_fix:
                "insert Node::Barrier { ordering: MemoryOrdering::GridSync } between the \
                 divergent producer and the consumer; without it, threads in non-zero blocks \
                 race on the producer's stores and read stale memory",
        });
    }

    // Diverging on the CONSUMER side: the producer feeds into a divergent
    // arm. Sound only if the producer is itself synchronized at grid scope
    // (so all blocks have observed the producer's writes before any thread
    // enters the divergent arm).
    if matches!(consumer, Diverging) {
        if matches!(producer, Synchronized(SyncScope::Grid)) {
            return Ok(Diverging);
        }
        // Pure / ReadAtomic / ReadWriteAtomic / Synchronized(Subgroup|Workgroup)
        // → REFUSE; consumer's divergent stores need a producer barrier first.
        return Err(RefusalReason::EffectLatticeViolation {
            producer: producer.kind(),
            consumer: "diverging",
            suggested_fix:
                "insert Node::Barrier { ordering: MemoryOrdering::GridSync } between the \
                 producer and the divergent consumer so all blocks observe the producer's \
                 writes before the divergent arm reads them",
        });
    }

    // The non-Diverging arms compose by lattice join.
    let combined = match (producer, consumer) {
        (Pure, c) => c,
        (p, Pure) => p,
        (ReadAtomic, ReadAtomic) => ReadAtomic,
        (ReadAtomic, ReadWriteAtomic(o)) | (ReadWriteAtomic(o), ReadAtomic) => ReadWriteAtomic(o),
        (ReadWriteAtomic(o1), ReadWriteAtomic(o2)) => ReadWriteAtomic(o1.join(o2)),
        (Synchronized(s), ReadAtomic) | (ReadAtomic, Synchronized(s)) => Synchronized(s),
        (Synchronized(s), ReadWriteAtomic(_)) | (ReadWriteAtomic(_), Synchronized(s)) => {
            Synchronized(s)
        }
        (Synchronized(s1), Synchronized(s2)) => Synchronized(s1.join(s2)),
        (Diverging, _) | (_, Diverging) => unreachable!("Diverging handled above"),
    };
    Ok(combined)
}

/// Compute the cumulative effect level of an entire `Program` as the
/// lattice-join (max) of every top-level entry node's effect. Used by the
/// fusion-refusal pass to derive the lattice PROFILE of one program before
/// deciding whether it composes with ANOTHER program.
///
/// This is intentionally NOT `compose`-based  -  `compose` is the producer→consumer
/// rule that refuses unsound fusions BETWEEN two programs. Within one program,
/// the user's intent is encoded as written; the program-level effect simply
/// summarizes "what's the strongest effect this program ever exhibits," which
/// is the lattice join. A program that contains a `if invocation_id == K`-gated
/// store summarises as `Diverging`; a downstream fusion pass that tries to
/// compose this with a Pure consumer gets the structured refusal at THAT site,
/// not while summarising the program in isolation.
///
/// Defaults a node's effect to `Pure` when no analysis can name a stronger
/// effect. Detects divergence via the same `if invocation_id == K` pattern the
/// `cost::CostCertificate` divergence-score walker uses, so the two analyses
/// stay consistent.
#[must_use]
pub fn program_effect_level(program: &Program) -> EffectLevel {
    let mut acc = EffectLevel::Pure;
    for node in program.entry() {
        let node_effect = node_effect_level(node);
        acc = lattice_join(acc, node_effect);
    }
    acc
}

/// Lattice join  -  take the strongest of two effect levels. Used by
/// `program_effect_level` to summarise a program (no refusal semantics);
/// `compose` is the version used by fusion-refusal that returns Err on
/// unsound producer→consumer pairs.
#[must_use]
pub fn lattice_join(a: EffectLevel, b: EffectLevel) -> EffectLevel {
    use EffectLevel::{Diverging, Pure, ReadAtomic, ReadWriteAtomic, Synchronized};
    match (a, b) {
        (Diverging, _) | (_, Diverging) => Diverging,
        (Synchronized(s1), Synchronized(s2)) => Synchronized(s1.join(s2)),
        (Synchronized(s), _) | (_, Synchronized(s)) => Synchronized(s),
        (ReadWriteAtomic(o1), ReadWriteAtomic(o2)) => ReadWriteAtomic(o1.join(o2)),
        (ReadWriteAtomic(o), _) | (_, ReadWriteAtomic(o)) => ReadWriteAtomic(o),
        (ReadAtomic, _) | (_, ReadAtomic) => ReadAtomic,
        (Pure, Pure) => Pure,
    }
}

/// Derive the lattice point for a single `Node`. Inspects the node shape;
/// recurses into nested bodies; surfaces `Diverging` when an
/// `if invocation_id == K { ... }` pattern is found at any depth.
#[must_use]
pub fn node_effect_level(node: &Node) -> EffectLevel {
    match node {
        Node::Store { .. } => EffectLevel::ReadWriteAtomic(AtomicOrdering::SeqCst),
        Node::Let { value, .. } | Node::Assign { value, .. } => expr_effect_level(value),
        Node::AsyncLoad { .. } => EffectLevel::ReadAtomic,
        Node::AsyncStore { .. } => EffectLevel::ReadWriteAtomic(AtomicOrdering::Release),
        Node::AsyncWait { .. } => EffectLevel::Synchronized(SyncScope::Workgroup),
        Node::Barrier { ordering } => barrier_effect(*ordering),
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            // Divergence check: `if invocation_id == K { ... }` makes this
            // node Diverging at this site. Otherwise the effect is the join
            // of the two arms.
            if is_invocation_id_eq_constant(cond) {
                return EffectLevel::Diverging;
            }
            join_arms(then.iter().chain(otherwise.iter()))
        }
        Node::Loop { body, .. } | Node::Block(body) => join_arms(body.iter()),
        Node::Region { body, .. } => join_arms(body.iter()),
        Node::IndirectDispatch { .. } => EffectLevel::Synchronized(SyncScope::Grid),
        Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. } => EffectLevel::Synchronized(SyncScope::Grid),
        Node::Trap { .. } | Node::Resume { .. } | Node::Return | Node::Opaque(_) => {
            EffectLevel::Pure
        }
    }
}

fn join_arms<'a>(nodes: impl IntoIterator<Item = &'a Node>) -> EffectLevel {
    let mut acc = EffectLevel::Pure;
    for child in nodes {
        // Use lattice_join (not compose) for SUMMARISING the effect of a
        // node tree  -  within one node tree the user's intent is encoded;
        // the lattice's job is to summarise, not to refuse. Refusal applies
        // only when fusing two programs (see `compose`).
        acc = lattice_join(acc, node_effect_level(child));
    }
    acc
}

fn expr_effect_level(expr: &Expr) -> EffectLevel {
    match expr {
        Expr::Atomic { .. } => EffectLevel::ReadWriteAtomic(AtomicOrdering::SeqCst),
        Expr::Load { .. } => EffectLevel::ReadAtomic,
        _ => EffectLevel::Pure,
    }
}

#[allow(unreachable_patterns)]
fn barrier_effect(ordering: crate::memory_model::MemoryOrdering) -> EffectLevel {
    use crate::memory_model::MemoryOrdering;
    match ordering {
        MemoryOrdering::GridSync => EffectLevel::Synchronized(SyncScope::Grid),
        MemoryOrdering::Relaxed => EffectLevel::ReadWriteAtomic(AtomicOrdering::Acquire),
        // Conservative default  -  an unrecognized ordering is treated as the
        // strongest available.
        _ => EffectLevel::Synchronized(SyncScope::Workgroup),
    }
}

fn is_invocation_id_eq_constant(cond: &Expr) -> bool {
    use crate::ir::BinOp;
    match cond {
        Expr::BinOp {
            op: BinOp::Eq | BinOp::Ne,
            left,
            right,
        } => {
            is_invocation_id_expr(left) && matches!(**right, Expr::LitU32(_))
                || is_invocation_id_expr(right) && matches!(**left, Expr::LitU32(_))
        }
        _ => false,
    }
}

fn is_invocation_id_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::InvocationId { .. } | Expr::LocalId { .. } | Expr::SubgroupLocalId
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};
    use crate::memory_model::MemoryOrdering;

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn divergent_store_program() -> Program {
        // if invocation_id == 0 { store buf[0] = 1 }  -  Diverging by construction.
        Program::wrapped(
            vec![buf()],
            [256, 1, 1],
            vec![Node::if_then(
                Expr::BinOp {
                    op: BinOp::Eq,
                    left: Box::new(Expr::gid_x()),
                    right: Box::new(Expr::u32(0)),
                },
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
            )],
        )
    }

    #[test]
    fn pure_composes_with_pure() {
        let r = compose(EffectLevel::Pure, EffectLevel::Pure);
        assert_eq!(r, Ok(EffectLevel::Pure));
    }

    #[test]
    fn pure_then_diverging_refuses_with_grid_sync_fix() {
        let r = compose(EffectLevel::Pure, EffectLevel::Diverging);
        match r {
            Err(RefusalReason::EffectLatticeViolation {
                producer,
                consumer,
                suggested_fix,
            }) => {
                assert_eq!(producer, "pure");
                assert_eq!(consumer, "diverging");
                assert!(
                    suggested_fix.contains("GridSync"),
                    "fix string must name MemoryOrdering::GridSync; got: {suggested_fix}"
                );
            }
            other => panic!(
                "expected EffectLatticeViolation refusing to fuse Pure with Diverging consumer; \
                 got {other:?}"
            ),
        }
    }

    #[test]
    fn diverging_then_pure_refuses_with_grid_sync_fix() {
        let r = compose(EffectLevel::Diverging, EffectLevel::Pure);
        match r {
            Err(RefusalReason::EffectLatticeViolation {
                producer,
                consumer,
                suggested_fix,
            }) => {
                assert_eq!(producer, "diverging");
                assert_eq!(consumer, "pure");
                assert!(
                    suggested_fix.contains("GridSync"),
                    "fix string must name MemoryOrdering::GridSync; got: {suggested_fix}"
                );
            }
            other => panic!(
                "expected EffectLatticeViolation refusing to fuse Diverging producer with Pure; \
                 got {other:?}"
            ),
        }
    }

    #[test]
    fn diverging_followed_by_grid_sync_composes() {
        // The recall-zero-past-block-zero fix: if you put a GridSync after the
        // divergent producer, the lattice accepts the composition and reports
        // the combined effect as Synchronized(Grid).
        let r = compose(
            EffectLevel::Diverging,
            EffectLevel::Synchronized(SyncScope::Grid),
        );
        assert_eq!(r, Ok(EffectLevel::Synchronized(SyncScope::Grid)));
    }

    #[test]
    fn read_write_atomic_compose_joins_orderings() {
        let r = compose(
            EffectLevel::ReadWriteAtomic(AtomicOrdering::Acquire),
            EffectLevel::ReadWriteAtomic(AtomicOrdering::Release),
        );
        assert_eq!(
            r,
            Ok(EffectLevel::ReadWriteAtomic(AtomicOrdering::AcqRel)),
            "Acquire ∘ Release must synthesize AcqRel  -  without this the lattice would lose \
             the Release-side guarantee"
        );
    }

    #[test]
    fn synchronized_compose_joins_to_larger_scope() {
        let r = compose(
            EffectLevel::Synchronized(SyncScope::Workgroup),
            EffectLevel::Synchronized(SyncScope::Grid),
        );
        assert_eq!(
            r,
            Ok(EffectLevel::Synchronized(SyncScope::Grid)),
            "Workgroup ∘ Grid must escalate to Grid; the smaller scope is absorbed"
        );
    }

    #[test]
    fn from_class_lifts_every_existing_side_effect_class() {
        assert_eq!(
            EffectLevel::from_class(SideEffectClass::Pure),
            EffectLevel::Pure
        );
        assert_eq!(
            EffectLevel::from_class(SideEffectClass::ReadsMemory),
            EffectLevel::ReadAtomic
        );
        assert!(matches!(
            EffectLevel::from_class(SideEffectClass::WritesMemory),
            EffectLevel::ReadWriteAtomic(_)
        ));
        assert!(matches!(
            EffectLevel::from_class(SideEffectClass::Synchronizing),
            EffectLevel::Synchronized(_)
        ));
        assert!(matches!(
            EffectLevel::from_class(SideEffectClass::Atomic),
            EffectLevel::ReadWriteAtomic(_)
        ));
    }

    #[test]
    fn program_effect_level_detects_divergent_store_pattern() {
        let p = divergent_store_program();
        let level = program_effect_level(&p);
        // The outer wrapper is a Region; the inner If-then with `gid==0` is
        // Diverging. The program-level effect propagates that.
        assert_eq!(
            level,
            EffectLevel::Diverging,
            "a program containing `if invocation_id == K {{ store ... }}` must surface as \
             Diverging at the program level  -  without this the fusion-refusal pass cannot \
             catch the recall-zero-past-block-zero shape"
        );
    }

    #[test]
    fn program_effect_level_pure_program_stays_pure() {
        let p = Program::wrapped(vec![buf()], [1, 1, 1], vec![Node::Return]);
        let level = program_effect_level(&p);
        assert_eq!(
            level,
            EffectLevel::Pure,
            "a pure program (just Return) must stay Pure  -  without this every pass would \
             see a stronger effect than the program actually has"
        );
    }

    #[test]
    fn barrier_grid_sync_node_lifts_to_synchronized_grid() {
        let node = Node::Barrier {
            ordering: MemoryOrdering::GridSync,
        };
        assert_eq!(
            node_effect_level(&node),
            EffectLevel::Synchronized(SyncScope::Grid)
        );
    }

    #[test]
    fn store_node_lifts_to_read_write_atomic() {
        let node = Node::store("buf", Expr::u32(0), Expr::u32(7));
        assert!(matches!(
            node_effect_level(&node),
            EffectLevel::ReadWriteAtomic(_)
        ));
    }

    #[test]
    fn divergent_program_paired_with_grid_sync_composes_cleanly() {
        // Diverging program followed by Synchronized(Grid) program  -  the
        // canonical fix. The fusion pass would compose the two and get Ok back.
        let div = program_effect_level(&divergent_store_program());
        let sync = EffectLevel::Synchronized(SyncScope::Grid);
        let composed = compose(div, sync);
        assert_eq!(composed, Ok(EffectLevel::Synchronized(SyncScope::Grid)));
    }

    #[test]
    fn read_atomic_then_pure_stays_read_atomic() {
        let r = compose(EffectLevel::ReadAtomic, EffectLevel::Pure);
        assert_eq!(r, Ok(EffectLevel::ReadAtomic));
    }

    #[test]
    fn synchronized_then_synchronized_subgroup_does_not_swallow_grid() {
        let r = compose(
            EffectLevel::Synchronized(SyncScope::Grid),
            EffectLevel::Synchronized(SyncScope::Subgroup),
        );
        assert_eq!(
            r,
            Ok(EffectLevel::Synchronized(SyncScope::Grid)),
            "Grid scope MUST dominate Subgroup scope on join  -  losing the larger scope would \
             silently downgrade the program's synchronization guarantee"
        );
    }
}

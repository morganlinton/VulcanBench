//! Loop / reduction catalog (Phase 4B).
//!
//! Loop-shape rewrites: trip-count zero elimination + bounded
//! compile-time unroll + redundant-bound-check guard elimination. These are
//! IR-level transformations; target-specific loop emission remains inside the
//! driver crates.

/// Tighten a `Node::Loop` upper bound when its body is a single
/// `If(Lt(Var(loop_var), Lit(n)), ...)` with `n < to` (ROADMAP A19).
pub mod loop_bound_tighten;
/// Fission a single `Node::Loop` body into two sibling loops sharing
/// the same iteration space when the body partitions cleanly into
/// buffer-disjoint, name-flow-isolated halves (ROADMAP A27).
pub mod loop_fission;
/// Fuse adjacent `Node::Loop` siblings whose bounds match and whose
/// bodies touch disjoint buffer sets (ROADMAP A26).
pub mod loop_fusion;
/// Hoist loop-invariant `Node::Let` bindings out of `Node::Loop`
/// bodies (ROADMAP A17).
pub mod loop_licm;
/// Polyhedral lower-bound normalization: rewrite `Loop(i, lo, hi, body)`
/// with `lo > 0` to `Loop(i', 0, hi-lo, body[i := i'+lo])` so
/// downstream tile/strip-mine/fusion passes see canonical
/// `from=0` bounds (ROADMAP A30).
pub mod loop_lower_bound_normalize;
/// Peel the first iteration of `Node::Loop` when guarded by
/// `If(Eq(Var(loop_var), Lit(0)), ...)` (ROADMAP A28).
pub mod loop_peel;
/// Drop redundant `if loop_var < to { ... }` guards inside matching loops.
pub mod loop_redundant_bound_check_elide;
/// 2-stage Load-then-Store software pipelining: rewrite a loop
/// body whose Load + dependent Store touch distinct buffers into
/// prologue + steady-state-with-prefetch + epilogue (ROADMAP A31).
pub mod loop_software_pipeline;
/// Strip-mine large literal loops into tiled outer and fixed-size
/// inner loops (ROADMAP A29).
pub mod loop_strip_mine;
/// Drop `Node::Loop` whose compile-time-known trip count is zero.
pub mod loop_trip_zero_eliminate;
/// Compile-time-known bounded loop expansion.
pub mod loop_unroll;
/// Loop-induction range facts that fold known-true / known-false
/// `If(Cmp(Var(i), LitU32(n)), then, else)` conditions inside
/// `Loop(i, lo, hi, body)` (ROADMAP A16  -  range facts into branch
/// elision via the structural loop range).
pub mod loop_var_range_fold;
mod substitution;

//! Synchronization / effects catalog (Phase 4D).
//!
//! Barrier + memory-ordering rewrites: coalescing consecutive
//! `Node::Barrier` siblings into one barrier of the joined ordering.

/// Coalesce consecutive `Node::Barrier` siblings into one barrier of
/// the joined `MemoryOrdering`.
pub mod barrier_coalesce;

//! Tier 2.5 topological-data-analysis primitives (#15, #32).
//!
//! Persistent homology + simplicial-complex operations. Composes
//! with `vyre-primitives::math` and `vyre-primitives::graph`.

/// Vietoris-Rips filtration boundary-matrix construction (#15).
pub mod vietoris_rips;

/// Simplicial neural network message-passing step (#32). Triangle-
/// level boundary-operator message aggregation.
pub mod simplicial;

/// Full H_1 cycle counting on a Rips 1-skeleton (P-PRIM-4).
/// Computes (b0, b1, edge_count) via union-find in one pass.
pub mod betti_persistence;

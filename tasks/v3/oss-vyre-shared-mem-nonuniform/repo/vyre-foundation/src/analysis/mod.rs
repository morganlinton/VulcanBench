//! Static-analysis surface  -  graph views + adjacency analyses over
//! `Program::entry()`.
//!
//! Audit cleanup A12 (2026-04-30): grouped from `vyre-foundation/src/`
//! root scatter. The longer-term home for `validate/` may also collapse
//! here once that subdir's surface is reviewed.

/// `GraphView`  -  read-only adjacency view over a Program for analyses.
pub mod graph_view;

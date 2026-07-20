//! Layer 3 complete compute engines.
//!
//! Each engine is a self-contained GPU compute pipeline: structured input
//! in, compute passes on a real GPU backend, typed output back.
//!
//! This module owns backend execution helpers only: command graphs, work
//! partitioning policy, persistent queues, host-ingress compatibility, and
//! shared record/readback. Domain algorithms live in `vyre-libs` or
//! wrappers that consume vyre.

/// Per-thread scratch arenas for record/readback hot-path vectors.
pub(crate) mod dispatch_scratch;
/// GPU-resident command graph execution.
pub mod graph;
/// Mockable multi-GPU work partitioning.
pub mod multi_gpu;
/// Resident persistent-kernel queue engine.
pub mod persistent;
/// Shared command recording and readback for vyre IR dispatch paths.
pub(crate) mod record_and_readback;
/// Host-ingress chunk bridge for callers that still receive bytes through CPU memory.
pub mod streaming;

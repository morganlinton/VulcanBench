//! Scheduling, fusion, batching, and dispatch-strategy substrate modules.

pub mod branch_compaction;
pub mod frontier_partitioning;
pub mod frontier_typed_ir;
pub mod megakernel_schedule;
pub mod multi_corpus_batching;
pub mod planar_rewrite_pass_scheduler;
pub mod polyhedral_fusion;
pub mod spectral_schedule;
pub mod submodular_cache_eviction;

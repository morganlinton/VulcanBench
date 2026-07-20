//! Hardware dispatch, GPU residency, and device-bound evidence contracts.

pub mod device_resident_token_fact_graph;
pub(crate) mod dispatch_buffers;
pub(crate) mod dispatch_program_cache;
pub mod gpu_preprocessing_coverage;
pub mod gpu_probe_contract;
pub mod memory_ownership_contract;
pub(crate) mod scratch;

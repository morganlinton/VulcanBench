use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WgpuBackendConfig {
    pub max_retained_bytes: u64,
    pub default_max_matches: usize,
    pub max_dfa_matches: usize,
    pub default_ring_size: usize,
    pub stream_workers: usize,
    pub default_intrusive_lru_capacity: usize,
    pub array_queue_capacity: usize,
    pub max_validation_cache_entries: usize,
}

impl Default for WgpuBackendConfig {
    fn default() -> Self {
        Self {
            max_retained_bytes: 1 << 30, // 1GB
            default_max_matches: 65_536,
            max_dfa_matches: 4 * 1024 * 1024,
            default_ring_size: 1024,
            stream_workers: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
            default_intrusive_lru_capacity: 1024,
            array_queue_capacity: 1024,
            max_validation_cache_entries: 1024,
        }
    }
}

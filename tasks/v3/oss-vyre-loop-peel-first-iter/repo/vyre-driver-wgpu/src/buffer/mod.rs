//! Persistent GPU buffer handles and reusable allocation pools.

mod handle;
mod pool;

pub(crate) use handle::write_padded;
pub use handle::{
    BindGroupCache, BindGroupCacheStats, GpuBufferHandle, StagingBufferPool, StagingBufferPoolStats,
};
pub use pool::{BufferPool, BufferPoolStats};

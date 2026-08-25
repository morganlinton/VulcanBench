//! Power-of-two GPU buffer pool for persistent dispatch.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use crossbeam_queue::ArrayQueue;
use vyre_driver::accounting::{
    pinning_atomic_add_usize_with_order, repair_atomic_sub_usize_fetch_with_order,
};
use vyre_driver::{BackendError, DispatchConfig};

use super::handle::GpuBufferHandle;

#[derive(Debug, Default)]
#[repr(align(64))]
struct PaddedAtomicUsize(AtomicUsize);

impl PaddedAtomicUsize {
    fn new(value: usize) -> Self {
        Self(AtomicUsize::new(value))
    }

    fn load(&self, order: Ordering) -> usize {
        self.0.load(order)
    }

    fn fetch_add(&self, value: usize, order: Ordering) -> usize {
        pinning_atomic_add_usize_with_order(&self.0, value, order, Ordering::Relaxed, |_, _| {
            tracing::error!(
                "WGPU buffer-pool counter reached usize::MAX. Fix: reset pool stats or shard retained-buffer accounting before counters wrap."
            );
        })
    }

    fn fetch_sub(&self, value: usize, order: Ordering) -> usize {
        repair_atomic_sub_usize_fetch_with_order(
            &self.0,
            value,
            Ordering::Relaxed,
            order,
            Ordering::Relaxed,
            |_, _| {
                tracing::error!(
                    "WGPU buffer-pool counter underflow was repaired to zero. Fix: rebuild buffer-pool accounting before continuing."
                );
            },
        )
    }
}

/// Snapshot of [`BufferPool`] counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BufferPoolStats {
    /// docs
    pub allocations: usize,
    /// docs
    pub hits: usize,
    /// docs
    pub releases: usize,
    /// docs
    pub evictions: usize,
    /// docs
    pub retained_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct PoolStats {
    allocations: PaddedAtomicUsize,
    hits: PaddedAtomicUsize,
    releases: PaddedAtomicUsize,
    evictions: PaddedAtomicUsize,
    retained_bytes: PaddedAtomicUsize,
}

impl Default for PoolStats {
    fn default() -> Self {
        Self {
            allocations: PaddedAtomicUsize::new(0),
            hits: PaddedAtomicUsize::new(0),
            releases: PaddedAtomicUsize::new(0),
            evictions: PaddedAtomicUsize::new(0),
            retained_bytes: PaddedAtomicUsize::new(0),
        }
    }
}

#[derive(Clone)]
pub(crate) struct PoolReturn {
    inner: Weak<PoolInner>,
}

/// Reusable GPU buffer pool.
#[derive(Clone)]
pub struct BufferPool {
    inner: Arc<PoolInner>,
}

impl fmt::Debug for BufferPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferPool")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

const NUM_SIZE_CLASSES: usize = 64;
const DEFAULT_MAX_RETAINED_BYTES: usize = 1 << 30;
const MAX_FREE_ENTRIES_PER_BUCKET: usize = 1024;

/// Canonical usage masks used to key each size-class sub-bucket.
///
/// Reduces the full `wgpu::BufferUsages` bitfield to a small enum so
/// that alternating workloads (e.g. INPUT vs OUTPUT) no longer
/// collide in the same queue and fall through to fresh allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
enum UsageKind {
    Input = 0,
    Output = 1,
    Uniform = 2,
    Workgroup = 3,
    Other = 4,
}

const NUM_USAGE_KINDS: usize = 5;

impl UsageKind {
    fn index(self) -> usize {
        match self {
            Self::Input => 0,
            Self::Output => 1,
            Self::Uniform => 2,
            Self::Workgroup => 3,
            Self::Other => 4,
        }
    }
}

fn canonical_usage_kind(usage: wgpu::BufferUsages) -> UsageKind {
    use wgpu::BufferUsages as U;
    if usage == U::STORAGE | U::COPY_DST {
        UsageKind::Input
    } else if usage == U::STORAGE | U::COPY_SRC | U::COPY_DST | U::INDIRECT {
        UsageKind::Output
    } else if usage == U::UNIFORM | U::COPY_DST {
        UsageKind::Uniform
    } else if usage == U::STORAGE | U::COPY_SRC | U::COPY_DST {
        UsageKind::Workgroup
    } else {
        UsageKind::Other
    }
}

const TIERING_EVENT_CAPACITY_MIN: usize = 1024;
const TIERING_EVENT_CAPACITY_MAX: usize = 65_536;

/// Opt-in hot/cold tiered metadata layered over the power-of-two pool.
///
/// Off by default. Consumers that batch many small dispatches (inference
/// servers, Karyx streaming scanners, Soleno batched probes) wire one
/// via [`BufferPool::with_tiering`] and tag hot allocations through the
/// returned handle. The tiering layer records allocation reuse through
/// a bounded non-blocking event queue and drains it into `TieredCache`
/// on a dedicated metadata worker. This keeps acquire/release free of
/// a global mutex while preserving the cache policy's per-tier O(1)
/// LRU accounting.
///
/// Kept as `pub(crate) Option<Arc<...>>` so the absence of a tiering
/// policy costs exactly one `Option::is_none()` branch on the hot
/// acquire path.
pub(crate) struct PoolTiering {
    events: Sender<TieringEvent>,
    pending_events: Arc<AtomicUsize>,
    dropped_events: AtomicUsize,
}

#[derive(Clone, Copy, Debug)]
enum TieringEvent {
    Retain { key: u64, size: u64 },
    Access { key: u64 },
}

impl PoolTiering {
    fn new(
        cache: crate::runtime::cache::TieredCache,
        capacity: usize,
    ) -> Result<Self, BackendError> {
        let capacity = capacity.clamp(TIERING_EVENT_CAPACITY_MIN, TIERING_EVENT_CAPACITY_MAX);
        let (events, receiver) = bounded(capacity);
        let pending_events = Arc::new(AtomicUsize::new(0));
        let worker_pending = Arc::clone(&pending_events);
        std::thread::Builder::new()
            .name("vyre-buffer-tiering".to_string())
            .spawn(move || drain_tiering_events(cache, receiver, worker_pending))
            .map_err(|error| {
                BackendError::new(format!(
                    "failed to spawn vyre buffer tiering worker: {error}. Fix: raise process thread limits or disable buffer-pool tiering."
                ))
            })?;
        Ok(Self {
            events,
            pending_events,
            dropped_events: AtomicUsize::new(0),
        })
    }

    #[inline]
    fn record_retained(&self, key: u64, size: u64) {
        self.enqueue(TieringEvent::Retain { key, size });
    }

    #[inline]
    fn record_access(&self, key: u64) {
        self.enqueue(TieringEvent::Access { key });
    }

    #[inline]
    fn enqueue(&self, event: TieringEvent) {
        self.pending_events.fetch_add(1, Ordering::Release);
        match self.events.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.pending_events.fetch_sub(1, Ordering::AcqRel);
                self.dropped_events.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    #[cfg(test)]
    fn drain_all_for_test(&self) {
        // The metadata worker is woken via crossbeam channel; under
        // contention from multiple acquire/release threads it can
        // accumulate a backlog before the OS schedules it. Use bounded
        // adaptive parking rather than a fixed millisecond sleep; fixed
        // sleeps create thundering-herd latency under high test fanout.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut backoff = crate::wait_backoff::AdaptiveWaitBackoff::from_micros(64, 2, 50, 5);
        while std::time::Instant::now() < deadline {
            if self.pending_events.load(Ordering::Acquire) == 0 {
                return;
            }
            backoff.idle_until(deadline);
        }
        panic!("Fix: tiering metadata worker did not drain pending buffer-pool events");
    }

    #[cfg(test)]
    fn dropped_events_for_test(&self) -> usize {
        self.dropped_events.load(Ordering::Relaxed)
    }
}

fn drain_tiering_events(
    mut cache: crate::runtime::cache::TieredCache,
    receiver: Receiver<TieringEvent>,
    pending_events: Arc<AtomicUsize>,
) {
    while let Ok(event) = receiver.recv() {
        match event {
            TieringEvent::Retain { key, size } => {
                if cache.get(key).is_none() {
                    if let Err(error) = cache.insert(key, size) {
                        tracing::warn!(
                            "buffer pool tiering rejected retained buffer {key} ({size} bytes): {error}. Fix: increase tier capacity or disable tiering for oversized buffers."
                        );
                        pending_events.fetch_sub(1, Ordering::AcqRel);
                        continue;
                    }
                }
                cache.record_access(key);
                if let Err(error) = cache.promote(key) {
                    tracing::warn!(
                        "buffer pool tier promotion failed for retained buffer {key}: {error}. Fix: repair tier sizing or promotion accounting."
                    );
                }
            }
            TieringEvent::Access { key } => {
                cache.record_access(key);
                if let Err(error) = cache.promote(key) {
                    tracing::warn!(
                        "buffer pool tier promotion failed for accessed buffer {key}: {error}. Fix: repair tier sizing or promotion accounting."
                    );
                }
            }
        }
        pending_events.fetch_sub(1, Ordering::AcqRel);
    }
}

struct PoolInner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    free: [[ArrayQueue<FreeEntry>; NUM_USAGE_KINDS]; NUM_SIZE_CLASSES],
    non_empty_classes: AtomicU64,
    stats: PoolStats,
    max_retained_bytes: usize,
    /// Optional tiered cache. `None` = power-of-two pool only.
    tiering: Option<Arc<PoolTiering>>,
}

struct FreeEntry {
    buffer: Arc<wgpu::Buffer>,
    allocation_len: u64,
    usage: wgpu::BufferUsages,
}

impl BufferPool {
    #[must_use]
    /// docs
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, config: &DispatchConfig) -> Self {
        let max_retained_bytes = config
            .max_output_bytes
            .unwrap_or(DEFAULT_MAX_RETAINED_BYTES);
        let capacity = free_bucket_capacity(max_retained_bytes);
        let free = std::array::from_fn(|_| std::array::from_fn(|_| ArrayQueue::new(capacity)));
        Self {
            inner: Arc::new(PoolInner {
                device,
                queue,
                free,
                non_empty_classes: AtomicU64::new(0),
                stats: PoolStats::default(),
                max_retained_bytes,
                tiering: None,
            }),
        }
    }

    /// Opt-in hot/cold tiered caching on top of the power-of-two pool.
    ///
    /// Returns a new `BufferPool` that shares the underlying device
    /// and queue but wraps every acquire/release in a `TieredCache`
    /// governed by the supplied tiers + policy. Consumers that batch
    /// many small dispatches (inference servers, streaming scanners,
    /// batched probes) use this to keep hot allocations resident and
    /// demote/evict cold ones via per-tier O(1) LRU.
    ///
    /// The `tiers` vector is ordered coldest-first; `policy` controls
    /// promotion/eviction. Defaults at
    /// `TieredCache::try_new(vec![CacheTier::try_new("hot", 1 << 24)?,
    /// CacheTier::try_new("cold", 1 << 30)?])` are a reasonable starting
    /// point for 16 MiB hot / 1 GiB cold.
    pub fn with_tiering(
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: &DispatchConfig,
        tiers: Vec<crate::runtime::cache::CacheTier>,
    ) -> Result<Self, BackendError> {
        let mut pool = Self::new(device, queue, config);
        let tiered = crate::runtime::cache::TieredCache::try_new(tiers)?;
        let max_retained_bytes = config
            .max_output_bytes
            .unwrap_or(DEFAULT_MAX_RETAINED_BYTES);
        let event_capacity =
            free_bucket_capacity(max_retained_bytes).max(TIERING_EVENT_CAPACITY_MIN);
        let tiering = Arc::new(PoolTiering::new(tiered, event_capacity)?);
        let inner = Arc::get_mut(&mut pool.inner).ok_or_else(|| {
            BackendError::new(
                "buffer pool tiering could not get unique pool ownership during construction. Fix: attach tiering before cloning the pool.",
            )
        })?;
        inner.tiering = Some(tiering);
        Ok(pool)
    }

    #[must_use]
    /// docs
    pub fn queue(&self) -> &wgpu::Queue {
        &self.inner.queue
    }

    #[must_use]
    /// docs
    pub fn device(&self) -> &wgpu::Device {
        &self.inner.device
    }

    /// docs
    pub fn acquire(
        &self,
        len: u64,
        usage: wgpu::BufferUsages,
    ) -> Result<GpuBufferHandle, BackendError> {
        let allocation_len = size_class(len)?;
        let class_idx = class_index(allocation_len)?;
        let usage_kind = canonical_usage_kind(usage);

        // O(1) free-class search via trailing_zeros on the masked
        // non_empty_classes bitmap.  Within each size class we probe
        // only the sub-bucket that matches the canonical usage mask;
        // if that sub-bucket is empty we mask the class out and keep
        // scanning larger classes.  This eliminates the old "pop wrong
        // usage, push back, fall through to fresh alloc" path.
        let mut mask: u64 = if class_idx >= NUM_SIZE_CLASSES {
            0
        } else {
            !((1u64 << class_idx).wrapping_sub(1))
        };
        loop {
            let non_empty = self.inner.non_empty_classes.load(Ordering::Relaxed) & mask;
            if non_empty == 0 {
                break;
            }
            let idx = u32_to_usize(non_empty.trailing_zeros(), "buffer-pool non-empty class")?;
            if idx >= NUM_SIZE_CLASSES {
                break;
            }

            if let Some(entry) = self.inner.free[idx][usage_kind.index()].pop() {
                if let Some(tiering) = &self.inner.tiering {
                    let key = buffer_identity_key(Arc::as_ptr(&entry.buffer));
                    tiering.record_access(key);
                }
                // Defensive: if the stored usage doesn't cover the request,
                // route it to its correct canonical sub-bucket rather than
                // leaving it stranded in the wrong queue (POOL-1 point 4).
                if !entry.usage.contains(usage) {
                    let correct_kind = canonical_usage_kind(entry.usage);
                    let correct_class = match class_index(entry.allocation_len) {
                        Ok(class) => class,
                        Err(error) => {
                            tracing::warn!(
                                "buffer pool encountered an invalid retained entry while correcting usage metadata: {error}. Dropping the entry."
                            );
                            self.inner.stats.retained_bytes.fetch_sub(
                                u64_to_usize(entry.allocation_len, "retained buffer bytes")
                                    .unwrap_or(usize::MAX),
                                Ordering::Relaxed,
                            );
                            self.inner.stats.evictions.fetch_add(1, Ordering::Relaxed);
                            mask &= !(1 << idx);
                            continue;
                        }
                    };
                    match self.inner.free[correct_class][correct_kind.index()].push(entry) {
                        Ok(()) => {
                            if correct_class != idx {
                                self.inner
                                    .non_empty_classes
                                    .fetch_or(1 << correct_class, Ordering::Relaxed);
                            }
                        }
                        Err(overflow) => {
                            tracing::warn!(
                                "buffer pool class {correct_class} usage bucket {correct_kind:?} is full while correcting a wrong-usage entry; dropping {} retained bytes. Fix: increase max_output_bytes or inspect usage canonicalization drift.",
                                overflow.allocation_len
                            );
                            self.inner.stats.retained_bytes.fetch_sub(
                                u64_to_usize(overflow.allocation_len, "retained buffer bytes")
                                    .unwrap_or(usize::MAX),
                                Ordering::Relaxed,
                            );
                            self.inner.stats.evictions.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    mask &= !(1 << idx);
                    continue;
                }

                if self.inner.free[idx].iter().all(|q| q.is_empty()) {
                    self.inner
                        .non_empty_classes
                        .fetch_and(!(1 << idx), Ordering::Relaxed);
                }
                self.inner.stats.retained_bytes.fetch_sub(
                    u64_to_usize(entry.allocation_len, "retained buffer bytes")
                        .unwrap_or(usize::MAX),
                    Ordering::Relaxed,
                );
                let host_len = usize::try_from(len).map_err(|error| {
                    BackendError::new(format!(
                        "GpuBufferPool::acquire received logical byte length {len} that does not fit usize on this host while reusing a pooled buffer: {error}. Fix: shard the GPU buffer before allocating or run on a host with a wide enough address space."
                    ))
                })?;
                self.inner.stats.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(GpuBufferHandle::from_parts(
                    entry.buffer,
                    len,
                    entry.allocation_len,
                    host_len,
                    entry.usage,
                    Some(self.pool_return()),
                ));
            }

            // Sub-bucket empty (lost race or genuinely empty).  Clear the
            // class bit only when *every* sub-bucket is empty so that other
            // usage kinds are not disturbed.
            if self.inner.free[idx].iter().all(|q| q.is_empty()) {
                self.inner
                    .non_empty_classes
                    .fetch_and(!(1 << idx), Ordering::Relaxed);
            }
            mask &= !(1 << idx);
        }

        let host_len = usize::try_from(len).map_err(|error| {
            BackendError::new(format!(
                "GpuBufferPool::acquire received logical byte length {len} that does not fit usize on this host: {error}. Fix: shard the GPU buffer before allocating or run on a host with a wide enough address space."
            ))
        })?;
        let buffer = self.inner.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vyre persistent pooled buffer"),
            size: allocation_len,
            usage,
            mapped_at_creation: false,
        });
        self.inner.stats.allocations.fetch_add(1, Ordering::Relaxed);
        Ok(GpuBufferHandle::from_parts(
            Arc::new(buffer),
            len,
            allocation_len,
            host_len,
            usage,
            Some(self.pool_return()),
        ))
    }

    /// docs
    pub fn release(&self, handle: GpuBufferHandle) {
        drop(handle);
    }

    #[must_use]
    /// docs
    pub fn stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            allocations: self.inner.stats.allocations.load(Ordering::Relaxed),
            hits: self.inner.stats.hits.load(Ordering::Relaxed),
            releases: self.inner.stats.releases.load(Ordering::Relaxed),
            evictions: self.inner.stats.evictions.load(Ordering::Relaxed),
            retained_bytes: self.inner.stats.retained_bytes.load(Ordering::Relaxed),
        }
    }

    fn pool_return(&self) -> PoolReturn {
        PoolReturn {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

impl PoolReturn {
    pub(crate) fn release(
        self,
        buffer: Arc<wgpu::Buffer>,
        _byte_len: u64,
        allocation_len: u64,
        usage: wgpu::BufferUsages,
    ) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        let class_idx = match class_index(allocation_len) {
            Ok(class) => class,
            Err(error) => {
                tracing::warn!(
                    "dropping persistent pooled buffer with invalid allocation size {allocation_len}: {error}. Fix: keep GpuBufferHandle allocation metadata produced by BufferPool::acquire."
                );
                inner.stats.evictions.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };
        let usage_kind = canonical_usage_kind(usage);

        if let Some(tiering) = &inner.tiering {
            let key = buffer_identity_key(Arc::as_ptr(&buffer));
            tiering.record_retained(key, allocation_len);
        }

        if inner.free[class_idx][usage_kind.index()]
            .push(FreeEntry {
                buffer,
                allocation_len,
                usage,
            })
            .is_ok()
        {
            inner
                .non_empty_classes
                .fetch_or(1 << class_idx, Ordering::Relaxed);
            inner.stats.releases.fetch_add(1, Ordering::Relaxed);
            let Ok(allocation_len_usize) = u64_to_usize(allocation_len, "retained buffer bytes")
            else {
                inner.stats.evictions.fetch_add(1, Ordering::Relaxed);
                return;
            };
            inner
                .stats
                .retained_bytes
                .fetch_add(allocation_len_usize, Ordering::Relaxed);

            while inner.stats.retained_bytes.load(Ordering::Relaxed) > inner.max_retained_bytes {
                let mask = inner.non_empty_classes.load(Ordering::Relaxed);
                if mask == 0 {
                    break;
                }
                let leading_zeros = match u32_to_usize(
                    mask.leading_zeros(),
                    "buffer-pool non-empty leading-zero count",
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        tracing::warn!(
                            "buffer-pool eviction could not convert leading-zero count: {error}. Fix: keep buffer-pool class bitmap width representable by host usize."
                        );
                        inner.non_empty_classes.store(0, Ordering::Relaxed);
                        break;
                    }
                };
                let highest_class = if leading_zeros >= NUM_SIZE_CLASSES {
                    inner.non_empty_classes.store(0, Ordering::Relaxed);
                    break;
                } else {
                    NUM_SIZE_CLASSES - 1 - leading_zeros
                };
                let mut evicted = None;
                for kind in 0..NUM_USAGE_KINDS {
                    if let Some(e) = inner.free[highest_class][kind].pop() {
                        evicted = Some(e);
                        break;
                    }
                }
                if let Some(evicted) = evicted {
                    inner.stats.retained_bytes.fetch_sub(
                        u64_to_usize(evicted.allocation_len, "retained buffer bytes")
                            .unwrap_or(usize::MAX),
                        Ordering::Relaxed,
                    );
                    inner.stats.evictions.fetch_add(1, Ordering::Relaxed);
                    if inner.free[highest_class].iter().all(|q| q.is_empty()) {
                        inner
                            .non_empty_classes
                            .fetch_and(!(1 << highest_class), Ordering::Relaxed);
                    }
                } else {
                    inner
                        .non_empty_classes
                        .fetch_and(!(1 << highest_class), Ordering::Relaxed);
                }
            }
        }
    }
}

fn buffer_identity_key(buffer: *const wgpu::Buffer) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    buffer.addr().hash(&mut hasher);
    hasher.finish()
}

fn u64_to_usize(value: u64, label: &'static str) -> Result<usize, BackendError> {
    usize::try_from(value).map_err(|source| {
        BackendError::new(format!(
            "WGPU buffer-pool {label} value {value} cannot fit usize: {source}. Fix: shard retained-buffer accounting before pooling."
        ))
    })
}

fn u32_to_usize(value: u32, label: &'static str) -> Result<usize, BackendError> {
    usize::try_from(value).map_err(|source| {
        BackendError::new(format!(
            "WGPU buffer-pool {label} value {value} cannot fit usize: {source}. Fix: keep buffer-pool class indices within host index width."
        ))
    })
}

fn size_class(len: u64) -> Result<u64, BackendError> {
    len.max(4).checked_next_power_of_two().ok_or_else(|| {
        BackendError::new(format!(
            "buffer length {len} cannot be rounded to a power-of-two persistent pool size class without overflowing u64. Fix: split the dispatch into smaller buffers."
        ))
    })
}

fn class_index(len: u64) -> Result<usize, BackendError> {
    let normalized = len.max(4);
    if !normalized.is_power_of_two() {
        return Err(BackendError::new(format!(
            "buffer allocation length {len} is not a power-of-two persistent pool size class. Fix: only release handles produced by BufferPool::acquire."
        )));
    }
    let idx = u32_to_usize(normalized.trailing_zeros(), "buffer size class")?;
    if idx >= NUM_SIZE_CLASSES {
        return Err(BackendError::new(format!(
            "buffer size class index {idx} exceeds the {NUM_SIZE_CLASSES}-class persistent buffer pool. Fix: split the dispatch into smaller buffers."
        )));
    }
    Ok(idx)
}

fn free_bucket_capacity(max_retained_bytes: usize) -> usize {
    (max_retained_bytes / 4)
        .max(1)
        .min(MAX_FREE_ENTRIES_PER_BUCKET)
}

#[cfg(test)]
mod tests {
    use super::{class_index, free_bucket_capacity, size_class, BufferPool};
    use proptest::prelude::*;

    #[test]
    fn retained_byte_budget_is_not_used_as_queue_capacity() {
        assert_eq!(
            free_bucket_capacity(1 << 30),
            1024,
            "Fix: a 1 GiB byte budget must not allocate 1 GiB queue slots per bucket"
        );
        assert_eq!(
            free_bucket_capacity(8),
            2,
            "Fix: tiny retained-byte budgets should still translate to bounded entry capacity"
        );
    }

    #[test]
    fn oversized_size_classes_return_errors_instead_of_panicking() {
        let error = size_class((1u64 << 63) + 1)
            .expect_err("oversized buffer length must be rejected before pool indexing");
        assert!(
            error
                .to_string()
                .contains("power-of-two persistent pool size class"),
            "unexpected error: {error}"
        );

        assert_eq!(
            class_index(0).expect("Fix: minimum size class should fit"),
            2
        );
        let error =
            class_index(u64::MAX).expect_err("invalid retained allocation length must be rejected");
        assert!(
            error.to_string().contains("not a power-of-two"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn production_pool_class_selection_has_no_narrowing_casts() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/buffer/pool.rs"))
                .expect("Fix: buffer pool source must be readable");
        let production = src
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: meta-test scans production sources; update fixture path if module moved - production section must exist");
        assert!(
            !production.contains(" as usize"),
            "buffer-pool release/acquire class selection must use checked conversion helpers"
        );
        assert!(production.contains("u32_to_usize("));
        assert!(production.contains("mask.leading_zeros()"));
    }

    #[test]
    fn acquire_release_reuses_power_of_two_classes() {
        let arc = crate::runtime::cached_device()
            .expect("Fix: GPU device is required for persistent buffer pool test");
        let (device, queue) = &*arc;
        let config = vyre_driver::DispatchConfig::default();
        let pool = BufferPool::new(device.clone(), queue.clone(), &config);
        for len in 1..=1000 {
            let handle = pool
                .acquire(
                    len,
                    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                )
                .expect("Fix: pooled allocation should succeed");
            pool.release(handle);
        }
        assert!(
            pool.stats().allocations <= 16,
            "Fix: pool should allocate by power-of-two classes, stats={:?}",
            pool.stats()
        );
    }

    #[test]
    fn pooled_reuse_updates_logical_element_count() {
        let arc = crate::runtime::cached_device()
            .expect("Fix: GPU device is required for persistent buffer pool test");
        let (device, queue) = &*arc;
        let config = vyre_driver::DispatchConfig::default();
        let pool = BufferPool::new(device.clone(), queue.clone(), &config);
        let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

        let large = pool
            .acquire(64, usage)
            .expect("Fix: initial pooled allocation should succeed");
        assert_eq!(large.element_count(), 64);
        pool.release(large);

        let small = pool
            .acquire(7, usage)
            .expect("Fix: pooled reuse should succeed");
        assert_eq!(
            small.element_count(),
            7,
            "Fix: reusing a larger allocation must not leak the previous logical element count"
        );
        assert_eq!(small.byte_len(), 7);
    }

    #[test]
    fn tiering_acquire_release_is_nonblocking_under_contention() {
        let arc = crate::runtime::cached_device()
            .expect("Fix: GPU device is required for persistent buffer pool test");
        let (device, queue) = &*arc;
        let config = vyre_driver::DispatchConfig::default();
        let pool = BufferPool::with_tiering(
            device.clone(),
            queue.clone(),
            &config,
            vec![crate::runtime::cache::CacheTier::new("hot", 1 << 20)],
        )
        .expect("Fix: tiered buffer pool construction should succeed");
        let handle = pool
            .acquire(
                64,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            )
            .expect("Fix: acquire before poisoning should succeed");
        let tiering = pool
            .inner
            .tiering
            .as_ref()
            .expect("Fix: with_tiering must attach a tiering policy")
            .clone();
        pool.release(handle);
        let mut workers = Vec::new();
        for _ in 0..4 {
            let pool = pool.clone();
            workers.push(std::thread::spawn(move || {
                for _ in 0..32 {
                    let handle = pool
                        .acquire(
                            64,
                            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        )
                        .expect("Fix: pooled allocation should not fail under tiering contention");
                    pool.release(handle);
                }
            }));
        }
        for worker in workers {
            worker
                .join()
                .expect("Fix: buffer-pool contention worker must not panic");
        }
        tiering.drain_all_for_test();
        assert_eq!(
            tiering.dropped_events_for_test(),
            0,
            "Fix: normal contention must not drop tiering metadata events"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn alternating_usage_hit_rate(
            sizes in prop::collection::vec(1u64..=65536, 20..=200),
        ) {
            let arc = crate::runtime::cached_device()
                .expect("Fix: GPU device is required for persistent buffer pool test");
            let (device, queue) = &*arc;
            let config = vyre_driver::DispatchConfig::default();
            let pool = BufferPool::new(device.clone(), queue.clone(), &config);

            let usage_a = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
            let usage_b = wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::INDIRECT;

            // Round 1: acquire alternating usages, then release everything.
            let mut handles = Vec::with_capacity(sizes.len());
            for (i, &len) in sizes.iter().enumerate() {
                let usage = if i % 2 == 0 { usage_a } else { usage_b };
                handles.push(pool.acquire(len, usage).unwrap());
            }
            for h in handles {
                pool.release(h);
            }

            let stats_after_first = pool.stats();
            prop_assert_eq!(
                stats_after_first.hits, 0,
                "first round should be 100% fresh allocations"
            );

            // Round 2: identical pattern.
            let mut handles = Vec::with_capacity(sizes.len());
            for (i, &len) in sizes.iter().enumerate() {
                let usage = if i % 2 == 0 { usage_a } else { usage_b };
                handles.push(pool.acquire(len, usage).unwrap());
            }
            for h in handles {
                pool.release(h);
            }

            let stats_after_second = pool.stats();
            let second_round_hits = stats_after_second.hits - stats_after_first.hits;
            let total = sizes.len();
            let hit_rate = second_round_hits as f64 / total as f64;
            prop_assert!(
                hit_rate >= 0.95,
                "second round hit rate should be >= 95%, got {:.2}% ({}/{}), stats={:?}",
                hit_rate * 100.0,
                second_round_hits,
                total,
                stats_after_second
            );
        }
    }
}

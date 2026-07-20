//! Public persistent GPU buffer handle.

use std::cmp::{Ordering as CmpOrdering, Reverse};
use std::collections::BinaryHeap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::time::Instant;

use dashmap::DashMap;
use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;
use vyre_driver::BackendError;

use super::pool::PoolReturn;

static NEXT_BUFFER_ID: AtomicU64 = AtomicU64::new(1);
static RESIDENT_BUFFERS: OnceLock<DashMap<u64, Weak<GpuBufferInner>>> = OnceLock::new();
const STAGING_BUFFER_POOL_CLASS_CAP: usize = 16;

fn resident_buffers() -> &'static DashMap<u64, Weak<GpuBufferInner>> {
    RESIDENT_BUFFERS.get_or_init(DashMap::new)
}

fn pointer_identity_key<T>(ptr: *const T) -> u64 {
    let mut hasher = FxHasher::default();
    ptr.addr().hash(&mut hasher);
    hasher.finish()
}

/// Cheaply cloneable handle for a GPU-resident buffer.
///
/// The handle records the byte length originally requested by the caller,
/// the backing allocation length, the logical element count, and the actual
/// usage flags used to create the underlying `wgpu::Buffer`.
#[derive(Clone)]
pub struct GpuBufferHandle {
    inner: Arc<GpuBufferInner>,
}

struct GpuBufferInner {
    id: u64,
    buffer: Arc<wgpu::Buffer>,
    byte_len: u64,
    allocation_len: u64,
    element_count: usize,
    usage: wgpu::BufferUsages,
    pool_return: Option<PoolReturn>,
}

/// Snapshot of [`StagingBufferPool`] counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StagingBufferPoolStats {
    /// Number of fresh GPU buffer allocations.
    pub allocations: usize,
    /// Number of times a free buffer was reused.
    pub hits: usize,
}

/// Device-local staging buffer pool keyed by `(size, usage)`.
///
/// Hot dispatch paths (e.g. [`GpuBufferHandle::readback_until`]) acquire
/// readback staging buffers from this pool instead of creating a fresh
/// `wgpu::Buffer` on every call. Each `(size, usage)` class is capped at
/// [`STAGING_BUFFER_POOL_CLASS_CAP`] entries; evictions drop the
/// least-recently-used buffer.
#[derive(Clone, Default)]
pub struct StagingBufferPool {
    inner: Arc<Mutex<StagingBufferPoolInner>>,
}

impl std::fmt::Debug for StagingBufferPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StagingBufferPool").finish_non_exhaustive()
    }
}

#[derive(Default)]
struct StagingBufferPoolInner {
    free: FxHashMap<(u64, u32), SmallVec<[wgpu::Buffer; STAGING_BUFFER_POOL_CLASS_CAP]>>,
    allocations: usize,
    hits: usize,
}

impl StagingBufferPool {
    fn lock_inner(&self) -> MutexGuard<'_, StagingBufferPoolInner> {
        self.inner.lock().unwrap_or_else(|error| {
            tracing::error!(
                "Vyre WGPU staging buffer pool lock was poisoned: {error}. Fix: discard the pool after a panic; continuing with recovered state."
            );
            error.into_inner()
        })
    }

    /// Create an empty staging buffer pool.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return allocation and hit counters.
    #[must_use]
    pub fn stats(&self) -> StagingBufferPoolStats {
        let inner = self.lock_inner();
        StagingBufferPoolStats {
            allocations: inner.allocations,
            hits: inner.hits,
        }
    }

    /// Acquire a staging buffer with exactly `size` bytes and `usage`.
    ///
    /// Reuses a free buffer when one is available; otherwise creates a fresh
    /// GPU allocation and increments the allocation counter.
    pub fn acquire(
        &self,
        device: &wgpu::Device,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        let key = (size, usage.bits());
        let mut inner = self.lock_inner();
        if let Some(buffers) = inner.free.get_mut(&key) {
            if let Some(buffer) = buffers.pop() {
                inner.hits += 1;
                return buffer;
            }
        }
        inner.allocations += 1;
        drop(inner);
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vyre staging readback"),
            size,
            usage,
            mapped_at_creation: false,
        })
    }

    /// Release a staging buffer back to the pool.
    ///
    /// The buffer is pushed to the MRU position of its `(size, usage)` class.
    /// If the class already holds 16 buffers, the LRU entry is dropped.
    pub fn release(&self, buffer: wgpu::Buffer, size: u64, usage: wgpu::BufferUsages) {
        let key = (size, usage.bits());
        let mut inner = self.lock_inner();
        let buffers = inner.free.entry(key).or_insert_with(SmallVec::new);
        if buffers.len() == STAGING_BUFFER_POOL_CLASS_CAP {
            buffers.remove(0);
        }
        buffers.push(buffer);
    }
}

impl GpuBufferHandle {
    /// Upload `bytes` into a new GPU buffer.
    ///
    /// The created buffer always includes `COPY_DST` so the upload is legal.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the requested allocation length cannot fit
    /// `u64`.
    pub fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        usage: wgpu::BufferUsages,
    ) -> Result<Self, BackendError> {
        let allocation_len = aligned_len(bytes.len())?;
        let final_usage = usage | wgpu::BufferUsages::COPY_DST;
        // Fast path: create the buffer already mapped and memcpy host bytes
        // DIRECTLY into its host-visible / BAR backing store, then unmap. This
        // is ONE host copy with no wgpu-internal staging buffer and no GPU copy
        // command — the slow `queue.write_buffer` path routes every large upload
        // through wgpu's `StagingBelt`, which on Vulkan allocates + maps a fresh
        // staging buffer per write (the ~90 MB/s catalog-upload bottleneck on
        // the ~1 GB megakernel DFA catalog). `mapped_at_creation` works for ANY
        // usage flags (it does not require MAP_WRITE) and is correct for ALL
        // sizes, so it replaces the staged path unconditionally for non-empty
        // uploads. Zero-length buffers cannot be mapped at creation, so they
        // take the (no-op) `write_padded` path below — `aligned_len(0) == 0`,
        // and wgpu rejects a 0-byte `mapped_at_creation` buffer.
        if allocation_len > 0 {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vyre persistent upload"),
                size: allocation_len,
                usage: final_usage,
                mapped_at_creation: true,
            });
            {
                let mut mapped = buffer.slice(..).get_mapped_range_mut();
                crate::padded_upload::write_padded_into_mapped(&mut mapped, bytes)?;
            }
            buffer.unmap();
            let logical_len = u64::try_from(bytes.len()).map_err(|source| {
                BackendError::new(format!(
                    "GPU upload logical byte length cannot fit u64: {source}. Fix: split the dispatch input."
                ))
            })?;
            return Ok(Self::from_parts(
                Arc::new(buffer),
                logical_len,
                allocation_len,
                bytes.len(),
                final_usage,
                None,
            ));
        }
        // Zero-length upload: allocate a minimal buffer (wgpu forbids both a
        // 0-byte allocation and a 0-byte mapped_at_creation buffer). `write_padded`
        // is a no-op here; the handle reports a logical length of 0.
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vyre persistent upload"),
            size: allocation_len,
            usage: final_usage,
            mapped_at_creation: false,
        });
        write_padded(queue, &buffer, bytes, allocation_len)?;
        Ok(Self::from_parts(
            Arc::new(buffer),
            0,
            allocation_len,
            bytes.len(),
            final_usage,
            None,
        ))
    }

    /// Allocate a GPU-resident buffer without uploading host contents.
    ///
    /// # Errors
    ///
    /// Returns a backend error when `len` cannot be represented as a valid
    /// wgpu buffer size.
    pub fn alloc(
        device: &wgpu::Device,
        len: u64,
        usage: wgpu::BufferUsages,
    ) -> Result<Self, BackendError> {
        let allocation_len = aligned_len_u64(len, "persistent GPU allocation length")?;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vyre persistent alloc"),
            size: allocation_len,
            usage,
            mapped_at_creation: false,
        });
        let host_len = usize::try_from(len).map_err(|error| {
            BackendError::new(format!(
                "GpuBufferHandle::alloc received logical byte length {len} that does not fit usize on this host: {error}. Fix: shard the GPU buffer before allocating or run on a host with a wide enough address space."
            ))
        })?;
        Ok(Self::from_parts(
            Arc::new(buffer),
            len,
            allocation_len,
            host_len,
            usage,
            None,
        ))
    }

    /// Download this GPU buffer into `out`.
    ///
    /// This is intended for terminal output and test assertions, not hot-loop
    /// dispatch. The buffer must have `COPY_SRC` usage.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the handle is not copy-readable or the GPU
    /// mapping fails.
    pub fn readback(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        self.readback_until(device, None, queue, out, None)
    }

    /// Download the first `len` logical bytes of this GPU buffer into `out`.
    ///
    /// Hot paths that publish a device-side count should read back only the
    /// counted prefix instead of the whole capacity-sized buffer. The copy is
    /// rounded up to wgpu's 4-byte copy granularity internally, then truncated
    /// back to exactly `len` bytes before returning.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the handle is not copy-readable, `len`
    /// exceeds the logical buffer length, or the GPU mapping fails.
    pub fn readback_prefix(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        len: u64,
        out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        self.readback_prefix_until(device, None, queue, len, out, None)
    }

    /// Download `len` logical bytes starting at `byte_offset` into `out`.
    ///
    /// The internal GPU copy is alignment-expanded when necessary, then the
    /// returned host slice is trimmed back to exactly the requested range.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the handle is not copy-readable, the range
    /// exceeds the logical buffer length, or the GPU mapping fails.
    pub fn readback_range(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        byte_offset: u64,
        len: u64,
        out: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        self.readback_range_until(device, None, queue, byte_offset, len, out, None)
    }

    pub(crate) fn readback_until(
        &self,
        device: &wgpu::Device,
        pool: Option<&StagingBufferPool>,
        queue: &wgpu::Queue,
        out: &mut Vec<u8>,
        deadline: Option<Instant>,
    ) -> Result<(), BackendError> {
        self.readback_prefix_until(device, pool, queue, self.byte_len(), out, deadline)
    }

    pub(crate) fn readback_prefix_until(
        &self,
        device: &wgpu::Device,
        pool: Option<&StagingBufferPool>,
        queue: &wgpu::Queue,
        len: u64,
        out: &mut Vec<u8>,
        deadline: Option<Instant>,
    ) -> Result<(), BackendError> {
        self.readback_range_until(device, pool, queue, 0, len, out, deadline)
    }

    pub(crate) fn readback_range_until(
        &self,
        device: &wgpu::Device,
        pool: Option<&StagingBufferPool>,
        queue: &wgpu::Queue,
        byte_offset: u64,
        len: u64,
        out: &mut Vec<u8>,
        deadline: Option<Instant>,
    ) -> Result<(), BackendError> {
        if !self.usage().contains(wgpu::BufferUsages::COPY_SRC) {
            return Err(BackendError::new(
                "GpuBufferHandle readback requires COPY_SRC usage. Fix: allocate terminal-output buffers with COPY_SRC.",
            ));
        }
        let logical_end = byte_offset.checked_add(len).ok_or_else(|| {
            BackendError::new(format!(
                "GpuBufferHandle range readback overflows u64 at offset {byte_offset} len {len}. Fix: split the readback range before dispatch."
            ))
        })?;
        if logical_end > self.byte_len() {
            return Err(BackendError::new(format!(
                "GpuBufferHandle range readback requested bytes [{byte_offset}..{logical_end}) from a {} byte buffer. Fix: clamp the requested range to the device-published count.",
                self.byte_len()
            )));
        }
        if len == 0 {
            out.clear();
            return Ok(());
        }
        let copy_start = byte_offset & !3;
        let trim_start = byte_offset - copy_start;
        let visible_copy_len = trim_start.checked_add(len).ok_or_else(|| {
            BackendError::new(format!(
                "GpuBufferHandle range readback copy length overflows u64 at trim {trim_start} len {len}. Fix: split the readback range before dispatch."
            ))
        })?;
        let read_len = aligned_len_u64(visible_copy_len, "GPU readback visible copy length")?;
        let copy_end = copy_start.checked_add(read_len).ok_or_else(|| {
            BackendError::new(format!(
                "GpuBufferHandle range readback aligned copy overflows u64 at start {copy_start} len {read_len}. Fix: split the readback range before dispatch."
            ))
        })?;
        if copy_end > self.inner.allocation_len {
            return Err(BackendError::new(format!(
                "GpuBufferHandle range readback rounded bytes [{byte_offset}..{logical_end}) to aligned bytes [{copy_start}..{copy_end}), beyond allocation length {}. Fix: allocate buffers with 4-byte padding.",
                self.inner.allocation_len
            )));
        }
        let readback_usage = wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ;
        let readback = if let Some(pool) = pool {
            pool.acquire(device, read_len, readback_usage)
        } else {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vyre persistent handle readback"),
                size: read_len,
                usage: readback_usage,
                mapped_at_creation: false,
            })
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("vyre persistent handle readback encoder"),
        });
        encoder.copy_buffer_to_buffer(self.buffer(), copy_start, &readback, 0, read_len);
        let submission = queue.submit(std::iter::once(encoder.finish()));
        let slice = readback.slice(0..read_len);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            if let Err(error) = sender.send(result) {
                tracing::error!(
                    ?error,
                    "persistent buffer readback map_async result was lost because the receiver dropped"
                );
            }
        });
        let mapping = if let Some(deadline) = deadline {
            let mut backoff = crate::wait_backoff::AdaptiveWaitBackoff::from_micros(64, 2, 50, 5);
            loop {
                crate::runtime::device::poll_device_once(device)?;
                match receiver.try_recv() {
                    Ok(result) => break result,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return Err(BackendError::new(
                            "persistent buffer readback channel closed before completion. Fix: keep the GPU device alive until readback completes.",
                        ));
                    }
                }
                let now = Instant::now();
                if now >= deadline {
                    return Err(BackendError::new(
                        "dispatch cancelled after DispatchConfig.timeout before readback completed. Fix: raise DispatchConfig.timeout or split the program into smaller chunks.",
                    ));
                }
                backoff.idle_for(deadline.saturating_duration_since(now));
            }
        } else {
            crate::runtime::device::poll_device_wait_for(device, submission)?;
            receiver
                .recv_timeout(std::time::Duration::from_secs(30))
                .map_err(|source| {
                    BackendError::new(format!(
                        "persistent buffer readback callback did not complete after submission wait: {source}. Fix: keep the GPU device alive and inspect driver callback progress."
                    ))
                })?
        };
        let result = mapping.map_err(|source| {
            BackendError::new(format!(
                "persistent buffer readback mapping failed: {source:?}. Fix: use COPY_SRC handles and MAP_READ staging buffers."
            ))
        });
        result?;
        let mapped = slice.get_mapped_range();
        let visible_len = usize::try_from(len).map_err(|source| {
            BackendError::new(format!(
                "persistent buffer prefix length {len} cannot fit usize: {source}. Fix: split the buffer before readback.",
            ))
        })?;
        let trim_start = usize::try_from(trim_start).map_err(|source| {
            BackendError::new(format!(
                "persistent buffer range trim offset {trim_start} cannot fit usize: {source}. Fix: split the buffer before readback.",
            ))
        })?;
        let trim_end = trim_start.checked_add(visible_len).ok_or_else(|| {
            BackendError::new(format!(
                "persistent buffer range trim overflows usize at offset {trim_start} len {visible_len}. Fix: split the buffer before readback."
            ))
        })?;
        let visible = &mapped[trim_start..trim_end];
        if out.len() == visible_len {
            out.copy_from_slice(visible);
        } else {
            if visible_len > out.capacity() {
                let additional = visible_len - out.capacity();
                out.try_reserve_exact(additional).map_err(|source| {
                    BackendError::new(format!(
                        "persistent buffer readback could not reserve {visible_len} output bytes exactly: {source}. Fix: lower max_output_bytes or stream readback in smaller shards."
                    ))
                })?;
            }
            out.clear();
            out.extend_from_slice(visible);
        }
        drop(mapped);
        readback.unmap();
        if let Some(pool) = pool {
            pool.release(readback, read_len, readback_usage);
        }
        Ok(())
    }

    /// Stable process-local handle id used for cache signatures.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.inner.id
    }

    /// Stable process-local identity for the backing GPU allocation.
    ///
    /// Unlike [`Self::id`], this survives pool release/reacquire cycles for the
    /// same underlying `wgpu::Buffer`. Bind-group caches must key on this value
    /// plus the logical binding range; otherwise hot dispatches miss every time
    /// a pooled allocation is wrapped in a fresh handle.
    #[must_use]
    pub(crate) fn allocation_identity(&self) -> u64 {
        pointer_identity_key(Arc::as_ptr(&self.inner.buffer))
    }

    /// Resolve a process-local resident buffer id back into a live GPU handle.
    #[must_use]
    pub fn from_resident_id(id: u64) -> Option<Self> {
        let registry = resident_buffers();
        let entry = registry.get(&id)?;
        let upgraded = entry.value().upgrade();
        drop(entry);
        match upgraded {
            Some(inner) => Some(Self { inner }),
            None => {
                registry.remove(&id);
                None
            }
        }
    }

    /// Underlying `wgpu::Buffer`.
    #[must_use]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.inner.buffer
    }

    /// Clone the internal `Arc<wgpu::Buffer>`  -  cheap, reference-
    /// count only. Used by the indirect dispatch path (C-B4) which
    /// needs to stash the buffer alongside other args.
    #[must_use]
    pub fn buffer_arc(&self) -> Arc<wgpu::Buffer> {
        Arc::clone(&self.inner.buffer)
    }

    /// Logical byte length requested by the caller.
    #[must_use]
    pub fn byte_len(&self) -> u64 {
        self.inner.byte_len
    }

    /// Backing allocation length.
    #[must_use]
    pub fn allocation_len(&self) -> u64 {
        self.inner.allocation_len
    }

    /// Logical element count. Byte buffers report one element per byte.
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.inner.element_count
    }

    /// Actual usage flags on the underlying GPU allocation.
    #[must_use]
    pub fn usage(&self) -> wgpu::BufferUsages {
        self.inner.usage
    }

    pub(crate) fn from_parts(
        buffer: Arc<wgpu::Buffer>,
        byte_len: u64,
        allocation_len: u64,
        element_count: usize,
        usage: wgpu::BufferUsages,
        pool_return: Option<PoolReturn>,
    ) -> Self {
        let inner = Arc::new(GpuBufferInner {
            id: NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed),
            buffer,
            byte_len,
            allocation_len,
            element_count,
            usage,
            pool_return,
        });
        resident_buffers().insert(inner.id, Arc::downgrade(&inner));
        Self { inner }
    }
}

impl std::fmt::Debug for GpuBufferHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GpuBufferHandle")
            .field("id", &self.id())
            .field("byte_len", &self.byte_len())
            .field("allocation_len", &self.allocation_len())
            .field("element_count", &self.element_count())
            .field("usage", &self.usage())
            .finish()
    }
}

impl Drop for GpuBufferInner {
    fn drop(&mut self) {
        resident_buffers().remove(&self.id);
        if let Some(pool_return) = self.pool_return.take() {
            pool_return.release(
                Arc::clone(&self.buffer),
                self.byte_len,
                self.allocation_len,
                self.usage,
            );
        }
    }
}

pub(crate) fn aligned_len(len: usize) -> Result<u64, BackendError> {
    let padded = aligned_len_usize(len, "GPU buffer length")?;
    u64::try_from(padded).map_err(|source| {
        BackendError::new(format!(
            "GPU buffer length {padded} cannot fit u64: {source}. Fix: split the dispatch input."
        ))
    })
}

fn aligned_len_u64(len: u64, label: &'static str) -> Result<u64, BackendError> {
    crate::numeric::align_up_u64(len, 4, label)
}

fn aligned_len_usize(len: usize, label: &'static str) -> Result<usize, BackendError> {
    crate::numeric::align_up_usize(len, 4, label)
}

pub(crate) fn write_padded(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    bytes: &[u8],
    allocation_len: u64,
) -> Result<(), BackendError> {
    crate::padded_upload::write_padded_and_zero_fill(queue, buffer, bytes, allocation_len)
}

/// Default cap for the [`BindGroupCache`] LRU.
const BIND_GROUP_CACHE_CAP: usize = 256;

/// Inline storage for bind-group cache keys: typical shaders use few bindings;
/// `SmallVec` avoids a heap `Vec` on most `get_or_create` calls.
type BindGroupHandleKey = SmallVec<[u64; 16]>;

/// Bounded LRU cache for wgpu bind groups, keyed by layout identity and
/// the ordered set of buffer handles bound to that layout.
///
/// wgpu bind-group creation is non-trivial; this cache eliminates the
/// redundant cost on repeated dispatches that share the same buffer
/// handles.  Capped at 256 entries with LRU eviction to prevent
/// descriptor-heap exhaustion on long-running servers.
#[derive(Clone)]
pub struct BindGroupCache {
    cache: Arc<Mutex<BindGroupCacheInner>>,
    hits: Arc<AtomicUsize>,
    misses: Arc<AtomicUsize>,
    evictions: Arc<AtomicUsize>,
}

struct BindGroupCacheInner {
    entries: FxHashMap<BindGroupCacheKey, BindGroupCacheEntry>,
    lru: BinaryHeap<Reverse<BindGroupLruEntry>>,
    cap: usize,
    next_generation: u64,
}

struct BindGroupCacheEntry {
    bind_group: Arc<wgpu::BindGroup>,
    last_seen: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BindGroupLruEntry {
    last_seen: u64,
    key: BindGroupCacheKey,
}

impl Ord for BindGroupLruEntry {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        self.last_seen
            .cmp(&other.last_seen)
            .then_with(|| self.key.cmp(&other.key))
    }
}

impl PartialOrd for BindGroupLruEntry {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

fn push_bind_group_handle_key(key: &mut BindGroupHandleKey, handle: &GpuBufferHandle) -> bool {
    key.push(handle.allocation_identity());
    let Ok(aligned_len) = aligned_len_u64(handle.byte_len(), "bind-group handle key byte length")
    else {
        key.pop();
        return false;
    };
    key.push(aligned_len);
    true
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct BindGroupCacheKey {
    layout_id: usize,
    handles: BindGroupHandleKey,
}

impl std::fmt::Debug for BindGroupCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BindGroupCache")
            .field("hits", &self.hits.load(Ordering::Relaxed))
            .field("misses", &self.misses.load(Ordering::Relaxed))
            .field("evictions", &self.evictions.load(Ordering::Relaxed))
            .field("entries", &self.lock_cache().entries.len())
            .finish_non_exhaustive()
    }
}

impl Default for BindGroupCache {
    fn default() -> Self {
        Self::new()
    }
}

impl BindGroupCacheInner {
    fn next_lru_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        generation
    }

    fn touch_existing(&mut self, key: &BindGroupCacheKey) -> Option<Arc<wgpu::BindGroup>> {
        let generation = self.next_lru_generation();
        let bind_group = {
            let entry = self.entries.get_mut(key)?;
            entry.last_seen = generation;
            Arc::clone(&entry.bind_group)
        };
        self.lru.push(Reverse(BindGroupLruEntry {
            last_seen: generation,
            key: key.clone(),
        }));
        self.compact_lru_if_needed();
        Some(bind_group)
    }

    fn insert_entry(&mut self, key: BindGroupCacheKey, bind_group: Arc<wgpu::BindGroup>) {
        let generation = self.next_lru_generation();
        self.entries.insert(
            key.clone(),
            BindGroupCacheEntry {
                bind_group,
                last_seen: generation,
            },
        );
        self.lru.push(Reverse(BindGroupLruEntry {
            last_seen: generation,
            key,
        }));
        self.compact_lru_if_needed();
    }

    fn evict_to_cap(&mut self, mut on_evict: impl FnMut()) {
        while self.entries.len() > self.cap {
            let Some(key) = self.pop_lru_key() else { break };
            if self.entries.remove(&key).is_some() {
                on_evict();
            }
        }
    }

    fn pop_lru_key(&mut self) -> Option<BindGroupCacheKey> {
        while let Some(Reverse(entry)) = self.lru.pop() {
            if self
                .entries
                .get(&entry.key)
                .is_some_and(|current| current.last_seen == entry.last_seen)
            {
                return Some(entry.key);
            }
        }
        None
    }

    fn compact_lru_if_needed(&mut self) {
        let live = self.entries.len();
        if let Some(limit) = stale_lru_limit(live) {
            if self.lru.len() <= limit {
                return;
            }
        }
        self.lru.clear();
        self.lru.extend(self.entries.iter().map(|(key, entry)| {
            Reverse(BindGroupLruEntry {
                last_seen: entry.last_seen,
                key: key.clone(),
            })
        }));
    }
}

fn stale_lru_limit(live: usize) -> Option<usize> {
    live.checked_mul(4).map(|limit| limit.max(8))
}

impl BindGroupCache {
    fn lock_cache(&self) -> MutexGuard<'_, BindGroupCacheInner> {
        self.cache.lock().unwrap_or_else(|error| {
            tracing::error!(
                "Vyre WGPU bind-group cache lock was poisoned: {error}. Fix: discard the cache after a panic; continuing with recovered state."
            );
            error.into_inner()
        })
    }

    /// Create a bind-group cache with the default 256-entry cap.
    #[must_use]
    pub fn new() -> Self {
        Self::with_cap(BIND_GROUP_CACHE_CAP)
    }

    /// Create with an explicit cap (used by tests and consumers that
    /// want to size the LRU against known working-set bounds).
    #[must_use]
    pub fn with_cap(cap: usize) -> Self {
        Self {
            cache: Arc::new(Mutex::new(BindGroupCacheInner {
                entries: FxHashMap::default(),
                lru: BinaryHeap::new(),
                cap: cap.max(1),
                next_generation: 0,
            })),
            hits: Arc::new(AtomicUsize::new(0)),
            misses: Arc::new(AtomicUsize::new(0)),
            evictions: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Return a cached bind group or create one with `factory`.
    ///
    /// `layout_id` must uniquely identify the `wgpu::BindGroupLayout`
    /// (e.g. `Arc::as_ptr(layout).addr()`).
    /// `handles` must be in the same order as the `wgpu::BindGroupEntry`
    /// slice that the caller will pass to `create_bind_group` so that
    /// identical handle sets map to the same cache key.
    pub fn get_or_create(
        &self,
        layout_id: usize,
        handles: &[GpuBufferHandle],
        factory: impl FnOnce() -> wgpu::BindGroup,
    ) -> Arc<wgpu::BindGroup> {
        let Some(key_part_count) = handles.len().checked_mul(2) else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            return Arc::new(factory());
        };
        let mut key_parts = SmallVec::with_capacity(key_part_count);
        for handle in handles {
            if !push_bind_group_handle_key(&mut key_parts, handle) {
                self.misses.fetch_add(1, Ordering::Relaxed);
                return Arc::new(factory());
            }
        }
        self.get_or_create_by_ids(layout_id, key_parts, factory)
    }

    pub(crate) fn get_or_create_by_ids(
        &self,
        layout_id: usize,
        handles: SmallVec<[u64; 16]>,
        factory: impl FnOnce() -> wgpu::BindGroup,
    ) -> Arc<wgpu::BindGroup> {
        let key = BindGroupCacheKey { layout_id, handles };
        {
            let mut cache = self.lock_cache();
            if let Some(existing) = cache.touch_existing(&key) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return existing;
            }
        }
        let bg = Arc::new(factory());
        let mut cache = self.lock_cache();
        cache.insert_entry(key, Arc::clone(&bg));
        cache.evict_to_cap(|| {
            self.evictions.fetch_add(1, Ordering::Relaxed);
        });
        self.misses.fetch_add(1, Ordering::Relaxed);
        bg
    }

    pub(crate) fn get_by_ids(
        &self,
        layout_id: usize,
        handles: &[u64],
    ) -> Option<Arc<wgpu::BindGroup>> {
        let key = BindGroupCacheKey {
            layout_id,
            handles: SmallVec::from_slice(handles),
        };
        let mut cache = self.lock_cache();
        let existing = cache.touch_existing(&key)?;
        self.hits.fetch_add(1, Ordering::Relaxed);
        Some(existing)
    }

    pub(crate) fn insert_by_ids(
        &self,
        layout_id: usize,
        handles: &[u64],
        bind_group: wgpu::BindGroup,
    ) -> Arc<wgpu::BindGroup> {
        let key = BindGroupCacheKey {
            layout_id,
            handles: SmallVec::from_slice(handles),
        };
        let mut cache = self.lock_cache();
        if let Some(existing) = cache.touch_existing(&key) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return existing;
        }
        let bg = Arc::new(bind_group);
        cache.insert_entry(key, Arc::clone(&bg));
        cache.evict_to_cap(|| {
            self.evictions.fetch_add(1, Ordering::Relaxed);
        });
        self.misses.fetch_add(1, Ordering::Relaxed);
        bg
    }

    /// Return cache statistics for diagnostics and tests.
    #[must_use]
    pub fn stats(&self) -> BindGroupCacheStats {
        BindGroupCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            entries: self.lock_cache().entries.len(),
        }
    }
}

/// Bind-group cache statistics for a compiled wgpu pipeline.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BindGroupCacheStats {
    /// Number of cached bind-group hits.
    pub hits: usize,
    /// Number of bind-group creations caused by cache misses.
    pub misses: usize,
    /// Number of cached bind-group entries evicted to honor the cap.
    pub evictions: usize,
    /// Current number of entries held.
    pub entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// StagingBufferPool must reuse buffers across readback calls so that 100
    /// readbacks of the same size allocate only ~1 buffer.
    #[test]
    fn staging_pool_reuses_buffers_on_hot_readback_loop() {
        let arc = crate::runtime::cached_device()
            .expect("Fix: GPU device is required for staging pool test");
        let (device, queue) = &*arc;

        // Create a small COPY_SRC buffer with known contents.
        let contents: Vec<u8> = vec![0xAB; 64];
        let handle =
            GpuBufferHandle::upload(device, queue, &contents, wgpu::BufferUsages::COPY_SRC)
                .expect("Fix: upload should succeed");

        let pool = StagingBufferPool::new();

        for _ in 0..100 {
            let mut out = Vec::new();
            handle
                .readback_until(device, Some(&pool), queue, &mut out, None)
                .expect("Fix: pooled readback should succeed");
            assert_eq!(out, contents, "readback bytes must match uploaded bytes");
        }

        let stats = pool.stats();
        assert!(
            stats.allocations <= 2,
            "hot loop of 100 identical readbacks should allocate at most 2 staging buffers, got {} allocations and {} hits",
            stats.allocations,
            stats.hits
        );
    }

    /// Without a pool, readback must still work and always create fresh buffers.
    #[test]
    fn readback_without_pool_always_allocates() {
        let arc = crate::runtime::cached_device()
            .expect("Fix: GPU device is required for readback regression test");
        let (device, queue) = &*arc;

        let contents: Vec<u8> = vec![0xCD; 32];
        let handle =
            GpuBufferHandle::upload(device, queue, &contents, wgpu::BufferUsages::COPY_SRC)
                .expect("Fix: upload should succeed");

        for _ in 0..5 {
            let mut out = Vec::new();
            handle
                .readback(device, queue, &mut out)
                .expect("Fix: unpooled readback should succeed");
            assert_eq!(out, contents);
        }
    }

    /// The mapped-at-creation upload fast path (the StagingBelt replacement)
    /// must produce a buffer whose contents byte-for-byte equal the input across
    /// every boundary class: sub-word, exactly-a-word, word+tail, and a large
    /// payload (the catalog-scale path). A regression here is a silent data
    /// corruption on the ~1 GB DFA-catalog upload.
    #[test]
    fn mapped_upload_roundtrips_exact_bytes_across_boundaries() {
        let arc = crate::runtime::cached_device()
            .expect("Fix: live GPU device required for mapped upload roundtrip test");
        let (device, queue) = &*arc;
        // 1,3 exercise the 4-byte tail; 4 is exactly aligned; 5 is word+tail;
        // 257 is multi-word+tail; 1 MiB + 3 is the large, tail-padded path that
        // used to route through the slow per-write StagingBelt.
        for &len in &[1usize, 3, 4, 5, 257, (1 << 20) + 3] {
            let contents: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let handle = GpuBufferHandle::upload(
                device,
                queue,
                &contents,
                wgpu::BufferUsages::COPY_SRC,
            )
            .expect("Fix: mapped upload should succeed at every size");
            let mut out = Vec::new();
            handle
                .readback(device, queue, &mut out)
                .expect("Fix: readback should succeed");
            assert_eq!(
                out, contents,
                "mapped upload corrupted {len}-byte payload: readback != input"
            );
        }
    }

    /// `write_padded_into_mapped` (the fast-path filler) must copy the logical
    /// bytes and zero the alignment tail deterministically — proved without a
    /// GPU so the contract holds on every host.
    #[test]
    fn write_padded_into_mapped_zeroes_the_tail() {
        // Allocation of 8 bytes, 5 logical: bytes 0..5 copied, 5..8 zeroed even
        // if the destination started with garbage.
        let mut mapped = [0xAAu8; 8];
        let bytes = [1u8, 2, 3, 4, 5];
        crate::padded_upload::write_padded_into_mapped(&mut mapped, &bytes)
            .expect("Fix: filling a large-enough mapped slice must succeed");
        assert_eq!(&mapped[..5], &bytes);
        assert_eq!(&mapped[5..], &[0u8, 0, 0], "alignment tail must be zeroed");
        // A slice smaller than the data must fail closed, never truncate.
        let mut too_small = [0u8; 2];
        assert!(crate::padded_upload::write_padded_into_mapped(&mut too_small, &bytes).is_err());
    }

    #[test]
    fn resident_registry_handles_concurrent_lookup_and_drop() {
        let arc = crate::runtime::cached_device()
            .expect("Fix: GPU device is required for resident registry concurrency test");
        let (device, queue) = &*arc;
        let handle =
            GpuBufferHandle::upload(device, queue, &[1, 2, 3, 4], wgpu::BufferUsages::COPY_SRC)
                .expect("Fix: upload should register a resident buffer");
        let id = handle.id();

        // Phase 1: while the handle is alive, 8 concurrent readers
        // must always resolve the resident id. Join BEFORE the drop so
        // there is no readers-vs-drop race producing flaky panics.
        let readers = (0..8)
            .map(|_| {
                std::thread::spawn(move || {
                    for _ in 0..1_000 {
                        let resident = GpuBufferHandle::from_resident_id(id)
                            .expect("Fix: resident id must resolve while handle is alive");
                        assert_eq!(resident.id(), id);
                    }
                })
            })
            .collect::<Vec<_>>();
        for reader in readers {
            reader
                .join()
                .expect("Fix: concurrent resident lookups must not panic");
        }

        // Phase 2: dropping the handle must remove the id from the
        // registry so subsequent lookups return None.
        drop(handle);
        assert!(
            GpuBufferHandle::from_resident_id(id).is_none(),
            "dropped handles must be removed from the resident registry"
        );
    }

    #[test]
    fn poisoned_staging_pool_lock_recovers_without_aborting_dispatch_path() {
        let pool = StagingBufferPool::new();
        let poisoned = pool.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoned.lock_inner();
            panic!("poison staging buffer pool");
        })
        .join();

        std::panic::catch_unwind(|| {
            let _ = pool.stats();
        })
        .expect("Fix: poisoned staging pool must recover so GPU readback pooling does not abort");
    }

    #[test]
    fn poisoned_bind_group_cache_lock_recovers_without_aborting_dispatch_path() {
        let cache = BindGroupCache::new();
        let poisoned = cache.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoned.lock_cache();
            panic!("poison bind group cache");
        })
        .join();

        std::panic::catch_unwind(|| {
            let _ = cache.stats();
        })
        .expect("Fix: poisoned bind-group cache must recover so GPU dispatch does not abort");
    }

    #[test]
    fn bind_group_cache_lru_heap_stays_capacity_scale() {
        let arc = crate::runtime::cached_device()
            .expect("Fix: GPU device is required for bind-group cache test");
        let (device, _) = &*arc;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vyre bind-group cache lru test layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(4),
                },
                count: None,
            }],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vyre bind-group cache lru test buffer"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vyre bind-group cache lru test bind group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let cache = BindGroupCache::with_cap(4);

        for i in 0..64u64 {
            cache.insert_by_ids(1, &[i, 4], bind_group.clone());
        }

        let inner = cache.lock_cache();
        assert_eq!(inner.entries.len(), 4);
        assert!(
            inner.lru.len() <= inner.entries.len().saturating_mul(4).max(8),
            "Fix: bind-group LRU heap must compact stale entries to cache-capacity scale"
        );
    }
}

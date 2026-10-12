use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// Deterministic content-addressed device pick.
///
/// Computes `blake3(key)` and maps the first 4 bytes (little-endian) onto
/// `[0, n_gpus)`. Callers use this as the initial landing device; overflow
/// handling lives in [`StreamShardAllocator`].
///
/// `n_gpus == 0` is a configuration bug; WGPU stream sharding is a
/// GPU-resident path and must not silently route work to a non-existent device.
///
/// # Errors
///
/// Returns [`StreamShardError::ZeroGpus`] when no GPU devices are available.
pub fn shard_by_blake3(key: &[u8], n_gpus: u32) -> Result<u32, StreamShardError> {
    if n_gpus == 0 {
        return Err(StreamShardError::ZeroGpus);
    }
    let hash = blake3::hash(key);
    let bytes = hash.as_bytes();
    let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
    Ok(u32::from_le_bytes(bytes) % n_gpus)
}

/// Stream-shard scheduling failure.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StreamShardError {
    /// No GPU devices were available to schedule onto.
    ZeroGpus,
    /// A GPU/device index did not fit host indexing.
    IndexTooLarge {
        /// Field being converted.
        label: &'static str,
    },
    /// A host index did not fit the public u32 device-index ABI.
    IndexTooWide {
        /// Field being converted.
        label: &'static str,
    },
    /// A supplied device id was outside the live GPU set.
    DeviceOutOfRange {
        /// Device id supplied by the caller.
        device: u32,
        /// Number of live GPUs.
        n_gpus: u32,
    },
    /// Accumulated per-GPU load overflowed.
    LoadOverflow,
    /// Stale heap compaction threshold overflowed host indexing.
    HeapLimitOverflow,
    /// Host scheduler state allocation failed before GPU stream assignment.
    AllocationFailed {
        /// Scheduler state being allocated.
        label: &'static str,
    },
}

impl std::fmt::Display for StreamShardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroGpus => write!(
                f,
                "stream-shard allocator received zero GPUs. Fix: probe adapters before scheduling and fail configuration if none are visible."
            ),
            Self::IndexTooLarge { label } => write!(
                f,
                "stream-shard {label} cannot fit host usize. Fix: reduce GPU count or shard the scheduler."
            ),
            Self::IndexTooWide { label } => write!(
                f,
                "stream-shard {label} cannot fit u32. Fix: reduce GPU count or shard the scheduler."
            ),
            Self::DeviceOutOfRange { device, n_gpus } => write!(
                f,
                "stream-shard device {device} is outside live GPU count {n_gpus}. Fix: only seed load for probed GPU ordinals."
            ),
            Self::LoadOverflow => write!(
                f,
                "stream-shard GPU load overflowed u64. Fix: shard the stream or lower per-item cost before scheduling."
            ),
            Self::HeapLimitOverflow => write!(
                f,
                "stream-shard heap compaction threshold overflowed usize. Fix: recreate the allocator before continuing."
            ),
            Self::AllocationFailed { label } => write!(
                f,
                "stream-shard {label} allocation failed. Fix: split the stream batch or lower host memory pressure before scheduling."
            ),
        }
    }
}

impl std::error::Error for StreamShardError {}

/// Streaming shard allocator.
///
/// Callers feed `(key, cost)` pairs; the allocator returns the target device
/// plus a running snapshot of per-device load. Initial landing is
/// [`shard_by_blake3`]. If the target device's running cost exceeds the
/// least-loaded device's cost by more than `spill_threshold`, the item spills
/// to the least-loaded device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShardHeapEntry {
    cost: u64,
    device: u32,
}

impl Ord for ShardHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cost
            .cmp(&other.cost)
            .then_with(|| self.device.cmp(&other.device))
    }
}

impl PartialOrd for ShardHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Streaming shard allocator.
///
/// Callers feed `(key, cost)` pairs; the allocator returns the target device
/// plus a running snapshot of per-device load. Initial landing is
/// [`shard_by_blake3`]. If the target device's running cost exceeds the
/// least-loaded device's cost by more than `spill_threshold`, the item spills
/// to the least-loaded device. Least-loaded selection is heap-backed and keeps
/// stale heap entries compacted to GPU-count scale.
pub struct StreamShardAllocator {
    per_device_cost: Vec<u64>,
    least_loaded: BinaryHeap<Reverse<ShardHeapEntry>>,
    n_gpus: u32,
    spill_threshold: u64,
}

impl StreamShardAllocator {
    /// Create an allocator for `n_gpus` devices with an initial zero-cost load
    /// vector.
    pub fn new(n_gpus: u32, spill_threshold: u64) -> Result<Self, StreamShardError> {
        if n_gpus == 0 {
            return Err(StreamShardError::ZeroGpus);
        }
        let gpus = n_gpus;
        let gpu_capacity = u32_to_usize(gpus, "GPU count")?;
        let mut least_loaded = BinaryHeap::new();
        vyre_foundation::allocation::try_reserve_binary_heap_to_capacity(
            &mut least_loaded,
            gpu_capacity,
        )
        .map_err(|_| StreamShardError::AllocationFailed {
            label: "least-loaded heap",
        })?;
        for device in 0..gpus {
            least_loaded.push(Reverse(ShardHeapEntry { cost: 0, device }));
        }
        let mut per_device_cost = Vec::new();
        vyre_driver::allocation::try_reserve_vec_to_capacity(&mut per_device_cost, gpu_capacity)
            .map_err(|_| StreamShardError::AllocationFailed {
                label: "per-device cost vector",
            })?;
        per_device_cost.resize(gpu_capacity, 0);
        Ok(Self {
            per_device_cost,
            least_loaded,
            n_gpus: gpus,
            spill_threshold,
        })
    }

    /// Inject pre-existing load, such as already-queued work.
    pub fn seed_load(&mut self, device: u32, cost: u64) -> Result<(), StreamShardError> {
        let device_index = u32_to_usize(device, "device index")?;
        let slot = self.per_device_cost.get_mut(device_index).ok_or(
            StreamShardError::DeviceOutOfRange {
                device,
                n_gpus: self.n_gpus,
            },
        )?;
        *slot = checked_load_add(*slot, cost)?;
        ensure_heap_spare(&mut self.least_loaded, 1, "least-loaded heap update")?;
        self.least_loaded.push(Reverse(ShardHeapEntry {
            cost: *slot,
            device,
        }));
        self.compact_heap_if_needed()?;
        Ok(())
    }

    /// Assign one item.
    ///
    /// Returns the chosen device index, or `None` when `cost` is zero.
    pub fn assign(&mut self, key: &[u8], cost: u64) -> Result<Option<u32>, StreamShardError> {
        if cost == 0 {
            return Ok(None);
        }
        let initial = u32_to_usize(shard_by_blake3(key, self.n_gpus)?, "initial device index")?;
        let initial_cost = self.per_device_cost[initial];

        let (least_idx, least_cost) =
            self.least_loaded_device()?
                .ok_or(StreamShardError::DeviceOutOfRange {
                    device: 0,
                    n_gpus: self.n_gpus,
                })?;
        let least_index = u32_to_usize(least_idx, "least-loaded device index")?;

        let target =
            if initial_cost > least_cost && initial_cost - least_cost > self.spill_threshold {
                least_index
            } else {
                initial
            };

        self.per_device_cost[target] = checked_load_add(self.per_device_cost[target], cost)?;
        ensure_heap_spare(&mut self.least_loaded, 1, "least-loaded heap update")?;
        self.least_loaded.push(Reverse(ShardHeapEntry {
            cost: self.per_device_cost[target],
            device: usize_to_u32(target, "target device index")?,
        }));
        self.compact_heap_if_needed()?;
        Ok(Some(usize_to_u32(target, "target device index")?))
    }

    /// Snapshot of per-device cost. Index = device id.
    #[must_use]
    pub fn load(&self) -> &[u64] {
        &self.per_device_cost
    }

    fn least_loaded_device(&mut self) -> Result<Option<(u32, u64)>, StreamShardError> {
        while let Some(Reverse(entry)) = self.least_loaded.peek().copied() {
            let current = self
                .per_device_cost
                .get(u32_to_usize(entry.device, "heap device index")?)
                .copied();
            let Some(current) = current else {
                self.least_loaded.pop();
                continue;
            };
            if current == entry.cost {
                return Ok(Some((entry.device, entry.cost)));
            }
            self.least_loaded.pop();
        }
        Ok(None)
    }

    fn compact_heap_if_needed(&mut self) -> Result<(), StreamShardError> {
        let live = self.per_device_cost.len();
        if self.least_loaded.len() <= stale_heap_limit(live)? {
            return Ok(());
        }
        self.least_loaded.clear();
        vyre_foundation::allocation::try_reserve_binary_heap_to_capacity(
            &mut self.least_loaded,
            live,
        )
        .map_err(|_| StreamShardError::AllocationFailed {
            label: "least-loaded heap compaction",
        })?;
        for (device, &cost) in self.per_device_cost.iter().enumerate() {
            self.least_loaded.push(Reverse(ShardHeapEntry {
                cost,
                device: usize_to_u32(device, "heap rebuild device index")?,
            }));
        }
        Ok(())
    }

    #[cfg(test)]
    fn heap_len_for_diagnostics(&self) -> usize {
        self.least_loaded.len()
    }
}

fn u32_to_usize(value: u32, label: &'static str) -> Result<usize, StreamShardError> {
    usize::try_from(value).map_err(|_| StreamShardError::IndexTooLarge { label })
}

fn usize_to_u32(value: usize, label: &'static str) -> Result<u32, StreamShardError> {
    u32::try_from(value).map_err(|_| StreamShardError::IndexTooWide { label })
}

fn checked_load_add(current: u64, cost: u64) -> Result<u64, StreamShardError> {
    current
        .checked_add(cost)
        .ok_or(StreamShardError::LoadOverflow)
}

fn stale_heap_limit(live: usize) -> Result<usize, StreamShardError> {
    Ok(live
        .checked_mul(4)
        .ok_or(StreamShardError::HeapLimitOverflow)?
        .max(8))
}

fn ensure_heap_spare(
    heap: &mut BinaryHeap<Reverse<ShardHeapEntry>>,
    additional: usize,
    label: &'static str,
) -> Result<(), StreamShardError> {
    let target_capacity = heap
        .len()
        .checked_add(additional)
        .ok_or(StreamShardError::HeapLimitOverflow)?;
    vyre_foundation::allocation::try_reserve_binary_heap_to_capacity(heap, target_capacity)
        .map_err(|_| StreamShardError::AllocationFailed { label })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_by_blake3_is_deterministic() {
        let key = b"src/foo.rs";
        let a = shard_by_blake3(key, 4).expect("Fix: non-zero GPU count should shard");
        let b = shard_by_blake3(key, 4).expect("Fix: non-zero GPU count should shard");
        assert_eq!(a, b);
        assert!(a < 4);
    }

    #[test]
    fn shard_by_blake3_spreads_across_devices() {
        let keys: Vec<Vec<u8>> = (0..128)
            .map(|i| format!("src/file_{i}.rs").into_bytes())
            .collect();
        let mut hits = [0u32; 4];
        for k in &keys {
            hits[shard_by_blake3(k, 4).expect("Fix: non-zero GPU count should shard") as usize] +=
                1;
        }
        for h in &hits {
            assert!(*h > 0, "blake3 sharding must hit every device: {hits:?}");
        }
    }

    #[test]
    fn shard_by_blake3_n_zero_returns_error_instead_of_faking_device_zero() {
        let error = shard_by_blake3(b"anything", 0)
            .expect_err("zero visible GPUs must be a configuration failure");
        let message = error.to_string();
        assert!(
            message.contains("zero GPUs") && message.contains("probe adapters"),
            "zero-GPU sharding failure must explain the configuration fix: {message}"
        );
    }

    #[test]
    fn stream_allocator_initial_placement_matches_hash() {
        let mut allocator =
            StreamShardAllocator::new(4, 100).expect("Fix: non-zero GPU count should construct");
        let key = b"cold/file.bin";
        let initial = shard_by_blake3(key, 4).expect("Fix: non-zero GPU count should shard");
        let assigned = allocator
            .assign(key, 10)
            .expect("Fix: stream sharding should not overflow")
            .expect("Fix: non-zero cost accepted; restore this invariant before continuing.");
        assert_eq!(assigned, initial);
        assert_eq!(allocator.load()[initial as usize], 10);
    }

    #[test]
    fn stream_allocator_rejects_zero_cost() {
        let mut allocator =
            StreamShardAllocator::new(2, 0).expect("Fix: non-zero GPU count should construct");
        assert!(allocator
            .assign(b"x", 0)
            .expect("Fix: zero-cost assignment should not overflow")
            .is_none());
    }

    #[test]
    fn stream_allocator_spills_when_imbalance_exceeds_threshold() {
        let mut allocator =
            StreamShardAllocator::new(2, 5).expect("Fix: non-zero GPU count should construct");
        let mut key = vec![0u8; 4];
        while shard_by_blake3(&key, 2).expect("Fix: non-zero GPU count should shard") != 0 {
            key[0] = key[0].wrapping_add(1);
        }
        allocator
            .seed_load(0, 100)
            .expect("Fix: seed load should fit");

        let target = allocator
            .assign(&key, 1)
            .expect("Fix: stream sharding should not overflow")
            .expect("Fix: assigned; restore this invariant before continuing.");
        assert_eq!(target, 1, "heavy initial must spill to least-loaded");
    }

    #[test]
    fn stream_allocator_stays_affine_under_threshold() {
        let mut allocator =
            StreamShardAllocator::new(2, 100).expect("Fix: non-zero GPU count should construct");
        let mut key = vec![0u8; 4];
        while shard_by_blake3(&key, 2).expect("Fix: non-zero GPU count should shard") != 0 {
            key[0] = key[0].wrapping_add(1);
        }
        allocator
            .seed_load(0, 50)
            .expect("Fix: seed load should fit");
        let target = allocator
            .assign(&key, 1)
            .expect("Fix: stream sharding should not overflow")
            .expect("Fix: assigned; restore this invariant before continuing.");
        assert_eq!(target, 0, "affinity wins when imbalance <= spill_threshold");
    }

    #[test]
    fn stream_allocator_load_monotone() {
        let mut allocator =
            StreamShardAllocator::new(3, 0).expect("Fix: non-zero GPU count should construct");
        for i in 0..30 {
            let key = format!("path{i}").into_bytes();
            allocator
                .assign(&key, 1)
                .expect("Fix: stream sharding should not overflow")
                .expect("Fix: assigned; restore this invariant before continuing.");
        }
        let total: u64 = allocator.load().iter().sum();
        assert_eq!(total, 30, "every assignment must bump total load by cost");
    }

    #[test]
    fn stream_allocator_heap_compacts_stale_updates_to_gpu_count_scale() {
        let mut allocator =
            StreamShardAllocator::new(4, 0).expect("Fix: non-zero GPU count should construct");
        for i in 0..128 {
            allocator
                .assign(format!("path{i}").as_bytes(), 1)
                .expect("Fix: stream sharding should not overflow")
                .expect("Fix: non-zero work must assign to a GPU");
        }
        let load_before = allocator.load().to_vec();

        allocator
            .assign(b"trigger-stale-pop", 1)
            .expect("Fix: stream sharding should not overflow")
            .expect("Fix: non-zero work must assign to a GPU");

        assert_eq!(
            allocator.load().iter().sum::<u64>(),
            load_before.iter().sum::<u64>() + 1,
            "Fix: heap-backed assignment must preserve exact load accounting"
        );
        assert!(
            allocator.heap_len_for_diagnostics() <= allocator.load().len() * 4,
            "Fix: stale heap entries must be compacted to GPU-count scale instead of stream-length scale"
        );
    }

    #[test]
    fn stream_shard_source_has_no_release_path_infallible_allocation() {
        let source = include_str!("stream_shard.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: stream-shard production source must precede tests");
        assert!(
            !production.contains("BinaryHeap::with_capacity")
                && !production.contains("vec![0u64; gpu_capacity]")
                && !production.contains(".reserve_exact("),
            "Fix: WGPU stream sharding must report allocation pressure instead of aborting on infallible scheduler allocation."
        );
        assert!(
            production.contains("try_reserve_binary_heap_to_capacity")
                && production.contains("try_reserve_vec_to_capacity")
                && production.contains("ensure_heap_spare")
                && production.contains("AllocationFailed"),
            "Fix: WGPU stream sharding must reserve scheduler state fallibly before GPU assignment."
        );
    }
}

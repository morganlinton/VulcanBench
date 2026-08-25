//! Persistent-thread engine + host-side work queue (G7).
//!
//! # What this is
//!
//! A single long-lived GPU dispatch owns a chunk of the device.
//! Host workers push `PersistentWorkItem`s into a device-visible ring buffer
//! via an atomic head counter; the device's persistent threads
//! poll a tail counter and pick up items. The host waits on
//! per-item completion markers to gather results.
//!
//! Eliminates the per-file kernel-launch cost (~5–20 µs on today's
//! drivers) so a stream of 10 000 × 1 KiB scan jobs pays launch
//! overhead once, not 10 000 times.
//!
//! # Scope of this file
//!
//! This module owns the **host-side ring buffer**  -  the atomic
//! head/tail pair, the lock-free claim protocol, and exhaustive
//! tests. The actual persistent GPU kernel that consumes the queue
//! lives behind the `persistent` cargo feature and talks to the owning
//! backend's native queue API. The host queue is proven correct in isolation
//! so device integration only worries about the kernel side.
//!
//! # Memory ordering
//!
//! - Producers `AcqRel` on the head CAS; writes to the slot
//!   before the CAS happen-before the head increment.
//! - Consumers `AcqRel` on the tail CAS; after observing the
//!   incremented head, they see the producer's slot writes.
//! - A `Release` fence on the producer after the slot write and
//!   an `Acquire` fence on the consumer before reading the slot
//!   guarantees visibility across the weakest memory models we
//!   need to support (x86, ARM, RISC-V GPU consumers).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Caller-controlled persistent-thread dispatch policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PersistentThreadMode {
    /// Use the persistent path when the backend advertises support.
    #[default]
    Auto,
    /// Require the persistent path; fail loudly if unavailable.
    Force,
    /// Never use the persistent path.
    Disable,
}

/// One scan-unit descriptor.
///
/// All fields are plain 32-bit numbers so the same struct lays out
/// identically on host and device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PersistentWorkItem {
    /// Byte offset into the persistent input buffer.
    pub input_offset: u32,
    /// Number of bytes in this scan unit.
    pub input_len: u32,
    /// Rule-set / fused-megakernel output-slot bank id.
    pub rule_set_id: u32,
    /// Caller-opaque correlation id  -  echoed into the per-item
    /// completion counter so the host can match results back to a
    /// scan job without a shadow map.
    pub correlation: u32,
}

/// Shared atomics between host producers and device consumers.
#[derive(Debug)]
pub struct RingAtomics {
    /// Monotonically increasing next-slot-to-claim by a producer.
    pub head: AtomicU64,
    /// Monotonically increasing next-slot-to-claim by a consumer.
    pub tail: AtomicU64,
    /// Per-slot publication sequence. A producer writes the slot payload first
    /// and then publishes `head + 1` here with `Release`; consumers wait for
    /// that exact sequence before reading the packed payload.
    pub ready: Vec<AtomicU64>,
    /// Per-slot completion marker (1 = done).
    pub done: Vec<AtomicU32>,
}

impl RingAtomics {
    fn try_new(ring_size: u32) -> Result<Self, String> {
        let capacity = persistent_ring_capacity(ring_size)?;
        let mut ready = Vec::new();
        crate::allocation::try_reserve_vec_to_capacity(&mut ready, capacity).map_err(|error| {
            format!("Fix: persistent ring could not reserve {capacity} ready marker(s): {error}.")
        })?;
        for slot in 0..ring_size {
            ready.push(AtomicU64::new(u64::from(slot)));
        }

        let mut done = Vec::new();
        crate::allocation::try_reserve_vec_to_capacity(&mut done, capacity).map_err(|error| {
            format!("Fix: persistent ring could not reserve {capacity} done marker(s): {error}.")
        })?;
        for _ in 0..ring_size {
            done.push(AtomicU32::new(0));
        }

        Ok(Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            ready,
            done,
        })
    }
}

#[derive(Debug)]
struct WorkSlot {
    lo: AtomicU64,
    hi: AtomicU64,
}

impl WorkSlot {
    fn new(item: PersistentWorkItem) -> Self {
        let (lo, hi) = pack_work_item(item);
        Self {
            lo: AtomicU64::new(lo),
            hi: AtomicU64::new(hi),
        }
    }

    fn store(&self, item: PersistentWorkItem) {
        let (lo, hi) = pack_work_item(item);
        self.lo.store(lo, Ordering::Relaxed);
        self.hi.store(hi, Ordering::Relaxed);
    }

    fn load(&self) -> PersistentWorkItem {
        unpack_work_item(
            self.lo.load(Ordering::Relaxed),
            self.hi.load(Ordering::Relaxed),
        )
    }
}

fn pack_work_item(item: PersistentWorkItem) -> (u64, u64) {
    (
        u64::from(item.input_offset) | (u64::from(item.input_len) << 32),
        u64::from(item.rule_set_id) | (u64::from(item.correlation) << 32),
    )
}

fn unpack_work_item(lo: u64, hi: u64) -> PersistentWorkItem {
    PersistentWorkItem {
        input_offset: lo as u32,
        input_len: (lo >> 32) as u32,
        rule_set_id: hi as u32,
        correlation: (hi >> 32) as u32,
    }
}

/// Persistent-engine handle. Owns the host-side view of the ring
/// buffer. The GPU kernel is a separate concern gated behind
/// the `persistent` cargo feature.
#[derive(Debug)]
pub struct PersistentEngine {
    slots: Vec<WorkSlot>,
    atomics: RingAtomics,
    ring_size: u32,
}

impl PersistentEngine {
    /// Construct an engine with a ring capacity of `ring_size`
    /// slots. Must be a nonzero power of two so
    /// `index = slot & (cap-1)` is correct.
    pub fn new(ring_size: u32) -> Self {
        let ring_size = ring_size
            .checked_next_power_of_two()
            .filter(|&size| size > 0)
            .unwrap_or(1);
        Self::with_valid_ring_size(ring_size)
    }

    /// Construct an engine only when the ring capacity already satisfies
    /// the persistent-ring indexing contract.
    pub fn try_new(ring_size: u32) -> Result<Self, String> {
        if ring_size.is_power_of_two() && ring_size > 0 {
            Self::try_with_valid_ring_size(ring_size)
        } else {
            Err(format!(
                "Fix: ring_size must be a nonzero power of two, got {ring_size}."
            ))
        }
    }

    fn with_valid_ring_size(ring_size: u32) -> Self {
        match Self::try_with_valid_ring_size(ring_size) {
            Ok(engine) => engine,
            Err(_) => Self::try_with_valid_ring_size(1).unwrap_or_else(|_| std::process::abort()),
        }
    }

    fn try_with_valid_ring_size(ring_size: u32) -> Result<Self, String> {
        let zero = PersistentWorkItem {
            input_offset: 0,
            input_len: 0,
            rule_set_id: 0,
            correlation: 0,
        };
        let capacity = persistent_ring_capacity(ring_size)?;
        let mut slots = Vec::new();
        crate::allocation::try_reserve_vec_to_capacity(&mut slots, capacity).map_err(|error| {
            format!("Fix: persistent ring could not reserve {capacity} work slot(s): {error}.")
        })?;
        for _ in 0..ring_size {
            slots.push(WorkSlot::new(zero));
        }

        Ok(Self {
            slots,
            atomics: RingAtomics::try_new(ring_size)?,
            ring_size,
        })
    }

    /// Capacity of the ring buffer.
    pub fn ring_size(&self) -> u32 {
        self.ring_size
    }

    /// Enqueue a PersistentWorkItem. Returns `Ok(slot_index)` on success, or
    /// `Err(QueueFull)` if the ring is full. Thread-safe under
    /// concurrent producers (lock-free CAS on `head`).
    pub fn enqueue(&self, item: PersistentWorkItem) -> Result<u32, QueueFull> {
        loop {
            let head = self.atomics.head.load(Ordering::Acquire);
            let slot_idx = (head as u32) & (self.ring_size - 1);
            let slot_offset = slot_idx as usize;
            let Some(ready) = self.atomics.ready.get(slot_offset) else {
                return Err(QueueFull);
            };
            match ring_sequence_order(ready.load(Ordering::Acquire), head) {
                RingSequenceOrder::Free => {}
                RingSequenceOrder::Behind => return Err(QueueFull),
                RingSequenceOrder::Ahead => {
                    std::hint::spin_loop();
                    continue;
                }
            }
            match self.atomics.head.compare_exchange(
                head,
                head.wrapping_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let Some(slot) = self.slots.get(slot_offset) else {
                        return Err(QueueFull);
                    };
                    slot.store(item);
                    self.atomics.done[slot_offset].store(0, Ordering::Release);
                    self.atomics.ready[slot_offset].store(head.wrapping_add(1), Ordering::Release);
                    return Ok(slot_idx);
                }
                Err(_) => continue,
            }
        }
    }

    /// Consumer-side claim. Returns the next available item or
    /// `None` if the queue is empty. Thread-safe under concurrent
    /// consumers.
    pub fn claim(&self) -> Option<PersistentWorkItem> {
        loop {
            let tail = self.atomics.tail.load(Ordering::Acquire);
            let slot_idx = (tail as u32) & (self.ring_size - 1);
            let slot_offset = slot_idx as usize;
            let published = tail.wrapping_add(1);
            let Some(ready) = self.atomics.ready.get(slot_offset) else {
                return None;
            };
            match ring_sequence_order(ready.load(Ordering::Acquire), published) {
                RingSequenceOrder::Free => {}
                RingSequenceOrder::Behind => {
                    if tail >= self.atomics.head.load(Ordering::Acquire) {
                        return None;
                    }
                    std::hint::spin_loop();
                    continue;
                }
                RingSequenceOrder::Ahead => {
                    std::hint::spin_loop();
                    continue;
                }
            }
            match self.atomics.tail.compare_exchange(
                tail,
                tail.wrapping_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let slot = self.slots.get(slot_offset)?;
                    let item = slot.load();
                    self.atomics.ready[slot_offset].store(
                        tail.wrapping_add(u64::from(self.ring_size)),
                        Ordering::Release,
                    );
                    return Some(item);
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark item at `slot_idx` as done.
    pub fn mark_done(&self, slot_idx: u32) -> Result<(), String> {
        let Some(done) = self.atomics.done.get(slot_idx as usize) else {
            return Err(format!(
                "Fix: persistent ring slot_idx={slot_idx} is outside ring_size={}. Reject stale or corrupt completion markers before marking done.",
                self.ring_size
            ));
        };
        done.store(1, Ordering::Release);
        Ok(())
    }

    /// Whether the consumer finished the item at `slot_idx`.
    pub fn is_done(&self, slot_idx: u32) -> Result<bool, String> {
        let Some(done) = self.atomics.done.get(slot_idx as usize) else {
            return Err(format!(
                "Fix: persistent ring slot_idx={slot_idx} is outside ring_size={}. Reject stale or corrupt completion markers before reading done state.",
                self.ring_size
            ));
        };
        Ok(done.load(Ordering::Acquire) != 0)
    }

    /// Number of items queued and pending claim.
    pub fn try_in_flight(&self) -> Result<u32, String> {
        let pending = self
            .atomics
            .head
            .load(Ordering::Acquire)
            .wrapping_sub(self.atomics.tail.load(Ordering::Acquire));
        u32::try_from(pending).map_err(|_| {
            format!(
                "Fix: persistent engine in-flight count {pending} exceeds u32::MAX. Drain the ring or use the 64-bit counters before exporting GPU-visible queue metadata."
            )
        })
    }

    /// Number of items queued and pending claim.
    pub fn in_flight(&self) -> u32 {
        self.try_in_flight().unwrap_or(u32::MAX)
    }

    /// Monotonic head counter (modulo `ring_size` = slot index).
    pub fn head_counter(&self) -> u64 {
        self.atomics.head.load(Ordering::Acquire)
    }

    /// Monotonic head counter exposed through the legacy u32 API.
    pub fn head(&self) -> u32 {
        let head = self.head_counter();
        u32::try_from(head).unwrap_or(u32::MAX)
    }

    /// Monotonic tail counter.
    pub fn tail_counter(&self) -> u64 {
        self.atomics.tail.load(Ordering::Acquire)
    }

    /// Monotonic tail counter exposed through the legacy u32 API.
    pub fn tail(&self) -> u32 {
        let tail = self.tail_counter();
        u32::try_from(tail).unwrap_or(u32::MAX)
    }
}

fn persistent_ring_capacity(ring_size: u32) -> Result<usize, String> {
    usize::try_from(ring_size).map_err(|_| {
        format!("Fix: persistent ring_size {ring_size} does not fit this target's address space.")
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RingSequenceOrder {
    Behind,
    Free,
    Ahead,
}

fn ring_sequence_order(sequence: u64, position: u64) -> RingSequenceOrder {
    match (sequence.wrapping_sub(position) as i64).cmp(&0) {
        std::cmp::Ordering::Less => RingSequenceOrder::Behind,
        std::cmp::Ordering::Equal => RingSequenceOrder::Free,
        std::cmp::Ordering::Greater => RingSequenceOrder::Ahead,
    }
}

/// Enqueue attempted but the ring is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueFull;

impl std::fmt::Display for QueueFull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("persistent engine ring buffer is full")
    }
}

impl std::error::Error for QueueFull {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn item(i: u32) -> PersistentWorkItem {
        PersistentWorkItem {
            input_offset: i * 1024,
            input_len: 1024,
            rule_set_id: 0,
            correlation: i,
        }
    }

    #[test]
    fn invalid_ring_size_has_explicit_error_api() {
        let err = PersistentEngine::try_new(7).unwrap_err();
        assert!(err.contains("Fix:"));
        assert!(PersistentEngine::try_new(0).is_err());
    }

    #[test]
    fn infallible_constructor_normalizes_ring_size() {
        assert_eq!(PersistentEngine::new(7).ring_size(), 8);
        assert_eq!(PersistentEngine::new(0).ring_size(), 1);
    }

    #[test]
    fn enqueue_claim_fifo_single_thread() {
        let eng = PersistentEngine::new(8);
        for i in 0..8 {
            assert_eq!(eng.enqueue(item(i)).unwrap(), i);
        }
        for i in 0..8 {
            assert_eq!(eng.claim().unwrap().correlation, i);
        }
        assert!(eng.claim().is_none());
    }

    #[test]
    fn queue_full_on_overflow() {
        let eng = PersistentEngine::new(4);
        for i in 0..4 {
            eng.enqueue(item(i)).unwrap();
        }
        assert_eq!(eng.enqueue(item(99)), Err(QueueFull));
    }

    #[test]
    fn space_reclaims_after_claim() {
        let eng = PersistentEngine::new(4);
        for i in 0..4 {
            eng.enqueue(item(i)).unwrap();
        }
        assert!(eng.enqueue(item(99)).is_err());
        let claimed = eng.claim().unwrap();
        assert_eq!(claimed.correlation, 0);
        assert!(eng.enqueue(item(99)).is_ok());
    }

    #[test]
    fn in_flight_tracks_correctly() {
        let eng = PersistentEngine::new(16);
        assert_eq!(eng.in_flight(), 0);
        for i in 0..5 {
            eng.enqueue(item(i)).unwrap();
        }
        assert_eq!(eng.in_flight(), 5);
        eng.claim().unwrap();
        eng.claim().unwrap();
        assert_eq!(eng.in_flight(), 3);
    }

    #[test]
    fn done_marker_flows_through() {
        let eng = PersistentEngine::new(4);
        let slot = eng.enqueue(item(1)).unwrap();
        assert!(!eng.is_done(slot).unwrap());
        let claimed = eng.claim().unwrap();
        assert_eq!(claimed.correlation, 1);
        eng.mark_done(slot).unwrap();
        assert!(eng.is_done(slot).unwrap());
    }

    #[test]
    fn multi_producer_single_consumer_no_item_lost() {
        let eng = Arc::new(PersistentEngine::new(128));
        let producers = 4;
        let items_per_producer = 16;
        let mut handles = Vec::new();
        for p in 0..producers {
            let eng = Arc::clone(&eng);
            handles.push(thread::spawn(move || {
                for i in 0..items_per_producer {
                    let corr = (p * 1000 + i) as u32;
                    loop {
                        if eng.enqueue(item(corr)).is_ok() {
                            break;
                        }
                        std::hint::spin_loop();
                    }
                }
            }));
        }
        let consumer_eng = Arc::clone(&eng);
        let consumer = thread::spawn(move || {
            let total = (producers * items_per_producer) as usize;
            let mut seen = Vec::with_capacity(total);
            while seen.len() < total {
                if let Some(it) = consumer_eng.claim() {
                    seen.push(it.correlation);
                } else {
                    std::hint::spin_loop();
                }
            }
            seen
        });
        for h in handles {
            h.join().unwrap();
        }
        let seen = consumer.join().unwrap();
        let mut sorted = seen.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), seen.len(), "duplicate items consumed");
        for p in 0..producers {
            for i in 0..items_per_producer {
                let expected = (p * 1000 + i) as u32;
                assert!(
                    seen.contains(&expected),
                    "missing correlation id {expected}"
                );
            }
        }
    }

    #[test]
    fn wrap_around_works_for_large_throughput() {
        let eng = PersistentEngine::new(16);
        let passes = 10;
        for p in 0..passes {
            for i in 0..16 {
                let corr = (p * 1000 + i) as u32;
                assert!(eng.enqueue(item(corr)).is_ok());
            }
            for i in 0..16 {
                let corr = (p * 1000 + i) as u32;
                assert_eq!(eng.claim().unwrap().correlation, corr);
            }
        }
        assert_eq!(eng.head(), (passes * 16) as u32);
        assert_eq!(eng.tail(), (passes * 16) as u32);
        assert_eq!(eng.in_flight(), 0);
    }

    #[test]
    fn multi_consumer_no_double_claim() {
        let eng = Arc::new(PersistentEngine::new(128));
        let total = 100_u32;
        for i in 0..total {
            eng.enqueue(item(i)).unwrap();
        }
        let consumers = 4;
        let mut handles = Vec::new();
        let shared_consumed = Arc::new(std::sync::Mutex::new(Vec::new()));
        for _ in 0..consumers {
            let eng = Arc::clone(&eng);
            let out = Arc::clone(&shared_consumed);
            handles.push(thread::spawn(move || {
                let mut local = Vec::new();
                while let Some(it) = eng.claim() {
                    local.push(it.correlation);
                }
                out.lock().unwrap().extend(local);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut consumed = Arc::try_unwrap(shared_consumed)
            .unwrap()
            .into_inner()
            .unwrap();
        consumed.sort();
        assert_eq!(consumed.len(), total as usize);
        for (i, c) in consumed.iter().enumerate() {
            assert_eq!(*c, i as u32, "duplicated or missing item at idx {i}");
        }
    }

    #[test]
    fn queue_full_error_display_is_useful() {
        let s = format!("{QueueFull}");
        assert!(s.contains("ring buffer"));
    }
}

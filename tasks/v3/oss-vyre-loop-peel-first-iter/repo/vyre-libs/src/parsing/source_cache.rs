//! ROADMAP L2 / E2  -  content-hash LRU cache for parsed source.
//!
//! Substrate that any language's parse pipeline can opt into without
//! plumbing a cache through every layer of the parser. The cache is
//! keyed by the BLAKE3 content hash of the source bytes (or any
//! caller-chosen extra-key extension), so two callers with the same
//! source share the parsed artifact even if they hold distinct
//! string allocations.
//!
//! ## Why content hash, not string identity
//!
//! In the downstream analyzer scan loop the same `.h` header is included from
//! many translation units. Identity-keyed memoisation misses every
//! caller because each caller holds its own `String`. Content-hash
//! lookup lets every translation unit share a single parse.
//!
//! ## Why LRU, not unbounded
//!
//! Workspace scans touch tens of thousands of distinct files. An
//! unbounded cache grows without bound; an LRU bounded by entry
//! count keeps the working set in memory and evicts cold entries
//! deterministically. Hits refresh recency in O(1); eviction scans
//! only the bounded live set on insert.
//!
//! ## Thread safety
//!
//! The cache is `Send + Sync`  -  backed by a `Mutex<...>` so the
//! parse work proceeds outside the global cache lock and only the lookup /
//! insert / eviction touches it. Concurrent callers asking for the same key
//! coalesce through a per-key in-flight slot, so the expensive parse closure
//! runs once and every waiter receives the same `Arc<T>`.

use blake3::Hasher;
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// 32-byte BLAKE3 content hash used as the cache key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceHash(pub [u8; 32]);

impl SourceHash {
    /// Hash `source` plus the optional `extra` discriminator. The
    /// `extra` channel lets callers separate caches that share source
    /// bytes but differ in build flags (e.g. preprocessor `-D` set).
    #[must_use]
    pub fn of(source: &[u8], extra: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(source);
        hasher.update(&[0u8; 1]);
        hasher.update(extra);
        let mut out = [0u8; 32];
        out.copy_from_slice(hasher.finalize().as_bytes());
        Self(out)
    }
}

/// Convert source byte length to the non-zero `u32` extent used by generated
/// parsing Programs.
///
/// Empty inputs still need one logical lane so generated Programs keep their
/// buffer declarations valid. Inputs larger than `u32::MAX` saturate at the
/// maximum Program-visible extent instead of panicking during cache population.
#[must_use]
pub fn source_len_u32_nonzero(source: &[u8]) -> u32 {
    u32::try_from(source.len()).unwrap_or(u32::MAX).max(1)
}

/// Bounded LRU cache mapping `SourceHash` to `Arc<T>`. Eviction is
/// LRU by last-touched order. Hits are O(1); eviction scans the bounded live
/// set only when an insert needs space.
pub struct ParsedSourceLru<T> {
    inner: Mutex<LruInner<T>>,
}

struct LruInner<T> {
    capacity: usize,
    entries: HashMap<SourceHash, Arc<T>>,
    recency: HashMap<SourceHash, u64>,
    coldest: BinaryHeap<Reverse<RecencyEntry>>,
    in_flight: HashMap<SourceHash, Arc<InFlight<T>>>,
    clock: u64,
}

struct InFlight<T> {
    state: Mutex<InFlightState<T>>,
    ready: Condvar,
}

struct InFlightState<T> {
    result: Option<Arc<T>>,
    panicked: bool,
}

enum CacheMissAction<T> {
    Hit(Arc<T>),
    Parse(Arc<InFlight<T>>),
    Wait(Arc<InFlight<T>>),
}

#[cfg(test)]
mod generated_source_extent_tests {
    use super::source_len_u32_nonzero;

    #[test]
    fn source_len_u32_nonzero_pins_empty_small_and_boundary_inputs() {
        assert_eq!(source_len_u32_nonzero(b""), 1);
        assert_eq!(source_len_u32_nonzero(b"x"), 1);
        assert_eq!(source_len_u32_nonzero(b"abcdef"), 6);

        for len in 0usize..4096 {
            let bytes = vec![0u8; len];
            assert_eq!(
                source_len_u32_nonzero(&bytes),
                u32::try_from(len).unwrap_or(u32::MAX).max(1),
                "generated source length case {len} must match the shared parser extent contract"
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecencyEntry {
    tick: u64,
    key: SourceHash,
}

impl Ord for RecencyEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tick
            .cmp(&other.tick)
            .then_with(|| self.key.cmp(&other.key))
    }
}

impl PartialOrd for RecencyEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> ParsedSourceLru<T> {
    /// Build an empty cache that holds at most `capacity` entries.
    /// `capacity == 0` disables caching entirely (every lookup is a
    /// miss and nothing is stored).
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(LruInner {
                capacity,
                entries: HashMap::with_capacity(capacity),
                recency: HashMap::with_capacity(capacity),
                coldest: BinaryHeap::with_capacity(capacity),
                in_flight: HashMap::new(),
                clock: 0,
            }),
        }
    }

    /// Look up `key`. Returns the cached `Arc<T>` on hit (and bumps
    /// recency); returns `None` on miss.
    #[must_use]
    pub fn get(&self, key: SourceHash) -> Option<Arc<T>> {
        let mut inner = self.lock_inner();
        let value = inner.entries.get(&key)?.clone();
        bump_recency(&mut inner, key);
        Some(value)
    }

    /// Insert `value` for `key`, evicting the oldest entry if the
    /// cache is at capacity. Returns the inserted `Arc<T>`.
    pub fn insert(&self, key: SourceHash, value: T) -> Arc<T> {
        let arc = Arc::new(value);
        let mut inner = self.lock_inner();
        if inner.capacity == 0 {
            return arc;
        }
        if !inner.entries.contains_key(&key) && inner.entries.len() >= inner.capacity {
            if let Some(evicted) = pop_coldest_key(&mut inner) {
                inner.entries.remove(&evicted);
                inner.recency.remove(&evicted);
            }
        }
        inner.entries.insert(key, arc.clone());
        bump_recency(&mut inner, key);
        arc
    }

    /// Look up `key`; on miss, run `parse(source)` to produce the
    /// value and insert it. Returns the cached or freshly inserted
    /// `Arc<T>`.
    pub fn get_or_parse<F>(&self, source: &[u8], extra: &[u8], parse: F) -> Arc<T>
    where
        F: FnOnce(&[u8]) -> T,
    {
        let key = SourceHash::of(source, extra);
        let mut parse = Some(parse);
        loop {
            match self.miss_action(key) {
                CacheMissAction::Hit(hit) => return hit,
                CacheMissAction::Wait(in_flight) => {
                    if let Some(result) = wait_for_in_flight(&in_flight) {
                        return result;
                    }
                }
                CacheMissAction::Parse(in_flight) => {
                    let Some(parse) = parse.take() else {
                        if let Some(result) = wait_for_in_flight(&in_flight) {
                            return result;
                        }
                        continue;
                    };
                    let parsed =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse(source)));
                    match parsed {
                        Ok(value) => return self.finish_parse(key, in_flight, value),
                        Err(payload) => {
                            self.finish_panicked_parse(key, &in_flight);
                            std::panic::resume_unwind(payload);
                        }
                    }
                }
            }
        }
    }

    /// Total number of entries currently held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock_inner().entries.len()
    }

    /// `true` iff the cache holds zero entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock_inner(&self) -> MutexGuard<'_, LruInner<T>> {
        // Fail closed on a poisoned lock instead of silently recovering with
        // `into_inner()` (Law 10). A poison here means another thread panicked
        // mid-mutation of the LRU maps (entries/recency/coldest can be left
        // inconsistent), so handing that half-updated state back as if nothing
        // happened would silently corrupt every subsequent get/insert. Propagate
        // the panic loudly — same poison policy as the readiness mutex below.
        // (The in-flight-parse state lock is a SEPARATE, deliberate protocol:
        // a panicking parse is an expected, flagged condition there.)
        self.inner
            .lock()
            .expect("parsed-source LRU cache lock was poisoned")
    }

    #[cfg(test)]
    fn coldest_heap_len_for_diagnostics(&self) -> usize {
        self.lock_inner().coldest.len()
    }

    fn miss_action(&self, key: SourceHash) -> CacheMissAction<T> {
        let mut inner = self.lock_inner();
        if let Some(value) = inner.entries.get(&key).cloned() {
            bump_recency(&mut inner, key);
            return CacheMissAction::Hit(value);
        }
        if let Some(in_flight) = inner.in_flight.get(&key) {
            return CacheMissAction::Wait(Arc::clone(in_flight));
        }
        let in_flight = Arc::new(InFlight {
            state: Mutex::new(InFlightState {
                result: None,
                panicked: false,
            }),
            ready: Condvar::new(),
        });
        inner.in_flight.insert(key, Arc::clone(&in_flight));
        CacheMissAction::Parse(in_flight)
    }

    fn finish_parse(&self, key: SourceHash, in_flight: Arc<InFlight<T>>, value: T) -> Arc<T> {
        let arc = Arc::new(value);
        {
            let mut inner = self.lock_inner();
            if inner.capacity != 0 {
                if !inner.entries.contains_key(&key) && inner.entries.len() >= inner.capacity {
                    if let Some(evicted) = pop_coldest_key(&mut inner) {
                        inner.entries.remove(&evicted);
                        inner.recency.remove(&evicted);
                    }
                }
                inner.entries.insert(key, Arc::clone(&arc));
                bump_recency(&mut inner, key);
            }
            inner.in_flight.remove(&key);
        }
        let mut state = lock_in_flight_state(&in_flight);
        state.result = Some(Arc::clone(&arc));
        in_flight.ready.notify_all();
        arc
    }

    fn finish_panicked_parse(&self, key: SourceHash, in_flight: &InFlight<T>) {
        self.lock_inner().in_flight.remove(&key);
        let mut state = lock_in_flight_state(in_flight);
        state.panicked = true;
        in_flight.ready.notify_all();
    }
}

fn wait_for_in_flight<T>(in_flight: &InFlight<T>) -> Option<Arc<T>> {
    let mut state = lock_in_flight_state(in_flight);
    loop {
        if let Some(result) = &state.result {
            return Some(Arc::clone(result));
        }
        if state.panicked {
            return None;
        }
        state = in_flight
            .ready
            .wait(state)
            .unwrap_or_else(|error| error.into_inner());
    }
}

fn lock_in_flight_state<T>(in_flight: &InFlight<T>) -> MutexGuard<'_, InFlightState<T>> {
    in_flight
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn bump_recency<T>(inner: &mut LruInner<T>, key: SourceHash) {
    inner.clock = inner.clock.saturating_add(1);
    let tick = inner.clock;
    inner.recency.insert(key, tick);
    inner.coldest.push(Reverse(RecencyEntry { tick, key }));
    compact_coldest_heap_if_needed(inner);
}

fn pop_coldest_key<T>(inner: &mut LruInner<T>) -> Option<SourceHash> {
    while let Some(Reverse(entry)) = inner.coldest.pop() {
        if inner.entries.contains_key(&entry.key)
            && inner.recency.get(&entry.key).copied() == Some(entry.tick)
        {
            return Some(entry.key);
        }
    }
    None
}

fn compact_coldest_heap_if_needed<T>(inner: &mut LruInner<T>) {
    let live = inner.entries.len();
    if inner.coldest.len() <= live.saturating_mul(4).max(8) {
        return;
    }
    inner.coldest.clear();
    inner.coldest.reserve(live);
    inner.coldest.extend(
        inner
            .recency
            .iter()
            .map(|(&key, &tick)| Reverse(RecencyEntry { tick, key })),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;

    /// Same content + same extra hash to the same key.
    #[test]
    fn source_hash_equal_for_equal_inputs() {
        let a = SourceHash::of(b"int x = 1;", b"");
        let b = SourceHash::of(b"int x = 1;", b"");
        assert_eq!(a, b);
    }

    /// Distinct content hashes to distinct keys.
    #[test]
    fn source_hash_differs_for_different_source() {
        let a = SourceHash::of(b"int x = 1;", b"");
        let b = SourceHash::of(b"int x = 2;", b"");
        assert_ne!(a, b);
    }

    /// Distinct extras hash to distinct keys even with the same source.
    #[test]
    fn source_hash_differs_for_different_extra() {
        let a = SourceHash::of(b"int x = 1;", b"-DA");
        let b = SourceHash::of(b"int x = 1;", b"-DB");
        assert_ne!(a, b);
    }

    /// `get_or_parse` is invoked once per content-hash, even when the
    /// source bytes come from distinct caller `Vec` allocations.
    #[test]
    fn get_or_parse_dedups_across_callers() {
        let cache: ParsedSourceLru<usize> = ParsedSourceLru::with_capacity(4);
        let parse_calls = AtomicUsize::new(0);
        let parse = || {
            parse_calls.fetch_add(1, Ordering::SeqCst);
            42usize
        };
        let src_a = b"hello world".to_vec();
        let src_b = b"hello world".to_vec();
        let a = cache.get_or_parse(&src_a, b"", |_s| parse());
        let b = cache.get_or_parse(&src_b, b"", |_s| parse());
        assert_eq!(*a, 42);
        assert_eq!(*b, 42);
        assert_eq!(parse_calls.load(Ordering::SeqCst), 1);
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn concurrent_get_or_parse_coalesces_in_flight_parse() {
        let cache = Arc::new(ParsedSourceLru::<usize>::with_capacity(4));
        let source = Arc::new(b"same translation unit".to_vec());
        let workers = 8usize;
        let barrier = Arc::new(Barrier::new(workers));
        let parse_calls = Arc::new(AtomicUsize::new(0));
        let ready_to_parse = Arc::new((Mutex::new(0usize), std::sync::Condvar::new()));
        let mut handles = Vec::with_capacity(workers);

        for _ in 0..workers {
            let cache = Arc::clone(&cache);
            let source = Arc::clone(&source);
            let barrier = Arc::clone(&barrier);
            let parse_calls = Arc::clone(&parse_calls);
            let ready_to_parse = Arc::clone(&ready_to_parse);
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                {
                    let (lock, wake) = ready_to_parse.as_ref();
                    let mut ready = lock
                        .lock()
                        .expect("Fix: source-cache readiness mutex must not be poisoned");
                    *ready += 1;
                    wake.notify_all();
                }
                cache.get_or_parse(source.as_slice(), b"", |_| {
                    parse_calls.fetch_add(1, Ordering::SeqCst);
                    let (lock, wake) = ready_to_parse.as_ref();
                    let mut ready = lock
                        .lock()
                        .expect("Fix: source-cache readiness mutex must not be poisoned");
                    while *ready < workers {
                        ready = wake
                            .wait(ready)
                            .expect("Fix: source-cache readiness condvar must not be poisoned");
                    }
                    99usize
                })
            }));
        }

        let results = handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .expect("Fix: source-cache worker must not panic")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            parse_calls.load(Ordering::SeqCst),
            1,
            "Fix: concurrent same-key parse requests must share one in-flight parse"
        );
        for result in &results {
            assert_eq!(**result, 99);
            assert!(
                Arc::ptr_eq(result, &results[0]),
                "Fix: all waiters must receive the same cached Arc"
            );
        }
    }

    /// LRU eviction kicks the least-recently-used entry when capacity
    /// is reached.
    #[test]
    fn lru_evicts_oldest_when_capacity_reached() {
        let cache: ParsedSourceLru<u32> = ParsedSourceLru::with_capacity(2);
        let _a = cache.get_or_parse(b"a", b"", |_| 1u32);
        let _b = cache.get_or_parse(b"b", b"", |_| 2u32);
        let _c = cache.get_or_parse(b"c", b"", |_| 3u32);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(SourceHash::of(b"a", b"")).is_none());
        assert!(cache.get(SourceHash::of(b"b", b"")).is_some());
        assert!(cache.get(SourceHash::of(b"c", b"")).is_some());
    }

    /// Re-fetching an entry bumps it to most-recently-used so a
    /// subsequent insertion evicts a different one.
    #[test]
    fn lru_recency_promotes_on_get() {
        let cache: ParsedSourceLru<u32> = ParsedSourceLru::with_capacity(2);
        let _a = cache.get_or_parse(b"a", b"", |_| 1u32);
        let _b = cache.get_or_parse(b"b", b"", |_| 2u32);
        assert!(cache.get(SourceHash::of(b"a", b"")).is_some());
        let _c = cache.get_or_parse(b"c", b"", |_| 3u32);
        assert!(cache.get(SourceHash::of(b"a", b"")).is_some());
        assert!(cache.get(SourceHash::of(b"b", b"")).is_none());
        assert!(cache.get(SourceHash::of(b"c", b"")).is_some());
    }

    #[test]
    fn lru_eviction_does_not_scan_or_retain_stream_length_stale_recency() {
        let cache: ParsedSourceLru<u32> = ParsedSourceLru::with_capacity(8);
        for i in 0..128u32 {
            let source = i.to_le_bytes();
            let _ = cache.get_or_parse(&source, b"", |_| i);
        }

        assert_eq!(cache.len(), 8);
        assert!(
            cache.coldest_heap_len_for_diagnostics() <= 32,
            "Fix: parsed-source LRU stale recency heap must stay cache-capacity scale, not corpus-size scale"
        );
    }

    /// Capacity 0 disables caching: the parse closure runs every call.
    #[test]
    fn capacity_zero_disables_caching() {
        let cache: ParsedSourceLru<u32> = ParsedSourceLru::with_capacity(0);
        let calls = AtomicUsize::new(0);
        assert_eq!(
            *cache.get_or_parse(b"a", b"", |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                7u32
            }),
            7
        );
        assert_eq!(
            *cache.get_or_parse(b"a", b"", |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                7u32
            }),
            7
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(cache.len(), 0);
    }

    /// `is_empty` reflects emptiness vs. populated-state.
    #[test]
    fn is_empty_tracks_population() {
        let cache: ParsedSourceLru<u32> = ParsedSourceLru::with_capacity(2);
        assert!(cache.is_empty());
        assert_eq!(*cache.get_or_parse(b"a", b"", |_| 1u32), 1);
        assert!(!cache.is_empty());
    }

    /// Updating an existing key keeps capacity stable (the `len` stays
    /// at one, no eviction loop fires).
    #[test]
    fn insert_existing_key_does_not_evict() {
        let cache: ParsedSourceLru<u32> = ParsedSourceLru::with_capacity(2);
        let key = SourceHash::of(b"a", b"");
        assert!(cache.get(key).is_none());
        let _first = cache.insert(key, 1);
        assert_eq!(*cache.get(key).expect("Fix: after first insert"), 1);
        let _second = cache.insert(key, 2);
        assert_eq!(*cache.get(key).expect("Fix: after second insert"), 2);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn poisoned_source_cache_lock_is_not_silently_recovered() {
        let cache = Arc::new(ParsedSourceLru::<u32>::with_capacity(2));
        let poisoned = Arc::clone(&cache);
        let _ = std::thread::spawn(move || {
            let _guard = poisoned.lock_inner();
            panic!("poison parsed-source cache");
        })
        .join();

        let panic = std::panic::catch_unwind(|| {
            let _ = cache.len();
        })
        .expect_err("poisoned parsed-source cache must panic instead of recovering");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&'static str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("parsed-source LRU cache lock was poisoned"),
            "{message}"
        );
    }
}

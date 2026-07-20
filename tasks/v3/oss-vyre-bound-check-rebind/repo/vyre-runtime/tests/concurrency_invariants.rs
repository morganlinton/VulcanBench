//! P1 inventory #91  -  schedule/concurrency invariants.
//!
//! The vyre runtime exposes pipeline-cache + tenant-queue surfaces
//! that must survive poisoned locks, abrupt worker shutdown, and
//! callback-loss without producing UB or unbounded growth.
//!
//! The tests here are CPU-only so they run on every CI matrix entry,
//! not just GPU runners. They drive the in-memory cache + bounded
//! reader paths through several adversarial threadings.

use std::sync::Arc;
use std::thread;

use vyre_runtime::pipeline_cache::{
    InMemoryPipelineCache, PipelineCacheStore, PipelineFingerprint,
};

#[test]
fn pipeline_cache_handles_concurrent_get_put_without_panic() {
    // Many threads racing through the in-memory cache. Each thread
    // alternates put / get on its own fingerprint domain. The cache
    // must not panic, must not lose entries from a thread that
    // finishes successfully, and must not deadlock.
    let cache: Arc<dyn PipelineCacheStore> =
        Arc::new(InMemoryPipelineCache::with_limits(256, 16 * 1024 * 1024));
    let mut handles = Vec::new();
    for tid in 0..8u8 {
        let cache = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for i in 0..32u8 {
                let mut bytes = [0u8; 32];
                bytes[0] = tid;
                bytes[1] = i;
                let fp = PipelineFingerprint(bytes);
                let payload = vec![tid; 16 + (i as usize)];
                cache.put(fp, payload.clone());
                let read = cache.get(&fp);
                assert_eq!(read.as_deref(), Some(payload.as_slice()));
            }
        }));
    }
    for h in handles {
        h.join().expect("worker thread panicked under contention");
    }
}

#[test]
fn pipeline_cache_drop_during_lookup_does_not_panic() {
    // Build a cache, hand a clone to a worker that sleeps briefly,
    // then drop the original handle. The worker's lookup must not
    // panic and must not produce a dangling reference. (The Arc keeps
    // the cache alive as long as any handle exists.)
    let cache: Arc<dyn PipelineCacheStore> =
        Arc::new(InMemoryPipelineCache::with_limits(64, 1024 * 1024));
    let fp = PipelineFingerprint([0u8; 32]);
    cache.put(fp, vec![1u8, 2, 3, 4]);

    let cache_clone = Arc::clone(&cache);
    let worker = thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        cache_clone
            .get(&fp)
            .expect("Fix: cache lost a put under concurrent drop")
    });

    drop(cache); // releases the spawning-side handle.
    let result = worker.join().expect("worker thread panicked");
    assert_eq!(&result[..], &[1u8, 2, 3, 4]);
}

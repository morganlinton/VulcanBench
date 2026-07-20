//! ROADMAP L3  -  parallel parse across file corpus.
//!
//! Fan out `ParsedSourceLru::get_or_parse` across all available cores via
//! `rayon::par_iter`.  Corpus-wide deduplication still happens because each
//! unique `(content, extra)` pair is submitted to the cache exactly once;
//! duplicate entries in the input slice map back to the same `Arc<T>`.
//!
//! ## Design notes
//!
//! - Ordering is preserved: the final sequential pass maps each input index
//!   back to its parsed `Arc<T>`.
//! - The cache is `Send + Sync` (backed by `Mutex<…>`), so sharing a
//!   `&ParsedSourceLru<T>` across rayon workers is safe.
//! - `parse` must be `Fn(&[u8]) -> T + Sync`; the closure is invoked from
//!   multiple threads but never mutates shared state.
//! - To avoid paying the parse cost multiple times for the same key under
//!   concurrent cache misses (the L2 cache does not dedup in-flight parses),
//!   the implementation first identifies unique keys, then calls
//!   `get_or_parse` once per unique key.

use rayon::prelude::*;
// SourceHash is a 32-byte digest  -  FxHash on byte arrays is materially
// faster than std SipHash, and these tables are pure-internal scratch
// (no adversarial-input concern; the hash is already a content digest).
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::sync::Arc;

use super::source_cache::{ParsedSourceLru, SourceHash};

/// Parse every `(content, extra_key)` pair in `sources` in parallel,
/// memoising through `cache`.  Returns `Arc<T>` values in input order.
///
/// # Type parameters
///
/// * `T`  -  parsed artifact; must be `Send + Sync` so it can cross thread
///   boundaries inside `Arc<T>`.
/// * `F`  -  parse function; must be `Sync` because it is called from
///   multiple rayon workers concurrently.
///
/// # Example
///
/// ```
/// use vyre_libs::parsing::source_cache::ParsedSourceLru;
/// use vyre_libs::parsing::parallel_parse::parse_corpus_parallel;
/// use std::sync::Arc;
///
/// let cache = ParsedSourceLru::with_capacity(64);
/// let sources: Vec<(Vec<u8>, Vec<u8>)> = vec![
///     (b"int main() {}".to_vec(), b"".to_vec()),
///     (b"float x;".to_vec(), b"-O2".to_vec()),
/// ];
/// let results: Vec<Arc<usize>> = parse_corpus_parallel(&sources, &cache, |src| src.len());
/// assert_eq!(*results[0], 13);
/// assert_eq!(*results[1], 8);
/// ```
pub fn parse_corpus_parallel<T, F>(
    sources: &[(Vec<u8>, Vec<u8>)],
    cache: &ParsedSourceLru<T>,
    parse: F,
) -> Vec<Arc<T>>
where
    T: Send + Sync,
    F: Fn(&[u8]) -> T + Sync,
{
    // Phase 1  -  compute all content hashes in parallel (no locking).
    let keys: Vec<SourceHash> = sources
        .par_iter()
        .map(|(content, extra)| SourceHash::of(content, extra))
        .collect();

    // Phase 2  -  sequentially identify the first index of each unique key.
    // This is O(N) and allocation-light; it guarantees that even on a
    // cold cache the expensive `parse` closure runs once per unique source.
    let mut unique_indices = Vec::with_capacity(keys.len());
    let mut seen = HashSet::with_capacity_and_hasher(keys.len(), Default::default());
    for (idx, key) in keys.iter().enumerate() {
        if seen.insert(*key) {
            unique_indices.push(idx);
        }
    }

    // Phase 3  -  parse each unique source in parallel via get_or_parse.
    let unique_parsed: Vec<(SourceHash, Arc<T>)> = unique_indices
        .into_par_iter()
        .map(|idx| {
            let (content, extra) = &sources[idx];
            let arc = cache.get_or_parse(content, extra, |s| parse(s));
            (keys[idx], arc)
        })
        .collect();

    // Phase 4  -  build lookup and map back to input order.
    let lookup: HashMap<SourceHash, Arc<T>> = unique_parsed.into_iter().collect();
    keys.into_iter()
        .enumerate()
        .map(|(idx, key)| {
            if let Some(parsed) = lookup.get(&key) {
                parsed.clone()
            } else {
                let (content, extra) = &sources[idx];
                cache.get_or_parse(content, extra, |s| parse(s))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Parse 10 distinct sources and assert every result matches the
    /// expected value.
    #[test]
    fn distinct_corpus_parses_correctly() {
        let cache: ParsedSourceLru<usize> = ParsedSourceLru::with_capacity(16);
        let sources: Vec<(Vec<u8>, Vec<u8>)> = (0..10)
            .map(|i| {
                let content = format!("source {}", i).into_bytes();
                (content, vec![])
            })
            .collect();

        let results = parse_corpus_parallel(&sources, &cache, |src| src.len());

        assert_eq!(results.len(), 10);
        for (i, arc) in results.iter().enumerate() {
            assert_eq!(**arc, format!("source {}", i).len());
        }
    }

    /// Many entries share the same content; the parse closure must run
    /// once per unique (content, extra) pair.
    #[test]
    fn shared_content_dedups_parse_calls() {
        let cache: ParsedSourceLru<usize> = ParsedSourceLru::with_capacity(16);
        let calls = AtomicUsize::new(0);

        // 20 entries, only 3 unique (content, extra) pairs.
        let sources: Vec<(Vec<u8>, Vec<u8>)> = vec![
            (b"alpha".to_vec(), b"".to_vec()),
            (b"beta".to_vec(), b"".to_vec()),
            (b"alpha".to_vec(), b"".to_vec()),
            (b"gamma".to_vec(), b"x".to_vec()),
            (b"beta".to_vec(), b"".to_vec()),
            (b"alpha".to_vec(), b"".to_vec()),
            (b"gamma".to_vec(), b"x".to_vec()),
            (b"beta".to_vec(), b"".to_vec()),
            (b"alpha".to_vec(), b"".to_vec()),
            (b"gamma".to_vec(), b"x".to_vec()),
            (b"beta".to_vec(), b"".to_vec()),
            (b"alpha".to_vec(), b"".to_vec()),
            (b"gamma".to_vec(), b"x".to_vec()),
            (b"beta".to_vec(), b"".to_vec()),
            (b"alpha".to_vec(), b"".to_vec()),
            (b"gamma".to_vec(), b"x".to_vec()),
            (b"beta".to_vec(), b"".to_vec()),
            (b"alpha".to_vec(), b"".to_vec()),
            (b"gamma".to_vec(), b"x".to_vec()),
            (b"beta".to_vec(), b"".to_vec()),
        ];

        let _results = parse_corpus_parallel(&sources, &cache, |src| {
            calls.fetch_add(1, Ordering::SeqCst);
            src.len()
        });

        // 3 unique keys => 3 parse calls.
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        // Cache should hold exactly 3 entries.
        assert_eq!(cache.len(), 3);
    }

    /// Empty corpus returns an empty vector without panicking.
    #[test]
    fn empty_corpus_returns_empty() {
        let cache: ParsedSourceLru<usize> = ParsedSourceLru::with_capacity(4);
        let sources: Vec<(Vec<u8>, Vec<u8>)> = vec![];
        let results = parse_corpus_parallel(&sources, &cache, |src| src.len());
        assert!(results.is_empty());
    }

    /// With many distinct sources, parsing must overlap across rayon
    /// workers instead of degenerating into serial execution.
    #[test]
    fn parallel_parse_overlaps_workers() {
        let cache: ParsedSourceLru<usize> = ParsedSourceLru::with_capacity(32);
        let n = rayon::current_num_threads().max(2) * 4;
        let sources: Vec<(Vec<u8>, Vec<u8>)> = (0..n)
            .map(|i| (format!("slow{}", i).into_bytes(), vec![]))
            .collect();

        let active = AtomicUsize::new(0);
        let max_active = AtomicUsize::new(0);
        let results = parse_corpus_parallel(&sources, &cache, |src| {
            let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
            let mut observed = max_active.load(Ordering::SeqCst);
            while now_active > observed {
                match max_active.compare_exchange(
                    observed,
                    now_active,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(next) => observed = next,
                }
            }
            let mut acc = 0usize;
            for i in 0..50_000usize {
                acc = acc.wrapping_add(i ^ src.len());
            }
            std::hint::black_box(acc);
            active.fetch_sub(1, Ordering::SeqCst);
            src.len()
        });

        assert_eq!(results.len(), n);
        for (i, arc) in results.iter().enumerate() {
            assert_eq!(
                **arc,
                format!("slow{}", i).len(),
                "result value mismatch at index {}",
                i
            );
        }

        if rayon::current_num_threads() > 1 {
            assert!(
                max_active.load(Ordering::SeqCst) > 1,
                "parse closures did not overlap across rayon workers"
            );
        }
    }

    /// Adversarial: zero-capacity cache with duplicate content.  Every
    /// entry must still parse successfully even though the cache discards
    /// everything.
    #[test]
    fn zero_capacity_cache_still_returns_all() {
        let cache: ParsedSourceLru<usize> = ParsedSourceLru::with_capacity(0);
        let sources: Vec<(Vec<u8>, Vec<u8>)> = vec![(b"dup".to_vec(), b"".to_vec()); 5];

        let results = parse_corpus_parallel(&sources, &cache, |src| src.len());

        assert_eq!(results.len(), 5);
        for arc in &results {
            assert_eq!(**arc, 3);
        }
    }

    /// Adversarial: extremely large corpus (10 000 entries) where every
    /// entry is identical.  Must not deadlock, must return correct length,
    /// and parse closure must run exactly once because dedup happens
    /// before any cache interaction.
    #[test]
    fn massive_identical_corpus_no_deadlock() {
        let cache: ParsedSourceLru<usize> = ParsedSourceLru::with_capacity(1);
        let n = 10_000;
        let sources: Vec<(Vec<u8>, Vec<u8>)> = vec![(b"identical".to_vec(), b"".to_vec()); n];

        let calls = AtomicUsize::new(0);
        let results = parse_corpus_parallel(&sources, &cache, |src| {
            calls.fetch_add(1, Ordering::SeqCst);
            src.len()
        });

        assert_eq!(results.len(), n);
        for arc in &results {
            assert_eq!(**arc, 9);
        }
        // Phase 2 dedup guarantees exactly one parse call for one unique key.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

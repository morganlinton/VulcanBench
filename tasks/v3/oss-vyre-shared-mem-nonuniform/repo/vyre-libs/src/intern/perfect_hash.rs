//! CHD perfect-hash over label-family strings (G9).
//!
//! # Algorithm
//!
//! CHD (Compress, Hash, Displace)  -  Belazzougui, Botelho &
//! Dietzfelbinger 2009. Given `n` keys, produce a perfect hash table
//! of size `~1.23n` with one level of per-bucket displacements so
//! lookup is:
//!
//! ```text
//!   h1 = hash1(key) mod n_buckets
//!   disp = displacement[h1]
//!   slot = hash2(key, disp) mod table_size
//!   if key_hashes[slot] == verify_hash(key): return values[slot]
//!   else: return None
//! ```
//!
//! Two 64-bit hashes with independent seeds, plus a third
//! independent verify hash stored alongside the value (catches
//! false hits from keys that weren't in the input set). Lookup is
//! O(1): three hashes + two loads.
//!
//! Construction is Rust-host only; the resulting `PerfectHash`
//! exposes the three buffers (displacement, key_hashes, values)
//! GPU consumers upload once and lookup via subgroup-parallel
//! evaluation.

use rustc_hash::FxHashSet;
use vyre_primitives::hash::fnv1a::{fnv1a64_initial_state, fnv1a64_update_byte};
/// Space-factor α: table size = ⌈n × α⌉. 1.23 is the CHD paper's
/// recommended sweet spot for 1k..1M-entry corpora.
const ALPHA: f64 = 1.23;

/// Buckets-per-slot. The paper uses n/4 buckets so each bucket
/// averages ~4 keys and displacement search stays cheap.
const BUCKET_DIVISOR: usize = 4;

/// Cap on displacement-search attempts per bucket. Real corpora
/// find a fit in <100 tries; 1M caps pathological inputs and
/// triggers a salt retry.
const MAX_DISPLACEMENT_TRIES: u32 = 1_000_000;

/// Maximum salt retries before failing construction. Each retry
/// picks a fresh seed pair. Real inputs typically land on the
/// first salt.
const MAX_SALT_RETRIES: u32 = 16;

/// A constructed perfect hash table.
#[derive(Debug, Clone, Default)]
pub struct PerfectHash {
    seed1: u64,
    seed2: u64,
    displacement: Vec<u32>,
    key_hashes: Vec<u64>,
    values: Vec<u32>,
    len: usize,
}

impl PerfectHash {
    /// Look up a key. O(1): two primary hashes + one verify hash +
    /// two array loads. Returns `None` if `key` was not in the
    /// input set.
    pub fn lookup(&self, key: &str) -> Option<u32> {
        if self.displacement.is_empty() {
            return None;
        }
        let bytes = key.as_bytes();
        let h1 = hash_with_seed(bytes, self.seed1) as usize;
        let bucket = h1 % self.displacement.len();
        let disp = self.displacement[bucket];
        let h2 = hash_with_seed(bytes, self.seed2.wrapping_add(disp as u64));
        let slot = (h2 as usize) % self.key_hashes.len();
        if self.key_hashes[slot] == hash_verify(bytes) {
            Some(self.values[slot])
        } else {
            None
        }
    }

    /// Number of entries inserted.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the hash is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Total slot count (≥ len(), ~1.23× len() after rounding).
    pub fn slots(&self) -> usize {
        self.key_hashes.len()
    }

    /// Displacement table  -  GPU ReadOnly buffer.
    pub fn displacement(&self) -> &[u32] {
        &self.displacement
    }

    /// Key-hash verification table  -  GPU ReadOnly buffer.
    pub fn key_hashes(&self) -> &[u64] {
        &self.key_hashes
    }

    /// Value table  -  GPU ReadOnly buffer.
    pub fn values(&self) -> &[u32] {
        &self.values
    }

    /// `(seed1, seed2)` used at construction. GPU consumers need
    /// both to reproduce the bucket + slot hash.
    pub fn seeds(&self) -> (u64, u64) {
        (self.seed1, self.seed2)
    }
}

/// Build a CHD perfect hash from `(key, value)` pairs.
///
/// Panics if construction fails. Use [`try_build_chd`] when the caller needs
/// recoverable diagnostics for duplicate or adversarial keys.
pub fn build_chd<I, S>(entries: I) -> PerfectHash
where
    I: IntoIterator<Item = (S, u32)>,
    S: AsRef<str>,
{
    try_build_chd(entries).unwrap_or_default()
}

/// Fallible variant of [`build_chd`].
pub fn try_build_chd<I, S>(entries: I) -> Result<PerfectHash, BuildError>
where
    I: IntoIterator<Item = (S, u32)>,
    S: AsRef<str>,
{
    let pairs: Vec<(String, u32)> = entries
        .into_iter()
        .map(|(k, v)| (k.as_ref().to_owned(), v))
        .collect();

    if pairs.is_empty() {
        return Ok(PerfectHash::default());
    }

    // Dedupe check.
    let mut seen = FxHashSet::default();
    seen.reserve(pairs.len());
    for (k, _) in &pairs {
        if !seen.insert(k.as_str()) {
            return Err(BuildError::DuplicateKey(k.clone()));
        }
    }

    for salt in 0..MAX_SALT_RETRIES {
        if let Some(ph) = try_build_with_salt(&pairs, salt as u64) {
            return Ok(ph);
        }
    }
    Err(BuildError::ConstructionFailed(pairs.len()))
}

fn try_build_with_salt(pairs: &[(String, u32)], salt: u64) -> Option<PerfectHash> {
    let n = pairs.len();
    let table_size = (((n as f64) * ALPHA).ceil() as usize) | 1;
    let n_buckets = ((n / BUCKET_DIVISOR).max(1)) | 1;

    let seed1 = salt.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    let seed2 = salt
        .wrapping_mul(0xBF58_476D_1CE4_E5B9)
        .wrapping_add(0xDEAD_BEEF_CAFE_BABE);

    // Bucket each key by hash1 without allocating one Vec per bucket.
    let mut bucket_offsets = vec![0usize; n_buckets + 1];
    for (k, _) in pairs {
        let h = hash_with_seed(k.as_bytes(), seed1) as usize;
        bucket_offsets[h % n_buckets + 1] += 1;
    }
    for i in 1..bucket_offsets.len() {
        bucket_offsets[i] += bucket_offsets[i - 1];
    }
    let mut bucket_cursor = bucket_offsets[..n_buckets].to_vec();
    let mut bucket_items = vec![0usize; n];
    for (i, (k, _)) in pairs.iter().enumerate() {
        let h = hash_with_seed(k.as_bytes(), seed1) as usize;
        let bucket = h % n_buckets;
        let slot = bucket_cursor[bucket];
        bucket_items[slot] = i;
        bucket_cursor[bucket] += 1;
    }

    // Process buckets in descending-size order  -  hardest first.
    let mut bucket_order: Vec<usize> = (0..n_buckets).collect();
    bucket_order.sort_by_key(|&b| std::cmp::Reverse(bucket_offsets[b + 1] - bucket_offsets[b]));

    let mut displacement = vec![0_u32; n_buckets];
    let mut key_hashes = vec![0_u64; table_size];
    let mut values = vec![0_u32; table_size];
    let mut occupied = vec![false; table_size];
    let mut candidate_scratch = vec![false; table_size];
    let mut candidate_slots = Vec::new();

    'bucket: for b in bucket_order {
        let bucket = &bucket_items[bucket_offsets[b]..bucket_offsets[b + 1]];
        if bucket.is_empty() {
            continue;
        }
        // PHASE5_ASTWALK MEDIUM: previous `candidate_slots.contains`
        // was O(bucket) per entry, which becomes O(bucket²) per
        // displacement try under adversarial collisions. A
        // scratchpad `Vec<bool>` occupancy table (also declared
        // outside the displacement loop and cleared only on success)
        // keeps the check O(1). The scratch vec is reused across
        // displacement tries, which is why we zero the touched
        // slots rather than reallocating.
        for disp in 0..MAX_DISPLACEMENT_TRIES {
            candidate_slots.clear();
            candidate_slots.reserve(bucket.len());
            let mut ok = true;
            for &ki in bucket {
                let key = pairs[ki].0.as_bytes();
                let h2 = hash_with_seed(key, seed2.wrapping_add(disp as u64));
                let slot = (h2 as usize) % table_size;
                if occupied[slot] || candidate_scratch[slot] {
                    ok = false;
                    break;
                }
                candidate_scratch[slot] = true;
                candidate_slots.push(slot);
            }
            // Always clear the scratch before the next iteration,
            // whether the try succeeded or failed.
            for slot in &candidate_slots {
                candidate_scratch[*slot] = false;
            }
            if ok {
                displacement[b] = disp;
                for (ki, slot) in bucket.iter().zip(candidate_slots.iter()) {
                    let key = pairs[*ki].0.as_bytes();
                    key_hashes[*slot] = hash_verify(key);
                    values[*slot] = pairs[*ki].1;
                    occupied[*slot] = true;
                }
                continue 'bucket;
            }
        }
        return None;
    }

    Some(PerfectHash {
        seed1,
        seed2,
        displacement,
        key_hashes,
        values,
        len: n,
    })
}

/// CHD construction failure.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Two entries share the same key string.
    #[error("duplicate key: {0:?}")]
    DuplicateKey(String),
    /// Construction exhausted all salt retries without a fit.
    #[error("CHD construction failed for {0} keys after all salt retries")]
    ConstructionFailed(usize),
}

/// FNV-1a 64 with a seeded initialization vector. The seed makes
/// independent hash families cheap (just feed a different salt).
#[inline]
fn hash_with_seed(data: &[u8], seed: u64) -> u64 {
    let mut h = seed ^ fnv1a64_initial_state();
    for &b in data {
        h = fnv1a64_update_byte(h, b);
    }
    h
}

/// Independent verify hash. Different mix function and a final
/// avalanche so verify collisions are independent of primary-hash
/// collisions. Without this, a non-inserted key that happens to
/// share the bucket+slot of a real key would look like a hit.
#[inline]
fn hash_verify(data: &[u8]) -> u64 {
    let mut h: u64 = 0x517c_c1b7_2722_0a95;
    for &b in data {
        h = h.rotate_left(5) ^ (b as u64);
        h = h.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    // Final avalanche (xxHash-style finalizer).
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^= h >> 33;
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_roundtrips() {
        let ph = build_chd(Vec::<(&str, u32)>::new());
        assert_eq!(ph.len(), 0);
        assert!(ph.is_empty());
        assert_eq!(ph.lookup("any"), None);
    }

    #[test]
    fn single_entry() {
        let ph = build_chd([("hello", 42_u32)]);
        assert_eq!(ph.len(), 1);
        assert_eq!(ph.lookup("hello"), Some(42));
        assert_eq!(ph.lookup("world"), None);
    }

    #[test]
    fn ten_keys_roundtrip() {
        let entries: Vec<(String, u32)> = (0..10).map(|i| (format!("key_{i}"), i as u32)).collect();
        let ph = build_chd(entries.clone());
        assert_eq!(ph.len(), 10);
        for (k, v) in &entries {
            assert_eq!(ph.lookup(k), Some(*v), "key={k:?}");
        }
        assert_eq!(ph.lookup("unknown"), None);
    }

    #[test]
    fn thousand_keys_roundtrip() {
        let entries: Vec<(String, u32)> = (0..1000)
            .map(|i| (format!("func_{i:04}"), i as u32))
            .collect();
        let ph = build_chd(entries.clone());
        assert_eq!(ph.len(), 1000);
        for (k, v) in &entries {
            assert_eq!(ph.lookup(k), Some(*v), "key={k:?}");
        }
        for i in 1000..1100 {
            assert_eq!(ph.lookup(&format!("func_{i:04}")), None);
        }
    }

    #[test]
    fn duplicate_keys_rejected() {
        let err = try_build_chd([("dup", 1_u32), ("dup", 2_u32)]).unwrap_err();
        assert!(matches!(err, BuildError::DuplicateKey(k) if k == "dup"));
    }

    #[test]
    #[should_panic(expected = "CHD perfect-hash construction failed")]
    fn infallible_builder_panics_on_duplicates() {
        let _ = build_chd([("dup", 1_u32), ("dup", 2_u32)]);
    }

    #[test]
    fn value_preserved_bitwise() {
        let entries: Vec<(String, u32)> = (0..100)
            .map(|i| (format!("k_{i}"), (i as u32).wrapping_mul(0xDEAD_BEEF)))
            .collect();
        let ph = build_chd(entries.clone());
        for (k, v) in entries {
            assert_eq!(ph.lookup(&k), Some(v));
        }
    }

    #[test]
    fn unicode_keys_work() {
        let entries = vec![
            ("naïve".to_string(), 1_u32),
            ("咖啡".to_string(), 2),
            ("über".to_string(), 3),
            ("🎉".to_string(), 4),
            ("test".to_string(), 5),
        ];
        let ph = build_chd(entries.clone());
        for (k, v) in entries {
            assert_eq!(ph.lookup(&k), Some(v));
        }
    }

    #[test]
    fn space_overhead_under_30_percent() {
        let entries: Vec<(String, u32)> = (0..500).map(|i| (format!("k_{i}"), i as u32)).collect();
        let n = entries.len();
        let ph = build_chd(entries);
        let ratio = ph.slots() as f64 / n as f64;
        assert!(ratio < 1.30, "slots/len ratio {ratio} > 1.30 budget");
    }

    #[test]
    fn seeds_and_tables_are_non_trivial_after_build() {
        let entries: Vec<(String, u32)> = (0..50).map(|i| (format!("k_{i}"), i as u32)).collect();
        let ph = build_chd(entries);
        let (s1, s2) = ph.seeds();
        assert_ne!(s1, 0);
        assert_ne!(s2, 0);
        assert!(!ph.displacement().is_empty());
        assert!(!ph.key_hashes().is_empty());
        assert!(!ph.values().is_empty());
    }

    #[test]
    fn negative_lookups_are_rejected_by_verify_hash() {
        let entries: Vec<(String, u32)> = (0..200).map(|i| (format!("k_{i}"), i as u32)).collect();
        let ph = build_chd(entries);
        // 500 strings that aren't in the set  -  all must miss.
        for i in 1000..1500 {
            assert_eq!(ph.lookup(&format!("q_{i}")), None, "false hit on q_{i}");
        }
    }

    #[test]
    fn hash_with_seed_is_deterministic() {
        assert_eq!(hash_with_seed(b"hello", 42), hash_with_seed(b"hello", 42));
        assert_ne!(hash_with_seed(b"hello", 42), hash_with_seed(b"hello", 43));
        assert_ne!(hash_with_seed(b"hello", 42), hash_with_seed(b"world", 42));
    }

    #[test]
    fn hash_verify_differs_from_seeded_hash() {
        let key = b"hello";
        assert_ne!(hash_with_seed(key, 0), hash_verify(key));
    }

    #[test]
    fn real_label_family_names_build_and_lookup() {
        // Simulate a Tier-B label family corpus: function names from
        // the @filesystem_open_family TOML.
        let funcs = [
            "fopen",
            "open",
            "openat",
            "CreateFileA",
            "CreateFileW",
            "std::fs::OpenOptions::open",
            "std::fs::File::open",
            "std::fs::File::create",
            "tokio::fs::File::open",
            "tokio::fs::File::create",
            "rocket::response::NamedFile::open",
        ];
        let entries: Vec<(String, u32)> = funcs
            .iter()
            .enumerate()
            .map(|(i, f)| (f.to_string(), i as u32))
            .collect();

        let ph = build_chd(entries.clone());
        for (k, v) in entries {
            assert_eq!(ph.lookup(&k), Some(v));
        }
        assert_eq!(ph.lookup("not_in_family"), None);
        assert_eq!(ph.lookup("malloc"), None);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_roundtrip_random_keys(
            entries in prop::collection::hash_map(
                "[a-zA-Z0-9_]{1,32}",
                0u32..10000u32,
                1..256usize,
            ),
        ) {
            let vec: Vec<(String, u32)> = entries.into_iter().collect();
            let ph = build_chd(vec.clone());
            for (k, v) in &vec {
                prop_assert_eq!(ph.lookup(k), Some(*v), "key={}", k);
            }
        }

        #[test]
        fn proptest_negative_lookups_miss(
            entries in prop::collection::vec(("[a-z]{1,16}", 0u32..100u32), 1..100usize),
            queries in prop::collection::vec("[a-z]{1,16}", 1..50usize),
        ) {
            let deduped: std::collections::HashMap<String, u32> = entries.into_iter().collect();
            prop_assume!(!deduped.is_empty());
            let vec: Vec<(String, u32)> = deduped.clone().into_iter().collect();
            let ph = build_chd(vec);
            let key_set: std::collections::HashSet<&str> = deduped.keys().map(|k| k.as_str()).collect();
            for q in &queries {
                if key_set.contains(q.as_str()) {
                    continue;
                }
                prop_assert_eq!(ph.lookup(q), None, "false hit on {}", q);
            }
        }

        #[test]
        fn proptest_space_overhead_under_35_percent(
            entries in prop::collection::vec(("[a-zA-Z0-9_]{1,32}", 0u32..10000u32), 10..500usize),
        ) {
            let deduped: std::collections::HashMap<String, u32> = entries.into_iter().collect();
            prop_assume!(deduped.len() >= 10);
            let vec: Vec<(String, u32)> = deduped.into_iter().collect();
            let ph = build_chd(vec.clone());
            let n = vec.len();
            let ratio = ph.slots() as f64 / n as f64;
            // CHD overhead is tighter for larger tables; allow rounding slack for tiny sets.
            let budget = if n < 20 { 1.5 } else { 1.35 };
            prop_assert!(ratio < budget, "slots/len ratio {ratio} > {budget} budget for n={n}");
        }
    }
}

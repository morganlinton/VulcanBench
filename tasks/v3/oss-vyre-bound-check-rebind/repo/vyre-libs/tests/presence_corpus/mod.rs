//! Shared planted region-presence corpus for the presence-by-region dispatch
//! proofs (async, prepared, and resident pipelines).
//!
//! One coalesced haystack carries three "files" with KNOWN literal hit sets, laid
//! out exactly as keyhog's phase-1 coalesced batch: ascending `region_starts`
//! beginning at 0, each file terminated by a separator byte (newline) that is in
//! NO literal so no match spans a region boundary. Every presence test asserts
//! the decoded bitmap reproduces these exact per-region bit SETS — real values,
//! never "non-empty".

use std::collections::BTreeSet;

/// pattern_id order: key=0 token=1 secret=2 AKIA=3 ghp_=4 sk_live_=5 password=6 api=7
pub(crate) const LITERALS: &[&[u8]] = &[
    b"key", b"token", b"secret", b"AKIA", b"ghp_", b"sk_live_", b"password", b"api",
];

/// A "file" carrying a known subset of literal hits, terminated by a separator
/// byte (newline) that is in NO literal, so no match spans the region boundary.
fn file_with(hits: &str) -> Vec<u8> {
    let mut v = hits.as_bytes().to_vec();
    v.push(b'\n');
    v
}

/// Three coalesced files with distinct, KNOWN hit sets, returned as a coalesced
/// haystack + ascending region starts (the keyhog phase-1 layout).
pub(crate) fn planted_corpus() -> (Vec<u8>, Vec<u32>) {
    let files = [
        file_with("api key here AKIA token secret"), // {api,key,AKIA,token,secret} = {7,0,3,1,2}
        file_with("ghp_abc sk_live_xyz password"),    // {ghp_,sk_live_,password} = {4,5,6}
        file_with("plain prose with no anchors here"), // {} (no literal occurs)
    ];
    let mut haystack = Vec::new();
    let mut region_starts = Vec::new();
    for f in &files {
        region_starts.push(haystack.len() as u32);
        haystack.extend_from_slice(f);
    }
    (haystack, region_starts)
}

/// The exact planted hit set for each region of [`planted_corpus`], indexed by
/// region. Lets a non-bitmap consumer (e.g. the resident host-orchestration mock)
/// assert against the same ground truth as [`assert_planted_bits`].
pub(crate) fn planted_region_sets() -> [BTreeSet<u32>; 3] {
    [
        BTreeSet::from([0, 1, 2, 3, 7]),
        BTreeSet::from([4, 5, 6]),
        BTreeSet::new(),
    ]
}

/// Decode one region's presence row into the set of pattern ids whose bit is set.
pub(crate) fn present_ids(row: &[u32], pattern_count: u32) -> BTreeSet<u32> {
    (0..pattern_count)
        .filter(|&p| {
            let w = (p >> 5) as usize;
            let b = p & 31;
            row.get(w).is_some_and(|word| (word >> b) & 1 == 1)
        })
        .collect()
}

/// Assert the full `region_count * words` bitmap carries EXACTLY the planted hit
/// sets per region (real values, not "non-empty").
pub(crate) fn assert_planted_bits(words_per_region: usize, pattern_count: u32, bitmap: &[u32]) {
    let sets = planted_region_sets();
    let row = |r: usize| &bitmap[r * words_per_region..(r + 1) * words_per_region];
    for (r, expected) in sets.iter().enumerate() {
        assert_eq!(
            &present_ids(row(r), pattern_count),
            expected,
            "region {r} must carry exactly {expected:?}"
        );
    }
}

//! Real-GPU end-to-end proof for the FUSED region-presence + match-positions scan
//! METHOD (`GpuLiteralSet::scan_presence_and_positions_by_region`).
//!
//! The CPU `reference_eval` gates prove the fused PROGRAM's semantics, but they
//! bypass the scan method's dispatch plumbing — the borrowed-input binding order and
//! the `outputs[0]=presence / [1]=count / [2]=matches` decode. This test exercises
//! that plumbing on the real wgpu backend (the RTX 5090 here) and asserts the one
//! fused dispatch reproduces BOTH separate scans:
//!   - its per-region presence bitmap == `scan_presence_by_region`'s, word-for-word,
//!   - its match triple set == `scan`'s (and the CPU `reference_scan` oracle's).
//! It also reports the one-dispatch-vs-two timing. MEASURED RESULT (RTX 5090, wgpu,
//! release): the fused one-pass is ~20x SLOWER than the two passes, not faster — a
//! refuted hypothesis. The hypothesis was that saving one haystack walk would win;
//! instead the fused kernel is ~3x larger (the suffix3 prefilter inlines the replay
//! 3x and the fused replay is big) and the occupancy loss dwarfs the saved walk. So
//! the fold ships as a CORRECTNESS-equivalent primitive only; the timing is reported,
//! not asserted (see the long comment at the perf section). The real GPU-8MiB lever
//! is segmentation / dispatch-overhead, not fusion.
//!
//! Skips cleanly when no GPU is available.
//!
//! Run:
//!   cargo test -p vyre-libs --test literal_set_presence_and_positions_gpu --release -- --nocapture

use std::collections::BTreeSet;

use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::scan::GpuLiteralSet;

const LITERALS: &[&[u8]] = &[
    b"key", b"token", b"secret", b"AKIA", b"ghp_", b"sk_live_", b"password", b"api",
];

/// A "file" carrying a known subset of literal hits, terminated by a separator byte
/// (newline) that is in NO literal, so no match spans the region boundary — exactly
/// keyhog's coalesced-batch layout.
fn file_with(hits: &str) -> Vec<u8> {
    let mut v = hits.as_bytes().to_vec();
    v.push(b'\n');
    v
}

fn presence_bit(row: &[u32], pattern_id: u32) -> bool {
    let w = (pattern_id >> 5) as usize;
    let b = pattern_id & 31;
    row.get(w).is_some_and(|word| (word >> b) & 1 == 1)
}

#[test]
fn fused_scan_method_matches_separate_scans_on_gpu() {
    let backend = match WgpuBackend::shared() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("no wgpu backend ({e}); skipping fused GPU scan test");
            return;
        }
    };
    let matcher = GpuLiteralSet::compile(LITERALS);
    let pattern_count = LITERALS.len() as u32;
    let presence_words = pattern_count.div_ceil(32).max(1) as usize;
    let max_matches: u32 = 4096;

    // Three coalesced files with distinct hit sets.
    let files = [
        file_with("api key here AKIA token secret"), // api,key,AKIA,token,secret
        file_with("ghp_abc sk_live_xyz password"),   // ghp_,sk_live_,password
        file_with("plain prose with no anchors here"), // (none)
    ];
    let mut haystack = Vec::new();
    let mut region_starts = Vec::new();
    for f in &files {
        region_starts.push(haystack.len() as u32);
        haystack.extend_from_slice(f);
    }

    // ---- Fused: ONE dispatch -> presence + positions ----
    let mut fused_matches = Vec::new();
    let fused_presence = matcher
        .scan_presence_and_positions_by_region(
            backend.as_ref(),
            &haystack,
            &region_starts,
            0,
            max_matches,
            &mut fused_matches,
        )
        .expect("fused gpu scan");

    // ---- Separate scan 1: presence-by-region ----
    let sep_presence = matcher
        .scan_presence_by_region(backend.as_ref(), &haystack, &region_starts)
        .expect("gpu presence-by-region scan");

    // ---- Separate scan 2: global match triples ----
    let sep_matches = matcher
        .scan(backend.as_ref(), &haystack, max_matches)
        .expect("gpu triple scan");

    // (1) Fused presence bitmap == separate presence-by-region, word-for-word.
    assert_eq!(
        fused_presence, sep_presence,
        "fused per-region presence must equal scan_presence_by_region on the GPU"
    );

    // (2) Fused triple set == separate scan's == CPU reference oracle's.
    let to_set = |ms: &[vyre_foundation::match_result::Match]| -> BTreeSet<(u32, u32, u32)> {
        ms.iter().map(|m| (m.pattern_id, m.start, m.end)).collect()
    };
    let fused_set = to_set(&fused_matches);
    let sep_set = to_set(&sep_matches);
    let oracle_set = to_set(&matcher.reference_scan(&haystack));
    assert_eq!(
        fused_set, sep_set,
        "fused match triples must equal the separate GPU triple scan"
    );
    assert_eq!(
        fused_set, oracle_set,
        "fused match triples must equal the CPU reference_scan oracle"
    );
    assert!(
        !fused_set.is_empty(),
        "corpus must produce matches or the test proves nothing"
    );

    // (3) Concrete per-region presence cross-check (ids by LITERALS order).
    //   file 0 -> {api,key,AKIA,token,secret}; file 1 -> {ghp_,sk_live_,password};
    //   file 2 -> {} (no anchors).
    let id = |lit: &[u8]| LITERALS.iter().position(|&l| l == lit).unwrap() as u32;
    let row = |r: usize| &fused_presence[r * presence_words..(r + 1) * presence_words];
    for lit in [&b"api"[..], b"key", b"AKIA", b"token", b"secret"] {
        assert!(presence_bit(row(0), id(lit)), "file 0 must fire {lit:?}");
    }
    for lit in [&b"ghp_"[..], b"sk_live_", b"password"] {
        assert!(presence_bit(row(1), id(lit)), "file 1 must fire {lit:?}");
    }
    assert!(
        row(2).iter().all(|&w| w == 0),
        "file 2 has no anchors; its presence row must be empty"
    );

    // ---- Timing: one fused dispatch vs the two it replaces ----
    // CRITICAL WORKLOAD CAVEAT: the fold's win is doing ONE haystack walk instead of
    // two, which dominates only on SPARSE match density — keyhog's coalesced GPU
    // phase-1 regime (an 8 MiB scan fires ~10^2-10^3 anchors, not 10^5). On DENSE
    // data (a hit every few bytes) the per-hit triple append dominates and fusing it
    // into the presence kernel is SLOWER than two specialized passes (measured ~2x
    // slower at ~350k hits) — there the two-pass presence(atomic_or) + positions
    // stays better. So this benchmarks the SPARSE regime the fold targets; a density
    // gate belongs in the consumer, not here. Kept under the wgpu workgroup cap
    // (bytes/128 < 65535 ~= 8.39 MB).
    const BIG: usize = 4 * 1024 * 1024;
    // Benign filler carrying NO literal, with a lone anchor every ~8 KiB and a region
    // boundary every ~64 KiB: ~64 regions, ~512 hits over 4 MiB (sparse, keyhog-like).
    let filler: &[u8] = b"plain benign filler text with no anchors here at all 0123456789\n";
    let anchor: &[u8] = b" AKIA1234 ";
    let mut big = Vec::with_capacity(BIG + 1024);
    let mut big_starts = Vec::new();
    let mut next_region = 0usize;
    let mut next_anchor = 4 * 1024usize;
    while big.len() < BIG {
        if big.len() >= next_region {
            big_starts.push(big.len() as u32);
            next_region += 64 * 1024;
        }
        if big.len() >= next_anchor {
            big.extend_from_slice(anchor);
            next_anchor += 8 * 1024;
        }
        big.extend_from_slice(filler);
    }
    let big_max: u32 = 4_000_000;
    let mut scratch_matches = Vec::new();

    // Warm up shader compile / first-dispatch init for all three paths.
    let _ = matcher.scan_presence_and_positions_by_region(
        backend.as_ref(),
        &big[..4096],
        &[0],
        0,
        64,
        &mut scratch_matches,
    );
    let _ = matcher.scan_presence_by_region(backend.as_ref(), &big[..4096], &[0]);
    let _ = matcher.scan(backend.as_ref(), &big[..4096], 64);

    let t = std::time::Instant::now();
    let _ = matcher
        .scan_presence_and_positions_by_region(
            backend.as_ref(),
            &big,
            &big_starts,
            0,
            big_max,
            &mut scratch_matches,
        )
        .expect("fused big scan");
    let fused_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = std::time::Instant::now();
    let _ = matcher
        .scan_presence_by_region(backend.as_ref(), &big, &big_starts)
        .expect("presence big scan");
    let presence_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = std::time::Instant::now();
    let _ = matcher.scan(backend.as_ref(), &big, big_max).expect("triple big scan");
    let positions_ms = t.elapsed().as_secs_f64() * 1000.0;

    let two_pass_ms = presence_ms + positions_ms;
    eprintln!(
        "\n=== fused one-pass vs two-pass on {:.1} MB ({} regions) ===",
        big.len() as f64 / 1e6,
        big_starts.len()
    );
    eprintln!("  fused (1 dispatch)            : {fused_ms:>8.2} ms");
    eprintln!("  presence-by-region (pass 1)   : {presence_ms:>8.2} ms");
    eprintln!("  triple scan        (pass 2)   : {positions_ms:>8.2} ms");
    eprintln!("  two-pass total                : {two_pass_ms:>8.2} ms");
    eprintln!(
        "  fused speedup over two-pass   : {:.2}x  (>1 = fused wins)",
        two_pass_ms / fused_ms.max(1e-9)
    );

    // MEASURED NEGATIVE RESULT (RTX 5090, wgpu, release): the fused one-pass is
    // ~20x SLOWER than the two passes it replaces, NOT faster. Root cause:
    // `suffix3_prefilter_body` inlines the replay 3x (the i==0 / i==1 / general
    // prefilter exits). The presence-only and positions-only replays are small, so
    // 3x inlining is fine; the FUSED replay (region binary search + atomic_or +
    // triple append) is large, so the fused kernel is ~3x bigger → register/occupancy
    // collapse drags the WHOLE scan (even the per-byte walk that rarely hits a
    // candidate). Saving one haystack walk does not pay for the occupancy loss.
    //
    // So this is NOT asserted as a perf win — the fold, as a naive kernel fusion, is
    // refuted on wgpu. It remains a CORRECTNESS-equivalent primitive (the assertions
    // above are the real gate); making it a perf win would need the prefilter body to
    // call the replay as a function instead of inlining it 3x (or a CUDA-backend
    // measurement, untested here). The win lever for keyhog's GPU-8MiB phase-1 is
    // segmentation of the existing passes / dispatch-overhead reduction, not fusion.
    // The hard gate is correctness; perf is reported, not asserted.
    assert!(
        fused_ms.is_finite() && fused_ms > 0.0,
        "fused scan must have produced a timing"
    );
}

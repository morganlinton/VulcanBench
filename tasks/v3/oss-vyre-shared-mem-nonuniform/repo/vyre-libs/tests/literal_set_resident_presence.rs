//! GPU parity proof for the RESIDENT region-presence pipeline
//! (`GpuLiteralSet::prepare_resident_presence` → `ResidentPresencePipeline`).
//!
//! The resident pipeline uploads the seven immutable region-presence tables (DFA
//! transition / output-offset / output-record / pattern-length tables + the three
//! suffix-prefilter masks) into backend-resident resources ONCE, then re-dispatches
//! across a corpus transferring only the per-file haystack and a presence-prefix
//! reset. This test pins, on REAL GPU hardware (wgpu, the RTX 5090 here), that the
//! resident bitmap is byte-identical to the borrowed `scan_presence_by_region`
//! across repeated scans AND carries the exact planted per-region hit sets —
//! the resident table-residency optimization must not change a single result bit
//! (Law 10). Skips cleanly with no GPU.
//!
//! Run:
//!   cargo test -p vyre-libs --test literal_set_resident_presence --release -- --nocapture

mod presence_corpus;

use std::time::Instant;

use presence_corpus::{assert_planted_bits, planted_corpus, LITERALS};
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::scan::GpuLiteralSet;

#[test]
fn resident_region_presence_matches_borrowed_and_planted_hits_on_gpu() {
    let backend = match WgpuBackend::shared() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("no wgpu backend ({e}); skipping resident region-presence GPU test");
            return;
        }
    };
    let matcher = GpuLiteralSet::compile(LITERALS);
    let pattern_count = LITERALS.len() as u32;
    let words = pattern_count.div_ceil(32).max(1) as usize;
    let (haystack, region_starts) = planted_corpus();
    let region_count = region_starts.len();

    // Ground truth: the borrowed per-region presence scan.
    let borrowed_words = matcher
        .scan_presence_by_region(backend.as_ref(), &haystack, &region_starts)
        .expect("borrowed gpu presence-by-region scan");

    // Prepare a resident session sized for this corpus (a couple regions of head
    // room proves the dynamic-region-count path: max_regions > region_count).
    let session = matcher
        .prepare_resident_presence(backend.as_ref(), haystack.len() + 64, region_count as u32 + 2)
        .expect("prepare resident region-presence session");
    assert_eq!(session.max_regions(), region_count as u32 + 2);
    assert_eq!(session.presence_words(), words as u32);
    assert_eq!(session.pattern_count(), pattern_count);

    // Re-dispatch the SAME corpus several times through the resident session: the
    // immutable tables stay resident (uploaded once at prepare), and every scan
    // must reproduce the borrowed bitmap word-for-word.
    let mut out = Vec::new();
    let mut scratch = Vec::new();
    for iter in 0..4 {
        session
            .scan_into(
                backend.as_ref(),
                &haystack,
                &region_starts,
                0,
                &mut out,
                &mut scratch,
            )
            .expect("resident region-presence scan");
        assert_eq!(
            out, borrowed_words,
            "iteration {iter}: resident bitmap must equal scan_presence_by_region word-for-word"
        );
        assert_eq!(
            out.len(),
            region_count * words,
            "iteration {iter}: bitmap must be region_count ({region_count}) * words ({words}) u32s"
        );
        assert_planted_bits(words, pattern_count, &out);
    }

    session
        .free(backend.as_ref())
        .expect("free resident region-presence session");
}

#[test]
fn resident_region_presence_serves_smaller_batches_under_the_cap_on_gpu() {
    // One resident session sized for the full corpus must also correctly scan a
    // SMALLER batch (fewer regions than max_regions) — proving the kernel reads the
    // live region count from buf_len(region_starts), not the compiled cap.
    let backend = match WgpuBackend::shared() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("no wgpu backend ({e}); skipping resident sub-batch GPU test");
            return;
        }
    };
    let matcher = GpuLiteralSet::compile(LITERALS);
    let pattern_count = LITERALS.len() as u32;
    let words = pattern_count.div_ceil(32).max(1) as usize;
    let (haystack, region_starts) = planted_corpus();

    let session = matcher
        .prepare_resident_presence(backend.as_ref(), haystack.len() + 64, region_starts.len() as u32)
        .expect("prepare resident region-presence session");

    // A two-region sub-batch: just the first file (regions {0} start, terminated by
    // the second file's start). Scan only region 0's bytes with a single region.
    let single_region_start = [0u32];
    let first_file_end = region_starts[1] as usize; // start of file 1 == end of file 0
    let first_file = &haystack[..first_file_end];

    let borrowed = matcher
        .scan_presence_by_region(backend.as_ref(), first_file, &single_region_start)
        .expect("borrowed single-region scan");

    let mut out = Vec::new();
    let mut scratch = Vec::new();
    session
        .scan_into(
            backend.as_ref(),
            first_file,
            &single_region_start,
            0,
            &mut out,
            &mut scratch,
        )
        .expect("resident single-region scan under the cap");

    assert_eq!(
        out, borrowed,
        "a 1-region sub-batch on a session capped for the full corpus must match the borrowed scan"
    );
    assert_eq!(out.len(), words, "single region -> exactly `words` u32s");
    // Region 0 of the planted corpus carries {key,token,secret,AKIA,api} = {0,1,2,3,7}.
    let present: std::collections::BTreeSet<u32> = presence_corpus::present_ids(&out, pattern_count);
    assert_eq!(
        present,
        std::collections::BTreeSet::from([0, 1, 2, 3, 7]),
        "the first planted file carries exactly {{key,token,secret,AKIA,api}}"
    );

    session.free(backend.as_ref()).expect("free session");
}

/// A coalesced batch: `regions` small files (each carrying a couple of the planted
/// detector tokens), separated by a newline that is in no token, plus its ascending
/// `region_starts`. Mirrors keyhog's phase-1 coalesced layout.
fn synth_batch(detectors: &[Vec<u8>], regions: usize, batch_seed: usize) -> (Vec<u8>, Vec<u32>) {
    let mut haystack = Vec::new();
    let mut region_starts = Vec::new();
    for r in 0..regions {
        region_starts.push(haystack.len() as u32);
        // Plant two detector tokens per region, varied by batch+region so different
        // batches exercise different presence rows (no trivial all-same bitmap).
        let a = &detectors[(batch_seed + r) % detectors.len()];
        let b = &detectors[(batch_seed * 7 + r * 3 + 1) % detectors.len()];
        haystack.extend_from_slice(b"prefix ");
        haystack.extend_from_slice(a);
        haystack.extend_from_slice(b" middle ");
        haystack.extend_from_slice(b);
        haystack.extend_from_slice(b" suffix\n");
    }
    (haystack, region_starts)
}

/// PERF (opt-in via `--ignored`): the resident pipeline must be FASTER end-to-end
/// than the borrowed `scan_presence_by_region` across a multi-batch corpus, because
/// it uploads the multi-MiB DFA tables (and builds the program) ONCE instead of on
/// every batch. Correctness is the hard gate on every batch; the timing is sized so
/// the table-residency win dominates GPU noise. Prints the measured speedup.
///
/// Run:
///   cargo test -p vyre-libs --test literal_set_resident_presence --release \
///     -- --ignored --nocapture resident_presence_throughput
#[test]
#[ignore = "perf measurement; needs a GPU and runs a timed multi-batch loop"]
fn resident_presence_throughput_beats_borrowed_across_a_corpus_gpu() {
    let backend = match WgpuBackend::shared() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("no wgpu backend ({e}); skipping resident throughput test");
            return;
        }
    };
    // A large detector set => a large DFA transition table: exactly the multi-MiB
    // per-scan re-upload the resident path eliminates.
    let detectors: Vec<Vec<u8>> = (0..400)
        .map(|i| format!("detector_token_{i:04}").into_bytes())
        .collect();
    let detector_refs: Vec<&[u8]> = detectors.iter().map(Vec::as_slice).collect();
    let matcher = GpuLiteralSet::compile(&detector_refs);

    // 32 coalesced batches of 24 regions each (heterogeneous content per batch).
    const BATCHES: usize = 32;
    const REGIONS: usize = 24;
    const ITERS: usize = 6;
    let corpus: Vec<(Vec<u8>, Vec<u32>)> = (0..BATCHES)
        .map(|seed| synth_batch(&detectors, REGIONS, seed))
        .collect();
    let cap_bytes = corpus.iter().map(|(h, _)| h.len()).max().unwrap() + 64;
    let max_regions = corpus.iter().map(|(_, rs)| rs.len()).max().unwrap() as u32;

    let session = matcher
        .prepare_resident_presence(backend.as_ref(), cap_bytes, max_regions)
        .expect("prepare resident throughput session");

    // Correctness gate (hard): every batch's resident bitmap equals the borrowed one.
    let mut out = Vec::new();
    let mut scratch = Vec::new();
    for (h, rs) in &corpus {
        let borrowed = matcher
            .scan_presence_by_region(backend.as_ref(), h, rs)
            .expect("borrowed scan");
        session
            .scan_into(backend.as_ref(), h, rs, 0, &mut out, &mut scratch)
            .expect("resident scan");
        assert_eq!(
            out, borrowed,
            "resident bitmap must equal borrowed for every batch before timing"
        );
    }

    // Warm up both paths (compile caches, device queues) before timing.
    for (h, rs) in &corpus {
        let _ = matcher.scan_presence_by_region(backend.as_ref(), h, rs);
        let _ = session.scan_into(backend.as_ref(), h, rs, 0, &mut out, &mut scratch);
    }

    // Timed: borrowed re-uploads tables + rebuilds the program every batch.
    let t_borrowed = Instant::now();
    for _ in 0..ITERS {
        for (h, rs) in &corpus {
            matcher
                .scan_presence_by_region(backend.as_ref(), h, rs)
                .expect("timed borrowed scan");
        }
    }
    let borrowed_time = t_borrowed.elapsed();

    // Timed: resident transfers only the haystack + presence reset per batch.
    let t_resident = Instant::now();
    for _ in 0..ITERS {
        for (h, rs) in &corpus {
            session
                .scan_into(backend.as_ref(), h, rs, 0, &mut out, &mut scratch)
                .expect("timed resident scan");
        }
    }
    let resident_time = t_resident.elapsed();

    let scans = BATCHES * ITERS;
    let speedup = borrowed_time.as_secs_f64() / resident_time.as_secs_f64();
    eprintln!(
        "resident region-presence throughput ({} detectors, {scans} scans): \
         borrowed {borrowed_time:?} vs resident {resident_time:?} = {speedup:.2}x faster",
        detectors.len()
    );

    assert!(
        resident_time < borrowed_time,
        "resident table-residency must beat re-uploading the DFA + rebuilding the program every batch \
         (borrowed {borrowed_time:?}, resident {resident_time:?})"
    );
    session.free(backend.as_ref()).expect("free session");
}

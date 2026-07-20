//! Proof for the ASYNC region-presence scan method
//! (`GpuLiteralSet::scan_presence_by_region_async`).
//!
//! The async entry submits the dispatch and hands back a
//! `PendingPresenceByRegion` so a caller can overlap host work with the
//! in-flight scan, then decode via `await_words`. Two tests pin it:
//!
//!   * `async_region_presence_matches_sync_and_planted_hits_on_gpu` — REAL GPU
//!     (wgpu, the RTX 5090 here): the async bitmap equals
//!     `scan_presence_by_region`'s word-for-word AND the decoded per-region bit
//!     SETS are exactly the planted hits. Skips cleanly with no GPU.
//!
//!   * `async_region_presence_equals_sync_on_cpu_reference` — CPU reference
//!     backend, runs EVERYWHERE (incl. GPU-less CI, where the GPU test skips).
//!     On a backend that does not pipeline host/device work, `dispatch_async`
//!     uses the synchronous default and the handle is trivially ready: this
//!     asserts `is_ready()` is `true`, the async words equal the sync words
//!     (NO silent change of result on the degraded path — Law 10), and the same
//!     exact planted bit sets.
//!
//! Run:
//!   cargo test -p vyre-libs --test literal_set_presence_by_region_async --release -- --nocapture

mod presence_corpus;

use presence_corpus::{assert_planted_bits, planted_corpus, LITERALS};
use vyre::VyreBackend;
use vyre_driver_reference::CpuRefBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::scan::GpuLiteralSet;

#[test]
fn async_region_presence_matches_sync_and_planted_hits_on_gpu() {
    let backend = match WgpuBackend::shared() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("no wgpu backend ({e}); skipping async region-presence GPU test");
            return;
        }
    };
    let matcher = GpuLiteralSet::compile(LITERALS);
    let pattern_count = LITERALS.len() as u32;
    let words = pattern_count.div_ceil(32).max(1) as usize;
    let (haystack, region_starts) = planted_corpus();
    let region_count = region_starts.len();

    let sync_words = matcher
        .scan_presence_by_region(backend.as_ref(), &haystack, &region_starts)
        .expect("sync gpu presence-by-region scan");

    let pending = matcher
        .scan_presence_by_region_async(backend.as_ref(), &haystack, &region_starts, 0)
        .expect("async gpu presence-by-region submit");
    // Exercise the non-blocking probe (value is backend/timing-dependent on a
    // pipelining backend, so we only require it be callable without blocking).
    let _ready_before_await = pending.is_ready();
    let async_words = pending.await_words().expect("async gpu presence-by-region await");

    // (1) Async bitmap == sync bitmap, WORD FOR WORD.
    assert_eq!(
        async_words, sync_words,
        "async region-presence bitmap must equal scan_presence_by_region word-for-word"
    );
    // (2) Full bitmap shape.
    assert_eq!(
        async_words.len(),
        region_count * words,
        "async bitmap must be region_count ({region_count}) * words ({words}) u32s"
    );
    // (3) Exact planted bit sets per region.
    assert_planted_bits(words, pattern_count, &async_words);
}

#[test]
fn async_region_presence_equals_sync_on_cpu_reference() {
    // CPU reference backend: no GPU, runs everywhere. dispatch_async here uses the
    // synchronous default (the backend does not pipeline host/device work), so the
    // pending handle is trivially ready and MUST yield the same bitmap as the sync
    // entry — the degraded path changes nothing about the result (Law 10).
    let matcher = GpuLiteralSet::compile(LITERALS);
    let pattern_count = LITERALS.len() as u32;
    let words = pattern_count.div_ceil(32).max(1) as usize;
    let (haystack, region_starts) = planted_corpus();
    let region_count = region_starts.len();

    let sync_words = matcher
        .scan_presence_by_region(&CpuRefBackend, &haystack, &region_starts)
        .expect("sync cpu-reference presence-by-region scan");

    let pending = matcher
        .scan_presence_by_region_async(&CpuRefBackend, &haystack, &region_starts, 0)
        .expect("async cpu-reference presence-by-region submit");
    // Non-pipelining backend -> trivially-ready handle: is_ready is deterministically true.
    assert!(
        pending.is_ready(),
        "a non-pipelining backend must return a trivially-ready handle (is_ready == true)"
    );
    let async_words = pending.await_words().expect("async cpu-reference await");

    assert_eq!(
        async_words, sync_words,
        "async region-presence on the CPU reference backend must equal the sync result \
         word-for-word — the non-overlapping path must not change the bitmap"
    );
    assert_eq!(
        async_words.len(),
        region_count * words,
        "async bitmap must be region_count ({region_count}) * words ({words}) u32s"
    );
    assert_planted_bits(words, pattern_count, &async_words);
}

#[test]
fn prepared_region_presence_payload_reproduces_sync_scan_on_cpu_reference() {
    // The RESIDENT prepared payload: a resident runtime uploads `inputs` once and
    // re-dispatches across a corpus; a direct caller dispatches the borrowed inputs
    // through the backend and decodes binding 0. Prove the payload reproduces
    // scan_presence_by_region exactly (CPU reference, runs everywhere).
    let matcher = GpuLiteralSet::compile(LITERALS);
    let pattern_count = LITERALS.len() as u32;
    let words = pattern_count.div_ceil(32).max(1) as usize;
    let (haystack, region_starts) = planted_corpus();
    let region_count = region_starts.len();

    let sync_words = matcher
        .scan_presence_by_region(&CpuRefBackend, &haystack, &region_starts)
        .expect("sync cpu-reference presence-by-region scan");

    let prepared = matcher
        .prepare_presence_by_region_dispatch(&haystack, &region_starts, 0)
        .expect("prepare region-presence dispatch payload");
    // Payload shape assertions (real values, not just non-empty).
    assert_eq!(prepared.region_count as usize, region_count);
    assert_eq!(prepared.total_words, region_count * words);
    assert_eq!(prepared.presence_output_bytes, region_count * words * 4);
    assert_eq!(prepared.inputs.len(), 12, "region-presence has 12 bindings");
    assert!(
        prepared.encoded_input_bytes > 0,
        "encoded input byte total must be the sum of all 12 buffers"
    );

    // Dispatch the prepared inputs exactly as a resident runtime would on its
    // first upload, then decode binding 0.
    let borrowed: Vec<&[u8]> = prepared.inputs.iter().map(Vec::as_slice).collect();
    let outputs = CpuRefBackend
        .dispatch_borrowed(&prepared.program, &borrowed, &prepared.dispatch_config)
        .expect("dispatch prepared region-presence payload");
    let prepared_words = prepared
        .decode_presence(&outputs)
        .expect("decode prepared region-presence bitmap");

    assert_eq!(
        prepared_words, sync_words,
        "prepared region-presence payload must reproduce scan_presence_by_region word-for-word"
    );
    assert_planted_bits(words, pattern_count, &prepared_words);
}

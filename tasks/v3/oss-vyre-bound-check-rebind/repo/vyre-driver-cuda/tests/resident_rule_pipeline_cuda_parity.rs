//! CUDA backend parity for the `vyre_libs` resident NFA mega-scan pipeline
//! (`RulePipeline::prepare_resident` → `ResidentRulePipeline`).
//!
//! Like the resident region-presence pipeline, `ResidentRulePipeline` reaches the
//! backend only through the `VyreBackend` trait's resident half. The CUDA backend's
//! resident dispatch rejects any borrowed resource (it resolves every binding to a
//! resident handle), so a resident dispatch must be ALL-resident. The pipeline used
//! to bind its two 1-u32 control buffers (haystack_len, max_scan_bytes) as borrowed,
//! which works on wgpu but fails closed on CUDA — meaning the all-resident fix needs
//! a CUDA proof, not just a wgpu/mock one. This test drives the real pipeline on
//! CudaBackend and asserts the resident match set is byte-identical to the borrowed
//! `RulePipeline::scan` across repeated re-dispatches.
//!
//! Skips cleanly when no CUDA device is present.
//!
//! Run:
//!   cargo test -p vyre-driver-cuda --test resident_rule_pipeline_cuda_parity --release -- --nocapture

use vyre_driver_cuda::{CudaBackend, CudaBackendRegistration};
use vyre_foundation::match_result::Match;
use vyre_libs::scan::build_rule_pipeline;

/// Sort matches into a deterministic order so the expected-set assertion is
/// independent of the kernel's per-workgroup emission order.
fn sorted(mut matches: Vec<Match>) -> Vec<Match> {
    matches.sort_by_key(|m| (m.start, m.end, m.pattern_id));
    matches
}

#[test]
fn resident_rule_pipeline_matches_borrowed_on_cuda() {
    let backend = match CudaBackend::acquire() {
        Ok(b) => CudaBackendRegistration::new(b),
        Err(e) => {
            eprintln!("no CUDA backend ({e}); skipping resident RulePipeline CUDA parity test");
            return;
        }
    };

    // Patterns ab=0, cd=1, xyz=2. Haystack plants known, overlap-free hits:
    // "ab"@[2,4), "cd"@[6,8), "xyz"@[8,11), "ab"@[11,13). The NFA program declares a
    // STATIC input buffer of `input_len` bytes (the CUDA backend enforces it, unlike
    // wgpu), so the haystack length, `build`'s input_len, and `prepare_resident`'s
    // capacity must all agree — 16 here (a multiple of 4 so the packed length equals
    // the raw length). The trailing "www" adds no match.
    const HAYSTACK_LEN: u32 = 16;
    let pipeline = build_rule_pipeline(&["ab", "cd", "xyz"], "input", "hits", HAYSTACK_LEN);
    let haystack = b"zzabqqcdxyzabwww";
    assert_eq!(haystack.len() as u32, HAYSTACK_LEN);

    // The NFA program statically declares the hits buffer for nfa::NUM_HIT_SLOTS
    // (10000 matches); CUDA enforces that static size, so scan + prepare_resident
    // must use this exact match cap (wgpu would accept any).
    const HIT_CAP: u32 = 10_000;

    // Ground truth on CUDA: the borrowed mega-scan.
    let borrowed = sorted(
        pipeline
            .scan(&backend, haystack, HIT_CAP)
            .expect("borrowed CUDA mega-scan"),
    );
    // Real-value gate: the borrowed scan must find exactly the planted hits, so the
    // parity comparison below is not vacuously comparing two empty/wrong results.
    assert_eq!(
        borrowed,
        vec![
            Match::new(0, 2, 4),
            Match::new(1, 6, 8),
            Match::new(2, 8, 11),
            Match::new(0, 11, 13),
        ],
        "borrowed CUDA scan must find exactly the planted ab/cd/xyz/ab hits"
    );

    // prepare_resident's haystack capacity must match the program's static input
    // declaration (HAYSTACK_LEN) for the CUDA backend's resident dispatch.
    let session = pipeline
        .prepare_resident(&backend, HAYSTACK_LEN as usize, HIT_CAP)
        .expect("prepare resident RulePipeline session on CUDA");

    // Re-dispatch several times: the NFA tables stay resident (uploaded once), and
    // every CUDA scan must reproduce the borrowed match set — proving the trait's
    // resident half (incl. the now-resident control buffers) is wired on CUDA.
    let mut matches = Vec::new();
    let mut scratch = Vec::new();
    for iter in 0..4 {
        session
            .scan_into(&backend, haystack, &mut matches, &mut scratch)
            .expect("resident CUDA mega-scan");
        assert_eq!(
            sorted(matches.clone()),
            borrowed,
            "iteration {iter}: CUDA resident match set must equal RulePipeline::scan"
        );
    }

    session
        .free(&backend)
        .expect("free resident RulePipeline session on CUDA");
}

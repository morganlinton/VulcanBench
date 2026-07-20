//! Single-file profiling driver that runs `parse_translation_unit` on one
//! corpus file with `VYRE_STAGE_TRACE` enabled so each stage-trace line
//! prints to stderr. Used to identify which dispatch stage in the GPU
//! parser pipeline dominates wall time on a system-include-bearing TU.
//!
//! `#[ignore]` because (a) it depends on the host C compiler for the
//! P1 system-include probe and (b) it's a profiling tool, not a
//! correctness gate. Run on demand:
//!
//! ```sh
//! cargo test -p vyre-frontend-c --release --test r2_corpus_single_stage_trace \
//!     -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::{parse_translation_unit, VyreCompileOptions};

#[test]
#[ignore = "single-file profiling; relies on caller-set VYRE_STAGE_TRACE and host C compiler"]
fn parse_one_corpus_file_with_stage_trace() {
    if std::env::var("VYRE_STAGE_TRACE").is_err() {
        eprintln!(
            "warning: VYRE_STAGE_TRACE not set; per-stage timings will not print. \
             Re-run with `VYRE_STAGE_TRACE=1 cargo test ... -- --ignored --nocapture` \
             to capture the parser stage breakdown."
        );
    }

    // mod/empty.c  -  known-passing baseline. Use this to capture the
    // floor cost of a parse with zero #include expansion.
    let corpus_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/r2_kernel_scripts");
    let baseline = corpus_root.join("mod/empty.c");
    eprintln!();
    eprintln!("=== profiling baseline: mod/empty.c ===");
    let baseline_options = VyreCompileOptions::default();
    let started = Instant::now();
    let baseline_result = parse_translation_unit(&baseline, &baseline_options);
    eprintln!("baseline elapsed: {:.3}s", started.elapsed().as_secs_f64());
    match baseline_result {
        Ok(summary) => eprintln!(
            "baseline OK source_bytes={} token_count={} ast_bytes={}",
            summary.source_bytes, summary.token_count, summary.ast_bytes
        ),
        Err(e) => eprintln!("baseline FAIL: {e}"),
    }

    eprintln!();
    eprintln!("=== profiling system-include TU: mod/mk_elfconfig.c ===");
    let mk_elfconfig = corpus_root.join("mod/mk_elfconfig.c");
    let options = VyreCompileOptions {
        include_dirs: vec![
            PathBuf::from("/usr/include"),
            PathBuf::from("/usr/include/x86_64-linux-gnu"),
        ],
        ..Default::default()
    };
    let started = Instant::now();
    let result = parse_translation_unit(&mk_elfconfig, &options);
    eprintln!(
        "system-include TU elapsed: {:.3}s",
        started.elapsed().as_secs_f64()
    );
    match result {
        Ok(summary) => eprintln!(
            "system-include TU OK source_bytes={} token_count={} ast_bytes={}",
            summary.source_bytes, summary.token_count, summary.ast_bytes
        ),
        Err(e) => eprintln!("system-include TU FAIL: {e}"),
    }
}

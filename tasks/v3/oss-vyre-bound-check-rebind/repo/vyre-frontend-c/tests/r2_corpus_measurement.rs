//! Real Linux kernel `scripts/` corpus pass-rate measurement on tip.
//!
//! Walks `tests/corpus/r2_kernel_scripts/` and runs the full
//! `parse_translation_unit` pipeline on each `.c` file. Reports pass/fail
//! per file and prints a markdown summary table to stdout.
//!
//! Marked `#[ignore]` because it is an on-demand corpus/performance gate.
//! The harness passes explicit fixture and system include roots; it must not
//! shell out to gcc/clang to discover host defaults. Run on demand:
//!
//! ```sh
//! cargo test -p vyre-frontend-c --test r2_corpus_measurement \
//!     -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[path = "r2_corpus_measurement/support.rs"]
mod r2_support;
#[path = "r2_corpus_measurement/throughput.rs"]
mod throughput;

use r2_support::{
    chrono_like_now, classify_error, collect_corpus, kernel_scripts_compile_options,
    run_file_in_subprocess, run_single_file_and_exit, CORPUS_ROOT, PER_FILE_TIMEOUT,
    SINGLE_FILE_ENV, SKIP_LOCAL_INCLUDE_HEADERS, SKIP_SYSTEM_INCLUDE_HEADERS,
};

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;

#[test]
#[ignore = "real-corpus measurement; uses explicit fixture/system include roots"]
fn r2_kernel_scripts_pass_rate() {
    // Single-file worker mode: parse one file and exit. The driver path
    // re-execs us with this env var set so each file gets its own
    // process / CUDA context.
    if let Ok(single) = std::env::var(SINGLE_FILE_ENV) {
        let path = PathBuf::from(single);
        run_single_file_and_exit(&path);
    }

    let corpus_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS_ROOT);
    let files = collect_corpus(&corpus_root);
    assert_ne!(
        files.len(),
        0,
        "Fix: Linux kernel scripts/ corpus must contain at least one .c file under {}",
        corpus_root.display()
    );

    let _options = kernel_scripts_compile_options();

    let mut passes = 0usize;
    let mut fails: Vec<(PathBuf, String)> = Vec::new();
    let mut skipped: Vec<(PathBuf, u64)> = Vec::new();
    let started = Instant::now();

    for (idx, file) in files.iter().enumerate() {
        let metadata = match std::fs::metadata(file) {
            Ok(m) => m,
            Err(e) => {
                fails.push((file.clone(), format!("stat: {e}")));
                continue;
            }
        };
        if let Ok(source) = std::fs::read_to_string(file) {
            if let Some(header) = SKIP_SYSTEM_INCLUDE_HEADERS
                .iter()
                .find(|h| source.contains(&format!("#include <{h}>")))
            {
                skipped.push((file.clone(), metadata.len()));
                eprintln!(
                    "[{}/{}] SKIP {} (#include <{}> in pipeline-cost-cap list)",
                    idx + 1,
                    files.len(),
                    file.strip_prefix(&corpus_root).unwrap_or(file).display(),
                    header
                );
                continue;
            }
            if let Some(header) = SKIP_LOCAL_INCLUDE_HEADERS
                .iter()
                .find(|h| source.contains(&format!("#include \"{h}\"")))
            {
                skipped.push((file.clone(), metadata.len()));
                eprintln!(
                    "[{}/{}] SKIP {} (#include \"{}\" in pipeline-cost-cap list)",
                    idx + 1,
                    files.len(),
                    file.strip_prefix(&corpus_root).unwrap_or(file).display(),
                    header
                );
                continue;
            }
        }
        let file_started = Instant::now();
        let outcome = run_file_in_subprocess(file, PER_FILE_TIMEOUT);
        let elapsed = file_started.elapsed().as_millis();
        match outcome {
            Ok(_) => {
                passes += 1;
                eprintln!(
                    "[{}/{}] OK   {} ({} ms)",
                    idx + 1,
                    files.len(),
                    file.strip_prefix(&corpus_root).unwrap_or(file).display(),
                    elapsed
                );
            }
            Err(message) => {
                let cluster = classify_error(&message);
                eprintln!(
                    "[{}/{}] FAIL {} ({} ms) [{}]",
                    idx + 1,
                    files.len(),
                    file.strip_prefix(&corpus_root).unwrap_or(file).display(),
                    elapsed,
                    cluster
                );
                fails.push((file.clone(), message));
            }
        }
    }

    let total_attempted = files.len() - skipped.len();
    let mut clusters: BTreeMap<String, (usize, PathBuf)> = BTreeMap::new();
    for (path, message) in &fails {
        let key = classify_error(message);
        let entry = clusters
            .entry(key)
            .or_insert_with(|| (0usize, path.clone()));
        entry.0 += 1;
    }

    let elapsed = started.elapsed();

    // Also write the report to a known path so we recover it even if
    // the test runner truncates stdout or the harness times out mid-loop.
    let report_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS_ROOT)
        .join("REPORT_TIP.md");
    let mut report = String::new();
    use std::fmt::Write as _;
    let _ = writeln!(
        report,
        "# r2_kernel_scripts_pass_rate (vyre-frontend-c on tip, {})",
        chrono_like_now()
    );
    let _ = writeln!(report, "\n- corpus root: {}", corpus_root.display());
    let _ = writeln!(report, "- total files: {}", files.len());
    let _ = writeln!(
        report,
        "- skipped by explicit header exemption: {}",
        skipped.len()
    );
    let _ = writeln!(report, "- attempted: {total_attempted}");
    let _ = writeln!(report, "- passed: {passes}");
    let _ = writeln!(report, "- failed: {}", fails.len());
    let _ = writeln!(report, "- elapsed: {:.2}s", elapsed.as_secs_f64());
    if !clusters.is_empty() {
        let _ = writeln!(report, "\n## Failure clusters\n\n| Count | Cluster |");
        let _ = writeln!(report, "|-------|---------|");
        for (cluster, (count, _)) in &clusters {
            let _ = writeln!(report, "| {count} | {cluster} |");
        }
    }
    if let Err(e) = std::fs::write(&report_path, &report) {
        eprintln!("warning: could not write {}: {e}", report_path.display());
    } else {
        eprintln!("report written: {}", report_path.display());
    }

    println!();
    println!("# r2_kernel_scripts_pass_rate (vyre-frontend-c on tip)");
    println!();
    println!("- corpus root: {}", corpus_root.display());
    println!("- total files: {}", files.len());
    println!("- skipped by explicit header exemption: {}", skipped.len());
    println!("- attempted: {total_attempted}");
    println!("- passed: {passes}");
    println!("- failed: {}", fails.len());
    println!("- elapsed: {:.2}s", elapsed.as_secs_f64());
    println!();

    if !skipped.is_empty() {
        println!("## SKIPPED (explicit header exemption)");
        println!();
        println!("| File | Size |");
        println!("|------|------|");
        for (path, size) in &skipped {
            let rel = path.strip_prefix(&corpus_root).unwrap_or(path).display();
            println!("| `{rel}` | {size} bytes |");
        }
        println!();
    }

    if !clusters.is_empty() {
        println!("## Failure clusters");
        println!();
        println!("| Count | Cluster | Example file |");
        println!("|-------|---------|--------------|");
        for (cluster, (count, example)) in &clusters {
            let rel = example
                .strip_prefix(&corpus_root)
                .unwrap_or(example)
                .display();
            println!("| {count} | {cluster} | `{rel}` |");
        }
        println!();
    }

    if passes > 0 {
        println!("## Passing files");
        println!();
        for file in &files {
            if !fails.iter().any(|(p, _)| p == file) {
                let rel = file.strip_prefix(&corpus_root).unwrap_or(file).display();
                println!("- `{rel}`");
            }
        }
        println!();
    }

    println!("## All failures (first 1KB of each error)");
    println!();
    for (path, message) in &fails {
        let rel = path.strip_prefix(&corpus_root).unwrap_or(path).display();
        let truncated = if message.len() > 1024 {
            &message[..1024]
        } else {
            message.as_str()
        };
        println!("### `{rel}`");
        println!();
        println!("```");
        println!("{truncated}");
        println!("```");
        println!();
    }

    if passes == 0 {
        panic!(
            "vyre-frontend-c parsed 0 of {total_attempted} attempted Linux kernel scripts/ files. \
             Fix: investigate the most common failure cluster above and re-wire the parser path that's missing."
        );
    }
}

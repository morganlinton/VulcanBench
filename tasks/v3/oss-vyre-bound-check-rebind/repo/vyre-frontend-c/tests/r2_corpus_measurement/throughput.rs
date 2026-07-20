use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::r2_support::{
    chrono_like_now, classify_error, collect_corpus, kernel_scripts_compile_options, CORPUS_ROOT,
};
use vyre_frontend_c::api::parse_translation_unit;

#[test]
#[ignore = "throughput measurement on synthetic safe corpus"]

fn r2_synthetic_throughput_files_per_ms() {
    use vyre_frontend_c::api::parse_syntax_batch_bytes;

    // Generate a corpus of small, fast-path-safe synthetic C source files.
    // Each file is ~2 KB  -  closer to the kernel-scripts average (~10 KB)
    // while staying inside the 8 MB batch ceiling at 4096 files.
    let mut sources: Vec<Vec<u8>> = Vec::new();
    let n_files: usize = std::env::var("THROUGHPUT_N_FILES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8192);
    let n_funcs_per_file: usize = std::env::var("THROUGHPUT_N_FUNCS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    for i in 0..n_files {
        let mut s = String::with_capacity(n_funcs_per_file * 80);
        for j in 0..n_funcs_per_file {
            s.push_str(&format!(
                "int func_{i}_{j}(int a, int b) {{ return (a + b) * {j} + {i}; }}\n"
            ));
        }
        s.push_str(&format!(
            "int helper_{i}(void) {{ return func_{i}_0(1, 2); }}\n"
        ));
        sources.push(s.into_bytes());
    }
    let total_bytes: u64 = sources.iter().map(|s| s.len() as u64).sum();
    eprintln!(
        "[throughput] generated {} files ({:.2} KB) at {}",
        sources.len(),
        total_bytes as f64 / 1024.0,
        chrono_like_now(),
    );

    // Warm pipeline cache.
    if let Some(first) = sources.first() {
        let warm = parse_syntax_batch_bytes(&[first.as_slice()]);
        eprintln!(
            "[throughput] warmup parse: {}",
            warm.as_ref()
                .map(|s| format!("{} files, {} tokens", s.file_count, s.token_count))
                .unwrap_or_else(|e| format!("FAIL {e}"))
        );
    }
    // Second warmup with full batch shape  -  first dispatch on this batch
    // size pays the per-shape cold compile.
    let refs: Vec<&[u8]> = sources.iter().map(Vec::as_slice).collect();
    let warm2 = parse_syntax_batch_bytes(&refs);
    eprintln!(
        "[throughput] full-batch warmup: {}",
        warm2
            .as_ref()
            .map(|s| format!("{} files, {} tokens", s.file_count, s.token_count))
            .unwrap_or_else(|e| format!("FAIL {e}"))
    );

    let started = Instant::now();
    let summary = parse_syntax_batch_bytes(&refs).expect("batch parse must succeed");
    let elapsed = started.elapsed();

    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    let files_per_ms = sources.len() as f64 / elapsed_ms.max(1e-9);
    let mb_per_sec = total_bytes as f64 / 1024.0 / 1024.0 / elapsed.as_secs_f64().max(1e-9);

    eprintln!("");
    eprintln!("[throughput] === RESULTS ===");
    eprintln!("[throughput] backend:        {}", summary.backend_id);
    eprintln!("[throughput] files:          {}", summary.file_count);
    eprintln!("[throughput] source bytes:   {}", summary.source_bytes);
    eprintln!("[throughput] tokens:         {}", summary.token_count);
    eprintln!("[throughput] ast nodes:      {}", summary.ast_node_count);
    eprintln!("[throughput] elapsed:        {:.3} ms", elapsed_ms);
    eprintln!("[throughput] files/ms:       {:.2}", files_per_ms);
    eprintln!("[throughput] MB/s:           {:.2}", mb_per_sec);
    eprintln!(
        "[throughput] target:         100 files/ms (currently {:.2}x of target)",
        files_per_ms / 100.0
    );
}

#[test]
#[ignore = "single-file timing for parse_translation_unit warm/cold profile"]
fn r2_single_file_warm_cold_timing() {
    let target = std::env::var("R2_TIMING_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(CORPUS_ROOT)
                .join("mod/empty.c")
        });
    eprintln!("[timing] target: {}", target.display());
    let warmups: usize = std::env::var("R2_TIMING_WARMUPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let trials: usize = std::env::var("R2_TIMING_TRIALS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let options = kernel_scripts_compile_options();
    for w in 0..warmups {
        let t = Instant::now();
        let r = parse_translation_unit(&target, &options);
        eprintln!(
            "[timing] warmup {w}: {} ms  -  {}",
            t.elapsed().as_millis(),
            r.map(|_| "ok".to_string())
                .unwrap_or_else(|e| format!("ERR {}", &e[..e.len().min(180)]))
        );
    }
    let mut elapsed_us: Vec<u128> = Vec::new();
    for trial in 0..trials {
        let t = Instant::now();
        let r = parse_translation_unit(&target, &options);
        let us = t.elapsed().as_micros();
        elapsed_us.push(us);
        eprintln!(
            "[timing] trial  {trial}: {us} us  ({} ms)  -  {}",
            us / 1000,
            r.map(|_| "ok".to_string())
                .unwrap_or_else(|e| format!("ERR {}", &e[..e.len().min(180)]))
        );
    }
    elapsed_us.sort_unstable();
    let median = elapsed_us[elapsed_us.len() / 2];
    let min = elapsed_us[0];
    let max = elapsed_us[elapsed_us.len() - 1];
    eprintln!("");
    eprintln!("[timing] === RESULTS ===");
    eprintln!("[timing] file:    {}", target.display());
    eprintln!("[timing] trials:  {}", trials);
    eprintln!("[timing] min:     {} us  ({} ms)", min, min / 1000);
    eprintln!("[timing] median:  {} us  ({} ms)", median, median / 1000);
    eprintln!("[timing] max:     {} us  ({} ms)", max, max / 1000);
}

#[test]
#[ignore = "real corpus per-file warm cost  -  bypasses summary cache by parsing each file once after pre-warming the per-file caches"]
fn r2_kernel_scripts_per_file_warm_throughput() {
    let corpus_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS_ROOT);
    let files = collect_corpus(&corpus_root);
    assert_ne!(files.len(), 0, "corpus must contain .c files");

    let options = kernel_scripts_compile_options();

    // Pre-warm: parse each file once to populate every cache layer.
    eprintln!("[corpus-throughput] prewarming {} files…", files.len());
    let prewarm_start = Instant::now();
    let mut warmed: Vec<&PathBuf> = Vec::new();
    let mut warm_fails: Vec<(PathBuf, String)> = Vec::new();
    for f in &files {
        match parse_translation_unit(f, &options) {
            Ok(_) => warmed.push(f),
            Err(e) => warm_fails.push((f.clone(), e)),
        }
    }
    eprintln!(
        "[corpus-throughput] prewarm: {} ok, {} fail, {:.2}s",
        warmed.len(),
        warm_fails.len(),
        prewarm_start.elapsed().as_secs_f64()
    );
    if std::env::var_os("R2_PRINT_FAILURES").is_some() {
        let mut clusters: BTreeMap<String, usize> = BTreeMap::new();
        for (path, message) in &warm_fails {
            let rel = path.strip_prefix(&corpus_root).unwrap_or(path).display();
            let cluster = classify_error(message);
            *clusters.entry(cluster.clone()).or_insert(0) += 1;
            eprintln!(
                "[corpus-throughput] FAIL {} [{}] :: {}",
                rel,
                cluster,
                &message[..message.len().min(220)].replace('\n', " ")
            );
        }
        eprintln!("[corpus-throughput] failure clusters:");
        for (cluster, count) in &clusters {
            eprintln!("[corpus-throughput]   {count} × {cluster}");
        }
    }

    // Measured run: parse each warmed file again  -  every cache layer hits.
    let measured_start = Instant::now();
    let mut total_ms = 0u128;
    let mut per_file_us: Vec<u128> = Vec::with_capacity(warmed.len());
    for f in &warmed {
        let t = Instant::now();
        let _ = parse_translation_unit(f, &options);
        let us = t.elapsed().as_micros();
        per_file_us.push(us);
        total_ms = total_ms.saturating_add(us / 1000);
    }
    let elapsed = measured_start.elapsed();
    per_file_us.sort_unstable();
    let median = per_file_us.get(per_file_us.len() / 2).copied().unwrap_or(0);
    let mean: u128 = per_file_us.iter().sum::<u128>() / per_file_us.len().max(1) as u128;
    let max = per_file_us.last().copied().unwrap_or(0);
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    let files_per_sec = warmed.len() as f64 / elapsed.as_secs_f64().max(1e-9);

    eprintln!("");
    eprintln!("[corpus-throughput] === WARM-PATH RESULTS ===");
    eprintln!("[corpus-throughput] files measured: {}", warmed.len());
    eprintln!("[corpus-throughput] elapsed total:  {:.2} ms", elapsed_ms);
    eprintln!("[corpus-throughput] mean per file:  {} us", mean);
    eprintln!("[corpus-throughput] median:         {} us", median);
    eprintln!("[corpus-throughput] max:            {} us", max);
    eprintln!("[corpus-throughput] files/sec:      {:.0}", files_per_sec);
    eprintln!(
        "[corpus-throughput] vs ~580 ms baseline: {:.1}x speedup",
        (580_000.0_f64) / mean.max(1) as f64
    );
}

#[test]
#[ignore = "cold-per-file: warm GPU pipeline cache, then measure each new source"]
fn r2_kernel_scripts_cold_per_file_throughput() {
    let corpus_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS_ROOT);
    let files = collect_corpus(&corpus_root);
    assert_ne!(files.len(), 0, "corpus must contain .c files");

    let options = kernel_scripts_compile_options();

    // Find the smallest passing file to warm the GPU pipeline cache. Parse it
    // twice so the first cold-compile cost is amortised.
    let mut warm_target: Option<&PathBuf> = None;
    for f in &files {
        if parse_translation_unit(f, &options).is_ok() {
            warm_target = Some(f);
            break;
        }
    }
    let warm_target = warm_target.expect("need at least one passing file to warm");
    eprintln!(
        "[cold-per-file] warming pipeline cache with {}",
        warm_target.display()
    );
    let warm_started = Instant::now();
    for _ in 0..2 {
        let _ = parse_translation_unit(warm_target, &options);
    }
    eprintln!(
        "[cold-per-file] pipeline warmup: {:.2}s",
        warm_started.elapsed().as_secs_f64()
    );

    // Measured: parse every other corpus file once.
    let measured_start = Instant::now();
    let mut per_file_us: Vec<u128> = Vec::new();
    let mut ok = 0usize;
    let mut fail = 0usize;
    for f in &files {
        if std::path::Path::new(f) == std::path::Path::new(warm_target) {
            continue;
        }
        let t = Instant::now();
        let r = parse_translation_unit(f, &options);
        let us = t.elapsed().as_micros();
        if r.is_ok() {
            ok += 1;
            per_file_us.push(us);
        } else {
            fail += 1;
        }
    }
    let elapsed = measured_start.elapsed();
    per_file_us.sort_unstable();
    if per_file_us.is_empty() {
        eprintln!("[cold-per-file] no passing files measured");
        return;
    }
    let median = per_file_us[per_file_us.len() / 2];
    let mean = per_file_us.iter().sum::<u128>() / per_file_us.len() as u128;
    let max = per_file_us.last().copied().unwrap_or(0);
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;

    eprintln!("");
    eprintln!("[cold-per-file] === COLD-PATH RESULTS (pipeline warm, source cold) ===");
    eprintln!("[cold-per-file] files measured: {} ok, {} fail", ok, fail);
    eprintln!("[cold-per-file] elapsed total:  {:.2} ms", elapsed_ms);
    eprintln!(
        "[cold-per-file] mean per file:  {} us  ({} ms)",
        mean,
        mean / 1000
    );
    eprintln!(
        "[cold-per-file] median:         {} us  ({} ms)",
        median,
        median / 1000
    );
    eprintln!(
        "[cold-per-file] max:            {} us  ({} ms)",
        max,
        max / 1000
    );
    eprintln!(
        "[cold-per-file] vs ~580 ms baseline: {:.1}x speedup",
        (580_000.0_f64) / mean.max(1) as f64
    );
}

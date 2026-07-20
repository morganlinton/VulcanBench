//! Cross-backend comparison harness (C-B12).
//!
//! `cargo_full run --bin xtask -- bench-crossback [program_name]`
//!
//! Dispatches a built-in Program through every registered backend
//! and emits a markdown comparison table. Complements the
//! performance contract in `BENCHMARKS.md` (A-C14b)  -  ships the
//! measurement path. The table format is pinned so CI diffs are
//! meaningful.
//!
//! This xtask records CPU-reference oracle timing only. GPU release
//! performance evidence is produced by the dedicated CUDA/WGPU benchmark
//! suites, never by fabricated cross-backend numbers.

use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

/// Entry point called from `main.rs`.
pub(crate) fn run(args: &[String]) {
    let program_name = args.get(2).cloned().unwrap_or_else(|| "xor-1k".to_string());
    if std::env::var("VYRE_BENCH_GPU").ok().as_deref() == Some("1") {
        eprintln!(
            "Fix: bench-crossback no longer emits unmeasured GPU timing. Run the release CUDA/WGPU benchmark suites for real GPU evidence."
        );
        process::exit(1);
    }

    let programs: [(&str, usize); 2] = [("xor-1k", 1024usize), ("xor-1m", 1024 * 1024)];
    let matches: Vec<&(&str, usize)> = programs
        .iter()
        .filter(|(name, _)| *name == program_name.as_str() || program_name == "*")
        .collect();

    if matches.is_empty() {
        eprintln!(
            "Fix: unknown program name `{}`. Known programs: {}",
            program_name,
            programs
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(", ")
        );
        process::exit(1);
    }

    let mut rows: Vec<Row> = Vec::new();
    for (name, size) in matches {
        // CPU reference timing.
        let cpu_ms = time_cpu_ref_xor(*size);
        rows.push(Row {
            program: (*name).to_string(),
            wgpu_ms: None,
            spirv_ms: None,
            ptx_ms: None,
            metal_ms: None,
            cpu_ref_ms: format!("{cpu_ms:.3}"),
        });
    }

    let markdown = render_markdown(&rows);

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("perf");
    fs::create_dir_all(&dir).unwrap_or_else(|error| {
        eprintln!("Fix: could not create {}: {error}", dir.display());
        process::exit(1);
    });
    let path = dir.join(format!("cross-backend-{}.md", program_name));
    fs::write(&path, &markdown).unwrap_or_else(|e| {
        eprintln!("Fix: could not write {}: {e}", path.display());
        process::exit(1);
    });

    println!("{markdown}");
    println!("wrote: {}", path.display());
}

struct Row {
    program: String,
    wgpu_ms: Option<String>,
    spirv_ms: Option<String>,
    ptx_ms: Option<String>,
    metal_ms: Option<String>,
    cpu_ref_ms: String,
}

fn cell(opt: &Option<String>) -> &str {
    opt.as_deref().unwrap_or("n/a")
}

fn render_markdown(rows: &[Row]) -> String {
    let mut out = String::new();
    out.push_str("# cross-backend comparison\n\n");
    out.push_str(
        "Produced by `cargo_full run --bin xtask -- bench-crossback <program>`. ms\n\
         values are CPU-reference oracle wall-clock per call. GPU release\n\
         evidence comes from the dedicated CUDA/WGPU benchmark suites.\n\n",
    );
    out.push_str("| program | wgpu | spirv | secondary_text | native_module | cpu-ref |\n");
    out.push_str("|---------|------|-------|-----|-------|---------|\n");
    for row in rows {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            row.program,
            cell(&row.wgpu_ms),
            cell(&row.spirv_ms),
            cell(&row.ptx_ms),
            cell(&row.metal_ms),
            row.cpu_ref_ms,
        ));
    }
    out
}

fn time_cpu_ref_xor(size: usize) -> f64 {
    // Simple reference XOR-over-bytes. Returns ms per call
    // amortized over 100 iterations to reduce clock jitter.
    let input = vec![0u8; size];
    let mut output = vec![0u8; size];
    let iters = 100;
    let start = Instant::now();
    for _ in 0..iters {
        for i in 0..size {
            output[i] = input[i] ^ 0xA5;
        }
    }
    let elapsed = start.elapsed();
    (elapsed.as_secs_f64() * 1000.0) / (iters as f64)
}

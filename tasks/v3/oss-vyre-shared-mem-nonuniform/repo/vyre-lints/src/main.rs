//! `vyre-lints` CLI. Runs the lint suite against a workspace.
//!
//! Usage:
//!   vyre-lints --workspace-root . [--allowlist vyre-lints/allowlist.toml] [--format json|text]
//!
//! Exits 0 if no violations (after allowlist filter). Exits 1 if any
//! violation. Exit 2 on I/O / parse failure.

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use vyre_lints::{
    run_consumer_coupling, run_gpu_skip_guards, run_module_forks, run_production_cpu_fallbacks,
    run_raw_ir_in_libs, Violation,
};

#[derive(Parser, Debug)]
#[command(
    name = "vyre-lints",
    version,
    about = "Lego-block enforcement lints for vyre"
)]
struct Cli {
    /// Workspace root (the dir containing vyre-libs/, vyre-foundation/, ...).
    #[arg(long, default_value = ".")]
    workspace_root: PathBuf,

    /// Allowlist file. If omitted, defaults to <workspace_root>/vyre-lints/allowlist.toml.
    #[arg(long)]
    allowlist: Option<PathBuf>,

    /// Output format: text (default) or json.
    #[arg(long, default_value = "text")]
    format: Format,

    /// Override the lib roots scanned. Defaults to vyre-libs/src.
    #[arg(long)]
    lib_root: Option<PathBuf>,

    /// Run the allowlist drift sentinel: fail if any allowlist entry
    /// is older than `--drift-budget-days` (default 14). Skips the
    /// raw-IR scan when set.
    #[arg(long)]
    check_drift: bool,

    /// Age budget for the drift sentinel, in days.
    #[arg(long, default_value_t = vyre_lints::drift::DEFAULT_AGE_BUDGET_DAYS)]
    drift_budget_days: i64,

    /// Today's date in YYYY-MM-DD form. Defaults to the OS clock.
    #[arg(long)]
    today: Option<String>,

    /// Run the production CPU fallback guard instead of the raw-IR lint.
    #[arg(long)]
    check_production_cpu_fallbacks: bool,

    /// Override production roots scanned by `--check-production-cpu-fallbacks`.
    /// Defaults to Vyre-owned production crates, excluding reference/conform crates.
    /// External consumers can be scanned by passing this flag repeatedly.
    #[arg(long)]
    production_root: Vec<PathBuf>,

    /// Run the consumer-name coupling guard over platform docs/comments.
    #[arg(long)]
    check_consumer_coupling: bool,

    /// Override roots scanned by `--check-consumer-coupling`.
    /// Defaults to current docs plus platform source crates.
    #[arg(long)]
    consumer_root: Vec<PathBuf>,

    /// Run the same-name module fork scanner over selected authority roots.
    #[arg(long)]
    check_module_forks: bool,

    /// Override roots scanned by `--check-module-forks`.
    /// Defaults to graph authority roots where fork drift has historically appeared.
    #[arg(long)]
    module_fork_root: Vec<PathBuf>,

    /// Run the GPU skip guard over CUDA/WGPU/runtime validation paths.
    #[arg(long)]
    check_gpu_skip_guards: bool,

    /// Override roots scanned by `--check-gpu-skip-guards`.
    #[arg(long)]
    gpu_skip_root: Vec<PathBuf>,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum Format {
    Text,
    Json,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let allowlist = cli
        .allowlist
        .clone()
        .unwrap_or_else(|| cli.workspace_root.join("vyre-lints/allowlist.toml"));

    if cli.check_drift {
        return run_drift(&allowlist, cli.drift_budget_days, cli.today.as_deref());
    }

    if cli.check_production_cpu_fallbacks {
        return run_production_cpu_fallbacks_cli(&cli);
    }

    if cli.check_consumer_coupling {
        return run_consumer_coupling_cli(&cli);
    }

    if cli.check_module_forks {
        return run_module_forks_cli(&cli);
    }

    if cli.check_gpu_skip_guards {
        return run_gpu_skip_guards_cli(&cli);
    }

    let lib_root = cli
        .lib_root
        .unwrap_or_else(|| cli.workspace_root.join("vyre-libs/src"));
    if !lib_root.exists() {
        anyhow::bail!("lib root not found: {}", lib_root.display());
    }
    let allowlist_arg = if allowlist.exists() {
        Some(allowlist.as_path())
    } else {
        None
    };

    let violations = run_raw_ir_in_libs(&[lib_root.as_path()], allowlist_arg)
        .context("running raw_ir_in_libs lint")?;

    match cli.format {
        Format::Text => emit_text(&violations),
        Format::Json => emit_json(&violations)?,
    }

    if violations.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn run_consumer_coupling_cli(cli: &Cli) -> Result<()> {
    let roots = if cli.consumer_root.is_empty() {
        default_consumer_coupling_roots(&cli.workspace_root)
    } else {
        cli.consumer_root.clone()
    };
    for root in &roots {
        if !root.exists() {
            anyhow::bail!(
                "consumer coupling root not found: {}. Fix: update the platform doc/comment guard roots instead of silently shrinking scan coverage.",
                root.display()
            );
        }
    }
    let root_refs: Vec<&std::path::Path> = roots.iter().map(|root| root.as_path()).collect();
    let violations =
        run_consumer_coupling(&root_refs).context("running consumer-name coupling guard")?;
    match cli.format {
        Format::Text => emit_text(&violations),
        Format::Json => emit_json(&violations)?,
    }
    if violations.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn run_module_forks_cli(cli: &Cli) -> Result<()> {
    let roots = if cli.module_fork_root.is_empty() {
        default_module_fork_roots(&cli.workspace_root)
    } else {
        cli.module_fork_root.clone()
    };
    for root in &roots {
        if !root.exists() {
            anyhow::bail!(
                "module fork root not found: {}. Fix: update the duplicate-module scan roots instead of silently shrinking scan coverage.",
                root.display()
            );
        }
    }
    let root_refs: Vec<&std::path::Path> = roots.iter().map(|root| root.as_path()).collect();
    let violations =
        run_module_forks(&root_refs).context("running same-name module fork scanner")?;
    match cli.format {
        Format::Text => emit_text(&violations),
        Format::Json => emit_json(&violations)?,
    }
    if violations.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn run_gpu_skip_guards_cli(cli: &Cli) -> Result<()> {
    let roots = if cli.gpu_skip_root.is_empty() {
        default_gpu_skip_roots(&cli.workspace_root)
    } else {
        cli.gpu_skip_root.clone()
    };
    for root in &roots {
        if !root.exists() {
            anyhow::bail!(
                "GPU skip guard root not found: {}. Fix: update CUDA/WGPU validation roots instead of silently shrinking scan coverage.",
                root.display()
            );
        }
    }
    let root_refs: Vec<&std::path::Path> = roots.iter().map(|root| root.as_path()).collect();
    let violations = run_gpu_skip_guards(&root_refs).context("running GPU skip guard")?;
    match cli.format {
        Format::Text => emit_text(&violations),
        Format::Json => emit_json(&violations)?,
    }
    if violations.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn run_drift(
    allowlist: &std::path::Path,
    budget_days: i64,
    today_override: Option<&str>,
) -> Result<()> {
    if !allowlist.exists() {
        anyhow::bail!("allowlist not found: {}", allowlist.display());
    }
    let today = match today_override {
        Some(s) => s.to_string(),
        None => current_iso_date(),
    };
    let resolver = vyre_lints::drift::GitBlameResolver::with_today(today);
    let findings = vyre_lints::drift::run(allowlist, budget_days, &resolver)
        .context("running allowlist drift sentinel")?;
    if findings.is_empty() {
        println!("vyre-lints drift: 0 stale entries (budget {budget_days} days)");
        return Ok(());
    }
    println!(
        "vyre-lints drift: {} stale entry(ies)  -  every entry should land its migration ticket within {budget_days} days.",
        findings.len()
    );
    for f in &findings {
        println!("{}", vyre_lints::drift::format_finding(f, budget_days));
    }
    std::process::exit(1);
}

fn run_production_cpu_fallbacks_cli(cli: &Cli) -> Result<()> {
    let roots = if cli.production_root.is_empty() {
        default_production_roots(&cli.workspace_root)
    } else {
        cli.production_root.clone()
    };
    for root in &roots {
        if !root.exists() {
            anyhow::bail!(
                "production root not found: {}. Fix: update the release CPU fallback guard roots instead of silently skipping this source tree.",
                root.display()
            );
        }
    }
    let root_refs: Vec<&std::path::Path> = roots.iter().map(|root| root.as_path()).collect();
    let violations = run_production_cpu_fallbacks(&root_refs)
        .context("running production CPU fallback guard")?;
    match cli.format {
        Format::Text => emit_text(&violations),
        Format::Json => emit_json(&violations)?,
    }
    if violations.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn default_production_roots(workspace_root: &std::path::Path) -> Vec<PathBuf> {
    [
        "vyre-aot/src",
        "vyre-core/src",
        "vyre-driver/src",
        "vyre-driver-cuda/src",
        "vyre-driver-wgpu/src",
        "vyre-frontend-c/src",
        "vyre-libs/src",
        "vyre-lower/src",
        "vyre-runtime/src",
        "vyre-self-substrate/src",
    ]
    .into_iter()
    .map(|root| workspace_root.join(root))
    .collect()
}

fn default_consumer_coupling_roots(workspace_root: &std::path::Path) -> Vec<PathBuf> {
    [
        "docs",
        "vyre-core/src",
        "vyre-driver/src",
        "vyre-driver-cuda/src",
        "vyre-driver-wgpu/src",
        "vyre-foundation/src",
        "vyre-libs/src",
        "vyre-lower/src",
        "vyre-primitives/src",
        "vyre-runtime/src",
        "vyre-self-substrate/src",
    ]
    .into_iter()
    .map(|root| workspace_root.join(root))
    .collect()
}

fn default_module_fork_roots(workspace_root: &std::path::Path) -> Vec<PathBuf> {
    [
        "vyre-primitives/src/graph",
        "vyre-self-substrate/src/graph",
        "vyre-libs/src/graph",
    ]
    .into_iter()
    .map(|root| workspace_root.join(root))
    .collect()
}

fn default_gpu_skip_roots(workspace_root: &std::path::Path) -> Vec<PathBuf> {
    [
        "vyre-driver-cuda/src",
        "vyre-driver-cuda/tests",
        "vyre-driver-wgpu/src",
        "vyre-driver-wgpu/tests",
        "vyre-runtime/src",
    ]
    .into_iter()
    .map(|root| workspace_root.join(root))
    .collect()
}

fn current_iso_date() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = now.div_euclid(86_400);
    iso_from_days(days)
}

fn iso_from_days(mut days: i64) -> String {
    let mut y = 1970i64;
    let is_leap = |y: i64| (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    loop {
        let len = if is_leap(y) { 366 } else { 365 };
        if days < len {
            break;
        }
        days -= len;
        y += 1;
    }
    let months: [i64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    while days >= months[m] {
        days -= months[m];
        m += 1;
    }
    format!("{y:04}-{:02}-{:02}", m + 1, days + 1)
}

fn emit_text(violations: &[Violation]) {
    if violations.is_empty() {
        println!("vyre-lints: 0 violations");
        return;
    }
    for v in violations {
        println!("{}:{}:{}: {}", v.file, v.line, v.column, v.message);
    }
    println!("vyre-lints: {} violation(s)", violations.len());
}

fn emit_json(violations: &[Violation]) -> Result<()> {
    use std::fmt::Write;
    let mut out = String::from("[\n");
    for (i, v) in violations.iter().enumerate() {
        let kind = match v.kind {
            vyre_lints::ViolationKind::RawNodeConstruction => "raw_node_construction",
            vyre_lints::ViolationKind::RawExprConstruction => "raw_expr_construction",
            vyre_lints::ViolationKind::ProductionCpuFallback => "production_cpu_fallback",
            vyre_lints::ViolationKind::ConsumerCoupling => "consumer_coupling",
            vyre_lints::ViolationKind::ModuleFork => "module_fork",
            vyre_lints::ViolationKind::GpuSkipGuard => "gpu_skip_guard",
        };
        if i > 0 {
            out.push_str(",\n");
        }
        write!(
            out,
            "  {{\"file\":{:?},\"line\":{},\"column\":{},\"kind\":{:?},\"message\":{:?}}}",
            v.file, v.line, v.column, kind, v.message
        )?;
    }
    out.push_str("\n]\n");
    print!("{out}");
    Ok(())
}

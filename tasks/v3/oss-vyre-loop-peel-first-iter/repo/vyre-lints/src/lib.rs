//! Lego-block enforcement lints for vyre.
//!
//! Source-of-truth: `SEPARATION_AUDIT_2026-05-01.md` section S0.
//!
//! Tier-3 dialect crates (`vyre-libs`) must compose Tier-2.5 primitives
//! (`vyre-primitives`); they must not reach into Tier-1 IR atoms
//! (`vyre-ir` / `vyre-foundation`) directly. Without this enforcement:
//!
//! - Two ops doing the same thing two different ways.
//! - The optimizer can't recognize either as a known shape.
//! - Egglog rule LHS patterns become brittle.
//! - Tier-2.5 primitive bug fixes don't propagate.
//!
//! This crate ships focused lints for raw IR construction, production CPU
//! fallbacks, consumer-name coupling, same-name module forks, and silent GPU
//! validation skips.

pub mod allowlist;
pub mod consumer_coupling;
pub mod drift;
pub mod gpu_skip_guards;
pub mod module_forks;
mod paths;
pub mod production_cpu_fallbacks;
pub mod raw_ir_in_libs;

use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;

const LINT_SOURCE_READ_CAP: usize = 64 * 1024 * 1024;

pub(crate) fn read_source_bounded(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("stat {}", path.display()))?
        .len();
    if len > LINT_SOURCE_READ_CAP as u64 {
        anyhow::bail!(
            "{} exceeds {LINT_SOURCE_READ_CAP} byte lint source read cap",
            path.display()
        );
    }
    let mut bytes = Vec::with_capacity(len as usize);
    let mut buf = [0u8; 8192];
    while bytes.len() <= LINT_SOURCE_READ_CAP {
        let read = file
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..read]);
        if bytes.len() > LINT_SOURCE_READ_CAP {
            anyhow::bail!(
                "{} exceeded {LINT_SOURCE_READ_CAP} byte lint source read cap while reading",
                path.display()
            );
        }
    }
    String::from_utf8(bytes).with_context(|| format!("decode UTF-8 {}", path.display()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub kind: ViolationKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    /// `Node::SomeVariant { .. }` or `Node::some_method(..)` outside
    /// `vyre-primitives` and outside test modules.
    RawNodeConstruction,
    /// `Expr::SomeVariant { .. }` or `Expr::some_method(..)` outside
    /// `vyre-primitives` and outside test modules.
    RawExprConstruction,
    /// Production code reached into CPU/reference execution instead of an
    /// explicit parity-test oracle.
    ProductionCpuFallback,
    /// Platform docs/comments mention a downstream consumer by name.
    ConsumerCoupling,
    /// Same Rust module basename appears in multiple scanned authority roots.
    ModuleFork,
    /// GPU/CUDA validation path silently skipped instead of failing loudly.
    GpuSkipGuard,
}

/// Run the `raw_ir_in_libs` lint over a directory tree.
///
/// `roots` are crate-root paths (e.g. `vyre-libs/src/`). The allowlist
/// is loaded from `allowlist_path`. Violations are returned in source
/// order (file, then line). Returns `Ok(violations)` on success;
/// `Err` only on I/O or parse failure.
pub fn run_raw_ir_in_libs(
    roots: &[&Path],
    allowlist_path: Option<&Path>,
) -> Result<Vec<Violation>> {
    let allow = match allowlist_path {
        Some(path) => allowlist::load(path)?,
        None => allowlist::Allowlist::empty(),
    };
    let mut all = Vec::new();
    for root in roots {
        all.extend(raw_ir_in_libs::scan_tree(root, &allow)?);
    }
    // Compare by (file, line) without cloning either field  -  the old
    // `(a.file.clone(), a.line).cmp(&(b.file.clone(), b.line))` cloned
    // two strings per compare (O(N log N) × 2 clones) which is wasted
    // work on a sort that fires on every audit.
    all.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    Ok(all)
}

/// Run the production CPU fallback guard over selected crate roots.
///
/// This is intentionally separate from `raw_ir_in_libs`: CPU/reference
/// execution is allowed in explicit oracle crates and tests, but not in
/// production dispatch paths.
pub fn run_production_cpu_fallbacks(roots: &[&Path]) -> Result<Vec<Violation>> {
    let mut all = Vec::new();
    for root in roots {
        all.extend(production_cpu_fallbacks::scan_tree(root)?);
    }
    all.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    Ok(all)
}

/// Run the consumer-name coupling guard over platform source/doc roots.
pub fn run_consumer_coupling(roots: &[&Path]) -> Result<Vec<Violation>> {
    let mut all = Vec::new();
    for root in roots {
        all.extend(consumer_coupling::scan_tree(root)?);
    }
    all.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    Ok(all)
}

/// Run the same-name module fork scanner over selected authority roots.
pub fn run_module_forks(roots: &[&Path]) -> Result<Vec<Violation>> {
    module_forks::scan_roots(roots)
}

/// Run the GPU skip guard over CUDA/WGPU/runtime source and test roots.
pub fn run_gpu_skip_guards(roots: &[&Path]) -> Result<Vec<Violation>> {
    let mut all = Vec::new();
    for root in roots {
        all.extend(gpu_skip_guards::scan_tree(root)?);
    }
    all.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    Ok(all)
}

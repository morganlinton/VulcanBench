//! Runtime introspection API.
//!
//! A-C11b ships the public surface tooling reads at runtime to
//! answer "what do you know?"  -  the list of dialects, ops, backends,
//! lowerings, and the dialect × backend coverage matrix.
//!
//! The data is **live**: every call iterates `inventory::iter` fresh
//! so changes take effect immediately. A cached snapshot would
//! silently drift from reality.
//!
//! Consumers:
//!
//! * IDE integrations that display "what ops exist in this dialect."
//! * CI gates that diff `docs/coverage-matrix.md` (the
//!   [`coverage_matrix`] output) against the committed file.
//! * Backend routers that filter by dialect support before dispatch.
//! * Documentation generators.

use std::collections::{BTreeMap, BTreeSet};

use crate::dialect::dialect::{OpBackendTarget, OpDefRegistration};
use crate::dialect::op_def::{Category, OpDef};
use crate::dialect::registry::Target;

/// Summary of a registered dialect.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialectSummary {
    /// Dialect identifier (e.g., `"math"`).
    pub id: &'static str,
    /// Number of ops registered under this dialect.
    pub op_count: usize,
}

/// Summary of a registered op.
#[derive(Debug, Clone)]
pub struct OpSummary {
    /// Fully qualified op id (e.g., `"math.add"`).
    pub id: &'static str,
    /// Parent dialect.
    pub dialect: &'static str,
    /// Category A/B/C.
    pub category: Category,
    /// List of backend targets that carry a lowering for this op.
    pub lowered_targets: Vec<Target>,
}

/// Summary of a registered backend.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BackendSummary {
    /// Backend id (e.g., `"wgpu"`, `"reference"`).
    pub id: &'static str,
    /// List of the backend's advertised targets.
    pub targets: Vec<Target>,
}

/// A single (dialect, backend) cell in the coverage matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageCell {
    /// Backend supports every op in the dialect.
    Full,
    /// Backend supports some but not all ops.
    Partial {
        /// Number of ops supported.
        covered: usize,
        /// Total number of ops in the dialect.
        total: usize,
    },
    /// Backend has no lowering for any op in this dialect.
    None,
}

impl CoverageCell {
    /// Render as the single-character label used in
    /// `docs/coverage-matrix.md`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CoverageCell::Full => "✓",
            CoverageCell::Partial { .. } => "partial",
            CoverageCell::None => "-",
        }
    }
}

/// The full dialect × backend matrix.
pub struct CoverageMatrix {
    /// Dialect ids in ordered iteration order.
    pub dialects: Vec<&'static str>,
    /// Backend ids in ordered iteration order.
    pub backends: Vec<&'static str>,
    /// `cells[(dialect, backend)]`  -  one entry per pair.
    pub cells: BTreeMap<(&'static str, &'static str), CoverageCell>,
}

/// Every registered dialect.
#[must_use]
pub fn dialects() -> Vec<DialectSummary> {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for reg in inventory::iter::<OpDefRegistration> {
        let def = (reg.op)();
        *counts.entry(def.dialect).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(id, op_count)| DialectSummary { id, op_count })
        .collect()
}

/// Every op registered under `dialect`. `None` when no op uses that
/// dialect id.
#[must_use]
pub fn ops(dialect: &str) -> Option<Vec<OpSummary>> {
    let mut any = false;
    let mut out: Vec<OpSummary> = Vec::new();
    for reg in inventory::iter::<OpDefRegistration> {
        let def = (reg.op)();
        if def.dialect == dialect {
            any = true;
            out.push(summary_for_op(&def));
        }
    }
    if any {
        out.sort_by_key(|op| op.id);
        Some(out)
    } else {
        None
    }
}

/// Every registered backend target.
///
/// The current `OpBackendTarget` shape is per-op: it says "op X
/// is supported on target Y". We aggregate across all such
/// registrations to identify the backend targets present in the
/// workspace. The returned summary's `id` is the target string
/// (`"wgsl"`, `"spirv"`, `"secondary_text"`, `"native_module"`, `"reference"`).
#[must_use]
pub fn backends() -> Vec<BackendSummary> {
    let mut target_set: BTreeSet<&'static str> = BTreeSet::new();
    for reg in inventory::iter::<OpBackendTarget> {
        target_set.insert(reg.target);
    }
    // Always include the reference CPU path since every op ships
    // with `cpu_ref` (the type forces it).
    target_set.insert("reference");
    target_set
        .into_iter()
        .map(|id| {
            let primary = match id {
                "wgsl" => Target::PrimaryText,
                "spirv" => Target::PrimaryBinary,
                "secondary_text" => Target::SecondaryText,
                "native_module" => Target::NativeModule,
                _ => Target::ReferenceBackend,
            };
            BackendSummary {
                id,
                targets: vec![primary],
            }
        })
        .collect()
}

/// Targets that have a lowering registered for the given op id.
///
/// Returns an empty Vec when the op is not registered. A
/// Category-C op (like the `io` dialect) returns the empty vec
/// because no backend opts in yet.
#[must_use]
pub fn lowerings(op_id: &str) -> Vec<Target> {
    for reg in inventory::iter::<OpDefRegistration> {
        let def = (reg.op)();
        if def.id == op_id {
            return targets_for_op(&def);
        }
    }
    Vec::new()
}

/// Build the dialect × backend coverage matrix from the live
/// registries.
#[must_use]
pub fn coverage_matrix() -> CoverageMatrix {
    let dialect_summaries = dialects();
    let backend_summaries = backends();

    let dialect_ids: Vec<&'static str> = dialect_summaries.iter().map(|d| d.id).collect();
    let backend_ids: Vec<&'static str> = backend_summaries.iter().map(|b| b.id).collect();

    // Group ops by dialect with their lowering targets.
    let mut by_dialect: BTreeMap<&'static str, Vec<Vec<Target>>> = BTreeMap::new();
    for reg in inventory::iter::<OpDefRegistration> {
        let def = (reg.op)();
        by_dialect
            .entry(def.dialect)
            .or_default()
            .push(targets_for_op(&def));
    }

    let mut cells = BTreeMap::new();
    for dialect in &dialect_ids {
        for backend in &backend_ids {
            cells.insert(
                (*dialect, *backend),
                cell_for(&by_dialect, dialect, backend),
            );
        }
    }

    CoverageMatrix {
        dialects: dialect_ids,
        backends: backend_ids,
        cells,
    }
}

impl CoverageMatrix {
    /// Render the matrix as a markdown table for
    /// `docs/coverage-matrix.md`.
    #[must_use]
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# dialect × backend coverage matrix\n\n");
        out.push_str(
            "Generated from the live `DialectRegistry` + `OpBackendTarget`\n\
             inventory. Regenerate via `VYRE_REGEN_COVERAGE=1 cargo_full test -p vyre\n\
             --test coverage_matrix`. Every cell transition from ✓ → - is a\n\
             coverage regression gated by CI.\n\n",
        );
        // Header row: dialect | backend1 | backend2 | ...
        out.push_str("| dialect |");
        for b in &self.backends {
            out.push_str(&format!(" {b} |"));
        }
        out.push('\n');
        out.push_str("|---------|");
        for _ in &self.backends {
            out.push_str("------|");
        }
        out.push('\n');
        for d in &self.dialects {
            out.push_str(&format!("| `{d}` |"));
            for b in &self.backends {
                let cell = self
                    .cells
                    .get(&(*d, *b))
                    .copied()
                    .unwrap_or(CoverageCell::None);
                out.push_str(&format!(" {} |", cell.label()));
            }
            out.push('\n');
        }
        out
    }
}

fn summary_for_op(def: &OpDef) -> OpSummary {
    OpSummary {
        id: def.id,
        dialect: def.dialect,
        category: def.category,
        lowered_targets: targets_for_op(def),
    }
}

fn targets_for_op(def: &OpDef) -> Vec<Target> {
    let mut out = Vec::new();
    if def.lowerings.primary_text.is_some() {
        out.push(Target::PrimaryText);
    }
    if def.lowerings.primary_binary.is_some() {
        out.push(Target::PrimaryBinary);
    }
    if def.lowerings.secondary_text.is_some() {
        out.push(Target::SecondaryText);
    }
    if def.lowerings.native_module.is_some() {
        out.push(Target::NativeModule);
    }
    // cpu_ref is always present (the type forces it).
    out.push(Target::ReferenceBackend);
    out
}

fn cell_for(
    by_dialect: &BTreeMap<&'static str, Vec<Vec<Target>>>,
    dialect: &str,
    backend: &str,
) -> CoverageCell {
    let Some(ops) = by_dialect.get(dialect) else {
        return CoverageCell::None;
    };
    // Map backend id to the single Target it cares about. The
    // mapping below is conservative  -  backends can advertise more
    // targets in their registration, but for the coverage matrix a
    // single "primary target" is enough to populate a cell.
    let target = match backend {
        "wgpu" => Target::PrimaryText,
        "spirv" => Target::PrimaryBinary,
        "secondary_text" => Target::SecondaryText,
        "native_module" => Target::NativeModule,
        "reference" | "cpu-ref" | "reference-backend" => Target::ReferenceBackend,
        _ => return CoverageCell::None,
    };
    let total = ops.len();
    let covered = ops
        .iter()
        .filter(|targets| targets.contains(&target))
        .count();
    if covered == 0 {
        CoverageCell::None
    } else if covered == total {
        CoverageCell::Full
    } else {
        CoverageCell::Partial { covered, total }
    }
}

/// Set of every unique backend id registered.
#[must_use]
pub fn backend_ids() -> BTreeSet<&'static str> {
    backends().into_iter().map(|b| b.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dialect_has_some_op() {
        let list = dialects();
        assert!(!list.is_empty(), "at least the io dialect is registered");
        for d in &list {
            assert!(d.op_count > 0, "dialect {d:?} has zero ops");
        }
    }

    #[test]
    fn ops_query_returns_ordered_entries() {
        let io = ops("io").expect("Fix: io dialect missing from DialectRegistry; ensure the io ops crate is linked in this test binary.");
        assert!(io.iter().all(|op| op.dialect == "io"));
        let ids: Vec<&'static str> = io.iter().map(|o| o.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "op list is deterministically sorted");
    }

    #[test]
    fn ops_unknown_dialect_returns_none() {
        assert!(ops("nothere").is_none());
    }

    #[test]
    fn lowerings_cpu_ref_always_present() {
        let targets = lowerings("io.dma_from_nvme");
        assert!(targets.contains(&Target::ReferenceBackend));
    }

    #[test]
    fn coverage_matrix_has_stable_shape() {
        let m = coverage_matrix();
        for d in &m.dialects {
            for b in &m.backends {
                assert!(m.cells.contains_key(&(*d, *b)));
            }
        }
    }

    #[test]
    fn coverage_matrix_renders_to_markdown() {
        let rendered = coverage_matrix().render_markdown();
        assert!(rendered.contains("# dialect × backend coverage matrix"));
    }
}

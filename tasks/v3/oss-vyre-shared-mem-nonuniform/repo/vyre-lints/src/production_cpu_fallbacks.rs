//! Production CPU fallback guard.
//!
//! CPU/reference execution is valid only as an explicit oracle in tests,
//! conformance harnesses, or reference backend crates. It is forbidden in
//! production GPU dispatch paths because a hidden reference path
//! can turn a GPU regression into a green release.

use crate::{Violation, ViolationKind};
use anyhow::Result;
use std::path::Path;

const FORBIDDEN_FRAGMENTS: &[&str] = &[
    "vyre_reference::reference_eval",
    "vyre_reference :: reference_eval",
    "vyre_driver_reference",
    "reference_c11_build_vast_nodes",
    "reference_c11_annotate_typedef_names",
    "reference_c11_classify_vast_node_kinds",
    "reference_ast_to_pg_nodes",
    "reference_c_keyword_types",
    "run_cpu_fixpoint_to_convergence",
    "cpu_vs_backend",
    "reference_semiring_gemm",
    "reference_sinkhorn_clustering",
    "reference_scc_components_via_substrate",
    "cpu_ref(",
    "cpu_op",
    "cpu_references",
];

const APPROVED_PARITY_PATHS: &[&str] = &[
    "/tests/",
    "/benches/",
    "/examples/",
    "/fixtures/",
    "/vyre-reference/",
    "/vyre-driver-reference/",
    "/conform/",
    "/vyre-conform/",
    "/vyre-test-harness/",
];

pub fn scan_tree(root: &Path) -> Result<Vec<Violation>> {
    let mut all = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let workspace_rel = workspace_relative(path);
        if is_approved_parity_path(&workspace_rel) || is_approved_parity_file(&workspace_rel) {
            continue;
        }
        all.extend(scan_file(path, &workspace_rel)?);
    }
    Ok(all)
}

fn workspace_relative(path: &Path) -> String {
    let s = path.to_string_lossy();
    for marker in [
        "vyre-aot/",
        "vyre-bench/",
        "vyre-core/",
        "vyre-debug/",
        "vyre-driver/",
        "vyre-driver-cuda/",
        "vyre-driver-spirv/",
        "vyre-driver-wgpu/",
        "vyre-emit-naga/",
        "vyre-emit-ptx/",
        "vyre-emit-spirv/",
        "vyre-foundation/",
        "vyre-frontend-c/",
        "vyre-harness/",
        "vyre-intrinsics/",
        "vyre-libs/",
        "vyre-lints/",
        "vyre-lower/",
        "vyre-macros/",
        "vyre-ops/",
        "vyre-primitives/",
        "vyre-runtime/",
        "vyre-self-substrate/",
        "vyre-spec/",
        "vyre-std/",
        "xtask/",
    ] {
        if let Some(idx) = s.find(marker) {
            return s[idx..].to_string();
        }
    }
    s.to_string()
}

fn is_approved_parity_path(workspace_rel: &str) -> bool {
    let wrapped = format!("/{workspace_rel}");
    APPROVED_PARITY_PATHS
        .iter()
        .any(|approved| wrapped.contains(approved))
}

fn is_approved_parity_file(workspace_rel: &str) -> bool {
    let file_name = workspace_rel.rsplit('/').next().unwrap_or(workspace_rel);
    file_name == "tests.rs"
        || file_name == "test.rs"
        || file_name == "reference.rs"
        || file_name == "oracle.rs"
        || file_name.ends_with("cpu_oracle.rs")
        || file_name == "cpu_fallback_reachability.rs"
        || file_name == "witness.rs"
        || file_name.starts_with("ref_")
        || file_name.ends_with("_tests.rs")
}

fn scan_file(path: &Path, workspace_rel: &str) -> Result<Vec<Violation>> {
    let source = crate::read_source_bounded(path)?;
    if source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//!"))
        .is_some_and(is_parity_cfg)
    {
        return Ok(Vec::new());
    }
    let mut violations = Vec::new();
    let mut cfg_test_depth = 0usize;
    let mut cfg_parity_depth = 0usize;
    let mut pending_cfg_test = false;
    let mut pending_cfg_parity = false;

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if is_test_cfg(trimmed) {
            pending_cfg_test = true;
            continue;
        }
        if is_parity_cfg(trimmed) {
            pending_cfg_parity = true;
            continue;
        }
        if pending_cfg_test && (trimmed.starts_with("#[") || trimmed.starts_with("///")) {
            continue;
        }
        if pending_cfg_test && starts_cfg_gated_item(trimmed) {
            cfg_test_depth = item_depth(line);
            pending_cfg_test = false;
            continue;
        }
        pending_cfg_test = false;

        if cfg_test_depth > 0 {
            cfg_test_depth = advance_cfg_item_depth(cfg_test_depth, line);
            continue;
        }
        if cfg_parity_depth > 0 {
            cfg_parity_depth = advance_cfg_item_depth(cfg_parity_depth, line);
            continue;
        }

        if pending_cfg_parity && (trimmed.starts_with("#[") || trimmed.starts_with("///")) {
            continue;
        }
        if pending_cfg_parity && starts_cfg_gated_item(trimmed) {
            cfg_parity_depth = item_depth(line);
            pending_cfg_parity = false;
            continue;
        }
        pending_cfg_parity = false;

        if is_comment_or_oracle_definition(trimmed) {
            continue;
        }

        if contains_cpu_helper_definition(trimmed) {
            violations.push(Violation {
                file: workspace_rel.to_string(),
                line: (line_idx + 1) as u32,
                column: line.find("cpu_").unwrap_or(0) as u32,
                kind: ViolationKind::ProductionCpuFallback,
                message:
                    "production CPU/reference helper definition outside approved parity surface"
                        .to_string(),
            });
            continue;
        }

        for fragment in FORBIDDEN_FRAGMENTS {
            if let Some(column) = line.find(fragment) {
                violations.push(Violation {
                    file: workspace_rel.to_string(),
                    line: (line_idx + 1) as u32,
                    column: column as u32,
                    kind: ViolationKind::ProductionCpuFallback,
                    message: format!(
                        "production CPU/reference fallback `{fragment}` outside approved parity surface"
                    ),
                });
            }
        }
    }
    Ok(violations)
}

fn contains_cpu_helper_definition(trimmed: &str) -> bool {
    (trimmed.contains("fn ") && (trimmed.contains("fn cpu_") || trimmed.contains("_cpu")))
        || trimmed.starts_with("mod cpu_")
        || trimmed.starts_with("pub mod cpu_")
        || trimmed.starts_with("pub(crate) mod cpu_")
        || trimmed.starts_with("pub use cpu_")
        || trimmed.starts_with("pub(crate) use cpu_")
}

fn is_test_cfg(trimmed: &str) -> bool {
    trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("# [cfg(test)]")
}

fn is_parity_cfg(trimmed: &str) -> bool {
    trimmed.contains("cfg(any(test, feature = \"cpu-parity\"))")
        || trimmed.contains("cfg(any(test,feature=\"cpu-parity\"))")
        || trimmed.contains("cfg(any(feature = \"cpu-parity\", test))")
        || trimmed.contains("cfg(feature = \"cpu-parity\")")
}

fn starts_cfg_gated_item(trimmed: &str) -> bool {
    trimmed.contains(" fn ")
        || trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("pub(crate) fn ")
        || trimmed.starts_with("pub(super) fn ")
        || trimmed.starts_with("mod ")
        || trimmed.starts_with("pub mod ")
        || trimmed.starts_with("pub(crate) mod ")
        || trimmed.starts_with("impl ")
        || trimmed.starts_with("pub struct ")
        || trimmed.starts_with("struct ")
        || trimmed.starts_with("pub enum ")
        || trimmed.starts_with("enum ")
        || trimmed.starts_with("pub use ")
        || trimmed.starts_with("pub(crate) use ")
}

fn item_depth(line: &str) -> usize {
    let depth = byte_count(line, b'{').saturating_sub(byte_count(line, b'}'));
    if depth == 0 && !line.contains('{') {
        usize::MAX
    } else {
        depth
    }
}

fn advance_cfg_item_depth(depth: usize, line: &str) -> usize {
    if depth == usize::MAX {
        return item_depth(line);
    }
    depth
        .saturating_add(byte_count(line, b'{'))
        .saturating_sub(byte_count(line, b'}'))
}

fn byte_count(line: &str, needle: u8) -> usize {
    line.as_bytes()
        .iter()
        .filter(|byte| **byte == needle)
        .count()
}

fn is_comment_or_oracle_definition(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || trimmed.starts_with("*")
        || (trimmed.contains("reference_")
            && !trimmed.contains('(')
            && (trimmed.ends_with(',') || trimmed.ends_with("};")))
        || trimmed.starts_with("pub fn reference_")
        || trimmed.starts_with("fn reference_")
        || trimmed.starts_with("pub(crate) fn reference_")
        || trimmed.starts_with("pub(super) fn reference_")
        || trimmed.starts_with("reference_")
        || trimmed.starts_with("use super::")
}

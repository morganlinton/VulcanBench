//! Boundary tests for self-substrate GPU dispatch wrappers.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_VIA_CALLS: &[&str] = &[
    "::cpu_ref(",
    "::cpu_ref_into(",
    "_cpu(",
    "_cpu_into(",
    "cpu_ref(",
    "cpu_ref_into(",
    "reference_",
    "reference_eval(",
    "xor_bind_cpu(",
];

const REFERENCE_ONLY_PUBLIC_NAMES: &[&str] = &[
    "DataflowFixpointScratch",
    "SinkhornClusteringScratch",
    "forward_backward_bitsets_for_pivot",
    "forward_backward_bitsets_for_pivot_into",
    "lineage_closure",
    "lineage_closure_into",
    "reference_scc_components_via_substrate_into",
    "reference_semiring_gemm",
    "reference_semiring_gemm_into",
    "reference_sinkhorn_clustering",
    "reference_sinkhorn_clustering_into",
    "scc_components_via_substrate",
    "shortest_path_closure",
    "shortest_path_closure_into",
];

#[test]
fn public_via_functions_do_not_call_reference_or_cpu_helpers() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("self-substrate source file must be readable");
        scan_via_functions(&path, &text, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "public *_via functions are production GPU-dispatch boundaries and must not call \
         CPU/reference helpers. Use a concrete Vyre dispatcher or rename the function out of \
         the production *_via surface.\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_modules_do_not_export_public_cpu_named_helpers() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("self-substrate source file must be readable");
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("pub fn ")
                && (trimmed.contains("_cpu") || trimmed.contains("cpu_"))
                && !trimmed.contains("reference_")
            {
                violations.push(format!("{}:{}: {}", path.display(), idx + 1, trimmed));
            }
            if trimmed.starts_with("pub use ")
                && (trimmed.contains("_cpu") || trimmed.contains("cpu_"))
                && !trimmed.contains("reference_")
            {
                violations.push(format!("{}:{}: {}", path.display(), idx + 1, trimmed));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "self-substrate must not export public CPU-named helpers from production modules. \
         Keep CPU/reference logic private or name it reference_* so consumers cannot mistake it \
         for a supported execution path.\n{}",
        violations.join("\n")
    );
}

#[test]
fn reference_only_public_surfaces_are_cfg_gated() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("self-substrate source file must be readable");
        let lines = text.lines().collect::<Vec<_>>();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !is_reference_only_public_surface(trimmed) {
                continue;
            }
            if !has_test_or_cpu_parity_guard(&lines, idx) {
                violations.push(format!("{}:{}: {}", path.display(), idx + 1, trimmed));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "self-substrate reference-only public surfaces must be gated behind \
         #[cfg(test)] or feature=\"cpu-parity\" so production builds expose only GPU dispatch \
         APIs.\n{}",
        violations.join("\n")
    );
}

#[test]
fn public_non_reference_functions_do_not_call_cpu_or_reference_helpers() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("self-substrate source file must be readable");
        scan_public_non_reference_functions(&path, &text, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "public production functions must not call CPU/reference helpers unless the function is \
         explicitly named reference_*. GPU execution boundaries stay under *_via; CPU oracles stay \
         visibly reference-only.\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_modules_do_not_rebuild_dispatch_input_vecs() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).expect("self-substrate source file must be readable");
        scan_production_dispatch_input_vecs(&path, &text, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "production GPU dispatch paths must use caller-owned scratch input slots instead of \
         rebuilding `let inputs = vec![...]` shells. Test modules may build fixtures; production \
         dispatch wrappers must use dispatch_buffers::ensure_input_slots plus in-place writers.\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("self-substrate src directory must be readable") {
        let entry = entry.expect("self-substrate src entry must be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn scan_production_dispatch_input_vecs(path: &Path, text: &str, violations: &mut Vec<String>) {
    let mut in_test_module_depth: Option<i32> = None;

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[cfg(test)]") {
            in_test_module_depth = Some(0);
        }
        if in_test_module_depth.is_none() && trimmed.contains("let inputs = vec![") {
            violations.push(format!("{}:{}: {}", path.display(), idx + 1, trimmed));
        }

        let delta = brace_delta(line);
        if let Some(depth) = in_test_module_depth.as_mut() {
            *depth += delta;
            if *depth <= 0 && line.contains('}') {
                in_test_module_depth = None;
            }
        }
    }
}

fn scan_public_non_reference_functions(path: &Path, text: &str, violations: &mut Vec<String>) {
    let mut in_test_module_depth: Option<i32> = None;
    let mut in_public_fn: Option<(String, i32)> = None;
    let mut pending_parity_cfg = false;

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[cfg(test)]") {
            in_test_module_depth = Some(0);
        }
        if trimmed.contains("cfg(test)") || trimmed.contains("feature = \"cpu-parity\"") {
            pending_parity_cfg = true;
        }
        if in_test_module_depth.is_none()
            && in_public_fn.is_none()
            && (trimmed.starts_with("pub fn ") || trimmed.starts_with("pub(crate) fn "))
        {
            if pending_parity_cfg {
                pending_parity_cfg = false;
            } else {
                let name = trimmed
                    .split_once("fn ")
                    .and_then(|(_, rest)| rest.split_once('('))
                    .map(|(name, _)| name.trim().to_owned())
                    .unwrap_or_else(|| "<unknown>".to_owned());
                if !name.starts_with("reference_") {
                    in_public_fn = Some((name, 0));
                }
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with("#[") {
            pending_parity_cfg = false;
        }

        if let Some((name, _)) = in_public_fn.as_ref() {
            if FORBIDDEN_VIA_CALLS
                .iter()
                .any(|needle| line.contains(needle))
            {
                violations.push(format!(
                    "{}:{}: {name} contains forbidden CPU/reference call: {}",
                    path.display(),
                    idx + 1,
                    trimmed
                ));
            }
        }

        let delta = brace_delta(line);
        if let Some(depth) = in_test_module_depth.as_mut() {
            *depth += delta;
            if *depth <= 0 && line.contains('}') {
                in_test_module_depth = None;
            }
        }
        if let Some((_, depth)) = in_public_fn.as_mut() {
            *depth += delta;
            if *depth <= 0 && line.contains('}') {
                in_public_fn = None;
            }
        }
    }
}

fn scan_via_functions(path: &Path, text: &str, violations: &mut Vec<String>) {
    let mut in_test_module_depth: Option<i32> = None;
    let mut in_via_fn: Option<(&str, i32)> = None;
    let mut pending_parity_cfg = false;

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[cfg(test)]") {
            in_test_module_depth = Some(0);
        }
        if trimmed.contains("cfg(test)") || trimmed.contains("feature = \"cpu-parity\"") {
            pending_parity_cfg = true;
        }
        if in_test_module_depth.is_none()
            && in_via_fn.is_none()
            && (trimmed.starts_with("pub fn ") || trimmed.starts_with("pub(crate) fn "))
            && trimmed.contains("_via(")
        {
            if pending_parity_cfg {
                pending_parity_cfg = false;
            } else {
                let name = trimmed
                    .split_once("fn ")
                    .and_then(|(_, rest)| rest.split_once('('))
                    .map(|(name, _)| name)
                    .unwrap_or("<unknown>");
                in_via_fn = Some((name, 0));
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with("#[") {
            pending_parity_cfg = false;
        }

        if let Some((name, _)) = in_via_fn {
            if FORBIDDEN_VIA_CALLS
                .iter()
                .any(|needle| line.contains(needle))
            {
                violations.push(format!(
                    "{}:{}: {name} contains forbidden CPU/reference call: {}",
                    path.display(),
                    idx + 1,
                    trimmed
                ));
            }
        }

        let delta = brace_delta(line);
        if let Some(depth) = in_test_module_depth.as_mut() {
            *depth += delta;
            if *depth <= 0 && line.contains('}') {
                in_test_module_depth = None;
            }
        }
        if let Some((_, depth)) = in_via_fn.as_mut() {
            *depth += delta;
            if *depth <= 0 && line.contains('}') {
                in_via_fn = None;
            }
        }
    }
}

fn brace_delta(line: &str) -> i32 {
    let mut delta = 0i32;
    for ch in line.chars() {
        match ch {
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

fn is_reference_only_public_surface(line: &str) -> bool {
    if !(line.starts_with("pub fn ")
        || line.starts_with("pub(crate) fn ")
        || line.starts_with("pub struct ")
        || line.starts_with("pub(crate) struct ")
        || line.starts_with("pub use ")
        || line.starts_with("pub(crate) use "))
    {
        return false;
    }

    if line.contains("reference_")
        || line.contains("cpu_ref")
        || line.contains("_cpu")
        || line.contains("cpu_")
    {
        return true;
    }

    public_item_name(line).is_some_and(|name| REFERENCE_ONLY_PUBLIC_NAMES.contains(&name))
}

fn public_item_name(line: &str) -> Option<&str> {
    for marker in ["fn ", "struct "] {
        if let Some((_, rest)) = line.split_once(marker) {
            return rest
                .split(|ch: char| ch == '(' || ch == '<' || ch == ' ' || ch == '{')
                .next();
        }
    }
    None
}

fn has_test_or_cpu_parity_guard(lines: &[&str], idx: usize) -> bool {
    let mut remaining_attrs = 8usize;
    let mut cursor = idx;
    while cursor > 0 && remaining_attrs > 0 {
        cursor -= 1;
        let prior = lines[cursor].trim();
        if prior.is_empty() {
            continue;
        }
        if !prior.starts_with("#[") {
            break;
        }
        if prior.contains("cfg(test)")
            || prior.contains("feature = \"cpu-parity\"")
            || prior.contains("feature=\"cpu-parity\"")
        {
            return true;
        }
        remaining_attrs -= 1;
    }
    false
}

//! Workspace GPU-boundary contracts.
//!
//! Production Vyre modules must not expose silent CPU execution paths. CPU and
//! reference code is allowed only when the path or symbol is explicitly marked
//! as oracle/reference/parity/test infrastructure.

use std::path::{Path, PathBuf};

const VYRE_WORKSPACE_CRATES: &[&str] = &[
    "vyre-core",
    "vyre-foundation",
    "vyre-driver",
    "vyre-driver-wgpu",
    "vyre-driver-spirv",
    "vyre-driver-cuda",
    "vyre-runtime",
    "vyre-libs",
    "vyre-self-substrate",
    "vyre-frontend-c",
    "vyre-lower",
    "vyre-aot",
];

const REQUIRED_ADJACENT_SOURCE_ROOTS: &[(&str, &str)] =
    &[("compiler-cli", "../../../../tools/vyrec/src")];

const FORBIDDEN_CPU_CALLS: &[&str] = &[
    "cpu_ref(",
    "cpu_ref_into(",
    "::cpu_ref(",
    "::cpu_ref_into(",
    "reference_eval(",
    "cpu_dispatch(",
    "dispatch_cpu(",
    "run_cpu(",
    "execute_cpu(",
    "fallback_cpu(",
];

const FORBIDDEN_FALLBACK_PHRASES: &[&str] = &[
    "cpu fallback",
    "fallback to cpu",
    "fall back to cpu",
    "cpu-only fallback",
    "host fallback",
    "fallback to host",
];

#[test]
fn production_sources_do_not_call_cpu_helpers_outside_oracles() {
    let workspace_root = workspace_root();
    let mut violations = Vec::new();

    for (_label, src) in source_roots(&workspace_root) {
        let mut files = Vec::new();
        collect_rust_files(&src, &mut files);
        for path in files {
            if is_explicit_reference_surface(&path) {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("Vyre source file must be readable");
            scan_forbidden_cpu_calls(&workspace_root, &path, &text, &mut violations);
        }
    }

    assert!(
        violations.is_empty(),
        "production Vyre source must not call CPU/reference helpers outside explicit oracle, \
         reference, parity, or test surfaces. Violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn release_host_must_expose_nvidia_gpu() {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,driver_version",
            "--format=csv,noheader",
        ])
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "GPU probe failed before execution. Fix: repair PATH/driver visibility; release hosts must expose NVIDIA GPUs, not skip GPU tests. Error: {error}"
            )
        });
    assert!(
        output.status.success(),
        "GPU probe command failed. Fix: repair NVIDIA driver/runtime visibility; do not skip GPU-required tests.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("NVIDIA") && !stdout.trim().is_empty(),
        "GPU probe did not report an NVIDIA adapter. Fix: repair adapter discovery; false no-GPU skips are release failures. stdout:\n{stdout}"
    );
}

#[test]
fn production_sources_do_not_advertise_cpu_or_host_fallbacks() {
    let workspace_root = workspace_root();
    let mut violations = Vec::new();

    for (_label, src) in source_roots(&workspace_root) {
        let mut files = Vec::new();
        collect_rust_files(&src, &mut files);
        for path in files {
            if is_explicit_reference_surface(&path) {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("Vyre source file must be readable");
            for (line_idx, line) in text.lines().enumerate() {
                let lowered = strip_line_comment(line).to_ascii_lowercase();
                if advertises_forbidden_fallback(&lowered) {
                    violations.push(format!(
                        "{}:{}: {}",
                        relative_path(&workspace_root, &path),
                        line_idx + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production Vyre source must fail loudly on missing GPU capability, not advertise CPU/host fallback paths. Violations:\n{}",
        violations.join("\n")
    );
}

fn advertises_forbidden_fallback(lowered_code: &str) -> bool {
    FORBIDDEN_FALLBACK_PHRASES
        .iter()
        .any(|phrase| lowered_code.contains(phrase))
        && !lowered_code.contains("no cpu fallback")
        && !lowered_code.contains("no host fallback")
        && !lowered_code.contains("without cpu fallback")
        && !lowered_code.contains("without host fallback")
}

fn source_roots(workspace_root: &Path) -> Vec<(String, PathBuf)> {
    let mut roots = Vec::new();
    for crate_name in VYRE_WORKSPACE_CRATES {
        let src = workspace_root.join(crate_name).join("src");
        if src.is_dir() {
            roots.push(((*crate_name).to_owned(), src));
        }
    }
    for (label, relative_src) in REQUIRED_ADJACENT_SOURCE_ROOTS {
        let src = workspace_root.join(relative_src);
        assert!(
            src.is_dir(),
            "required adjacent GPU-boundary scan root `{label}` is missing at {}",
            src.display()
        );
        roots.push(((*label).to_owned(), src));
    }
    roots.extend(adjacent_dataflow_source_roots(workspace_root));
    roots
}

fn adjacent_dataflow_source_roots(workspace_root: &Path) -> Vec<(String, PathBuf)> {
    let dataflow_root = workspace_root.join("../../../dataflow");
    assert!(
        dataflow_root.is_dir(),
        "required adjacent dataflow source root is missing at {}",
        dataflow_root.display()
    );

    let mut roots = Vec::new();
    let entries = std::fs::read_dir(&dataflow_root).unwrap_or_else(|error| {
        panic!(
            "required adjacent dataflow source root `{}` must be readable: {error}",
            dataflow_root.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("adjacent dataflow directory entry must be readable");
        let crate_root = entry.path();
        let src = crate_root.join("src");
        if src.is_dir() {
            let label = crate_root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("dataflow-crate")
                .to_owned();
            roots.push((format!("dataflow/{label}"), src));
        }
    }
    assert!(
        !roots.is_empty(),
        "required adjacent dataflow source root `{}` did not contain any crate src/ directories",
        dataflow_root.display()
    );
    roots
}

fn scan_forbidden_cpu_calls(
    workspace_root: &Path,
    path: &Path,
    text: &str,
    violations: &mut Vec<String>,
) {
    let mut in_test_module_depth: Option<i32> = None;
    let mut in_production_fn: Option<(String, i32)> = None;
    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[cfg(test)]") {
            in_test_module_depth = Some(0);
        }

        if in_test_module_depth.is_none() && in_production_fn.is_none() {
            if let Some(name) = function_name(trimmed) {
                if !is_reference_symbol(&name) {
                    in_production_fn = Some((name, 0));
                }
            }
        }

        if let Some((name, _)) = in_production_fn.as_ref() {
            let code = strip_line_comment(line);
            if FORBIDDEN_CPU_CALLS
                .iter()
                .any(|needle| code.contains(needle))
            {
                violations.push(format!(
                    "{}:{}: {name} contains forbidden CPU/reference call: {}",
                    relative_path(workspace_root, path),
                    line_idx + 1,
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
        if let Some((_, depth)) = in_production_fn.as_mut() {
            *depth += delta;
            if *depth <= 0 && line.contains('}') {
                in_production_fn = None;
            }
        }
    }
}

fn is_explicit_reference_surface(path: &Path) -> bool {
    let path_text = path.to_string_lossy().replace('\\', "/");
    path_text.contains("/tests/")
        || path_text.contains("/benches/")
        || path_text.contains("/examples/")
        || path_text.contains("/reference")
        || path_text.contains("/oracle")
        || path_text.contains("/cpu_reference")
        || path_text.contains("/cpu_references")
        || path_text.ends_with("/cpu_fallback_reachability.rs")
        || path_text.contains("/ref_")
        || path_text.ends_with("_tests.rs")
        || path_text.ends_with("/tests.rs")
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("Vyre src directory must be readable") {
        let entry = entry.expect("Vyre src entry must be readable");
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("__law7_split") {
                continue;
            }
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            if path.components().any(|c| c.as_os_str() == "__law7_split") {
                continue;
            }
            out.push(path);
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vyre-core must live under the Vyre workspace root")
        .to_path_buf()
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _comment)| code)
}

fn brace_delta(line: &str) -> i32 {
    line.chars().fold(0, |depth, ch| match ch {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

fn function_name(trimmed: &str) -> Option<String> {
    let rest = trimmed
        .strip_prefix("pub fn ")
        .or_else(|| trimmed.strip_prefix("pub(crate) fn "))
        .or_else(|| trimmed.strip_prefix("pub(super) fn "))
        .or_else(|| trimmed.strip_prefix("fn "))?;
    rest.split_once('(').map(|(name, _)| name.trim().to_owned())
}

fn is_reference_symbol(name: &str) -> bool {
    name.contains("cpu")
        || name.contains("reference")
        || name.contains("oracle")
        || name.contains("parity")
}

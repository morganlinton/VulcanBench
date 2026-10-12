//! Organization-level contract tests for the vyre-driver-wgpu crate.
//!
//! These tests enforce long-term structural contracts without relying on
//! brittle message wording. They may fail when code violates a contract.

use std::collections::HashSet;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// 1. Inline test modules are baselined in vyre-driver-wgpu/src
// ---------------------------------------------------------------------------

/// Organization contract: new tests must live in tests/ directories, not inline
/// source modules. Existing inline `#[cfg(test)]` blocks in vyre-driver-wgpu/src
/// are baselined; any new file with `#[cfg(test)]` is a violation.
#[test]
fn driver_wgpu_inline_test_modules_are_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("src");
    let mut found = HashSet::new();

    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let content = std::fs::read_to_string(&path).unwrap();
                if content.contains("#[cfg(test)]") {
                    let rel = path.strip_prefix(&manifest).unwrap_or(&path);
                    found.insert(rel.display().to_string());
                }
            }
        }
    }

    let known: HashSet<String> = [
        "src/buffer/handle.rs",
        "src/buffer/pool.rs",
        "src/engine/dispatch_scratch.rs",
        "src/engine/multi_gpu.rs",
        "src/engine/persistent.rs",
        "src/engine/streaming/async_copy.rs",
        "src/ext.rs",
        "src/lib.rs",
        "src/lowering/naga_emit/expr.rs",
        "src/lowering/naga_emit/mod.rs",
        "src/lowering/naga_emit/node.rs",
        "src/megakernel/batch.rs",
        "src/megakernel/dispatcher.rs",
        "src/parity_probe.rs",
        "src/pipeline.rs",
        "src/pipeline/bindings_reflection.rs",
        "src/pipeline/disk_cache.rs",
        "src/pipeline/persistent.rs",
        "src/runtime/adapter_caps_probe.rs",
        "src/runtime/cache.rs",
        "src/runtime/cache/pipeline.rs",
        "src/runtime/cache/buffer_pool.rs",
        "src/runtime/device/device.rs",
        "src/runtime/device/selector.rs",
        "src/runtime/indirect.rs",
        "src/runtime/router.rs",
        "src/spirv_backend.rs",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut new_violations: Vec<String> =
        found.into_iter().filter(|v| !known.contains(v)).collect();
    new_violations.sort();

    assert!(
        new_violations.is_empty(),
        "new inline test modules (#[cfg(test)]) are forbidden in vyre-driver-wgpu/src. \
         Add integration tests under tests/ instead. New violations:\n{}",
        new_violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 2. Wildcard pub-use surface is baselined
// ---------------------------------------------------------------------------

/// Scan vyre-driver-wgpu/src for `pub use ...::*` and baseline them.
/// New wildcard re-exports expand API surface unpredictably and are forbidden
/// without explicit approval.
#[test]
fn driver_wgpu_wildcard_pub_use_is_baselined() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("src");
    let mut found = Vec::new();

    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let content = std::fs::read_to_string(&path).unwrap();
                for (line_no, line) in content.lines().enumerate() {
                    let t = line.trim();
                    if t.starts_with("pub use") && t.ends_with("::*;") {
                        let rel = path.strip_prefix(&manifest).unwrap_or(&path);
                        found.push(format!("{}:{} {}", rel.display(), line_no + 1, t));
                    }
                }
            }
        }
    }

    // Currently zero wildcards in vyre-driver-wgpu/src.
    let known: HashSet<String> = HashSet::new();

    let new_violations: Vec<String> = found.into_iter().filter(|v| !known.contains(v)).collect();

    assert!(
        new_violations.is_empty(),
        "new wildcard pub re-exports are forbidden in vyre-driver-wgpu. Violations:\n{}",
        new_violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 3. Agent/skills artifacts stay out of production crate dirs
// ---------------------------------------------------------------------------

/// Organization contract: AGENTS.md, SKILL.md, and .kimi/ directories must not
/// appear in vyre-driver-wgpu production directories (src/ or crate root).
#[test]
fn driver_wgpu_agent_skills_artifacts_stay_out_of_production_dirs() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();

    // Scan src/ directory
    let src_dir = manifest.join("src");
    if src_dir.is_dir() {
        let mut stack = vec![src_dir];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let fname = path.file_name().and_then(|s| s.to_str());
                    if fname == Some("AGENTS.md") || fname == Some("SKILL.md") {
                        let rel = path.strip_prefix(&manifest).unwrap_or(&path);
                        found.push(rel.display().to_string());
                    }
                }
            }
        }
    }

    // Check crate root
    for name in ["AGENTS.md", "SKILL.md"] {
        let path = manifest.join(name);
        if path.exists() {
            let rel = path.strip_prefix(&manifest).unwrap_or(&path);
            found.push(rel.display().to_string());
        }
    }

    // Check for .kimi/ anywhere, excluding tests/benches/examples/target/.internals
    let mut kstack = vec![manifest.clone()];
    while let Some(dir) = kstack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.file_name().and_then(|s| s.to_str()) == Some(".kimi") {
                let rel = path.strip_prefix(&manifest).unwrap_or(&path);
                found.push(rel.display().to_string());
            } else {
                let fname = path.file_name().unwrap().to_string_lossy();
                if fname != "target"
                    && fname != "tests"
                    && fname != "benches"
                    && fname != "examples"
                    && fname != ".internals"
                    && !fname.starts_with('.')
                {
                    kstack.push(path);
                }
            }
        }
    }

    found.sort();

    assert!(
        found.is_empty(),
        "agent/skills artifacts (AGENTS.md, SKILL.md, .kimi/) are forbidden in production dirs. \
         Violations:\n{}",
        found.join("\n")
    );
}

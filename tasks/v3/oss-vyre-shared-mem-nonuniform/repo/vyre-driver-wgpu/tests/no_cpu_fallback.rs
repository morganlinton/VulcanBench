//! Failure-oriented tests: no fake CPU fallback on GPU machines.
//!
//! If a real GPU is present, the backend must bind to it. CPU or
//! "Other" adapters must be rejected with an actionable error.

#![allow(clippy::needless_range_loop)]

use std::path::PathBuf;
use vyre_driver_wgpu::WgpuBackend;

#[test]
fn successful_acquisition_means_non_cpu_adapter() {
    let backend = WgpuBackend::acquire().expect(
        "WgpuBackend::acquire failed on a machine that must have a GPU. \
         Fix: inspect driver visibility and adapter probing; this must not silently skip.",
    );
    let info = backend.adapter_info();
    assert!(
        !matches!(info.device_type, wgpu::DeviceType::Cpu | wgpu::DeviceType::Other),
        "Fix: WgpuBackend must never silently fall back to a CPU adapter on a machine with GPU support. \
         Adapter `{}` has type {:?}.",
        info.name,
        info.device_type
    );
}

#[test]
fn default_acquisition_prefers_discrete_gpu_when_enumerable() {
    let backend = WgpuBackend::acquire().expect(
        "WgpuBackend::acquire failed on a machine that must have a GPU. \
         Fix: inspect driver visibility and adapter probing; this must not silently skip.",
    );
    let adapters = vyre_driver_wgpu::runtime::device::enumerate_adapters();
    let has_discrete = adapters
        .iter()
        .any(|adapter| adapter.device_type == wgpu::DeviceType::DiscreteGpu);
    if has_discrete {
        assert_eq!(
            backend.adapter_info().device_type,
            wgpu::DeviceType::DiscreteGpu,
            "Fix: default WgpuBackend acquisition must prefer an enumerable discrete GPU over CPU/integrated adapters."
        );
    }
}

#[test]
fn backend_error_on_missing_gpu_is_actionable() {
    // If there's no compatible GPU, acquire() must fail with an actionable error.
    // If there IS a GPU, this test trivially passes.
    if let Err(e) = WgpuBackend::acquire() {
        let msg = e.to_string();
        assert!(
            msg.contains("Fix:"),
            "Fix: headless backend error must be actionable, got: {msg}"
        );
        assert!(
            msg.contains("adapter") || msg.contains("GPU") || msg.contains("driver"),
            "Fix: headless error must mention adapters, GPU, or driver so the user knows where to look, got: {msg}"
        );
    }
}

#[test]
fn backend_error_lists_probed_adapters() {
    // When acquisition fails, the error should enumerate what was probed
    // so the user can diagnose driver / visibility issues.
    if let Err(e) = WgpuBackend::acquire() {
        let msg = e.to_string();
        assert!(
            msg.contains("Probed adapters") || msg.contains("no compatible GPU adapter"),
            "Fix: backend error should list probed adapters or clearly state none were found, got: {msg}"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. No fake GPU skip paths in Rust code
// ---------------------------------------------------------------------------

/// Organization contract: code must not contain fake GPU skip paths that
/// silently bypass GPU validation. Any non-GPU feature gate or acquire failure
/// path must fail loudly (panic/assert/unreachable) rather than returning
/// or continuing silently.
#[test]
fn no_fake_gpu_skip_paths_in_tests() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();

    let mut violations = Vec::new();
    let mut dirs = vec![workspace_root.to_path_buf()];

    while let Some(dir) = dirs.pop() {
        // Skip VCS/build directories and generated outputs.
        let dir_name = dir.file_name().and_then(|s| s.to_str());
        if matches!(dir_name, Some(".git") | Some("target") | Some(".cache")) {
            continue;
        }
        if dir_name == Some(".rustup") || dir_name == Some(".cargo") {
            continue;
        }

        let mut entries = Vec::new();
        if let Ok(read_dir) = std::fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                entries.push(entry.path());
            }
        } else {
            continue;
        }
        for path in entries {
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }

            let content = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = content.lines().collect();
            for (idx, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("#[cfg(")
                    && trimmed.contains("not(")
                    && trimmed.contains("gpu")
                    && trimmed.contains(")")
                    || (trimmed.starts_with("#[cfg_attr(")
                        && trimmed.contains("not(")
                        && trimmed.contains("ignore")
                        && trimmed.contains("gpu"))
                {
                    let mut found_loud = false;
                    for j in (idx + 1)..lines.len().min(idx + 8) {
                        let inner = lines[j].trim();
                        if inner.is_empty() || inner == "{" {
                            continue;
                        }
                        if inner.starts_with("panic!(")
                            || inner.starts_with("assert!")
                            || inner.starts_with("assert_eq!")
                            || inner.starts_with("unreachable!")
                        {
                            found_loud = true;
                        }
                        break;
                    }
                    if !found_loud {
                        let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                        violations.push(format!("{}:{} {}", rel.display(), idx + 1, trimmed));
                    }
                }

                // Catch silent skip style:
                // `if WgpuBackend::acquire()..is_err() { return ... }`
                // without a loud panic/assert/return path.
                if (trimmed.starts_with("if")
                    && trimmed.contains("is_err()")
                    && trimmed.contains("acquire"))
                    || (trimmed.starts_with("if let Err") && trimmed.contains("acquire"))
                {
                    let mut found_loud = false;
                    let mut found_silent_return = false;
                    let mut brace_depth =
                        trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
                    for j in (idx + 1)..lines.len().min(idx + 12) {
                        let inner = lines[j].trim();
                        if inner.is_empty() {
                            continue;
                        }
                        if inner.starts_with("panic!(")
                            || inner.starts_with("assert!")
                            || inner.starts_with("assert_eq!")
                            || inner.starts_with("unreachable!")
                        {
                            found_loud = true;
                        }
                        if inner.starts_with("return") {
                            // Any return in an `acquire` failure branch is a silent
                            // skip unless a loud failure path is present.
                            found_silent_return = true;
                        }

                        brace_depth +=
                            inner.matches('{').count() as i32 - inner.matches('}').count() as i32;
                        if brace_depth <= 0 {
                            break;
                        }
                    }
                    if found_silent_return && !found_loud {
                        let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                        violations.push(format!(
                            "{}:{} {}",
                            rel.display(),
                            idx + 1,
                            "silent acquire() failure return path"
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "code must not contain fake GPU skip paths. \
         Every non-GPU path gate or acquire-failure branch in code must fail loudly. \
         Violations:\n{}",
        violations.join("\n")
    );
}

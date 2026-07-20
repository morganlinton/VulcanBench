use std::fs;
use std::process::Command;

#[test]
fn flags_skipped_no_gpu_test_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-driver-cuda/tests");
    fs::create_dir_all(&src).expect("create tests");
    let denied_message = ["skipped", ": no ", "GPU"].concat();
    let fixture = format!(
        r#"#[test]
fn parity() {{
    eprintln!("{denied_message}");
}}
"#
    );
    fs::write(src.join("parity.rs"), fixture).expect("write fixture");

    let violations = vyre_lints::run_gpu_skip_guards(&[src.as_path()]).expect("gpu skip scan");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].kind, vyre_lints::ViolationKind::GpuSkipGuard);
    assert!(violations[0].message.contains("fail loudly"));
}

#[test]
fn flags_gpu_unavailable_cpu_fallback() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-driver-wgpu/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("adapter.rs"),
        r#"pub fn dispatch() {
    if gpu_unavailable() {
        tracing::warn!("GPU unavailable; using CPU fallback");
    }
}
"#,
    )
    .expect("write fixture");

    let violations = vyre_lints::run_gpu_skip_guards(&[src.as_path()]).expect("gpu skip scan");
    assert_eq!(violations.len(), 1);
    assert!(violations[0].message.contains("silently skips"));
}

#[test]
fn flags_no_gpu_success_return() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-runtime/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("probe.rs"),
        r#"pub fn validate() -> anyhow::Result<()> {
    if no_gpu_adapter() {
        return Ok(());
    }
    Ok(())
}
"#,
    )
    .expect("write fixture");

    let violations = vyre_lints::run_gpu_skip_guards(&[src.as_path()]).expect("gpu skip scan");
    assert_eq!(violations.len(), 1);
}

#[test]
fn permits_loud_no_gpu_diagnostic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-driver-cuda/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("device.rs"),
        r#"pub fn diagnostic() -> &'static str {
    "no GPU adapter found. Fix: inspect CUDA probe logs and fail loudly."
}
"#,
    )
    .expect("write fixture");

    let violations = vyre_lints::run_gpu_skip_guards(&[src.as_path()]).expect("gpu skip scan");
    assert!(violations.is_empty());
}

#[test]
fn permits_do_not_skip_gpu_tests_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-driver-cuda/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("device.rs"),
        r#"pub const GPU_TEST_POLICY: &str = "do not skip GPU tests on this fleet";
"#,
    )
    .expect("write fixture");

    let violations = vyre_lints::run_gpu_skip_guards(&[src.as_path()]).expect("gpu skip scan");
    assert!(violations.is_empty());
}

#[test]
fn cli_rejects_missing_gpu_skip_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing-gpu-root");
    let output = Command::new(env!("CARGO_BIN_EXE_vyre-lints"))
        .arg("--check-gpu-skip-guards")
        .arg("--gpu-skip-root")
        .arg(&missing)
        .output()
        .expect("run vyre-lints");

    assert!(
        !output.status.success(),
        "missing GPU roots must fail instead of shrinking coverage"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("GPU skip guard root not found"),
        "missing-root diagnostic must be actionable, got: {stderr}"
    );
}

#[test]
fn cli_default_gpu_skip_roots_are_vyre_owned() {
    let dir = tempfile::tempdir().expect("tempdir");
    for root in [
        "vyre-driver-cuda/src",
        "vyre-driver-cuda/tests",
        "vyre-driver-wgpu/src",
        "vyre-driver-wgpu/tests",
        "vyre-runtime/src",
    ] {
        fs::create_dir_all(dir.path().join(root)).expect("create default GPU skip root");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_vyre-lints"))
        .arg("--check-gpu-skip-guards")
        .arg("--workspace-root")
        .arg(dir.path())
        .output()
        .expect("run vyre-lints");

    assert!(
        output.status.success(),
        "Vyre default GPU skip roots must not require external consumer checkouts: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).as_ref(),
        "",
        "successful default GPU skip-root scan must not emit diagnostics"
    );
}

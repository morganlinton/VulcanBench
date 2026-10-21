//! Executable contracts for the generated CUDA FFI launcher module.

use std::fs;
use std::process::Command;

#[test]
fn cuda_ffi_template_compiles_as_emitted_launcher_module() {
    let temp = tempfile::tempdir()
        .expect("Fix: tempdir must be available for generated CUDA FFI compile contract.");
    let src_dir = temp.path().join("src");
    fs::create_dir_all(&src_dir).expect("Fix: temp src dir must be creatable.");
    fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/templates/cuda_ffi.rs.tmpl"),
        src_dir.join("lib.rs"),
    )
    .expect("Fix: generated CUDA FFI template must be readable.");
    let emitted = fs::read_to_string(src_dir.join("lib.rs"))
        .expect("Fix: generated CUDA FFI template copy must be readable.");
    assert!(
        emitted.contains("cuModuleLoadData") && emitted.contains("cuLaunchKernel"),
        "Fix: generated CUDA FFI template must expose module loading and kernel launch bindings."
    );
    fs::write(
        temp.path().join("Cargo.toml"),
        r#"[package]
name = "vyre-cuda-ffi-template-check"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
libc = "0.2"
"#,
    )
    .expect("Fix: temp Cargo.toml must be writable.");

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .arg("check")
        .arg("--manifest-path")
        .arg(temp.path().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", temp.path().join("target"))
        .env("CARGO_BUILD_JOBS", "1")
        .output()
        .expect("Fix: cargo check must launch for generated CUDA FFI template contract.");

    assert!(
        output.status.success(),
        "Fix: generated CUDA FFI template must compile as a standalone emitted launcher module.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

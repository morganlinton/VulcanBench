use std::fs;
use std::process::Command;

#[test]
fn flags_reference_eval_in_production_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-frontend-c/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("dispatch.rs"),
        "pub fn bad() { let _ = vyre_reference::reference_eval(&program, &values); }\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert_eq!(violations.len(), 1);
    assert!(violations[0]
        .message
        .contains("production CPU/reference fallback"));
}

#[test]
fn cli_rejects_missing_production_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing-src");
    let output = Command::new(env!("CARGO_BIN_EXE_vyre-lints"))
        .arg("--check-production-cpu-fallbacks")
        .arg("--production-root")
        .arg(&missing)
        .output()
        .expect("run vyre-lints");

    assert!(
        !output.status.success(),
        "missing production roots must fail, not silently shrink scan coverage"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("production root not found"),
        "missing-root diagnostic must be actionable, got: {stderr}"
    );
}

#[test]
fn cli_default_production_roots_are_vyre_owned_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    for root in [
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
    ] {
        fs::create_dir_all(dir.path().join(root)).expect("create default production root");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_vyre-lints"))
        .arg("--check-production-cpu-fallbacks")
        .arg("--workspace-root")
        .arg(dir.path())
        .output()
        .expect("run vyre-lints");

    assert!(
        output.status.success(),
        "Vyre default production roots must not require external consumer checkouts: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).as_ref(),
        "",
        "successful default production-root scan must not emit diagnostics"
    );
}

#[test]
fn permits_reference_eval_inside_cfg_test_module() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-frontend-c/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("dispatch.rs"),
        "#[cfg(test)]\nmod tests {\n    fn oracle() { let _ = vyre_reference::reference_eval(&program, &values); }\n}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn permits_cfg_test_module_with_intervening_attributes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-frontend-c/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("dispatch.rs"),
        "#[cfg(test)]\n#[allow(clippy::unwrap_used)]\nmod tests {\n    fn oracle() { let _ = vyre_reference::reference_eval(&program, &values); }\n}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn permits_reference_eval_under_tests_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tests = dir.path().join("vyre-frontend-c/tests");
    fs::create_dir_all(&tests).expect("create tests");
    fs::write(
        tests.join("oracle.rs"),
        "fn oracle() { let _ = vyre_reference::reference_eval(&program, &values); }\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[dir.path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn permits_explicit_cpu_oracle_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("external-consumer/src/dominators");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("cpu_oracle.rs"),
        "pub(crate) fn compute_cpu() {}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[dir.path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn permits_explicit_cpu_fallback_reachability_validator() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-self-substrate/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("cpu_fallback_reachability.rs"),
        "pub fn validate_fallback_reachability() {}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[dir.path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn permits_multiline_cpu_ref_under_cpu_parity_cfg() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-primitives/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("predicate.rs"),
        "#[cfg(any(test, feature = \"cpu-parity\"))]\npub fn cpu_ref(\n    input: &[u32],\n) -> Vec<u32> {\n    input.to_vec()\n}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn flags_cpu_helper_definition_in_production_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-primitives/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("utf8.rs"),
        "fn cpu_class_at(input: &[u8]) -> u32 { input.len() as u32 }\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert_eq!(violations.len(), 1);
    assert!(violations[0]
        .message
        .contains("production CPU/reference helper definition"));
}

#[test]
fn flags_suffix_cpu_helper_definition_in_production_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-primitives/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("semiring.rs"),
        "pub fn semiring_gemm_cpu_into(input: &[u32], out: &mut Vec<u32>) { out.extend_from_slice(input); }\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert_eq!(violations.len(), 1);
    assert!(violations[0]
        .message
        .contains("production CPU/reference helper definition"));
}

#[test]
fn flags_cpu_module_export_in_production_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-self-substrate/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(src.join("lib.rs"), "pub mod cpu_fallback_reachability;\n").expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert_eq!(violations.len(), 1);
    assert!(violations[0]
        .message
        .contains("production CPU/reference helper definition"));
}

#[test]
fn flags_public_cpu_reexport_in_production_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-core/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(src.join("lib.rs"), "pub use vyre_foundation::cpu_op;\n").expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert_eq!(violations.len(), 1);
    assert!(violations[0]
        .message
        .contains("production CPU/reference fallback"));
}

#[test]
fn permits_pub_crate_test_module() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-libs/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("atomic.rs"),
        "#[cfg(test)]\npub(crate) mod testutil {\n    pub(crate) fn run(program: &Program) {\n        let _ = vyre_reference::reference_eval(program, &[]);\n    }\n}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn permits_file_level_cpu_parity_module() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("external-consumer/src/oracle");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("bitset.rs"),
        "#![cfg(any(test, feature = \"cpu-parity\"))]\n\npub fn oracle(input: &[u32]) -> Vec<u32> {\n    cpu_ref(input)\n}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn ignores_reference_eval_in_doc_comments() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-libs/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("dispatch.rs"),
        "/// Tests may call vyre_reference::reference_eval, production may not.\npub fn ok() {}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

#[test]
fn reports_external_consumer_production_cpu_reference_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("external-consumer/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("dispatch.rs"),
        "pub fn dispatch() { cpu_ref(); }\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[src.as_path()]).expect("fallback scan");
    assert_eq!(violations.len(), 1);
    assert!(violations[0]
        .file
        .ends_with("external-consumer/src/dispatch.rs"));
    assert!(violations[0].message.contains("cpu_ref("));
}

#[test]
fn allows_external_consumer_cpu_reference_only_in_parity_tests() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tests = dir.path().join("external-consumer/tests");
    fs::create_dir_all(&tests).expect("create tests");
    fs::write(
        tests.join("dispatch_parity.rs"),
        "#[test]\nfn parity() { cpu_ref(); }\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_production_cpu_fallbacks(&[dir.path()]).expect("fallback scan");
    assert!(violations.is_empty());
}

use std::fs;
use std::process::Command;

#[test]
fn flags_same_module_basename_across_authority_roots() {
    let dir = tempfile::tempdir().expect("tempdir");
    let primitive = dir.path().join("vyre-primitives/src/graph");
    let substrate = dir.path().join("vyre-self-substrate/src");
    fs::create_dir_all(&primitive).expect("create primitive root");
    fs::create_dir_all(&substrate).expect("create substrate root");
    fs::write(
        primitive.join("toposort.rs"),
        "pub fn build_toposort() {}\nfn primitive_only() {}\n",
    )
    .expect("write primitive module");
    fs::write(
        substrate.join("toposort.rs"),
        "pub fn build_toposort() {}\nfn substrate_only() {}\n",
    )
    .expect("write substrate module");

    let violations = vyre_lints::run_module_forks(&[primitive.as_path(), substrate.as_path()])
        .expect("module fork scan");

    assert_eq!(violations.len(), 2);
    assert!(violations
        .iter()
        .all(|v| v.kind == vyre_lints::ViolationKind::ModuleFork));
    assert!(violations
        .iter()
        .all(|v| v.message.contains("toposort.rs") && v.message.contains("Fix:")));
    assert!(violations
        .iter()
        .all(|v| v.message.contains("shared Rust symbols: build_toposort")));
}

#[test]
fn ignores_generic_rust_module_basenames() {
    let dir = tempfile::tempdir().expect("tempdir");
    let left = dir.path().join("left");
    let right = dir.path().join("right");
    fs::create_dir_all(&left).expect("create left root");
    fs::create_dir_all(&right).expect("create right root");
    for name in [
        "lib.rs", "main.rs", "mod.rs", "tests.rs", "error.rs", "types.rs",
    ] {
        fs::write(left.join(name), "pub fn left() {}\n").expect("write left module");
        fs::write(right.join(name), "pub fn right() {}\n").expect("write right module");
    }

    let violations =
        vyre_lints::run_module_forks(&[left.as_path(), right.as_path()]).expect("module scan");

    assert!(violations.is_empty());
}

#[test]
fn ignores_same_basename_repeated_inside_one_authority_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("vyre-self-substrate/src/graph");
    fs::create_dir_all(root.join("left")).expect("create left module");
    fs::create_dir_all(root.join("right")).expect("create right module");
    fs::write(root.join("left/dispatch.rs"), "pub fn left_dispatch() {}\n")
        .expect("write left dispatch");
    fs::write(
        root.join("right/dispatch.rs"),
        "pub fn right_dispatch() {}\n",
    )
    .expect("write right dispatch");

    let violations = vyre_lints::run_module_forks(&[root.as_path()]).expect("module scan");

    assert!(violations.is_empty());
}

#[test]
fn cli_rejects_missing_module_fork_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing-root");
    let output = Command::new(env!("CARGO_BIN_EXE_vyre-lints"))
        .arg("--check-module-forks")
        .arg("--module-fork-root")
        .arg(&missing)
        .output()
        .expect("run vyre-lints");

    assert!(
        !output.status.success(),
        "missing module fork roots must fail, not shrink scan coverage"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("module fork root not found"),
        "missing-root diagnostic must be actionable, got: {stderr}"
    );
}

use std::fs;
use std::process::Command;

#[test]
fn flags_consumer_name_in_platform_rust_doc_comment() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-libs/src/security");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("mod.rs"),
        "//! surgec-facing op surface.\n\npub fn taint_flow() {}\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_consumer_coupling(&[src.as_path()]).expect("consumer coupling scan");
    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0].kind,
        vyre_lints::ViolationKind::ConsumerCoupling
    );
    assert!(violations[0].message.contains("surgec"));
    assert!(violations[0].message.contains("Fix:"));
}

#[test]
fn flags_consumer_name_in_current_markdown() {
    let dir = tempfile::tempdir().expect("tempdir");
    let docs = dir.path().join("docs");
    fs::create_dir_all(&docs).expect("create docs");
    fs::write(
        docs.join("ARCHITECTURE.md"),
        "The foundation scheduler owns weir-dataflow phases.\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_consumer_coupling(&[docs.as_path()]).expect("consumer coupling scan");
    assert_eq!(violations.len(), 1);
    assert!(violations[0].message.contains("weir"));
}

#[test]
fn ignores_archived_markdown_consumer_history() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("docs/archive");
    fs::create_dir_all(&archive).expect("create archive");
    fs::write(
        archive.join("OLD_PLAN.md"),
        "Historical notes can mention keyhog, gossan, surgec, and weir.\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_consumer_coupling(&[dir.path()]).expect("consumer coupling scan");
    assert!(violations.is_empty());
}

#[test]
fn ignores_consumer_name_in_non_comment_rust_code() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-libs/src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("scan.rs"),
        "pub fn keyhog_counter() -> usize { 1 }\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_consumer_coupling(&[src.as_path()]).expect("consumer coupling scan");
    assert!(violations.is_empty());
}

#[test]
fn flags_consumer_name_in_platform_rust_string_literal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-libs/src/security");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("diagnostic.rs"),
        "pub fn diagnostic() -> &'static str { \"keyhog scanner path\" }\n",
    )
    .expect("write fixture");

    let violations =
        vyre_lints::run_consumer_coupling(&[src.as_path()]).expect("consumer coupling scan");
    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0].kind,
        vyre_lints::ViolationKind::ConsumerCoupling
    );
    assert!(violations[0].message.contains("string literal"));
    assert!(violations[0].message.contains("keyhog"));
}

#[test]
fn flags_consumer_name_in_platform_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("vyre-libs/src/security/surgec_bridge");
    fs::create_dir_all(&src).expect("create src");
    fs::write(src.join("mod.rs"), "pub fn neutral_code() {}\n").expect("write fixture");

    let violations =
        vyre_lints::run_consumer_coupling(&[dir.path()]).expect("consumer coupling scan");
    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0].kind,
        vyre_lints::ViolationKind::ConsumerCoupling
    );
    assert!(violations[0].message.contains("path"));
    assert!(violations[0].message.contains("surgec"));
}

#[test]
fn cli_rejects_missing_consumer_coupling_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing-docs");
    let output = Command::new(env!("CARGO_BIN_EXE_vyre-lints"))
        .arg("--check-consumer-coupling")
        .arg("--consumer-root")
        .arg(&missing)
        .output()
        .expect("run vyre-lints");

    assert!(
        !output.status.success(),
        "missing consumer coupling roots must fail, not shrink scan coverage"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("consumer coupling root not found"),
        "missing-root diagnostic must be actionable, got: {stderr}"
    );
}

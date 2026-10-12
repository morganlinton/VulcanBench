//! G6  -  Per-commit snapshot persistence.
//!
//! Verifies that `execute_suite` persists a snapshot JSON under
//! `snapshots/<commit>.json` and that the file is parseable back into
//! a `ReportSchema`.

#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_snapshot_written_on_run() {
    let mut config = RunConfig::default();
    config.warmup_samples = 1;
    config.measured_samples = Some(30);
    config.determinism_runs = 1;
    config.snapshot_on_pass = true;
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];

    let registry = vyre_bench::registry::collect_all();
    let report = execute_suite(&registry, SuiteKind::Smoke, &config);

    // Report must contain a git commit
    let commit = report
        .git
        .get("commit")
        .expect("report must contain git.commit");

    let snapshot_path = std::path::Path::new("snapshots").join(format!("{commit}.json"));
    assert!(
        snapshot_path.exists(),
        "Snapshot file must be written at {}",
        snapshot_path.display()
    );
    assert!(
        snapshot_path.metadata().unwrap().len() > 0,
        "Snapshot file must be non-empty"
    );

    // Verify the snapshot is parseable back into ReportSchema
    let contents = std::fs::read_to_string(&snapshot_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert!(parsed["cases"].is_array(), "Snapshot must contain cases");
    assert!(
        parsed["summary"].is_object(),
        "Snapshot must contain summary"
    );
    assert!(parsed["git"].is_object(), "Snapshot must contain git info");
    std::fs::remove_file(snapshot_path).unwrap();
}

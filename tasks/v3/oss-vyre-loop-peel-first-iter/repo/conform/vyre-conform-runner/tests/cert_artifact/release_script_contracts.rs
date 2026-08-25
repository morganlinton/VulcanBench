#[test]
fn release_scripts_make_sharded_conformance_certificate_load_bearing() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("Fix: test manifest must live under conform/vyre-conform-runner");
    let prove = std::fs::read_to_string(repo.join("scripts/prove-release-shards.sh"))
        .expect("Fix: sharded release proof helper must be readable");
    assert!(
        prove.contains("vyre_select_cargo_runner"),
        "Fix: sharded release proof must use the shared OOM-safe cargo runner selector."
    );
    assert!(
        prove.contains("metadata --no-deps --format-version 1")
            && prove.contains("target_directory"),
        "Fix: release proof must discover Cargo's configured target directory instead of assuming ./target."
    );
    assert!(
        prove.contains("VYRE_RELEASE_SHARD_WORKERS") && prove.contains("wait -n"),
        "Fix: release proof shards must run through a bounded parallel worker pool."
    );
    assert!(
        prove.contains("\"$RUNNER_BIN\" \"${prove_args[@]}\"")
            && prove.contains("\"$RUNNER_BIN\" \"${merge_args[@]}\""),
        "Fix: release proof must build vyre-conform-runner once, then use the binary for prove and merge."
    );

    let signoff =
        std::fs::read_to_string(repo.join("scripts/check_signed_conformance_certificate.sh"))
            .expect("Fix: signed conformance gate must be readable");
    assert!(
        signoff.contains("scripts/prove-release-shards.sh")
            && signoff.contains("VYRE_RELEASE_BACKEND")
            && signoff.contains("VYRE_RELEASE_SHARDS"),
        "Fix: signed conformance gate must execute sharded all-backend proof, not a narrow one-off test."
    );

    let final_launch = std::fs::read_to_string(repo.join("scripts/final-launch.sh"))
        .expect("Fix: final launch script must be readable");
    assert!(
        final_launch.contains("scripts/prove-release-shards.sh")
            && final_launch.contains("release/evidence/conformance/release-all-backends-certificate.json")
            && final_launch.contains("prove sharded all-backend conformance certificate"),
        "Fix: final launch must make the merged sharded certificate load-bearing release evidence before publish."
    );
}


use super::*;

#[test]
fn prove_merges_live_gpu_certificate_shards() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shard_a = dir.path().join("live-shard-a.json");
    let shard_b = dir.path().join("live-shard-b.json");
    let merged = dir.path().join("live-merged.json");
    let selected_backend = selected_backend_override();

    for (shard, path) in [("0/64", &shard_a), ("1/64", &shard_b)] {
        let mut command = Command::new("cargo");
        command
            .env("VYRE_CONFORM_PROOF_WORKERS", "16")
            .args([
                "run",
                "-p",
                "vyre-conform-runner",
                "--features",
                "gpu",
                "--quiet",
                "--",
                "prove",
                "--shard",
                shard,
            ]);
        if let Some(backend) = selected_backend.as_deref() {
            command.args(["--backend", backend]);
        }
        let status = command
            .arg("--out")
            .arg(path)
            .status()
            .expect("Fix: cargo must be available in PATH");
        assert!(
            status.success(),
            "Fix: live GPU proof shard {shard} must emit a signed certificate."
        );
    }

    let status = Command::new("cargo")
        .args([
            "run",
            "-p",
            "vyre-conform-runner",
            "--no-default-features",
            "--quiet",
            "--",
            "merge",
            "--out",
        ])
        .arg(&merged)
        .arg(&shard_a)
        .arg(&shard_b)
        .status()
        .expect("Fix: cargo must be available in PATH");
    assert!(
        status.success(),
        "Fix: merge must accept live signed GPU proof shards."
    );

    let merged_json =
        std::fs::read_to_string(&merged).expect("Fix: merge must write a readable artifact");
    let parsed: Value =
        serde_json::from_str(&merged_json).expect("Fix: merged artifact must be valid JSON");
    assert_eq!(
        parsed["backend_id"].as_str(),
        Some("merged"),
        "Fix: merged live certificate must use the aggregate backend id."
    );
    let pairs = parsed["pairs"]
        .as_array()
        .expect("Fix: merged live certificate must carry pair results.");
    assert!(
        pairs.len() >= if selected_backend.is_some() { 1 } else { 30 },
        "Fix: merged live certificate shards must cover multiple real GPU/backend pairs, got {}.",
        pairs.len()
    );
    let mut backends = std::collections::BTreeSet::new();
    for pair in pairs {
        let backend = pair["backend_id"]
            .as_str()
            .expect("Fix: merged live pair must carry backend_id");
        backends.insert(backend.to_string());
        assert_eq!(
            pair["passed"].as_bool(),
            Some(true),
            "Fix: merged live certificate must not contain failing pairs: {pair}"
        );
    }
    if let Some(selected) = selected_backend.as_deref() {
        assert_eq!(
            backends,
            [selected.to_string()].into_iter().collect(),
            "Fix: VYRE_BACKEND={selected} merged shards must preserve only the selected backend."
        );
    } else {
        for required in ["cuda", "wgpu", "cpu-ref"] {
            assert!(
                backends.contains(required),
                "Fix: merged live GPU shards must preserve backend `{required}`."
            );
        }
    }
    assert_eq!(
        parsed["plan"]["pair_count"].as_u64(),
        Some(pairs.len() as u64),
        "Fix: merged live plan pair_count must match carried pairs."
    );
    verify_certificate_signature(&parsed);
}

#[test]
fn release_shard_script_keeps_prove_merge_backend_and_worker_controls() {
    let script = include_str!("../../../../scripts/prove-release-shards.sh");
    for required in [
        "VYRE_RELEASE_SHARDS",
        "VYRE_RELEASE_BACKEND",
        "VYRE_CONFORM_PROOF_WORKERS",
        "prove",
        "--shard",
        "--backend",
        "merge",
        "--no-default-features",
        "merged.json",
    ] {
        assert!(
            script.contains(required),
            "Fix: release shard automation must keep `{required}` wired so GPU proof evidence remains reproducible."
        );
    }
}

#[test]
fn plan_emits_deterministic_shard_manifest_without_dispatch() {
    let out = tempfile::NamedTempFile::new().expect("tempfile");
    let status = Command::new("cargo")
        .args([
            "run",
            "-p",
            "vyre-conform-runner",
            "--features",
            "gpu",
            "--quiet",
            "--",
            "plan",
            "--shard",
            "0/64",
            "--out",
        ])
        .arg(out.path())
        .status()
        .expect("Fix: cargo must be available in PATH");
    assert!(
        status.success(),
        "Fix: proof planning must not acquire or dispatch a backend; it should only emit the selected executable shard manifest."
    );

    let plan_json =
        std::fs::read_to_string(out.path()).expect("Fix: plan must produce a readable artifact");
    let parsed: Value =
        serde_json::from_str(&plan_json).expect("Fix: plan artifact must be valid JSON");
    let plan = parsed["plan"]
        .as_object()
        .expect("Fix: plan artifact must include a plan object");
    let selection = plan["selection"]
        .as_object()
        .expect("Fix: plan summary must include selection metadata");
    assert_eq!(
        selection["shard_index"].as_u64(),
        Some(0),
        "Fix: plan must preserve the selected shard index."
    );
    assert_eq!(
        selection["shard_count"].as_u64(),
        Some(64),
        "Fix: plan must preserve the selected shard count."
    );
    assert!(
        plan["catalog_hash"].as_str().is_some(),
        "Fix: plan must carry a full-catalog hash shared by every shard."
    );
    assert!(
        plan["execution_hash"].as_str().is_some(),
        "Fix: plan must carry the selected shard execution hash."
    );
    assert!(
        matches!(parsed["backends"].as_array(), Some(backends) if !backends.is_empty()),
        "Fix: plan must name every backend selected for this shard."
    );
    assert!(
        matches!(parsed["ops"].as_array(), Some(ops) if !ops.is_empty()),
        "Fix: plan must name every op selected for this shard."
    );
}


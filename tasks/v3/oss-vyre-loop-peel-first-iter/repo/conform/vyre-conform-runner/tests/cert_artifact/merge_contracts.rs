use super::*;

#[test]
fn merge_verifies_and_resigns_disjoint_certificate_shards() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shard_a = dir.path().join("shard-a.json");
    let shard_b = dir.path().join("shard-b.json");
    let merged = dir.path().join("merged.json");
    write_signed_shard(
        &shard_a,
        "catalog-hash",
        "execution-a",
        "program-a",
        serde_json::json!([
            {
                "op_id": "vyre-test::a",
                "backend_id": "cuda",
                "passed": true,
                "message": "a matched"
            }
        ]),
    );
    write_signed_shard(
        &shard_b,
        "catalog-hash",
        "execution-b",
        "program-b",
        serde_json::json!([
            {
                "op_id": "vyre-test::b",
                "backend_id": "cuda",
                "passed": true,
                "message": "b matched"
            }
        ]),
    );

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
        "Fix: merge must accept signed disjoint shards"
    );

    let merged_json =
        std::fs::read_to_string(&merged).expect("Fix: merge must write a readable artifact");
    let parsed: Value =
        serde_json::from_str(&merged_json).expect("Fix: merged artifact must be valid JSON");
    assert_eq!(
        parsed["backend_id"].as_str(),
        Some("merged"),
        "Fix: aggregate certificate must name the merged backend set."
    );
    assert_eq!(
        parsed["plan"]["pair_count"].as_u64(),
        Some(2),
        "Fix: merged plan must count all shard pairs."
    );
    assert_eq!(
        parsed["plan"]["selection"]["shard_count"].as_u64(),
        Some(2),
        "Fix: merged plan must preserve source shard count."
    );
    assert_eq!(
        parsed["pairs"].as_array().map(Vec::len),
        Some(2),
        "Fix: merged certificate must carry all disjoint pairs."
    );
    verify_certificate_signature(&parsed);
}

#[test]

fn merge_rejects_tampered_certificate_shard() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shard = dir.path().join("tampered.json");
    let merged = dir.path().join("merged.json");
    write_signed_shard(
        &shard,
        "catalog-hash",
        "execution-a",
        "program-a",
        serde_json::json!([
            {
                "op_id": "vyre-test::a",
                "backend_id": "cuda",
                "passed": true,
                "message": "a matched"
            }
        ]),
    );
    let mut parsed: Value = serde_json::from_str(
        &std::fs::read_to_string(&shard).expect("Fix: shard should be readable"),
    )
    .expect("Fix: shard should parse");
    parsed["pairs"][0]["message"] = Value::String("tampered after signing".to_string());
    std::fs::write(
        &shard,
        serde_json::to_string_pretty(&parsed).expect("Fix: tampered shard should serialize"),
    )
    .expect("Fix: tampered shard should be writable");

    let output = Command::new("cargo")
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
        .arg(&shard)
        .output()
        .expect("Fix: cargo must be available in PATH");
    assert!(
        !output.status.success(),
        "Fix: merge must reject a shard whose signed body was tampered."
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("signature verification failed"),
        "Fix: merge must report signature verification failure; stderr={stderr}"
    );
    assert!(
        !merged.exists(),
        "Fix: merge must not emit an aggregate from a tampered shard."
    );
}


use super::*;

#[test]
fn prove_emits_signed_certificate_on_gpu_build() {
    let out = tempfile::NamedTempFile::new().expect("tempfile");
    let selected_backend = selected_backend_override();
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
        ]);
    if let Some(backend) = selected_backend.as_deref() {
        command.args(["--backend", backend]);
    }
    let status = command
        .arg("--out")
        .arg(out.path())
        .status()
        .expect("Fix: cargo must be available in PATH");
    assert!(
        status.success(),
        "TEST-034: `cargo run -p vyre-conform-runner --features gpu -- prove --out <path>` must succeed on a live GPU"
    );

    let cert =
        std::fs::read_to_string(out.path()).expect("Fix: prove must produce a readable artifact");
    let parsed: Value = serde_json::from_str(&cert).expect("TEST-034: artifact must be valid JSON");
    for required in &[
        "wire_format_version",
        "program_hash",
        "backend_id",
        "plan",
        "signature",
        "public_key",
        "pairs",
    ] {
        assert!(
            parsed.get(required).is_some(),
            "TEST-034: certificate missing required field `{required}`"
        );
    }
    let pairs = parsed
        .get("pairs")
        .and_then(|v| v.as_array())
        .expect("TEST-034: certificate must embed a pairs array");
    let plan = parsed
        .get("plan")
        .and_then(|v| v.as_object())
        .expect("Fix: signed certificate must embed an executable proof plan summary");
    for required in &[
        "backend_count",
        "op_count",
        "pair_count",
        "witness_case_count",
        "catalog_hash",
        "execution_hash",
        "selection",
    ] {
        assert!(
            plan.get(*required).is_some(),
            "Fix: proof plan summary missing `{required}`"
        );
    }
    assert!(
        !pairs.is_empty(),
        "TEST-034: pairs array must include every registered (backend, op) witness"
    );
    for pair in pairs {
        let passed = pair
            .get("passed")
            .and_then(|v| v.as_bool())
            .expect("TEST-034: every pair must carry a boolean `passed` field");
        assert!(
            passed,
            "TEST-034: prove emitted a certificate containing a failing pair: {pair}"
        );
    }

    let mut by_backend =
        std::collections::BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    for pair in pairs {
        let backend = pair["backend_id"]
            .as_str()
            .expect("Fix: certificate pair must carry backend_id")
            .to_string();
        let op = pair["op_id"]
            .as_str()
            .expect("Fix: certificate pair must carry op_id")
            .to_string();
        by_backend.entry(backend).or_default().insert(op);
    }
    if let Some(selected) = selected_backend.as_deref() {
        let ops = by_backend.get(selected).unwrap_or_else(|| {
            panic!("Fix: signed certificate must include selected backend `{selected}`.")
        });
        assert!(
            !ops.is_empty(),
            "Fix: signed certificate backend `{selected}` must cover executable registry pairs."
        );
        assert_eq!(
            by_backend.len(),
            1,
            "Fix: VYRE_BACKEND={selected} must restrict prove to the selected backend."
        );
    } else {
        for required_backend in ["cuda", "wgpu", "cpu-ref"] {
            let ops = by_backend.get(required_backend).unwrap_or_else(|| {
                panic!("Fix: signed certificate must include backend `{required_backend}`.")
            });
            assert!(
                ops.len() >= 300,
                "Fix: signed certificate backend `{required_backend}` must cover the catalog-scale executable registry, got {} ops.",
                ops.len()
            );
        }
        let cuda_ops = by_backend
            .get("cuda")
            .expect("Fix: signed certificate must include cuda ops");
        for backend in ["wgpu", "cpu-ref"] {
            let ops = by_backend
                .get(backend)
                .unwrap_or_else(|| panic!("Fix: signed certificate must include `{backend}` ops."));
            assert_eq!(
                ops, cuda_ops,
                "Fix: signed certificate backend `{backend}` must cover the same executable op set as cuda."
            );
        }
    }

    let signature_hex = parsed["signature"]
        .as_str()
        .expect("Fix: signed certificate must carry signature");
    let public_key_hex = parsed["public_key"]
        .as_str()
        .expect("Fix: signed certificate must carry public_key");
    let signature_bytes =
        hex::decode(signature_hex).expect("Fix: certificate signature must be hex");
    let public_key_bytes =
        hex::decode(public_key_hex).expect("Fix: certificate public key must be hex");
    let signature = Signature::from_slice(&signature_bytes)
        .expect("Fix: certificate signature must be a 64-byte Ed25519 signature");
    let public_key_array: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .expect("Fix: certificate public key must be 32 bytes");
    let verifying_key = VerifyingKey::from_bytes(&public_key_array)
        .expect("Fix: certificate public key must be a valid Ed25519 verifying key");
    let signable = serde_json::json!({
        "wire_format_version": parsed["wire_format_version"].clone(),
        "program_hash": parsed["program_hash"].clone(),
        "backend_id": parsed["backend_id"].clone(),
        "plan": parsed["plan"].clone(),
        "pairs": parsed["pairs"].clone(),
    });
    let signable_bytes =
        serde_json::to_vec(&signable).expect("Fix: certificate signable body must serialize");
    verifying_key
        .verify(&signable_bytes, &signature)
        .expect("Fix: certificate Ed25519 signature must verify over the canonical prove body");
}

#[test]
fn prove_emits_signed_cuda_release_certificate_on_gpu_build() {
    let out = tempfile::NamedTempFile::new().expect("tempfile");
    let selected_backend = selected_backend_override().unwrap_or_else(|| "cuda".to_string());
    let status = Command::new("cargo")
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
            "--backend",
        ])
        .arg(&selected_backend)
        .args([
            "--out",
        ])
        .arg(out.path())
        .status()
        .expect("Fix: cargo must be available in PATH");
    assert!(
        status.success(),
        "Fix: selected GPU release backend `{selected_backend}` must produce a signed certificate on a live GPU."
    );

    let cert =
        std::fs::read_to_string(out.path()).expect("Fix: prove must produce a readable artifact");
    let parsed: Value = serde_json::from_str(&cert).expect("Fix: artifact must be valid JSON");
    let pairs = parsed["pairs"]
        .as_array()
        .expect("Fix: CUDA certificate must carry executable parity pairs.");
    assert!(
        selected_backend != "cuda" || pairs.len() >= 300,
        "Fix: CUDA release certificate must cover the catalog-scale executable registry, got {} pairs.",
        pairs.len()
    );
    assert!(
        selected_backend == "cuda" || !pairs.is_empty(),
        "Fix: selected backend `{selected_backend}` release certificate must carry executable pairs."
    );
    for pair in pairs {
        assert_eq!(
            pair["backend_id"].as_str(),
            Some(selected_backend.as_str()),
            "Fix: selected release certificate must be filtered to `{selected_backend}`."
        );
        assert_eq!(
            pair["passed"].as_bool(),
            Some(true),
            "Fix: CUDA release certificate must not contain failing pairs: {pair}"
        );
    }
    assert_eq!(
        parsed["plan"]["backend_count"].as_u64(),
        Some(1),
        "Fix: CUDA release certificate must prove exactly one selected backend."
    );
    assert_eq!(
        parsed["plan"]["selection"]["selected_backend_count"].as_u64(),
        Some(1),
        "Fix: CUDA release certificate selection metadata must stay aligned with --backend cuda."
    );
    verify_certificate_signature(&parsed);
}


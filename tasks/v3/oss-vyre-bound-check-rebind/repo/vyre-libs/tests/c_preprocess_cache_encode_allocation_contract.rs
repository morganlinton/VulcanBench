//! Source contracts for checked C GPU-preprocess disk-cache encoding.

#[test]
fn disk_cache_encoders_use_exact_checked_reservation() {
    let classified_codec =
        include_str!("../src/parsing/c/preprocess/gpu_pipeline/cache/classified_codec.rs");
    assert!(
        classified_codec.contains(") -> Result<Vec<u8>, String>")
            && classified_codec.contains("fn classified_encoded_len(")
            && classified_codec.contains("reserve_encode_bytes(&mut out, encoded_len"),
        "classified cache encoding must calculate exact encoded size and reserve fallibly"
    );
    assert!(
        !classified_codec.contains("Vec::with_capacity(")
            && !classified_codec.contains("classified.tok_types.len() * 4")
            && !classified_codec.contains("path_bytes.len() as u64"),
        "classified cache encoding must not use infallible capacity or unchecked length casts"
    );

    let payload_codec =
        include_str!("../src/parsing/c/preprocess/gpu_pipeline/cache/payload_codec.rs");
    assert!(
        payload_codec.contains(") -> Result<Vec<u8>, String>")
            && payload_codec.contains("fn payloads_encoded_len(")
            && payload_codec.contains("fn payload_encoded_len(")
            && payload_codec.contains("out.try_reserve_exact(encoded_len)"),
        "payload cache encoding must calculate exact encoded size and reserve fallibly"
    );
    assert!(
        !payload_codec.contains("Vec::with_capacity(")
            && !payload_codec.contains(".unwrap_or(0)")
            && !payload_codec.contains("payloads.len() as u64"),
        "payload cache encoding must not use infallible capacity, overflow fallback, or unchecked length casts"
    );

    let classified_disk =
        include_str!("../src/parsing/c/preprocess/gpu_pipeline/cache/classified_disk.rs");
    let payload_disk =
        include_str!("../src/parsing/c/preprocess/gpu_pipeline/cache/payload_disk.rs");
    assert!(
        classified_disk.contains(") -> Result<(), String>")
            && classified_disk.contains("let encoded = encode_classified(key, classified)?;")
            && payload_disk.contains(") -> Result<(), String>")
            && payload_disk.contains("let encoded = encode_payloads(key, payloads)?;"),
        "disk-cache stores must propagate encode allocation failures"
    );
}

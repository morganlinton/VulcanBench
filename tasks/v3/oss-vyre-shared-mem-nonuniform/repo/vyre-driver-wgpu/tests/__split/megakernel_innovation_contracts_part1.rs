use super::*;

#[test]
fn dispatch_wait_uses_adaptive_parking_not_fixed_sleep() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/megakernel/dispatcher.rs"
    ))
    .expect("dispatcher source must be readable for wait-loop contract test");
    let prod = src.split("#[cfg(test)]").next().unwrap_or(&src);
    let wait_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/wait_backoff.rs"
    ))
    .expect("wait backoff source must be readable for wait-loop contract test");

    // Any form of fixed sleep is forbidden  -  it destroys tail-latency
    // under variable load.
    assert!(
        !prod.contains("std::thread::sleep") && !prod.contains("thread::sleep("),
        "dispatch wait must not use fixed thread::sleep"
    );
    assert!(
        !prod.contains("sleep_ms"),
        "dispatch wait must not use deprecated sleep_ms"
    );
    assert!(
        prod.contains("AdaptiveWaitBackoff"),
        "dispatch wait must use the shared adaptive backoff policy"
    );
    assert!(
        prod.contains("from_micros(64, 5, 50, 8)"),
        "dispatch wait must set explicit spin, min-park, max-park, and growth bounds"
    );
    assert!(
        wait_src.contains("spin_loop"),
        "dispatch wait must use short CPU pause spins before parking"
    );
    assert!(
        prod.contains("saturating_sub"),
        "dispatch wait must saturate remaining timeout to avoid over-wait"
    );
}

// ---------------------------------------------------------------------------
// 2. Control regions never alias for large tenant counts
// ---------------------------------------------------------------------------

#[test]
fn encode_control_with_u32_max_tenants_does_not_touch_metrics_or_observable() {
    let ctrl = Megakernel::encode_control(false, u32::MAX, /* observable_slots */ 0).unwrap();
    let words: Vec<u32> = ctrl
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();

    // Tenant mask table is capped at TENANT_QUOTA_BASE.
    for (i, w) in words
        .iter()
        .enumerate()
        .take(control::TENANT_QUOTA_BASE as usize)
        .skip(control::TENANT_BASE as usize + 1)
    {
        assert_eq!(
            *w, !0u32,
            "tenant mask word {i} must be set for u32::MAX tenants"
        );
    }

    // Quota table is capped at TENANT_FAIRNESS_BASE.
    for (i, w) in words
        .iter()
        .enumerate()
        .take(control::TENANT_FAIRNESS_BASE as usize)
        .skip(control::TENANT_QUOTA_BASE as usize)
    {
        assert_eq!(
            *w, 1_000_000,
            "quota word {i} must be set for u32::MAX tenants"
        );
    }

    // Fairness counters, metrics, epoch, priority offsets, and observable must remain untouched.
    for (i, w) in words
        .iter()
        .enumerate()
        .skip(control::TENANT_FAIRNESS_BASE as usize)
    {
        assert_eq!(
            *w, 0,
            "word {i} must not be touched by tenant encoding with u32::MAX tenants"
        );
    }
}

#[test]
fn try_encode_control_rejects_observable_overflow() {
    let err = Megakernel::try_encode_control(false, 1, u32::MAX)
        .expect_err("observable slot count overflow must be rejected");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "observable overflow must return QueueFull, got {err:?}"
    );
}

#[test]
fn control_region_capping_uses_min_in_source() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../vyre-runtime/src/megakernel/protocol/codec.rs"
    ))
    .expect("protocol source must be readable");
    let prod = src.split("#[cfg(test)]").next().unwrap_or(&src);

    assert!(
        prod.contains("core::cmp::min"),
        "encode_control must use core::cmp::min to cap tenant tables"
    );
    assert!(
        prod.contains("TENANT_QUOTA_BASE"),
        "encode_control must reference TENANT_QUOTA_BASE as a cap"
    );
    assert!(
        prod.contains("TENANT_FAIRNESS_BASE"),
        "encode_control must reference TENANT_FAIRNESS_BASE as a quota cap"
    );
}

// ---------------------------------------------------------------------------
// 3. Priority starvation accounting cannot overflow observable regions
// ---------------------------------------------------------------------------

#[test]
fn starvation_counter_never_reaches_observable_base() {
    // Even if PRIORITY_LEVELS doubles in the future, the counter must
    // still stay below the observable region.
    let hypothetical_levels = scheduler::PRIORITY_LEVELS * 2;
    let hypothetical_counter = scheduler::PRIORITY_OFFSETS_BASE + hypothetical_levels + 1;
    assert!(
        hypothetical_counter <= control::OBSERVABLE_BASE,
        "doubling priority levels would place counter at {hypothetical_counter}, \
         which must not exceed OBSERVABLE_BASE {}",
        control::OBSERVABLE_BASE
    );
}

#[test]
fn write_default_priority_offsets_does_not_clobber_starvation_counter() {
    let mut control = Megakernel::encode_control(false, 1, 0).unwrap();
    vyre_runtime::megakernel::scheduler::write_default_priority_offsets(&mut control, 256)
        .expect("write_default_priority_offsets must succeed for 256 slots");

    let off = PRIORITY_STARVATION_COUNTER as usize * 4;
    let word = u32::from_le_bytes(control[off..off + 4].try_into().unwrap());
    assert_eq!(
        word, 0,
        "write_default_priority_offsets must not touch PRIORITY_STARVATION_COUNTER"
    );
}

// ---------------------------------------------------------------------------
// 4. IO queue strict APIs reject malformed / misaligned buffers
// ---------------------------------------------------------------------------

#[test]
fn strict_poll_rejects_non_word_aligned_buffer() {
    let buf = vec![0xAA; 33]; // 33 bytes  -  not a multiple of 4
    let err = try_poll_io_requests(&buf).expect_err("non-word-aligned buffer must reject");
    assert!(
        matches!(err, PipelineError::Backend { .. }),
        "misaligned poll must return Backend error, got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("4-byte aligned"),
        "error must mention 4-byte alignment, got: {msg}"
    );
}

#[test]
fn strict_poll_rejects_partial_slot_aligned_buffer() {
    // 12 bytes = 3 u32 words  -  aligned to 4 but not a multiple of IO_SLOT_WORDS*4 = 32
    let buf = vec![0u8; 12];
    let err = try_poll_io_requests(&buf).expect_err("partial-slot aligned buffer must reject");
    assert!(
        matches!(err, PipelineError::Backend { .. }),
        "partial slot poll must return Backend error, got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("whole IO slots"),
        "error must mention whole IO slots, got: {msg}"
    );
}

#[test]
fn strict_completion_rejects_non_word_aligned_buffer() {
    let mut buf = vec![0xBB; 35];
    let err =
        try_complete_io_request(&mut buf, 0, true).expect_err("misaligned completion must reject");
    assert!(
        matches!(err, PipelineError::Backend { .. }),
        "misaligned completion must return Backend error, got {err:?}"
    );
}

#[test]
fn strict_completion_rejects_partial_slot_aligned_buffer() {
    let mut buf = vec![0u8; 36]; // aligned to 4, not a multiple of 32
    let err = try_complete_io_request(&mut buf, 0, true)
        .expect_err("partial-slot completion must reject");
    assert!(
        matches!(err, PipelineError::Backend { .. }),
        "partial slot completion must return Backend error, got {err:?}"
    );
}

#[test]
fn strict_completion_rejects_slot_beyond_buffer_end() {
    // 2 slots = 64 bytes. Slot index 2 is out of bounds.
    let mut buf = encode_empty_io_queue(2).expect("valid io_queue must encode");
    let err =
        try_complete_io_request(&mut buf, 2, true).expect_err("out-of-bounds slot must reject");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "OOB completion must return QueueFull, got {err:?}"
    );
}

#[test]
fn strict_poll_survives_maximal_misalignment_without_panic() {
    // A buffer of length 1 should produce a clean error, not panic.
    let buf = vec![0xCCu8; 1];
    let err = try_poll_io_requests(&buf).expect_err("1-byte buffer must reject cleanly");
    assert!(
        err.to_string().contains("Fix:"),
        "error must carry a Fix hint, got: {err}"
    );
}

#[test]
fn io_byte_view_rejects_slots_beyond_compiled_poll_window() {
    let oversize = vec![0u8; ((IO_SLOT_COUNT + 1) * IO_SLOT_WORDS * 4) as usize];
    let poll_err =
        try_poll_io_requests(&oversize).expect_err("poll must reject slots the GPU will not scan");
    assert!(
        matches!(poll_err, PipelineError::QueueFull { .. }),
        "oversized poll view must return QueueFull, got {poll_err:?}"
    );
    assert!(
        poll_err.to_string().contains("compiled IO poll window"),
        "error must explain the ABI poll window, got: {poll_err}"
    );

    let mut completion_view = oversize;
    let complete_err = try_complete_io_request(&mut completion_view, IO_SLOT_COUNT, true)
        .expect_err("completion must reject slots the GPU will not scan");
    assert!(
        matches!(complete_err, PipelineError::QueueFull { .. }),
        "oversized completion view must return QueueFull, got {complete_err:?}"
    );
}

#[test]
fn strict_io_queue_encoder_rejects_unpollable_queue_sizes() {
    let err = try_encode_empty_io_queue(IO_SLOT_COUNT + 1)
        .expect_err("strict encoder must reject queues beyond the GPU poll window");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "oversized queue encode must return QueueFull, got {err:?}"
    );
    assert!(
        err.to_string().contains("compiled IO poll window"),
        "error must explain the ABI poll window, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// 5. Batch catalog rejects duplicate / out-of-range rules without
//    corrupting accepted rules
// ---------------------------------------------------------------------------

#[test]
fn batch_rule_program_new_rejects_bad_transition_length() {
    let err = BatchRuleProgram::new(0, vec![0; 128], vec![0; 1], 1)
        .expect_err("transition table size mismatch must fail");
    assert!(
        err.to_string().contains("Fix:"),
        "error must be actionable, got: {err}"
    );
}

#[test]
fn batch_rule_program_new_rejects_bad_accept_length() {
    let err = BatchRuleProgram::new(0, vec![0; 256], vec![0; 1], 2)
        .expect_err("accept table size mismatch must fail");
    assert!(
        err.to_string().contains("Fix:"),
        "error must be actionable, got: {err}"
    );
}

#[test]
fn rule_catalog_source_contains_duplicate_and_range_rejection() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../vyre-runtime/src/megakernel/rule_catalog.rs"
    ))
    .expect("rule catalog source must be readable");
    let prod = src.split("#[cfg(test)]").next().unwrap_or(&src);

    assert!(
        prod.contains("duplicate rule_idx"),
        "catalog must contain explicit duplicate rejection prose"
    );
    assert!(
        prod.contains("outside 0.."),
        "catalog must contain explicit out-of-range rejection prose"
    );
    assert!(
        prod.contains("occupied") && prod.contains("addressed"),
        "catalog must track accepted vs addressed slots independently"
    );
}

#[test]
fn batch_dispatcher_source_wires_rejected_rules_into_report() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/megakernel/dispatcher.rs"
    ))
    .expect("dispatcher source must be readable");
    let prod = src.split("#[cfg(test)]").next().unwrap_or(&src);

    assert!(
        prod.contains("rejected_rules"),
        "BatchDispatchReport must carry rejected_rules field"
    );
    assert!(
        prod.contains("ensure_rule_buffers"),
        "dispatch path must call ensure_rule_buffers"
    );
    assert!(
        prod.contains("pack_rule_catalog_into"),
        "rule-catalog refresh must pack through caller-owned scratch storage"
    );
    assert!(
        prod.contains("accepted_rule_fingerprints_and_rejections_into"),
        "rule-catalog cache checks must reuse caller-owned fingerprint and rejection scratch"
    );
}

#[test]
fn file_batch_refresh_is_telemetry_capable_and_prefix_bounded() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/megakernel/batch.rs"
    ))
    .expect("batch source must be readable");
    let prod = src.split("#[cfg(test)]").next().unwrap_or(&src);

    assert!(
        prod.contains("pub struct FileBatchRefreshReport"),
        "FileBatch refresh must expose public resident reuse telemetry"
    );
    assert!(
        prod.contains("pub fn refresh_with_report"),
        "FileBatch must expose telemetry-capable in-place refresh"
    );
    assert!(
        prod.contains("resident_allocations")
            && prod.contains("reused_buffers")
            && prod.contains("bytes_uploaded"),
        "FileBatchRefreshReport must account allocation reuse and host/device bytes"
    );
    assert!(
        prod.contains("write_padded_prefix"),
        "FileBatch refresh must write logical prefixes into reused resident buffers"
    );
    let prefix_writer = prod
        .split("fn write_padded_prefix(")
        .nth(1)
        .and_then(|tail| tail.split("fn padded_write_len").next())
        .expect("write_padded_prefix body must be discoverable");
    assert!(
        prefix_writer.contains("&bytes[..aligned_len]"),
        "prefix writer must upload the logical aligned prefix"
    );
    assert!(
        !prefix_writer.contains("allocation_len"),
        "prefix writer must not zero-fill the full old allocation"
    );
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatcher_borrows_gpu_handles_for_persistent_launch() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/megakernel/dispatcher.rs"
    ))
    .expect("dispatcher source must be readable");
    let dispatch_body = src
        .split("pub fn dispatch_into(")
        .nth(1)
        .and_then(|tail| tail.split("fn ensure_rule_buffers").next())
        .expect("BatchDispatcher::dispatch_into body must be discoverable");

    assert!(
        dispatch_body.contains("dispatch_persistent_borrowed"),
        "batch megakernel dispatch must use the borrowed persistent path"
    );
    assert!(
        !dispatch_body.contains(".clone()"),
        "batch megakernel dispatch must not clone GpuBufferHandle values while assembling launch bindings"
    );
}

// End-to-end batch rejection coverage lives in the neighboring split file.

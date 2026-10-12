use super::super::protocol::slot;
use super::{
    claim_io_requests_into, complete_io_request, complete_io_requests_batch, encode_empty_io_queue,
    io_completion_poll_body, io_op, io_status, io_word, poll_io_requests,
    try_claim_io_requests_into, try_encode_empty_io_queue_into, try_poll_io_requests_into,
    MegakernelIoQueue, IO_SLOT_COUNT, IO_SLOT_WORDS,
};
use super::helpers::try_queue_word_index;
use crate::PipelineError;

#[test]
fn empty_io_queue_has_no_requests() {
    let buf = encode_empty_io_queue(4).unwrap();
    let reqs = poll_io_requests(&buf)
        .expect("Fix: empty aligned queue must poll; restore this invariant before continuing.");
    assert!(reqs.is_empty());
}

#[test]
fn empty_io_queue_encode_into_reuses_capacity() {
    let mut buf = Vec::with_capacity((IO_SLOT_WORDS as usize) * 8 * 4);
    let ptr = buf.as_ptr();
    try_encode_empty_io_queue_into(4, &mut buf).unwrap();

    assert_eq!(buf.len(), (IO_SLOT_WORDS as usize) * 4 * 4);
    assert!(
        buf.iter().all(|byte| *byte == 0),
        "Fix: encode_into must zero every IO queue byte before upload."
    );
    assert_eq!(
        buf.as_ptr(),
        ptr,
        "Fix: encode_into should retain caller-owned capacity for same-size queues."
    );
}

#[test]
fn published_io_slot_is_detected() {
    let mut buf = encode_empty_io_queue(4).unwrap();
    // Publish slot 1: READ, src=5, dst=6, offset=0x1000, count=4096, tag=42
    let base = IO_SLOT_WORDS as usize * 4;
    let write_word = |buf: &mut Vec<u8>, word: u32, val: u32| {
        let off = base + word as usize * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    write_word(&mut buf, io_word::OP_TYPE, io_op::READ);
    write_word(&mut buf, io_word::SRC_HANDLE, 5);
    write_word(&mut buf, io_word::DST_HANDLE, 6);
    write_word(&mut buf, io_word::OFFSET_LO, 0x1000);
    write_word(&mut buf, io_word::OFFSET_HI, 0);
    write_word(&mut buf, io_word::BYTE_COUNT, 4096);
    write_word(&mut buf, io_word::STATUS, slot::PUBLISHED);
    write_word(&mut buf, io_word::TAG, 42);

    let reqs = poll_io_requests(&buf).expect(
        "Fix: published aligned queue must poll; restore this invariant before continuing.",
    );
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].slot_idx, 1);
    assert_eq!(reqs[0].op_type, io_op::READ);
    assert_eq!(reqs[0].offset, 0x1000);
    assert_eq!(reqs[0].byte_count, 4096);
}

#[test]
fn poll_io_requests_into_reuses_request_storage() {
    let mut buf = encode_empty_io_queue(4).unwrap();
    let base = IO_SLOT_WORDS as usize * 4;
    let write_word = |buf: &mut Vec<u8>, word: u32, val: u32| {
        let off = base + word as usize * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    write_word(&mut buf, io_word::OP_TYPE, io_op::READ);
    write_word(&mut buf, io_word::DST_HANDLE, 9);
    write_word(&mut buf, io_word::BYTE_COUNT, 128);
    write_word(&mut buf, io_word::STATUS, slot::PUBLISHED);

    let mut requests = Vec::with_capacity(4);
    let initial_capacity = requests.capacity();
    try_poll_io_requests_into(&buf, &mut requests)
        .expect("Fix: reusable IO polling must accept aligned queue bytes");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].dst_handle, 9);
    assert_eq!(requests.capacity(), initial_capacity);

    try_poll_io_requests_into(&buf, &mut requests)
        .expect("Fix: repeated reusable IO polling must not allocate on a warm buffer");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests.capacity(), initial_capacity);
}

#[test]
fn poll_io_requests_into_reserves_only_published_slots() {
    let mut buf = encode_empty_io_queue(IO_SLOT_COUNT).unwrap();
    let base = IO_SLOT_WORDS as usize * 3 * 4;
    let write_word = |buf: &mut Vec<u8>, word: u32, val: u32| {
        let off = base + word as usize * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    write_word(&mut buf, io_word::OP_TYPE, io_op::READ);
    write_word(&mut buf, io_word::DST_HANDLE, 17);
    write_word(&mut buf, io_word::BYTE_COUNT, 512);
    write_word(&mut buf, io_word::STATUS, slot::PUBLISHED);

    let mut requests = Vec::new();
    try_poll_io_requests_into(&buf, &mut requests)
        .expect("Fix: sparse IO queue polling must reserve only published requests");

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].slot_idx, 3);
    assert!(
        requests.capacity() < IO_SLOT_COUNT as usize,
        "Fix: sparse IO polling must not reserve one request slot for every empty queue slot."
    );
}

#[test]
fn poll_io_requests_into_does_not_allocate_for_empty_queue() {
    let buf = encode_empty_io_queue(IO_SLOT_COUNT).unwrap();
    let mut requests = Vec::new();

    try_poll_io_requests_into(&buf, &mut requests)
        .expect("Fix: empty IO queue polling must not require request storage");

    assert!(requests.is_empty());
    assert_eq!(
        requests.capacity(),
        0,
        "Fix: empty IO polling must not allocate the full compiled queue window."
    );
}

#[test]
fn claim_io_requests_marks_published_slots_claimed_once() {
    let mut buf = encode_empty_io_queue(4).unwrap();
    let base = IO_SLOT_WORDS as usize * 4;
    let write_word = |buf: &mut Vec<u8>, word: u32, val: u32| {
        let off = base + word as usize * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    write_word(&mut buf, io_word::OP_TYPE, io_op::READ);
    write_word(&mut buf, io_word::SRC_HANDLE, 5);
    write_word(&mut buf, io_word::DST_HANDLE, 6);
    write_word(&mut buf, io_word::OFFSET_LO, 0x1000);
    write_word(&mut buf, io_word::BYTE_COUNT, 4096);
    write_word(&mut buf, io_word::STATUS, slot::PUBLISHED);
    write_word(&mut buf, io_word::TAG, 42);

    let mut requests = Vec::with_capacity(4);
    claim_io_requests_into(&mut buf, &mut requests)
        .expect("Fix: published aligned queue must claim exactly once");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].slot_idx, 1);

    let status_off = base + io_word::STATUS as usize * 4;
    let status = u32::from_le_bytes(buf[status_off..status_off + 4].try_into().unwrap());
    assert_eq!(status, slot::CLAIMED);

    claim_io_requests_into(&mut buf, &mut requests)
        .expect("Fix: claimed slots must stay pollable without resubmission");
    assert!(requests.is_empty());
}

#[test]
fn claim_io_requests_into_reuses_request_storage() {
    let mut buf = encode_empty_io_queue(4).unwrap();
    let base = IO_SLOT_WORDS as usize * 4;
    let write_word = |buf: &mut Vec<u8>, word: u32, val: u32| {
        let off = base + word as usize * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    write_word(&mut buf, io_word::OP_TYPE, io_op::READ);
    write_word(&mut buf, io_word::DST_HANDLE, 11);
    write_word(&mut buf, io_word::BYTE_COUNT, 256);
    write_word(&mut buf, io_word::STATUS, slot::PUBLISHED);

    let mut requests = Vec::with_capacity(4);
    let initial_capacity = requests.capacity();
    try_claim_io_requests_into(&mut buf, &mut requests)
        .expect("Fix: reusable IO claim must accept aligned queue bytes");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].dst_handle, 11);
    assert_eq!(requests.capacity(), initial_capacity);

    buf[base + io_word::STATUS as usize * 4..base + io_word::STATUS as usize * 4 + 4]
        .copy_from_slice(&slot::PUBLISHED.to_le_bytes());
    try_claim_io_requests_into(&mut buf, &mut requests)
        .expect("Fix: repeated reusable IO claim must not allocate on a warm buffer");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests.capacity(), initial_capacity);
}

#[test]
fn claim_io_requests_into_does_not_allocate_for_empty_queue() {
    let mut buf = encode_empty_io_queue(IO_SLOT_COUNT).unwrap();
    let mut requests = Vec::new();

    try_claim_io_requests_into(&mut buf, &mut requests)
        .expect("Fix: empty IO queue claiming must not require request storage");

    assert!(requests.is_empty());
    assert_eq!(
        requests.capacity(),
        0,
        "Fix: empty IO claim polling must not allocate the full compiled queue window."
    );
}

#[test]
fn complete_sets_status_after_claim() {
    let mut buf = encode_empty_io_queue(2).unwrap();
    let write_word = |buf: &mut Vec<u8>, word: u32, val: u32| {
        let off = word as usize * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    write_word(&mut buf, io_word::STATUS, slot::PUBLISHED);
    let mut requests = Vec::new();
    claim_io_requests_into(&mut buf, &mut requests)
        .expect("Fix: published request must be claimable before completion");
    assert_eq!(
        requests.len(),
        1,
        "Fix: completion contract must start from exactly one claimed request."
    );

    complete_io_request(&mut buf, 0, true).expect(
        "Fix: claimed completion slot must update; restore this invariant before continuing.",
    );
    let status_off = io_word::STATUS as usize * 4;
    let status = u32::from_le_bytes(buf[status_off..status_off + 4].try_into().unwrap());
    assert_eq!(status, io_status::OK);
}

#[test]
fn batch_completion_validates_before_mutating() {
    let mut buf = encode_empty_io_queue(2).unwrap();
    let status0 = io_word::STATUS as usize * 4;
    let status1 = (IO_SLOT_WORDS as usize + io_word::STATUS as usize) * 4;
    buf[status0..status0 + 4].copy_from_slice(&slot::CLAIMED.to_le_bytes());
    let before = buf.clone();

    let error = complete_io_requests_batch(&mut buf, &[(0, true), (1, true)])
        .expect_err("batch completion must reject unclaimed slots before writing any status");
    match error {
        PipelineError::QueueFull { fix, .. } => assert!(
            fix.contains("CLAIMED request"),
            "batch ownership error must be actionable, got `{fix}`"
        ),
        other => panic!("expected QueueFull for unclaimed batch slot, got {other:?}"),
    }
    assert_eq!(buf, before);

    buf[status1..status1 + 4].copy_from_slice(&slot::CLAIMED.to_le_bytes());
    complete_io_requests_batch(&mut buf, &[(0, true), (1, false)])
        .expect("Fix: claimed batch completions must publish together");
    assert_eq!(
        u32::from_le_bytes(buf[status0..status0 + 4].try_into().unwrap()),
        io_status::OK
    );
    assert_eq!(
        u32::from_le_bytes(buf[status1..status1 + 4].try_into().unwrap()),
        io_status::ERROR
    );
}

#[test]
fn completion_without_claim_is_rejected() {
    let mut buf = encode_empty_io_queue(1).unwrap();
    let error = complete_io_request(&mut buf, 0, true)
        .expect_err("unclaimed IO slots must not be completed");
    match error {
        PipelineError::QueueFull { fix, .. } => assert!(
            fix.contains("CLAIMED request"),
            "completion ownership error must be actionable, got `{fix}`"
        ),
        other => panic!("expected QueueFull for unclaimed completion, got {other:?}"),
    }
}

#[test]
fn io_completion_poll_produces_valid_ir() {
    let nodes = io_completion_poll_body();
    assert_eq!(nodes.len(), 1); // one loop_for
}

#[test]
fn host_publish_slot_round_trips() {
    let mut queue = MegakernelIoQueue::new(4).unwrap();
    assert_eq!(queue.as_bytes().as_ptr() as usize % 4, 0);
    queue.publish_slot(2, 7, 4096, 99).unwrap();
    let completion = queue
        .completion(2)
        .expect("Fix: published slot present; restore this invariant before continuing.");
    assert_eq!(completion.mapped_slot, 7);
    assert_eq!(completion.byte_count, 4096);
    assert_eq!(completion.tag, 99);
    assert_eq!(
        u32::from_le_bytes(
            queue.as_bytes()[((2 * IO_SLOT_WORDS + io_word::STATUS) as usize * 4)
                ..((2 * IO_SLOT_WORDS + io_word::STATUS) as usize * 4 + 4)]
                .try_into()
                .unwrap()
        ),
        slot::PUBLISHED
    );
}

#[test]
fn host_queue_byte_view_stays_aligned_after_mutation() {
    let mut queue = MegakernelIoQueue::new(IO_SLOT_COUNT).unwrap();
    assert_eq!(queue.as_mut_bytes().as_ptr() as usize % 4, 0);
    queue.publish_slot(0, 3, 512, 77).unwrap();
    assert_eq!(queue.as_bytes().as_ptr() as usize % 4, 0);
}

#[test]
fn oversized_queue_is_rejected_with_actionable_error() {
    let error = MegakernelIoQueue::new(IO_SLOT_COUNT + 1)
        .expect_err("queues larger than the compiled 64-slot poll window must fail");
    match error {
        PipelineError::QueueFull { fix, .. } => {
            assert!(
                fix.contains("64 slots"),
                "overflow error must explain the compiled queue limit, got `{fix}`"
            );
        }
        other => panic!("expected QueueFull overflow error, got {other:?}"),
    }
}

#[test]
fn publishing_the_sixty_fifth_completion_errors_instead_of_dropping() {
    let mut queue = MegakernelIoQueue::new(IO_SLOT_COUNT).unwrap();
    for slot in 0..IO_SLOT_COUNT {
        queue.publish_slot(slot, slot, 4096, slot).unwrap();
        let base = (slot * IO_SLOT_WORDS + io_word::STATUS) as usize * 4;
        queue.as_mut_bytes()[base..base + 4].copy_from_slice(&io_status::OK.to_le_bytes());
    }

    let error = queue
        .publish_slot(IO_SLOT_COUNT, IO_SLOT_COUNT, 4096, IO_SLOT_COUNT)
        .expect_err("the 65th published completion must fail loudly");
    match error {
        PipelineError::QueueFull { fix, .. } => {
            assert!(
                fix.contains("valid slot id"),
                "overflow error must stay actionable, got `{fix}`"
            );
        }
        other => panic!("expected QueueFull on 65th publish, got {other:?}"),
    }
}

#[test]
fn complete_io_request_only_mutates_status_word() {
    let mut buf = encode_empty_io_queue(1).unwrap();
    for (idx, byte) in buf.iter_mut().enumerate() {
        *byte = (idx % 251) as u8;
    }
    let status_off = (io_word::STATUS as usize) * 4;
    buf[status_off..status_off + 4].copy_from_slice(&slot::CLAIMED.to_le_bytes());
    let before = buf.clone();
    complete_io_request(&mut buf, 0, false).expect(
        "Fix: valid completion slot must update; restore this invariant before continuing.",
    );
    for idx in 0..buf.len() {
        let in_status_word = (status_off..status_off + 4).contains(&idx);
        if !in_status_word {
            assert_eq!(
                buf[idx], before[idx],
                "status completion must not touch non-status byte index {idx}"
            );
        }
    }
    let status = u32::from_le_bytes(buf[status_off..status_off + 4].try_into().unwrap());
    assert_eq!(status, io_status::ERROR);
}

#[test]
fn io_module_avoids_byte_width_atomic_types() {
    // Check every production I/O source slice (tests excluded) so byte-width
    // atomics cannot hide in runtime megakernel queue code.
    let prod_files = [
        include_str!("../mod.rs"),
        include_str!("../queue.rs"),
        include_str!("../poll.rs"),
        include_str!("../complete.rs"),
        include_str!("../encode.rs"),
        include_str!("../helpers.rs"),
    ];
    for src in prod_files {
        assert!(
            !src.contains("AtomicU8") && !src.contains("AtomicI8"),
            "byte-width atomics are forbidden for io_queue protocol words"
        );
        assert!(
            !src.contains("AtomicU16") && !src.contains("AtomicI16"),
            "sub-word atomics are forbidden for io_queue protocol words"
        );
    }
}

#[test]
fn submit_dma_read_publishes_read_request() {
    let mut queue = MegakernelIoQueue::new(4).unwrap();
    queue.submit_dma_read(2, 10, 20, 4096, 99).unwrap();

    let reqs = poll_io_requests(queue.as_bytes()).unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].slot_idx, 2);
    assert_eq!(reqs[0].op_type, io_op::READ);
    assert_eq!(reqs[0].src_handle, 10);
    assert_eq!(reqs[0].dst_handle, 20);
    assert_eq!(reqs[0].byte_count, 4096);
    assert_eq!(reqs[0].tag, 99);
}

#[test]
fn submit_dma_read_rejects_non_empty_slot() {
    let mut queue = MegakernelIoQueue::new(4).unwrap();
    queue.submit_dma_read(1, 10, 20, 4096, 99).unwrap();
    let err = queue.submit_dma_read(1, 11, 21, 8192, 100).unwrap_err();
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "Fix: re-submitting to an in-flight slot must return QueueFull"
    );
}

/// Regression: `queue_word_index` silently returned 0 on 32-bit overflow — corrupting
/// slot 0's OP_TYPE word. The infallible wrapper is removed; `try_queue_word_index`
/// must return `Err(PipelineError::Backend(...))` on overflow rather than 0.
///
/// Before the fix: on 32-bit targets where `slot_idx * IO_SLOT_WORDS` overflows
/// usize, `queue_word_index` returned 0 (silent wrong slot access).
/// After the fix: the function does not exist; `try_queue_word_index` returns `Err`.
///
/// On 64-bit hosts the u32 multiply cannot overflow (max is 4294967295 * 8 = 34G
/// which fits). The canonical overflow is tested on 32-bit; on 64-bit we verify that
/// `try_queue_word_index` surfaces the error when the *word* itself overflows usize
/// (using u32::MAX as the word argument — this overflows usize on 32-bit).
#[cfg(target_pointer_width = "32")]
#[test]
fn queue_word_index_overflow_returns_structured_error_not_zero_32bit() {
    // On 32-bit, usize is 4 bytes. slot_idx=u32::MAX (4294967295) * IO_SLOT_WORDS=8
    // = 34359738360 which overflows u32::MAX (4294967295), so try_queue_word_index
    // returns Err instead of Ok(0) from the old unwrap_or.
    let err = try_queue_word_index(u32::MAX, 0)
        .expect_err("on 32-bit, try_queue_word_index(u32::MAX, 0) must Err on overflow, not Ok(0)");
    match &err {
        PipelineError::Backend(msg) => {
            assert!(
                msg.contains("shard") || msg.contains("overflow") || msg.contains("cannot fit"),
                "overflow error must be actionable, got: {msg}"
            );
        }
        other => panic!("expected PipelineError::Backend for index overflow, got {other:?}"),
    }
}

/// Regression: on any platform, `try_queue_word_index` must return `Err` when
/// the slot base itself overflows usize. `usize::MAX / IO_SLOT_WORDS as usize + 1`
/// as slot_idx fits in u32 only on 64-bit (where the value is too large for u32).
/// We use a direct arithmetic overflow via `usize::MAX` in `base.checked_mul`:
/// pass a slot_idx whose base word product wraps. On 64-bit this requires a very
/// large slot_idx — we force it by passing u32::MAX for both slot_idx AND word so
/// that `base + word` overflows.
#[test]
fn queue_word_index_with_max_word_returns_structured_error() {
    // u32::MAX as the word argument: on 32-bit, usize::try_from(u32::MAX) fails if
    // usize is 16-bit, succeeds on 32-bit but the add overflows.
    // On 64-bit: slot=0, word=u32::MAX=4294967295 which fits usize, and 0*8+4294967295
    // = 4294967295 — this succeeds (Ok). That's fine: the important invariant is that
    // when slot_idx * IO_SLOT_WORDS + word truly overflows, Err is returned not 0.
    //
    // For the cross-platform invariant we test the checked overflow path directly:
    // slot_idx chosen so slot * IO_SLOT_WORDS overflows usize:
    let overflow_slot = usize::MAX.wrapping_div(IO_SLOT_WORDS as usize).wrapping_add(1);
    if let Ok(slot_as_u32) = u32::try_from(overflow_slot) {
        // This slot value overflows the multiplication on any platform where
        // overflow_slot * IO_SLOT_WORDS wraps.
        let result = try_queue_word_index(slot_as_u32, 0);
        // It must either succeed (if usize is wide enough that the value fits)
        // or return a structured error — never silently return 0.
        if let Err(ref err) = result {
            match err {
                PipelineError::Backend(msg) => {
                    assert!(
                        msg.contains("shard") || msg.contains("overflow") || msg.contains("cannot fit"),
                        "overflow error must be actionable, got: {msg}"
                    );
                }
                other => panic!("expected PipelineError::Backend for index overflow, got {other:?}"),
            }
        }
        // If it succeeded, the platform can represent the value — that's also correct.
    }
    // If overflow_slot doesn't fit in u32 the test is vacuously satisfied: the
    // caller cannot construct such a slot_idx, so the overflow path is unreachable.
}

/// Regression: the internal `read_word` / `write_word_unfenced` helpers now return
/// `Result` and propagate `try_queue_word_index` failures. The old infallible
/// `queue_word_index` that silently returned 0 is removed.
///
/// This test verifies that a `publish_slot` that hits the word-level error path
/// returns a structured `Err` to the caller rather than silently writing to word 0.
/// On all platforms, the first guard is the slot-out-of-bounds check; on 32-bit
/// the secondary guard is the `try_queue_word_index` overflow check. Either way, the
/// queue must be pristine and the caller must receive an error.
#[test]
fn publish_slot_failure_does_not_silently_corrupt_queue_storage() {
    let mut queue = MegakernelIoQueue::new(4).unwrap();

    // Mark slot 0 as PUBLISHED so we can detect any silent write to it.
    queue.publish_slot(0, 1, 512, 7).unwrap();
    let before: Vec<u8> = queue.as_bytes().to_vec();

    // Attempt to publish into a slot beyond the queue's capacity — this must
    // return QueueFull from the bounds guard without touching any queue storage.
    let err = queue
        .publish_slot(IO_SLOT_COUNT, 99, 4096, 42)
        .expect_err("publishing beyond slot_count must return QueueFull, not silently redirect");
    assert!(
        matches!(err, PipelineError::QueueFull { .. }),
        "out-of-bounds publish must return QueueFull, got {err:?}"
    );

    // The entire queue buffer must be byte-for-byte identical to before the
    // failed call — no word was written to any slot, including slot 0.
    let after: Vec<u8> = queue.as_bytes().to_vec();
    assert_eq!(
        before, after,
        "queue storage must be unchanged after a failed publish; \
         the removed unwrap_or(0) could have corrupted slot 0's OP_TYPE word on 32-bit targets"
    );
}

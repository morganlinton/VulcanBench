//! Public megakernel IO APIs must surface malformed queue errors.

use vyre_runtime::megakernel::io::{complete_io_request, poll_io_requests};
#[test]
fn public_poll_rejects_misaligned_queue_view() {
    let err = poll_io_requests(&[0u8; 3]).expect_err("public poll must not hide bad queue views");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:") && msg.contains("4-byte aligned"),
        "misaligned queue must return actionable PipelineError, got {err:?}"
    );
}

#[test]
fn public_complete_rejects_out_of_bounds_slot() {
    let mut bytes = vec![0u8; 8 * 4];
    let err = complete_io_request(&mut bytes, 1, true)
        .expect_err("public completion must not hide bad slot ids");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:") && msg.contains("slot"),
        "bad completion slot must return actionable PipelineError, got {err:?}"
    );
}

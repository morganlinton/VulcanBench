//! Freeze tests for `ErrorCode` stable integer identifiers.
//!
//! Downstream systems (telemetry, alerting, retry policies) persist these
//! IDs. Renumbering or reordering them is a breaking change for operational
//! consumers. These tests assert that every variant maps to its expected
//! stable id and that new variants are append-only.

use vyre::backend::{BackendError, ErrorCode};

#[test]
fn device_out_of_memory_is_1001() {
    assert_eq!(ErrorCode::DeviceOutOfMemory.stable_id(), 1001);
}

#[test]
fn unsupported_feature_is_1002() {
    assert_eq!(ErrorCode::UnsupportedFeature.stable_id(), 1002);
}

#[test]
fn poisoned_lock_is_1003() {
    assert_eq!(ErrorCode::PoisonedLock.stable_id(), 1003);
}

#[test]
fn kernel_compile_failed_is_1004() {
    assert_eq!(ErrorCode::KernelCompileFailed.stable_id(), 1004);
}

#[test]
fn dispatch_failed_is_1005() {
    assert_eq!(ErrorCode::DispatchFailed.stable_id(), 1005);
}

#[test]
fn invalid_program_is_1006() {
    assert_eq!(ErrorCode::InvalidProgram.stable_id(), 1006);
}

#[test]
fn unknown_is_1999() {
    assert_eq!(ErrorCode::Unknown.stable_id(), 1999);
}

#[test]
fn all_codes_are_unique() {
    let codes = vec![
        ErrorCode::DeviceOutOfMemory,
        ErrorCode::UnsupportedFeature,
        ErrorCode::PoisonedLock,
        ErrorCode::KernelCompileFailed,
        ErrorCode::DispatchFailed,
        ErrorCode::InvalidProgram,
        ErrorCode::Unknown,
    ];
    let mut ids: Vec<u32> = codes.iter().map(|c| c.stable_id()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 7, "every ErrorCode must have a unique stable_id");
}

#[test]
fn backend_error_message_contains_fix_hint() {
    let err = BackendError::new("something broke. Fix: try again.");
    assert!(
        err.message().contains("Fix:"),
        "every BackendError must contain Fix: hint"
    );
}

#[test]
fn backend_error_code_roundtrips_through_message() {
    let err = BackendError::unsupported_extension("test_backend", "test_ext", "test_identity");
    assert_eq!(err.code(), ErrorCode::UnsupportedFeature);
}

#[test]
fn poisoned_lock_produces_correct_code() {
    let binding = std::sync::Mutex::new(0);
    let poison: std::sync::PoisonError<std::sync::MutexGuard<'_, i32>> =
        std::sync::PoisonError::new(binding.lock().unwrap());
    let err = BackendError::poisoned_lock(poison);
    assert_eq!(err.code(), ErrorCode::PoisonedLock);
}

#[test]
fn error_codes_are_monotonically_increasing() {
    // This is a soft contract: new codes should get higher numbers.
    // If this fails, someone may have reordered variants.
    let ids = [
        ErrorCode::DeviceOutOfMemory.stable_id(),
        ErrorCode::UnsupportedFeature.stable_id(),
        ErrorCode::PoisonedLock.stable_id(),
        ErrorCode::KernelCompileFailed.stable_id(),
        ErrorCode::DispatchFailed.stable_id(),
        ErrorCode::InvalidProgram.stable_id(),
    ];
    for window in ids.windows(2) {
        assert!(
            window[0] < window[1],
            "stable ids must be monotonically increasing to avoid collision risk, found {} before {}",
            window[0],
            window[1]
        );
    }
}

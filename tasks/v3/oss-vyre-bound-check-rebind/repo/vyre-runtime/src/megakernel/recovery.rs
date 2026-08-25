//! Device-loss classification and persistent-pipeline rebuild policy.

use std::sync::Arc;

use vyre_driver::backend::{CompiledPipeline, DispatchConfig, VyreBackend};
use vyre_driver::BackendError;
use vyre_foundation::ir::Program;

/// Recovery action taken after a backend device-loss symptom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MegakernelRecoveryDecision {
    /// The runtime rebuilt the compiled pipeline on the same backend.
    RecompiledPipeline,
}

/// Coarse failure class used by persistent megakernel recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MegakernelRecoveryClass {
    /// Backend context, adapter, or compiled-pipeline state was lost or stale.
    DeviceLoss,
    /// Queue/resource pressure that can be retried without recompilation.
    TransientQueue,
    /// Program/lowering/kernel-source failure that should not be retried as-is.
    ProgramBug,
    /// No safe automated recovery class could be inferred.
    Unclassified,
}

/// Runtime recovery policy for persistent megakernel dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelRecoveryPolicy {
    /// Retry a dispatch once after a device-loss-like backend error.
    pub retry_device_loss_once: bool,
}

impl Default for MegakernelRecoveryPolicy {
    fn default() -> Self {
        Self {
            retry_device_loss_once: true,
        }
    }
}

/// Return true when a backend error is consistent with device loss or a stale
/// compiled pipeline.
#[must_use]
pub fn backend_error_indicates_device_loss(error: &BackendError) -> bool {
    classify_backend_recovery_error(error) == MegakernelRecoveryClass::DeviceLoss
}

/// Classify a backend failure for persistent megakernel recovery.
#[must_use]
pub fn classify_backend_recovery_error(error: &BackendError) -> MegakernelRecoveryClass {
    match error {
        BackendError::DeviceOutOfMemory { .. } | BackendError::PoisonedLock { .. } => {
            MegakernelRecoveryClass::TransientQueue
        }
        BackendError::KernelCompileFailed { .. }
        | BackendError::InvalidProgram { .. }
        | BackendError::UnsupportedFeature { .. } => MegakernelRecoveryClass::ProgramBug,
        BackendError::DispatchFailed { message, .. } => classify_recovery_message(message),
        BackendError::Raw(message) => classify_recovery_message(message),
        _ => classify_recovery_message(&error.to_string()),
    }
}

fn classify_recovery_message(message: &str) -> MegakernelRecoveryClass {
    if text_contains_any_marker(message, DEVICE_LOSS_MARKERS) {
        return MegakernelRecoveryClass::DeviceLoss;
    }
    if text_contains_any_marker(message, TRANSIENT_QUEUE_MARKERS) {
        return MegakernelRecoveryClass::TransientQueue;
    }
    if text_contains_any_marker(message, PROGRAM_BUG_MARKERS) {
        return MegakernelRecoveryClass::ProgramBug;
    }
    MegakernelRecoveryClass::Unclassified
}

/// Return true when a backend error is consistent with device loss or a stale
/// compiled pipeline.
#[must_use]
pub fn backend_error_message_indicates_device_loss(error: &BackendError) -> bool {
    let text = error.to_string();
    text_contains_any_marker(&text, DEVICE_LOSS_MARKERS)
}

const DEVICE_LOSS_MARKERS: &[&str] = &[
    "device lost",
    "devicelost",
    "context lost",
    "lost device",
    "adapter lost",
    "gpu reset",
    "device_error_context_is_destroyed",
    "device_error_context_is_current",
    "device_error_deinitialized",
    "stale pipeline",
];

const TRANSIENT_QUEUE_MARKERS: &[&str] = &[
    "queue full",
    "backpressure",
    "temporarily unavailable",
    "try again",
    "would block",
    "timeout",
    "timed out",
    "out of memory",
    "device out of memory",
];

const PROGRAM_BUG_MARKERS: &[&str] = &[
    "invalid program",
    "kernel-source compile failed",
    "compile failed",
    "unsupported feature",
    "validation failed",
    "lowering failed",
    "type error",
];

fn text_contains_any_marker(text: &str, markers: &[&str]) -> bool {
    markers
        .iter()
        .any(|marker| contains_ascii_case_insensitive(text, marker))
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return true;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

/// Recompile a persistent megakernel pipeline after a recoverable device
/// failure.
///
/// # Errors
///
/// Returns the backend compile error if the backend cannot rebuild the program.
pub fn recover_compiled_pipeline(
    backend: &Arc<dyn VyreBackend>,
    program: Arc<Program>,
    config: &DispatchConfig,
) -> Result<Arc<dyn CompiledPipeline>, BackendError> {
    vyre_driver::pipeline::compile_shared(Arc::clone(backend), program, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_classifier_separates_device_loss_transient_queue_and_program_bug() {
        let device_loss = BackendError::DispatchFailed {
            code: None,
            message: "DeviceLost after queue submit".to_string(),
        };
        assert_eq!(
            classify_backend_recovery_error(&device_loss),
            MegakernelRecoveryClass::DeviceLoss
        );
        assert!(backend_error_indicates_device_loss(&device_loss));
        assert!(backend_error_message_indicates_device_loss(&device_loss));

        let transient = BackendError::new("queue full during publish. Fix: retry after drain.");
        assert_eq!(
            classify_backend_recovery_error(&transient),
            MegakernelRecoveryClass::TransientQueue
        );
        assert!(!backend_error_indicates_device_loss(&transient));

        let program_bug = BackendError::InvalidProgram {
            fix: "Fix: validate descriptor before backend lowering.".to_string(),
        };
        assert_eq!(
            classify_backend_recovery_error(&program_bug),
            MegakernelRecoveryClass::ProgramBug
        );
    }

    #[test]
    fn recovery_classifier_prefers_device_loss_over_transient_markers() {
        let error = BackendError::new(
            "queue full because stale pipeline hit adapter lost. Fix: rebuild the pipeline.",
        );

        assert_eq!(
            classify_backend_recovery_error(&error),
            MegakernelRecoveryClass::DeviceLoss
        );
    }

    #[test]
    fn recovery_classifier_leaves_unknown_errors_unclassified() {
        let error =
            BackendError::new("backend returned vendor code 17. Fix: inspect backend logs.");

        assert_eq!(
            classify_backend_recovery_error(&error),
            MegakernelRecoveryClass::Unclassified
        );
    }
}

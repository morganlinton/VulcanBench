//! CUDA regex hardware-comparison evidence.
//!
//! CUDA scan evidence must be able to compare against hardware regex engines
//! without implying that CUDA itself provides one. This module records the
//! reusable compiled scan artifact, software path, and shared driver
//! accelerator capability record for each comparison.

use vyre_driver::backend::{
    RegexAcceleratorCapability, RegexAcceleratorClass, RegexAcceleratorEvidence,
};
use vyre_driver::BackendError;

use crate::CUDA_BACKEND_ID;

/// Schema version for CUDA regex hardware-comparison evidence.
pub const CUDA_REGEX_HARDWARE_COMPARISON_SCHEMA_VERSION: u32 = 1;

/// CUDA scan evidence comparing software/CUDA paths with a regex accelerator capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CudaRegexHardwareComparisonEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Baseline id, for example `hyperscan-compatible` or `rxp-like`.
    pub baseline_id: &'static str,
    /// CUDA software path used when no hardware regex accelerator is present.
    pub software_path: &'static str,
    /// Reusable compiled scan artifact id or module-cache identity.
    pub compiled_scan_artifact_id: &'static str,
    /// Shared driver accelerator evidence.
    pub accelerator: RegexAcceleratorEvidence,
    /// True only when the shared accelerator capability is supported.
    pub hardware_available: bool,
}

impl CudaRegexHardwareComparisonEvidence {
    /// Return true when the evidence cannot overclaim CUDA hardware regex support.
    #[must_use]
    pub fn is_complete(self) -> bool {
        self.schema_version == CUDA_REGEX_HARDWARE_COMPARISON_SCHEMA_VERSION
            && !self.baseline_id.is_empty()
            && !self.software_path.is_empty()
            && !self.compiled_scan_artifact_id.is_empty()
            && self.hardware_available == self.accelerator.supported
            && self.accelerator.is_complete()
    }
}

/// Build CUDA regex hardware-comparison evidence from an explicit capability.
///
/// # Errors
///
/// Returns [`BackendError::InvalidProgram`] when required comparison identity
/// fields are empty.
pub fn cuda_regex_hardware_comparison_evidence(
    baseline_id: &'static str,
    software_path: &'static str,
    compiled_scan_artifact_id: &'static str,
    accelerator: RegexAcceleratorCapability,
    transfer_bytes: u64,
) -> Result<CudaRegexHardwareComparisonEvidence, BackendError> {
    if baseline_id.is_empty() {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: CUDA regex hardware-comparison baseline_id must be non-empty."
                .to_string(),
        });
    }
    if software_path.is_empty() {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: CUDA regex hardware-comparison software_path must be non-empty."
                .to_string(),
        });
    }
    if compiled_scan_artifact_id.is_empty() {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: CUDA regex hardware-comparison compiled_scan_artifact_id must be non-empty."
                .to_string(),
        });
    }
    let accelerator = accelerator.evidence(transfer_bytes);
    Ok(CudaRegexHardwareComparisonEvidence {
        schema_version: CUDA_REGEX_HARDWARE_COMPARISON_SCHEMA_VERSION,
        baseline_id,
        software_path,
        compiled_scan_artifact_id,
        hardware_available: accelerator.supported,
        accelerator,
    })
}

/// Build CUDA evidence for the ordinary software path with no hardware regex accelerator.
///
/// # Errors
///
/// Returns [`BackendError::InvalidProgram`] when required comparison identity
/// fields are empty.
pub fn cuda_regex_software_fallback_comparison_evidence(
    baseline_id: &'static str,
    software_path: &'static str,
    compiled_scan_artifact_id: &'static str,
) -> Result<CudaRegexHardwareComparisonEvidence, BackendError> {
    cuda_regex_hardware_comparison_evidence(
        baseline_id,
        software_path,
        compiled_scan_artifact_id,
        RegexAcceleratorCapability::unsupported(
            CUDA_BACKEND_ID,
            RegexAcceleratorClass::RxpLike,
            "CUDA backend has no RXP-like hardware regex accelerator capability record",
        ),
        0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_driver::backend::{
        RegexAcceleratorMatchSchema, RegexAcceleratorStreamMode,
    };

    #[test]
    fn cuda_software_fallback_records_unsupported_hardware_fields() {
        let evidence = cuda_regex_software_fallback_comparison_evidence(
            "rxp-like-baseline",
            "cuda-software-regex",
            "cuda-module-cache:scan:v1",
        )
        .expect("Fix: CUDA regex software fallback evidence should build");

        assert_eq!(evidence.schema_version, CUDA_REGEX_HARDWARE_COMPARISON_SCHEMA_VERSION);
        assert_eq!(evidence.baseline_id, "rxp-like-baseline");
        assert_eq!(evidence.software_path, "cuda-software-regex");
        assert_eq!(evidence.compiled_scan_artifact_id, "cuda-module-cache:scan:v1");
        assert!(!evidence.hardware_available);
        assert!(!evidence.accelerator.supported);
        assert_eq!(evidence.accelerator.backend, CUDA_BACKEND_ID);
        assert_eq!(
            evidence.accelerator.unsupported_reason,
            "CUDA backend has no RXP-like hardware regex accelerator capability record"
        );
        assert!(evidence.accelerator.match_parity_required);
        assert!(evidence.is_complete());
    }

    #[test]
    fn cuda_supported_hardware_comparison_requires_capability_record() {
        let capability = RegexAcceleratorCapability::supported(
            CUDA_BACKEND_ID,
            RegexAcceleratorClass::RxpLike,
            "rxp-sidecar:v1",
            8192,
            RegexAcceleratorStreamMode::Streaming,
            RegexAcceleratorMatchSchema::PatternIdOffsets,
        );

        let evidence = cuda_regex_hardware_comparison_evidence(
            "rxp-like-baseline",
            "cuda-software-regex",
            "cuda-module-cache:scan:v1",
            capability,
            4096,
        )
        .expect("Fix: CUDA regex hardware evidence should build for explicit capability");

        assert!(evidence.hardware_available);
        assert!(evidence.accelerator.supported);
        assert_eq!(evidence.accelerator.device_signature, "rxp-sidecar:v1");
        assert_eq!(evidence.accelerator.rule_capacity, 8192);
        assert_eq!(evidence.accelerator.transfer_bytes, 4096);
        assert!(evidence.is_complete());
    }
}

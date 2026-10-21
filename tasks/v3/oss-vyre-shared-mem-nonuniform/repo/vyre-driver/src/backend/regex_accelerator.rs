//! Regex accelerator backend capability contract.
//!
//! Hardware regex engines are useful only when the backend exposes an explicit
//! capability record. This module keeps unsupported devices fail-closed and
//! makes software fallback, stream mode, match schema, parity, and transfer
//! accounting visible to benchmark evidence.

use super::BackendError;

/// Schema version for regex accelerator benchmark evidence.
pub const REGEX_ACCELERATOR_EVIDENCE_SCHEMA_VERSION: u32 = 1;

/// Regex accelerator class advertised by a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegexAcceleratorClass {
    /// RXP-like hardware regex acceleration.
    RxpLike,
    /// DPU-attached regex acceleration.
    Dpu,
    /// FPGA-style regex offload.
    Fpga,
    /// Software reference or fallback path.
    Software,
}

impl RegexAcceleratorClass {
    /// Stable label for evidence artifacts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RxpLike => "rxp_like",
            Self::Dpu => "dpu",
            Self::Fpga => "fpga",
            Self::Software => "software",
        }
    }
}

/// Regex stream mode supported by an accelerator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegexAcceleratorStreamMode {
    /// No accelerator stream mode is available.
    Unavailable,
    /// Independent block scans.
    Block,
    /// Stateless streaming over segmented input.
    Streaming,
    /// Stateful streaming that preserves cross-chunk automata state.
    StatefulStreaming,
}

impl RegexAcceleratorStreamMode {
    /// Stable label for evidence artifacts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Block => "block",
            Self::Streaming => "streaming",
            Self::StatefulStreaming => "stateful_streaming",
        }
    }
}

/// Match schema emitted by an accelerator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegexAcceleratorMatchSchema {
    /// No accelerator match schema is available.
    Unavailable,
    /// Start/end offsets only.
    Offsets,
    /// Pattern id plus start/end offsets.
    PatternIdOffsets,
    /// Pattern id, stream id, and start/end offsets.
    StreamPatternIdOffsets,
}

impl RegexAcceleratorMatchSchema {
    /// Stable label for evidence artifacts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Offsets => "offsets",
            Self::PatternIdOffsets => "pattern_id_offsets",
            Self::StreamPatternIdOffsets => "stream_pattern_id_offsets",
        }
    }
}

/// Backend capability for hardware or software regex acceleration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegexAcceleratorCapability {
    /// Backend id that owns this capability.
    pub backend: &'static str,
    /// Accelerator class.
    pub accelerator_class: RegexAcceleratorClass,
    /// True only when the backend can execute this accelerator class.
    pub supported: bool,
    /// Device signature for supported accelerators.
    pub device_signature: &'static str,
    /// Maximum rule count accepted by the accelerator.
    pub rule_capacity: u32,
    /// Stream mode accepted by the accelerator.
    pub stream_mode: RegexAcceleratorStreamMode,
    /// Match schema emitted by the accelerator.
    pub match_schema: RegexAcceleratorMatchSchema,
    /// Unsupported reason for fail-closed capability records.
    pub unsupported_reason: &'static str,
}

impl RegexAcceleratorCapability {
    /// Construct a fail-closed unsupported capability record.
    #[must_use]
    pub const fn unsupported(
        backend: &'static str,
        accelerator_class: RegexAcceleratorClass,
        unsupported_reason: &'static str,
    ) -> Self {
        Self {
            backend,
            accelerator_class,
            supported: false,
            device_signature: "",
            rule_capacity: 0,
            stream_mode: RegexAcceleratorStreamMode::Unavailable,
            match_schema: RegexAcceleratorMatchSchema::Unavailable,
            unsupported_reason,
        }
    }

    /// Construct a supported accelerator capability record.
    #[must_use]
    pub const fn supported(
        backend: &'static str,
        accelerator_class: RegexAcceleratorClass,
        device_signature: &'static str,
        rule_capacity: u32,
        stream_mode: RegexAcceleratorStreamMode,
        match_schema: RegexAcceleratorMatchSchema,
    ) -> Self {
        Self {
            backend,
            accelerator_class,
            supported: true,
            device_signature,
            rule_capacity,
            stream_mode,
            match_schema,
            unsupported_reason: "",
        }
    }

    /// Fail closed when a caller requires hardware regex acceleration.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::UnsupportedFeature`] when this backend does not
    /// advertise the requested accelerator class.
    pub fn require_supported(self) -> Result<Self, BackendError> {
        if self.supported {
            return Ok(self);
        }
        Err(BackendError::UnsupportedFeature {
            name: format!("regex_accelerator:{}", self.accelerator_class.as_str()),
            backend: self.backend.to_string(),
        })
    }

    /// Emit benchmark evidence for this regex accelerator capability.
    #[must_use]
    pub const fn evidence(self, transfer_bytes: u64) -> RegexAcceleratorEvidence {
        RegexAcceleratorEvidence {
            schema_version: REGEX_ACCELERATOR_EVIDENCE_SCHEMA_VERSION,
            backend: self.backend,
            accelerator_class: self.accelerator_class,
            supported: self.supported,
            device_signature: self.device_signature,
            rule_capacity: self.rule_capacity,
            stream_mode: self.stream_mode,
            match_schema: self.match_schema,
            unsupported_reason: self.unsupported_reason,
            transfer_bytes,
            match_parity_required: true,
        }
    }
}

/// Regex accelerator benchmark evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegexAcceleratorEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Backend id that produced this evidence.
    pub backend: &'static str,
    /// Accelerator class under comparison.
    pub accelerator_class: RegexAcceleratorClass,
    /// True only when a real capability record exists.
    pub supported: bool,
    /// Device signature for supported accelerators.
    pub device_signature: &'static str,
    /// Maximum rule count accepted by the accelerator.
    pub rule_capacity: u32,
    /// Stream mode accepted by the accelerator.
    pub stream_mode: RegexAcceleratorStreamMode,
    /// Match schema emitted by the accelerator.
    pub match_schema: RegexAcceleratorMatchSchema,
    /// Unsupported reason when no accelerator is available.
    pub unsupported_reason: &'static str,
    /// Host/device transfer bytes attributed to the accelerator path.
    pub transfer_bytes: u64,
    /// True when software and accelerator outputs must be compared.
    pub match_parity_required: bool,
}

impl RegexAcceleratorEvidence {
    /// Return true when the evidence cannot overclaim accelerator support.
    #[must_use]
    pub fn is_complete(self) -> bool {
        if self.schema_version != REGEX_ACCELERATOR_EVIDENCE_SCHEMA_VERSION {
            return false;
        }
        if !self.match_parity_required || self.backend.is_empty() {
            return false;
        }
        if self.supported {
            !self.device_signature.is_empty()
                && self.rule_capacity != 0
                && self.stream_mode != RegexAcceleratorStreamMode::Unavailable
                && self.match_schema != RegexAcceleratorMatchSchema::Unavailable
                && self.unsupported_reason.is_empty()
        } else {
            self.device_signature.is_empty()
                && self.rule_capacity == 0
                && self.stream_mode == RegexAcceleratorStreamMode::Unavailable
                && self.match_schema == RegexAcceleratorMatchSchema::Unavailable
                && !self.unsupported_reason.is_empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_regex_accelerator_fails_closed_with_evidence() {
        let capability = RegexAcceleratorCapability::unsupported(
            "wgpu",
            RegexAcceleratorClass::RxpLike,
            "backend has no RXP-like regex accelerator",
        );

        let error = capability
            .require_supported()
            .expect_err("Fix: unsupported regex accelerator capability must fail closed");
        match error {
            BackendError::UnsupportedFeature { name, backend } => {
                assert_eq!(name, "regex_accelerator:rxp_like");
                assert_eq!(backend, "wgpu");
            }
            other => panic!("expected UnsupportedFeature, got {other:?}"),
        }
        let evidence = capability.evidence(0);
        assert!(!evidence.supported);
        assert_eq!(evidence.unsupported_reason, "backend has no RXP-like regex accelerator");
        assert_eq!(evidence.stream_mode, RegexAcceleratorStreamMode::Unavailable);
        assert_eq!(evidence.match_schema, RegexAcceleratorMatchSchema::Unavailable);
        assert!(evidence.match_parity_required);
        assert!(evidence.is_complete());
    }

    #[test]
    fn supported_regex_accelerator_reports_device_signature_and_schema() {
        let capability = RegexAcceleratorCapability::supported(
            "rxp",
            RegexAcceleratorClass::RxpLike,
            "bluefield-rxp:v1",
            4096,
            RegexAcceleratorStreamMode::StatefulStreaming,
            RegexAcceleratorMatchSchema::StreamPatternIdOffsets,
        );

        let supported = capability
            .require_supported()
            .expect("Fix: supported regex accelerator capability should pass");
        let evidence = supported.evidence(8192);

        assert!(evidence.supported);
        assert_eq!(evidence.device_signature, "bluefield-rxp:v1");
        assert_eq!(evidence.rule_capacity, 4096);
        assert_eq!(
            evidence.stream_mode,
            RegexAcceleratorStreamMode::StatefulStreaming
        );
        assert_eq!(
            evidence.match_schema,
            RegexAcceleratorMatchSchema::StreamPatternIdOffsets
        );
        assert_eq!(evidence.transfer_bytes, 8192);
        assert!(evidence.match_parity_required);
        assert!(evidence.is_complete());
    }
}

//! Soundness regime markers for dataflow primitives.
//!
//! Rules with zero-FP precision contracts MUST only compose primitives
//! whose marker is [`Soundness::Exact`], or [`Soundness::MayOver`]
//! primitives gated by an explicit sanitizer filter downstream.
//!
//! Lives in `vyre-foundation` because soundness is a primitive lattice
//! over IR-level analyses; dataflow engines and composition crates both
//! consume it. Per the LEGO discipline (consumers call vyre, vyre never calls
//! consumers) the canonical definition must live in vyre.

/// Soundness regime of a dataflow primitive.
///
/// Rules with zero-FP precision contracts MUST only compose primitives
/// whose marker is `Exact`, or `MayOver` primitives gated by an explicit
/// sanitizer filter downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Soundness {
    /// Over-approximates: may report taint where none exists. Safe for
    /// recall-driven rules paired with a downstream filter.
    MayOver,
    /// Under-approximates: may miss taint that exists. Safe only when
    /// the rule semantics explicitly accept false negatives.
    MustUnder,
    /// Exact: reports taint iff taint exists on the given CFG. No false
    /// positives, no false negatives, given a correct input AST.
    Exact,
}

/// Precision contract requested by a consumer pipeline.
///
/// This is the policy layer above individual primitive markers. A
/// pipeline that promises zero false positives cannot freely compose
/// every `MayOver` analysis; it must either stay `Exact` end to end or
/// prove that a downstream sanitizer filter bounds the over-approximate
/// primitive before the result escapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PrecisionContract {
    /// Results must not contain false positives.
    ZeroFalsePositive,
    /// Results must not contain false negatives.
    RecallDriven,
    /// The consumer explicitly accepts false negatives.
    FalseNegativesAccepted,
}

/// Soundness evidence for one primitive in a composed pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PrimitiveSoundness {
    /// Stable primitive id, normally the `vyre_harness::OpEntry::id`.
    pub op_id: &'static str,
    /// Primitive soundness marker.
    pub soundness: Soundness,
    /// Whether a downstream sanitizer/filter makes a `MayOver` primitive
    /// safe for a zero-false-positive consumer.
    pub sanitizer_filter: bool,
}

/// Serializable soundness evidence for one primitive in a finding or release
/// artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DynamicPrimitiveSoundness {
    /// Stable primitive id, normally the `vyre_harness::OpEntry::id`.
    pub op_id: String,
    /// Primitive soundness marker.
    pub soundness: Soundness,
    /// Whether a downstream sanitizer/filter makes a `MayOver` primitive
    /// safe for a zero-false-positive consumer.
    pub sanitizer_filter: bool,
}

impl PrimitiveSoundness {
    /// Construct primitive soundness evidence with no sanitizer filter.
    #[must_use]
    pub const fn new(op_id: &'static str, soundness: Soundness) -> Self {
        Self {
            op_id,
            soundness,
            sanitizer_filter: false,
        }
    }

    /// Mark this primitive as bounded by an explicit downstream filter.
    #[must_use]
    pub const fn with_sanitizer_filter(mut self) -> Self {
        self.sanitizer_filter = true;
        self
    }
}

impl DynamicPrimitiveSoundness {
    /// Construct serializable primitive soundness evidence with no sanitizer
    /// filter.
    #[must_use]
    pub fn new(op_id: impl Into<String>, soundness: Soundness) -> Self {
        Self {
            op_id: op_id.into(),
            soundness,
            sanitizer_filter: false,
        }
    }

    /// Mark this primitive as bounded by an explicit downstream filter.
    #[must_use]
    pub fn with_sanitizer_filter(mut self) -> Self {
        self.sanitizer_filter = true;
        self
    }
}

/// Mechanical rejection reason for an invalid soundness composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SoundnessViolation {
    /// Primitive that violates the requested consumer contract.
    pub op_id: &'static str,
    /// Primitive soundness marker.
    pub soundness: Soundness,
    /// Consumer policy that rejected the primitive.
    pub contract: PrecisionContract,
    /// Human-readable fix direction.
    pub fix: &'static str,
}

/// Mechanical rejection reason for an invalid dynamic soundness composition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DynamicSoundnessViolation {
    /// Primitive that violates the requested consumer contract.
    pub op_id: String,
    /// Primitive soundness marker.
    pub soundness: Soundness,
    /// Consumer policy that rejected the primitive.
    pub contract: PrecisionContract,
    /// Human-readable fix direction.
    pub fix: &'static str,
}

impl Soundness {
    /// Conservative join of two soundness markers.
    ///
    /// The join is the least precise soundness that soundly describes
    /// the composition of two primitives.
    #[must_use]
    pub const fn join(self, other: Soundness) -> Soundness {
        match (self, other) {
            (Soundness::MayOver, _) | (_, Soundness::MayOver) => Soundness::MayOver,
            (Soundness::MustUnder, Soundness::MustUnder) => Soundness::MustUnder,
            (Soundness::MustUnder, Soundness::Exact) | (Soundness::Exact, Soundness::MustUnder) => {
                Soundness::MustUnder
            }
            (Soundness::Exact, Soundness::Exact) => Soundness::Exact,
        }
    }
}

/// Validate that a primitive pipeline can satisfy a consumer precision
/// contract, returning the composed soundness marker on success.
///
/// This is intentionally conservative. A `ZeroFalsePositive` pipeline
/// rejects `MustUnder` because under-approximation can hide required
/// sanitizer evidence, and rejects unfiltered `MayOver` because that
/// can leak false positives to the consumer. A `RecallDriven` pipeline
/// rejects `MustUnder` because false negatives break recall.
///
/// # Errors
///
/// Returns [`SoundnessViolation`] when any primitive is incompatible with the
/// requested `contract`.
pub fn validate_pipeline(
    contract: PrecisionContract,
    primitives: &[PrimitiveSoundness],
) -> Result<Soundness, SoundnessViolation> {
    let mut joined = Soundness::Exact;
    for primitive in primitives {
        validate_primitive(contract, *primitive)?;
        joined = joined.join(primitive.soundness);
    }
    Ok(joined)
}

/// Validate a serializable primitive pipeline against a consumer precision
/// contract, returning the composed soundness marker on success.
///
/// This has the same semantics as [`validate_pipeline`] but accepts owned
/// primitive ids for finding evidence, release artifacts, and decoded manifests.
///
/// # Errors
///
/// Returns [`DynamicSoundnessViolation`] when any primitive is incompatible
/// with the requested `contract`.
pub fn validate_dynamic_pipeline(
    contract: PrecisionContract,
    primitives: &[DynamicPrimitiveSoundness],
) -> Result<Soundness, DynamicSoundnessViolation> {
    let mut joined = Soundness::Exact;
    for primitive in primitives {
        validate_dynamic_primitive(contract, primitive)?;
        joined = joined.join(primitive.soundness);
    }
    Ok(joined)
}

/// Validate one primitive against a consumer precision contract.
///
/// # Errors
///
/// Returns [`SoundnessViolation`] when `primitive` cannot soundly satisfy
/// `contract`.
pub fn validate_primitive(
    contract: PrecisionContract,
    primitive: PrimitiveSoundness,
) -> Result<(), SoundnessViolation> {
    match violation_fix(contract, primitive.soundness, primitive.sanitizer_filter) {
        None => Ok(()),
        Some(fix) => Err(SoundnessViolation {
            op_id: primitive.op_id,
            soundness: primitive.soundness,
            contract,
            fix,
        }),
    }
}

/// Validate one serializable primitive against a consumer precision contract.
///
/// # Errors
///
/// Returns [`DynamicSoundnessViolation`] when `primitive` cannot soundly
/// satisfy `contract`.
pub fn validate_dynamic_primitive(
    contract: PrecisionContract,
    primitive: &DynamicPrimitiveSoundness,
) -> Result<(), DynamicSoundnessViolation> {
    match violation_fix(contract, primitive.soundness, primitive.sanitizer_filter) {
        None => Ok(()),
        Some(fix) => Err(DynamicSoundnessViolation {
            op_id: primitive.op_id.clone(),
            soundness: primitive.soundness,
            contract,
            fix,
        }),
    }
}

fn violation_fix(
    contract: PrecisionContract,
    soundness: Soundness,
    sanitizer_filter: bool,
) -> Option<&'static str> {
    match (contract, soundness, sanitizer_filter) {
        (PrecisionContract::ZeroFalsePositive, Soundness::Exact, _)
        | (PrecisionContract::ZeroFalsePositive, Soundness::MayOver, true)
        | (PrecisionContract::RecallDriven, Soundness::Exact | Soundness::MayOver, _)
        | (PrecisionContract::FalseNegativesAccepted, _, _) => None,
        (PrecisionContract::ZeroFalsePositive, Soundness::MayOver, false) => {
            Some("Fix: add an explicit sanitizer filter or use only Exact primitives.")
        }
        (PrecisionContract::ZeroFalsePositive, Soundness::MustUnder, _) => {
            Some("Fix: zero-FP pipelines require Exact primitives or filtered MayOver primitives.")
        }
        (PrecisionContract::RecallDriven, Soundness::MustUnder, _) => {
            Some("Fix: recall-driven pipelines cannot include under-approximating primitives.")
        }
    }
}

/// Trait for types that carry a soundness marker.
pub trait SoundnessTagged {
    /// Return the soundness regime of this primitive.
    fn soundness(&self) -> Soundness;
}

#[cfg(test)]
mod tests {
    use super::{
        validate_dynamic_pipeline, DynamicPrimitiveSoundness, PrecisionContract, Soundness,
    };

    #[test]
    fn dynamic_pipeline_rejects_zero_false_positive_unfiltered_mayover() {
        let error = validate_dynamic_pipeline(
            PrecisionContract::ZeroFalsePositive,
            &[DynamicPrimitiveSoundness::new(
                "vyre-libs::security::flows_to",
                Soundness::MayOver,
            )],
        )
        .expect_err("unfiltered MayOver must not satisfy zero false positive contracts");

        assert_eq!(error.op_id, "vyre-libs::security::flows_to");
        assert_eq!(error.soundness, Soundness::MayOver);
        assert_eq!(error.contract, PrecisionContract::ZeroFalsePositive);
        assert!(error.fix.contains("explicit sanitizer filter"));
    }

    #[test]
    fn dynamic_pipeline_accepts_zero_false_positive_filtered_mayover() {
        let soundness = validate_dynamic_pipeline(
            PrecisionContract::ZeroFalsePositive,
            &[DynamicPrimitiveSoundness::new(
                "vyre-libs::security::flows_to_with_sanitizer",
                Soundness::MayOver,
            )
            .with_sanitizer_filter()],
        )
        .expect("filtered MayOver should satisfy zero false positive contracts");

        assert_eq!(soundness, Soundness::MayOver);
    }
}

//! Integration contracts for soundness lattice and pipeline precision policy.

use vyre_foundation::soundness::{
    validate_pipeline, PrecisionContract, PrimitiveSoundness, Soundness,
};

#[test]
fn may_over_join_must_under_is_may_over() {
    assert_eq!(
        Soundness::MayOver.join(Soundness::MustUnder),
        Soundness::MayOver
    );
    assert_eq!(
        Soundness::MustUnder.join(Soundness::MayOver),
        Soundness::MayOver
    );
}

#[test]
fn zero_fp_pipeline_accepts_exact_primitives() {
    let joined = validate_pipeline(
        PrecisionContract::ZeroFalsePositive,
        &[
            PrimitiveSoundness::new("vyre::ssa", Soundness::Exact),
            PrimitiveSoundness::new("vyre::reaching_def", Soundness::Exact),
        ],
    )
    .expect("exact primitives should satisfy a zero-FP pipeline");
    assert_eq!(joined, Soundness::Exact);
}

#[test]
fn zero_fp_pipeline_rejects_unfiltered_may_over() {
    let err = validate_pipeline(
        PrecisionContract::ZeroFalsePositive,
        &[PrimitiveSoundness::new(
            "vyre::points_to",
            Soundness::MayOver,
        )],
    )
    .expect_err("unfiltered MayOver primitive must not satisfy zero-FP");
    assert_eq!(err.op_id, "vyre::points_to");
    assert_eq!(err.soundness, Soundness::MayOver);
}

#[test]
fn zero_fp_pipeline_accepts_filtered_may_over() {
    let joined = validate_pipeline(
        PrecisionContract::ZeroFalsePositive,
        &[PrimitiveSoundness::new("vyre::points_to", Soundness::MayOver).with_sanitizer_filter()],
    )
    .expect("sanitizer-filtered MayOver primitive should be allowed");
    assert_eq!(joined, Soundness::MayOver);
}

#[test]
fn recall_driven_pipeline_rejects_under_approximation() {
    let err = validate_pipeline(
        PrecisionContract::RecallDriven,
        &[PrimitiveSoundness::new(
            "vyre::lossy_summary",
            Soundness::MustUnder,
        )],
    )
    .expect_err("recall-driven pipelines cannot include MustUnder primitives");
    assert_eq!(err.contract, PrecisionContract::RecallDriven);
}

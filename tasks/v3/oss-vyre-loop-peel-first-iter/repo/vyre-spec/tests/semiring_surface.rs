//! External contract tests for the frozen semiring selector.
//!
//! Semiring tags cross optimizer, lowering, and primitive boundaries. These
//! tests pin the public selector names and accumulator identities so downstream
//! backends cannot silently reinterpret dataflow algebra.

use std::collections::BTreeSet;

use vyre_spec::Semiring;

#[test]
fn semiring_accumulator_identities_are_exact() {
    let cases = [
        (Semiring::Real, 0),
        (Semiring::MinPlus, u32::MAX),
        (Semiring::MaxPlus, 0),
        (Semiring::MaxTimes, 0),
        (Semiring::BoolOr, 0),
        (Semiring::BoolAnd, u32::MAX),
        (Semiring::Gf2, 0),
        (Semiring::Lineage, 0),
    ];

    for (semiring, identity) in cases {
        assert_eq!(
            semiring.identity(),
            identity,
            "{semiring:?} accumulator identity changed"
        );
    }
}

#[test]
fn semiring_serde_names_are_exact_and_unique() {
    let cases = [
        (Semiring::Real, "\"Real\""),
        (Semiring::MinPlus, "\"MinPlus\""),
        (Semiring::MaxPlus, "\"MaxPlus\""),
        (Semiring::MaxTimes, "\"MaxTimes\""),
        (Semiring::BoolOr, "\"BoolOr\""),
        (Semiring::BoolAnd, "\"BoolAnd\""),
        (Semiring::Gf2, "\"Gf2\""),
        (Semiring::Lineage, "\"Lineage\""),
    ];

    let mut encoded_names = BTreeSet::new();
    for (semiring, expected_json) in cases {
        let encoded = serde_json::to_string(&semiring).expect("Semiring must serialize");
        assert_eq!(encoded, expected_json, "{semiring:?} serde tag changed");
        assert!(
            encoded_names.insert(encoded.clone()),
            "duplicate Semiring serde tag {encoded}"
        );

        let decoded: Semiring = serde_json::from_str(&encoded).expect("Semiring must deserialize");
        assert_eq!(decoded, semiring, "{semiring:?} serde round-trip changed");
    }
}

#[test]
fn semiring_rejects_unknown_serde_name() {
    let error = serde_json::from_str::<Semiring>("\"CustomGpuSemiring\"")
        .expect_err("unknown Semiring tags must not deserialize");
    assert!(
        error.to_string().contains("unknown variant"),
        "unexpected Semiring serde error: {error}"
    );
}

#[test]
fn semiring_hash_and_ordering_inputs_have_no_aliases() {
    let variants = [
        Semiring::Real,
        Semiring::MinPlus,
        Semiring::MaxPlus,
        Semiring::MaxTimes,
        Semiring::BoolOr,
        Semiring::BoolAnd,
        Semiring::Gf2,
        Semiring::Lineage,
    ];

    let unique: BTreeSet<String> = variants
        .iter()
        .map(|variant| format!("{variant:?}"))
        .collect();
    assert_eq!(
        unique.len(),
        variants.len(),
        "Semiring debug tags must be unique"
    );

    for left in variants {
        for right in variants {
            assert_eq!(
                left == right,
                format!("{left:?}") == format!("{right:?}"),
                "Semiring equality no longer matches the frozen variant identity"
            );
        }
    }
}

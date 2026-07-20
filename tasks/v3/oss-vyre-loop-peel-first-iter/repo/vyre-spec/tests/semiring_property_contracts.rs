//! Generated property coverage for semiring selector contracts.

use proptest::prelude::*;
use vyre_spec::Semiring;

fn semiring_strategy() -> impl Strategy<Value = Semiring> {
    prop_oneof![
        Just(Semiring::Real),
        Just(Semiring::MinPlus),
        Just(Semiring::MaxPlus),
        Just(Semiring::MaxTimes),
        Just(Semiring::BoolOr),
        Just(Semiring::BoolAnd),
        Just(Semiring::Gf2),
        Just(Semiring::Lineage),
    ]
}

proptest! {
    #[test]
    fn generated_semirings_round_trip_through_json(semiring in semiring_strategy()) {
        let encoded = serde_json::to_string(&semiring)
            .expect("Fix: Semiring must serialize through the frozen spec contract");
        let decoded: Semiring = serde_json::from_str(&encoded)
            .expect("Fix: Semiring JSON must deserialize through the frozen spec contract");

        prop_assert_eq!(decoded, semiring);
    }

    #[test]
    fn generated_semiring_identities_are_exact_sentinels(semiring in semiring_strategy()) {
        let identity = semiring.identity();
        match semiring {
            Semiring::MinPlus | Semiring::BoolAnd => prop_assert_eq!(identity, u32::MAX),
            _ => prop_assert_eq!(identity, 0),
        }
    }
}

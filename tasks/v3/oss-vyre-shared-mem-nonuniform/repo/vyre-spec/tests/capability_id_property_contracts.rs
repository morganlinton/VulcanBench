//! Property gates for `CapabilityId` string identity.

use proptest::prelude::*;
use vyre_spec::CapabilityId;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn as_str_round_trips_name(name in "[a-z][a-z0-9_.-]{0,48}") {
        let id = CapabilityId::new(name.clone());
        prop_assert_eq!(id.as_str(), name.as_str());
    }

    #[test]
    fn distinct_names_produce_distinct_ids(
        a in "[a-z]{1,12}",
        b in "[a-z]{1,12}",
    ) {
        prop_assume!(a != b);
        let first = CapabilityId::new(a);
        let second = CapabilityId::new(b);

        prop_assert_ne!(first.as_str(), second.as_str());
    }

    #[test]
    fn new_is_idempotent_on_same_name(name in "[a-z]{1,20}") {
        let c1 = CapabilityId::new(name.clone());
        let c2 = CapabilityId::new(name);
        prop_assert_eq!(c1.as_str(), c2.as_str());
    }
}

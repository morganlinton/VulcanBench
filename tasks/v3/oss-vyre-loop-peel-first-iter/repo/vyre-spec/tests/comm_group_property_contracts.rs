//! Property gates for `CommGroup` identifiers.

use proptest::prelude::*;
use vyre_spec::CommGroup;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn as_u32_round_trips(id in any::<u32>()) {
        let group = CommGroup(id);
        prop_assert_eq!(group.as_u32(), id);
    }

    #[test]
    fn equality_follows_inner_id(a in any::<u32>(), b in any::<u32>()) {
        prop_assert_eq!(CommGroup(a) == CommGroup(b), a == b);
    }
}

#[test]
fn world_group_is_zero() {
    assert_eq!(CommGroup::WORLD.as_u32(), 0);
}

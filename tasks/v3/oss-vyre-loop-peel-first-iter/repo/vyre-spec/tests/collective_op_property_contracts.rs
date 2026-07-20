//! Property gates for `CollectiveOp` wire tags.

use proptest::prelude::*;
use vyre_spec::CollectiveOp;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn builtin_wire_tag_round_trips(op in 0u8..=5u8) {
        let collective = match op {
            0 => CollectiveOp::Sum,
            1 => CollectiveOp::Min,
            2 => CollectiveOp::Max,
            3 => CollectiveOp::BitAnd,
            4 => CollectiveOp::BitOr,
            _ => CollectiveOp::BitXor,
        };
        let tag = collective.builtin_wire_tag();
        prop_assert_eq!(CollectiveOp::from_wire_tag(tag).unwrap(), collective);
    }

    #[test]
    fn unknown_tags_are_rejected(tag in any::<u8>()) {
        prop_assume!(!matches!(tag, 0x01..=0x06));
        prop_assert!(CollectiveOp::from_wire_tag(tag).is_err());
    }
}

#[test]
fn wire_tags_are_unique_and_nonzero() {
    let ops = [
        CollectiveOp::Sum,
        CollectiveOp::Min,
        CollectiveOp::Max,
        CollectiveOp::BitAnd,
        CollectiveOp::BitOr,
        CollectiveOp::BitXor,
    ];
    let tags: Vec<u8> = ops.iter().map(|op| op.builtin_wire_tag()).collect();

    assert!(tags.iter().all(|tag| *tag != 0));
    let mut seen = std::collections::HashSet::new();
    for tag in tags {
        assert!(seen.insert(tag));
    }
}

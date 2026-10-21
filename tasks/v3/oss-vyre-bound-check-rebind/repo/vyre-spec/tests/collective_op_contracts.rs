//! Frozen RFC-0004 collective operation contracts.

use vyre_spec::{CollectiveOp, CommGroup};

#[test]
fn collective_op_wire_tags_are_dense_and_frozen() {
    let cases = [
        (CollectiveOp::Sum, 0x01),
        (CollectiveOp::Min, 0x02),
        (CollectiveOp::Max, 0x03),
        (CollectiveOp::BitAnd, 0x04),
        (CollectiveOp::BitOr, 0x05),
        (CollectiveOp::BitXor, 0x06),
    ];

    for (op, tag) in cases {
        assert_eq!(
            op.builtin_wire_tag(),
            tag,
            "Fix: RFC-0004 collective op tags are part of the public wire ABI."
        );
        assert_eq!(
            CollectiveOp::from_wire_tag(tag).expect("assigned tag must decode"),
            op,
            "Fix: CollectiveOp tag {tag} must decode to its frozen operator."
        );
    }
}

#[test]
fn collective_op_wire_decoder_rejects_unassigned_tags() {
    for tag in [0, 7, 0xff] {
        let error =
            CollectiveOp::from_wire_tag(tag).expect_err("unassigned collective op tags must fail");
        assert!(
            error.contains("Fix: unknown CollectiveOp tag"),
            "Fix: collective op decode failures must be actionable, got `{error}`."
        );
    }
}

#[test]
fn world_comm_group_is_stable_zero() {
    assert_eq!(
        CommGroup::WORLD.as_u32(),
        0,
        "Fix: group 0 is the stable world communicator id."
    );
    assert_eq!(CommGroup(17).as_u32(), 17);
}

//! Boundary contracts for the CPU subgroup simulator.
//!
//! The simulator defines host-side oracle behavior for lane collectives. These
//! tests pin width clamping, active-lane truncation, and out-of-range lane
//! behavior that GPU backends must match during parity checks.

use vyre_reference::subgroup::SubgroupSimulator;

#[test]
fn zero_width_is_clamped_to_one_lane() {
    let simulator = SubgroupSimulator::new(0);

    assert_eq!(simulator.width(), 1);
    assert_eq!(simulator.ballot_slice(&[true, true, true]), 0b1);
    assert_eq!(simulator.add(&[u32::MAX, 1]), u32::MAX);
}

#[test]
fn ballot_ignores_lanes_past_configured_width_and_u32_mask_width() {
    let simulator = SubgroupSimulator::new(64);
    let mask = vec![true; 64];
    assert_eq!(simulator.ballot_slice(&mask), u32::MAX);

    let simulator = SubgroupSimulator::new(3);
    assert_eq!(
        simulator.ballot_slice(&[true, false, true, true, true]),
        0b101
    );
}

#[test]
fn shuffle_truncates_to_active_width_and_zeroes_missing_sources() {
    let simulator = SubgroupSimulator::new(3);

    assert_eq!(
        simulator.shuffle(&[10, 20, 30, 40], &[2, 9, 0, 1]),
        vec![30, 0, 10]
    );
    assert_eq!(simulator.shuffle(&[10, 20], &[1, 0, 0]), vec![20, 10]);
    assert_eq!(simulator.shuffle(&[10, 20, 30], &[0]), vec![10]);
}

#[test]
fn add_is_wrapping_and_width_limited() {
    let simulator = SubgroupSimulator::new(3);

    assert_eq!(simulator.add(&[u32::MAX, 1, 5, 99]), 5);
}

#[test]
fn subgroup_bounds_partition_valid_lane_indices() {
    let simulator = SubgroupSimulator::new(4);

    assert_eq!(simulator.subgroup_bounds(10, 0), (0, 4));
    assert_eq!(simulator.subgroup_bounds(10, 3), (0, 4));
    assert_eq!(simulator.subgroup_bounds(10, 4), (4, 8));
    assert_eq!(simulator.subgroup_bounds(10, 8), (8, 10));
    assert_eq!(simulator.subgroup_bounds(0, 0), (0, 0));
}

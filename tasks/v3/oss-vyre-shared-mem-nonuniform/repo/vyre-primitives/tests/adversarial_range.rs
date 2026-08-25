//! Failure-oriented adversarial tests for vyre-primitives::range.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.

use vyre_primitives::range::ByteRange;

#[test]
fn new_panics_on_reversed_range() {
    let cases: [(u32, u32, u32); 6] = [
        (0, 10, 5),
        (0, u32::MAX, 0),
        (0, 1, 0),
        (42, 100, 99),
        (0, 100, 0),
        (u32::MAX, 1, 0),
    ];
    for (tag, start, end) in cases {
        let result = std::panic::catch_unwind(|| ByteRange::new(tag, start, end));
        result.expect_err("ByteRange::new({tag}, {start}, {end}) must panic when end < start");
    }
}

#[test]
fn len_at_boundaries() {
    let cases = [
        ((0, 0, 0), 0),
        ((0, 0, 1), 1),
        ((0, 0, u32::MAX), u32::MAX),
        ((0, 1, 1), 0),
        ((0, u32::MAX - 1, u32::MAX), 1),
    ];
    for ((tag, start, end), expected) in cases {
        let r = ByteRange::new(tag, start, end);
        assert_eq!(r.len(), expected, "len mismatch for ({tag},{start},{end})");
    }
}

#[test]
fn is_empty_boundary() {
    let r1 = ByteRange::new(0, 5, 5);
    assert!(r1.is_empty());
    let r2 = ByteRange::new(0, 5, 6);
    assert!(!r2.is_empty());
}

#[test]
fn contains_property_invariants() {
    let outer = ByteRange::new(0, 0, 100);
    let inner = ByteRange::new(0, 10, 90);
    assert!(outer.contains(&inner));
    assert!(!inner.contains(&outer));
    assert!(outer.contains(&outer)); // reflexive
}

#[test]
fn ends_before_property_invariants() {
    let a = ByteRange::new(0, 0, 10);
    let b = ByteRange::new(0, 10, 20);
    let c = ByteRange::new(0, 5, 15);
    let d = ByteRange::new(0, 10, 10);
    assert!(a.ends_before(&b));
    assert!(!a.ends_before(&c));
    assert!(a.ends_before(&d)); // a.end == d.start, disjoint
    assert!(!a.ends_before(&a)); // a overlaps itself
}

#[test]
fn layout_is_repr_c_u32x3() {
    // The layout stability is load-bearing for backend marshalling and FFI.
    assert_eq!(std::mem::size_of::<ByteRange>(), 12);
    assert_eq!(std::mem::align_of::<ByteRange>(), 4);
}

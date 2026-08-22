//! Hidden pass-to-pass guards: addition, accessors and until() unchanged.

use jiff_core::tz::Offset;

#[test]
fn vb_checked_add_still_adds() {
    assert_eq!(Offset::UTC.checked_add(3600).unwrap(), Offset::constant_seconds(3600));
    assert!(Offset::MAX.checked_add(1).is_err());
}

#[test]
fn vb_accessors_and_until() {
    let a = Offset::constant_seconds(-3600);
    let b = Offset::constant_seconds(1800);
    assert_eq!(a.seconds(), -3600);
    assert_eq!(a.until(b), 5400);
}

// Errors at base too (range check catches it either way): a guard.
#[test]
fn vb_rejects_i32_min_subtrahend() {
    // Negating i32::MIN overflows i32; the checked routine must error, not
    // wrap or panic.
    assert!(Offset::UTC.checked_sub(i32::MIN).is_err());
}

//! Hidden fail-to-pass tests: jiff-core's Offset checked subtraction must
//! subtract. Compiles at base (public API) and fails there because the
//! routine added the seconds instead.

use jiff_core::tz::Offset;

#[test]
fn vb_subtracts_positive_seconds() {
    let got = Offset::UTC.checked_sub(1).unwrap();
    assert_eq!(got, Offset::constant_seconds(-1));
}

#[test]
fn vb_subtracts_negative_seconds() {
    let got = Offset::UTC.checked_sub(-1).unwrap();
    assert_eq!(got, Offset::constant_seconds(1));
}

#[test]
fn vb_rejects_overflow_at_bounds() {
    assert!(Offset::MIN.checked_sub(1).is_err());
    assert!(Offset::MAX.checked_sub(-1).is_err());
}

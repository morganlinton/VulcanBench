// Hidden fail_to_pass tests: TryFrom<SignedDuration> for std::time::Duration
// must return Err (not panic) when the signed duration has a zero second
// component and a negative subsecond component. At the unfixed base these
// inputs reach `u32::try_from(negative).unwrap()` and panic.
use std::time::Duration;

use jiff::SignedDuration;

#[test]
fn zero_secs_minus_one_nano_is_err() {
    assert!(Duration::try_from(SignedDuration::new(0, -1)).is_err());
}

#[test]
fn zero_secs_max_negative_subsec_is_err() {
    assert!(Duration::try_from(SignedDuration::new(0, -999_999_999)).is_err());
}

#[test]
fn zero_secs_half_negative_subsec_is_err() {
    assert!(Duration::try_from(SignedDuration::new(0, -500_000_000)).is_err());
}

// pass_to_pass regression guards: these hold at the unfixed base and must
// continue to hold after the fix. They do NOT reference the fixed code path.
use std::time::Duration;

use jiff::SignedDuration;

#[test]
fn non_negative_roundtrips_ok() {
    let dur = Duration::try_from(SignedDuration::new(5, 123_000_000)).expect("ok");
    assert_eq!(dur, Duration::new(5, 123_000_000));
}

#[test]
fn negative_seconds_already_errors() {
    // as_secs() < 0 fails the u64 conversion at base already (no panic).
    assert!(Duration::try_from(SignedDuration::new(-5, 0)).is_err());
}

#[test]
fn from_std_duration_roundtrips() {
    let sd = SignedDuration::try_from(Duration::new(5, 123_000_000)).expect("ok");
    assert_eq!(sd, SignedDuration::new(5, 123_000_000));
}

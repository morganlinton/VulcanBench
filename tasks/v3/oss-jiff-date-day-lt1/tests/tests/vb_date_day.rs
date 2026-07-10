// Hidden fail_to_pass tests: Date::new must reject a day component < 1.
// At the unfixed base the constructor only checks day > days_in_month, so
// zero and negative days slip through and return Ok.
use jiff::civil::Date;

#[test]
fn zero_day_is_err() {
    assert!(Date::new(1998, 1, 0).is_err());
}

#[test]
fn negative_day_is_err() {
    assert!(Date::new(1998, 1, -1).is_err());
}

#[test]
fn large_negative_day_is_err() {
    assert!(Date::new(2020, 6, -5).is_err());
}

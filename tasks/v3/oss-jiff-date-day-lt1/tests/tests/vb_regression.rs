// pass_to_pass regression guards: valid dates stay valid, and days beyond the
// month length stay rejected. These hold at base and after the fix.
use jiff::civil::Date;

#[test]
fn leap_day_is_ok() {
    assert!(Date::new(2004, 2, 29).is_ok());
}

#[test]
fn non_leap_feb29_is_err() {
    assert!(Date::new(1998, 2, 29).is_err());
}

#[test]
fn ordinary_date_is_ok() {
    assert!(Date::new(2024, 12, 31).is_ok());
}

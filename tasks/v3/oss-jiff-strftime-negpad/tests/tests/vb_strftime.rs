// Hidden fail_to_pass tests: strftime must count the minus sign as part of the
// requested field width when formatting negative values. At the unfixed base
// the sign is written first and the digits are padded to the full width, so a
// negative year is one character too wide.
use jiff::civil::Date;

fn fmt(f: &str, y: i16, m: i8, d: i8) -> String {
    Date::new(y, m, d).unwrap().strftime(f).to_string()
}

#[test]
fn year_default_width_negative() {
    // %Y pads to width 4 including the sign: "-024", not "-0024".
    assert_eq!(fmt("%Y", -24, 7, 14), "-024");
}

#[test]
fn iso_week_year_default_width_negative() {
    assert_eq!(fmt("%G", -19, 11, 30), "-019");
}

#[test]
fn year_zero_padded_negative() {
    // %06Y: total width 6 including the sign: "-02025", not "-002025".
    assert_eq!(fmt("%06Y", -2025, 1, 13), "-02025");
}

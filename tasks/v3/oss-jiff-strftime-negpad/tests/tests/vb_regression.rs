// pass_to_pass regression guards: positive values do not hit the negative-sign
// path, so their padding is unchanged. These hold at base and after the fix.
use jiff::civil::Date;

fn fmt(f: &str, y: i16, m: i8, d: i8) -> String {
    Date::new(y, m, d).unwrap().strftime(f).to_string()
}

#[test]
fn year_default_width_positive() {
    assert_eq!(fmt("%Y", 2024, 7, 14), "2024");
}

#[test]
fn year_zero_padded_positive() {
    assert_eq!(fmt("%06Y", 2025, 1, 13), "002025");
}

#[test]
fn year_space_padded_positive() {
    assert_eq!(fmt("%_6Y", 2025, 1, 13), "  2025");
}

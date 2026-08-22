//! Hidden pass-to-pass guards: the ordinary constructor path is unchanged.
//! Compiles and passes at the base commit.

use regex::Regex;

#[test]
fn vb_regex_new_still_works() {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    assert!(re.is_match("2026-08-22"));
    assert!(!re.is_match("not a date"));
}

#[test]
fn vb_invalid_pattern_still_errors() {
    assert!(Regex::new("(unclosed").is_err());
}

#[test]
fn vb_bytes_regex_new_still_works() {
    let re = regex::bytes::Regex::new(r"(?-u)\xFF").unwrap();
    assert!(re.is_match(b"\xFF"));
}

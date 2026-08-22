//! Hidden fail-to-pass tests: the `regex!` macro — lazily compiled, statically
//! cached per call site, with a `bytes::regex!` twin. Fails to compile at the
//! base commit (the macro does not exist), so these are all fail_to_pass.

use regex::{bytes, regex, Regex};

#[test]
fn vb_macro_matches_and_coexists() {
    // More than one regex! in a scope must work (the static lives in a block).
    assert!(regex!("foo").is_match("foo"));
    assert!(regex!("bar").is_match("bar"));
    assert!(!regex!(r"^\d+$").is_match("abc"));
}

#[test]
fn vb_macro_returns_static_regex_reference() {
    // The macro yields a plain &Regex usable anywhere a &Regex is.
    fn takes_regex(re: &Regex) -> bool {
        re.is_match("2026-08-22")
    }
    assert!(takes_regex(regex!(r"^\d{4}-\d{2}-\d{2}$")));
}

#[test]
fn vb_macro_reuses_one_compilation_per_call_site() {
    fn the_regex() -> *const Regex {
        regex!(r"reuse|me") as *const Regex
    }
    // Same call site, same static: pointer identity across calls.
    assert_eq!(the_regex(), the_regex());
}

#[test]
fn vb_bytes_macro_matches() {
    assert!(bytes::regex!(r"(?-u)\xFF").is_match(b"\xFF"));
    assert!(bytes::regex!("bar|baz").is_match(b"path/to/baz"));
    assert!(!bytes::regex!("^qux$").is_match(b"quux"));
}

#[test]
fn vb_macro_captures_work() {
    let caps = regex!(r"(?<y>\d{4})-(?<m>\d{2})").captures("2026-08").unwrap();
    assert_eq!(&caps["y"], "2026");
    assert_eq!(&caps["m"], "08");
}

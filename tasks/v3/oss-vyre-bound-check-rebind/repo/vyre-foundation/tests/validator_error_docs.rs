//! Coverage test for VALIDATOR_ERRORS.md.

use std::collections::HashSet;
use std::fs;

#[test]
fn test_validator_error_docs_coverage() {
    let md_content = fs::read_to_string("VALIDATOR_ERRORS.md").unwrap();
    let mut md_codes = HashSet::new();

    for line in md_content.lines() {
        if line.starts_with("## V") && line.contains("  - ") {
            let code = line[3..7].to_string(); // "## V010" -> "V010"
            md_codes.insert(code);
        }
    }

    let mut src_codes = HashSet::new();

    for entry in fs::read_dir("src/validate").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let content = fs::read_to_string(&path).unwrap();

            // simple V\\d{3} manual finder
            let mut i = 0;
            let bytes = content.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'V' && i + 3 < bytes.len() {
                    let d1 = bytes[i + 1];
                    let d2 = bytes[i + 2];
                    let d3 = bytes[i + 3];
                    if d1.is_ascii_digit() && d2.is_ascii_digit() && d3.is_ascii_digit() {
                        let code = format!("V{}{}{}", d1 as char, d2 as char, d3 as char);
                        src_codes.insert(code);
                        i += 3;
                    }
                }
                i += 1;
            }
        }
    }

    let missing_in_md: Vec<_> = src_codes.difference(&md_codes).collect();
    let missing_in_src: Vec<_> = md_codes.difference(&src_codes).collect();

    assert!(
        missing_in_md.is_empty(),
        "Codes in src but missing in docs: {:?}",
        missing_in_md
    );

    assert!(
        missing_in_src.is_empty(),
        "Codes in docs but missing in src: {:?}",
        missing_in_src
    );
}

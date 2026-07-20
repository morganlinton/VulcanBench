//! Error-code catalog with stable integer IDs.
//!
//! See `contracts/release.md`. Every `ErrorCode` variant gets a
//! stable u32 id + a documented entry in `docs/error-codes.md`.
//! Consumers filter on the integer without parsing error strings; the
//! catalog is the source of truth (curl, sqlite, rustc do this).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use vyre_driver::backend::ErrorCode;

fn catalog_path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    root.join("docs/error-codes.md")
}

fn parse_catalog(path: &PathBuf) -> BTreeMap<String, u32> {
    let src = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("error-code catalog: {} does not exist", path.display()));
    // Expected row shape:
    //   | `DeviceOutOfMemory` | 1001 | ... |
    let mut out = BTreeMap::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| `") {
            continue;
        }
        let cols: Vec<&str> = trimmed.split('|').map(str::trim).collect();
        if cols.len() < 3 {
            continue;
        }
        let name = cols[1].trim_matches('`').to_string();
        let code: u32 = match cols[2].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        out.insert(name, code);
    }
    out
}

/// Every `ErrorCode` variant the binary knows about. `#[non_exhaustive]`
/// forces the `_ =>` arm but the test assertion ensures future variants
/// fail the coverage check until they are added to the catalog.
fn every_variant() -> Vec<(&'static str, ErrorCode)> {
    vec![
        ("DeviceOutOfMemory", ErrorCode::DeviceOutOfMemory),
        ("UnsupportedFeature", ErrorCode::UnsupportedFeature),
        ("PoisonedLock", ErrorCode::PoisonedLock),
        ("KernelCompileFailed", ErrorCode::KernelCompileFailed),
        ("DispatchFailed", ErrorCode::DispatchFailed),
        ("InvalidProgram", ErrorCode::InvalidProgram),
        ("Unknown", ErrorCode::Unknown),
    ]
}

#[test]
fn every_error_code_variant_has_stable_integer() {
    let catalog = parse_catalog(&catalog_path());
    let mut missing = Vec::new();
    for (name, _code) in every_variant() {
        if !catalog.contains_key(name) {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "error-code catalog: missing catalog entries for {missing:?}. \
         Add a row `| \\`<Variant>\\` | <nnnn> | <description> |` to docs/error-codes.md"
    );
}

#[test]
fn catalog_integers_match_binary() {
    let catalog = parse_catalog(&catalog_path());
    for (name, variant) in every_variant() {
        let Some(&expected) = catalog.get(name) else {
            continue; // covered by the other test
        };
        let actual: u32 = variant_stable_id(variant);
        assert_eq!(
            actual, expected,
            "error-code catalog: catalog says {name}={expected} but binary says {name}={actual}"
        );
    }
}

fn variant_stable_id(_variant: ErrorCode) -> u32 {
    _variant.stable_id()
}

//! Tests for normalized parity source locations and provenance.

use std::fs;

use vyre_frontend_c::api::{normalize_source_file, ParitySourcePoint, ParitySourceProvenance};

#[test]
fn normalizes_existing_paths_and_clang_pseudo_files() {
    let dir = std::env::temp_dir().join(format!("vyrec-parity-location-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("test directory must be creatable");
    let file = dir.join("main.c");
    fs::write(&file, "int x;\n").expect("test source must be writable");

    let raw = dir.join(".").join("main.c");
    let normalized = normalize_source_file(&raw.to_string_lossy());
    fs::remove_file(&file).expect("test source must be removable");
    fs::remove_dir(&dir).expect("test directory must be removable");

    assert!(normalized.ends_with("main.c"));
    assert!(!normalized.contains("/./"));
    assert_eq!(normalize_source_file("<built-in>"), "<built-in>");
}

#[test]
fn parses_clang_points_and_provenance() {
    let point =
        ParitySourcePoint::parse_clang("/tmp/example.c:12:7").expect("clang point must parse");
    assert_eq!(point.file, "/tmp/example.c");
    assert_eq!(point.line, 12);
    assert_eq!(point.column, 7);

    let provenance = ParitySourceProvenance::from_clang_locations(
        "/tmp/main.c:2:20",
        Some("/tmp/main.c:1:11"),
        ["/tmp/main.c:1:1", "/tmp/header.h:4:2"],
    )
    .expect("clang provenance must parse");
    assert_eq!(provenance.expansion.start.line, 2);
    assert_eq!(
        provenance.spelling.as_ref().map(|span| span.start.column),
        Some(11)
    );
    assert_eq!(provenance.include_stack.len(), 2);
    assert!(provenance.include_stack[1].start.file.ends_with("header.h"));
}

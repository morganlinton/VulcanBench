//! VYRECOB2 container format contract tests.
//!
//! These tests exercise the serialization/deserialization round-trip of the
//! VYRECOB2 multi-section container that vyre-frontend-c uses to ship compiled artifacts.

use vyre_frontend_c::object_format::{
    push_section, serialize_vyrecob2, SectionTag, VYRECOB2_MAGIC, VYRECOB2_VERSION,
};

// ── Magic and version ────────────────────────────────────────────────

#[test]
fn vyrecob2_magic_is_8_bytes_null_terminated() {
    assert_eq!(VYRECOB2_MAGIC.len(), 8);
    assert_eq!(
        VYRECOB2_MAGIC[7], 0,
        "Fix: VYRECOB2 magic must be null-terminated."
    );
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn vyrecob2_version_is_nonzero() {
    assert!(VYRECOB2_VERSION > 0, "Fix: VYRECOB2 version must be > 0.");
}

// ── push_section ─────────────────────────────────────────────────────

#[test]
fn push_section_encodes_tag_length_payload() {
    let mut buf = Vec::new();
    let payload = b"hello";
    push_section(&mut buf, SectionTag::Lex, payload)
        .expect("Fix: small section payload must serialize.");

    // tag: u32 LE
    let tag = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    assert_eq!(
        tag,
        SectionTag::Lex as u32,
        "Fix: section tag must be Lex (1)."
    );

    // length: u32 LE
    let len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    assert_eq!(len, 5, "Fix: section length must match payload length.");

    // payload
    assert_eq!(
        &buf[8..],
        payload,
        "Fix: section payload must follow tag+length."
    );
}

#[test]
fn push_section_empty_payload() {
    let mut buf = Vec::new();
    push_section(&mut buf, SectionTag::Calls, &[])
        .expect("Fix: empty section payload must serialize.");

    let len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    assert_eq!(len, 0, "Fix: empty payload must encode as length 0.");
    assert_eq!(buf.len(), 8, "Fix: header-only section must be 8 bytes.");
}

// ── serialize_vyrecob2 ──────────────────────────────────────────────

#[test]
fn serialize_vyrecob2_empty_sections() {
    let blob = serialize_vyrecob2(&[]).expect("Fix: empty VYRECOB2 must serialize.");
    // magic (8) + version (4) + section_count (4) = 16
    assert_eq!(
        blob.len(),
        16,
        "Fix: empty VYRECOB2 must be exactly 16 bytes."
    );
    assert_eq!(
        &blob[0..8],
        VYRECOB2_MAGIC,
        "Fix: blob must start with VYRECOB2 magic."
    );
    let version = u32::from_le_bytes([blob[8], blob[9], blob[10], blob[11]]);
    assert_eq!(version, VYRECOB2_VERSION);
    let count = u32::from_le_bytes([blob[12], blob[13], blob[14], blob[15]]);
    assert_eq!(
        count, 0,
        "Fix: section count must be 0 for empty container."
    );
}

#[test]
fn serialize_vyrecob2_multiple_sections_round_trip() {
    let sections: Vec<(SectionTag, &[u8])> = vec![
        (SectionTag::Lex, b"lex_data"),
        (SectionTag::ParenPairs, b"paren_data"),
        (SectionTag::Functions, &[0xDE, 0xAD, 0xBE, 0xEF]),
    ];
    let blob = serialize_vyrecob2(&sections).expect("Fix: multi-section VYRECOB2 must serialize.");

    // Verify header
    assert_eq!(&blob[0..8], VYRECOB2_MAGIC);
    let count = u32::from_le_bytes([blob[12], blob[13], blob[14], blob[15]]);
    assert_eq!(count, 3, "Fix: section count must be 3.");

    // Walk sections and verify tags + payloads
    let mut offset = 16;
    for (expected_tag, expected_payload) in &sections {
        let tag = u32::from_le_bytes([
            blob[offset],
            blob[offset + 1],
            blob[offset + 2],
            blob[offset + 3],
        ]);
        assert_eq!(tag, *expected_tag as u32);
        let len = u32::from_le_bytes([
            blob[offset + 4],
            blob[offset + 5],
            blob[offset + 6],
            blob[offset + 7],
        ]) as usize;
        assert_eq!(len, expected_payload.len());
        assert_eq!(&blob[offset + 8..offset + 8 + len], *expected_payload);
        offset += 8 + len;
    }
    assert_eq!(
        offset,
        blob.len(),
        "Fix: consumed bytes must equal blob length."
    );
}

// ── SectionTag coverage ──────────────────────────────────────────────

#[test]
fn section_tags_have_distinct_discriminants() {
    let tags = [
        SectionTag::Lex,
        SectionTag::ParenPairs,
        SectionTag::BracePairs,
        SectionTag::Functions,
        SectionTag::Calls,
        SectionTag::Elf,
        SectionTag::PreprocMask,
        SectionTag::MacroTypes,
        SectionTag::AbiLayout,
        SectionTag::AbiTypes,
        SectionTag::Ast,
        SectionTag::Cfg,
        SectionTag::Megakernel,
        SectionTag::Vast,
        SectionTag::ProgramGraph,
        SectionTag::SemaScope,
        SectionTag::ExpressionShape,
        SectionTag::SemanticProgramGraphNodes,
        SectionTag::SemanticProgramGraphEdges,
    ];
    let mut seen = std::collections::HashSet::new();
    for tag in tags {
        let val = tag as u32;
        assert!(
            seen.insert(val),
            "Fix: duplicate SectionTag discriminant {val} for {tag:?}."
        );
    }
}

// ── build_vyrecob1_lex_section ───────────────────────────────────────

#[test]
fn build_vyrecob1_lex_section_smoke() {
    use std::path::Path;
    use vyre_frontend_c::object_format::build_vyrecob1_lex_section;

    // 2 tokens, each stream is 2 × u32 = 8 bytes
    let types = [1u32.to_le_bytes(), 2u32.to_le_bytes()].concat();
    let starts = [0u32.to_le_bytes(), 5u32.to_le_bytes()].concat();
    let lens = [3u32.to_le_bytes(), 4u32.to_le_bytes()].concat();

    let blob = build_vyrecob1_lex_section(Path::new("test.c"), &types, &starts, &lens, 2)
        .expect("Fix: build_vyrecob1_lex_section must succeed for valid inputs.");

    assert!(
        blob.starts_with(b"VYRECOB1"),
        "Fix: VYRECOB1 lex section must start with VYRECOB1 magic."
    );
    assert!(
        blob.len() > 8,
        "Fix: VYRECOB1 blob must contain more than just the magic."
    );
}

#[test]
fn build_vyrecob1_lex_section_rejects_short_stream() {
    use std::path::Path;
    use vyre_frontend_c::object_format::build_vyrecob1_lex_section;

    // Claim 10 tokens but only provide 2 u32s (8 bytes) per stream
    let short = vec![0u8; 8];
    let result = build_vyrecob1_lex_section(Path::new("test.c"), &short, &short, &short, 10);
    assert!(
        matches!(result, Err(_)),
        "Fix: build_vyrecob1_lex_section must reject streams shorter than n_tokens."
    );
}

// ── write_vyrecob2 disk round-trip ───────────────────────────────────

#[test]
fn write_vyrecob2_creates_readable_file() {
    use vyre_frontend_c::object_format::write_vyrecob2;

    let mut dir = std::env::temp_dir();
    dir.push(format!("vyre-frontend-c-objfmt-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.vyrecob2");
    let sections: Vec<(SectionTag, &[u8])> = vec![(SectionTag::Elf, b"ELF_STUB")];

    write_vyrecob2(&path, &sections).expect("Fix: write_vyrecob2 must succeed.");

    let blob = std::fs::read(&path).expect("Fix: written VYRECOB2 must be readable.");
    assert_eq!(&blob[0..8], VYRECOB2_MAGIC);
    let count = u32::from_le_bytes([blob[12], blob[13], blob[14], blob[15]]);
    assert_eq!(count, 1);

    let _ = std::fs::remove_dir_all(&dir);
}

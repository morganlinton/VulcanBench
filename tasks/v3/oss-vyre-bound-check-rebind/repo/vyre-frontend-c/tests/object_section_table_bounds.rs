//! Compiled object section table bounds: every tag/length pair and payload stays inside the
//! VYRECOB2 payload, and the table exactly consumes the payload.

mod support;

use std::sync::OnceLock;

use support::{compile_source, read_u32, MAGIC};

static SECTION_TABLE_OBJECT: OnceLock<Vec<u8>> = OnceLock::new();

fn compiled_object_bytes() -> &'static [u8] {
    SECTION_TABLE_OBJECT
        .get_or_init(|| {
            compile_source(
                "section_table_bounds_shared",
                "int section_table_bounds(void) { return 0; }\n",
                Vec::new(),
            )
            .into_inner()
        })
        .as_slice()
}

fn compiled_payload() -> &'static [u8] {
    let bytes = compiled_object_bytes();
    assert_eq!(&bytes[0..4], b"\x7fELF");
    let payload_offset = bytes
        .windows(MAGIC.len())
        .position(|window| window == MAGIC)
        .expect("compiled object embeds a VYRECOB2 payload");
    &bytes[payload_offset..]
}

#[test]
fn compiled_object_section_table_respects_payload_bounds() {
    let payload = compiled_payload();
    let payload_len = payload.len();
    assert!(
        payload_len >= MAGIC.len() + 4 + 4,
        "payload fits header: magic({}) + version(4) + count(4)",
        MAGIC.len()
    );

    let mut offset = MAGIC.len();
    let _version = read_u32(payload, &mut offset);
    let section_count = read_u32(payload, &mut offset) as usize;

    let mut accumulated_meta = 0usize;
    let mut accumulated_payload = 0usize;

    for i in 0..section_count {
        assert!(
            offset + 8 <= payload_len,
            "section {i} descriptor (tag+len) fits inside payload"
        );
        let tag = read_u32(payload, &mut offset);
        let len = read_u32(payload, &mut offset) as usize;
        accumulated_meta += 8;

        let section_end = offset.saturating_add(len);
        assert!(
            section_end <= payload_len,
            "section {i} (tag={tag}, len={len}) payload fits: end={section_end} <= {payload_len}"
        );
        assert!(
            section_end >= offset,
            "section {i} length does not overflow usize"
        );
        offset = section_end;
        accumulated_payload += len;
    }

    assert!(
        offset <= payload_len,
        "section table plus payloads does not exceed embedded payload: {offset} <= {payload_len}"
    );
    assert_eq!(
        MAGIC.len() + 4 + 4 + accumulated_meta + accumulated_payload,
        offset,
        "header + metadata + payloads == parsed VYRECOB2 length"
    );
}

#[test]
fn compiled_object_sections_are_iterable_without_panic() {
    let env = support::ObjectEnvelope::from_elf(compiled_object_bytes().to_vec());
    let tags = env.section_tags();
    assert!(!tags.is_empty(), "compiled object has at least one section");

    for tag in tags {
        let len = env.section_len(tag);
        let data = env.section(tag).expect("section is present");
        assert_eq!(data.len(), len, "section {tag} length is consistent");
    }
}

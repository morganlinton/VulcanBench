//! VYRECOB2 object envelope detection and version parsing.
mod support;

use support::object::MAGIC;
use support::object_envelope::{ObjectEnvelope, ObjectFlavor};

#[test]
fn envelope_from_payload_detects_magic_and_version() {
    let mut payload = Vec::new();
    payload.extend_from_slice(MAGIC);
    payload.extend_from_slice(&6u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());

    let env = ObjectEnvelope::from_payload(payload);
    assert_eq!(env.flavor(), ObjectFlavor::RawVyrecob2);
    env.assert_carrier();
    env.assert_version(6);
    assert_eq!(env.section_count(), 0);
    assert!(env.section_tags().is_empty());
}

#[test]
fn envelope_section_iteration_and_assertions() {
    let mut payload = Vec::new();
    payload.extend_from_slice(MAGIC);
    payload.extend_from_slice(&6u32.to_le_bytes());
    payload.extend_from_slice(&3u32.to_le_bytes());

    // section 7: 4 bytes
    payload.extend_from_slice(&7u32.to_le_bytes());
    payload.extend_from_slice(&4u32.to_le_bytes());
    payload.extend_from_slice(&0xDEADBEEFu32.to_le_bytes());

    // section 8: 8 bytes
    payload.extend_from_slice(&8u32.to_le_bytes());
    payload.extend_from_slice(&8u32.to_le_bytes());
    payload.extend_from_slice(&0xCAFEBABEu32.to_le_bytes());
    payload.extend_from_slice(&0x11223344u32.to_le_bytes());

    // section 9: 0 bytes
    payload.extend_from_slice(&9u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());

    let env = ObjectEnvelope::from_payload(payload);
    assert_eq!(env.section_count(), 3);
    assert_eq!(env.section_tags(), vec![7, 8, 9]);

    env.assert_section_present(7);
    env.assert_section_present(8);
    env.assert_section_present(9);
    env.assert_section_absent(1);

    assert_eq!(env.section_len(7), 4);
    assert_eq!(env.section_len(8), 8);
    assert_eq!(env.section_len(9), 0);
    assert_eq!(env.section_len(99), 0);

    env.assert_section_bytes(7, &[0xEF, 0xBE, 0xAD, 0xDE]); // little-endian
    env.assert_section_words(8, &[0xCAFEBABEu32, 0x11223344]);
}

#[test]
fn envelope_from_elf_finds_embedded_payload() {
    let mut elf = Vec::new();
    elf.extend_from_slice(b"\x7fELF");
    elf.extend_from_slice(&[0; 20]);

    let mut payload = Vec::new();
    payload.extend_from_slice(MAGIC);
    payload.extend_from_slice(&6u32.to_le_bytes());
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&42u32.to_le_bytes());
    payload.extend_from_slice(&4u32.to_le_bytes());
    payload.extend_from_slice(&0x01020304u32.to_le_bytes());
    elf.extend_from_slice(&payload);

    let env = ObjectEnvelope::from_elf(elf);
    assert_eq!(env.flavor(), ObjectFlavor::Elf);
    env.assert_carrier();
    env.assert_version(6);
    env.assert_section_present(42);
    env.assert_section_words(42, &[0x01020304]);
}

#[test]
#[should_panic(expected = "expected raw payload to start with VYRECOB2 magic")]
fn envelope_from_payload_panics_without_magic() {
    ObjectEnvelope::from_payload(vec![0, 1, 2, 3]);
}

#[test]
#[should_panic(expected = "expected ELF carrier magic at the start of the object")]
fn envelope_from_elf_panics_without_magic() {
    ObjectEnvelope::from_elf(vec![0, 1, 2, 3]);
}

#[test]
#[should_panic(expected = "expected ELF carrier to embed a VYRECOB2 payload")]
fn envelope_from_elf_panics_without_payload_magic() {
    ObjectEnvelope::from_elf(vec![0x7f, b'E', b'L', b'F', 0, 0, 0, 0]);
}

#[test]
fn detect_routes_raw_and_elf_carriers() {
    let mut raw = Vec::new();
    raw.extend_from_slice(MAGIC);
    raw.extend_from_slice(&6u32.to_le_bytes());
    raw.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        ObjectEnvelope::detect(raw).flavor(),
        ObjectFlavor::RawVyrecob2
    );

    let mut elf = b"\x7fELF".to_vec();
    elf.extend_from_slice(&[0; 8]);
    elf.extend_from_slice(MAGIC);
    elf.extend_from_slice(&6u32.to_le_bytes());
    elf.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(ObjectEnvelope::detect(elf).flavor(), ObjectFlavor::Elf);
}

#[test]
#[should_panic(expected = "expected VYRECOB2 section 5 to be present")]
fn envelope_assert_section_present_panics_when_missing() {
    let mut payload = Vec::new();
    payload.extend_from_slice(MAGIC);
    payload.extend_from_slice(&6u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    let env = ObjectEnvelope::from_payload(payload);
    env.assert_section_present(5);
}

#[test]
#[should_panic(expected = "expected VYRECOB2 section 5 to be absent")]
fn envelope_assert_section_absent_panics_when_present() {
    let mut payload = Vec::new();
    payload.extend_from_slice(MAGIC);
    payload.extend_from_slice(&6u32.to_le_bytes());
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&5u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    let env = ObjectEnvelope::from_payload(payload);
    env.assert_section_absent(5);
}

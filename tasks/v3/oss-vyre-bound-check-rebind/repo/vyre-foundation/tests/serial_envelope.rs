//! Wire envelope contracts for cacheable vyre payloads.

use vyre_foundation::serial::{EnvelopeError, WireReader, WireWriter};

const TEST_MAGIC: &[u8; 4] = b"TEST";
const TEST_VERSION: u32 = 1;

#[test]
fn round_trip_section_words_u32() {
    let mut writer = WireWriter::new(TEST_MAGIC, TEST_VERSION);
    writer.write_section(b"hello").unwrap();
    writer.write_words(&[1, 2, 3]).unwrap();
    writer.write_u32(0xDEADBEEF);
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION).unwrap();
    assert_eq!(reader.read_section().unwrap(), b"hello");
    assert_eq!(reader.read_words().unwrap(), vec![1, 2, 3]);
    assert_eq!(reader.read_u32().unwrap(), 0xDEADBEEF);
}

#[test]
fn rejects_short_header() {
    let bytes = [0u8; 3];
    match WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION) {
        Err(EnvelopeError::Truncated { needed: 8, got: 3 }) => {}
        other => panic!("expected Truncated, got {other:?}"),
    }
}

#[test]
fn rejects_bad_magic() {
    let mut bytes = WireWriter::new(b"WRNG", TEST_VERSION).into_bytes();
    bytes.extend_from_slice(&[0u8; 4]);
    match WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION) {
        Err(EnvelopeError::BadMagic { .. }) => {}
        other => panic!("expected BadMagic, got {other:?}"),
    }
}

#[test]
fn rejects_version_mismatch() {
    let bytes = WireWriter::new(TEST_MAGIC, TEST_VERSION + 1).into_bytes();
    match WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION) {
        Err(EnvelopeError::VersionMismatch {
            expected: 1,
            found: 2,
        }) => {}
        other => panic!("expected VersionMismatch, got {other:?}"),
    }
}

#[test]
fn rejects_truncated_section() {
    let mut writer = WireWriter::new(TEST_MAGIC, TEST_VERSION);
    writer.write_section(b"hello").unwrap();
    let bytes = writer.into_bytes();
    let cut = &bytes[..bytes.len() - 2];
    let mut reader = WireReader::new(cut, TEST_MAGIC, TEST_VERSION).unwrap();
    match reader.read_section() {
        Err(EnvelopeError::Truncated { .. }) => {}
        other => panic!("expected Truncated, got {other:?}"),
    }
}

#[test]
fn rejects_truncated_words() {
    let mut writer = WireWriter::new(TEST_MAGIC, TEST_VERSION);
    writer.write_words(&[1, 2, 3]).unwrap();
    let bytes = writer.into_bytes();
    let cut = &bytes[..bytes.len() - 4];
    let mut reader = WireReader::new(cut, TEST_MAGIC, TEST_VERSION).unwrap();
    match reader.read_words() {
        Err(EnvelopeError::Truncated { .. }) => {}
        other => panic!("expected Truncated, got {other:?}"),
    }
}

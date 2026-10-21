//! Adversarial oracle tests for `text::char_class` reference mapping.

use vyre_foundation::ir::DataType;
use vyre_primitives::text::char_class::{char_class, char_class_u8, reference_char_class};
use vyre_reference::value::Value;

fn pack_u32s(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for &word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn unpack_u32s(bytes: &[u8]) -> Vec<u32> {
    let mut words = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        words.push(u32::from_le_bytes(
            chunk.try_into().expect("Fix: u32 chunk conversion failed"),
        ));
    }
    words
}

fn run_program(source: &[u8], table: &[u32; 256]) -> Vec<u32> {
    let n = source.len();
    let program = char_class("source", "classified", n as u32);
    let cap = n.max(1);
    let mut input_bytes = Vec::with_capacity(cap * 4);
    for &b in source {
        input_bytes.extend_from_slice(&(b as u32).to_le_bytes());
    }
    while input_bytes.len() < cap * 4 {
        input_bytes.extend_from_slice(&0u32.to_le_bytes());
    }
    let table_bytes = pack_u32s(table);
    let zero_classified = vec![0u8; cap * 4];
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(input_bytes),
            Value::from(table_bytes),
            Value::from(zero_classified),
        ],
    )
    .expect("Fix: char_class reference evaluation must succeed");
    let out_bytes = outputs[0].to_bytes();
    let mut out_u32s = unpack_u32s(&out_bytes);
    out_u32s.truncate(n);
    out_u32s
}

fn run_packed_u8_program(source: &[u8], table: &[u32; 256]) -> Vec<u32> {
    let program = char_class_u8("source", "classified", source.len() as u32);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(source.to_vec()), Value::from(pack_u32s(table))],
    )
    .expect("Fix: packed-u8 char_class reference evaluation must succeed");
    let mut out_u32s = unpack_u32s(&outputs[0].to_bytes());
    out_u32s.truncate(source.len());
    out_u32s
}

#[test]
fn char_class_hostile_corpus_table_driven() {
    const ZEROS: [u32; 256] = [0u32; 256];
    const MAXES: [u32; 256] = [0xffff_ffffu32; 256];
    let cases: &[(&[u8], &[u32; 256], &[u32])] = &[
        (b"", &ZEROS, &[]),
        (b"\x00", &ZEROS, &[0]),
        (b"\xff", &MAXES, &[0xffff_ffff]),
        (b"vyre", &ZEROS, &[0, 0, 0, 0]),
        (
            b"\x00\xff\xfe",
            &MAXES,
            &[0xffff_ffff, 0xffff_ffff, 0xffff_ffff],
        ),
    ];
    for (idx, (source, table, expected)) in cases.iter().enumerate() {
        let got = reference_char_class(source, table);
        assert_eq!(
            got,
            *expected,
            "Fix: char_class oracle mismatch on hostile case {idx} (len={})",
            source.len()
        );
        assert_eq!(
            run_program(source, table),
            *expected,
            "Fix: char_class compiled program mismatch on hostile case {idx} (len={})",
            source.len()
        );
        assert_eq!(
            run_packed_u8_program(source, table),
            *expected,
            "Fix: packed-u8 char_class compiled program mismatch on hostile case {idx} (len={})",
            source.len()
        );
    }
}

#[test]
fn packed_u8_char_class_uses_one_source_byte_per_element() {
    let program = char_class_u8("source", "classified", 1024);
    let source = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "source")
        .expect("Fix: packed-u8 char_class source buffer must be declared");
    let table = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "table")
        .expect("Fix: char_class table buffer must be declared");
    let classified = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "classified")
        .expect("Fix: char_class output buffer must be declared");

    assert_eq!(source.element(), DataType::U8);
    assert_eq!(source.count(), 1024);
    assert_eq!(
        source.count() as usize * DataType::U8.min_bytes(),
        1024,
        "Fix: packed-u8 char_class must consume one byte per source byte."
    );
    assert_eq!(
        source.count() as usize * DataType::U32.min_bytes(),
        4096,
        "Fix: compatibility char_class remains the four-byte-per-source-byte path."
    );
    assert_eq!(table.element(), DataType::U32);
    assert_eq!(table.count(), 256);
    assert_eq!(classified.element(), DataType::U32);
    assert_eq!(classified.count(), 1024);
}

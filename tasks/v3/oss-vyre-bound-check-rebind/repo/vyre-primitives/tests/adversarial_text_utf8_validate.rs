//! Adversarial oracle tests for `text::utf8_validate` classification.

use vyre_foundation::ir::{DataType, Program};
use vyre_primitives::text::utf8_validate::{
    reference_utf8_validate, utf8_validate, utf8_validate_u8, UTF8_ASCII, UTF8_CONT, UTF8_INVALID,
    UTF8_LEAD_2, UTF8_LEAD_4,
};
use vyre_reference::value::Value;

fn output_u32s(program: &Program, outputs: &[Value], n: usize) -> Vec<u32> {
    let out_bytes = outputs[0].to_bytes();
    let mut out_u32s = Vec::with_capacity(n.max(1));
    for chunk in out_bytes.chunks_exact(4) {
        out_u32s.push(u32::from_le_bytes(
            chunk.try_into().expect("Fix: u32 chunk conversion failed"),
        ));
    }
    out_u32s.truncate(n);
    assert_eq!(
        program.buffers()[1].element(),
        DataType::U32,
        "Fix: UTF-8 class output must remain U32 for downstream token stages."
    );
    out_u32s
}

fn run_program(source: &[u8]) -> Vec<u32> {
    let n = source.len();
    let program = utf8_validate("source", "classes", n as u32);
    let cap = n.max(1);
    let mut input_bytes = Vec::with_capacity(cap * 4);
    for &b in source {
        input_bytes.extend_from_slice(&(b as u32).to_le_bytes());
    }
    while input_bytes.len() < cap * 4 {
        input_bytes.extend_from_slice(&0u32.to_le_bytes());
    }
    let zero_classes = vec![0u8; cap * 4];
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(input_bytes), Value::from(zero_classes)],
    )
    .expect("Fix: utf8_validate reference evaluation must succeed");
    output_u32s(&program, &outputs, n)
}

fn run_packed_u8_program(source: &[u8]) -> Vec<u32> {
    let n = source.len();
    let program = utf8_validate_u8("source", "classes", n as u32);
    let source_buffer = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "source")
        .expect("Fix: packed-u8 UTF-8 source buffer must be declared");
    assert_eq!(source_buffer.element(), DataType::U8);
    assert_eq!(source_buffer.count(), n as u32);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(source.to_vec())])
        .expect("Fix: packed-u8 utf8_validate reference evaluation must succeed");
    output_u32s(&program, &outputs, n)
}

#[test]
fn utf8_validate_hostile_corpus() {
    let cases: &[(&[u8], &[u32])] = &[
        (b"", &[]),
        (b"a", &[UTF8_ASCII]),
        (b"\xff", &[UTF8_INVALID]),
        (b"\xc2\x80", &[UTF8_LEAD_2, UTF8_CONT]),
        (
            b"\xf0\x90\x80\x80",
            &[UTF8_LEAD_4, UTF8_CONT, UTF8_CONT, UTF8_CONT],
        ),
    ];
    for (idx, (source, expected)) in cases.iter().enumerate() {
        assert_eq!(
            reference_utf8_validate(source),
            *expected,
            "Fix: utf8_validate oracle mismatch on case {idx}"
        );
        assert_eq!(
            run_program(source),
            *expected,
            "Fix: utf8_validate compiled program mismatch on case {idx}"
        );
        assert_eq!(
            run_packed_u8_program(source),
            *expected,
            "Fix: packed-u8 utf8_validate compiled program mismatch on case {idx}"
        );
    }
}

#[test]
fn utf8_validate_u8_uses_packed_source_storage() {
    let source = b"a\xc3\xa9\xf0\x90\x80\x80";
    let program = utf8_validate_u8("source", "classes", source.len() as u32);
    let source_buffer = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "source")
        .expect("Fix: packed-u8 UTF-8 source buffer must be declared");

    assert_eq!(source_buffer.element(), DataType::U8);
    assert_eq!(source_buffer.count(), source.len() as u32);
    assert_eq!(source.len() * DataType::U8.min_bytes(), 7);
    assert_eq!(source.len() * DataType::U32.min_bytes(), 28);
    assert_eq!(
        run_packed_u8_program(source),
        reference_utf8_validate(source)
    );
}

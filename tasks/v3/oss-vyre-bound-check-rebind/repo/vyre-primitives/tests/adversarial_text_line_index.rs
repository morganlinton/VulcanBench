//! Adversarial oracle tests for `text::line_index`.

use vyre_foundation::ir::{BufferAccess, DataType, Program};
use vyre_primitives::text::line_index::{line_index, line_index_u8, reference_line_index};
use vyre_reference::value::Value;

fn output_index(program: &Program, name: &str) -> usize {
    program
        .buffers()
        .iter()
        .filter(|buffer| {
            buffer.is_output()
                || buffer.is_pipeline_live_out()
                || matches!(
                    buffer.access(),
                    BufferAccess::ReadWrite | BufferAccess::WriteOnly
                )
        })
        .position(|buffer| buffer.name() == name)
        .expect("Fix: line_index final output buffer must be declared")
}

fn output_names(program: &Program) -> Vec<&str> {
    program
        .buffers()
        .iter()
        .filter(|buffer| {
            buffer.is_output()
                || buffer.is_pipeline_live_out()
                || matches!(
                    buffer.access(),
                    BufferAccess::ReadWrite | BufferAccess::WriteOnly
                )
        })
        .map(|buffer| buffer.name())
        .collect()
}

fn unpack_u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            u32::from_le_bytes(chunk.try_into().expect("Fix: u32 chunk conversion failed"))
        })
        .collect()
}

fn run_program(source: &[u8]) -> Vec<u32> {
    let n = source.len();
    let program = line_index("source", "lines", n as u32);
    let cap = n.max(1);
    let mut input_bytes = Vec::with_capacity(cap * 4);
    for &b in source {
        input_bytes.extend_from_slice(&(b as u32).to_le_bytes());
    }
    while input_bytes.len() < cap * 4 {
        input_bytes.extend_from_slice(&0u32.to_le_bytes());
    }
    let lines_index = output_index(&program, "lines");
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(input_bytes)])
        .expect("Fix: line_index reference evaluation must succeed");
    let out_bytes = outputs[lines_index].to_bytes();
    let mut out_u32s = Vec::with_capacity(cap);
    out_u32s.extend(unpack_u32s(&out_bytes));
    out_u32s.truncate(n);
    out_u32s
}

fn run_packed_u8_program(source: &[u8]) -> Vec<u32> {
    let n = source.len();
    let program = line_index_u8("source", "lines", n as u32);
    let lines_index = output_index(&program, "lines");
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(source.to_vec())])
        .expect("Fix: packed-u8 line_index reference evaluation must succeed");
    let out_bytes = outputs[lines_index].to_bytes();
    let mut out_u32s = Vec::with_capacity(n);
    out_u32s.extend(unpack_u32s(&out_bytes));
    out_u32s.truncate(n);
    out_u32s
}

#[test]
fn line_index_hostile_corpus() {
    let cases: &[(&[u8], &[u32])] = &[
        (b"", &[]),
        (b"a", &[0]),
        (b"a\nb", &[0, 0, 1]),
        (b"a\r\nb", &[0, 0, 0, 1]),
        (b"\r\n\r\n", &[0, 0, 1, 1]),
    ];
    for (idx, (source, expected)) in cases.iter().enumerate() {
        assert_eq!(
            reference_line_index(source),
            *expected,
            "Fix: line_index oracle mismatch on case {idx}"
        );
        assert_eq!(
            run_program(source),
            *expected,
            "Fix: line_index compiled program mismatch on case {idx}"
        );
        assert_eq!(
            run_packed_u8_program(source),
            *expected,
            "Fix: packed-u8 line_index compiled program mismatch on case {idx}"
        );
    }
}

#[test]
fn line_index_pipeline_intermediates_match_registered_fixture_shape() {
    let source = b"ab\ncd";
    let program = line_index("source", "lines", source.len() as u32);
    assert_eq!(
        output_names(&program),
        vec!["__lines_line_start_flags", "lines"]
    );

    let mut input_bytes = Vec::with_capacity(source.len() * 4);
    for &byte in source {
        input_bytes.extend_from_slice(&(byte as u32).to_le_bytes());
    }
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(input_bytes)])
        .expect("Fix: line_index pipeline fixture reference evaluation must succeed");

    assert_eq!(unpack_u32s(&outputs[0].to_bytes()), vec![0, 0, 0, 1, 0]);
    assert_eq!(unpack_u32s(&outputs[1].to_bytes()), vec![0, 0, 0, 1, 1]);
}

#[test]
fn line_index_u8_uses_packed_source_storage() {
    let source = b"ab\r\ncd\n";
    let program = line_index_u8("source", "lines", source.len() as u32);
    let source_buffer = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "source")
        .expect("Fix: packed-u8 line_index source buffer must be declared");

    assert_eq!(source_buffer.element(), DataType::U8);
    assert_eq!(source_buffer.count(), source.len() as u32);
    assert_eq!(source.len(), 7);
    assert_eq!(
        source.len() * DataType::U8.min_bytes(),
        7,
        "Fix: packed-u8 line_index must consume one byte per source byte."
    );
    assert_eq!(
        source.len() * DataType::U32.min_bytes(),
        28,
        "Fix: compatibility line_index remains the four-byte-per-source-byte path."
    );
    assert_eq!(run_packed_u8_program(source), reference_line_index(source));
}

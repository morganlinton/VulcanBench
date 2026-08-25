//! `line_splice_classify` reference roundtrip  -  validates the packed-word
//! compatibility program and the raw-byte program against real C-shaped
//! inputs. Catches IR-level bugs (wrong opcode, bad operand wiring, missing
//! buffer binding) without needing the GPU driver.

use vyre_foundation::ir::DataType;
use vyre_primitives::parsing::line_splice_classify::{
    line_splice_classify, line_splice_classify_u8,
};
use vyre_reference::value::Value;

fn reference_line_splice_classify(source: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(source.len());
    for i in 0..source.len() {
        let b_m2 = i.checked_sub(2).map(|j| source[j]).unwrap_or(0);
        let b_m1 = i.checked_sub(1).map(|j| source[j]).unwrap_or(0);
        let b_0 = source[i];
        let b_p1 = source.get(i + 1).copied().unwrap_or(0);
        let case1 = b_0 == b'\\' && b_p1 == b'\n';
        let case2 = b_0 == b'\\' && b_p1 == b'\r';
        let case3 = b_m1 == b'\\' && b_0 == b'\n';
        let case4 = b_m1 == b'\\' && b_0 == b'\r';
        let case5 = b_m2 == b'\\' && b_m1 == b'\r' && b_0 == b'\n';
        out.push(u32::from(!(case1 || case2 || case3 || case4 || case5)));
    }
    out
}

fn unpack_mask(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 chunk")))
        .collect()
}

fn run_program(source: &[u8]) -> Vec<u32> {
    let n = source.len();
    let program = line_splice_classify(n as u32);
    // Pad host-side buffers to match the declared capacities exactly; the IR
    // doesn't tolerate undersized bindings.
    let cap = n.max(1);
    // bytes_in is declared as packed U32; pad input bytes to a multiple of 4
    // so the last word is fully covered.
    let pad = (cap.div_ceil(4) * 4).max(4);
    let mut input = source.to_vec();
    input.resize(pad, 0);
    let zero_mask = vec![0u8; cap * 4];
    let outputs =
        vyre_reference::reference_eval(&program, &[Value::from(input), Value::from(zero_mask)])
            .expect("line_splice_classify reference evaluation must succeed");
    let mut mask = unpack_mask(&outputs[0].to_bytes());
    mask.truncate(n); // trim the byte_count.max(1) padding
    mask
}

fn run_program_u8(source: &[u8]) -> Vec<u32> {
    let n = source.len();
    let program = line_splice_classify_u8(n as u32);
    let zero_mask = vec![0u8; n.max(1) * 4];
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(source.to_vec()), Value::from(zero_mask)],
    )
    .expect("raw-u8 line_splice_classify reference evaluation must succeed");
    let mut mask = unpack_mask(&outputs[0].to_bytes());
    mask.truncate(n);
    mask
}

fn assert_programs_match_cpu(source: &[u8]) {
    let cpu = reference_line_splice_classify(source);
    assert_eq!(run_program(source), cpu);
    assert_eq!(run_program_u8(source), cpu);
}

#[test]
fn ir_program_matches_cpu_reference_on_no_splice_input() {
    let src = b"int main(void) { return 0; }";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_backslash_lf_pair() {
    let src = b"a\\\nb";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_backslash_cr_lf_triple() {
    let src = b"a\\\r\nb";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_backslash_cr_alone() {
    let src = b"a\\\rb";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_back_to_back_splices() {
    let src = b"a\\\nb\\\nc";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_double_backslash_then_lf() {
    // Adversarial: only the SECOND '\\' splices with the '\n'; the
    // first '\\' must survive.
    let src = b"a\\\\\nb";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_splice_at_buffer_start() {
    let src = b"\\\nx";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_splice_at_buffer_end() {
    let src = b"x\\\n";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_lone_backslash_at_eof() {
    let src = b"x\\";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_real_macro_continuation() {
    // The shape that justifies phase-2 line splicing in the first place.
    let src = b"#define MAX(a, b) \\\n    ((a) > (b) ? (a) : (b))\n";
    assert_programs_match_cpu(src);
}

#[test]
fn ir_program_matches_cpu_reference_on_long_no_splice_input() {
    // Stress: 1024 bytes of plain C tokens, no splice.
    let unit = b"int x = 1; ";
    let mut src = Vec::with_capacity(1024);
    while src.len() + unit.len() <= 1024 {
        src.extend_from_slice(unit);
    }
    assert_programs_match_cpu(&src);
}

#[test]
fn ir_program_matches_cpu_reference_on_dense_splice_pattern() {
    // Stress: every other line is a backslash-newline continuation.
    let mut src = Vec::with_capacity(512);
    for i in 0..32 {
        src.extend_from_slice(format!("line{i:03}\\\n  cont\n").as_bytes());
    }
    assert_programs_match_cpu(&src);
}

#[test]
fn u8_program_declares_runtime_sized_source_buffer() {
    let program = line_splice_classify_u8(64);
    let bytes = &program.buffers()[0];
    let mask = &program.buffers()[1];

    assert_eq!(bytes.name(), "bytes_in");
    assert_eq!(bytes.element(), DataType::U8);
    assert_eq!(bytes.count(), 0);
    assert_eq!(mask.name(), "kept_mask_out");
    assert_eq!(mask.element(), DataType::U32);
    assert_eq!(mask.count(), 64);
}

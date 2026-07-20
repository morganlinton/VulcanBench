//! GPU comment-strip mask reference roundtrip.
//!
//! Pins the GPU `Program` against the reference oracle for every comment
//! shape: bare line comment, bare block comment, intermixed code,
//! unterminated block comment, lone `/`, multi-line block.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::preprocess::gpu_comment_strip_mask::{
    gpu_comment_strip_mask, gpu_comment_strip_mask_u8, reference_gpu_comment_strip_mask,
};
use vyre_reference::value::Value;

fn unpack(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn run(source: &[u8]) -> Vec<u32> {
    let n = source.len();
    let cap = n.max(1);
    // bytes_in is declared as packed U32; pad input bytes to a
    // multiple of 4.
    let pad = (cap.div_ceil(4) * 4).max(4);
    let mut input = source.to_vec();
    input.resize(pad, 0);
    let out_init = vec![0u8; cap * 4];
    let prog = gpu_comment_strip_mask(n as u32);
    let outputs =
        vyre_reference::reference_eval(&prog, &[Value::from(input), Value::from(out_init)])
            .expect("gpu_comment_strip_mask reference eval");
    let mut mask = unpack(&outputs[0].to_bytes());
    mask.truncate(n);
    mask
}

fn run_u8(source: &[u8]) -> Vec<u32> {
    let n = source.len();
    let out_init = vec![0u8; n.max(1) * 4];
    let prog = gpu_comment_strip_mask_u8(n as u32);
    let outputs = vyre_reference::reference_eval(
        &prog,
        &[Value::from(source.to_vec()), Value::from(out_init)],
    )
    .expect("raw-u8 gpu_comment_strip_mask reference eval");
    let mut mask = unpack(&outputs[0].to_bytes());
    mask.truncate(n);
    mask
}

fn assert_both_layouts_match_reference(source: &[u8]) {
    let expected = reference_gpu_comment_strip_mask(source);
    assert_eq!(run(source), expected);
    assert_eq!(run_u8(source), expected);
}

#[test]
fn ir_matches_reference_no_comment() {
    let src = b"int x = 1;";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_line_comment() {
    let src = b"//foo\nx";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_block_comment_inline() {
    let src = b"/*x*/";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_block_comment_with_code_around() {
    let src = b"a/*c*/b";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_unterminated_block() {
    let src = b"a/*xyz";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_lone_slash() {
    let src = b"a/b";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_block_spanning_lines() {
    let src = b"a\n/* multi\nline\ncomment */\nb";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_two_line_comments() {
    let src = b"//a\n//b\nc";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_block_then_line_comment() {
    let src = b"/* foo */ //bar\nbaz";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_matches_reference_realistic_c_snippet() {
    let src = b"// header guard\n#ifndef X\n#define X /* opaque */\nint main(void) { return 0; }\n#endif\n";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_preserves_comment_markers_inside_string_literal() {
    let src = br#"char *s = "/* not comment */";"#;
    assert_eq!(reference_gpu_comment_strip_mask(src), vec![0; src.len()]);
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_closes_string_before_following_block_comment() {
    let src = br#"char *s = "/* not comment */"; int x = /* strip */ 1;"#;
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_closes_char_before_following_line_comment() {
    let src = b"char slash = '/'; // strip\nint x = 1;\n";
    assert_both_layouts_match_reference(src);
}

#[test]
fn ir_preserves_comment_markers_inside_char_literal() {
    let src = b"int c = '/'; int d = '*'; char slashslash[] = \"//\";";
    assert_eq!(reference_gpu_comment_strip_mask(src), vec![0; src.len()]);
    assert_both_layouts_match_reference(src);
}

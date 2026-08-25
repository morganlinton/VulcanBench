use super::*;
use vyre::ir::DataType;

#[test]
fn op_id_is_canonical_and_stable() {
    assert_eq!(
        OP_ID,
        "vyre-libs::parsing::c::preprocess::gpu_comment_strip_mask"
    );
}

#[test]
fn binding_indices_are_canonical_and_stable() {
    assert_eq!(BINDING_BYTES_IN, 0);
    assert_eq!(BINDING_COMMENT_MASK_OUT, 1);
}

#[test]
fn build_program_returns_well_formed_program() {
    let p = gpu_comment_strip_mask(64);
    assert_eq!(p.buffers().len(), 2);
    assert_eq!(p.workgroup_size(), [1, 1, 1]);
}

#[test]
fn u8_program_declares_runtime_sized_source_buffer() {
    let p = gpu_comment_strip_mask_u8(64);
    assert_eq!(p.buffers().len(), 2);
    assert_eq!(p.buffers()[0].name(), "bytes_in");
    assert_eq!(p.buffers()[0].element(), DataType::U8);
    assert_eq!(p.buffers()[0].count(), 0);
    assert_eq!(p.buffers()[1].name(), "comment_mask_out");
    assert_eq!(p.buffers()[1].element(), DataType::U32);
    assert_eq!(p.buffers()[1].count(), 64);
}

#[test]
fn cpu_no_comment_returns_all_zero() {
    assert_eq!(reference_gpu_comment_strip_mask(b"int x = 1;"), vec![0; 10]);
}

#[test]
fn cpu_line_comment_to_eol() {
    // First slash becomes replacement space, rest of comment drops,
    // newline and following code stay.
    assert_eq!(
        reference_gpu_comment_strip_mask(b"//foo\nx"),
        vec![2, 1, 1, 1, 1, 0, 0]
    );
}

#[test]
fn cpu_block_comment_inline() {
    assert_eq!(
        reference_gpu_comment_strip_mask(b"/*x*/"),
        vec![2, 1, 1, 1, 1]
    );
}

#[test]
fn cpu_block_comment_with_code_around() {
    let src = b"a/*c*/b";
    // a=code, /*c*/=comment, b=code.
    assert_eq!(
        reference_gpu_comment_strip_mask(src),
        vec![0, 2, 1, 1, 1, 1, 0]
    );
}

#[test]
fn cpu_unterminated_block_comment_runs_to_eof() {
    let src = b"a/*xyz";
    // a=code, /*xyz=all comment.
    assert_eq!(
        reference_gpu_comment_strip_mask(src),
        vec![0, 2, 1, 1, 1, 1]
    );
}

#[test]
fn cpu_lone_slash_is_code() {
    let src = b"a/b";
    assert_eq!(reference_gpu_comment_strip_mask(src), vec![0, 0, 0]);
}

#[test]
fn cpu_block_inside_string_is_not_comment() {
    let src = b"\"/* */\"";
    assert_eq!(reference_gpu_comment_strip_mask(src), vec![0; src.len()]);
}

#[test]
fn cpu_line_comment_inside_char_is_not_comment() {
    let src = b"'//' x";
    assert_eq!(reference_gpu_comment_strip_mask(src), vec![0; src.len()]);
}

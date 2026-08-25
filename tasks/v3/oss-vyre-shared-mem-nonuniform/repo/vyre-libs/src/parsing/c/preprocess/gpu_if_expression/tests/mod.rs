#[cfg(test)]
use super::*;
use vyre::ir::DataType;
use vyre_reference::value::Value;

use crate::scan::dispatch_io::pack_u32_slice as pack_u32_words;

fn pack_source_bytes(source: &[u8]) -> Vec<u8> {
    let mut packed = Vec::with_capacity(source.len().div_ceil(4).max(1) * 4);
    for chunk in source.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        packed.extend_from_slice(&word);
    }
    if packed.is_empty() {
        packed.extend_from_slice(&0u32.to_le_bytes());
    }
    packed
}

fn pack_empty_macro_values_with_builtin_hashes() -> Vec<u8> {
    pack_u32_words(&crate::parsing::c::parse::gnu_builtins::gpu_builtin_hash_table_words())
}

fn eval_one_if(source: &[u8]) -> u32 {
    let program = gpu_if_expression(1, source.len() as u32);
    let inputs = vec![
        Value::Bytes(pack_u32_words(&[0]).into()),
        Value::Bytes(pack_u32_words(&[source.len() as u32]).into()),
        Value::Bytes(pack_u32_words(&[crate::parsing::c::lex::tokens::TOK_PP_IF]).into()),
        Value::Bytes(pack_source_bytes(source).into()),
        Value::Bytes(Vec::<u8>::new().into()),
        Value::Bytes(Vec::<u8>::new().into()),
        Value::Bytes(pack_empty_macro_values_with_builtin_hashes().into()),
        Value::Bytes(pack_u32_words(&[0]).into()),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs)
        .expect("Fix: GPU #if expression reference evaluation must run.");
    let raw = outputs[0].to_bytes();
    vyre_primitives::wire::read_u32_le_word(&raw, 0, "gpu-if-expression test output")
        .expect("Fix: GPU #if expression output must contain one u32.")
}

#[test]
fn op_id_is_canonical_and_stable() {
    assert_eq!(
        OP_ID,
        "vyre-libs::parsing::c::preprocess::gpu_if_expression"
    );
}

#[test]
fn binding_indices_are_canonical_and_stable() {
    assert_eq!(BINDING_TOK_STARTS, 0);
    assert_eq!(BINDING_TOK_LENS, 1);
    assert_eq!(BINDING_DIRECTIVE_KINDS, 2);
    assert_eq!(BINDING_SOURCE, 3);
    assert_eq!(BINDING_MACRO_NAMES_PACKED, 4);
    assert_eq!(BINDING_MACRO_OFFSETS, 5);
    assert_eq!(BINDING_MACRO_VALUES, 6);
    assert_eq!(BINDING_DIRECTIVE_VALUES, 7);
}

#[test]
fn build_program_returns_well_formed_program() {
    let p = gpu_if_expression(8, 64);
    assert_eq!(p.buffers().len(), 8);
    assert_eq!(p.workgroup_size(), [256, 1, 1]);
}

#[test]
fn macro_value_buffer_is_runtime_sized() {
    let p = gpu_if_expression(8, 64);
    let buffer = p
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "macro_values")
        .expect("Fix: macro_values buffer must exist");
    assert_eq!(
        buffer.count, 0,
        "macro_values must be runtime-sized so one #if evaluator program serves all macro-table sizes"
    );
}

#[test]
fn source_buffer_layouts_preserve_packed_abi_and_raw_u8_variant() {
    let packed = gpu_if_expression(8, 64);
    let raw_u8 = gpu_if_expression_u8(8, 64);
    for name in ["source", "macro_names_packed"] {
        let packed_buffer = packed
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == name)
            .unwrap_or_else(|| panic!("Fix: packed #if evaluator {name} buffer must exist"));
        let raw_u8_buffer = raw_u8
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == name)
            .unwrap_or_else(|| panic!("Fix: raw-U8 #if evaluator {name} buffer must exist"));

        assert_eq!(packed_buffer.element(), DataType::U32);
        assert_eq!(packed_buffer.count(), 0);
        assert_eq!(raw_u8_buffer.element(), DataType::U8);
        assert_eq!(raw_u8_buffer.count(), 0);
    }
}

#[test]
fn reference_eval_modern_integer_literals_match_c_preprocessor_truth() {
    assert_eq!(eval_one_if(b"#if 1'024ULL == 1024\n"), 1);
    assert_eq!(eval_one_if(b"#if 0xFF'00z == 65280\n"), 1);
    assert_eq!(eval_one_if(b"#if 0b1010'0101WB == 165\n"), 1);
}

#[test]
fn reference_eval_has_builtin_uses_shared_gnu_catalog() {
    assert_eq!(eval_one_if(b"#if __has_builtin(__builtin_expect)\n"), 1);
    assert_eq!(eval_one_if(b"#if __has_builtin(__builtin_popcount)\n"), 1);
    assert_eq!(
        eval_one_if(b"#if __has_constexpr_builtin(__builtin_bitreverse32)\n"),
        1
    );
    assert_eq!(
        eval_one_if(b"#if __has_builtin(__builtin_vyre_unknown)\n"),
        0
    );
    assert_eq!(eval_one_if(b"#if __has_builtin(ordinary_identifier)\n"), 0);
}

#[test]
fn reference_eval_generic_has_operators_consume_arguments_as_false() {
    assert_eq!(eval_one_if(b"#if !__has_attribute(visibility)\n"), 1);
    assert_eq!(eval_one_if(b"#if __has_feature(c_static_assert)\n"), 0);
    assert_eq!(eval_one_if(b"#if __has_extension(c_alignof)\n"), 0);
    assert_eq!(eval_one_if(b"#if __has_include(<linux/types.h>)\n"), 0);
    assert_eq!(eval_one_if(b"#if __has_embed(__FILE__ limit(4))\n"), 0);
}

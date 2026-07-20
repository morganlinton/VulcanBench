use super::{gpu_if_expression, gpu_ifdef_value};
use vyre::ir::Program;

fn assert_program_shape(program: Program, buffers: usize) {
    assert_eq!(program.buffers().len(), buffers);
    assert_eq!(program.workgroup_size(), [256, 1, 1]);
}

#[test]
fn gpu_if_expression_abi_is_canonical_and_stable() {
    assert_eq!(
        gpu_if_expression::OP_ID,
        "vyre-libs::parsing::c::preprocess::gpu_if_expression"
    );
    assert_eq!(gpu_if_expression::BINDING_TOK_STARTS, 0);
    assert_eq!(gpu_if_expression::BINDING_TOK_LENS, 1);
    assert_eq!(gpu_if_expression::BINDING_DIRECTIVE_KINDS, 2);
    assert_eq!(gpu_if_expression::BINDING_SOURCE, 3);
    assert_eq!(gpu_if_expression::BINDING_MACRO_NAMES_PACKED, 4);
    assert_eq!(gpu_if_expression::BINDING_MACRO_OFFSETS, 5);
    assert_eq!(gpu_if_expression::BINDING_MACRO_VALUES, 6);
    assert_eq!(gpu_if_expression::BINDING_DIRECTIVE_VALUES, 7);

    assert_program_shape(gpu_if_expression::gpu_if_expression(8, 64), 8);
}

#[test]
fn gpu_ifdef_value_abi_is_canonical_and_stable() {
    assert_eq!(
        gpu_ifdef_value::OP_ID,
        "vyre-libs::parsing::c::preprocess::gpu_ifdef_value"
    );
    assert_eq!(gpu_ifdef_value::BINDING_TOK_STARTS, 0);
    assert_eq!(gpu_ifdef_value::BINDING_TOK_LENS, 1);
    assert_eq!(gpu_ifdef_value::BINDING_DIRECTIVE_KINDS, 2);
    assert_eq!(gpu_ifdef_value::BINDING_SOURCE, 3);
    assert_eq!(gpu_ifdef_value::BINDING_MACRO_NAMES_PACKED, 4);
    assert_eq!(gpu_ifdef_value::BINDING_MACRO_OFFSETS, 5);
    assert_eq!(gpu_ifdef_value::BINDING_DIRECTIVE_VALUES, 6);

    assert_program_shape(gpu_ifdef_value::gpu_ifdef_value(8, 64), 7);
}

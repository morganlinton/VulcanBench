#[cfg(test)]
use super::*;

#[test]
fn op_id_is_canonical_and_stable() {
    assert_eq!(
        OP_ID,
        "vyre-libs::parsing::c::preprocess::gpu_char_constant_scan"
    );
}

#[test]
fn binding_indices_are_canonical_and_stable() {
    assert_eq!(BINDING_SOURCE, 0);
    assert_eq!(BINDING_START_POS, 1);
    assert_eq!(BINDING_VALUE_OUT, 2);
    assert_eq!(BINDING_BYTES_CONSUMED_OUT, 3);
    assert_eq!(BINDING_OK_OUT, 4);
}

#[test]
fn build_program_returns_well_formed_program() {
    let p = gpu_char_constant_scan(64);
    assert_eq!(p.buffers().len(), 5);
    assert_eq!(p.workgroup_size(), [256, 1, 1]);
}

#[test]
fn source_buffer_is_runtime_sized_not_source_length_specialized() {
    let p = gpu_char_constant_scan(64);
    let source = p
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "source")
        .expect("Fix: source buffer must exist");
    assert_eq!(
        source.count(),
        0,
        "source must be runtime-sized so one scanner program serves all source lengths"
    );
}

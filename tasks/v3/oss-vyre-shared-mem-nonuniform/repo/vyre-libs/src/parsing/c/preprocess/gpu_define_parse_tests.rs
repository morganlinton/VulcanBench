use super::gpu_define_parse::*;

#[test]
fn op_id_is_canonical_and_stable() {
    assert_eq!(OP_ID, "vyre-libs::parsing::c::preprocess::gpu_define_parse");
}

#[test]
fn build_program_returns_well_formed_program() {
    let p = gpu_define_parse(8, 64);
    assert_eq!(p.buffers().len(), 11);
    assert_eq!(p.workgroup_size(), [256, 1, 1]);
}

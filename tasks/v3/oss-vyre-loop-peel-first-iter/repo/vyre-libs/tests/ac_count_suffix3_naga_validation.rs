//! Validate that the suffix3 AC count prefilter lowers cleanly through Naga.

use vyre_libs::scan::classic_ac::{
    build_ac_bounded_count_suffix3_prefilter_program, classic_ac_compile,
};

#[test]
fn ac_count_suffix3_prefilter_naga_validates() {
    let patterns: [&[u8]; 5] = [b"AKIA", b"ghp_", b"password=", b"BEGIN", b"unsafe {"];
    let ac = classic_ac_compile(&patterns);
    let program = build_ac_bounded_count_suffix3_prefilter_program(&ac.dfa);

    let lowered =
        vyre_lower::lower_for_emit(&program).expect("suffix3 AC count must lower to descriptor");
    let module =
        vyre_emit_naga::emit(&lowered.descriptor).expect("suffix3 AC count must emit to Naga");
    let validation = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module);

    if let Err(error) = validation {
        panic!("suffix3 AC count Naga validation failed: {error:?}");
    }
}

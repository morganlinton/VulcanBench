// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn gpu_parity_nested_conditional_preproc_stream() {
    let fix = fixture_nested_conditional_preproc();
    assert_full_pipeline_parity(&fix, "nested_conditional_preproc_stream");
}

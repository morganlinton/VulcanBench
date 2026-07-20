use super::*;

#[test]
fn gpu_parity_range_designator_array() {
    let (tok_types, tok_starts, tok_lens) = fixture_range_designator_array();
    assert_full_pipeline_parity(&tok_types, &tok_starts, &tok_lens, "range_designator_array");
}

#[test]
fn gpu_parity_union_field_designator() {
    let (tok_types, tok_starts, tok_lens) = fixture_union_field_designator();
    assert_full_pipeline_parity(&tok_types, &tok_starts, &tok_lens, "union_field_designator");
}

#[test]
fn gpu_parity_mixed_positional_designated() {
    let (tok_types, tok_starts, tok_lens) = fixture_mixed_positional_designated();
    assert_full_pipeline_parity(
        &tok_types,
        &tok_starts,
        &tok_lens,
        "mixed_positional_designated",
    );
}

#[test]
fn gpu_parity_compound_literal_nested() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_nested();
    assert_full_pipeline_parity(
        &tok_types,
        &tok_starts,
        &tok_lens,
        "compound_literal_nested",
    );
}

#[test]
fn gpu_parity_assignment_suppression() {
    let (tok_types, tok_starts, tok_lens) = fixture_assignment_suppression();
    assert_full_pipeline_parity(&tok_types, &tok_starts, &tok_lens, "assignment_suppression");
}

#[test]
fn gpu_parity_designator_assignment_class() {
    let (tok_types, tok_starts, tok_lens) = fixture_designator_assignment_class();
    assert_full_pipeline_parity(
        &tok_types,
        &tok_starts,
        &tok_lens,
        "designator_assignment_class",
    );
}

#[test]
fn gpu_parity_string_char_array_nested() {
    let (tok_types, tok_starts, tok_lens) = fixture_string_char_array_nested();
    assert_full_pipeline_parity(
        &tok_types,
        &tok_starts,
        &tok_lens,
        "string_char_array_nested",
    );
}

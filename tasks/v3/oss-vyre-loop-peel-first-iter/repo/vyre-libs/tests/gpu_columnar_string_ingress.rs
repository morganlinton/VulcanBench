//! Gpu columnar string ingress test suite.

const ABI: &str = include_str!("../../docs/optimization/GPU_COLUMNAR_STRING_INGRESS.toml");

const REQUIRED_BUFFERS: &[&str] = &["offsets", "chars", "null_mask", "row_range_output"];

#[test]
fn gpu_columnar_string_ingress_covers_required_buffers() {
    for buffer in REQUIRED_BUFFERS {
        assert!(
            ABI.contains(&format!("buffer_id = \"{buffer}\"")),
            "Fix: GPU columnar string ingress ABI must include buffer `{buffer}`"
        );
    }
}

#[test]
fn gpu_columnar_string_ingress_records_types_boundaries_and_zero_copy() {
    for required in [
        "element_type = \"u32\"",
        "element_type = \"u8\"",
        "element_type = \"bitmask\"",
        "element_type = \"span_record\"",
        "row_boundary_role = \"string_start_end\"",
        "row_boundary_role = \"per_row_match_ranges\"",
        "zero_copy_eligible = true",
        "zero_copy_eligible = false",
    ] {
        assert!(
            ABI.contains(required),
            "Fix: GPU columnar string ingress ABI must include `{required}`"
        );
    }
    assert_eq!(
        ABI.matches("evidence_path = \"vyre-libs/tests/gpu_columnar_string_ingress.rs\"").count(),
        ABI.matches("[[buffer]]").count(),
        "Fix: every GPU columnar string ingress row must point at this proof gate"
    );
}

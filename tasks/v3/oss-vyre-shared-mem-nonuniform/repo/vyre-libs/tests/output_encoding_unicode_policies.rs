//! Output encoding unicode policies test suite.

const CONTROLS: &str =
    include_str!("../../docs/optimization/CONTROL_CHARACTER_OUTPUT_POLICY.toml");
const ENCODING: &str =
    include_str!("../../docs/optimization/STRUCTURED_OUTPUT_ENCODING_POLICY.toml");
const UNICODE: &str =
    include_str!("../../docs/optimization/UNICODE_IDENTIFIER_SPOOFING_POLICY.toml");

#[test]
fn control_character_output_policy_records_surfaces_forbidden_controls_and_neutralization() {
    for required in [
        "surface_id",
        "output_surface",
        "forbidden_controls",
        "neutralization_policy",
        "terminal_policy",
        "log_policy",
        "machine_output_policy",
        "VYRE_CRLF_OUTPUT_REFUSED",
    ] {
        assert!(
            CONTROLS.contains(required),
            "control character output policy must include {required}"
        );
    }
}

#[test]
fn structured_output_encoding_policy_records_json_sarif_problem_encoding_contracts() {
    for required in [
        "encoding_id",
        "format",
        "string_policy",
        "number_policy",
        "field_name_policy",
        "control_character_policy",
        "roundtrip_contract",
        "VYRE_JSON_OUTPUT_ENCODING_REFUSED",
        "VYRE_SARIF_OUTPUT_ENCODING_REFUSED",
    ] {
        assert!(
            ENCODING.contains(required),
            "structured output encoding policy must include {required}"
        );
    }
}

#[test]
fn unicode_identifier_spoofing_policy_records_normalization_confusable_mixed_script_and_bidi_rules() {
    for required in [
        "identifier_class",
        "allowed_profile",
        "normalization_policy",
        "confusable_policy",
        "mixed_script_policy",
        "bidi_policy",
        "display_policy",
        "diagnostic",
        "reject-bidi-controls",
    ] {
        assert!(
            UNICODE.contains(required),
            "Unicode identifier spoofing policy must include {required}"
        );
    }
}

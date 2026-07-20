//! Url network security policies test suite.

const URL_POLICY: &str =
    include_str!("../../docs/optimization/URL_CANONICALIZATION_POLICY.toml");
const SSRF_GUARDS: &str =
    include_str!("../../docs/optimization/SSRF_DNS_REBINDING_GUARDS.toml");

#[test]
fn url_canonicalization_policy_records_parser_normalization_host_scheme_port_and_origin_rules() {
    for required in [
        "policy_id",
        "parser",
        "normalization_steps",
        "host_classification",
        "scheme_policy",
        "port_policy",
        "origin_policy",
        "diagnostic",
    ] {
        assert!(
            URL_POLICY.contains(required),
            "URL canonicalization policy must include {required}"
        );
    }
}

#[test]
fn ssrf_dns_rebinding_guards_record_resolution_address_redirect_cache_and_refusal_policy() {
    for required in [
        "guard_id",
        "url_policy",
        "dns_resolution_policy",
        "address_classification",
        "rebinding_check",
        "redirect_revalidation",
        "cache_policy",
        "refusal_diagnostic",
        "block-metadata-service",
    ] {
        assert!(
            SSRF_GUARDS.contains(required),
            "SSRF DNS rebinding guard must include {required}"
        );
    }
}

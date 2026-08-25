//! Http proxy redirect policy test suite.

const HTTP_POLICY: &str =
    include_str!("../../docs/optimization/HTTP_PROXY_REDIRECT_POLICY.toml");

#[test]
fn http_proxy_redirect_policy_records_method_header_redirect_proxy_tls_and_response_limits() {
    for required in [
        "policy_id",
        "method_policy",
        "header_policy",
        "redirect_policy",
        "proxy_policy",
        "tls_policy",
        "response_limit",
        "diagnostic",
        "proxy-env-ignored-unless-explicitly-enabled",
    ] {
        assert!(
            HTTP_POLICY.contains(required),
            "HTTP proxy redirect policy must include {required}"
        );
    }
}

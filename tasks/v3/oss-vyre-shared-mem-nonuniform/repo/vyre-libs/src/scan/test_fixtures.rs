//! Canonical literal / regex / haystack fixtures shared by every
//! integration test in `vyre-libs::matching`.
//!
//! Why a public module instead of a `tests/` helper file: integration
//! tests live in separate target binaries and can't share a private
//! `tests/common.rs` without path attributes. Exposing the
//! fixtures behind a feature flag keeps the public API surface
//! explicit, lets downstream consumers (scanner tests, conformance harness,
//! benchmark suites) reuse the *exact* corpus our regression tests use, and
//! avoids accidental drift between "the fixtures the tests use" and "the
//! fixtures downstream smoke-tests against."
//!
//! Compiled in two contexts:
//!
//! 1. `#[cfg(test)]`  -  always available to in-tree tests.
//! 2. `feature = "test-fixtures"`  -  exported to downstream crates that
//!    opt in.
//!
//! The fixtures are tiny by design. Production detector corpora belong in
//! dedicated corpus modules so small parity tests stay fast and readable.

/// Canonical AWS Access Key ID literal  -  used in nearly every parity,
/// cache, and dedup test as the "obvious-hit" payload.
pub const AKIA_LITERAL: &[u8] = b"AKIA";

/// Canonical GitHub PAT prefix used in mixed-pattern tests.
pub const GHP_PREFIX: &[u8] = b"ghp_";

/// A small mixed-credential haystack that contains exactly one AKIA
/// hit and one ghp_ hit at predictable offsets. Designed so that
/// brute-force `memchr` references and engine outputs can both
/// confirm the same `(pattern, start, end)` triples.
pub const MIXED_HAYSTACK: &[u8] = b"foo AKIA bar ghp_test baz";

/// A long synthesised haystack  -  32 repetitions of the mixed pattern.
/// Used by parity-on-large-input tests where the literal-set DFA is
/// expected to keep up over many KB.
#[must_use]
pub fn long_repeating_haystack() -> Vec<u8> {
    let mut buf = Vec::with_capacity(1024);
    for _ in 0..32 {
        buf.extend_from_slice(b"foo AKIA bar ghp_test baz ");
    }
    buf
}

/// Canonical literal-set fixture: two distinct, non-overlapping
/// patterns over [`MIXED_HAYSTACK`].
#[must_use]
pub fn canonical_literal_pair() -> (&'static [&'static [u8]], &'static [u8]) {
    static PATTERNS: &[&[u8]] = &[AKIA_LITERAL, GHP_PREFIX];
    (PATTERNS, MIXED_HAYSTACK)
}

/// Canonical overlapping-literal fixture: `abc` and `bc` share a
/// suffix. Used to exercise NFA-vs-DFA overlap-policy differences
/// without crashing.
#[must_use]
pub fn overlapping_literal_pair() -> (&'static [&'static [u8]], &'static [u8]) {
    static PATTERNS: &[&[u8]] = &[b"abc", b"bc"];
    (PATTERNS, b"xyz_abc_end")
}

/// Canonical regex fixtures: the small set every `matching-regex`
/// integration test uses. Paired with a haystack that contains at
/// least one match for each regex.
#[must_use]
pub fn canonical_regex_set() -> (&'static [&'static str], &'static [u8]) {
    static PATTERNS: &[&str] = &["AKIA[A-Z0-9]{4}", "ghp_[A-Za-z0-9]+", "[0-9]{4}"];
    static HAYSTACK: &[u8] = b"AKIAABCD foo ghp_token1 1234";
    (PATTERNS, HAYSTACK)
}

/// 200 realistic detector pattern bytestrings  -  the same corpus the
/// `cache_key_collision` integration test runs against. Kept in this
/// module so future cache-key contracts can exercise the same
/// production-shaped input set without duplicating the array.
#[must_use]
pub fn realistic_detector_pattern_corpus() -> &'static [&'static [u8]] {
    REALISTIC_DETECTOR_PATTERNS
}

const REALISTIC_DETECTOR_PATTERNS: &[&[u8]] = &[
    b"AKIA",
    b"ASIA",
    b"AGPA",
    b"AROA",
    b"AIDA",
    b"AIPA",
    b"ANPA",
    b"ANVA",
    b"ghp_",
    b"gho_",
    b"ghu_",
    b"ghs_",
    b"ghr_",
    b"github_pat_",
    b"sk-proj-",
    b"sk-ant-",
    b"sk-",
    b"AIza",
    b"ya29.",
    b"glpat-",
    b"xoxb-",
    b"xoxp-",
    b"xoxa-",
    b"xoxr-",
    b"xoxs-",
    b"xoxe.",
    b"slack_",
    b"npm_",
    b"npms-",
    b"py-",
    b"pypi-",
    b"dckr_",
    b"dckr_pat_",
    b"crates_",
    b"crates_io_",
    b"hf_",
    b"hub_",
    b"r8_",
    b"replicate_",
    b"sk-or-",
    b"sk-svcacct-",
    b"sgp_",
    b"sgs_",
    b"shppa_",
    b"shpat_",
    b"shpca_",
    b"shpss_",
    b"sq0atp-",
    b"sq0csp-",
    b"sq0idp-",
    b"sqOatp-",
    b"key-",
    b"SK_",
    b"PK_",
    b"acct_",
    b"AC",
    b"SK",
    b"rk_test_",
    b"rk_live_",
    b"sk_test_",
    b"sk_live_",
    b"pk_test_",
    b"pk_live_",
    b"whsec_",
    b"phc_",
    b"Bearer ",
    b"bearer ",
    b"BEARER ",
    b"Token ",
    b"-----BEGIN PRIVATE KEY-----",
    b"-----BEGIN RSA PRIVATE KEY-----",
    b"-----BEGIN OPENSSH PRIVATE KEY-----",
    b"-----BEGIN EC PRIVATE KEY-----",
    b"-----BEGIN DSA PRIVATE KEY-----",
    b"-----BEGIN PGP PRIVATE KEY BLOCK-----",
    b"-----BEGIN ENCRYPTED PRIVATE KEY-----",
    b"-----BEGIN CERTIFICATE-----",
    b"-----BEGIN PUBLIC KEY-----",
    b"jwt_",
    b"eyJ",
    b"oauth2:",
    b"oauth_",
    b"basic_",
    b"Basic ",
    b"BASIC ",
    b"AWS4-HMAC-SHA256",
    b"AWS4-",
    b"AWS_",
    b"aws_",
    b"AWS-",
    b"x-amz-",
    b"x-aws-",
    b"x-api-key:",
    b"X-API-Key:",
    b"X-API-KEY:",
    b"X-Auth-Token",
    b"x-auth-token",
    b"datadog-",
    b"DD_API_KEY",
    b"DD_APP_KEY",
    b"newrelic-",
    b"sentry_",
    b"sntry_",
    b"SENTRY_",
    b"sentry@",
    b"opsgenie-",
    b"pagerduty-",
    b"pagerdutyapi-",
    b"twilio_",
    b"AC[A-Za-z0-9]",
    b"SK[A-Za-z0-9]",
    b"firebase_",
    b"FIREBASE_",
    b"mongo_",
    b"mongodb_",
    b"redis_",
    b"REDIS_",
    b"postgres_",
    b"POSTGRES_",
    b"PG_",
    b"DATABASE_",
    b"PGPASSWORD",
    b"MYSQL_",
    b"mysql_",
    b"snowflake_",
    b"SNOWFLAKE_",
    b"databricks-",
    b"DATABRICKS_",
    b"airtable_",
    b"keyAa",
    b"key_aa",
    b"linear_",
    b"LINEAR_",
    b"asana_",
    b"ASANA_",
    b"jira_",
    b"JIRA_",
    b"confluence_",
    b"CONFLUENCE_",
    b"notion_",
    b"NOTION_",
    b"discord_",
    b"DISCORD_",
    b"twitch_",
    b"TWITCH_",
    b"telegram_",
    b"TELEGRAM_",
    b"signal_",
    b"SIGNAL_",
    b"matrix_",
    b"MATRIX_",
    b"webex_",
    b"WEBEX_",
    b"intercom_",
    b"INTERCOM_",
    b"zendesk_",
    b"ZENDESK_",
    b"freshdesk_",
    b"FRESHDESK_",
    b"servicenow_",
    b"SERVICENOW_",
    b"okta_",
    b"OKTA_",
    b"ssws ",
    b"SSWS ",
    b"auth0_",
    b"AUTH0_",
    b"clerk_",
    b"CLERK_",
    b"supabase_",
    b"SUPABASE_",
    b"vercel_",
    b"VERCEL_",
    b"netlify_",
    b"NETLIFY_",
    b"cloudflare_",
    b"CLOUDFLARE_",
    b"do_",
    b"DO_",
    b"linode_",
    b"LINODE_",
    b"vultr_",
    b"VULTR_",
    b"hetzner_",
    b"HETZNER_",
    b"ovh_",
    b"OVH_",
    b"scaleway_",
    b"SCALEWAY_",
    b"upstash_",
    b"UPSTASH_",
    b"planetscale_",
    b"PLANETSCALE_",
    b"neon_",
    b"NEON_",
    b"render_",
    b"RENDER_",
    b"flyio_",
    b"FLYIO_",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_is_nonempty_and_unique() {
        let pats = realistic_detector_pattern_corpus();
        assert!(!pats.is_empty(), "corpus must not be empty");
        let mut sorted: Vec<&[u8]> = pats.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            pats.len(),
            "corpus must contain only unique patterns",
        );
    }

    #[test]
    fn canonical_literal_pair_haystack_contains_each_pattern() {
        let (pats, hay) = canonical_literal_pair();
        for p in pats {
            assert!(
                hay.windows(p.len()).any(|w| w == *p),
                "haystack must contain pattern {p:?}",
            );
        }
    }

    #[test]
    fn long_repeating_haystack_has_multiple_hits() {
        let buf = long_repeating_haystack();
        let count = buf
            .windows(AKIA_LITERAL.len())
            .filter(|w| *w == AKIA_LITERAL)
            .count();
        assert_eq!(count, 32, "32 repetitions × 1 AKIA each");
    }

    #[test]
    fn overlapping_pair_haystack_has_both() {
        let (_pats, hay) = overlapping_literal_pair();
        assert!(hay.windows(3).any(|w| w == b"abc"));
        assert!(hay.windows(2).any(|w| w == b"bc"));
    }
}

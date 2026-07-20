//! Crypto rng key lifecycle test suite.

const LIFECYCLE: &str =
    include_str!("../../docs/optimization/CRYPTO_RNG_KEY_LIFECYCLE.toml");

#[test]
fn crypto_rng_key_lifecycle_records_algorithm_strength_rng_nonce_rotation_and_destruction() {
    for required in [
        "crypto_id",
        "purpose",
        "algorithm_policy",
        "key_strength",
        "rng_source",
        "nonce_policy",
        "rotation_policy",
        "destruction_policy",
        "diagnostic",
        "os-cryptographic-random",
    ] {
        assert!(
            LIFECYCLE.contains(required),
            "crypto RNG key lifecycle must include {required}"
        );
    }
}

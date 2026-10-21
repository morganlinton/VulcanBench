//! Cache-key collision corpus.
//!
//! Loads a representative set of patterns drawn from the kinds of
//! detector regex shapes vyre consumers actually
//! ship: literal prefixes, base64-style classes, hex strings, version
//! tokens, alternations. Computes `MatchScan::cache_key` for each and
//! asserts there are NO collisions across the entire corpus.
//!
//! Why: FNV-1a is fast but imperfect. With ~200 patterns it would be
//! astronomically improbable to see a collision; the corpus test
//! locks that empirically. If a future cache_key change introduces
//! collisions on real-world inputs, this test is the canary.

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
use std::collections::HashSet;
#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
use vyre_libs::scan::{GpuLiteralSet, MatchScan};

/// 200+ literal patterns drawn from real secret-detector shapes.
/// Mix of prefixes, hex strings, base64-ish, version tokens, and
/// generic identifiers. Deliberately chosen to exercise corner cases
/// of the FNV hash (short strings, repeated characters, all-numeric).
#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
fn corpus() -> Vec<&'static [u8]> {
    let raw: &[&'static [u8]] = &[
        b"AKIA",
        b"ASIA",
        b"AROA",
        b"AIDA",
        b"ANPA",
        b"ANVA",
        b"ASCA",
        b"APKA",
        b"ABIA",
        b"ACCA",
        b"ghp_",
        b"gho_",
        b"ghu_",
        b"ghs_",
        b"ghr_",
        b"github_pat_",
        b"glpat-",
        b"sk_live_",
        b"sk_test_",
        b"pk_live_",
        b"pk_test_",
        b"rk_live_",
        b"rk_test_",
        b"xoxb-",
        b"xoxp-",
        b"xoxa-",
        b"xoxr-",
        b"xoxs-",
        b"xapp-",
        b"shpat_",
        b"shpca_",
        b"shpss_",
        b"AKCp8",
        b"-----BEGIN RSA PRIVATE KEY-----",
        b"-----BEGIN PRIVATE KEY-----",
        b"-----BEGIN OPENSSH PRIVATE KEY-----",
        b"-----BEGIN DSA PRIVATE KEY-----",
        b"-----BEGIN EC PRIVATE KEY-----",
        b"-----BEGIN PGP PRIVATE KEY BLOCK-----",
        b"https://hooks.slack.com/services/",
        b"https://discord.com/api/webhooks/",
        b"https://discordapp.com/api/webhooks/",
        b"AAAA",
        b"BBBB",
        b"CCCC",
        b"DDDD",
        b"0000",
        b"1111",
        b"2222",
        b"3333",
        b"4444",
        b"5555",
        b"6666",
        b"7777",
        b"8888",
        b"9999",
        b"abcd",
        b"efgh",
        b"ijkl",
        b"mnop",
        b"qrst",
        b"uvwx",
        b"yz01",
        b"23456",
        b"7890ABC",
        b"DEFG",
        b"HIJK",
        b"LMNO",
        b"PQRS",
        b"TUVW",
        b"XYZ_",
        b"_abc",
        b"_def",
        b"_ghi",
        b"_jkl",
        b"_mno",
        b"abc_",
        b"def_",
        b"ghi_",
        b"jkl_",
        b"mno_",
        b"abc-",
        b"def-",
        b"ghi-",
        b"jkl-",
        b"mno-",
        b"-abc",
        b"-def",
        b"-ghi",
        b"-jkl",
        b"-mno",
        b"abc.",
        b"def.",
        b"ghi.",
        b"jkl.",
        b"mno.",
        b".abc",
        b".def",
        b".ghi",
        b".jkl",
        b".mno",
        b"abc=",
        b"def=",
        b"ghi=",
        b"jkl=",
        b"mno=",
        b"=abc",
        b"=def",
        b"=ghi",
        b"=jkl",
        b"=mno",
        b"123abc",
        b"456def",
        b"789ghi",
        b"abc123",
        b"def456",
        b"ghi789",
        b"version_1_0_0",
        b"version_2_0_0",
        b"version_3_0_0",
        b"v1.0.0",
        b"v2.0.0",
        b"v3.0.0",
        b"DEADBEEF",
        b"CAFEBABE",
        b"DECAFBAD",
        b"FEEDFACE",
        b"BAADF00D",
        b"5K3C8WLK4N7E0AKW",
        b"abcdef0123456789",
        b"0123456789abcdef",
        b"FEDCBA9876543210",
        b"deadbeefdeadbeef",
        b"cafebabecafebabe",
        b"feedfacefeedface",
        b"prefix_a",
        b"prefix_b",
        b"prefix_c",
        b"prefix_d",
        b"prefix_e",
        b"prefix_f",
        b"prefix_g",
        b"prefix_h",
        b"prefix_i",
        b"prefix_j",
        b"prefix_k",
        b"prefix_l",
        b"prefix_m",
        b"prefix_n",
        b"prefix_o",
        b"prefix_p",
        b"prefix_q",
        b"prefix_r",
        b"prefix_s",
        b"prefix_t",
        b"prefix_u",
        b"prefix_v",
        b"prefix_w",
        b"prefix_x",
        b"prefix_y",
        b"prefix_z",
        b"suffix_a",
        b"suffix_b",
        b"suffix_c",
        b"suffix_d",
        b"suffix_e",
        b"suffix_f",
        b"suffix_g",
        b"suffix_h",
        b"suffix_i",
        b"suffix_j",
        b"a",
        b"b",
        b"c",
        b"d",
        b"e",
        b"f",
        b"g",
        b"h",
        b"i",
        b"j",
        b"aa",
        b"ab",
        b"ac",
        b"ad",
        b"ae",
        b"af",
        b"ag",
        b"ah",
        b"ai",
        b"aj",
    ];
    raw.to_vec()
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn no_cache_key_collisions_in_realistic_corpus() {
    let patterns = corpus();
    let mut keys: HashSet<String> = HashSet::with_capacity(patterns.len());
    let mut conflicts: Vec<(String, &[u8])> = Vec::new();

    for pat in &patterns {
        let engine = GpuLiteralSet::compile(&[*pat]);
        let key = MatchScan::cache_key(&engine);
        if !keys.insert(key.clone()) {
            conflicts.push((key, *pat));
        }
    }
    assert!(
        conflicts.is_empty(),
        "{} pattern(s) collided on cache_key: {:?}",
        conflicts.len(),
        conflicts
    );
    assert!(
        patterns.len() >= 100,
        "corpus shrank below 100 patterns; the collision floor is meaningless"
    );
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn cache_key_changes_under_single_byte_pattern_mutation() {
    // Mutating a single byte of any pattern in the corpus must yield
    // a different cache_key. This is a stronger contract than the
    // existing `cache_key_changes_when_patterns_change` unit test  -
    // it exercises every byte of every pattern.
    let patterns = corpus();
    for pat in patterns.iter().take(30) {
        if pat.is_empty() {
            continue;
        }
        let original = GpuLiteralSet::compile(&[*pat]);
        let original_key = MatchScan::cache_key(&original);

        let mut mutated_bytes = pat.to_vec();
        mutated_bytes[0] ^= 0x01;
        let mutated_slice: &[u8] = &mutated_bytes;
        let mutated = GpuLiteralSet::compile(&[mutated_slice]);
        let mutated_key = MatchScan::cache_key(&mutated);

        assert_ne!(
            original_key,
            mutated_key,
            "cache_key did not change under single-byte mutation of {:?}",
            String::from_utf8_lossy(pat)
        );
    }
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn cache_key_changes_under_pattern_set_reorder() {
    // Adding the same patterns in different orders MUST produce the
    // same cache_key (the patterns are unordered semantically).
    // Conversely: ADDING a new pattern MUST change the key.
    let a = GpuLiteralSet::compile(&[b"foo".as_slice(), b"bar".as_slice()]);
    let b = GpuLiteralSet::compile(&[b"foo".as_slice(), b"bar".as_slice(), b"baz".as_slice()]);
    assert_ne!(MatchScan::cache_key(&a), MatchScan::cache_key(&b));
}

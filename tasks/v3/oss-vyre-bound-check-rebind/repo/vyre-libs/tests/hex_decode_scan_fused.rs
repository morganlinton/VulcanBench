//! FUSE-1 parity: hex_decode_then_aho_corasick vs (hex_decode → aho_corasick).

#![cfg(feature = "matching-dfa")]
#![allow(deprecated)]
mod common;
use common::{decode_u32_words, u32_bytes};
use vyre_libs::decode::{hex_decode, hex_decode_table, hex_decode_then_aho_corasick};
use vyre_libs::scan::{aho_corasick, dfa_compile};
use vyre_reference::value::Value;

fn hex_encode(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|b| [hex_digit(b >> 4), hex_digit(b & 0x0F)])
        .collect()
}

fn hex_digit(n: u8) -> u8 {
    match n {
        0..=9 => b'0' + n,
        10..=15 => b'a' + (n - 10),
        _ => b'0',
    }
}

fn run_fused(encoded: &[u8], dfa: &vyre_libs::scan::CompiledDfa) -> Vec<u32> {
    let input_len = encoded.len() as u32;
    let decoded_len = input_len / 2;
    let program = hex_decode_then_aho_corasick(
        "encoded",
        "decoded",
        "transitions",
        "accept",
        "matches",
        input_len,
        dfa.state_count,
    );
    let inputs = vec![
        Value::from(u32_bytes(
            &encoded.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(),
        )),
        Value::from(vec![0u8; decoded_len as usize * 4]),
        Value::from(u32_bytes(&dfa.transitions)),
        Value::from(u32_bytes(&dfa.accept)),
        Value::from(vec![0u8; decoded_len as usize * 4]),
        Value::from(u32_bytes(&hex_decode_table())),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs).expect("fused must run");
    // `decoded` is the first ReadWrite buffer (outputs[0]); `matches` is the second (outputs[1]).
    decode_u32_words(&outputs[1].to_bytes())
}

fn run_separate(encoded: &[u8], dfa: &vyre_libs::scan::CompiledDfa) -> Vec<u32> {
    let input_len = encoded.len() as u32;
    let decoded_len = input_len / 2;

    // Step 1: hex decode
    let decode_program = hex_decode("encoded", "decoded", input_len);
    let decode_inputs = vec![
        Value::from(u32_bytes(
            &encoded.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(),
        )),
        Value::from(vec![0u8; decoded_len as usize * 4]),
        Value::from(u32_bytes(&hex_decode_table())),
    ];
    let decode_outputs = vyre_reference::reference_eval(&decode_program, &decode_inputs)
        .expect("hex_decode must run");
    let decoded = decode_outputs[0].to_bytes();

    // Step 2: aho-corasick scan
    let scan_program = aho_corasick(
        "decoded",
        "transitions",
        "accept",
        "matches",
        decoded_len,
        dfa.state_count,
    );
    let scan_inputs = vec![
        Value::from(decoded),
        Value::from(u32_bytes(&dfa.transitions)),
        Value::from(u32_bytes(&dfa.accept)),
        Value::from(vec![0u8; decoded_len as usize * 4]),
    ];
    let scan_outputs =
        vyre_reference::reference_eval(&scan_program, &scan_inputs).expect("aho_corasick must run");
    decode_u32_words(&scan_outputs[0].to_bytes())
}

#[test]
fn fused_matches_separate_hex_decode_then_aho_corasick() {
    let patterns: &[&[u8]] = &[b"ab", b"cd", b"ef"];
    let haystack = b"abcdefabcd";
    let encoded = hex_encode(haystack);
    let dfa = dfa_compile(patterns);

    let fused = run_fused(&encoded, &dfa);
    let separate = run_separate(&encoded, &dfa);

    assert_eq!(
        fused, separate,
        "fused hex_decode_then_aho_corasick diverged from separate stages\n  haystack = {haystack:?}\n  fused    = {fused:?}\n  separate = {separate:?}"
    );
}

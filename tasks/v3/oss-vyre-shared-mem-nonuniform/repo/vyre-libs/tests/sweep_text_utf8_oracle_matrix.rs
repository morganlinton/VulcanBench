//! Handwritten oracle matrix for UTF-8 validation classification.
//!
//! Compares `vyre_primitives::text::utf8_validate::reference_utf8_validate`
//! against an independent byte-classification oracle over 1024 hostile corpora.

#![forbid(unsafe_code)]
#![cfg(feature = "text")]

use vyre_primitives::text::utf8_validate::{
    reference_utf8_validate, UTF8_ASCII, UTF8_CONT, UTF8_INVALID, UTF8_LEAD_2, UTF8_LEAD_3,
    UTF8_LEAD_4,
};

const UTF8_CASES: u32 = 1024;

#[test]
fn utf8_validate_oracle_matrix_matches_independent_byte_classifier() {
    let mut assertions = 0usize;
    for case in 0..UTF8_CASES {
        let bytes = hostile_utf8_bytes(case);
        let actual = reference_utf8_validate(&bytes);
        let expected = oracle_utf8_validate(&bytes);
        assert_eq!(
            actual,
            expected,
            "Fix: utf8_validate case {case} len={} must match the independent oracle.",
            bytes.len()
        );
        assertions += 1;

        assert_eq!(
            actual.len(),
            bytes.len(),
            "Fix: utf8_validate case {case} must emit one class per input byte."
        );
        assertions += 1;
    }
    assert_eq!(assertions, UTF8_CASES as usize * 2);
}

fn oracle_utf8_validate(source: &[u8]) -> Vec<u32> {
    (0..source.len())
        .map(|idx| oracle_class_at(source, idx))
        .collect()
}

fn oracle_is_cont(byte: u8) -> bool {
    matches!(byte, 0x80..=0xBF)
}

fn oracle_valid_lead2(source: &[u8], idx: usize) -> bool {
    matches!(source[idx], 0xC2..=0xDF) && source.get(idx + 1).copied().is_some_and(oracle_is_cont)
}

fn oracle_valid_lead3(source: &[u8], idx: usize) -> bool {
    let Some(&b1) = source.get(idx + 1) else {
        return false;
    };
    let Some(&b2) = source.get(idx + 2) else {
        return false;
    };
    let first_ok = match source[idx] {
        0xE0 => matches!(b1, 0xA0..=0xBF),
        0xE1..=0xEC | 0xEE..=0xEF => oracle_is_cont(b1),
        0xED => matches!(b1, 0x80..=0x9F),
        _ => false,
    };
    first_ok && oracle_is_cont(b2)
}

fn oracle_valid_lead4(source: &[u8], idx: usize) -> bool {
    let Some(&b1) = source.get(idx + 1) else {
        return false;
    };
    let Some(&b2) = source.get(idx + 2) else {
        return false;
    };
    let Some(&b3) = source.get(idx + 3) else {
        return false;
    };
    let first_ok = match source[idx] {
        0xF0 => matches!(b1, 0x90..=0xBF),
        0xF1..=0xF3 => oracle_is_cont(b1),
        0xF4 => matches!(b1, 0x80..=0x8F),
        _ => false,
    };
    first_ok && oracle_is_cont(b2) && oracle_is_cont(b3)
}

fn oracle_valid_cont_position(source: &[u8], idx: usize) -> bool {
    idx.checked_sub(1).is_some_and(|lead| {
        oracle_valid_lead2(source, lead)
            || oracle_valid_lead3(source, lead)
            || oracle_valid_lead4(source, lead)
    }) || idx
        .checked_sub(2)
        .is_some_and(|lead| oracle_valid_lead3(source, lead) || oracle_valid_lead4(source, lead))
        || idx
            .checked_sub(3)
            .is_some_and(|lead| oracle_valid_lead4(source, lead))
}

fn oracle_class_at(source: &[u8], idx: usize) -> u32 {
    match source[idx] {
        0x00..=0x7F => UTF8_ASCII,
        0x80..=0xBF if oracle_valid_cont_position(source, idx) => UTF8_CONT,
        0x80..=0xBF => UTF8_INVALID,
        0xC2..=0xDF if oracle_valid_lead2(source, idx) => UTF8_LEAD_2,
        0xE0..=0xEF if oracle_valid_lead3(source, idx) => UTF8_LEAD_3,
        0xF0..=0xF4 if oracle_valid_lead4(source, idx) => UTF8_LEAD_4,
        _ => UTF8_INVALID,
    }
}

fn hostile_utf8_bytes(seed: u32) -> Vec<u8> {
    const MUTATIONS: &[&[u8]] = &[
        b"",
        b"a",
        b"\xC3\xA9",
        b"\xE2\x82\xAC",
        b"\xF0\x9F\x98\x80",
        b"\xC0\xC1",
        b"\xF8\xFC\xFF",
        b"\x80",
        b"\xED\xA0\x80",
        b"\xE0\x80\x80",
        b"\xF0\x80\x80\x80",
    ];
    let base = MUTATIONS[(seed as usize) % MUTATIONS.len()];
    let extra_len = ((seed.wrapping_mul(37) ^ seed.rotate_left(11)) % 64) as usize;
    let mut out = base.to_vec();
    let mut state = seed ^ 0x8F8E_5EED;
    for idx in 0..extra_len {
        state = state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            .rotate_left((idx as u32) & 15);
        out.push(match (state as usize + idx) % 7 {
            0 => 0x00,
            1 => 0x7F,
            2 => 0x80 | (state as u8 & 0x3F),
            3 => 0xC0 | (state as u8 & 0x1F),
            4 => 0xE0 | (state as u8 & 0x0F),
            5 => 0xF0 | (state as u8 & 0x07),
            _ => state as u8,
        });
    }
    out
}

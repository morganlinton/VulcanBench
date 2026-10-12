//! Generated oracle checks for histogram-based text encoding classification.

#![cfg(all(feature = "text", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::text::byte_histogram::reference_byte_histogram;
use vyre_primitives::text::encoding_classify::{
    classify_from_histogram, ENC_ASCII, ENC_ISO8859_1, ENC_UTF16LE, ENC_UTF8,
};

fn weighted_byte_strategy() -> impl Strategy<Value = u8> {
    prop_oneof![
        12 => 0x20u8..=0x7e,
        2 => Just(0u8),
        4 => 0x80u8..=0xbf,
        4 => 0xc2u8..=0xdf,
        3 => 0xe0u8..=0xef,
        2 => 0xf0u8..=0xf4,
        1 => prop_oneof![Just(0xc0u8), Just(0xc1u8), Just(0xf5u8), Just(0xffu8)],
        8 => any::<u8>(),
    ]
}

fn count_bytes(source: &[u8], pred: impl Fn(u8) -> bool) -> u32 {
    source
        .iter()
        .filter(|&&byte| pred(byte))
        .count()
        .try_into()
        .expect("generated text encoding test input length fits u32")
}

fn range_sum_saturating(histogram: &[u32], start: usize, end: usize) -> u32 {
    histogram[start..end]
        .iter()
        .fold(0u32, |acc, &count| acc.saturating_add(count))
}

fn range_weighted_sum_saturating(histogram: &[u32], start: usize, end: usize, weight: u32) -> u32 {
    histogram[start..end].iter().fold(0u32, |acc, &count| {
        acc.saturating_add(count.saturating_mul(weight))
    })
}

fn independent_encoding_from_bytes(source: &[u8]) -> u32 {
    let count: u32 = source
        .len()
        .try_into()
        .expect("generated text encoding test input length fits u32");
    if count == 0 {
        return ENC_ASCII;
    }

    let null_count = count_bytes(source, |byte| byte == 0);
    let ascii_count = count_bytes(source, |byte| byte < 0x80);
    let high_count = count - ascii_count;

    if null_count > count / 8 {
        return ENC_UTF16LE;
    }
    if high_count == 0 {
        return ENC_ASCII;
    }

    let continuation = count_bytes(source, |byte| (0x80..0xc0).contains(&byte));
    let starter_2 = count_bytes(source, |byte| (0xc2..0xe0).contains(&byte));
    let starter_3 = count_bytes(source, |byte| (0xe0..0xf0).contains(&byte));
    let starter_4 = count_bytes(source, |byte| (0xf0..0xf5).contains(&byte));
    let expected_continuation = starter_2
        .saturating_add(starter_3.saturating_mul(2))
        .saturating_add(starter_4.saturating_mul(3));
    let tolerance = count.saturating_add(19) / 20;

    if continuation.abs_diff(expected_continuation) < tolerance {
        ENC_UTF8
    } else {
        ENC_ISO8859_1
    }
}

fn independent_encoding_from_histogram(histogram: &[u32; 256], count: u32) -> u32 {
    if count == 0 {
        return ENC_ASCII;
    }

    let null_count = histogram[0];
    let ascii_count = range_sum_saturating(histogram, 0, 128);
    let high_count = count.saturating_sub(ascii_count);

    if null_count > count / 8 {
        return ENC_UTF16LE;
    }
    if high_count == 0 {
        return ENC_ASCII;
    }

    let continuation = range_sum_saturating(histogram, 0x80, 0xc0);
    let expected_continuation = range_sum_saturating(histogram, 0xc2, 0xe0)
        .saturating_add(range_weighted_sum_saturating(histogram, 0xe0, 0xf0, 2))
        .saturating_add(range_weighted_sum_saturating(histogram, 0xf0, 0xf5, 3));
    let tolerance = count.saturating_add(19) / 20;

    if continuation.abs_diff(expected_continuation) < tolerance {
        ENC_UTF8
    } else {
        ENC_ISO8859_1
    }
}

fn generated_histogram_case(case: u32) -> ([u32; 256], u32, u32) {
    let mut histogram = [0u32; 256];
    match case % 8 {
        0 => {
            let len = 1 + case % 2048;
            for step in 0..len {
                let byte = b' ' as usize + ((case + step * 17) % 95) as usize;
                histogram[byte] += 1;
            }
            (histogram, len, ENC_ASCII)
        }
        1 => {
            let pairs = 1 + case % 2048;
            histogram[0xc3] = pairs;
            histogram[0xa9] = pairs;
            (histogram, pairs * 2, ENC_UTF8)
        }
        2 => {
            let triples = 1 + case % 1365;
            histogram[0xe2] = triples;
            histogram[0x82] = triples;
            histogram[0xac] = triples;
            (histogram, triples * 3, ENC_UTF8)
        }
        3 => {
            let quads = 1 + case % 1024;
            histogram[0xf0] = quads;
            histogram[0x9f] = quads;
            histogram[0x98] = quads;
            histogram[0x80] = quads;
            (histogram, quads * 4, ENC_UTF8)
        }
        4 => {
            let starters = 32 + case % 2048;
            histogram[0xc3] = starters;
            (histogram, starters, ENC_ISO8859_1)
        }
        5 => {
            let count = 8 + case % 2048;
            let nulls = count / 8 + 1;
            histogram[0] = nulls;
            histogram[b'a' as usize] = count - nulls;
            (histogram, count, ENC_UTF16LE)
        }
        6 => (histogram, 0, ENC_ASCII),
        _ if case & 8 == 0 => {
            histogram[0xe0] = u32::MAX / 2 + 1;
            (histogram, histogram[0xe0], ENC_ISO8859_1)
        }
        _ => {
            histogram[0x80] = 2;
            histogram[0xf0] = u32::MAX / 3 + 1;
            (histogram, histogram[0x80] + histogram[0xf0], ENC_ISO8859_1)
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn histogram_classifier_matches_independent_byte_oracle(
        source in proptest::collection::vec(weighted_byte_strategy(), 0..=512),
    ) {
        let histogram = reference_byte_histogram(&source);
        let expected = independent_encoding_from_bytes(&source);
        prop_assert_eq!(
            classify_from_histogram(&histogram, source.len() as u32),
            expected,
            "source_len={}",
            source.len()
        );
    }

    #[test]
    fn null_density_takes_utf16_path(len in 1u32..=512) {
        let nulls = len / 8 + 1;
        let mut source = vec![0u8; nulls as usize];
        source.resize(len as usize, b'a');
        let histogram = reference_byte_histogram(&source);
        prop_assert_eq!(
            classify_from_histogram(&histogram, len),
            ENC_UTF16LE,
            "len={} nulls={}",
            len,
            nulls
        );
    }

    #[test]
    fn imbalanced_high_bytes_fall_back_to_iso(starters in 32u32..=2048) {
        let mut histogram = [0u32; 256];
        histogram[0xc3] = starters;
        prop_assert_eq!(
            classify_from_histogram(&histogram, starters),
            ENC_ISO8859_1,
            "starters={}",
            starters
        );
    }

    #[test]
    fn balanced_high_bytes_take_utf8_path(pairs in 1u32..=2048) {
        let mut histogram = [0u32; 256];
        histogram[0xc3] = pairs;
        histogram[0xa9] = pairs;
        prop_assert_eq!(
            classify_from_histogram(&histogram, pairs * 2),
            ENC_UTF8,
            "pairs={}",
            pairs
        );
    }
}

#[test]
fn generated_histogram_matrix_matches_independent_oracle() {
    let mut saw_ascii = false;
    let mut saw_utf8 = false;
    let mut saw_utf16 = false;
    let mut saw_iso = false;

    for case in 0..4096u32 {
        let (histogram, count, expected_class) = generated_histogram_case(case);
        let independent = independent_encoding_from_histogram(&histogram, count);
        let actual = classify_from_histogram(&histogram, count);
        assert_eq!(
            independent, expected_class,
            "generated independent class mismatch case={case} count={count}"
        );
        assert_eq!(
            actual, independent,
            "generated classifier mismatch case={case} count={count}"
        );

        saw_ascii |= expected_class == ENC_ASCII;
        saw_utf8 |= expected_class == ENC_UTF8;
        saw_utf16 |= expected_class == ENC_UTF16LE;
        saw_iso |= expected_class == ENC_ISO8859_1;
    }

    assert!(saw_ascii);
    assert!(saw_utf8);
    assert!(saw_utf16);
    assert!(saw_iso);
}

//! Failure-oriented adversarial tests for text primitives without existing adversarial suites.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(all(feature = "text", feature = "cpu-parity"))]
#![allow(clippy::needless_range_loop)]

use vyre_foundation::ir::DataType;
use vyre_primitives::text::byte_histogram::{byte_histogram_256_u8, reference_byte_histogram};
use vyre_primitives::text::utf8_shape_counts::reference_utf8_shape_counts;

#[test]
fn reference_byte_histogram_empty() {
    let got = reference_byte_histogram(b"");
    assert!(got.iter().all(|&c| c == 0));
}

#[test]
fn reference_byte_histogram_all_same_byte() {
    for byte in [0x00, 0x7F, 0x80, 0xFF] {
        let input = vec![byte; 1024];
        let got = reference_byte_histogram(&input);
        assert_eq!(got[byte as usize], 1024, "byte 0x{byte:02X} count mismatch");
        assert!(
            got.iter()
                .enumerate()
                .all(|(i, &c)| i == byte as usize || c == 0),
            "only byte 0x{byte:02X} should have non-zero count"
        );
    }
}

#[test]
fn reference_byte_histogram_hostile_lengths() {
    for len in [0, 1, 31, 32, 33, 255, 256, 1023, 1024] {
        let input = vec![0xABu8; len];
        let got = reference_byte_histogram(&input);
        assert_eq!(got[0xAB], len as u32, "count mismatch for length {len}");
    }
}

#[test]
fn reference_byte_histogram_every_byte_once() {
    let input: Vec<u8> = (0..=255).collect();
    let got = reference_byte_histogram(&input);
    assert!(got.iter().all(|&c| c == 1));
}

#[test]
fn packed_u8_byte_histogram_uses_one_source_byte_per_element() {
    let program = byte_histogram_256_u8("source", "histogram", 1024);
    let source = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "source")
        .expect("Fix: packed-u8 byte histogram source buffer must be declared");
    let histogram = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "histogram")
        .expect("Fix: byte histogram output buffer must be declared");

    assert_eq!(source.element(), DataType::U8);
    assert_eq!(source.count(), 1024);
    assert_eq!(
        source.count() as usize * DataType::U8.min_bytes(),
        1024,
        "Fix: packed-u8 byte histogram must consume one byte per source byte."
    );
    assert_eq!(
        source.count() as usize * DataType::U32.min_bytes(),
        4096,
        "Fix: compatibility byte histogram remains the four-byte-per-source-byte path."
    );
    assert_eq!(histogram.element(), DataType::U32);
    assert_eq!(histogram.count(), 256);
}

#[test]
fn reference_utf8_shape_counts_empty_histogram() {
    let histogram = [0u32; 256];
    let (cont, expected) = reference_utf8_shape_counts(&histogram);
    assert_eq!(cont, 0);
    assert_eq!(expected, 0);
}

#[test]
fn reference_utf8_shape_counts_all_continuations() {
    let mut histogram = [0u32; 256];
    for i in 0x80..0xC0 {
        histogram[i] = 1;
    }
    let (cont, expected) = reference_utf8_shape_counts(&histogram);
    assert_eq!(cont, 0xC0 - 0x80);
    assert_eq!(expected, 0);
}

#[test]
fn reference_utf8_shape_counts_all_starters() {
    let mut histogram = [0u32; 256];
    // 2-byte starters
    for i in 0xC2..0xE0 {
        histogram[i] = 1;
    }
    // 3-byte starters
    for i in 0xE0..0xF0 {
        histogram[i] = 1;
    }
    // 4-byte starters
    for i in 0xF0..0xF5 {
        histogram[i] = 1;
    }
    let (cont, expected) = reference_utf8_shape_counts(&histogram);
    assert_eq!(cont, 0);
    let starter_2 = 0xE0 - 0xC2;
    let starter_3 = 0xF0 - 0xE0;
    let starter_4 = 0xF5 - 0xF0;
    assert_eq!(expected, starter_2 + starter_3 * 2 + starter_4 * 3);
}

#[test]
fn reference_utf8_shape_counts_large_counts() {
    let mut histogram = [0u32; 256];
    histogram[0xC2] = 100_000_000;
    histogram[0xE0] = 100_000_000;
    histogram[0xF0] = 100_000_000;
    let (cont, expected) = reference_utf8_shape_counts(&histogram);
    assert_eq!(cont, 0);
    assert_eq!(expected, 100_000_000 + 100_000_000 * 2 + 100_000_000 * 3);
}

#[test]
fn reference_utf8_shape_counts_saturates_three_byte_multiplier() {
    let mut histogram = [0u32; 256];
    histogram[0xE0] = u32::MAX / 2 + 1;
    let (cont, expected) = reference_utf8_shape_counts(&histogram);
    assert_eq!(cont, 0);
    assert_eq!(expected, u32::MAX);
}

#[test]
fn reference_utf8_shape_counts_saturates_four_byte_multiplier() {
    let mut histogram = [0u32; 256];
    histogram[0xF0] = u32::MAX / 3 + 1;
    let (cont, expected) = reference_utf8_shape_counts(&histogram);
    assert_eq!(cont, 0);
    assert_eq!(expected, u32::MAX);
}

#[test]
fn reference_utf8_shape_counts_saturates_accumulated_expected_count() {
    let mut histogram = [0u32; 256];
    histogram[0xC2] = u32::MAX;
    histogram[0xE0] = 1;
    let (cont, expected) = reference_utf8_shape_counts(&histogram);
    assert_eq!(cont, 0);
    assert_eq!(expected, u32::MAX);
}

#[test]
fn reference_utf8_shape_counts_boundary_bytes() {
    // Test exact boundary bytes
    let mut histogram = [0u32; 256];
    histogram[0x7F] = 1; // just before continuation
    histogram[0x80] = 1; // first continuation
    histogram[0xBF] = 1; // last continuation
    histogram[0xC0] = 1; // first invalid starter range
    histogram[0xC1] = 1; // second invalid starter range
    histogram[0xC2] = 1; // first valid 2-byte starter
    histogram[0xDF] = 1; // last 2-byte starter
    histogram[0xE0] = 1; // first 3-byte starter
    histogram[0xEF] = 1; // last 3-byte starter
    histogram[0xF0] = 1; // first 4-byte starter
    histogram[0xF4] = 1; // last 4-byte starter
    histogram[0xF5] = 1; // just after 4-byte range (excluded by loop)
    let (cont, expected) = reference_utf8_shape_counts(&histogram);
    assert_eq!(cont, 2); // 0x80, 0xBF
                         // 0xC0, 0xC1 are NOT counted (excluded by >0xC1)
                         // 0xC2, 0xDF counted as 2-byte (2)
                         // 0xE0, 0xEF counted as 3-byte (2*2=4)
                         // 0xF0, 0xF4 counted as 4-byte (2*3=6)
    assert_eq!(expected, 2 + 4 + 6);
}

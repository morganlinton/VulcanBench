//! Property gates for `text::byte_histogram::reference_byte_histogram`.
//! Requires `cpu-parity` because the reference CPU oracle is gated
//! behind that feature alongside `text`.
#![cfg(all(feature = "text", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_foundation::ir::DataType;
use vyre_primitives::text::byte_histogram::{byte_histogram_256_u8, reference_byte_histogram};
use vyre_reference::value::Value;

fn unpack_u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            u32::from_le_bytes(chunk.try_into().expect("Fix: u32 chunk conversion failed"))
        })
        .collect()
}

fn run_packed_u8_program(source: &[u8]) -> Vec<u32> {
    let program = byte_histogram_256_u8("source", "histogram", source.len() as u32);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(source.to_vec())])
        .expect("Fix: packed-u8 byte_histogram reference evaluation must succeed");
    let mut histogram = unpack_u32s(&outputs[0].to_bytes());
    histogram.truncate(256);
    histogram
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn histogram_sum_equals_input_length(
        bytes in proptest::collection::vec(any::<u8>(), 0..=256),
    ) {
        let hist = reference_byte_histogram(&bytes);
        let sum: u32 = hist.iter().sum();
        prop_assert_eq!(sum, bytes.len() as u32, "histogram sum must equal input length");
    }

    #[test]
    fn histogram_bins_are_nonnegative(
        bytes in proptest::collection::vec(any::<u8>(), 0..=256),
    ) {
        let hist = reference_byte_histogram(&bytes);
        for (i, &count) in hist.iter().enumerate() {
            prop_assert!(count <= bytes.len() as u32, "bin {i} exceeds input length");
        }
    }

    #[test]
    fn empty_input_yields_all_zeros(_dummy in 0u32..1) {
        let hist = reference_byte_histogram(b"");
        prop_assert_eq!(hist, [0u32; 256]);
    }

    #[test]
    fn single_byte_increments_exactly_one_bin(b in any::<u8>()) {
        let hist = reference_byte_histogram(&[b]);
        let sum: u32 = hist.iter().sum();
        prop_assert_eq!(sum, 1);
        prop_assert_eq!(hist[b as usize], 1);
    }

    #[test]
    fn histogram_is_additive_over_concatenation(
        a in proptest::collection::vec(any::<u8>(), 0..=128),
        b in proptest::collection::vec(any::<u8>(), 0..=128),
    ) {
        let mut combined = a.clone();
        combined.extend_from_slice(&b);
        let hist_a = reference_byte_histogram(&a);
        let hist_b = reference_byte_histogram(&b);
        let hist_combined = reference_byte_histogram(&combined);
        for i in 0..256 {
            prop_assert_eq!(
                hist_combined[i],
                hist_a[i].wrapping_add(hist_b[i]),
                "bin {} must be additive over concatenation",
                i
            );
        }
    }

    #[test]
    fn packed_u8_builder_keeps_byte_source(
        n in 0u32..=4096,
    ) {
        let program = byte_histogram_256_u8("source", "histogram", n);
        let has_u8_source = program.buffers().iter().any(|buffer| {
            buffer.name() == "source"
                && buffer.element() == DataType::U8
                && (n == 0 || buffer.count() == n)
        });
        let has_u32_histogram = program.buffers().iter().any(|buffer| {
            buffer.name() == "histogram"
                && buffer.element() == DataType::U32
                && buffer.count() == 256
                && buffer.is_output()
        });

        prop_assert!(has_u8_source, "byte_histogram_256_u8 source must be packed U8 for n={n}");
        prop_assert!(has_u32_histogram, "byte_histogram_256_u8 output must remain a 256-bin U32 histogram for n={n}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_048))]

    #[test]
    fn packed_u8_program_matches_reference_histogram(
        source in proptest::collection::vec(any::<u8>(), 0..=256),
    ) {
        prop_assert_eq!(
            run_packed_u8_program(&source),
            reference_byte_histogram(&source).to_vec()
        );
    }
}

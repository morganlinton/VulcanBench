//! `whitespace_classify_word`  -  word-at-a-time whitespace classification
//! emitting a per-word bitmap of "is-whitespace" lanes.
//!
//! Op id: `vyre-primitives::parsing::whitespace_classify_word`. Soundness:
//! `Exact` over the JSON / CSV / structural-parser whitespace set
//! `{0x20 SP, 0x09 TAB, 0x0A LF, 0x0D CR}`. The Reference oracle at the
//! bottom of this file is the contract; the GPU `Program` matches it
//! lane-for-lane.
//!
//! ## Why it matters
//!
//! Every structural parser (simdjson-style JSON, CSV, HTTP header, protobuf
//! text-format, INI, YAML) starts with a whitespace-skip pass that compresses
//! N input bytes down to N/k structural bytes. The bottleneck on GPU is the
//! per-byte branch  -  naive `if (c == ' ' || c == '\t' || ...) skip` collapses
//! warp efficiency the moment the input has mixed structure.
//!
//! The fix is the simdjson trick: load a whole word (4 bytes per u32 here),
//! compare every lane to every whitespace value in parallel using bitmask
//! arithmetic, OR-reduce the 4 bitmaps into one 4-bit mask per word, and
//! emit the bitmap. A prefix-scan compacts the non-whitespace bytes into
//! a dense output. This module ships the classify half consumed by
//! `stream_compact`.
//!
//! ## Wire layout
//!
//! Inputs:
//!   - `bytes_in`  -  packed u32 stream where each u32 holds 4 little-endian
//!     bytes (byte 0 in bits 0..7, byte 1 in 8..15, etc). The host is
//!     responsible for padding the final word with non-whitespace bytes
//!     (0xFF is canonical) so the classifier doesn't emit spurious skip
//!     bits past the actual end-of-input.
//!
//! Outputs:
//!   - `whitespace_mask_out`  -  one u32 per input word. Low 4 bits are the
//!     per-lane "is-whitespace" mask: bit 0 = byte 0, bit 1 = byte 1, etc.
//!     The high 28 bits are zero and remain available to wider-lane
//!     variants.
//!
//! ## Why the bitmask and not a per-byte branch
//!
//! Per-byte branches force the warp into 4-way divergence on every word.
//! The bitmask approach uses pure arithmetic  -  no branches, every lane
//! does the same work, GPU throughput stays at peak.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::parsing::whitespace_classify_word";

/// Canonical binding index for the input byte stream.
pub const BINDING_BYTES_IN: u32 = 0;
/// Canonical binding index for the output bitmap.
pub const BINDING_WHITESPACE_MASK_OUT: u32 = 1;
/// Word-lane workgroup used by the whitespace classifier.
pub const WHITESPACE_CLASSIFY_WORD_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for classifying `word_count` packed u32 words.
#[must_use]
pub const fn whitespace_classify_word_dispatch_grid(word_count: u32) -> [u32; 3] {
    let blocks = word_count.div_ceil(WHITESPACE_CLASSIFY_WORD_WORKGROUP_SIZE[0]);
    if blocks == 0 {
        [1, 1, 1]
    } else {
        [blocks, 1, 1]
    }
}

/// JSON / CSV / structural whitespace set: SP, TAB, LF, CR.
const WS_SP: u32 = 0x20;
const WS_TAB: u32 = 0x09;
const WS_LF: u32 = 0x0A;
const WS_CR: u32 = 0x0D;

/// Build the IR `Program` that classifies whitespace word-by-word.
///
/// One thread per word. Each thread:
///   1. Loads `bytes_in[gid]` as a u32 holding 4 bytes.
///   2. For each of the 4 byte lanes, computes whether the byte equals one
///      of {SP, TAB, LF, CR} via subtract-and-zero-detect arithmetic
///      (no branches).
///   3. Packs the 4 per-lane booleans into a 4-bit mask in the low bits of
///      one u32, stores at `whitespace_mask_out[gid]`.
///
/// `word_count` is the number of u32 words in `bytes_in`. Workgroup size
/// is fixed at 256 lanes.
#[must_use]
pub fn whitespace_classify_word(word_count: u32) -> Program {
    let body = vec![
        Node::let_bind("word_idx", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("word_idx"), Expr::u32(word_count)),
            vec![
                Node::let_bind("word", Expr::load("bytes_in", Expr::var("word_idx"))),
                // Extract each byte lane.
                Node::let_bind("b0", Expr::bitand(Expr::var("word"), Expr::u32(0xFF))),
                Node::let_bind(
                    "b1",
                    Expr::bitand(Expr::shr(Expr::var("word"), Expr::u32(8)), Expr::u32(0xFF)),
                ),
                Node::let_bind(
                    "b2",
                    Expr::bitand(Expr::shr(Expr::var("word"), Expr::u32(16)), Expr::u32(0xFF)),
                ),
                Node::let_bind(
                    "b3",
                    Expr::bitand(Expr::shr(Expr::var("word"), Expr::u32(24)), Expr::u32(0xFF)),
                ),
                // Per-lane is-whitespace booleans (returned as bool by `eq`).
                Node::let_bind(
                    "ws0",
                    Expr::or(
                        Expr::or(
                            Expr::eq(Expr::var("b0"), Expr::u32(WS_SP)),
                            Expr::eq(Expr::var("b0"), Expr::u32(WS_TAB)),
                        ),
                        Expr::or(
                            Expr::eq(Expr::var("b0"), Expr::u32(WS_LF)),
                            Expr::eq(Expr::var("b0"), Expr::u32(WS_CR)),
                        ),
                    ),
                ),
                Node::let_bind(
                    "ws1",
                    Expr::or(
                        Expr::or(
                            Expr::eq(Expr::var("b1"), Expr::u32(WS_SP)),
                            Expr::eq(Expr::var("b1"), Expr::u32(WS_TAB)),
                        ),
                        Expr::or(
                            Expr::eq(Expr::var("b1"), Expr::u32(WS_LF)),
                            Expr::eq(Expr::var("b1"), Expr::u32(WS_CR)),
                        ),
                    ),
                ),
                Node::let_bind(
                    "ws2",
                    Expr::or(
                        Expr::or(
                            Expr::eq(Expr::var("b2"), Expr::u32(WS_SP)),
                            Expr::eq(Expr::var("b2"), Expr::u32(WS_TAB)),
                        ),
                        Expr::or(
                            Expr::eq(Expr::var("b2"), Expr::u32(WS_LF)),
                            Expr::eq(Expr::var("b2"), Expr::u32(WS_CR)),
                        ),
                    ),
                ),
                Node::let_bind(
                    "ws3",
                    Expr::or(
                        Expr::or(
                            Expr::eq(Expr::var("b3"), Expr::u32(WS_SP)),
                            Expr::eq(Expr::var("b3"), Expr::u32(WS_TAB)),
                        ),
                        Expr::or(
                            Expr::eq(Expr::var("b3"), Expr::u32(WS_LF)),
                            Expr::eq(Expr::var("b3"), Expr::u32(WS_CR)),
                        ),
                    ),
                ),
                // Pack the 4 booleans into one u32 with bits 0..3 set.
                Node::let_bind(
                    "bit0",
                    Expr::select(Expr::var("ws0"), Expr::u32(1), Expr::u32(0)),
                ),
                Node::let_bind(
                    "bit1",
                    Expr::select(Expr::var("ws1"), Expr::u32(2), Expr::u32(0)),
                ),
                Node::let_bind(
                    "bit2",
                    Expr::select(Expr::var("ws2"), Expr::u32(4), Expr::u32(0)),
                ),
                Node::let_bind(
                    "bit3",
                    Expr::select(Expr::var("ws3"), Expr::u32(8), Expr::u32(0)),
                ),
                Node::let_bind(
                    "mask",
                    Expr::bitor(
                        Expr::bitor(Expr::var("bit0"), Expr::var("bit1")),
                        Expr::bitor(Expr::var("bit2"), Expr::var("bit3")),
                    ),
                ),
                Node::store(
                    "whitespace_mask_out",
                    Expr::var("word_idx"),
                    Expr::var("mask"),
                ),
            ],
        ),
    ];

    let buffers = vec![
        BufferDecl::storage(
            "bytes_in",
            BINDING_BYTES_IN,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(word_count),
        BufferDecl::storage(
            "whitespace_mask_out",
            BINDING_WHITESPACE_MASK_OUT,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(word_count),
    ];

    let entry = vec![Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(body),
    }];
    Program::wrapped(buffers, WHITESPACE_CLASSIFY_WORD_WORKGROUP_SIZE, entry)
}

/// Returns true if `byte` is one of {SP, TAB, LF, CR}. Inlined into the
/// Reference oracle and the per-byte fixture builders.
#[must_use]
#[inline]
pub const fn is_structural_whitespace(byte: u8) -> bool {
    matches!(byte, 0x20 | 0x09 | 0x0A | 0x0D)
}

/// Reference oracle. Returns the per-word whitespace bitmaps for the input
/// byte stream (already packed 4 bytes per u32, little-endian). Matches
/// the GPU `Program` lane-for-lane.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_whitespace_classify_word(words_in: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    try_reference_whitespace_classify_word_into(words_in, &mut out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - whitespace word-classifier reference allocation failed");
    out
}

/// Reference oracle into caller-owned output storage.
///
/// Clears `out`, then reuses its capacity.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_whitespace_classify_word_into(words_in: &[u32], out: &mut Vec<u32>) {
    try_reference_whitespace_classify_word_into(words_in, out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - whitespace word-classifier reference allocation failed");
}

/// Fallible reference oracle into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_whitespace_classify_word_into(
    words_in: &[u32],
    out: &mut Vec<u32>,
) -> Result<(), String> {
    if words_in.len() > out.capacity() {
        out.try_reserve_exact(words_in.len() - out.capacity())
            .map_err(|err| {
                format!(
                    "whitespace word-classifier reference could not reserve {} output words: {err}",
                    words_in.len()
                )
            })?;
    }
    out.clear();
    for word in words_in {
        let bytes = [
            (*word & 0xFF) as u8,
            ((*word >> 8) & 0xFF) as u8,
            ((*word >> 16) & 0xFF) as u8,
            ((*word >> 24) & 0xFF) as u8,
        ];
        let mut mask = 0u32;
        for (lane, byte) in bytes.iter().enumerate() {
            if is_structural_whitespace(*byte) {
                mask |= 1u32 << lane;
            }
        }
        out.push(mask);
    }
    Ok(())
}

/// Pack 4 bytes into one little-endian u32 word. Helper for tests +
/// host-side fixture building.
#[must_use]
#[inline]
pub const fn pack_bytes_le(b0: u8, b1: u8, b2: u8, b3: u8) -> u32 {
    (b0 as u32) | ((b1 as u32) << 8) | ((b2 as u32) << 16) | ((b3 as u32) << 24)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || whitespace_classify_word(256),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            let mut words = vec![0x78787878; 256];
            words[0] = 0x20 | (0x09 << 8) | (0x78 << 16) | (0x0A << 24);
            vec![vec![
                to_bytes(&words),                // bytes_in
                to_bytes(&[0; 256]),             // whitespace_mask_out
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            let mut expected = vec![0; 256];
            expected[0] = 11;
            vec![vec![to_bytes(&expected)]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_all_non_whitespace_emits_zero_mask() {
        let words = vec![pack_bytes_le(b'a', b'b', b'c', b'd')];
        assert_eq!(reference_whitespace_classify_word(&words), vec![0]);
    }

    #[test]
    fn classify_into_reuses_output_and_clears_stale_tail() {
        let words = [
            pack_bytes_le(b' ', b'\t', b'x', b'\n'),
            pack_bytes_le(b'a', b'b', b'c', b'd'),
        ];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[u32::MAX; 8]);
        let ptr = out.as_ptr();

        try_reference_whitespace_classify_word_into(&words, &mut out).unwrap();

        assert_eq!(out, vec![0b1011, 0]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn classify_all_whitespace_emits_full_4_bit_mask() {
        let words = vec![pack_bytes_le(b' ', b'\t', b'\n', b'\r')];
        assert_eq!(reference_whitespace_classify_word(&words), vec![0b1111]);
    }

    #[test]
    fn classify_mixed_word_marks_correct_lanes() {
        // a SP b TAB → bits 1 and 3 set → 0b1010
        let words = vec![pack_bytes_le(b'a', b' ', b'b', b'\t')];
        assert_eq!(reference_whitespace_classify_word(&words), vec![0b1010]);
    }

    #[test]
    fn classify_every_per_lane_position_independently() {
        // Set whitespace in just one lane each, verify the bit is in the
        // right position.
        for lane in 0..4u32 {
            let mut bytes = [b'x', b'x', b'x', b'x'];
            bytes[lane as usize] = b' ';
            let word = pack_bytes_le(bytes[0], bytes[1], bytes[2], bytes[3]);
            let result = reference_whitespace_classify_word(&[word]);
            assert_eq!(
                result[0],
                1u32 << lane,
                "lane {lane} whitespace must set bit {lane}"
            );
        }
    }

    #[test]
    fn classify_rejects_close_byte_values_that_are_not_whitespace() {
        // 0x21 (just past SP), 0x08 (just before TAB), 0x0B (between LF and
        // CR), 0x0E (just past CR)  -  all NOT whitespace by the structural
        // parser definition.
        let words = vec![pack_bytes_le(0x21, 0x08, 0x0B, 0x0E)];
        assert_eq!(
            reference_whitespace_classify_word(&words),
            vec![0],
            "values adjacent to but not exactly the whitespace set must NOT classify as ws"
        );
    }

    #[test]
    fn classify_does_not_match_unicode_whitespace() {
        // U+00A0 (non-breaking space), U+2003 (em space)  -  NOT in the
        // structural-parser set. Adjusting that set is a wire-format-visible
        // change and this test enforces the contract.
        let words = vec![pack_bytes_le(0xA0, 0xC2, 0xE2, 0x80)];
        assert_eq!(
            reference_whitespace_classify_word(&words),
            vec![0],
            "structural-parser whitespace is ASCII only by contract"
        );
    }

    #[test]
    fn classify_handles_long_input_byte_for_byte() {
        // Build 64 words of alternating SP / 'x' (one in every other lane
        // is whitespace). Every word should have bits 0 and 2 set.
        let words = vec![pack_bytes_le(b' ', b'x', b' ', b'x'); 64];
        let masks = reference_whitespace_classify_word(&words);
        assert_eq!(masks.len(), 64);
        for mask in &masks {
            assert_eq!(*mask, 0b0101);
        }
    }

    #[test]
    fn classify_generated_4096_byte_corpus_byte_for_byte() {
        let mut bytes = Vec::with_capacity(4096);
        for index in 0..4096u32 {
            let byte = match index % 31 {
                0 => b' ',
                7 => b'\t',
                11 => b'\n',
                19 => b'\r',
                _ => index.wrapping_mul(37).wrapping_add(13) as u8,
            };
            bytes.push(byte);
        }
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|chunk| pack_bytes_le(chunk[0], chunk[1], chunk[2], chunk[3]))
            .collect();
        let masks = reference_whitespace_classify_word(&words);

        assert_eq!(masks.len(), 1024);
        for (word_idx, mask) in masks.iter().copied().enumerate() {
            let mut expected = 0u32;
            for lane in 0..4 {
                let byte = bytes[word_idx * 4 + lane];
                if is_structural_whitespace(byte) {
                    expected |= 1u32 << lane;
                }
            }
            assert_eq!(mask, expected, "word {word_idx}");
        }
    }

    #[test]
    fn classify_empty_input_emits_empty_output() {
        let masks = reference_whitespace_classify_word(&[]);
        assert!(masks.is_empty());
    }

    #[test]
    fn classify_does_not_set_high_bits() {
        // High 28 bits MUST be zero  -  they're reserved for lane widening.
        // Adjusting this is a wire-format-visible change.
        let words = vec![pack_bytes_le(b' ', b' ', b' ', b' ')];
        let masks = reference_whitespace_classify_word(&words);
        assert_eq!(
            masks[0] >> 4,
            0,
            "high 28 bits must remain zero (reserved for lane widening)"
        );
    }

    #[test]
    fn pack_bytes_le_is_canonical() {
        assert_eq!(pack_bytes_le(0x78, 0x56, 0x34, 0x12), 0x1234_5678);
        assert_eq!(pack_bytes_le(0xFF, 0, 0, 0), 0xFF);
        assert_eq!(pack_bytes_le(0, 0xFF, 0, 0), 0xFF00);
    }

    #[test]
    fn classify_into_reuses_output_capacity() {
        let words = [pack_bytes_le(b' ', b'x', b'\n', b'y')];
        let mut out = Vec::with_capacity(32);
        let before = out.capacity();
        reference_whitespace_classify_word_into(&words, &mut out);
        assert_eq!(out, vec![0b0101]);
        assert_eq!(out.capacity(), before);
    }

    #[test]
    fn build_program_returns_well_formed_program() {
        let program = whitespace_classify_word(64);
        assert_eq!(program.buffers().len(), 2, "bytes_in + whitespace_mask_out");
        assert_eq!(
            program.workgroup_size(),
            WHITESPACE_CLASSIFY_WORD_WORKGROUP_SIZE
        );
    }

    #[test]
    fn dispatch_grid_packs_word_lanes_into_blocks() {
        assert_eq!(whitespace_classify_word_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(whitespace_classify_word_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(whitespace_classify_word_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(whitespace_classify_word_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(whitespace_classify_word_dispatch_grid(1025), [5, 1, 1]);
    }

    #[test]
    fn build_program_is_deterministic_across_calls() {
        let p1 = whitespace_classify_word(128);
        let p2 = whitespace_classify_word(128);
        assert_eq!(p1.buffers().len(), p2.buffers().len());
        assert_eq!(p1.workgroup_size(), p2.workgroup_size());
    }

    #[test]
    fn op_id_is_canonical_and_stable() {
        assert_eq!(OP_ID, "vyre-primitives::parsing::whitespace_classify_word");
    }

    #[test]
    fn binding_indices_are_canonical_and_stable() {
        assert_eq!(BINDING_BYTES_IN, 0);
        assert_eq!(BINDING_WHITESPACE_MASK_OUT, 1);
    }

    #[test]
    fn is_structural_whitespace_matches_only_the_canonical_four() {
        for byte in 0u8..=255 {
            let expected = matches!(byte, 0x20 | 0x09 | 0x0A | 0x0D);
            assert_eq!(
                is_structural_whitespace(byte),
                expected,
                "byte 0x{byte:02X} structural-ws classification"
            );
        }
    }
}

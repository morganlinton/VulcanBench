//! Advanced bitset transform pipeline over `vyre-primitives::bitset`.
//!
//! These wrappers cover the in-place and tiled bitset primitives used when
//! self-substrate passes keep masks resident: frontier subtraction, monotone
//! accumulator growth, symmetric-difference change detection, select navigation,
//! and Method-of-Four-Russians byte-tile application.

use vyre_foundation::ir::Program;
use vyre_primitives::bitset::{
    and_into::bitset_and_into,
    and_not::bitset_and_not,
    and_not_into::bitset_and_not_into,
    any::bitset_any,
    copy::bitset_copy,
    four_russians::{
        binary_byte_lut, cached_binary_byte_lut, dense_matvec_byte_lut,
        dense_matvec_byte_lut_words, four_russians_apply_byte_lut,
        four_russians_dense_matvec_byte_lut, frontier_words_for_byte_tiles, BooleanTileOp,
    },
    or_into::bitset_or_into,
    select::select1_query,
    xor_into::bitset_xor_into,
};

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::bitset::{
    and_into::cpu_ref as primitive_and_into,
    and_not::cpu_ref as primitive_and_not,
    and_not_into::cpu_ref as primitive_and_not_into,
    any::cpu_ref as primitive_any,
    copy::cpu_ref as primitive_copy,
    four_russians::{
        cpu_ref as primitive_four_russians, dense_matvec_cpu_ref as primitive_dense_matvec,
    },
    or_into::cpu_ref as primitive_or_into,
    xor_into::cpu_ref as primitive_xor_into,
};

/// Build `out = lhs & !rhs` for subtracting an exclusion mask.
#[must_use]
pub fn subtract_mask_program(lhs: &str, rhs: &str, out: &str, words: u32) -> Program {
    bitset_and_not(lhs, rhs, out, words)
}

/// Build `target &= mask` for in-place frontier narrowing.
#[must_use]
pub fn narrow_mask_in_place_program(target: &str, mask: &str, words: u32) -> Program {
    bitset_and_into(target, mask, words)
}

/// Build `target |= addend` for monotone accumulator growth.
#[must_use]
pub fn grow_mask_in_place_program(target: &str, addend: &str, words: u32) -> Program {
    bitset_or_into(target, addend, words)
}

/// Build `target ^= addend` for symmetric-difference change detection.
#[must_use]
pub fn diff_mask_in_place_program(target: &str, addend: &str, words: u32) -> Program {
    bitset_xor_into(target, addend, words)
}

/// Build `target &= !subtrahend` for in-place set subtraction.
#[must_use]
pub fn subtract_mask_in_place_program(target: &str, subtrahend: &str, words: u32) -> Program {
    bitset_and_not_into(target, subtrahend, words)
}

/// Build `target = source` for same-shape bitset transfer.
#[must_use]
pub fn copy_mask_program(target: &str, source: &str, words: u32) -> Program {
    bitset_copy(target, source, words)
}

/// Build `out[0] = any(input != 0)` for fast sparse-frontier tests.
#[must_use]
pub fn any_mask_program(input: &str, out: &str, words: u32) -> Program {
    bitset_any(input, out, words)
}

/// Build select1 navigation over a packed bitvector.
#[must_use]
pub fn select1_navigation_program(
    bits: &str,
    k_indices: &str,
    out: &str,
    word_count: u32,
    query_count: u32,
) -> Program {
    select1_query(bits, k_indices, out, word_count, query_count)
}

/// Build a Method-of-Four-Russians byte-tile LUT.
#[must_use]
pub fn boolean_tile_lut(op: BooleanTileOp) -> Vec<u32> {
    binary_byte_lut(op)
}

/// Reuse a cached Method-of-Four-Russians byte-tile LUT.
#[must_use]
pub fn cached_boolean_tile_lut(op: BooleanTileOp) -> &'static [u32] {
    cached_binary_byte_lut(op)
}

/// Frontier words needed for dense byte-tile Four-Russians matvec.
#[must_use]
pub const fn dense_matvec_frontier_words(tile_count: u32) -> u32 {
    frontier_words_for_byte_tiles(tile_count)
}

/// LUT words needed for dense byte-tile Four-Russians matvec.
#[must_use]
pub fn dense_matvec_lut_words(tile_count: u32, dst_words: u32) -> u32 {
    dense_matvec_byte_lut_words(tile_count, dst_words)
}

/// Build a dense byte-tile Four-Russians matvec LUT from source columns.
#[must_use]
pub fn dense_boolean_matvec_lut(columns: &[u32], tile_count: u32, dst_words: u32) -> Vec<u32> {
    dense_matvec_byte_lut(columns, tile_count, dst_words)
}

/// Build the GPU byte-tile lookup program.
#[must_use]
pub fn four_russians_transform_program(
    lhs: &str,
    rhs: &str,
    lut: &str,
    out: &str,
    words: u32,
) -> Program {
    four_russians_apply_byte_lut(lhs, rhs, lut, out, words)
}

/// Build dense boolean matvec over packed frontier byte tiles.
#[must_use]
pub fn four_russians_dense_matvec_program(
    frontier: &str,
    tile_lut: &str,
    out: &str,
    tile_count: u32,
    dst_words: u32,
) -> Program {
    four_russians_dense_matvec_byte_lut(frontier, tile_lut, out, tile_count, dst_words)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_subtract_mask(lhs: &[u32], rhs: &[u32]) -> Vec<u32> {
    primitive_and_not(lhs, rhs)
}

#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_narrow_mask_in_place(target: &mut [u32], mask: &[u32]) {
    primitive_and_into(target, mask);
}

#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_grow_mask_in_place(target: &mut [u32], addend: &[u32]) {
    primitive_or_into(target, addend);
}

#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_diff_mask_in_place(target: &mut [u32], addend: &[u32]) {
    primitive_xor_into(target, addend);
}

#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_subtract_mask_in_place(target: &mut [u32], subtrahend: &[u32]) {
    primitive_and_not_into(target, subtrahend);
}

#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_copy_mask(target: &mut [u32], source: &[u32]) {
    primitive_copy(target, source);
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_any_mask(input: &[u32]) -> bool {
    primitive_any(input) != 0
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_four_russians_transform(lhs: &[u32], rhs: &[u32], lut: &[u32]) -> Vec<u32> {
    primitive_four_russians(lhs, rhs, lut)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_dense_boolean_matvec(
    frontier: &[u32],
    tile_lut: &[u32],
    tile_count: u32,
    dst_words: u32,
) -> Vec<u32> {
    primitive_dense_matvec(frontier, tile_lut, tile_count, dst_words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_builders_emit_expected_bitset_primitives() {
        let cases = [
            (
                subtract_mask_program("lhs", "rhs", "out", 2),
                vyre_primitives::bitset::and_not::OP_ID,
            ),
            (
                narrow_mask_in_place_program("target", "mask", 2),
                vyre_primitives::bitset::and_into::OP_ID,
            ),
            (
                grow_mask_in_place_program("target", "addend", 2),
                vyre_primitives::bitset::or_into::OP_ID,
            ),
            (
                diff_mask_in_place_program("target", "addend", 2),
                vyre_primitives::bitset::xor_into::OP_ID,
            ),
            (
                subtract_mask_in_place_program("target", "sub", 2),
                vyre_primitives::bitset::and_not_into::OP_ID,
            ),
            (
                copy_mask_program("target", "source", 2),
                vyre_primitives::bitset::copy::OP_ID,
            ),
            (
                any_mask_program("input", "out", 2),
                vyre_primitives::bitset::any::OP_ID,
            ),
            (
                select1_navigation_program("bits", "queries", "out", 2, 2),
                vyre_primitives::bitset::select::OP_ID,
            ),
            (
                four_russians_transform_program("lhs", "rhs", "lut", "out", 2),
                vyre_primitives::bitset::four_russians::OP_ID,
            ),
            (
                four_russians_dense_matvec_program("frontier", "tile_lut", "out", 2, 2),
                vyre_primitives::bitset::four_russians::DENSE_MATVEC_OP_ID,
            ),
        ];

        for (program, expected) in cases {
            let actual = program
                .entry
                .iter()
                .find_map(|node| match node {
                    vyre_foundation::ir::Node::Region { generator, .. } => Some(generator.as_str()),
                    _ => None,
                })
                .expect("Fix: primitive program should have a region generator");
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn cpu_reference_wrappers_match_primitive_contracts() {
        assert_eq!(
            reference_subtract_mask(&[0xFFFF, 0xAAAA_AAAA], &[0x00FF, 0x5555_5555]),
            vec![0xFF00, 0xAAAA_AAAA]
        );

        let mut target = vec![0xFFFFu32, 0x0F0F];
        reference_narrow_mask_in_place(&mut target, &[0xFF00, 0xFFFF]);
        assert_eq!(target, vec![0xFF00, 0x0F0F]);
        reference_grow_mask_in_place(&mut target, &[0x00FF, 0xF0F0]);
        assert_eq!(target, vec![0xFFFF, 0xFFFF]);
        reference_diff_mask_in_place(&mut target, &[0x0F0F, 0x00FF]);
        assert_eq!(target, vec![0xF0F0, 0xFF00]);
        reference_subtract_mask_in_place(&mut target, &[0xF000, 0x0F00]);
        assert_eq!(target, vec![0x00F0, 0xF000]);

        let mut copied = vec![0u32; 2];
        reference_copy_mask(&mut copied, &target);
        assert_eq!(copied, target);
        assert!(reference_any_mask(&copied));
    }

    #[test]
    fn four_russians_lut_and_cache_match_transform_contract() {
        let lut = boolean_tile_lut(BooleanTileOp::AndNot);
        assert_eq!(
            lut.as_slice(),
            cached_boolean_tile_lut(BooleanTileOp::AndNot)
        );
        let lhs = [0xFF00_FF00u32];
        let rhs = [0xF0F0_F0F0u32];
        assert_eq!(
            reference_four_russians_transform(&lhs, &rhs, &lut),
            vec![0x0F00_0F00]
        );
    }

    #[test]
    fn dense_four_russians_matvec_lut_matches_transform_contract() {
        let columns = [
            0b0001_u32, 0b0010, 0b0100, 0b1000, 0b0001, 0b0010, 0b0100, 0b1000,
        ];
        let lut = dense_boolean_matvec_lut(&columns, 1, 1);

        assert_eq!(dense_matvec_frontier_words(1), 1);
        assert_eq!(dense_matvec_lut_words(1, 1), 256);
        assert_eq!(
            reference_dense_boolean_matvec(&[0b0000_0101], &lut, 1, 1),
            vec![0b0101]
        );
    }
}

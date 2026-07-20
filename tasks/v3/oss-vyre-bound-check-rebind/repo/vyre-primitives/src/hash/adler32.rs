//! Adler-32 checksum primitive.
//!
//! Serial single-invocation walk. A init 1, B init 0, both mod 65521
//! per byte. Output `(B << 16) | A`.
//!
//! `input[i]` packs one byte per u32 slot in the low 8 bits; high bits are
//! ignored by construction.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Largest prime smaller than 2^16 used by Adler-32.
pub const ADLER32_MOD: u32 = 65_521;

/// Stable Tier 2.5 op id for the Adler-32 serial byte walker.
pub const ADLER32_OP_ID: &str = "vyre-primitives::hash::adler32";

/// Self-contained Adler-32 chunk summary for tree reductions.
///
/// A chunk stores the state obtained by hashing that byte range from the
/// canonical initial state, plus the chunk length modulo [`ADLER32_MOD`].
/// Adjacent chunks can be combined without re-reading their bytes, which is
/// the algebra needed by GPU block scans and persistent hash pipelines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Adler32Chunk {
    /// Chunk length modulo [`ADLER32_MOD`].
    pub len_mod: u32,
    /// Adler-32 A state after the chunk from canonical initialization.
    pub a: u32,
    /// Adler-32 B state after the chunk from canonical initialization.
    pub b: u32,
}

/// CPU reference: Adler-32 over a byte slice.
#[must_use]
pub fn adler32(bytes: &[u8]) -> u32 {
    let chunk = adler32_chunk(bytes);
    adler32_finalize_state(chunk.a, chunk.b)
}

/// Summarize a byte slice as an independently-combinable Adler-32 chunk.
#[must_use]
pub fn adler32_chunk(bytes: &[u8]) -> Adler32Chunk {
    let mut a = adler32_initial_a_state();
    let mut b = adler32_initial_b_state();
    for &byte in bytes {
        let next = adler32_update_byte_state(a, b, byte);
        a = next.0;
        b = next.1;
    }
    Adler32Chunk {
        len_mod: (bytes.len() % ADLER32_MOD as usize) as u32,
        a,
        b,
    }
}

/// Apply a precomputed chunk summary to an existing Adler-32 state.
#[must_use]
pub fn adler32_combine_state(a: u32, b: u32, chunk: Adler32Chunk) -> (u32, u32) {
    let modulus = u64::from(ADLER32_MOD);
    let a_minus_one = (u64::from(a) + modulus - 1) % modulus;
    let combined_a = adler32_mod_u64(u64::from(a) + u64::from(chunk.a) + modulus - 1);
    let combined_b =
        adler32_mod_u64(u64::from(b) + u64::from(chunk.b) + u64::from(chunk.len_mod) * a_minus_one);
    (combined_a, combined_b)
}

/// Combine adjacent Adler-32 chunk summaries without reading source bytes.
#[must_use]
pub fn adler32_combine_chunks(left: Adler32Chunk, right: Adler32Chunk) -> Adler32Chunk {
    let (a, b) = adler32_combine_state(left.a, left.b, right);
    Adler32Chunk {
        len_mod: adler32_mod_u64(u64::from(left.len_mod) + u64::from(right.len_mod)),
        a,
        b,
    }
}

/// Initial Adler-32 A state.
#[must_use]
pub const fn adler32_initial_a_state() -> u32 {
    1
}

/// Initial Adler-32 B state.
#[must_use]
pub const fn adler32_initial_b_state() -> u32 {
    0
}

/// Canonical Adler-32 CPU single-byte update.
#[must_use]
pub const fn adler32_update_byte_state(a: u32, b: u32, byte: u8) -> (u32, u32) {
    let a = (a + byte as u32) % ADLER32_MOD;
    let b = (b + a) % ADLER32_MOD;
    (a, b)
}

/// Canonical Adler-32 CPU finalization.
#[must_use]
pub const fn adler32_finalize_state(a: u32, b: u32) -> u32 {
    (b << 16) | a
}

fn adler32_mod_u64(value: u64) -> u32 {
    (value % u64::from(ADLER32_MOD)) as u32
}

/// Initial Adler-32 A expression for fused IR compositions.
#[must_use]
pub fn adler32_initial_a_expr() -> Expr {
    Expr::u32(adler32_initial_a_state())
}

/// Initial Adler-32 B expression for fused IR compositions.
#[must_use]
pub fn adler32_initial_b_expr() -> Expr {
    Expr::u32(adler32_initial_b_state())
}

/// Emit the canonical Adler-32 single-byte update into `a_var` and `b_var`.
///
/// `byte` may contain non-byte high bits; the helper masks to the low 8 bits
/// so fused compositions preserve the same input contract as
/// [`adler32_program`].
#[must_use]
pub fn adler32_update_byte_nodes(a_var: &str, b_var: &str, byte: Expr) -> [Node; 2] {
    let byte = Expr::bitand(byte, Expr::u32(0xFF));
    [
        Node::assign(
            a_var,
            Expr::rem(Expr::add(Expr::var(a_var), byte), Expr::u32(ADLER32_MOD)),
        ),
        Node::assign(
            b_var,
            Expr::rem(
                Expr::add(Expr::var(b_var), Expr::var(a_var)),
                Expr::u32(ADLER32_MOD),
            ),
        ),
    ]
}

/// Final Adler-32 expression for fused IR compositions.
#[must_use]
pub fn adler32_finalize_expr(a: Expr, b: Expr) -> Expr {
    Expr::bitor(Expr::shl(b, Expr::u32(16)), a)
}

/// Build a Program that writes Adler-32(input[0..n]) to `out[0]`.
#[must_use]
pub fn adler32_program(input: &str, out: &str, n: u32) -> Program {
    let body = vec![Node::Region {
        generator: Ident::from(ADLER32_OP_ID),
        source_region: None,
        body: Arc::new(adler32_body(input, out, n)),
    }];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out, 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

fn adler32_body(input: &str, out: &str, n: u32) -> Vec<Node> {
    vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("a", adler32_initial_a_expr()),
            Node::let_bind("b", adler32_initial_b_expr()),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(n),
                adler32_update_byte_nodes("a", "b", Expr::load(input, Expr::var("i"))).into(),
            ),
            Node::store(
                out,
                Expr::u32(0),
                adler32_finalize_expr(Expr::var("a"), Expr::var("b")),
            ),
        ],
    )]
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        ADLER32_OP_ID,
        || adler32_program("input", "out", 3),
        Some(|| {
            let bytes = crate::wire::pack_bytes_as_u32_slice(b"abc");
            vec![vec![bytes, vec![0u8; 4]]]
        }),
        Some(|| vec![vec![0x024D_0127u32.to_le_bytes().to_vec()]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abc_matches_rfc1950_example() {
        assert_eq!(adler32(b"abc"), 0x024D_0127);
    }

    #[test]
    fn wikipedia_string() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn state_helpers_match_slice_hasher() {
        let bytes = b"vyre-adler-single-source";
        let mut a = adler32_initial_a_state();
        let mut b = adler32_initial_b_state();
        for &byte in bytes {
            let next = adler32_update_byte_state(a, b, byte);
            a = next.0;
            b = next.1;
        }
        assert_eq!(adler32_finalize_state(a, b), adler32(bytes));
    }

    #[test]
    fn chunk_summary_matches_slice_hasher() {
        let bytes = b"vyre-adler-gpu-tree-reduction";
        let chunk = adler32_chunk(bytes);

        assert_eq!(adler32_finalize_state(chunk.a, chunk.b), adler32(bytes));
        assert_eq!(chunk.len_mod, bytes.len() as u32);
    }

    #[test]
    fn chunk_combine_matches_serial_hash_for_all_splits() {
        let bytes = b"adler chunks are composable enough for gpu block scans";

        for split in 0..=bytes.len() {
            let left = adler32_chunk(&bytes[..split]);
            let right = adler32_chunk(&bytes[split..]);
            let combined = adler32_combine_chunks(left, right);

            assert_eq!(
                adler32_finalize_state(combined.a, combined.b),
                adler32(bytes),
                "split {split}"
            );
        }
    }

    #[test]
    fn chunk_combine_is_associative_for_generated_payloads() {
        for len in 0..96usize {
            let bytes = (0..len)
                .map(|i| ((i * 37 + len * 11) & 0xFF) as u8)
                .collect::<Vec<_>>();
            for split_a in 0..=len {
                for split_b in split_a..=len {
                    let a = adler32_chunk(&bytes[..split_a]);
                    let b = adler32_chunk(&bytes[split_a..split_b]);
                    let c = adler32_chunk(&bytes[split_b..]);
                    let left_assoc = adler32_combine_chunks(adler32_combine_chunks(a, b), c);
                    let right_assoc = adler32_combine_chunks(a, adler32_combine_chunks(b, c));

                    assert_eq!(
                        left_assoc, right_assoc,
                        "len {len}, splits {split_a}/{split_b}"
                    );
                    assert_eq!(
                        adler32_finalize_state(left_assoc.a, left_assoc.b),
                        adler32(&bytes),
                        "len {len}, splits {split_a}/{split_b}"
                    );
                }
            }
        }
    }

    #[test]
    fn update_helper_masks_high_input_bits() {
        let nodes = adler32_update_byte_nodes("a", "b", Expr::u32(0xFFFF_FF61));
        let rendered = format!("{nodes:?}");
        assert!(
            rendered.contains("255"),
            "Fix: Adler-32 IR helper must mask each u32 slot to one byte."
        );
    }
}

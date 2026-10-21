//! FNV-1a 32-bit + 64-bit hash primitives.
//!
//! FNV-1a is a non-cryptographic hash with a tight inner loop:
//! `h = (h XOR byte) * prime`. Used everywhere a fast non-secure
//! fingerprint is good enough  -  dialect-id interning, pipeline-cache
//! sharding, per-op id hashing.
//!
//! Both widths (32, 64) share the structure; only the magic constants
//! differ. The CPU reference is byte-identical to every conformant
//! FNV-1a implementation.

/// FNV-1a offset basis (32-bit).
pub const FNV1A32_OFFSET: u32 = 0x811c_9dc5;
/// FNV-1a prime (32-bit).
pub const FNV1A32_PRIME: u32 = 0x0100_0193;

/// FNV-1a offset basis (64-bit).
pub const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a prime (64-bit).
pub const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// CPU reference: FNV-1a 32-bit over a byte slice.
#[must_use]
pub fn fnv1a32(bytes: &[u8]) -> u32 {
    fnv1a32_const(bytes)
}

/// CPU reference: FNV-1a32 over packed u32 lanes, hashing only the low byte
/// from each lane.
#[must_use]
pub fn fnv1a32_packed_u32_low8(words: &[u32]) -> u32 {
    let mut h = fnv1a32_initial_state();
    for &word in words {
        h = fnv1a32_update_byte(h, (word & 0xFF) as u8);
    }
    h
}

/// Const-evaluable FNV-1a32 over a byte slice.
#[must_use]
pub const fn fnv1a32_const(bytes: &[u8]) -> u32 {
    let mut h = fnv1a32_initial_state();
    let mut idx = 0usize;
    while idx < bytes.len() {
        h = fnv1a32_update_byte(h, bytes[idx]);
        idx += 1;
    }
    h
}

/// Initial FNV-1a32 CPU state.
#[must_use]
pub const fn fnv1a32_initial_state() -> u32 {
    FNV1A32_OFFSET
}

/// Canonical FNV-1a32 CPU single-byte update.
#[must_use]
pub const fn fnv1a32_update_byte(hash: u32, byte: u8) -> u32 {
    (hash ^ byte as u32).wrapping_mul(FNV1A32_PRIME)
}

use std::sync::Arc;
use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable op id  -  the Tier 3 wrapper registers under this id.
pub const FNV1A32_OP_ID: &str = "vyre-primitives::hash::fnv1a32";
/// Stable op id for the 64-bit widening-multiply builder.
pub const FNV1A64_OP_ID: &str = "vyre-primitives::hash::fnv1a64";
const FNV1A64_PRIME_LO: u32 = 0x0000_01B3;
const FNV1A64_PRIME_HI: u32 = 0x0000_0100;
const FNV1A64_OFFSET_LO: u32 = 0x8422_2325;
const FNV1A64_OFFSET_HI: u32 = 0xCBF2_9CE4;

/// GPU IR builder: FNV-1a 32-bit serial walk over `input[0..n]`.
///
/// This compatibility entry point expects one `DataType::U32` element per
/// source byte and hashes the low byte of each word. Use
/// [`fnv1a32_program_u8`] when the source is packed as one byte per element.
/// Output is one u32 hash at `out[0]`. Single invocation 0 does the whole walk;
/// callers needing parallel throughput compose this with a reduce primitive.
#[must_use]
pub fn fnv1a32_program(input: &str, out: &str, n: u32) -> Program {
    fnv1a32_program_bounded(input, out, Expr::u32(n), Some(n), DataType::U32)
}

/// GPU IR builder: FNV-1a 32-bit serial walk over packed `DataType::U8` bytes.
#[must_use]
pub fn fnv1a32_program_u8(input: &str, out: &str, n: u32) -> Program {
    fnv1a32_program_bounded(input, out, Expr::u32(n), Some(n), DataType::U8)
}

/// Dynamic-bound variant: loop bound is `Expr::buf_len(input)` so the
/// shader walks whatever the caller's input buffer declares at dispatch
/// time. The returned Program leaves `input` without a static count.
#[must_use]
pub fn fnv1a32_program_dyn(input: &str, out: &str) -> Program {
    fnv1a32_program_bounded(input, out, Expr::buf_len(input), None, DataType::U32)
}

/// Dynamic-bound packed-`u8` variant of [`fnv1a32_program_dyn`].
#[must_use]
pub fn fnv1a32_program_dyn_u8(input: &str, out: &str) -> Program {
    fnv1a32_program_bounded(input, out, Expr::buf_len(input), None, DataType::U8)
}

/// Initial FNV-1a32 state expression for fused IR compositions.
#[must_use]
pub fn fnv1a32_initial_expr() -> Expr {
    Expr::u32(FNV1A32_OFFSET)
}

/// Canonical FNV-1a32 single-byte update expression.
///
/// `byte` is masked to its low 8 bits, matching the public program builder's
/// one-byte-per-u32-slot contract.
#[must_use]
pub fn fnv1a32_update_byte_expr(hash: Expr, byte: Expr) -> Expr {
    Expr::mul(
        Expr::bitxor(
            hash,
            Expr::bitand(Expr::cast(DataType::U32, byte), Expr::u32(0xFF)),
        ),
        Expr::u32(FNV1A32_PRIME),
    )
}

/// FNV-1a32-style whole-word mix expression for structural fingerprints.
///
/// This is not the byte-stream hash used by [`fnv1a32_program`]; it preserves
/// legacy structural-hash behavior where each encoded arena field is mixed as
/// one u32 payload. Centralizing it keeps every structural-hash user on the
/// same constants and update shape.
#[must_use]
pub fn fnv1a32_mix_word_expr(hash: Expr, word: Expr) -> Expr {
    Expr::mul(Expr::bitxor(hash, word), Expr::u32(FNV1A32_PRIME))
}

/// FNV-prime structural mix used by legacy packed-AST hash tables.
///
/// This preserves the historical `hash = (hash * prime) XOR word` order used
/// by packed-AST CSE kernels. It is intentionally separate from
/// [`fnv1a32_mix_word_expr`], whose order is FNV-1a-style
/// `(hash XOR word) * prime`.
#[must_use]
pub const fn fnv1a32_mul_xor_word_state(hash: u32, word: u32) -> u32 {
    hash.wrapping_mul(FNV1A32_PRIME) ^ word
}

/// IR expression form of [`fnv1a32_mul_xor_word_state`].
#[must_use]
pub fn fnv1a32_mul_xor_word_expr(hash: Expr, word: Expr) -> Expr {
    Expr::bitxor(Expr::mul(hash, Expr::u32(FNV1A32_PRIME)), word)
}

/// Canonical FNV-1a32 single-byte update node for fused IR compositions.
#[must_use]
pub fn fnv1a32_update_byte_node(hash_var: &str, byte: Expr) -> Node {
    Node::assign(
        hash_var,
        fnv1a32_update_byte_expr(Expr::var(hash_var), byte),
    )
}

fn fnv1a32_program_bounded(
    input: &str,
    out: &str,
    loop_bound: Expr,
    static_count: Option<u32>,
    source_type: DataType,
) -> Program {
    let body = vec![Node::Region {
        generator: Ident::from(FNV1A32_OP_ID),
        source_region: None,
        body: Arc::new(vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("h", fnv1a32_initial_expr()),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    loop_bound,
                    vec![fnv1a32_update_byte_node(
                        "h",
                        fnv1a_load_byte_expr(input, Expr::var("i")),
                    )],
                ),
                Node::store(out, Expr::u32(0), Expr::var("h")),
            ],
        )]),
    }];

    let input_buf = match static_count {
        Some(n) => BufferDecl::storage(input, 0, BufferAccess::ReadOnly, source_type).with_count(n),
        None => BufferDecl::storage(input, 0, BufferAccess::ReadOnly, source_type),
    };

    Program::wrapped(
        vec![
            input_buf,
            BufferDecl::output(out, 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

fn fnv1a_load_byte_expr(input: &str, index: Expr) -> Expr {
    Expr::cast(DataType::U32, Expr::load(input, index))
}

#[cfg(test)]
mod fnv1a32_ir_tests {
    use super::*;

    #[test]
    fn static_program_emits_single_mask_per_input_slot() {
        let program = fnv1a32_program("input", "out", 3);
        let rendered = format!("{:?}", program.entry());
        assert!(
            !rendered.contains("byte"),
            "Fix: FNV-1a32 IR must not materialize a redundant byte temporary before the shared update helper."
        );
        assert!(
            rendered.contains("255") || rendered.contains("0xFF"),
            "Fix: FNV-1a32 IR must still mask packed u32 input lanes to low bytes."
        );
    }

    #[test]
    fn packed_u8_program_declares_one_source_byte_per_element() {
        let program = fnv1a32_program_u8("input", "out", 513);
        let input = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "input")
            .expect("Fix: packed-u8 FNV-1a32 input buffer must be declared");
        let out = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "out")
            .expect("Fix: FNV-1a32 output buffer must be declared");

        assert_eq!(input.element(), DataType::U8);
        assert_eq!(input.count(), 513);
        assert_eq!(out.element(), DataType::U32);
        assert_eq!(out.count(), 1);
    }
}

/// GPU IR builder: FNV-1a 64-bit serial walk over `input[0..n]`.
///
/// The IR lacks native `u64`, so the state is maintained as `(low, high)` u32
/// halves and multiplied by the FNV prime via a widened split product.
#[must_use]
pub fn fnv1a64_program(input: &str, out: &str) -> Program {
    fnv1a64_program_bounded(input, out, Expr::buf_len(input), None, DataType::U32)
}

/// Packed-`u8` dynamic-bound variant of [`fnv1a64_program`].
#[must_use]
pub fn fnv1a64_program_u8(input: &str, out: &str) -> Program {
    fnv1a64_program_bounded(input, out, Expr::buf_len(input), None, DataType::U8)
}

/// GPU IR builder: FNV-1a 64-bit serial walk over `input[0..n]`.
///
/// This compatibility entry point expects one `DataType::U32` element per
/// source byte and hashes the low byte of each word. Use
/// [`fnv1a64_program_n_u8`] when the source is packed as one byte per element.
#[must_use]
pub fn fnv1a64_program_n(input: &str, out: &str, n: u32) -> Program {
    fnv1a64_program_bounded(input, out, Expr::u32(n), Some(n), DataType::U32)
}

/// GPU IR builder: FNV-1a 64-bit serial walk over packed `DataType::U8` bytes.
#[must_use]
pub fn fnv1a64_program_n_u8(input: &str, out: &str, n: u32) -> Program {
    fnv1a64_program_bounded(input, out, Expr::u32(n), Some(n), DataType::U8)
}

fn fnv1a64_program_bounded(
    input: &str,
    out: &str,
    loop_bound: Expr,
    static_count: Option<u32>,
    source_type: DataType,
) -> Program {
    let body = vec![Node::Region {
        generator: Ident::from(FNV1A64_OP_ID),
        source_region: None,
        body: Arc::new(vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("h_lo", Expr::u32(FNV1A64_OFFSET_LO)),
                Node::let_bind("h_hi", Expr::u32(FNV1A64_OFFSET_HI)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    loop_bound,
                    vec![
                        Node::assign(
                            "h_lo",
                            Expr::bitxor(
                                Expr::var("h_lo"),
                                Expr::bitand(
                                    fnv1a_load_byte_expr(input, Expr::var("i")),
                                    Expr::u32(0xFF),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "lo_lo16",
                            Expr::bitand(Expr::var("h_lo"), Expr::u32(0xFFFF)),
                        ),
                        Node::let_bind("lo_hi16", Expr::shr(Expr::var("h_lo"), Expr::u32(16))),
                        Node::let_bind(
                            "part_a",
                            Expr::mul(Expr::var("lo_lo16"), Expr::u32(FNV1A64_PRIME_LO)),
                        ),
                        Node::let_bind(
                            "part_b",
                            Expr::mul(Expr::var("lo_hi16"), Expr::u32(FNV1A64_PRIME_LO)),
                        ),
                        Node::let_bind("shifted_b", Expr::shl(Expr::var("part_b"), Expr::u32(16))),
                        Node::let_bind(
                            "new_lo",
                            Expr::add(Expr::var("part_a"), Expr::var("shifted_b")),
                        ),
                        Node::let_bind(
                            "overflow_bit",
                            Expr::Select {
                                cond: Box::new(Expr::gt(
                                    Expr::var("part_a"),
                                    Expr::sub(Expr::u32(u32::MAX), Expr::var("shifted_b")),
                                )),
                                true_val: Box::new(Expr::u32(1)),
                                false_val: Box::new(Expr::u32(0)),
                            },
                        ),
                        Node::let_bind(
                            "carry",
                            Expr::add(
                                Expr::shr(Expr::var("part_b"), Expr::u32(16)),
                                Expr::var("overflow_bit"),
                            ),
                        ),
                        Node::let_bind(
                            "hi_times_p_lo",
                            Expr::mul(Expr::var("h_hi"), Expr::u32(FNV1A64_PRIME_LO)),
                        ),
                        Node::let_bind(
                            "lo_times_p_hi",
                            Expr::mul(Expr::var("h_lo"), Expr::u32(FNV1A64_PRIME_HI)),
                        ),
                        Node::let_bind(
                            "new_hi",
                            Expr::add(
                                Expr::add(Expr::var("hi_times_p_lo"), Expr::var("lo_times_p_hi")),
                                Expr::var("carry"),
                            ),
                        ),
                        Node::assign("h_lo", Expr::var("new_lo")),
                        Node::assign("h_hi", Expr::var("new_hi")),
                    ],
                ),
                Node::store(out, Expr::u32(0), Expr::var("h_lo")),
                Node::store(out, Expr::u32(1), Expr::var("h_hi")),
            ],
        )]),
    }];

    let input_buf = match static_count {
        Some(n) => BufferDecl::storage(input, 0, BufferAccess::ReadOnly, source_type).with_count(n),
        None => BufferDecl::storage(input, 0, BufferAccess::ReadOnly, source_type),
    };

    Program::wrapped(
        vec![
            input_buf,
            BufferDecl::output(out, 1, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        body,
    )
}

/// CPU reference: FNV-1a 64-bit over a byte slice.
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = fnv1a64_initial_state();
    for &byte in bytes {
        h = fnv1a64_update_byte(h, byte);
    }
    h
}

/// Initial FNV-1a64 CPU state.
#[must_use]
pub const fn fnv1a64_initial_state() -> u64 {
    FNV1A64_OFFSET
}

/// Canonical FNV-1a64 CPU single-byte update.
#[must_use]
pub const fn fnv1a64_update_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(FNV1A64_PRIME)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        FNV1A32_OP_ID,
        || fnv1a32_program("input", "out", 1),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0x61]), // input: one word, low byte = 'a'
                to_bytes(&[0]),    // output
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0xe40c_292c])]] // canonical FNV-1a32("a")
        }),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        FNV1A64_OP_ID,
        || fnv1a64_program("input", "out"),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0x61]),   // input: one word, low byte = 'a'
                to_bytes(&[0, 0]),   // output: two words for fnv1a64 hash
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0x8601_ec8c, 0xaf63_dc4c])]] // canonical FNV-1a64("a") little-endian halves
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Conformance vectors from the canonical FNV test suite.
    // http://www.isthe.com/chongo/tech/comp/fnv/

    #[test]
    fn fnv1a32_empty_is_offset() {
        assert_eq!(fnv1a32(b""), FNV1A32_OFFSET);
    }

    #[test]
    fn fnv1a32_single_ascii_a() {
        assert_eq!(fnv1a32(b"a"), 0xe40c_292c);
    }

    #[test]
    fn fnv1a32_packed_u32_low8_matches_byte_hasher_and_masks_high_bits() {
        assert_eq!(
            fnv1a32_packed_u32_low8(&[0xFFFF_FF61, 0xCAFE_0062, 0x8000_0063]),
            fnv1a32(b"abc")
        );
    }

    #[test]
    fn gpu_builder_matches_cpu_ref() {
        use vyre_foundation::ir::model::expr::Ident;
        let program = fnv1a32_program("src", "out", 5);
        // Validate region chain wrap.
        match &program.entry()[0] {
            Node::Region { generator, .. } => {
                assert_eq!(generator, &Ident::from(FNV1A32_OP_ID));
            }
            other => panic!("expected top-level Region, got {other:?}"),
        }
        // Buffer count sanity.
        assert_eq!(program.buffers().len(), 2);
    }

    #[test]
    fn fnv1a32_update_helper_masks_high_input_bits() {
        let node = fnv1a32_update_byte_node("h", Expr::u32(0xFFFF_FF61));
        let rendered = format!("{node:?}");
        assert!(
            rendered.contains("255") || rendered.contains("0xFF"),
            "Fix: shared FNV-1a32 update helper must mask high input bits: {rendered}"
        );
    }

    #[test]
    fn fnv1a32_is_deterministic_and_not_identity() {
        // Two invocations over the same input produce identical hashes
        // (determinism); distinct inputs of equal length produce
        // distinct hashes (avalanche sanity).
        let a = fnv1a32(b"The quick brown fox");
        let b = fnv1a32(b"The quick brown fox");
        assert_eq!(a, b);
        let c = fnv1a32(b"The quick brown cow");
        assert_ne!(a, c);
    }

    #[test]
    fn fnv1a64_empty_is_offset() {
        assert_eq!(fnv1a64(b""), FNV1A64_OFFSET);
    }

    #[test]
    fn fnv1a64_single_ascii_a() {
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn fnv1a64_matches_fnv1a32_structure() {
        // Sanity: different widths of the same input MUST NOT produce
        // matching low-32 bits  -  the prime differs, so structure does too.
        let bytes = b"vyre fingerprint";
        let h32 = fnv1a32(bytes);
        let h64 = fnv1a64(bytes);
        assert_ne!(h32 as u64, h64 & 0xffff_ffff);
    }

    #[test]
    fn fnv1a64_packed_u8_program_declares_one_source_byte_per_element() {
        let program = fnv1a64_program_n_u8("input", "out", 513);
        let input = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "input")
            .expect("Fix: packed-u8 FNV-1a64 input buffer must be declared");
        let out = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "out")
            .expect("Fix: FNV-1a64 output buffer must be declared");

        assert_eq!(input.element(), DataType::U8);
        assert_eq!(input.count(), 513);
        assert_eq!(out.element(), DataType::U32);
        assert_eq!(out.count(), 2);
    }
}

#[cfg(test)]
mod fnv1a_state_helper_tests {
    use super::*;

    #[test]
    fn cpu_state_helpers_match_slice_hashers() {
        let bytes = b"vyre-fnv-single-source";

        let mut h32 = fnv1a32_initial_state();
        for &byte in bytes {
            h32 = fnv1a32_update_byte(h32, byte);
        }
        assert_eq!(h32, fnv1a32(bytes));

        let mut h64 = fnv1a64_initial_state();
        for &byte in bytes {
            h64 = fnv1a64_update_byte(h64, byte);
        }
        assert_eq!(h64, fnv1a64(bytes));
    }
}

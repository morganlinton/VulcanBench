//! LZ4 sequence-index literal-copy primitive.
//!
//! LZ4-style formats have serial sequence discovery but parallel literal
//! copying once an index exists. This primitive is the reusable second stage:
//! one lane per sequence copies `[literal_start, literal_start + literal_len)`
//! into the prefix-summed output offset. Producers may be CPU, CUDA, WGPU, or
//! a future persistent decode megakernel as long as they satisfy the same
//! sequence-index contract.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical primitive op id.
pub const OP_ID: &str = "vyre-primitives::decode::ziftsieve_literal_copy";
/// One invocation processes one indexed LZ4 sequence.
pub const WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];
/// Defensive upper bound for one compressed block.
pub const MAX_BLOCK_SIZE: usize = 4 * 1024 * 1024;
/// Defensive upper bound for sequence count in one block.
pub const MAX_SEQUENCES_PER_BLOCK: usize = 100_000;

/// Host-side reference: sequential LZ4 literal extraction.
///
/// # Errors
///
/// Returns an actionable error string on malformed input. Every error message
/// includes a `Fix:` tag.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn ziftsieve_reference_extract_literals(
    compressed: &[u8],
    max_output: usize,
) -> Result<Vec<u8>, String> {
    let initial_cap = compressed
        .len()
        .saturating_mul(2)
        .min(max_output)
        .min(MAX_BLOCK_SIZE);
    let mut literals = Vec::with_capacity(initial_cap);
    let mut pos = 0usize;
    let mut sequence_count = 0usize;

    while pos < compressed.len() {
        sequence_count += 1;
        if sequence_count > MAX_SEQUENCES_PER_BLOCK {
            return Err(format!(
                "too many LZ4 sequences (max {MAX_SEQUENCES_PER_BLOCK}). \
                 Fix: use a smaller LZ4 block or increase MAX_SEQUENCES_PER_BLOCK"
            ));
        }

        let token = compressed[pos];
        pos += 1;

        let literal_len = (token >> 4) as usize;
        let match_len = (token & 0x0F) as usize;

        let literal_len = if literal_len == 15 {
            decode_length(compressed, &mut pos, literal_len)?
        } else {
            literal_len
        };

        if literal_len > MAX_BLOCK_SIZE {
            return Err(format!(
                "literal length {literal_len} exceeds MAX_BLOCK_SIZE {MAX_BLOCK_SIZE}. \
                 Fix: use a valid LZ4 stream"
            ));
        }

        if pos + literal_len > compressed.len() {
            return Err(format!(
                "literal exceeds block bounds at offset {pos}. \
                 Fix: use a valid LZ4 stream"
            ));
        }

        let remaining_output = max_output.saturating_sub(literals.len());
        let to_copy = literal_len.min(remaining_output);
        if to_copy > 0 {
            literals.extend_from_slice(&compressed[pos..pos + to_copy]);
        }
        pos += literal_len;

        if pos < compressed.len() {
            if pos + 2 > compressed.len() {
                return Err(format!(
                    "truncated match offset at offset {pos}. \
                     Fix: use a complete LZ4 stream"
                ));
            }
            pos += 2;

            if match_len == 15 {
                let _match_len_extension = decode_length(compressed, &mut pos, match_len)?;
            }
        }
    }

    Ok(literals)
}

fn decode_length(data: &[u8], pos: &mut usize, initial: usize) -> Result<usize, String> {
    let mut len = initial;
    loop {
        if *pos >= data.len() {
            return Err(format!(
                "truncated length encoding at offset {pos}. \
                 Fix: use a complete LZ4 stream"
            ));
        }
        let byte = data[*pos];
        *pos += 1;
        len = len.checked_add(byte as usize).ok_or_else(|| {
            "length overflow in variable-length encoding. Fix: use a valid LZ4 stream".to_string()
        })?;
        if byte < 255 {
            break;
        }
        if len > MAX_BLOCK_SIZE {
            return Err(format!(
                "length {len} exceeds MAX_BLOCK_SIZE {MAX_BLOCK_SIZE}. \
                 Fix: use a valid LZ4 stream"
            ));
        }
    }
    Ok(len)
}

/// Build the primitive body for indexed literal copy.
#[must_use]
pub fn ziftsieve_literal_copy_body(
    input: &str,
    output: &str,
    seq_literal_start: &str,
    seq_literal_len: &str,
    seq_literal_offset: &str,
    seq_count: u32,
) -> Vec<Node> {
    vec![
        Node::let_bind("seq_idx", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("seq_idx"), Expr::u32(seq_count)),
            vec![
                Node::let_bind(
                    "literal_start",
                    Expr::load(seq_literal_start, Expr::var("seq_idx")),
                ),
                Node::let_bind(
                    "literal_len",
                    Expr::load(seq_literal_len, Expr::var("seq_idx")),
                ),
                Node::let_bind(
                    "literal_offset",
                    Expr::load(seq_literal_offset, Expr::var("seq_idx")),
                ),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::var("literal_len"),
                    vec![
                        Node::let_bind(
                            "src",
                            Expr::load(
                                input,
                                Expr::add(Expr::var("literal_start"), Expr::var("i")),
                            ),
                        ),
                        Node::store(
                            output,
                            Expr::add(Expr::var("literal_offset"), Expr::var("i")),
                            Expr::var("src"),
                        ),
                    ],
                ),
            ],
        ),
    ]
}

/// Build a Program that copies indexed LZ4 literals in parallel.
#[must_use]
pub fn ziftsieve_literal_copy(
    input: &str,
    output: &str,
    seq_literal_start: &str,
    seq_literal_len: &str,
    seq_literal_offset: &str,
    input_len: u32,
    seq_count: u32,
    max_output: u32,
) -> Program {
    ziftsieve_literal_copy_with_op_id(
        OP_ID,
        input,
        output,
        seq_literal_start,
        seq_literal_len,
        seq_literal_offset,
        input_len,
        seq_count,
        max_output,
    )
}

/// Build a Program with a caller-provided op id.
///
/// Composition crates use this to preserve their public inventory id while
/// reusing the primitive-owned IR builder.
#[must_use]
pub fn ziftsieve_literal_copy_with_op_id(
    op_id: &str,
    input: &str,
    output: &str,
    seq_literal_start: &str,
    seq_literal_len: &str,
    seq_literal_offset: &str,
    input_len: u32,
    seq_count: u32,
    max_output: u32,
) -> Program {
    let body = ziftsieve_literal_copy_body(
        input,
        output,
        seq_literal_start,
        seq_literal_len,
        seq_literal_offset,
        seq_count,
    );

    let input_decl = BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32);
    let input_decl = if input_len == 0 {
        input_decl
    } else {
        input_decl.with_count(input_len)
    };

    Program::wrapped(
        vec![
            input_decl,
            BufferDecl::storage(seq_literal_start, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(seq_count.max(1)),
            BufferDecl::storage(seq_literal_len, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(seq_count.max(1)),
            BufferDecl::storage(seq_literal_offset, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(seq_count.max(1)),
            BufferDecl::storage(output, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(max_output.max(1)),
        ],
        WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
fn fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let input = crate::wire::pack_u32_slice(&[0x10, b'A' as u32, 0x20, b'B' as u32, b'C' as u32]);
    let seq_literal_start = crate::wire::pack_u32_slice(&[1, 3]);
    let seq_literal_len = crate::wire::pack_u32_slice(&[1, 2]);
    let seq_literal_offset = crate::wire::pack_u32_slice(&[0, 1]);
    vec![vec![
        input,
        seq_literal_start,
        seq_literal_len,
        seq_literal_offset,
        vec![0u8; 3 * 4],
    ]]
}

#[cfg(feature = "inventory-registry")]
fn fixture_outputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![crate::wire::pack_u32_slice(&[
        b'A' as u32,
        b'B' as u32,
        b'C' as u32,
    ])]]
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || ziftsieve_literal_copy("input", "output", "seq_start", "seq_len", "seq_off", 5, 2, 3),
        Some(fixture_inputs),
        Some(fixture_outputs),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    fn run(input: &[u8], seq_starts: &[u32], seq_lens: &[u32], seq_offsets: &[u32]) -> Vec<u32> {
        let seq_count = seq_starts.len() as u32;
        let max_output = seq_lens.iter().copied().sum::<u32>();
        let input_words = input.iter().map(|&b| u32::from(b)).collect::<Vec<_>>();
        let program = ziftsieve_literal_copy(
            "input",
            "output",
            "seq_start",
            "seq_len",
            "seq_off",
            input.len() as u32,
            seq_count,
            max_output,
        );
        let inputs = vec![
            Value::from(crate::wire::pack_u32_slice(&input_words)),
            Value::from(crate::wire::pack_u32_slice(seq_starts)),
            Value::from(crate::wire::pack_u32_slice(seq_lens)),
            Value::from(crate::wire::pack_u32_slice(seq_offsets)),
            Value::from(vec![0u8; (max_output.max(1) as usize) * 4]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: ziftsieve literal-copy primitive must run.");
        let words = crate::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
        words.into_iter().take(max_output as usize).collect()
    }

    #[test]
    fn single_literal() {
        assert_eq!(run(&[0x10, b'A'], &[1], &[1], &[0]), vec![b'A' as u32]);
    }

    #[test]
    fn two_sequences() {
        assert_eq!(
            run(&[0x10, b'A', 0x20, b'B', b'C'], &[1, 3], &[1, 2], &[0, 1]),
            vec![b'A' as u32, b'B' as u32, b'C' as u32]
        );
    }

    #[test]
    fn zero_literal_sequence_is_nop() {
        assert_eq!(
            run(&[0x00, 0x10, b'A'], &[0], &[0], &[0]),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn reference_extracts_simple_literal() {
        let result = ziftsieve_reference_extract_literals(&[0x10, b'A'], 1024).unwrap();
        assert_eq!(result, b"A");
    }

    #[test]
    fn reference_extracts_with_match_skip() {
        let data = [0x11, b'A', 0x01, 0x00];
        let result = ziftsieve_reference_extract_literals(&data, 1024).unwrap();
        assert_eq!(result, b"A");
    }

    #[test]
    fn reference_rejects_truncated_literal() {
        let err = ziftsieve_reference_extract_literals(&[0x20, b'A'], 1024).unwrap_err();
        assert!(err.contains("truncated") || err.contains("literal"));
    }

    #[test]
    fn reference_accepts_exact_max_sequence_count() {
        let mut data = Vec::new();
        for _ in 1..MAX_SEQUENCES_PER_BLOCK {
            data.push(0x10);
            data.push(b'X');
            data.extend_from_slice(&[0x00, 0x00]);
        }
        data.push(0x10);
        data.push(b'X');

        let result = ziftsieve_reference_extract_literals(&data, MAX_SEQUENCES_PER_BLOCK)
            .expect("Fix: MAX_SEQUENCES_PER_BLOCK is an inclusive maximum, not an exclusive one.");
        assert_eq!(result.len(), MAX_SEQUENCES_PER_BLOCK);
        assert!(result.iter().all(|&byte| byte == b'X'));
    }

    #[test]
    fn reference_rejects_too_many_sequences() {
        let mut data = Vec::new();
        for _ in 0..=MAX_SEQUENCES_PER_BLOCK {
            data.push(0x10);
            data.push(b'X');
            data.extend_from_slice(&[0x00, 0x00]);
        }
        let err = ziftsieve_reference_extract_literals(&data, 1024).unwrap_err();
        assert!(err.contains("sequence") || err.contains("MAX"));
    }
}

//! Batched Rust lexer program: one GPU lane scans one packed source slice.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::parsing::rust::lex::tokens::EOF;

use super::{emit_token, scan_one_token, WORKGROUP_SIZE};

/// Build a many-source lexer for packed Rust nano-subset source text.
///
/// `haystack` stores all source bytes with one byte per `u32` word.
/// `source_offsets` and `source_lens` hold one row per source. Output token
/// columns are laid out as `source_index * token_stride + token_index`, with
/// token starts relative to each source slice.
#[must_use]
pub fn rust_lexer_batch(
    haystack: &str,
    source_offsets: &str,
    source_lens: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
    source_count: u32,
    token_stride: u32,
) -> Program {
    let token_slots = source_count.max(1).saturating_mul(token_stride.max(1));
    let source_slots = source_count.max(1);

    let mut body = vec![
        Node::let_bind(
            "source_start",
            Expr::load(source_offsets, Expr::var("lane")),
        ),
        Node::let_bind("source_len", Expr::load(source_lens, Expr::var("lane"))),
        Node::let_bind(
            "source_end",
            Expr::min(
                Expr::add(Expr::var("source_start"), Expr::var("source_len")),
                Expr::u32(haystack_len),
            ),
        ),
        Node::let_bind(
            "token_base",
            Expr::mul(Expr::var("lane"), Expr::u32(token_stride)),
        ),
        Node::let_bind("cursor", Expr::var("source_start")),
        Node::let_bind("tok_idx", Expr::u32(0)),
        Node::loop_for(
            "scan_iter",
            Expr::u32(0),
            Expr::add(Expr::var("source_len"), Expr::u32(1)),
            vec![Node::if_then(
                Expr::lt(Expr::var("cursor"), Expr::var("source_end")),
                scan_one_token(
                    haystack,
                    Expr::var("source_start"),
                    Expr::var("source_end"),
                    Expr::var("token_base"),
                    out_tok_types,
                    out_tok_starts,
                    out_tok_lens,
                ),
            )],
        ),
    ];
    body.extend(emit_token(
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        Expr::var("token_base"),
        Expr::u32(u32::from(EOF)),
        Expr::var("source_len"),
        Expr::u32(0),
    ));
    body.push(Node::store(
        out_counts,
        Expr::var("lane"),
        Expr::var("tok_idx"),
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len.max(1)),
            BufferDecl::storage(source_offsets, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(source_slots),
            BufferDecl::storage(source_lens, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(source_slots),
            BufferDecl::storage(out_tok_types, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(token_slots),
            BufferDecl::storage(out_tok_starts, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(token_slots),
            BufferDecl::storage(out_tok_lens, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(token_slots),
            BufferDecl::storage(out_counts, 6, BufferAccess::ReadWrite, DataType::U32)
                .with_count(source_slots),
        ],
        [WORKGROUP_SIZE, 1, 1],
        vec![
            Node::let_bind("lane", Expr::InvocationId { axis: 0 }),
            Node::if_then(Expr::lt(Expr::var("lane"), Expr::u32(source_count)), body),
        ],
    )
    .with_entry_op_id("vyre-libs::parsing::rust_lexer_batch")
    .with_non_composable_with_self(true)
}

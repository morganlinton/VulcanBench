//! Succinct bitvector metadata primitives.
//!
//! These ops build the rank side of rank/select navigation for compact token,
//! AST, and graph bitvectors. They keep hot navigation state as packed `u32`
//! words plus sparse superblock counters, so GPU kernels trade bandwidth-heavy
//! pointer chasing for popcount math over coalesced words.

use core::fmt;

use crate::region::{tag_program, wrap_anonymous, wrap_child};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;

const RANK_SUPERBLOCKS_OP_ID: &str = "vyre-libs::math::succinct::rank1_superblocks";
const RANK_QUERY_OP_ID: &str = "vyre-libs::math::succinct::rank1_query";
const SELECT_QUERY_OP_ID: &str = "vyre-libs::math::succinct::select1_query";

/// Build-time errors for succinct bitvector Programs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuccinctBuildError {
    /// Superblock size must be non-zero.
    ZeroBlockWords,
    /// The derived superblock output length overflowed `u32`.
    SuperblockCountOverflow,
}

impl fmt::Display for SuccinctBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBlockWords => {
                write!(f, "Fix: rank superblock size must be at least one u32 word")
            }
            Self::SuperblockCountOverflow => write!(
                f,
                "Fix: rank superblock count overflowed u32; shard the bitvector"
            ),
        }
    }
}

impl std::error::Error for SuccinctBuildError {}

fn superblock_count(word_count: u32, block_words: u32) -> Result<u32, SuccinctBuildError> {
    if block_words == 0 {
        return Err(SuccinctBuildError::ZeroBlockWords);
    }
    let full_blocks = word_count / block_words;
    let has_partial = u32::from(word_count % block_words != 0);
    full_blocks
        .checked_add(has_partial)
        .and_then(|blocks| blocks.checked_add(1))
        .ok_or(SuccinctBuildError::SuperblockCountOverflow)
}

/// Build sparse rank1 superblocks for a packed u32 bitvector.
///
/// `superblocks[0]` is always zero. Each following entry stores the cumulative
/// count of set bits before that superblock. The final sentinel stores the
/// total popcount for the whole bitvector.
#[must_use]
pub fn rank1_superblocks(
    bits: &str,
    superblocks: &str,
    word_count: u32,
    block_words: u32,
) -> Program {
    try_rank1_superblocks(bits, superblocks, word_count, block_words).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            RANK_SUPERBLOCKS_OP_ID,
            superblocks,
            DataType::U32,
            format!("{err}"),
        )
    })
}

/// Checked builder for [`rank1_superblocks`].
///
/// # Errors
///
/// Returns [`SuccinctBuildError`] when `block_words` is zero or the derived
/// metadata length overflows `u32`.
pub fn try_rank1_superblocks(
    bits: &str,
    superblocks: &str,
    word_count: u32,
    block_words: u32,
) -> Result<Program, SuccinctBuildError> {
    let out_count = superblock_count(word_count, block_words)?;
    let body = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::store(superblocks, Expr::u32(0), Expr::u32(0)),
            Node::let_bind("rank_acc", Expr::u32(0)),
            Node::loop_for(
                "rank_word",
                Expr::u32(0),
                Expr::u32(word_count),
                vec![
                    Node::if_then(
                        Expr::and(
                            Expr::gt(Expr::var("rank_word"), Expr::u32(0)),
                            Expr::eq(
                                Expr::rem(Expr::var("rank_word"), Expr::u32(block_words)),
                                Expr::u32(0),
                            ),
                        ),
                        vec![Node::store(
                            superblocks,
                            Expr::div(Expr::var("rank_word"), Expr::u32(block_words)),
                            Expr::var("rank_acc"),
                        )],
                    ),
                    Node::assign(
                        "rank_acc",
                        Expr::add(
                            Expr::var("rank_acc"),
                            Expr::popcount(Expr::load(bits, Expr::var("rank_word"))),
                        ),
                    ),
                ],
            ),
            Node::store(superblocks, Expr::u32(out_count - 1), Expr::var("rank_acc")),
        ],
    )];
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(bits, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(word_count.max(1)),
            BufferDecl::output(superblocks, 1, DataType::U32).with_count(out_count),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            RANK_SUPERBLOCKS_OP_ID,
            vec![wrap_child(
                vyre_primitives::graph::path_reconstruct::OP_ID,
                GeneratorRef {
                    name: RANK_SUPERBLOCKS_OP_ID.to_string(),
                },
                body,
            )],
        )],
    ))
}

/// Answer rank1-before-position queries from sparse superblocks.
///
/// Each `bit_indices[q]` is a zero-based bit offset. The output is the number
/// of set bits strictly before that offset. Query offsets must address an
/// existing packed word; use the final superblock sentinel for total popcount.
#[must_use]
pub fn rank1_query(
    bits: &str,
    superblocks: &str,
    bit_indices: &str,
    out: &str,
    word_count: u32,
    query_count: u32,
    block_words: u32,
) -> Program {
    try_rank1_query(
        bits,
        superblocks,
        bit_indices,
        out,
        word_count,
        query_count,
        block_words,
    )
    .unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            RANK_QUERY_OP_ID,
            out,
            DataType::U32,
            format!("{err}"),
        )
    })
}

/// Checked builder for [`rank1_query`].
///
/// # Errors
///
/// Returns [`SuccinctBuildError`] when `block_words` is zero or the derived
/// metadata length overflows `u32`.
pub fn try_rank1_query(
    bits: &str,
    superblocks: &str,
    bit_indices: &str,
    out: &str,
    word_count: u32,
    query_count: u32,
    block_words: u32,
) -> Result<Program, SuccinctBuildError> {
    let sb_count = superblock_count(word_count, block_words)?;
    let q = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(q.clone(), Expr::u32(query_count)),
        vec![
            Node::let_bind("bit_index", Expr::load(bit_indices, q.clone())),
            Node::let_bind(
                "word_index",
                Expr::div(Expr::var("bit_index"), Expr::u32(32)),
            ),
            Node::if_then(
                Expr::ge(Expr::var("word_index"), Expr::u32(word_count)),
                vec![Node::trap(
                    Expr::var("bit_index"),
                    "rank-query-out-of-bounds",
                )],
            ),
            Node::let_bind(
                "block_index",
                Expr::div(Expr::var("word_index"), Expr::u32(block_words)),
            ),
            Node::let_bind(
                "rank_acc",
                Expr::load(superblocks, Expr::var("block_index")),
            ),
            Node::let_bind(
                "block_start_word",
                Expr::mul(Expr::var("block_index"), Expr::u32(block_words)),
            ),
            Node::loop_for(
                "rank_word",
                Expr::var("block_start_word"),
                Expr::var("word_index"),
                vec![Node::assign(
                    "rank_acc",
                    Expr::add(
                        Expr::var("rank_acc"),
                        Expr::popcount(Expr::load(bits, Expr::var("rank_word"))),
                    ),
                )],
            ),
            Node::let_bind(
                "bit_offset",
                Expr::rem(Expr::var("bit_index"), Expr::u32(32)),
            ),
            Node::let_bind(
                "partial_mask",
                Expr::select(
                    Expr::eq(Expr::var("bit_offset"), Expr::u32(0)),
                    Expr::u32(0),
                    Expr::sub(
                        Expr::shl(Expr::u32(1), Expr::var("bit_offset")),
                        Expr::u32(1),
                    ),
                ),
            ),
            Node::assign(
                "rank_acc",
                Expr::add(
                    Expr::var("rank_acc"),
                    Expr::popcount(Expr::bitand(
                        Expr::load(bits, Expr::var("word_index")),
                        Expr::var("partial_mask"),
                    )),
                ),
            ),
            Node::store(out, q, Expr::var("rank_acc")),
        ],
    )];
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(bits, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(word_count.max(1)),
            BufferDecl::storage(superblocks, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(sb_count),
            BufferDecl::storage(bit_indices, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(query_count.max(1)),
            BufferDecl::output(out, 3, DataType::U32).with_count(query_count.max(1)),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(RANK_QUERY_OP_ID, body)],
    ))
}

/// Answer select1 queries over a packed u32 bitvector.
///
/// Each `k_indices[q]` is a one-based rank. The output is the zero-based bit
/// position of the `k`-th set bit. `k == 0` and `k > total_popcount` trap
/// loudly so callers cannot silently navigate to a bogus AST or graph node.
#[must_use]
pub fn select1_query(
    bits: &str,
    k_indices: &str,
    out: &str,
    word_count: u32,
    query_count: u32,
) -> Program {
    try_select1_query(bits, k_indices, out, word_count, query_count).unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            SELECT_QUERY_OP_ID,
            out,
            DataType::U32,
            format!("{err}"),
        )
    })
}

/// Checked builder for [`select1_query`].
///
/// # Errors
///
/// Currently this builder has no static failure modes. Runtime queries still
/// trap when `k == 0` or when `k` exceeds the bitvector popcount.
pub fn try_select1_query(
    bits: &str,
    k_indices: &str,
    out: &str,
    word_count: u32,
    query_count: u32,
) -> Result<Program, SuccinctBuildError> {
    Ok(tag_program(
        SELECT_QUERY_OP_ID,
        vyre_primitives::bitset::select::select1_query(
            bits,
            k_indices,
            out,
            word_count,
            query_count,
        ),
    ))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: RANK_SUPERBLOCKS_OP_ID,
        build: || rank1_superblocks("bits", "superblocks", 4, 2),
        test_inputs: Some(|| {
            let bits = [0b1011u32, 0x8000_0000, 0xFFFF_0000, 0u32];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&bits)]]
        }),
        expected_output: Some(|| {
            let expected = [0u32, 4, 20];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SELECT_QUERY_OP_ID,
        build: || select1_query("bits", "queries", "out", 4, 5),
        test_inputs: Some(|| {
            let bits = [0b1011u32, 0x8000_0000, 0xFFFF_0000, 0u32];
            let queries = [1u32, 2, 3, 4, 5];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&bits), to_bytes(&queries)]]
        }),
        expected_output: Some(|| {
            let expected = [0u32, 1, 3, 63, 80];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: RANK_QUERY_OP_ID,
        build: || rank1_query("bits", "superblocks", "queries", "out", 4, 5, 2),
        test_inputs: Some(|| {
            let bits = [0b1011u32, 0x8000_0000, 0xFFFF_0000, 0u32];
            let superblocks = [0u32, 4, 20];
            let queries = [0u32, 1, 4, 63, 80];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&bits), to_bytes(&superblocks), to_bytes(&queries)]]
        }),
        expected_output: Some(|| {
            let expected = [0u32, 1, 3, 3, 4];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

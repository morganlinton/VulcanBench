//! CRC-32 (IEEE 802.3) hash primitive.
//!
//! Polynomial `0xEDB88320`  -  the reflected form of `0x04C11DB7`, the
//! one used by gzip, zip, Ethernet, PNG, rsync. Byte-at-a-time
//! table-driven. Reference implementation is a straight port of the
//! textbook slicing algorithm.

use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical CRC-32 initial value.
pub const CRC32_INIT: u32 = 0xFFFF_FFFF;

/// Reflected IEEE 802.3 polynomial.
pub const CRC32_POLY: u32 = 0xEDB8_8320;

/// Stable Tier 2.5 op id for the CRC-32 serial byte walker.
pub const CRC32_OP_ID: &str = "vyre-primitives::hash::crc32";

/// Stable Tier 2.5 op id for parallel CRC-32 chunk summary emission.
pub const CRC32_CHUNK_OP_ID: &str = "vyre-primitives::hash::crc32_chunk";

/// Stable Tier 2.5 op id for pairwise CRC-32 chunk-summary reduction.
pub const CRC32_PAIR_REDUCE_OP_ID: &str = "vyre-primitives::hash::crc32_pair_reduce";

static CRC32_TABLE: OnceLock<[u32; 256]> = OnceLock::new();

/// Self-contained CRC-32 chunk summary for associative reductions.
///
/// The `crc` field is the normal finalized CRC-32 value for the chunk. Adjacent
/// chunks can be combined with [`crc32_combine_chunks`] without reading the
/// original bytes, which is the algebra needed by GPU block scans and resident
/// streaming pipelines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Crc32Chunk {
    /// Exact chunk length in bytes.
    pub len: u64,
    /// Finalized CRC-32 for this chunk.
    pub crc: u32,
}

/// Kind of executable step in a CRC-32 GPU map-reduce plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Crc32MapReduceStepKind {
    /// Emit one `[crc, len]` summary per input byte chunk.
    ChunkSummary,
    /// Pair-reduce adjacent `[crc, len]` summaries.
    PairReduce,
}

/// One executable CUDA/WGPU dispatch shape in a CRC-32 map-reduce plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Crc32MapReduceStep {
    /// Step kind.
    pub kind: Crc32MapReduceStepKind,
    /// Logical input item count: bytes for [`Crc32MapReduceStepKind::ChunkSummary`],
    /// pairs for [`Crc32MapReduceStepKind::PairReduce`].
    pub input_items: u32,
    /// Number of output `[crc, len]` pairs produced by this step.
    pub output_pairs: u32,
    /// Number of u32 input words expected by the step Program.
    pub input_words: u32,
    /// Number of u32 output words produced by the step Program.
    pub output_words: u32,
    /// Dispatch grid override for this step.
    pub grid: [u32; 3],
}

/// Single-source execution plan for CRC-32 GPU map-reduce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Crc32MapReducePlan {
    /// Input length in byte slots.
    pub input_len: u32,
    /// Byte slots hashed by each chunk-summary invocation.
    pub chunk_size: NonZeroU32,
    /// Ordered dispatch steps: one chunk-summary step followed by zero or more
    /// pair-reduction steps.
    pub steps: Vec<Crc32MapReduceStep>,
}

/// CPU reference: CRC-32 over a byte slice. Returns the post-complement
/// value (matches the gzip / zip convention).
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    let table = crc32_table();
    let mut crc = crc32_initial_state();
    for &byte in bytes {
        crc = crc32_update_byte_state(crc, table, byte);
    }
    crc32_finalize_state(crc)
}

/// Summarize a byte slice as an independently-combinable CRC-32 chunk.
#[must_use]
pub fn crc32_chunk(bytes: &[u8]) -> Crc32Chunk {
    Crc32Chunk {
        len: bytes.len() as u64,
        crc: crc32(bytes),
    }
}

/// Combine two finalized CRC-32 values.
///
/// `right_len` is the exact byte length of the right-hand input. The result is
/// `crc32(left_bytes || right_bytes)` when `left_crc == crc32(left_bytes)` and
/// `right_crc == crc32(right_bytes)`.
#[must_use]
pub fn crc32_combine(left_crc: u32, right_crc: u32, right_len: u64) -> u32 {
    if right_len == 0 {
        return left_crc;
    }

    let mut odd = [0u32; 32];
    let mut even = [0u32; 32];

    odd[0] = CRC32_POLY;
    let mut row = 1u32;
    for slot in odd.iter_mut().skip(1) {
        *slot = row;
        row <<= 1;
    }

    gf2_matrix_square(&mut even, &odd);
    gf2_matrix_square(&mut odd, &even);

    let mut len = right_len;
    let mut crc = left_crc;
    loop {
        gf2_matrix_square(&mut even, &odd);
        if (len & 1) != 0 {
            crc = gf2_matrix_times(&even, crc);
        }
        len >>= 1;
        if len == 0 {
            break;
        }

        gf2_matrix_square(&mut odd, &even);
        if (len & 1) != 0 {
            crc = gf2_matrix_times(&odd, crc);
        }
        len >>= 1;
        if len == 0 {
            break;
        }
    }

    crc ^ right_crc
}

/// Combine adjacent CRC-32 chunk summaries without reading source bytes.
#[must_use]
pub fn crc32_combine_chunks(left: Crc32Chunk, right: Crc32Chunk) -> Option<Crc32Chunk> {
    Some(Crc32Chunk {
        len: left.len.checked_add(right.len)?,
        crc: crc32_combine(left.crc, right.crc, right.len),
    })
}

/// Pair-reduce adjacent CRC-32 chunks. Odd tails are carried forward.
#[must_use]
pub fn crc32_pair_reduce_chunks(chunks: &[Crc32Chunk]) -> Option<Vec<Crc32Chunk>> {
    let mut reduced = Vec::with_capacity(chunks.len().div_ceil(2));
    for pair in chunks.chunks(2) {
        let chunk = match pair {
            [left, right] => crc32_combine_chunks(*left, *right)?,
            [tail] => *tail,
            [] => continue,
            _ => unreachable!("slice chunks of two contain at most two items"),
        };
        reduced.push(chunk);
    }
    Some(reduced)
}

/// Pack CRC-32 chunk summaries into the executable `[crc, len]` u32 ABI.
#[must_use]
pub fn crc32_pack_chunks_u32(chunks: &[Crc32Chunk]) -> Option<Vec<u32>> {
    let mut words = Vec::with_capacity(chunks.len().checked_mul(2)?);
    for chunk in chunks {
        words.push(chunk.crc);
        words.push(u32::try_from(chunk.len).ok()?);
    }
    Some(words)
}

/// Unpack executable `[crc, len]` u32 words into CRC-32 chunk summaries.
#[must_use]
pub fn crc32_unpack_chunks_u32(words: &[u32]) -> Option<Vec<Crc32Chunk>> {
    let pairs = words.chunks_exact(2);
    if !pairs.remainder().is_empty() {
        return None;
    }
    Some(
        pairs
            .map(|pair| Crc32Chunk {
                crc: pair[0],
                len: u64::from(pair[1]),
            })
            .collect(),
    )
}

/// Pair-reduce packed executable `[crc, len]` u32 words.
#[must_use]
pub fn crc32_pair_reduce_chunk_words(words: &[u32]) -> Option<Vec<u32>> {
    let chunks = crc32_unpack_chunks_u32(words)?;
    let reduced = crc32_pair_reduce_chunks(&chunks)?;
    crc32_pack_chunks_u32(&reduced)
}

/// Process-wide CRC-32 table used by CPU references and tests.
///
/// The table is deterministic and immutable. Caching it removes a fixed
/// 256-slot rebuild from every reference invocation without changing the
/// public [`build_table`] helper used by tests and artifact generation.
#[must_use]
pub fn crc32_table() -> &'static [u32; 256] {
    CRC32_TABLE.get_or_init(build_table)
}

/// Initial CRC-32 CPU state before byte updates.
#[must_use]
pub const fn crc32_initial_state() -> u32 {
    CRC32_INIT
}

/// Canonical CRC-32 CPU single-byte update.
#[must_use]
pub fn crc32_update_byte_state(crc: u32, table: &[u32; 256], byte: u8) -> u32 {
    let idx = ((crc ^ u32::from(byte)) & 0xFF) as usize;
    (crc >> 8) ^ table[idx]
}

/// Final CRC-32 CPU state complement.
#[must_use]
pub const fn crc32_finalize_state(crc: u32) -> u32 {
    crc ^ CRC32_INIT
}

/// Build the 256-entry CRC-32 table at runtime. Deterministic; the
/// GPU-side op loads this buffer from the host.
#[must_use]
pub fn build_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 {
                (c >> 1) ^ CRC32_POLY
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    table
}

fn gf2_matrix_times(matrix: &[u32; 32], mut vector: u32) -> u32 {
    let mut sum = 0u32;
    let mut index = 0usize;
    while vector != 0 {
        if (vector & 1) != 0 {
            sum ^= matrix[index];
        }
        vector >>= 1;
        index += 1;
    }
    sum
}

fn gf2_matrix_square(square: &mut [u32; 32], matrix: &[u32; 32]) {
    for index in 0..32 {
        square[index] = gf2_matrix_times(matrix, matrix[index]);
    }
}

/// Build a Program that writes CRC-32(input[0..n]) to `out[0]`.
///
/// `input[i]` packs one byte per u32 slot in the low 8 bits; high bits are
/// ignored by construction. This is the single source of truth for the CRC-32
/// executable IR body; higher-tier wrappers may rename buffers or stamp their
/// own region id, but must delegate to this primitive body instead of forking
/// the bit loop.
#[must_use]
pub fn crc32_program(input: &str, out: &str, n: u32) -> Program {
    let body = vec![Node::Region {
        generator: Ident::from(CRC32_OP_ID),
        source_region: None,
        body: Arc::new(crc32_body(input, out, n)),
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

/// Build a Program that emits CRC-32 chunk summaries in parallel.
///
/// Each invocation `gid.x` hashes at most `chunk_size` byte slots starting at
/// `gid.x * chunk_size`. Results are written as adjacent words in one ABI-legal
/// output buffer: `out[chunk * 2] = finalized chunk CRC` and
/// `out[chunk * 2 + 1] = chunk byte length`. The summaries can be reduced with
/// [`crc32_combine_chunks`].
#[must_use]
pub fn crc32_chunk_program(input: &str, out: &str, n: u32, chunk_size: NonZeroU32) -> Program {
    let chunk_size = chunk_size.get();
    let chunk_count = crc32_chunk_count(n, chunk_size);
    let output_words = match crc32_chunk_output_words(n, chunk_size) {
        Some(words) => words,
        None => u32::MAX,
    };
    let body = vec![Node::Region {
        generator: Ident::from(CRC32_CHUNK_OP_ID),
        source_region: None,
        body: Arc::new(crc32_chunk_body(input, out, n, chunk_size)),
    }];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::output(out, 1, DataType::U32).with_count(output_words),
        ],
        [1, 1, 1],
        body,
    )
}

/// Build a Program that pair-reduces packed CRC-32 chunk summaries on GPU.
///
/// Input and output use the same packed ABI as [`crc32_chunk_program`]:
/// `[crc0, len0, crc1, len1, ...]`. Each invocation combines two input pairs
/// with the associative CRC-32 operator and writes one output pair. Odd tails
/// are copied unchanged.
#[must_use]
pub fn crc32_pair_reduce_program(input: &str, out: &str, pair_count: NonZeroU32) -> Program {
    let pair_count = pair_count.get();
    let input_words = match pair_count.checked_mul(2) {
        Some(words) => words,
        None => u32::MAX,
    };
    let output_pairs = crc32_pair_reduce_output_pairs(pair_count);
    let output_words = match output_pairs.checked_mul(2) {
        Some(words) => words,
        None => u32::MAX,
    };
    let body = vec![Node::Region {
        generator: Ident::from(CRC32_PAIR_REDUCE_OP_ID),
        source_region: None,
        body: Arc::new(crc32_pair_reduce_body(input, out, pair_count)),
    }];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_words),
            BufferDecl::output(out, 1, DataType::U32).with_count(output_words),
        ],
        [1, 1, 1],
        body,
    )
}

/// Number of chunk-summary rows produced by [`crc32_chunk_program`].
#[must_use]
pub const fn crc32_chunk_count(n: u32, chunk_size: u32) -> u32 {
    if chunk_size == 0 {
        0
    } else if n == 0 {
        1
    } else {
        n.div_ceil(chunk_size)
    }
}

/// Number of u32 output words produced by [`crc32_chunk_program`].
#[must_use]
pub const fn crc32_chunk_output_words(n: u32, chunk_size: u32) -> Option<u32> {
    crc32_chunk_count(n, chunk_size).checked_mul(2)
}

/// Number of output pairs produced by [`crc32_pair_reduce_program`].
#[must_use]
pub const fn crc32_pair_reduce_output_pairs(pair_count: u32) -> u32 {
    pair_count.div_ceil(2)
}

/// Build a single-source CRC-32 GPU map-reduce plan for `input_len` byte slots.
#[must_use]
pub fn crc32_map_reduce_plan(input_len: u32, chunk_size: NonZeroU32) -> Option<Crc32MapReducePlan> {
    let chunk_size_words = chunk_size.get();
    let chunk_count = crc32_chunk_count(input_len, chunk_size_words);
    let chunk_output_words = crc32_chunk_output_words(input_len, chunk_size_words)?;
    let mut steps = Vec::with_capacity(1 + 32);
    steps.push(Crc32MapReduceStep {
        kind: Crc32MapReduceStepKind::ChunkSummary,
        input_items: input_len,
        output_pairs: chunk_count,
        input_words: input_len.max(1),
        output_words: chunk_output_words,
        grid: [chunk_count, 1, 1],
    });

    let mut pair_count = chunk_count;
    while pair_count > 1 {
        let output_pairs = crc32_pair_reduce_output_pairs(pair_count);
        let input_words = pair_count.checked_mul(2)?;
        let output_words = output_pairs.checked_mul(2)?;
        steps.push(Crc32MapReduceStep {
            kind: Crc32MapReduceStepKind::PairReduce,
            input_items: pair_count,
            output_pairs,
            input_words,
            output_words,
            grid: [output_pairs, 1, 1],
        });
        pair_count = output_pairs;
    }

    Some(Crc32MapReducePlan {
        input_len,
        chunk_size,
        steps,
    })
}

/// Initial CRC expression for IR compositions that fuse CRC-32 with other
/// one-pass byte walkers.
#[must_use]
pub fn crc32_initial_expr() -> Expr {
    Expr::u32(CRC32_INIT)
}

/// Emit the canonical CRC-32 single-byte update into `crc_var`.
///
/// `byte` may contain non-byte high bits; the helper masks to the low 8 bits
/// so fused compositions preserve the same input contract as [`crc32_program`].
#[must_use]
pub fn crc32_update_byte_nodes(crc_var: &str, bit_var: &str, byte: Expr) -> Vec<Node> {
    vec![
        Node::assign(
            crc_var,
            Expr::bitxor(Expr::var(crc_var), Expr::bitand(byte, Expr::u32(0xFF))),
        ),
        Node::loop_for(
            bit_var,
            Expr::u32(0),
            Expr::u32(8),
            vec![Node::assign(
                crc_var,
                Expr::Select {
                    cond: Box::new(Expr::ne(
                        Expr::bitand(Expr::var(crc_var), Expr::u32(1)),
                        Expr::u32(0),
                    )),
                    true_val: Box::new(Expr::bitxor(
                        Expr::shr(Expr::var(crc_var), Expr::u32(1)),
                        Expr::u32(CRC32_POLY),
                    )),
                    false_val: Box::new(Expr::shr(Expr::var(crc_var), Expr::u32(1))),
                },
            )],
        ),
    ]
}

/// Final CRC expression for IR compositions that fuse CRC-32 with other
/// one-pass byte walkers.
#[must_use]

pub fn crc32_finalize_expr(crc: Expr) -> Expr {
    Expr::bitxor(crc, Expr::u32(CRC32_INIT))
}

fn crc32_body(input: &str, out: &str, n: u32) -> Vec<Node> {
    vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("crc", crc32_initial_expr()),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(n),
                crc32_update_byte_nodes("crc", "bit", Expr::load(input, Expr::var("i"))),
            ),
            Node::store(out, Expr::u32(0), crc32_finalize_expr(Expr::var("crc"))),
        ],
    )]
}

fn crc32_chunk_body(input: &str, out: &str, n: u32, chunk_size: u32) -> Vec<Node> {
    let gid = Expr::InvocationId { axis: 0 };
    vec![Node::if_then(
        Expr::lt(gid.clone(), Expr::u32(crc32_chunk_count(n, chunk_size))),
        vec![
            Node::let_bind("chunk_start", Expr::mul(gid, Expr::u32(chunk_size))),
            Node::let_bind(
                "chunk_remaining",
                Expr::sub(Expr::u32(n), Expr::var("chunk_start")),
            ),
            Node::let_bind(
                "chunk_len",
                Expr::Select {
                    cond: Box::new(Expr::lt(
                        Expr::var("chunk_remaining"),
                        Expr::u32(chunk_size),
                    )),
                    true_val: Box::new(Expr::var("chunk_remaining")),
                    false_val: Box::new(Expr::u32(chunk_size)),
                },
            ),
            Node::let_bind("crc", crc32_initial_expr()),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::var("chunk_len"),
                crc32_update_byte_nodes(
                    "crc",
                    "bit",
                    Expr::load(input, Expr::add(Expr::var("chunk_start"), Expr::var("i"))),
                ),
            ),
            Node::store(
                out,
                Expr::mul(Expr::InvocationId { axis: 0 }, Expr::u32(2)),
                crc32_finalize_expr(Expr::var("crc")),
            ),
            Node::store(
                out,
                Expr::add(
                    Expr::mul(Expr::InvocationId { axis: 0 }, Expr::u32(2)),
                    Expr::u32(1),
                ),
                Expr::var("chunk_len"),
            ),
        ],
    )]
}

fn crc32_pair_reduce_body(input: &str, out: &str, pair_count: u32) -> Vec<Node> {
    let gid = Expr::InvocationId { axis: 0 };
    let output_pairs = crc32_pair_reduce_output_pairs(pair_count);
    let mut body = vec![
        Node::let_bind("left_pair", Expr::mul(gid.clone(), Expr::u32(2))),
        Node::let_bind(
            "right_pair",
            Expr::add(Expr::var("left_pair"), Expr::u32(1)),
        ),
        Node::let_bind("left_base", Expr::mul(Expr::var("left_pair"), Expr::u32(2))),
        Node::let_bind(
            "right_base",
            Expr::mul(Expr::var("right_pair"), Expr::u32(2)),
        ),
        Node::let_bind("out_base", Expr::mul(gid.clone(), Expr::u32(2))),
        Node::let_bind("left_crc", Expr::load(input, Expr::var("left_base"))),
        Node::let_bind(
            "left_len",
            Expr::load(input, Expr::add(Expr::var("left_base"), Expr::u32(1))),
        ),
        Node::let_bind("right_crc", Expr::u32(0)),
        Node::let_bind("right_len", Expr::u32(0)),
        Node::if_then(
            Expr::lt(Expr::var("right_pair"), Expr::u32(pair_count)),
            vec![
                Node::assign("right_crc", Expr::load(input, Expr::var("right_base"))),
                Node::assign(
                    "right_len",
                    Expr::load(input, Expr::add(Expr::var("right_base"), Expr::u32(1))),
                ),
            ],
        ),
        Node::let_bind("combined_crc", Expr::var("left_crc")),
    ];
    body.extend(crc32_combine_u32_len_nodes(
        "combined_crc",
        "right_len",
        "right_crc",
    ));
    body.extend([
        Node::store(out, Expr::var("out_base"), Expr::var("combined_crc")),
        Node::store(
            out,
            Expr::add(Expr::var("out_base"), Expr::u32(1)),
            Expr::add(Expr::var("left_len"), Expr::var("right_len")),
        ),
    ]);

    vec![Node::if_then(Expr::lt(gid, Expr::u32(output_pairs)), body)]
}

fn crc32_combine_u32_len_nodes(
    crc_var: &str,
    right_len_var: &str,
    right_crc_var: &str,
) -> Vec<Node> {
    let byte_shift = crc32_byte_shift_matrix();
    let mut nodes = Vec::with_capacity(1 + 32 + 32 * 65 + 1);
    for (index, value) in byte_shift.iter().enumerate() {
        nodes.push(Node::let_bind(format!("crc_op_{index}"), Expr::u32(*value)));
    }
    for bit in 0..32 {
        let mask = 1u32 << bit;
        nodes.push(Node::assign(
            crc_var,
            Expr::Select {
                cond: Box::new(Expr::ne(
                    Expr::bitand(Expr::var(right_len_var), Expr::u32(mask)),
                    Expr::u32(0),
                )),
                true_val: Box::new(gf2_matrix_times_expr("crc_op", Expr::var(crc_var))),
                false_val: Box::new(Expr::var(crc_var)),
            },
        ));
        for index in 0..32 {
            nodes.push(Node::let_bind(
                format!("crc_next_{bit}_{index}"),
                gf2_matrix_times_expr("crc_op", Expr::var(format!("crc_op_{index}"))),
            ));
        }
        for index in 0..32 {
            nodes.push(Node::assign(
                format!("crc_op_{index}"),
                Expr::var(format!("crc_next_{bit}_{index}")),
            ));
        }
    }
    nodes.push(Node::assign(
        crc_var,
        Expr::Select {
            cond: Box::new(Expr::eq(Expr::var(right_len_var), Expr::u32(0))),
            true_val: Box::new(Expr::var(crc_var)),
            false_val: Box::new(Expr::bitxor(Expr::var(crc_var), Expr::var(right_crc_var))),
        },
    ));
    nodes
}

fn gf2_matrix_times_expr(matrix_prefix: &str, vector: Expr) -> Expr {
    let mut sum = Expr::u32(0);
    for bit in 0..32 {
        let mask = 1u32 << bit;
        let selected = Expr::Select {
            cond: Box::new(Expr::ne(
                Expr::bitand(vector.clone(), Expr::u32(mask)),
                Expr::u32(0),
            )),
            true_val: Box::new(Expr::var(format!("{matrix_prefix}_{bit}"))),
            false_val: Box::new(Expr::u32(0)),
        };
        sum = Expr::bitxor(sum, selected);
    }
    sum
}

fn crc32_byte_shift_matrix() -> [u32; 32] {
    let mut odd = [0u32; 32];
    let mut even = [0u32; 32];
    odd[0] = CRC32_POLY;
    let mut row = 1u32;
    for slot in odd.iter_mut().skip(1) {
        *slot = row;
        row <<= 1;
    }
    gf2_matrix_square(&mut even, &odd);
    gf2_matrix_square(&mut odd, &even);
    gf2_matrix_square(&mut even, &odd);
    even
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        CRC32_OP_ID,
        || crc32_program("input", "out", 3),
        Some(|| {
            let bytes = crate::wire::pack_bytes_as_u32_slice(b"abc");
            vec![vec![bytes, vec![0u8; 4]]]
        }),
        Some(|| vec![vec![0x3524_41c2u32.to_le_bytes().to_vec()]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference vectors from RFC 3720 (iSCSI) + the Castagnoli paper.

    #[test]
    fn crc32_empty_is_zero() {
        // CRC-32("" ) = 0 after the final complement.
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn crc32_single_zero_byte() {
        // crc32([0x00]) = 0xD202_EF8D
        assert_eq!(crc32(&[0x00]), 0xD202_EF8D);
    }

    #[test]
    fn crc32_nine_ones() {
        // crc32("123456789") = 0xCBF4_3926  -  classic test vector.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_table_128_slot() {
        // First row after zero should be 1→polynomial-shift.
        let table = build_table();
        assert_eq!(table[0], 0);
        // Standard table[1] for 0xEDB88320.
        assert_eq!(table[1], 0x7707_3096);
    }

    #[test]
    fn crc32_cached_table_matches_builder_and_reuses_allocation() {
        let first = crc32_table();
        let second = crc32_table();
        assert!(
            std::ptr::eq(first, second),
            "Fix: CRC32 CPU reference must reuse its immutable lookup table instead of rebuilding it per call."
        );
        assert_eq!(*first, build_table());
    }

    #[test]
    fn crc32_program_is_single_primitive_region() {
        let program = crc32_program("input", "out", 3);
        assert_eq!(program.entry().len(), 1);
        match &program.entry()[0] {
            Node::Region { generator, .. } => assert_eq!(generator.as_str(), CRC32_OP_ID),
            other => panic!("expected primitive CRC32 region, got {other:?}"),
        }
    }

    #[test]
    fn crc32_chunk_program_declares_parallel_summary_outputs() {
        let chunk_size = NonZeroU32::new(16).expect("Fix: test chunk size must be non-zero.");
        let program = crc32_chunk_program("input", "out", 33, chunk_size);
        assert_eq!(program.entry().len(), 1);
        match &program.entry()[0] {
            Node::Region { generator, .. } => assert_eq!(generator.as_str(), CRC32_CHUNK_OP_ID),
            other => panic!("expected primitive CRC32 chunk region, got {other:?}"),
        }
        assert_eq!(program.buffers()[1].count(), 6);
    }

    #[test]
    fn crc32_chunk_count_keeps_empty_input_reducible() {
        assert_eq!(crc32_chunk_count(0, 64), 1);
        assert_eq!(crc32_chunk_count(1, 64), 1);
        assert_eq!(crc32_chunk_count(64, 64), 1);
        assert_eq!(crc32_chunk_count(65, 64), 2);
        assert_eq!(crc32_chunk_output_words(65, 64), Some(4));
    }

    #[test]
    fn crc32_pair_reduce_reference_preserves_associative_map_reduce_shape() {
        let bytes = (0..1025)
            .map(|index| (index as u8).wrapping_mul(41).wrapping_add(3))
            .collect::<Vec<_>>();
        let chunks = bytes
            .chunks(64)
            .map(crc32_chunk)
            .collect::<Vec<Crc32Chunk>>();
        let mut reduced = chunks;
        while reduced.len() > 1 {
            reduced = crc32_pair_reduce_chunks(&reduced)
                .expect("Fix: generated CRC pair-reduce chunk lengths must not overflow.");
        }
        assert_eq!(reduced[0].len, bytes.len() as u64);
        assert_eq!(reduced[0].crc, crc32(&bytes));
    }

    #[test]
    fn crc32_packed_chunk_abi_round_trips_and_reduces() {
        let chunks = [
            crc32_chunk(b"abc"),
            crc32_chunk(b"defgh"),
            crc32_chunk(b"ijk"),
        ];
        let packed = crc32_pack_chunks_u32(&chunks)
            .expect("Fix: generated CRC32 chunks should fit packed u32 ABI.");
        assert_eq!(
            crc32_unpack_chunks_u32(&packed),
            Some(chunks.to_vec()),
            "Fix: CRC32 packed summary ABI must round-trip exactly."
        );

        let reduced_words = crc32_pair_reduce_chunk_words(&packed)
            .expect("Fix: packed CRC32 pair reduction should succeed.");
        let reduced = crc32_unpack_chunks_u32(&reduced_words)
            .expect("Fix: reduced CRC32 packed words should remain valid pairs.");
        assert_eq!(reduced.len(), 2);
        assert_eq!(
            reduced[0],
            crc32_combine_chunks(chunks[0], chunks[1])
                .expect("Fix: generated CRC32 chunk length should not overflow.")
        );
        assert_eq!(reduced[1], chunks[2]);
    }

    #[test]
    fn crc32_packed_chunk_abi_rejects_odd_words_and_large_lengths() {
        assert_eq!(
            crc32_unpack_chunks_u32(&[crc32(b"abc")]),
            None,
            "Fix: CRC32 packed summary ABI must reject odd word counts."
        );
        assert_eq!(
            crc32_pack_chunks_u32(&[Crc32Chunk {
                crc: 0,
                len: u64::from(u32::MAX) + 1,
            }]),
            None,
            "Fix: CRC32 packed summary ABI must reject chunk lengths that cannot execute in u32 IR."
        );
    }

    #[test]
    fn crc32_pair_reduce_program_declares_single_packed_output() {
        let pair_count = NonZeroU32::new(5).expect("Fix: pair count must be non-zero.");
        let program = crc32_pair_reduce_program("pairs", "reduced", pair_count);
        assert_eq!(program.entry().len(), 1);
        match &program.entry()[0] {
            Node::Region { generator, .. } => {
                assert_eq!(generator.as_str(), CRC32_PAIR_REDUCE_OP_ID)
            }
            other => panic!("expected primitive CRC32 pair-reduce region, got {other:?}"),
        }
        assert_eq!(program.buffers()[0].count(), 10);
        assert_eq!(program.buffers()[1].count(), 6);
    }

    #[test]
    fn crc32_map_reduce_plan_single_sources_dispatch_shapes() {
        let chunk_size = NonZeroU32::new(64).expect("Fix: test chunk size must be non-zero.");
        let plan = crc32_map_reduce_plan(1500, chunk_size)
            .expect("Fix: CRC32 generated plan should fit u32 shape accounting.");
        assert_eq!(plan.steps[0].kind, Crc32MapReduceStepKind::ChunkSummary);
        assert_eq!(plan.steps[0].input_items, 1500);
        assert_eq!(plan.steps[0].output_pairs, 24);
        assert_eq!(plan.steps[0].output_words, 48);
        assert_eq!(plan.steps[0].grid, [24, 1, 1]);

        let mut pairs = plan.steps[0].output_pairs;
        for step in plan.steps.iter().skip(1) {
            assert_eq!(step.kind, Crc32MapReduceStepKind::PairReduce);
            assert_eq!(step.input_items, pairs);
            assert_eq!(step.input_words, pairs * 2);
            assert_eq!(step.output_pairs, pairs.div_ceil(2));
            assert_eq!(step.output_words, step.output_pairs * 2);
            assert_eq!(step.grid, [step.output_pairs, 1, 1]);
            pairs = step.output_pairs;
        }
        assert_eq!(pairs, 1);
    }

    #[test]
    fn crc32_update_helper_masks_high_input_bits() {
        let nodes = crc32_update_byte_nodes("crc", "bit", Expr::u32(0xFFFF_FF61));
        let rendered = format!("{nodes:?}");
        assert!(
            rendered.contains("255") || rendered.contains("0xFF"),
            "Fix: shared CRC update helper must mask high input bits before updating CRC: {rendered}"
        );
    }

    #[test]
    fn crc32_combine_matches_direct_crc_for_every_split() {
        let bytes = b"vyre resident cuda crc block scan reduction";
        for split in 0..=bytes.len() {
            let left = crc32(&bytes[..split]);
            let right = crc32(&bytes[split..]);
            assert_eq!(
                crc32_combine(left, right, (bytes.len() - split) as u64),
                crc32(bytes),
                "Fix: CRC32 chunk combine must match direct CRC at split {split}."
            );
        }
    }

    #[test]
    fn crc32_chunk_combine_is_associative_for_generated_inputs() {
        for len in [0usize, 1, 2, 3, 7, 31, 128, 1024] {
            let bytes = (0..len)
                .map(|index| (index as u8).wrapping_mul(37).wrapping_add(19))
                .collect::<Vec<_>>();
            let a_end = len / 3;
            let b_end = (len * 2) / 3;
            let a = crc32_chunk(&bytes[..a_end]);
            let b = crc32_chunk(&bytes[a_end..b_end]);
            let c = crc32_chunk(&bytes[b_end..]);

            let left_grouped = crc32_combine_chunks(
                crc32_combine_chunks(a, b)
                    .expect("Fix: generated CRC chunk length must not overflow."),
                c,
            )
            .expect("Fix: generated CRC chunk length must not overflow.");
            let right_grouped = crc32_combine_chunks(
                a,
                crc32_combine_chunks(b, c)
                    .expect("Fix: generated CRC chunk length must not overflow."),
            )
            .expect("Fix: generated CRC chunk length must not overflow.");

            assert_eq!(left_grouped, right_grouped);
            assert_eq!(left_grouped.len, len as u64);
            assert_eq!(left_grouped.crc, crc32(&bytes));
        }
    }

    #[test]
    fn crc32_generated_map_reduce_partitions_match_direct_crc() {
        let mut assertions = 0usize;
        for seed in 0u32..8192 {
            let len =
                (seed.wrapping_mul(1_103_515_245).rotate_left(seed & 15) ^ 0x9E37_79B9) % 1537;
            let chunk_size = (seed.wrapping_mul(37) ^ seed.rotate_left(5)) % 127 + 1;
            let chunk_size =
                NonZeroU32::new(chunk_size).expect("Fix: generated chunk size must be non-zero.");
            let mut state = seed ^ 0xA5A5_5A5A;
            let bytes = (0..len)
                .map(|index| {
                    state = state
                        .wrapping_mul(1_664_525)
                        .wrapping_add(1_013_904_223)
                        .rotate_left(index & 17);
                    (state ^ index.wrapping_mul(0x045D_9F3B)) as u8
                })
                .collect::<Vec<_>>();
            let plan = crc32_map_reduce_plan(len, chunk_size)
                .expect("Fix: generated CRC32 map-reduce plan must fit u32 accounting.");
            let mut chunks = if bytes.is_empty() {
                vec![crc32_chunk(&[])]
            } else {
                bytes
                    .chunks(chunk_size.get() as usize)
                    .map(crc32_chunk)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                chunks.len() as u32,
                plan.steps[0].output_pairs,
                "Fix: generated CRC32 chunk count drifted at seed {seed}."
            );

            let mut expected_pair_reduce_steps = 0usize;
            while chunks.len() > 1 {
                chunks = crc32_pair_reduce_chunks(&chunks)
                    .expect("Fix: generated CRC32 chunk lengths must not overflow.");
                expected_pair_reduce_steps += 1;
            }

            assert_eq!(
                plan.steps.len(),
                expected_pair_reduce_steps + 1,
                "Fix: generated CRC32 dispatch plan must mirror pair-reduce depth at seed {seed}."
            );
            assert_eq!(
                chunks[0].len,
                u64::from(len),
                "Fix: generated CRC32 reduction lost byte length at seed {seed}."
            );
            assert_eq!(
                chunks[0].crc,
                crc32(&bytes),
                "Fix: generated CRC32 map-reduce result must equal direct serial CRC at seed {seed}."
            );
            assertions += 1;
        }
        assert_eq!(assertions, 8192);
    }

    #[test]
    fn crc32_chunk_combine_rejects_length_overflow() {
        let left = Crc32Chunk {
            len: u64::MAX,
            crc: crc32(b"left"),
        };
        let right = Crc32Chunk {
            len: 1,
            crc: crc32(b"r"),
        };

        assert_eq!(
            crc32_combine_chunks(left, right),
            None,
            "Fix: CRC32 chunk summaries must reject impossible combined lengths instead of wrapping."
        );
    }
}

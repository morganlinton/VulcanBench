//! GPU-native common-subexpression elimination on the encoded arena.
//!
//! Two passes operating on the canonical 5-buffer arena:
//!
//! 1. **structural_hash**  -  level-parallel bottom-up. Each Expr's hash
//!    is `mix(kind, payload, child_hashes...)` so that two arena rows
//!    representing the same syntactic Expr collapse to the same hash.
//!    Runs in a single fused kernel with workgroup-scope barriers
//!    between levels (same single-workgroup pattern as the fused
//!    const-fold). One dispatch.
//!
//! 2. **canonical_id**  -  for each hash bucket, the smallest expr id
//!    with that hash wins. Implemented as a length-`2*expr_count`
//!    open-addressed direct map with atomic-min on the value slot.
//!    Linear probing on hash collision. Capacity > 2× ensures load
//!    factor ≤ 0.5 and bounded probe length. One dispatch.
//!
//! After both passes, `canonical[i]` gives the smallest expr id
//! structurally equivalent to `i`. Identity (`canonical[i] == i`)
//! means `i` is its own canonical; otherwise `i` is a duplicate of
//! `canonical[i]`. The IR rewrite that consumes `canonical[]` is in
//! `apply_cse_canonicals`.
//!
//! Hash function: a Fowler–Noll–Vo–style mix over kind / op / child
//! hashes / payload. 32-bit; collision probability over 5k-expr
//! arenas is ~0.3% per arena. Future versions can promote to 64-bit
//! for stronger guarantees; the architecture here doesn't change.

use std::sync::Arc;

use rustc_hash::FxHashMap;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};
use vyre_primitives::hash::fnv1a::{fnv1a32_initial_expr, fnv1a32_mix_word_expr};

use crate::dispatch_buffers::{
    decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes, write_zero_bytes,
};

use super::dispatcher::{DispatchError, OptimizerDispatcher};
use super::encode::EncodeError;
use super::expr_arena::{encode_expr_arena, expr_kind, ExprArenaEncoding};

#[derive(Debug, Default)]
struct CseKernelScratch {
    hash_inputs: Vec<Vec<u8>>,
    canonical_inputs: Vec<Vec<u8>>,
    max_depth: [u32; 1],
    hash_words: Vec<u32>,
    table_init_words: Vec<u32>,
}

/// Errors surfaced by `gpu_cse_canonicals`.
#[derive(Debug)]
pub enum CseError {
    /// Encoder did not accept the input shape.
    Encode(EncodeError),
    /// Dispatcher rejected or failed.
    Dispatch(DispatchError),
}

impl std::fmt::Display for CseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "gpu_cse encode error: {err:?}"),
            Self::Dispatch(err) => write!(f, "gpu_cse dispatch error: {err}"),
        }
    }
}

impl std::error::Error for CseError {}

/// Workgroup size used by both CSE kernels. Matches the rest of the
/// optimizer keystone for cache coherence on per-Expr passes.
pub const WORKGROUP_X: u32 = 256;

/// Capacity multiplier for the canonical-id direct-map. Must stay
/// strictly above `1` so the table's load factor stays bounded; `2`
/// keeps probe length to a small constant in expectation.
pub const CANONICAL_TABLE_MULT: u32 = 2;

/// Lookup contract for CSE canonical ids.
///
/// Dense GPU CSE returns `canonical[id]` for every arena id. Resident
/// CUDA pipelines can instead read back only non-identity pairs from a
/// device-side compaction kernel; consumers should not care which
/// representation produced the lookup.
pub trait CanonicalLookup {
    /// True when no non-identity canonical mappings exist.
    fn is_empty(&self) -> bool;

    /// Return the canonical id for `id`, defaulting to identity.
    fn canonical_of(&self, id: u32) -> u32;
}

impl CanonicalLookup for [u32] {
    fn is_empty(&self) -> bool {
        <[u32]>::is_empty(self)
    }

    fn canonical_of(&self, id: u32) -> u32 {
        self.get(id as usize).copied().unwrap_or(id)
    }
}

/// Sparse canonical map decoded from `(expr_id, canonical_id)` pairs.
#[derive(Debug, Clone, Default)]
pub struct SparseCanonicalMap {
    expr_count: u32,
    overrides: FxHashMap<u32, u32>,
}

impl SparseCanonicalMap {
    /// Decode compacted pair words emitted by
    /// [`build_canonical_delta_compact_program`].
    pub fn from_compacted_pair_words(
        expr_count: u32,
        pair_count: u32,
        pair_words: &[u32],
        context: &str,
    ) -> Result<Self, DispatchError> {
        let count = pair_count as usize;
        let expected_words = count.checked_mul(2).ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: {context} compact canonical pair count overflows usize: {pair_count}."
            ))
        })?;
        if pair_words.len() != expected_words {
            return Err(DispatchError::BadInputs(format!(
                "Fix: {context} compact canonical expected {expected_words} pair word(s) for {pair_count} pair(s), got {}.",
                pair_words.len()
            )));
        }

        let mut overrides = FxHashMap::default();
        overrides.try_reserve(count).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: reserve {context} compact canonical map for {count} pair(s): {error}."
            ))
        })?;
        for pair in pair_words.chunks_exact(2) {
            let id = pair[0];
            let canonical = pair[1];
            if id >= expr_count || canonical >= expr_count {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: {context} compact canonical pair ({id}, {canonical}) exceeds expr_count {expr_count}."
                )));
            }
            if canonical > id {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: {context} compact canonical pair ({id}, {canonical}) is not monotonic; canonical ids must be <= source ids."
                )));
            }
            if canonical == id {
                continue;
            }
            if let Some(previous) = overrides.insert(id, canonical) {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: {context} compact canonical emitted duplicate source id {id} with values {previous} and {canonical}."
                )));
            }
        }

        Ok(Self {
            expr_count,
            overrides,
        })
    }

    /// Number of non-identity canonical overrides.
    #[must_use]
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }
}

impl CanonicalLookup for SparseCanonicalMap {
    fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    fn canonical_of(&self, id: u32) -> u32 {
        if id >= self.expr_count {
            return id;
        }
        self.overrides.get(&id).copied().unwrap_or(id)
    }
}

/// Run CSE analysis. Returns a `canonical` vector where `canonical[i]`
/// is the smallest expr id structurally equivalent to `i`. Identity
/// (`canonical[i] == i`) means `i` is its own canonical. Use the
/// returned vector with `apply_cse_canonicals` to rewrite the
/// Program.
pub fn gpu_cse_canonicals(
    program: &Program,
    dispatcher: &dyn OptimizerDispatcher,
) -> Result<(ExprArenaEncoding, Vec<u32>), CseError> {
    let arena = encode_expr_arena(program).map_err(CseError::Encode)?;
    let n = arena.expr_count;
    if n == 0 {
        return Ok((arena, Vec::new()));
    }
    let mut scratch = CseKernelScratch::default();
    let mut canonical = Vec::with_capacity(n as usize);
    run_cse_kernels_with_scratch_into(&arena, dispatcher, &mut scratch, &mut canonical)
        .map_err(CseError::Dispatch)?;
    Ok((arena, canonical))
}

#[cfg(test)]
fn run_cse_kernels_into(
    arena: &ExprArenaEncoding,
    dispatcher: &dyn OptimizerDispatcher,
    canonical: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = CseKernelScratch::default();
    run_cse_kernels_with_scratch_into(arena, dispatcher, &mut scratch, canonical)
}

fn run_cse_kernels_with_scratch_into(
    arena: &ExprArenaEncoding,
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut CseKernelScratch,
    canonical: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let n = arena.expr_count;
    let words = n as usize;
    let state_bytes = words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: gpu_cse state byte count overflows usize for expr_count={n}."
            ))
        })?;

    // ---- Pass A: structural hash ----------------------------------
    let hash_program = build_structural_hash_program(n, arena.max_depth.saturating_add(1).max(1));
    scratch.max_depth[0] = arena.max_depth;
    ensure_input_slots(&mut scratch.hash_inputs, 7);
    write_u32_slice_le_bytes(&mut scratch.hash_inputs[0], &arena.kinds);
    write_u32_slice_le_bytes(&mut scratch.hash_inputs[1], &arena.arg0);
    write_u32_slice_le_bytes(&mut scratch.hash_inputs[2], &arena.arg1);
    write_u32_slice_le_bytes(&mut scratch.hash_inputs[3], &arena.arg2);
    write_u32_slice_le_bytes(&mut scratch.hash_inputs[4], &arena.depths);
    write_u32_slice_le_bytes(&mut scratch.hash_inputs[5], &scratch.max_depth);
    write_zero_bytes(&mut scratch.hash_inputs[6], state_bytes);
    let hash_outputs = dispatcher.dispatch(&hash_program, &scratch.hash_inputs, Some([1, 1, 1]))?;
    if hash_outputs.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: gpu_cse hash dispatch expected exactly one hash output, got {}.",
            hash_outputs.len()
        )));
    }
    decode_u32_output_exact(
        &hash_outputs[0],
        words,
        "gpu_cse hash",
        &mut scratch.hash_words,
    )?;

    // ---- Pass B: canonical-id direct-map --------------------------
    let capacity = (n.saturating_mul(CANONICAL_TABLE_MULT)).max(2);
    let canonical_program = build_canonical_id_program(n, capacity);
    // Initial state for table_canonical: u32::MAX (empty marker).
    scratch.table_init_words.clear();
    scratch.table_init_words.resize(capacity as usize, u32::MAX);
    // 7 inputs: hash, canonical (RW scratch), table_canonical (RW dummy),
    // arena_kinds, arena_arg0, arena_arg1, arena_arg2. The four arena
    // buffers supply the structural tuple comparison that makes the
    // hash-equality pre-filter sound; without them a 32-bit hash collision
    // would silently merge non-equivalent exprs.
    ensure_input_slots(&mut scratch.canonical_inputs, 7);
    scratch.canonical_inputs[0].clear();
    scratch.canonical_inputs[0].extend_from_slice(&hash_outputs[0]);
    write_zero_bytes(&mut scratch.canonical_inputs[1], state_bytes);
    write_u32_slice_le_bytes(&mut scratch.canonical_inputs[2], &scratch.table_init_words);
    write_u32_slice_le_bytes(&mut scratch.canonical_inputs[3], &arena.kinds);
    write_u32_slice_le_bytes(&mut scratch.canonical_inputs[4], &arena.arg0);
    write_u32_slice_le_bytes(&mut scratch.canonical_inputs[5], &arena.arg1);
    write_u32_slice_le_bytes(&mut scratch.canonical_inputs[6], &arena.arg2);
    let canonical_outputs = dispatcher.dispatch(
        &canonical_program,
        &scratch.canonical_inputs,
        // Grid covers expr_count threads (n_workgroups = ceil(n/256))
        Some([(n + WORKGROUP_X - 1) / WORKGROUP_X, 1, 1]),
    )?;
    if canonical_outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: gpu_cse canonical dispatch expected at least one canonical output, got {}.",
            canonical_outputs.len()
        )));
    }
    decode_u32_output_exact(&canonical_outputs[0], words, "gpu_cse canonical", canonical)
}

/// Build the structural-hash analysis Program. Single-workgroup,
/// fused level-loop with workgroup-scope barriers. Each thread
/// strides over expr ids in chunks of `WORKGROUP_X` per level.
///
/// Buffer layout:
///   0: arena_kinds (RO)
///   1: arena_arg0  (RO)
///   2: arena_arg1  (RO)
///   3: arena_arg2  (RO)
///   4: arena_depths (RO)
///   5: max_depth_buf (RO; single u32)
///   6: hash (RW; init zeros)
#[must_use]
pub fn build_structural_hash_program(expr_count: u32, max_depth_iter_cap: u32) -> Program {
    let buffers = vec![
        BufferDecl::storage("arena_kinds", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg0", 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg1", 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg2", 3, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_depths", 4, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("max_depth_buf", 5, BufferAccess::ReadOnly, DataType::U32)
            .with_count(1),
        BufferDecl::storage("hash", 6, BufferAccess::ReadWrite, DataType::U32)
            .with_count(expr_count.max(1)),
    ];

    let chunk_cap = (expr_count + WORKGROUP_X - 1) / WORKGROUP_X;

    // Per-Expr body: structural-hash mixer. Critical invariant:
    // mix child HASHES (h0/h1/h2), never raw arg slots (a0/a1/a2)
    // for parent kinds  -  raw args carry arena-position-dependent
    // child ids that break canonical-equivalence across duplicates.
    // For leaves the raw a0/a1/a2 carry the actual payload (literal
    // value, name id, axis, buffer name id) and ARE structural.
    let mix = |var_name: &str| -> Vec<Node> {
        vec![Node::assign(
            "h",
            fnv1a32_mix_word_expr(Expr::var("h"), Expr::var(var_name)),
        )]
    };
    let per_expr_body = vec![
        Node::let_bind("kind", Expr::load("arena_kinds", Expr::var("i"))),
        Node::let_bind("a0", Expr::load("arena_arg0", Expr::var("i"))),
        Node::let_bind("a1", Expr::load("arena_arg1", Expr::var("i"))),
        Node::let_bind("a2", Expr::load("arena_arg2", Expr::var("i"))),
        // Child hashes (the post-order encoding guarantees children's
        // hashes are already written by the time the parent's level
        // runs). For leaves these reads are harmless (a0/a1/a2 carry
        // payloads that may index outside the arena, but `hash` was
        // zero-initialized so out-of-bounds reads return 0 inside the
        // backend's CSR-bounds clamp; the leaf branch ignores h0/h1/h2
        // anyway).
        Node::let_bind("h0", Expr::load("hash", Expr::var("a0"))),
        Node::let_bind("h1", Expr::load("hash", Expr::var("a1"))),
        Node::let_bind("h2", Expr::load("hash", Expr::var("a2"))),
        // Mix kind first (the family discriminator).
        Node::let_bind("h", fnv1a32_initial_expr()),
        Node::assign(
            "h",
            fnv1a32_mix_word_expr(Expr::var("h"), Expr::var("kind")),
        ),
        // Leaves with a payload in a0: literals, vars, buf_len,
        // invocation/workgroup/local id (axis lives in a0).
        Node::if_then(
            Expr::or(
                Expr::or(
                    Expr::or(
                        Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::LIT_U32)),
                        Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::LIT_I32)),
                    ),
                    Expr::or(
                        Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::LIT_F32)),
                        Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::LIT_BOOL)),
                    ),
                ),
                Expr::or(
                    Expr::or(
                        Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::VAR)),
                        Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::BUF_LEN)),
                    ),
                    Expr::or(
                        Expr::or(
                            Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::INVOCATION_ID)),
                            Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::WORKGROUP_ID)),
                        ),
                        Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::LOCAL_ID)),
                    ),
                ),
            ),
            mix("a0"),
        ),
        // BIN_OP: a0 = op_tag (structural), a1/a2 = child ids (NOT
        // structural). Mix op_tag + child hashes in position order.
        // (Commutative-friendly mixing was tried and reverted  -  the
        // extra Selects + tag-flag chain doubled the per-Expr kernel
        // runtime and the speculative CSE gain didn't justify it.)
        Node::if_then(Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::BIN_OP)), {
            let mut body = mix("a0");
            body.extend(mix("h1"));
            body.extend(mix("h2"));
            body
        }),
        // UN_OP: a0 = op_tag, a1 = child id.
        Node::if_then(Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::UN_OP)), {
            let mut body = mix("a0");
            body.extend(mix("h1"));
            body
        }),
        // LOAD: a0 = buffer name id (structural), a1 = index Expr id.
        Node::if_then(Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::LOAD)), {
            let mut body = mix("a0");
            body.extend(mix("h1"));
            body
        }),
        // SELECT, FMA: 3 child ids in a0/a1/a2; payload-free.
        Node::if_then(
            Expr::or(
                Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::SELECT)),
                Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::FMA)),
            ),
            {
                let mut body = mix("h0");
                body.extend(mix("h1"));
                body.extend(mix("h2"));
                body
            },
        ),
        // SUBGROUP_LOCAL_ID / SUBGROUP_SIZE: payload-free; the kind
        // mix above is sufficient.
        Node::store("hash", Expr::var("i"), Expr::var("h")),
    ];

    let chunk_loop = Node::loop_for(
        "chunk",
        Expr::u32(0),
        Expr::u32(chunk_cap.max(1)),
        vec![
            Node::let_bind(
                "i",
                Expr::add(
                    Expr::gid_x(),
                    Expr::mul(Expr::var("chunk"), Expr::u32(WORKGROUP_X)),
                ),
            ),
            Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
                vec![
                    Node::let_bind("my_depth", Expr::load("arena_depths", Expr::var("i"))),
                    Node::if_then(
                        Expr::eq(Expr::var("my_depth"), Expr::var("level")),
                        per_expr_body,
                    ),
                ],
            ),
        ],
    );

    let outer = Node::loop_for(
        "level",
        Expr::u32(0),
        Expr::u32(max_depth_iter_cap.max(1)),
        vec![
            Node::let_bind("md", Expr::load("max_depth_buf", Expr::u32(0))),
            Node::if_then(
                Expr::le(Expr::var("level"), Expr::var("md")),
                vec![chunk_loop],
            ),
            Node::Barrier {
                ordering: vyre_foundation::MemoryOrdering::SeqCst,
            },
        ],
    );

    Program::wrapped(buffers, [WORKGROUP_X, 1, 1], vec![outer])
}

/// Build the canonical-id Program. Single dispatch: each thread `i`
/// computes `canonical[i]` by brute-force scanning `0..i` for the
/// smallest `j` that is structurally identical to `i`.
///
/// Structural identity requires BOTH the hash pre-filter AND a full
/// `(kind, arg0, arg1, arg2)` tuple comparison. The hash alone is a
/// 32-bit FNV value whose collision probability grows with arena size
/// (birthday bound ~0.3% per 5k-expr arena); relying on hash equality
/// alone would silently merge non-equivalent exprs (miscompile). The
/// tuple check is the definitive correctness guard; the hash serves
/// only as a fast-reject to reduce wasted tuple reads.
///
/// Buffer layout:
///   0: hash          (RO)
///   1: canonical     (RW)
///   2: table_canonical (RW; init `u32::MAX`; used as a dummy RW
///      binding so backends do not prune the buffer slot)
///   3: arena_kinds   (RO)
///   4: arena_arg0    (RO)
///   5: arena_arg1    (RO)
///   6: arena_arg2    (RO)
#[must_use]
pub fn build_canonical_id_program(expr_count: u32, capacity: u32) -> Program {
    let buffers = vec![
        BufferDecl::storage("hash", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("canonical", 1, BufferAccess::ReadWrite, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("table_canonical", 2, BufferAccess::ReadWrite, DataType::U32)
            .with_count(capacity.max(1)),
        // Structural tuple buffers: hash collision alone must never
        // declare two exprs equivalent. These four buffers supply the
        // definitive (kind, arg0, arg1, arg2) tuple comparison.
        BufferDecl::storage("arena_kinds", 3, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg0", 4, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg1", 5, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg2", 6, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
    ];

    // Per-thread body: brute-force scan 0..i.
    // The post-order encoding ensures children appear before parents,
    // so structurally-equivalent siblings always have a prior candidate
    // at a smaller index.
    //
    // Equivalence predicate: hash pre-filter (fast reject) THEN full
    // structural tuple check (correctness gate). Both must hold before
    // `found_canonical` is updated.
    let body = vec![
        Node::let_bind("i", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
            vec![
                Node::let_bind("my_hash", Expr::load("hash", Expr::var("i"))),
                // Load this thread's structural tuple once (avoids
                // re-reading the same arena row on every inner iteration).
                Node::let_bind("my_kind", Expr::load("arena_kinds", Expr::var("i"))),
                Node::let_bind("my_a0", Expr::load("arena_arg0", Expr::var("i"))),
                // Child structural HASHES, not raw arg1/arg2. For BIN_OP /
                // UN_OP / LOAD the arg1/arg2 slots hold arena child indices,
                // which differ between structurally-equal duplicates that sit
                // at different positions; a raw index comparison would reject
                // those true duplicates (the hash mixer mixes child hashes for
                // exactly this reason). For every leaf kind arg1 = arg2 = 0, so
                // hash[0] == hash[0] holds trivially and leaf identity is
                // decided by `my_kind` + `my_a0` below.
                Node::let_bind(
                    "my_h1",
                    Expr::load("hash", Expr::load("arena_arg1", Expr::var("i"))),
                ),
                Node::let_bind(
                    "my_h2",
                    Expr::load("hash", Expr::load("arena_arg2", Expr::var("i"))),
                ),
                Node::let_bind("found_canonical", Expr::var("i")),
                Node::loop_for(
                    "j",
                    Expr::u32(0),
                    Expr::var("i"),
                    vec![
                        Node::let_bind("their_hash", Expr::load("hash", Expr::var("j"))),
                        // Gate 1: hash pre-filter. Mismatched hashes
                        // structurally different exprs almost always.
                        // Gate 2: structural confirmation. Hash equality
                        // alone is not structural identity because two
                        // distinct exprs can share a 32-bit hash value
                        // (birthday collision). Confirm kind, arg0 (op tag /
                        // payload), and BOTH child structural hashes match
                        // before declaring `j` canonical for `i`. The child
                        // hashes (not raw child indices) are what make
                        // position-independent duplicates compare equal.
                        // Gate 3: only take the first (smallest-index)
                        // match by checking `found_canonical == i`.
                        Node::if_then(
                            Expr::and(
                                Expr::and(
                                    Expr::and(
                                        Expr::and(
                                            Expr::and(
                                                Expr::eq(
                                                    Expr::var("their_hash"),
                                                    Expr::var("my_hash"),
                                                ),
                                                Expr::eq(
                                                    Expr::load("arena_kinds", Expr::var("j")),
                                                    Expr::var("my_kind"),
                                                ),
                                            ),
                                            Expr::eq(
                                                Expr::load("arena_arg0", Expr::var("j")),
                                                Expr::var("my_a0"),
                                            ),
                                        ),
                                        Expr::eq(
                                            Expr::load(
                                                "hash",
                                                Expr::load("arena_arg1", Expr::var("j")),
                                            ),
                                            Expr::var("my_h1"),
                                        ),
                                    ),
                                    Expr::eq(
                                        Expr::load(
                                            "hash",
                                            Expr::load("arena_arg2", Expr::var("j")),
                                        ),
                                        Expr::var("my_h2"),
                                    ),
                                ),
                                Expr::eq(Expr::var("found_canonical"), Expr::var("i")),
                            ),
                            vec![Node::assign("found_canonical", Expr::var("j"))],
                        ),
                    ],
                ),
                Node::store("canonical", Expr::var("i"), Expr::var("found_canonical")),
                // Touch table_canonical so it's a true RW binding the
                // backend can see (avoid unused-buffer prune); store
                // identity so the buffer carries no semantics.
                Node::if_then(
                    Expr::lt(Expr::var("i"), Expr::u32(capacity)),
                    vec![Node::store(
                        "table_canonical",
                        Expr::var("i"),
                        Expr::var("found_canonical"),
                    )],
                ),
            ],
        ),
    ];

    Program::wrapped(buffers, [WORKGROUP_X, 1, 1], body)
}

/// Build a compact readback Program for CSE canonical ids.
///
/// Buffer layout:
///   0: canonical (RO)
///   1: canonical_delta (RW), where word 0 is an atomic pair count and
///      words `1 + 2*k .. 3 + 2*k` are `(expr_id, canonical_id)`.
#[must_use]
pub fn build_canonical_delta_compact_program(expr_count: u32) -> Program {
    let delta_words = expr_count.saturating_mul(2).saturating_add(1).max(1);
    let buffers = vec![
        BufferDecl::storage("canonical", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("canonical_delta", 1, BufferAccess::ReadWrite, DataType::U32)
            .with_count(delta_words),
    ];
    let body = vec![
        Node::let_bind("i", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
            vec![
                Node::let_bind("canonical_id", Expr::load("canonical", Expr::var("i"))),
                Node::if_then(
                    Expr::ne(Expr::var("canonical_id"), Expr::var("i")),
                    vec![
                        Node::let_bind(
                            "slot",
                            Expr::atomic_add("canonical_delta", Expr::u32(0), Expr::u32(1)),
                        ),
                        Node::let_bind(
                            "base",
                            Expr::add(Expr::u32(1), Expr::mul(Expr::var("slot"), Expr::u32(2))),
                        ),
                        Node::store("canonical_delta", Expr::var("base"), Expr::var("i")),
                        Node::store(
                            "canonical_delta",
                            Expr::add(Expr::var("base"), Expr::u32(1)),
                            Expr::var("canonical_id"),
                        ),
                    ],
                ),
            ],
        ),
    ];

    Program::wrapped(buffers, [WORKGROUP_X, 1, 1], body)
}

/// Apply `canonical[i]` to rewrite `program`. Replaces every
/// `Node::Let` whose value-Expr is a CSE duplicate with
/// `Expr::Var(orig_name)`, where `orig_name` is the first binding in
/// the same scope that produced the canonical expression.
///
/// Delegates to [`apply_cse_let_dedupe_with_lookup`], which implements
/// the correct let-level rewrite using `arena.node_top_level_exprs` to
/// correlate node walk order with arena expr ids. The two functions
/// must produce identical results for the let-level case; use this
/// entry point when you have a dense `canonical` slice.
pub fn apply_cse_canonicals(
    program: &Program,
    arena: &ExprArenaEncoding,
    canonical: &[u32],
) -> Program {
    apply_cse_let_dedupe_with_lookup(program, arena, canonical)
}

/// Apply a let-level CSE rewrite: when an entire `Node::Let { name,
/// value: V }` has a value-Expr structurally equivalent to an earlier
/// Let's value in the SAME scope, replace `V` with `Expr::Var(orig)`.
/// This is the safe-by-construction subset of CSE rewrite  -  no
/// cross-scope hoisting needed.
///
/// Walks the program in the same DFS order the arena encoder uses.
/// Tracks a per-scope `expr_id → let_name` map; entering a new scope
/// (If/Loop/Block branches) pushes a fresh map so duplicates only
/// dedupe against same-scope siblings.
pub fn apply_cse_let_dedupe(
    program: &Program,
    arena: &ExprArenaEncoding,
    canonical: &[u32],
) -> Program {
    apply_cse_let_dedupe_with_lookup(program, arena, canonical)
}

/// Sparse/dense-agnostic variant of [`apply_cse_let_dedupe`].
pub fn apply_cse_let_dedupe_with_lookup<C: CanonicalLookup + ?Sized>(
    program: &Program,
    arena: &ExprArenaEncoding,
    canonical: &C,
) -> Program {
    if canonical.is_empty() {
        return program.clone();
    }
    let mut walker = LetDedupeWalker {
        canonical,
        node_index: 1, // node_top_level_exprs[0] is the synthetic ROOT
        node_top_level_exprs: &arena.node_top_level_exprs,
    };

    let body: Vec<Node> = match program.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };
    let new_body = walker.rewrite_scope(&body);

    let new_entry = match program.entry() {
        [Node::Region {
            generator,
            source_region,
            ..
        }] => vec![Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(new_body),
        }],
        _ => new_body,
    };
    program.with_rewritten_entry(new_entry)
}

struct LetDedupeWalker<'a, C: CanonicalLookup + ?Sized> {
    canonical: &'a C,
    /// Mirrors the encoder's `node_top_level_exprs` allocation order.
    /// Increments by exactly one per `encode_node` call.
    node_index: usize,
    node_top_level_exprs: &'a [Vec<u32>],
}

impl<C: CanonicalLookup + ?Sized> LetDedupeWalker<'_, C> {
    fn rewrite_scope(&mut self, body: &[Node]) -> Vec<Node> {
        let prefix_len = super::encode::reachable_prefix_len(body);
        let mut out = Vec::with_capacity(prefix_len);
        // Per-scope map: expr_id of a Let's value → that Let's name.
        // Two Let nodes in the SAME scope whose values are CSE-
        // equivalent will both have canonical[their_value_id] equal
        // to the earlier one's value id.
        let mut expr_to_name: rustc_hash::FxHashMap<u32, Ident> = rustc_hash::FxHashMap::default();
        for node in &body[..prefix_len] {
            out.push(self.rewrite_node(node, &mut expr_to_name));
        }
        out
    }

    fn rewrite_node(
        &mut self,
        node: &Node,
        expr_to_name: &mut rustc_hash::FxHashMap<u32, Ident>,
    ) -> Node {
        // Allocate this node's slot in lockstep with the encoder.
        let node_idx = self.node_index;
        self.node_index += 1;

        let rewritten = match node {
            Node::Let { name, value: _ } => {
                let top_id = self
                    .node_top_level_exprs
                    .get(node_idx)
                    .and_then(|v| v.first())
                    .copied();
                if let Some(top_id) = top_id {
                    let canon = self.canonical.canonical_of(top_id);
                    if canon != top_id {
                        if let Some(orig_name) = expr_to_name.get(&canon).cloned() {
                            // Duplicate: replace value with `Var(orig)`.
                            // Don't update the map  -  `name` is bound to
                            // the same value, but we keep the original
                            // canonical mapping for further duplicates.
                            return Node::let_bind(name.clone(), Expr::var(orig_name));
                        }
                    }
                    // First occurrence of this canonical value at this
                    // scope. Record the binding so siblings can dedupe.
                    expr_to_name.insert(canon, name.clone());
                }
                node.clone()
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let new_then = self.rewrite_scope(then);
                let new_otherwise = self.rewrite_scope(otherwise);
                Node::if_then_else(cond.clone(), new_then, new_otherwise)
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                let new_body = self.rewrite_scope(body);
                Node::loop_for(var.clone(), from.clone(), to.clone(), new_body)
            }
            Node::Block(body) => Node::Block(self.rewrite_scope(body)),
            Node::Region {
                generator,
                source_region,
                body,
            } => Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: Arc::new(self.rewrite_scope(body.as_slice())),
            },
            // No-Expr-payload Nodes pass through. Assign-style Nodes
            // are intentionally not deduplicated  -  they reassign an
            // existing binding, so the value substitution would change
            // observable behaviour at runtime.
            other => other.clone(),
        };
        rewritten
    }
}



#[cfg(test)]

mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use std::cell::RefCell;

    struct CseDispatcher {
        outputs: RefCell<Vec<Vec<Vec<u8>>>>,
    }

    impl OptimizerDispatcher for CseDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    fn one_expr_arena() -> ExprArenaEncoding {
        ExprArenaEncoding {
            expr_count: 1,
            kinds: vec![expr_kind::LIT_U32],
            arg0: vec![0],
            arg1: vec![0],
            arg2: vec![0],
            depths: vec![0],
            max_depth: 0,
            ..ExprArenaEncoding::default()
        }
    }

    #[test]
    fn structural_hash_program_compiles_to_program() {
        let p = build_structural_hash_program(8, 4);
        assert!(p.buffers().iter().any(|b| b.name() == "hash"));
        assert!(p.buffers().iter().any(|b| b.name() == "max_depth_buf"));
    }

    #[test]
    fn canonical_id_program_carries_table_buffer() {
        let p = build_canonical_id_program(8, 16);
        assert!(p.buffers().iter().any(|b| b.name() == "canonical"));
        assert!(p.buffers().iter().any(|b| b.name() == "table_canonical"));
        // Structural tuple buffers must be present so the kernel can
        // confirm hash-equal exprs are actually structurally identical.
        assert!(
            p.buffers().iter().any(|b| b.name() == "arena_kinds"),
            "canonical-id program must declare arena_kinds for structural tuple check"
        );
        assert!(
            p.buffers().iter().any(|b| b.name() == "arena_arg0"),
            "canonical-id program must declare arena_arg0 for structural tuple check"
        );
        assert!(
            p.buffers().iter().any(|b| b.name() == "arena_arg1"),
            "canonical-id program must declare arena_arg1 for structural tuple check"
        );
        assert!(
            p.buffers().iter().any(|b| b.name() == "arena_arg2"),
            "canonical-id program must declare arena_arg2 for structural tuple check"
        );
    }

    #[test]
    fn canonical_delta_compact_program_carries_sparse_output_buffer() {
        let p = build_canonical_delta_compact_program(8);
        assert!(p.buffers().iter().any(|b| b.name() == "canonical"));
        assert!(p.buffers().iter().any(|b| b.name() == "canonical_delta"));
    }

    #[test]
    fn sparse_canonical_map_defaults_identity_and_overrides_duplicates() {
        let map = SparseCanonicalMap::from_compacted_pair_words(
            8,
            2,
            &[3, 1, 7, 2],
            "test sparse canonical",
        )
        .expect("Fix: valid compact canonical pairs decode");
        assert_eq!(map.override_count(), 2);
        assert_eq!(map.canonical_of(0), 0);
        assert_eq!(map.canonical_of(3), 1);
        assert_eq!(map.canonical_of(7), 2);
    }

    #[test]
    fn sparse_canonical_map_rejects_malformed_pair_count() {
        let err =
            SparseCanonicalMap::from_compacted_pair_words(8, 2, &[3, 1], "test sparse canonical")
                .expect_err("compact canonical pair count must match pair words exactly");
        assert!(
            matches!(err, DispatchError::BadInputs(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn cse_kernels_decode_exact_canonical_into_reused_buffer() {
        let dispatcher = CseDispatcher {
            outputs: RefCell::new(vec![
                vec![u32_slice_to_le_bytes(&[123])],
                vec![u32_slice_to_le_bytes(&[0])],
            ]),
        };
        let mut canonical = Vec::with_capacity(4);
        let ptr = canonical.as_ptr();
        run_cse_kernels_into(&one_expr_arena(), &dispatcher, &mut canonical)
            .expect("Fix: dispatch succeeds");
        assert_eq!(canonical, vec![0]);
        assert_eq!(canonical.as_ptr(), ptr);
    }

    #[test]
    fn cse_kernels_with_scratch_reuse_dispatch_decode_and_output_storage() {
        let dispatcher = CseDispatcher {
            outputs: RefCell::new(vec![
                vec![u32_slice_to_le_bytes(&[123])],
                vec![u32_slice_to_le_bytes(&[0])],
                vec![u32_slice_to_le_bytes(&[123])],
                vec![u32_slice_to_le_bytes(&[0])],
            ]),
        };
        let arena = one_expr_arena();
        let mut scratch = CseKernelScratch::default();
        let mut canonical = Vec::with_capacity(1);

        run_cse_kernels_with_scratch_into(&arena, &dispatcher, &mut scratch, &mut canonical)
            .expect("Fix: dispatch succeeds");

        let hash_input_capacities = scratch
            .hash_inputs
            .iter()
            .map(Vec::capacity)
            .collect::<Vec<_>>();
        let canonical_input_capacities = scratch
            .canonical_inputs
            .iter()
            .map(Vec::capacity)
            .collect::<Vec<_>>();
        let hash_words_capacity = scratch.hash_words.capacity();
        let table_capacity = scratch.table_init_words.capacity();
        let canonical_capacity = canonical.capacity();

        run_cse_kernels_with_scratch_into(&arena, &dispatcher, &mut scratch, &mut canonical)
            .expect("Fix: dispatch succeeds");

        assert_eq!(
            scratch
                .hash_inputs
                .iter()
                .map(Vec::capacity)
                .collect::<Vec<_>>(),
            hash_input_capacities
        );
        assert_eq!(
            scratch
                .canonical_inputs
                .iter()
                .map(Vec::capacity)
                .collect::<Vec<_>>(),
            canonical_input_capacities
        );
        assert_eq!(scratch.hash_words.capacity(), hash_words_capacity);
        assert_eq!(scratch.table_init_words.capacity(), table_capacity);
        assert_eq!(canonical.capacity(), canonical_capacity);
        assert_eq!(canonical, vec![0]);
    }

    #[test]
    fn cse_rejects_extra_hash_outputs() {
        let dispatcher = CseDispatcher {
            outputs: RefCell::new(vec![vec![
                u32_slice_to_le_bytes(&[123]),
                u32_slice_to_le_bytes(&[0]),
            ]]),
        };
        let mut canonical = Vec::new();
        let err = run_cse_kernels_into(&one_expr_arena(), &dispatcher, &mut canonical)
            .expect_err("extra hash outputs must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn cse_rejects_trailing_canonical_bytes() {
        let dispatcher = CseDispatcher {
            outputs: RefCell::new(vec![
                vec![u32_slice_to_le_bytes(&[123])],
                vec![vec![0, 0, 0, 0, 1]],
            ]),
        };
        let mut canonical = Vec::new();
        let err = run_cse_kernels_into(&one_expr_arena(), &dispatcher, &mut canonical)
            .expect_err("trailing canonical bytes must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn structural_hash_uses_canonical_fnv_mix_helpers() {
        let source = include_str!("cse_via_encoded.rs");
        let release_path = source
            .split("\nmod tests {")
            .next()
            .expect("Fix: optimizer CSE release source must be visible.");
        assert!(
            release_path.contains("fnv1a32_initial_expr")
                && release_path.contains("fnv1a32_mix_word_expr"),
            "Fix: optimizer CSE structural hashing must use canonical primitive FNV helpers."
        );
        assert!(
            !release_path.contains("const FNV_PRIME") && !release_path.contains("const FNV_OFFSET"),
            "Fix: optimizer CSE must not redefine FNV constants."
        );
    }

    /// P0 regression: the canonical dispatch must receive all 7 input buffers
    /// (hash + canonical + table_canonical + 4 arena structural buffers).
    /// Before the fix, only 3 inputs were wired: the structural tuple buffers
    /// were absent, so the hash-only pre-filter was the sole equivalence
    /// predicate and a 32-bit collision would silently merge distinct exprs.
    #[test]
    fn canonical_dispatch_receives_seven_inputs_including_arena_structural_buffers() {
        use std::cell::Cell;
        struct InputCountDispatcher {
            canonical_input_count: Cell<usize>,
            call: Cell<usize>,
        }
        impl OptimizerDispatcher for InputCountDispatcher {
            fn dispatch(
                &self,
                _program: &Program,
                inputs: &[Vec<u8>],
                _grid: Option<[u32; 3]>,
            ) -> Result<Vec<Vec<u8>>, DispatchError> {
                let call = self.call.get();
                self.call.set(call + 1);
                if call == 1 {
                    // Second dispatch = canonical-id program.
                    self.canonical_input_count.set(inputs.len());
                }
                // Return one zero-word output (expr_count = 1).
                Ok(vec![u32_slice_to_le_bytes(&[0])])
            }
        }
        let arena = one_expr_arena();
        let dispatcher = InputCountDispatcher {
            canonical_input_count: Cell::new(0),
            call: Cell::new(0),
        };
        let mut canonical = Vec::new();
        run_cse_kernels_into(&arena, &dispatcher, &mut canonical)
            .expect("Fix: cse kernels dispatch succeeds");
        assert_eq!(
            dispatcher.canonical_input_count.get(),
            7,
            "canonical-id dispatch must receive 7 inputs: hash, canonical (RW), \
             table_canonical (RW dummy), arena_kinds, arena_arg0, arena_arg1, arena_arg2; \
             before the fix only 3 inputs were wired and hash collisions silently merged \
             non-equivalent exprs"
        );
    }

    /// P1 regression: `apply_cse_canonicals` must actually rewrite duplicate
    /// `Let` bindings. A program with two identical `LitU32(42)` bindings and
    /// a canonical map that points the second expr to the first should produce
    /// `let b = Var("a")`, not `let b = LitU32(42)`.
    #[test]
    fn apply_cse_canonicals_rewrites_duplicate_let_to_var() {
        use vyre_foundation::ir::{Expr, Ident, Node, Program};
        // Program:
        //   let a = LitU32(42)   // expr 0 in arena  → canonical[0] = 0 (self)
        //   let b = LitU32(42)   // expr 1 in arena  → canonical[1] = 0 (dup of a)
        let entry = vec![
            Node::let_bind("a", Expr::u32(42)),
            Node::let_bind("b", Expr::u32(42)),
        ];
        let prog = Program::wrapped(Vec::new(), [1, 1, 1], entry);
        let arena = encode_expr_arena(&prog).expect("Fix: simple program encodes");
        // expr 0 = LitU32(42) for 'a', expr 1 = LitU32(42) for 'b'.
        // Canonical: b's expr (id=1) is a dup of a's expr (id=0).
        assert_eq!(arena.expr_count, 2, "expected 2 exprs in arena");
        let canonical = vec![0u32, 0u32]; // canonical[1] = 0
        let rewritten = apply_cse_canonicals(&prog, &arena, &canonical);
        // Expect: let b = Var("a")
        let entry_nodes: Vec<Node> = match rewritten.entry() {
            [Node::Region { body, .. }] => body.as_ref().to_vec(),
            other => other.to_vec(),
        };
        assert_eq!(entry_nodes.len(), 2, "program must still have 2 nodes");
        match &entry_nodes[1] {
            Node::Let { name, value } => {
                assert_eq!(
                    name.as_ref(),
                    "b",
                    "second let must remain named 'b'"
                );
                assert_eq!(
                    value,
                    &Expr::Var(Ident::new(std::sync::Arc::from("a"))),
                    "apply_cse_canonicals must rewrite let b = LitU32(42) to let b = Var(\"a\") \
                     when canonical[1] == 0 and the canonical expr is bound to 'a'; \
                     before the fix the function was a no-op stub that returned the original program"
                );
            }
            other => panic!("expected Node::Let for 'b', got {other:?}"),
        }
    }
}

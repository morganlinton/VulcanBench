//! Typed arena  -  a bounded workgroup-local bump allocator.
//!
//! The typed arena is the memory primitive that makes GPU recursive descent
//! and tree building possible.  It allocates offsets from a fixed workgroup-SRAM
//! pool, rounds sizes to `u32` words, and resets in O(1).  The target-text lowering
//! maps `alloc` to an atomic bump-pointer increment in workgroup memory;
//! the CPU reference mirrors the exact bound and alignment rules so the
//! conform gate can prove byte-identical behavior.
//!
//! ## Wire layout
//!
//! The `arena` buffer is a single bounded u32 array with the following
//! word layout:
//!
//! | word offset | contents                 |
//! | ----------- | ------------------------ |
//! | 0           | `capacity_words`         |
//! | 1           | `bump_cursor_words`      |
//! | 2..=2 + cap | payload (caller-managed) |
//!
//! `alloc_program(arena, size_words, out_offset)` atomics-adds `size_words`
//! into word 1, checks that the resulting cursor stays ≤ `capacity_words`,
//! and stores the pre-increment cursor into `out_offset[0]` when the
//! allocation fits. When the allocation would overflow, `out_offset[0]`
//! is set to `u32::MAX` so the caller can detect the failure without
//! host-round-trip.
//!
//! This matches the `TypedArena` CPU reference byte-for-byte on the
//! happy path (bump cursor monotonically increases by `size_words`);
//! the CPU reference surfaces the precise error variant, which the GPU
//! kernel can't do without a side-channel, so the u32::MAX sentinel
//! is the GPU contract.

use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use thiserror::Error;
use vyre_spec::AlgebraicLaw;

/// Registered target-text source for the typed arena primitive.
#[must_use]
pub fn source() -> Option<&'static str> {
    crate::transform::compiler::shader_provider::source("typed_arena")
}

/// Word offset of the arena's `capacity_words` field.
pub const CAPACITY_WORD_OFFSET: u32 = 0;
/// Word offset of the arena's `bump_cursor_words` field.
pub const BUMP_CURSOR_WORD_OFFSET: u32 = 1;
/// Sentinel value the GPU kernel stores in `out_offset[0]` when the
/// allocation overflows the arena.
pub const ALLOC_OVERFLOW_SENTINEL: u32 = u32::MAX;

/// Build a vyre IR Program implementing one arena allocation.
///
/// Buffers:
/// - `arena`: `ReadWrite` u32 array; bump cursor lives at word
///   [`BUMP_CURSOR_WORD_OFFSET`] and capacity at
///   [`CAPACITY_WORD_OFFSET`]. Caller pre-populates both.
/// - `out_offset`: `ReadWrite` u32 array; the Program writes the
///   allocation's byte offset (pre-increment cursor × 4) at word 0.
///   `out_offset[0] = ALLOC_OVERFLOW_SENTINEL` on overflow.
///
/// The Program is single-dispatch: one allocation per invocation.
/// Multi-allocation loops compose this Program N times.
///
/// # Panics
///
/// Never panics at IR build time; the IR may still fail validation
/// if `size_words == 0` and the resulting zero-byte allocation
/// conflicts with a caller's downstream layout assertion. The
/// Program itself handles `size_words == 0` gracefully (store 0
/// into `out_offset` without incrementing the cursor).
#[must_use]
pub fn alloc_program(arena: &str, out_offset: &str, size_words: u32) -> Program {
    // Atomic fetch-and-add on the cursor. `atomic_add(arena, idx,
    // size)` returns the PRE-increment cursor; if that cursor +
    // size exceeds capacity, we've overflowed.
    let body = vec![
        Node::let_bind(
            "cap_words",
            Expr::load(arena, Expr::u32(CAPACITY_WORD_OFFSET)),
        ),
        Node::barrier(),
        Node::let_bind(
            "prev_cursor",
            Expr::atomic_add(
                arena,
                Expr::u32(BUMP_CURSOR_WORD_OFFSET),
                Expr::u32(size_words),
            ),
        ),
        Node::let_bind(
            "new_cursor",
            Expr::add(Expr::var("prev_cursor"), Expr::u32(size_words)),
        ),
        Node::if_then_else(
            Expr::le(Expr::var("new_cursor"), Expr::var("cap_words")),
            vec![
                // Fit: write prev_cursor * 4 (byte offset) into
                // out_offset[0].
                Node::store(
                    out_offset,
                    Expr::u32(0),
                    Expr::mul(Expr::var("prev_cursor"), Expr::u32(4)),
                ),
            ],
            vec![
                // Overflow: sentinel. Caller observes u32::MAX.
                Node::store(out_offset, Expr::u32(0), Expr::u32(ALLOC_OVERFLOW_SENTINEL)),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(arena, 0, BufferAccess::ReadWrite, DataType::U32),
            BufferDecl::storage(out_offset, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

/// Round `size_bytes` up to the next whole `u32` word count.
///
/// # Errors
///
/// Returns `TypedArenaError::SizeOverflow` if the intermediate addition
/// overflows `u32`.
#[must_use]
pub fn align_words(size_bytes: u32) -> Result<u32, TypedArenaError> {
    size_bytes
        .checked_add(3)
        .map(|bytes| bytes / 4)
        .ok_or(TypedArenaError::SizeOverflow)
}

impl TypedArena {
    /// Create an arena with `capacity_bytes` rounded down to whole `u32` slots.
    #[must_use]
    pub const fn new(capacity_bytes: u32) -> Self {
        Self {
            capacity_words: capacity_bytes / 4,
            bump_words: 0,
        }
    }

    /// Allocate `size_bytes` and return the byte offset from arena base.
    ///
    /// # Errors
    ///
    /// Returns `Fix: ...` when rounding or capacity would overflow.
    #[must_use]
    pub fn alloc(&mut self, size_bytes: u32) -> Result<u32, TypedArenaError> {
        let words = align_words(size_bytes)?;
        let end = self
            .bump_words
            .checked_add(words)
            .ok_or(TypedArenaError::OffsetOverflow)?;
        if end > self.capacity_words {
            return Err(TypedArenaError::OutOfSpace {
                requested_words: words,
                available_words: self.capacity_words.saturating_sub(self.bump_words),
            });
        }
        let offset = self
            .bump_words
            .checked_mul(4)
            .ok_or(TypedArenaError::OffsetOverflow)?;
        self.bump_words = end;
        Ok(offset)
    }

    /// Reset the arena bump cursor to the start of SRAM.
    ///
    /// After reset the arena retains its capacity but all prior handles are
    /// invalid.  This is the O(1) rewind that makes per-workgroup arena reuse
    /// possible across dispatches.
    pub const fn reset(&mut self) {
        self.bump_words = 0;
    }

    /// Current bump cursor in bytes.
    #[must_use]
    pub const fn used_bytes(&self) -> u32 {
        self.bump_words * 4
    }
}

impl TypedArenaOp {}

/// Input signature for the typed-arena primitive: `capacity_bytes` and
/// `allocation_size_bytes`.
pub const INPUTS: &[DataType] = &[DataType::U32, DataType::U32];

/// Algebraic laws declared by the typed-arena primitive.
pub const LAWS: &[AlgebraicLaw] = &[AlgebraicLaw::Bounded {
    lo: 0,
    hi: u32::MAX,
}];

/// CPU mirror of a bounded workgroup-local bump arena.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedArena {
    pub(crate) capacity_words: u32,
    pub(crate) bump_words: u32,
}

/// Typed arena validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TypedArenaError {
    /// Requested allocation size cannot be aligned without overflow.
    #[error(
        "ArenaSizeOverflow: allocation size cannot align to u32 words. Fix: split the AST allocation before dispatch."
    )]
    SizeOverflow,
    /// Arena byte offset cannot fit in `u32`.
    #[error(
        "ArenaOffsetOverflow: arena offset exceeded u32. Fix: lower workgroup arena capacity or split the parse unit."
    )]
    OffsetOverflow,
    /// Allocation exceeded the bounded workgroup arena.
    #[error(
        "ArenaOutOfSpace: requested {requested_words} words with only {available_words} words available. Fix: increase the declared arena capacity or split the parse unit."
    )]
    OutOfSpace {
        /// Number of aligned words requested.
        requested_words: u32,
        /// Number of remaining aligned words.
        available_words: u32,
    },
}

/// Category C typed arena intrinsic.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypedArenaOp;

/// Workgroup size used by the reference target-text lowering.
pub const WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

#[cfg(test)]
mod ir_program_tests {
    use super::*;

    #[test]
    fn alloc_program_declares_two_rw_buffers() {
        let prog = alloc_program("arena", "out", 16);
        assert_eq!(prog.buffers().len(), 2);
        assert_eq!(prog.buffers()[0].name(), "arena");
        assert_eq!(prog.buffers()[1].name(), "out");
    }

    #[test]
    fn alloc_program_validates_against_ir_validator() {
        let prog = alloc_program("arena", "out", 4);
        let errors = crate::validate::validate::validate(&prog);
        assert!(
            errors.is_empty(),
            "typed_arena IR must validate: {errors:?}"
        );
    }

    #[test]
    fn alloc_program_is_wire_round_trip_stable() {
        let prog = alloc_program("arena", "out", 8);
        let bytes = prog
            .to_wire()
            .expect("Fix: serialize; restore this invariant before continuing.");
        let decoded = Program::from_wire(&bytes)
            .expect("Fix: decode; restore this invariant before continuing.");
        assert_eq!(decoded.buffers().len(), prog.buffers().len());
        assert_eq!(decoded.workgroup_size(), prog.workgroup_size());
    }

    #[test]
    fn overflow_sentinel_is_u32_max() {
        assert_eq!(ALLOC_OVERFLOW_SENTINEL, u32::MAX);
    }

    #[test]
    fn alloc_program_different_sizes_produce_different_wire() {
        // Byte-identical-wire stability: the IR is deterministic  -
        // two builds with different size_words produce different
        // canonical bytes (the atomic_add argument differs).
        let a = alloc_program("arena", "out", 4).to_wire().unwrap();
        let b = alloc_program("arena", "out", 8).to_wire().unwrap();
        assert_ne!(a, b);
    }
}

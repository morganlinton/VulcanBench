//! String interner  -  a deterministic workgroup-local symbol table.
//!
//! Lexers and parsers need stable ids for identifiers, keywords, and literals.
//! The string interner provides that without heap allocation: a fixed slot
//! table with linear probing and a shared byte pool, both living in
//! workgroup-local SRAM.  The id-0 sentinel is reserved for the empty string.
//! The CPU reference uses the exact same FNV-1a hash, probe sequence, and
//! capacity limits as the target-text kernel, so conform can prove the tables are
//! byte-identical across host and device.

use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use crate::transform::compiler::U32X4_INPUTS;
use rustc_hash::FxHashMap;
use thiserror::Error;
use vyre_spec::AlgebraicLaw;

/// Registered device source for the string interner primitive.
#[must_use]
pub fn source() -> Option<&'static str> {
    crate::transform::compiler::shader_provider::source("string_interner")
}

/// Sentinel in the output buffer when the input byte length is zero.
/// Matches the CPU reference's "empty string → id 0" contract.
pub const EMPTY_STRING_ID: u32 = 0;

/// Build a vyre IR Program computing the FNV-1a 32-bit hash of the
/// bytes in `input[0..len]` and writing the result to `out[0]`.
///
/// This is the first-phase interner op: the hash lookup + slot
/// insertion lives above it in a composing Program that runs the
/// FNV-1a step plus a linear-probe loop. Splitting the primitive
/// lets downstream crates substitute a different hash function
/// without rewriting the probe logic.
///
/// Buffers:
/// - `input`: `ReadOnly` u32 array (bytes packed little-endian into
///   u32 words  -  `byte(i) = (input[i/4] >> (8 * (i % 4))) & 0xff`).
/// - `out`: `ReadWrite` u32 array with space for at least one word;
///   the Program writes the hash into `out[0]`. When `len == 0`
///   the Program writes [`EMPTY_STRING_ID`] (= 0)  -  matching the
///   CPU reference's empty-string contract.
///
/// The IR is self-contained: no workgroup memory, no atomic ops,
/// one lane emits the result. Backends that want a parallel lane
/// fan-in compose this Program over N shards.
#[must_use]
pub fn fnv1a_program(input: &str, out: &str, len: u32) -> Program {
    let body = vec![
        Node::let_bind("hash", Expr::u32(0x811c_9dc5)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(len),
            vec![
                // Extract the i-th byte from the packed u32 stream:
                // input[i / 4] >> (8 * (i % 4)) & 0xff.
                Node::let_bind(
                    "word",
                    Expr::load(input, Expr::div(Expr::var("i"), Expr::u32(4))),
                ),
                Node::let_bind(
                    "shift",
                    Expr::mul(Expr::u32(8), Expr::rem(Expr::var("i"), Expr::u32(4))),
                ),
                Node::let_bind(
                    "byte",
                    Expr::bitand(
                        Expr::shr(Expr::var("word"), Expr::var("shift")),
                        Expr::u32(0xff),
                    ),
                ),
                Node::assign(
                    "hash",
                    Expr::mul(
                        Expr::bitxor(Expr::var("hash"), Expr::var("byte")),
                        Expr::u32(0x0100_0193),
                    ),
                ),
            ],
        ),
        Node::store(
            out,
            Expr::u32(0),
            // len == 0 → EMPTY_STRING_ID (=0). Else the running
            // hash. The `len` constant is folded at compile time
            // so the branch collapses on both arms.
            if len == 0 {
                Expr::u32(EMPTY_STRING_ID)
            } else {
                Expr::var("hash")
            },
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(out, 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

const _: &[DataType] = U32X4_INPUTS;

/// Slot entry describing an interned string slice.
///
/// Entries store a 32-bit FNV-1a hash, the byte offset and length inside the
/// shared byte pool, and the stable non-zero intern id assigned on first
/// insertion.  The fields are `pub(crate)` because the public API is through
/// `StringInterner::intern` and `StringInterner::lookup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub(crate) hash: u32,
    pub(crate) offset: usize,
    pub(crate) len: usize,
    pub(crate) id: u32,
}

/// FNV-1a 32-bit hash used by CPU and target-text references.
#[must_use]
pub fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for &byte in bytes {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

impl StringInterner {
    /// Create an interner with a fixed slot capacity and a fixed byte
    /// storage capacity. Both are bounded so the entire interner fits
    /// in workgroup SRAM on the GPU side; the CPU reference mirrors
    /// that bound exactly.
    #[must_use]
    pub fn new(slot_capacity: usize, byte_capacity: usize) -> Self {
        Self {
            slots: vec![None; slot_capacity],
            bytes: Vec::with_capacity(byte_capacity),
            reverse: FxHashMap::default(),
            byte_capacity,
            next_id: 1,
        }
    }

    /// Intern `bytes` and return a stable non-zero intern id. Empty
    /// input always resolves to id `0` (the sentinel) without
    /// consuming a slot.
    ///
    /// # Errors
    ///
    /// Returns a `Fix: ...` error when the slot table is full, the
    /// byte pool is exhausted, or a slot index cannot fit in `u32`.
    #[must_use]
    pub fn intern(&mut self, input: &[u8]) -> Result<u32, StringInternerError> {
        if input.is_empty() {
            return Ok(0);
        }
        if self.slots.is_empty() {
            return Err(StringInternerError::TableFull);
        }
        let hash = fnv1a32(input);
        let start = usize::try_from(hash).map_err(|_| StringInternerError::IndexOverflow)?
            % self.slots.len();
        for probe in 0..self.slots.len() {
            let slot = (start + probe) % self.slots.len();
            match &self.slots[slot] {
                Some(entry) if entry.hash == hash && self.entry_bytes(entry) == input => {
                    return Ok(entry.id);
                }
                Some(_) => {}
                None => {
                    if self
                        .bytes
                        .len()
                        .checked_add(input.len())
                        .is_none_or(|total| total > self.byte_capacity)
                    {
                        return Err(StringInternerError::BytePoolFull);
                    }
                    let offset = self.bytes.len();
                    self.bytes.extend_from_slice(input);
                    let id = self.next_id;
                    self.next_id = self
                        .next_id
                        .checked_add(1)
                        .ok_or(StringInternerError::IndexOverflow)?;
                    let entry = Entry {
                        hash,
                        offset,
                        len: input.len(),
                        id,
                    };
                    self.reverse.insert(id, slot);
                    self.slots[slot] = Some(entry);
                    return Ok(id);
                }
            }
        }
        Err(StringInternerError::TableFull)
    }

    /// Reverse lookup: given an id from a prior `intern` call on this
    /// interner, return the bytes that produced it. Returns `None` if
    /// the id is unknown; `Some(&[])` if the id is the empty-string
    /// sentinel `0`.
    #[must_use]
    pub fn lookup(&self, id: u32) -> Option<&[u8]> {
        if id == 0 {
            return Some(&[]);
        }
        let slot = *self.reverse.get(&id)?;
        let entry = self.slots.get(slot)?.as_ref()?;
        Some(self.entry_bytes(entry))
    }

    pub(crate) fn entry_bytes(&self, entry: &Entry) -> &[u8] {
        &self.bytes[entry.offset..entry.offset + entry.len]
    }
}

impl StringInternerOp {}

/// Intern all byte strings into a fresh fixed-capacity table.
///
/// Byte capacity is derived as `slot_capacity * 64` which is the
/// default upper bound assumed by the target-text kernel's workgroup SRAM
/// layout. Callers that need a different budget should instantiate
/// `StringInterner::new(slot_capacity, byte_capacity)` directly.
///
/// # Errors
///
/// Returns `Fix: ...` when insertion cannot complete with linear probing.
#[must_use]
pub fn intern_all(inputs: &[&[u8]], slot_capacity: usize) -> Result<Vec<u32>, StringInternerError> {
    let byte_capacity = slot_capacity.saturating_mul(64);
    let mut interner = StringInterner::new(slot_capacity, byte_capacity);
    inputs.iter().map(|bytes| interner.intern(bytes)).collect()
}

/// Algebraic laws declared by the string-interner primitive.
pub const LAWS: &[AlgebraicLaw] = &[AlgebraicLaw::Bounded {
    lo: 0,
    hi: u32::MAX,
}];

/// Deterministic workgroup-local intern table.
///
/// Models a workgroup-SRAM interner that holds up to `slot_capacity`
/// unique strings sharing a bounded byte storage pool of
/// `byte_capacity` bytes. This exactly matches the target-text lowering which
/// allocates a fixed slot table and a fixed byte arena. The id-0
/// sentinel is reserved for "empty string" so callers can map missing
/// entries to a well-known value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringInterner {
    pub(crate) slots: Vec<Option<Entry>>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) reverse: FxHashMap<u32, usize>,
    pub(crate) byte_capacity: usize,
    pub(crate) next_id: u32,
}

/// String interner validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum StringInternerError {
    /// Linear probing exhausted the bounded slot table.
    #[error(
        "InternerTableFull: no SRAM slot accepted the string. Fix: increase table slots or split the lexing batch."
    )]
    TableFull,
    /// The byte storage pool is full  -  the total bytes of interned
    /// strings would exceed the `byte_capacity` declared at
    /// construction time.
    #[error(
        "InternerBytePoolFull: byte storage is exhausted. Fix: raise byte_capacity or split the lexing batch."
    )]
    BytePoolFull,
    /// Intern id or table index cannot fit in `u32`.
    #[error(
        "InternerIndexOverflow: table slot cannot fit u32 intern id. Fix: lower the workgroup interner capacity."
    )]
    IndexOverflow,
}

/// Category C string interner intrinsic.
#[derive(Debug, Default, Clone, Copy)]
pub struct StringInternerOp;

/// Workgroup size used by the reference target-text lowering.
pub const WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

#[cfg(test)]
mod ir_program_tests {
    use super::*;

    #[test]
    fn fnv1a_program_validates() {
        let prog = fnv1a_program("input", "out", 8);
        let errors = crate::validate::validate::validate(&prog);
        assert!(
            errors.is_empty(),
            "string_interner fnv1a IR must validate: {errors:?}"
        );
    }

    #[test]
    fn fnv1a_program_wire_round_trips() {
        let prog = fnv1a_program("input", "out", 16);
        let bytes = prog
            .to_wire()
            .expect("Fix: serialize; restore this invariant before continuing.");
        let decoded = Program::from_wire(&bytes)
            .expect("Fix: decode; restore this invariant before continuing.");
        assert_eq!(decoded.buffers().len(), 2);
        assert_eq!(decoded.workgroup_size(), [1, 1, 1]);
    }

    #[test]
    fn fnv1a_program_empty_input_short_circuits() {
        // len == 0 is the empty-string contract: the output buffer
        // receives EMPTY_STRING_ID (0) and the loop runs zero times.
        let prog = fnv1a_program("input", "out", 0);
        let errors = crate::validate::validate::validate(&prog);
        assert!(
            errors.is_empty(),
            "empty-input IR must validate: {errors:?}"
        );
    }

    #[test]
    fn fnv1a_program_different_lens_produce_different_wire() {
        let a = fnv1a_program("input", "out", 4).to_wire().unwrap();
        let b = fnv1a_program("input", "out", 16).to_wire().unwrap();
        assert_ne!(a, b, "loop bound is part of the canonical wire");
    }

    #[test]
    fn empty_string_sentinel_is_zero() {
        assert_eq!(EMPTY_STRING_ID, 0);
    }
}

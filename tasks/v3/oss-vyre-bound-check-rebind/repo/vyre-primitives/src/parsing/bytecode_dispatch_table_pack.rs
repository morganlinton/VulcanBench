//! `bytecode_dispatch_table_pack`  -  pack an opcode-handler dispatch table
//! into a constant-buffer for fast GPU-side bytecode interpretation.
//!
//! Op id: `vyre-primitives::parsing::bytecode_dispatch_table_pack`. Soundness:
//! `Exact` over the opcode → handler-offset mapping. The canonical
//! bytecode-on-GPU interpreter loop reads `dispatch_table[opcode]` to find
//! which handler program to invoke, then executes it. Centralising the table
//! layout and validation here lets every interpreter dialect (Lua-shape,
//! JVM-shape, WASM-shape) share one well-typed packing format.
//!
//! ## Why it matters
//!
//! Bytecode interpreter loops on GPU lose on naive implementations because
//! the dispatch-table fetch + indirect-branch pattern is the canonical
//! "GPU loses to CPU" workload (CPU has branch predictor + huge L1; GPU
//! has neither). The fix: pack the dispatch table into a constant-buffer
//! that resides in shared memory + use uniform-control-flow patterns where
//! every thread executes the same handler in the same warp (warp-specialized
//! interpretation).
//!
//! This module ships the *packing* part. Interpreter loops read the packed
//! table through this stable wire layout.
//!
//! ## Wire format
//!
//! Each table entry is one u32 packed as:
//!
//! ```text
//!   bits 0..23   -  handler_offset (max 2^24 = 16M handlers  -  plenty)
//!   bits 24..27  -  handler_arity  (number of operand bytes, 0..15)
//!   bits 28..31  -  flags          (bit 28 = side_effecting, bit 29 = control_flow)
//! ```
//!
//! The packed format means dispatch is one u32 load + one mask-and-shift
//! per opcode. No pointer chasing.

/// One opcode → handler entry, the host-side representation that
/// `pack_dispatch_table` turns into a packed u32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcodeHandlerEntry {
    /// Offset of the handler routine within the program-pool buffer.
    /// Capped at 2^24 - 1 = 16777215.
    pub handler_offset: u32,
    /// Operand-byte count this handler reads after the opcode byte.
    /// Capped at 15.
    pub handler_arity: u8,
    /// True if the handler has observable side effects (writes a buffer,
    /// triggers a sync, etc). Codegen uses this to refuse fusion across.
    pub side_effecting: bool,
    /// True if the handler can change control flow (branch / call / return).
    /// Interpreter optimizer uses this to disable speculation across.
    pub control_flow: bool,
}

/// Pack errors. Returned when the host-side entry can't be encoded into the
/// 1-u32 wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PackError {
    /// Handler offset exceeded the 24-bit field budget.
    OffsetTooLarge {
        /// The opcode index whose entry overflowed.
        opcode: usize,
        /// The offset that exceeded `1 << 24`.
        offset: u32,
    },
    /// Handler arity exceeded the 4-bit field budget.
    ArityTooLarge {
        /// The opcode index whose entry overflowed.
        opcode: usize,
        /// The arity that exceeded `15`.
        arity: u8,
    },
    /// Caller-owned output could not reserve enough entries.
    Allocation {
        /// Requested packed entries.
        requested: usize,
        /// Allocator detail.
        source: String,
    },
}

/// Number of packed u32 words required for `entries`.
#[must_use]
#[inline]
pub const fn packed_dispatch_table_len(entries_len: usize) -> usize {
    entries_len
}

/// Pack a dispatch table of `OpcodeHandlerEntry` into one u32 per entry,
/// suitable for upload as a constant buffer. The output index matches the
/// input index; `output[opcode_byte]` is the packed entry.
///
/// # Errors
///
/// Returns the first encoding overflow encountered. Caller fixes by
/// reducing the handler-offset (split into chunks) or refusing to register
/// an arity > 15 handler.
pub fn pack_dispatch_table(entries: &[OpcodeHandlerEntry]) -> Result<Vec<u32>, PackError> {
    let mut out = Vec::new();
    pack_dispatch_table_into(entries, &mut out)?;
    Ok(out)
}

/// Pack a dispatch table into caller-owned storage.
///
/// This is the hot-path API for repeated interpreter construction: the
/// caller owns `out`, and this function clears then reuses its capacity.
///
/// # Errors
///
/// Returns the first encoding overflow or output allocation failure
/// encountered. On error, `out` is left unchanged.
pub fn pack_dispatch_table_into(
    entries: &[OpcodeHandlerEntry],
    out: &mut Vec<u32>,
) -> Result<(), PackError> {
    for (idx, entry) in entries.iter().enumerate() {
        if entry.handler_offset >= (1u32 << 24) {
            return Err(PackError::OffsetTooLarge {
                opcode: idx,
                offset: entry.handler_offset,
            });
        }
        if entry.handler_arity > 15 {
            return Err(PackError::ArityTooLarge {
                opcode: idx,
                arity: entry.handler_arity,
            });
        }
    }
    let len = packed_dispatch_table_len(entries.len());
    if len > out.capacity() {
        out.try_reserve_exact(len - out.capacity())
            .map_err(|source| PackError::Allocation {
                requested: len,
                source: source.to_string(),
            })?;
    }

    out.clear();
    out.extend(entries.iter().map(|entry| {
        let mut packed: u32 = entry.handler_offset & 0x00FF_FFFF;
        packed |= (u32::from(entry.handler_arity) & 0xF) << 24;
        if entry.side_effecting {
            packed |= 1 << 28;
        }
        if entry.control_flow {
            packed |= 1 << 29;
        }
        packed
    }));
    Ok(())
}

/// Pack one dispatch-table entry without validation.
#[must_use]
pub fn pack_entry(entry: OpcodeHandlerEntry) -> u32 {
    let mut packed = entry.handler_offset & 0x00FF_FFFF;
    packed |= (u32::from(entry.handler_arity) & 0xF) << 24;
    if entry.side_effecting {
        packed |= 1 << 28;
    }
    if entry.control_flow {
        packed |= 1 << 29;
    }
    packed
}

/// Unpack one u32 entry back into the host-side representation. Used by
/// interpreter Programs that read `dispatch_table[opcode]` and need to
/// know which handler to invoke + how many operand bytes to consume.
#[must_use]
pub fn unpack_entry(packed: u32) -> OpcodeHandlerEntry {
    OpcodeHandlerEntry {
        handler_offset: packed & 0x00FF_FFFF,
        handler_arity: ((packed >> 24) & 0xF) as u8,
        side_effecting: (packed >> 28) & 0x1 == 1,
        control_flow: (packed >> 29) & 0x1 == 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_entry_fields() {
        let entry = OpcodeHandlerEntry {
            handler_offset: 0x123456,
            handler_arity: 7,
            side_effecting: true,
            control_flow: false,
        };
        let packed = pack_dispatch_table(&[entry]).expect("Fix: pack must succeed");
        assert_eq!(packed.len(), 1);
        let recovered = unpack_entry(packed[0]);
        assert_eq!(recovered, entry, "round-trip must preserve every field");
        assert_eq!(unpack_entry(pack_entry(entry)), entry);
    }

    #[test]
    fn pack_into_reuses_output_and_is_transactional_on_invalid_entry() {
        let entries = [
            OpcodeHandlerEntry {
                handler_offset: 1,
                handler_arity: 2,
                side_effecting: false,
                control_flow: false,
            },
            OpcodeHandlerEntry {
                handler_offset: 3,
                handler_arity: 4,
                side_effecting: true,
                control_flow: true,
            },
        ];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[u32::MAX; 8]);
        let ptr = out.as_ptr();

        pack_dispatch_table_into(&entries, &mut out).unwrap();

        assert_eq!(
            out,
            entries.iter().copied().map(pack_entry).collect::<Vec<_>>()
        );
        assert_eq!(out.as_ptr(), ptr);
        let before = out.clone();
        let bad = [OpcodeHandlerEntry {
            handler_offset: 1 << 24,
            handler_arity: 0,
            side_effecting: false,
            control_flow: false,
        }];
        assert!(matches!(
            pack_dispatch_table_into(&bad, &mut out),
            Err(PackError::OffsetTooLarge { .. })
        ));
        assert_eq!(out, before);
    }

    #[test]
    fn round_trip_handles_all_flag_combinations() {
        for side_effecting in [false, true] {
            for control_flow in [false, true] {
                let entry = OpcodeHandlerEntry {
                    handler_offset: 42,
                    handler_arity: 3,
                    side_effecting,
                    control_flow,
                };
                let packed = pack_dispatch_table(&[entry]).unwrap();
                assert_eq!(unpack_entry(packed[0]), entry);
            }
        }
    }

    #[test]
    fn pack_rejects_offset_at_field_boundary() {
        let entry = OpcodeHandlerEntry {
            handler_offset: 1u32 << 24, // exactly the limit  -  must reject
            handler_arity: 0,
            side_effecting: false,
            control_flow: false,
        };
        match pack_dispatch_table(&[entry]) {
            Err(PackError::OffsetTooLarge { opcode: 0, offset }) => {
                assert_eq!(offset, 1u32 << 24);
            }
            other => panic!("expected OffsetTooLarge at the 24-bit boundary; got {other:?}"),
        }
    }

    #[test]
    fn pack_rejects_arity_at_field_boundary() {
        let entry = OpcodeHandlerEntry {
            handler_offset: 0,
            handler_arity: 16, // 4-bit field max is 15
            side_effecting: false,
            control_flow: false,
        };
        match pack_dispatch_table(&[entry]) {
            Err(PackError::ArityTooLarge { opcode: 0, arity }) => {
                assert_eq!(arity, 16);
            }
            other => panic!("expected ArityTooLarge at the 4-bit boundary; got {other:?}"),
        }
    }

    #[test]
    fn pack_preserves_per_entry_index_in_error() {
        // Entry 7 is the bad one; error must report opcode = 7.
        let mut entries = vec![
            OpcodeHandlerEntry {
                handler_offset: 0,
                handler_arity: 0,
                side_effecting: false,
                control_flow: false,
            };
            10
        ];
        entries[7].handler_offset = 1u32 << 25; // overflow
        match pack_dispatch_table(&entries) {
            Err(PackError::OffsetTooLarge { opcode: 7, .. }) => {}
            other => panic!("expected error at opcode 7; got {other:?}"),
        }
    }

    #[test]
    fn pack_empty_table_returns_empty_vec() {
        let packed = pack_dispatch_table(&[]).expect("Fix: empty pack must succeed");
        assert!(packed.is_empty());
    }

    #[test]
    fn pack_into_reuses_existing_capacity() {
        let entries = [
            OpcodeHandlerEntry {
                handler_offset: 8,
                handler_arity: 2,
                side_effecting: false,
                control_flow: true,
            },
            OpcodeHandlerEntry {
                handler_offset: 16,
                handler_arity: 3,
                side_effecting: true,
                control_flow: false,
            },
        ];
        let mut out = Vec::with_capacity(64);
        let before = out.capacity();
        pack_dispatch_table_into(&entries, &mut out).expect("Fix: pack_into must succeed");
        assert_eq!(out.len(), entries.len());
        assert_eq!(
            out.capacity(),
            before,
            "pack_into must reuse caller-owned capacity"
        );
        assert_eq!(unpack_entry(out[0]), entries[0]);
        assert_eq!(unpack_entry(out[1]), entries[1]);
    }

    #[test]
    fn required_len_matches_entry_count() {
        assert_eq!(packed_dispatch_table_len(0), 0);
        assert_eq!(packed_dispatch_table_len(256), 256);
    }

    #[test]
    fn pack_full_256_opcode_table_succeeds() {
        // Realistic interpreter: 256 opcodes, each with a handler.
        let entries: Vec<_> = (0..256u32)
            .map(|opcode| OpcodeHandlerEntry {
                handler_offset: opcode * 16,
                handler_arity: (opcode % 4) as u8,
                side_effecting: opcode % 2 == 0,
                control_flow: opcode % 8 == 0,
            })
            .collect();
        let packed = pack_dispatch_table(&entries).expect("Fix: full 256-table must pack");
        assert_eq!(packed.len(), 256);
        // Spot-check a few entries.
        assert_eq!(unpack_entry(packed[0]), entries[0]);
        assert_eq!(unpack_entry(packed[127]), entries[127]);
        assert_eq!(unpack_entry(packed[255]), entries[255]);
    }

    #[test]
    fn handler_arity_zero_packs_cleanly() {
        let entry = OpcodeHandlerEntry {
            handler_offset: 100,
            handler_arity: 0,
            side_effecting: false,
            control_flow: false,
        };
        let packed = pack_dispatch_table(&[entry]).unwrap();
        assert_eq!(packed[0], 100, "arity=0 + flags=0 packs as just the offset");
    }
}

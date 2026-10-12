/// Do nothing. Useful for heartbeat probes.
pub const NOP: u32 = 0;
/// `control[args[1]] = args[0]`.
pub const STORE_U32: u32 = 1;
/// `atomic_add(control[args[1]], args[0])`.
pub const ATOMIC_ADD: u32 = 2;
/// Read `control[args[0]]` into `control[OBSERVABLE_BASE + args[1]]`.
pub const LOAD_U32: u32 = 3;
/// `CAS(control[args[0]], expected=args[1], desired=args[2])`.
pub const COMPARE_SWAP: u32 = 4;
/// Bulk GPU-to-GPU copy within the control buffer.
pub const MEMCPY: u32 = 5;
/// DFA single-step: `next_state = dfa_table[args[0] * 256 + args[1]]`.
pub const DFA_STEP: u32 = 6;
/// Batch fence signaling that the published batch is complete.
pub const BATCH_FENCE: u32 = 7;
/// GPU-initiated load miss: the megakernel writes a DMA request to the IO
/// queue and polls for completion. The argument is the consumer's
/// resource identifier (32-bit, opaque to vyre). vyre is a generic GPU
/// substrate; it does not know what "the resource" is  -  that is the
/// consumer's domain. See the boundary rule in AGENTS.md.
pub const LOAD_MISS: u32 = 0x0000_FFFD;
/// Deprecated alias retained for source-level compatibility. Will be
/// removed once all in-tree consumers have migrated; new code must use
/// [`LOAD_MISS`].
#[deprecated(
    since = "0.5.0",
    note = "vyre is a generic GPU substrate  -  use `LOAD_MISS`. The wire \
            format is unchanged; only the symbolic name moves."
)]
pub const EXPERT_LOAD_MISS: u32 = LOAD_MISS;
/// Packed slot: one outer ring slot carries several inner ops.
pub const PACKED_SLOT: u32 = 0x8000_0001;
/// Write one PRINTF event to the debug log.
pub const PRINTF: u32 = 0x0000_FFFE;
/// Set `control[SHUTDOWN] = 1`.
pub const SHUTDOWN: u32 = u32::MAX;
/// High bit is reserved for system opcodes.
pub const SYSTEM_MASK: u32 = 0x8000_0000;
/// Lower bound for the high reserved range.
pub const RESERVED_MAX_RANGE_MIN: u32 = 0x0000_FFF0;

/// Return true if the opcode is reserved by the megakernel.
#[must_use]
pub const fn is_system(op: u32) -> bool {
    (op & SYSTEM_MASK) != 0
        || (op >= RESERVED_MAX_RANGE_MIN && op <= 0x0000_FFFF)
        || op <= BATCH_FENCE
}

/// Return true if the opcode is one of the frozen built-in opcodes.
#[must_use]
pub const fn is_builtin(op: u32) -> bool {
    op <= BATCH_FENCE || op == PACKED_SLOT || op == PRINTF || op == SHUTDOWN
}

/// Validate a user-defined opcode.
///
/// # Errors
///
/// Returns a static string when `op` overlaps a reserved system range.
pub const fn validate_user_opcode(op: u32) -> Result<(), &'static str> {
    if is_system(op) {
        Err("User opcode overlaps with reserved system range or uses the high bit.")
    } else {
        Ok(())
    }
}

/// Validate an opcode that is about to be written to the ring.
pub const fn validate_publish_opcode(op: u32) -> Result<(), &'static str> {
    if is_builtin(op) {
        Ok(())
    } else {
        validate_user_opcode(op)
    }
}

const _: () = {
    let opcodes = [
        NOP,
        STORE_U32,
        ATOMIC_ADD,
        LOAD_U32,
        COMPARE_SWAP,
        MEMCPY,
        DFA_STEP,
        BATCH_FENCE,
        LOAD_MISS,
        PACKED_SLOT,
        PRINTF,
        SHUTDOWN,
    ];
    let mut i = 0;
    while i < opcodes.len() {
        let mut j = i + 1;
        while j < opcodes.len() {
            assert!(opcodes[i] != opcodes[j], "Duplicate opcode");
            j += 1;
        }
        assert!(is_system(opcodes[i]), "Opcode is not system");
        i += 1;
    }
};

use crate::compiler::atomic_collect::atomic_collect_u32;
use vyre::ir::{Expr, Program};

const OP_ID: &str = "vyre-libs::parsing::opt_stack_layout_generation";

/// GPU SIMT Stack Layout Generator (Prologue/Epilogue Spiller)
///
/// When variables cannot fit into the physical 16 registers of x86_64, they "spill".
/// This module generates precise Main Memory stack offsets relative to `%rbp` (base pointer)
/// and emits `push`/`pop` sequences using SIMT inclusive scan prefix offsets.
#[must_use]
pub fn opt_stack_layout_generation(
    physical_registers: &str,
    out_spill_offsets: &str,
    num_regs: Expr,
) -> Program {
    atomic_collect_u32(
        OP_ID,
        physical_registers,
        out_spill_offsets,
        "tmp_stack_frame_size",
        num_regs,
        8,
        None,
        |reg_bound, _t| Expr::ge(reg_bound, Expr::u32(16)),
        |t, _stack_offset| t,
        |_t, stack_offset| stack_offset,
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || opt_stack_layout_generation("regs", "spills", Expr::u32(4)),
        // 4 virtual registers: two sit in the 0..=15 physical window,
        // two spill (reg_bound = 20, 30). Each spill claims an 8-byte
        // stack slot via atomic_add on a workgroup-scoped counter.
        // The reference interpreter runs one lane at a time in
        // monotonic invocation order, so spill_offsets[1] = 0 and
        // spill_offsets[3] = 8.
        test_inputs: Some(|| {
            let regs: [u32; 4] = [3, 20, 7, 30];
            let bytes = vyre_primitives::wire::pack_u32_slice(&regs);
            // regs (ReadOnly), out_spill_offsets (ReadWrite),
            // tmp_stack_frame_size (ReadWrite, 1 slot).
            vec![vec![bytes, vec![0u8; 4 * 4], vec![0u8; 4]]]
        }),
        expected_output: Some(|| {
            let spills: [u32; 4] = [0, 0, 0, 8];
            let frame_size: [u32; 1] = [16];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&spills), to_bytes(&frame_size)]]
        }),
        category: Some("compiler"),
    }
}

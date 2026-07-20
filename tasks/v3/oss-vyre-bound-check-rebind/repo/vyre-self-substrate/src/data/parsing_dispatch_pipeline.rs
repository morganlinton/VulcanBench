//! Self-substrate parsing dispatch data paths.
//!
//! Vyre's own resident optimizer and megakernel interpreter use the same
//! packed-AST and bytecode-dispatch table primitives that user dialects use.
//! This module keeps that recursion load-bearing: table packing, table decode,
//! and AST constant-fold wave construction all route through
//! `vyre-primitives::parsing` rather than duplicating layout rules here.

use vyre_foundation::ir::{Expr, Node};
use vyre_primitives::parsing::{
    ast_cse_constant_fold::{ast_cse_constant_fold, OP_ID as AST_CSE_CONSTANT_FOLD_PRIMITIVE_ID},
    bytecode_dispatch_table_pack::{
        pack_dispatch_table_into, packed_dispatch_table_len, unpack_entry, OpcodeHandlerEntry,
        PackError,
    },
};

/// Stable primitive id for the bytecode dispatch-table packing contract.
pub const BYTECODE_DISPATCH_TABLE_PACK_PRIMITIVE_ID: &str =
    "vyre-primitives::parsing::bytecode_dispatch_table_pack";

/// Primitive ids consumed by the self-hosted parsing dispatch pipeline.
pub const PARSING_DISPATCH_PIPELINE_PRIMITIVES: [&str; 2] = [
    AST_CSE_CONSTANT_FOLD_PRIMITIVE_ID,
    BYTECODE_DISPATCH_TABLE_PACK_PRIMITIVE_ID,
];

/// Emit the packed-AST constant-folding wave used by self-hosted parser passes.
///
/// The returned nodes are the primitive's Region body, not a proof artifact:
/// callers splice them into larger resident AST optimizer programs so the
/// substrate's parser optimizer and user-dialect parser optimizer share one
/// layout and mutation contract.
#[must_use]
pub fn emit_self_hosted_ast_constant_fold_wave(
    ast_opcodes: &str,
    ast_lefts: &str,
    ast_rights: &str,
    ast_vals: &str,
    out_modified_flag: &str,
    t: Expr,
) -> Vec<Node> {
    ast_cse_constant_fold(
        ast_opcodes,
        ast_lefts,
        ast_rights,
        ast_vals,
        out_modified_flag,
        t,
    )
}

/// Return the exact packed-word count needed for a bytecode handler table.
#[must_use]
pub const fn self_hosted_dispatch_table_words(entries_len: usize) -> usize {
    packed_dispatch_table_len(entries_len)
}

/// Pack a self-hosted interpreter dispatch table into caller-owned storage.
///
/// This is the hot-path API for resident interpreter construction: the caller
/// keeps allocation ownership, while the primitive owns the byte layout and
/// validation rules.
///
/// # Errors
///
/// Returns [`PackError`] when an entry cannot be represented in the one-word
/// dispatch-table ABI.
pub fn pack_self_hosted_bytecode_dispatch_table(
    entries: &[OpcodeHandlerEntry],
    out: &mut Vec<u32>,
) -> Result<(), PackError> {
    pack_dispatch_table_into(entries, out)
}

/// Decode one self-hosted interpreter dispatch-table entry.
#[must_use]
pub fn decode_self_hosted_dispatch_entry(packed: u32) -> OpcodeHandlerEntry {
    unpack_entry(packed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_table_round_trip_uses_primitive_layout() {
        let entry = OpcodeHandlerEntry {
            handler_offset: 0x00ab_cdef,
            handler_arity: 9,
            side_effecting: true,
            control_flow: true,
        };
        let mut packed = Vec::new();

        pack_self_hosted_bytecode_dispatch_table(&[entry], &mut packed)
            .expect("Fix: valid self-hosted dispatch entry must pack");

        assert_eq!(self_hosted_dispatch_table_words(1), 1);
        assert_eq!(packed.len(), 1);
        assert_eq!(decode_self_hosted_dispatch_entry(packed[0]), entry);
    }

    #[test]
    fn ast_constant_fold_wave_contains_mutating_nodes() {
        let nodes = emit_self_hosted_ast_constant_fold_wave(
            "ast_opcodes",
            "ast_lefts",
            "ast_rights",
            "ast_vals",
            "modified",
            Expr::var("t"),
        );

        assert_ne!(
            nodes.len(),
            0,
            "constant-fold wave must emit executable IR nodes"
        );
    }
}

//! Frozen predicate primitives  -  the compact engine primitives used by
//! source-query dialect standard libraries. Each is a thin wrapper that emits a vyre
//! Program composing [`crate::graph`] + [`crate::bitset`] +
//! [`crate::label`] primitives with a specific edge-kind mask, tag
//! mask, or node-kind constant.
//!
//! The ten primitives:
//! - `call_to`  -  edge kind `CallArg` from frontier to callee.
//! - `return_value_of`  -  edge kind `Return` from call to binding.
//! - `arg_of`  -  edge kind `CallArg` reverse (arg → call).
//! - `size_argument_of`  -  arg_of restricted to integer literal args.
//! - `edge`  -  raw edge matcher (forward, any mask).
//! - `in_function`  -  node_tags ∩ `TAG_FAMILY_FUNCTION`.
//! - `in_file`  -  node_tags ∩ `TAG_FAMILY_FILE`.
//! - `in_package`  -  node_tags ∩ `TAG_FAMILY_PACKAGE`.
//! - `literal_of`  -  `nodes[v] == NODE_KIND_LITERAL` AND value matches.
//! - `node_kind`  -  `nodes[v] == kind`.

/// Canonical edge-kind bitmasks matching the shared source-query
/// `ProgramGraph::EdgeKind`. One bit per kind; multiple bits can
/// coexist in the same `edge_kind_mask[e]` word.
pub mod edge_kind {
    /// Dataflow assignment edge.
    pub const ASSIGNMENT: u32 = 1 << 0;
    /// Function-call argument edge.
    pub const CALL_ARG: u32 = 1 << 1;
    /// Function return-value edge.
    pub const RETURN: u32 = 1 << 2;
    /// SSA Phi edge.
    pub const PHI: u32 = 1 << 3;
    /// Dominance edge.
    pub const DOMINANCE: u32 = 1 << 4;
    /// Alias edge.
    pub const ALIAS: u32 = 1 << 5;
    /// Memory store edge.
    pub const MEM_STORE: u32 = 1 << 6;
    /// Memory load edge.
    pub const MEM_LOAD: u32 = 1 << 7;
    /// Mutable reference edge.
    pub const MUT_REF: u32 = 1 << 8;
    /// Control-flow edge.
    pub const CONTROL: u32 = 1 << 9;

    // Slot-accessor edges  -  bits 10..14  -  emitted by source frontends'
    // walker on AST nodes whose semantic operands need direct access
    // by name (`base_of`, `index_of`, `upper_bound_of`,
    // `induction_variable_of`, `format_string_argument_of`). The
    // edge points FROM the parent AST node TO the operand SSA value
    // so a backward CSR traversal masked on the kind bit picks up
    // exactly the operand for any node in the input frontier.

    /// Edge from `arr[idx]` → `idx` operand. Emitted on
    /// subscript_expression / array_access / array_subscript nodes.
    pub const INDEX: u32 = 1 << 10;
    /// Edge from `arr[idx]` → `arr` operand.
    pub const BASE: u32 = 1 << 11;
    /// Edge from a for/while loop → its induction-variable
    /// declaration (the `i` in `for (int i = 0; ...; ...)`).
    pub const INDUCTION_VARIABLE: u32 = 1 << 12;
    /// Edge from a for/while/do-while loop → its upper-bound
    /// expression (the right-hand side of the loop test).
    pub const UPPER_BOUND: u32 = 1 << 13;
    /// Edge from a printf-family call → its format-string argument.
    /// The walker consults `printf_family.toml`'s [c.format_slot]
    /// table to determine which argument slot carries the format
    /// string (slot 0 for printf, 1 for fprintf/sprintf/snprintf,
    /// 2 for swprintf, etc.).
    pub const FORMAT_STRING_ARG: u32 = 1 << 14;

    // Per-slot CALL_ARG subkinds  -  bits 16..23. Pre-fix `arg_of(call,
    // N)` returned ALL CALL_ARG predecessors regardless of N because
    // the underlying csr_backward_traverse only filtered by the
    // generic CALL_ARG bit. With these subkind bits the walker emits
    // BOTH the generic CALL_ARG bit AND the per-slot bit on each
    // call-arg edge, and `arg_of(call, N)` masks on
    // `CALL_ARG_SLOT_BASE << N`. 8 slots cover every realistic
    // launch-shape arity (every shape uses index ≤ 2). A 9th slot
    // demand requires widening edge_kind_mask to u64  -  a substrate
    // change tracked in the open backlog.
    /// First per-slot call-argument bit. Slot `N` uses
    /// `CALL_ARG_SLOT_BASE << N` while the generic [`CALL_ARG`] bit remains
    /// set for recall-safe scans.
    pub const CALL_ARG_SLOT_BASE: u32 = 1 << 16;
    /// Edge from a call expression to argument slot 0.
    pub const CALL_ARG_0: u32 = CALL_ARG_SLOT_BASE;
    /// Edge from a call expression to argument slot 1.
    pub const CALL_ARG_1: u32 = CALL_ARG_SLOT_BASE << 1;
    /// Edge from a call expression to argument slot 2.
    pub const CALL_ARG_2: u32 = CALL_ARG_SLOT_BASE << 2;
    /// Edge from a call expression to argument slot 3.
    pub const CALL_ARG_3: u32 = CALL_ARG_SLOT_BASE << 3;
    /// Edge from a call expression to argument slot 4.
    pub const CALL_ARG_4: u32 = CALL_ARG_SLOT_BASE << 4;
    /// Edge from a call expression to argument slot 5.
    pub const CALL_ARG_5: u32 = CALL_ARG_SLOT_BASE << 5;
    /// Edge from a call expression to argument slot 6.
    pub const CALL_ARG_6: u32 = CALL_ARG_SLOT_BASE << 6;
    /// Edge from a call expression to argument slot 7.
    pub const CALL_ARG_7: u32 = CALL_ARG_SLOT_BASE << 7;

    /// Maximum directly-addressable CALL_ARG slot.
    pub const CALL_ARG_MAX_SLOT: u32 = 7;

    /// Slot-precise edge from a sized-input-read / sized-memory-copy /
    /// reallocator call to the argument carrying the byte-count
    /// (recv arg-2, memcpy arg-2, copy_from_user arg-2, realloc arg-1,
    /// fread arg-1, etc.). Walker emits this edge when the callee has
    /// an entry in `[<lang>.size_argument_slot]`.
    /// `size_argument_of($call)` walks back along this single edge
    /// instead of every CALL_ARG, restoring slot-precise FP elimination
    /// (every-arg-is-size over-match was the substrate-level FP
    /// source on every recv / memcpy / copy_from_user shape).
    pub const SIZE_ARG: u32 = 1 << 24;

    /// Block-membership edge: a CFG basic-block's entry node and every
    /// AST node contained in that block are joined by a BIDIRECTIONAL
    /// `BLOCK_MEMBER` pair (`block_entry -> node` and `node ->
    /// block_entry`). DOMINANCE idom edges only connect block-entry
    /// nodes to each other, so `dominates($a, $b)` on call-expression
    /// operands (which hang off blocks via PARENT, not the idom chain)
    /// would otherwise see an empty dominance graph. The
    /// `dominator_tree` shim traverses `DOMINANCE | BLOCK_MEMBER`, so a
    /// backward step from a call node reaches its block entry, walks the
    /// idom chain, then descends into each dominating block's contained
    /// nodes — yielding correct block-level dominance for arbitrary
    /// operands. Deliberately NOT in the CONTROL|DOMINANCE mask the CPU
    /// dominator bitmap (`sanitized_by`) uses, so that subsystem is
    /// unaffected.
    pub const BLOCK_MEMBER: u32 = 1 << 25;

    /// Build the per-slot mask. Slot N maps to
    /// `CALL_ARG_SLOT_BASE << N` for N in 0..=7. Beyond that the
    /// caller must fall back to the generic CALL_ARG bit (recall-safe
    /// but precision-loose) until the substrate widens to u64.
    #[must_use]
    pub const fn call_arg_slot(n: u32) -> u32 {
        if n > CALL_ARG_MAX_SLOT {
            CALL_ARG
        } else {
            CALL_ARG_SLOT_BASE << n
        }
    }
}

/// Canonical tag-family bitmasks matching the shared source-query `TagFamily`.
pub mod tag_family {
    /// `in_function` mask.
    pub const FUNCTION: u32 = 1 << 0;
    /// `in_file` mask.
    pub const FILE: u32 = 1 << 1;
    /// `in_package` mask.
    pub const PACKAGE: u32 = 1 << 2;
}

/// Canonical `NodeKind` constants mirroring the shared source-query enum.
pub mod node_kind {
    /// `Variable`.
    pub const VARIABLE: u32 = 1;
    /// `Call`.
    pub const CALL: u32 = 2;
    /// `Import`.
    pub const IMPORT: u32 = 3;
    /// `Literal`.
    pub const LITERAL: u32 = 4;
    /// `SSA`.
    pub const SSA: u32 = 5;
    /// `BasicBlock`.
    pub const BASIC_BLOCK: u32 = 6;
    /// `Binary`.
    pub const BINARY: u32 = 7;
    /// `FunctionDecl`.
    pub const FUNCTION_DECL: u32 = 8;
}

macro_rules! define_tag_family_predicate {
    (
        $module:ident,
        $function:ident,
        $op_id:literal,
        $family:expr,
        $fixture_tags:expr,
        $expected_nodeset:expr,
        $doc:literal
    ) => {
        #[doc = $doc]
        pub mod $module {
            use vyre_foundation::ir::Program;

            use crate::label::resolve_family::resolve_family;

            /// Canonical op id.
            pub const OP_ID: &str = $op_id;

            /// Build the canonical tag-family predicate program.
            #[must_use]
            pub fn $function(node_tags: &str, nodeset_out: &str, node_count: u32) -> Program {
                crate::program_region::tag_program(
                    OP_ID,
                    resolve_family(node_tags, nodeset_out, node_count, $family),
                )
            }

            /// CPU reference.
            #[must_use]
            #[cfg(any(test, feature = "cpu-parity"))]
            pub fn cpu_ref(node_tags: &[u32]) -> Vec<u32> {
                crate::label::resolve_family::cpu_ref(node_tags, $family)
            }

            #[cfg(feature = "inventory-registry")]
            inventory::submit! {
                crate::harness::OpEntry::new(
                    OP_ID,
                    || $function("tags", "nodeset", 4),
                    Some(|| {
                        let to_bytes = crate::predicate::inventory_u32_le_bytes;
                        vec![vec![
                            to_bytes($fixture_tags),
                            to_bytes(&[0]),
                        ]]
                    }),
                    Some(|| {
                        let to_bytes = crate::predicate::inventory_u32_le_bytes;
                        vec![vec![to_bytes($expected_nodeset)]]
                    }),
                )
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[test]
                fn cpu_ref_matches_inventory_fixture() {
                    assert_eq!(cpu_ref($fixture_tags), $expected_nodeset.to_vec());
                }
            }
        }
    };
}

macro_rules! define_fixed_forward_edge_predicate {
    (
        $module:ident,
        $function:ident,
        $op_id:literal,
        $edge_mask:expr,
        $edge_count:expr,
        $fixture_edge_offsets:expr,
        $fixture_edge_targets:expr,
        $fixture_edge_masks:expr,
        $expected_nodeset:expr,
        $module_doc:literal,
        $function_doc:literal,
        $region_label:literal
    ) => {
        #[doc = $module_doc]
        pub mod $module {
            use vyre_foundation::ir::Program;

            use crate::graph::program_graph::ProgramGraphShape;
            use crate::predicate::traversal::forward_edge_program;
            #[cfg(any(test, feature = "cpu-parity"))]
            use crate::predicate::traversal::{cpu_ref_forward, cpu_ref_forward_into};

            /// Canonical op id.
            pub const OP_ID: &str = $op_id;

            #[doc = $function_doc]
            #[must_use]
            pub fn $function(
                shape: ProgramGraphShape,
                frontier_in: &str,
                frontier_out: &str,
            ) -> Program {
                forward_edge_program(OP_ID, shape, frontier_in, frontier_out, $edge_mask)
            }

            /// CPU reference.
            #[must_use]
            #[cfg(any(test, feature = "cpu-parity"))]
            pub fn cpu_ref(
                node_count: u32,
                edge_offsets: &[u32],
                edge_targets: &[u32],
                edge_kind_mask: &[u32],
                frontier_in: &[u32],
            ) -> Vec<u32> {
                cpu_ref_forward(
                    node_count,
                    edge_offsets,
                    edge_targets,
                    edge_kind_mask,
                    frontier_in,
                    $edge_mask,
                )
            }

            /// CPU reference using caller-owned output storage.
            #[cfg(any(test, feature = "cpu-parity"))]
            pub fn cpu_ref_into(
                node_count: u32,
                edge_offsets: &[u32],
                edge_targets: &[u32],
                edge_kind_mask: &[u32],
                frontier_in: &[u32],
                out: &mut Vec<u32>,
            ) {
                cpu_ref_forward_into(
                    node_count,
                    edge_offsets,
                    edge_targets,
                    edge_kind_mask,
                    frontier_in,
                    $edge_mask,
                    out,
                );
            }

            #[cfg(feature = "inventory-registry")]
            inventory::submit! {
                crate::harness::OpEntry::new(
                    OP_ID,
                    || $function(ProgramGraphShape::new(4, $edge_count), "fin", "fout"),
                    Some(|| {
                        let b = crate::predicate::inventory_u32_le_bytes;
                        vec![vec![
                            b(&[2, 1, 1, 1]),
                            b($fixture_edge_offsets),
                            b($fixture_edge_targets),
                            b($fixture_edge_masks),
                            b(&[0, 0, 0, 0]),
                            b(&[0b0001]),
                            b(&[0]),
                        ]]
                    }),
                    Some(|| {
                        let b = crate::predicate::inventory_u32_le_bytes;
                        vec![vec![b($expected_nodeset)]]
                    }),
                )
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use crate::predicate::traversal::assert_region_op_id;

                #[test]
                fn preserves_wrapper_op_id() {
                    let program = $function(ProgramGraphShape::new(4, $edge_count), "fin", "fout");
                    assert_region_op_id(&program, OP_ID, $region_label);
                }
            }
        }
    };
}

pub mod arg_of;
define_fixed_forward_edge_predicate!(
    call_to,
    call_to,
    "vyre-primitives::predicate::call_to",
    crate::predicate::edge_kind::CALL_ARG,
    2,
    &[0, 1, 2, 2, 2],
    &[1, 2],
    &[2, 2],
    &[0b0010],
    "`call_to` - forward-traverse along `CALL_ARG` edges.",
    "Build a Program that emits the callee NodeSet reachable via `CallArg` edges from the input frontier.",
    "call_to"
);
pub mod edge;
mod traversal;
define_tag_family_predicate!(
    in_file,
    in_file,
    "vyre-primitives::predicate::in_file",
    crate::predicate::tag_family::FILE,
    &[2, 2, 0, 0],
    &[0b0011],
    "`in_file` - NodeSet of file-tagged nodes."
);
define_tag_family_predicate!(
    in_function,
    in_function,
    "vyre-primitives::predicate::in_function",
    crate::predicate::tag_family::FUNCTION,
    &[1, 0, 1, 0],
    &[0b0101],
    "`in_function` - NodeSet of function-tagged nodes."
);
define_tag_family_predicate!(
    in_package,
    in_package,
    "vyre-primitives::predicate::in_package",
    crate::predicate::tag_family::PACKAGE,
    &[4, 0, 4, 0],
    &[0b0101],
    "`in_package` - NodeSet of package-tagged nodes."
);
pub mod literal_of;
pub mod node_kind_eq;
define_fixed_forward_edge_predicate!(
    return_value_of,
    return_value_of,
    "vyre-primitives::predicate::return_value_of",
    crate::predicate::edge_kind::RETURN,
    1,
    &[0, 1, 1, 1, 1],
    &[1],
    &[4],
    &[0b0010],
    "`return_value_of` - forward-traverse along `RETURN` edges.",
    "Build a Program that emits the NodeSet of return-value bindings reached from the caller frontier via `Return` edges.",
    "return_value_of"
);
pub mod size_argument_of;

/// Little-endian `u32` word packing for [`inventory::submit!`] GPU fixtures.
///
/// Centralizes the repeated `to_le_bytes` flatten used by every graph
/// predicate's registry block (`audits/VYRE_PRIMITIVES_GAPS.md` dedup).
#[cfg(feature = "inventory-registry")]
pub(crate) fn inventory_u32_le_bytes(words: &[u32]) -> Vec<u8> {
    crate::wire::pack_u32_slice(words)
}

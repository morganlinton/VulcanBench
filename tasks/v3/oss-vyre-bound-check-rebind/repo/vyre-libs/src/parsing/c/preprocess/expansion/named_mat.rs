//! Materialized named macro-expansion dispatch builder.

#![allow(missing_docs)] // Internal macro-expansion helpers are documented at the owning module boundary.
use crate::parsing::c::preprocess::materialization::*;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::fnlike_mat::*;
use super::helpers::*;
use super::objlike_mat::*;
use super::*;

pub fn opt_named_macro_expansion_materialized(
    in_tok_types: &str,
    in_tok_starts: &str,
    in_tok_lens: &str,
    source_words: &str,
    macro_name_hashes: &str,
    macro_name_starts: &str,
    macro_name_lens: &str,
    macro_name_words: &str,
    macro_vals: &str,
    macro_sizes: &str,
    macro_kinds: &str,
    macro_param_counts: &str,
    macro_replacement_params: &str,
    macro_replacement_starts: &str,
    macro_replacement_lens: &str,
    macro_replacement_words: &str,
    runtime_counts: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_source_words: &str,
    out_tok_counts: &str,
    out_source_counts: &str,
    num_tokens: Expr,
    source_len: Expr,
    macro_replacement_source_len: Expr,
    max_input_tokens: u32,
    _max_source_bytes: u32,
    _max_replacement_source_bytes: u32,
    max_out_tokens: u32,
    max_out_source_bytes: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let tok_buffer_count = max_input_tokens.max(1);
    let out_buffer_count = max_out_tokens.max(1);
    let out_source_count = max_out_source_bytes.max(1);

    let mut process_current = emit_named_macro_scan_prefix(NamedMacroScanSpec {
        in_tok_types,
        in_tok_starts,
        in_tok_lens,
        source_words,
        source_layout: MacroByteLayout::RawU8,
        macro_name_hashes,
        macro_name_starts,
        macro_name_lens,
        macro_name_words,
        macro_name_layout: MacroByteLayout::RawU8,
        macro_vals,
        macro_kinds,
        macro_param_counts,
        source_len: source_len.clone(),
        decode_variadic_param_count: true,
    });
    process_current.push(Node::if_then_else(
        Expr::eq(Expr::var("named_macro_slot"), Expr::u32(EMPTY_MACRO_SLOT)),
        {
            let mut passthrough = emit_materialized_output_token(
                "passthrough",
                Expr::var("named_tok"),
                source_words,
                MacroByteLayout::RawU8,
                Expr::load(in_tok_starts, Expr::var("named_i")),
                Expr::load(in_tok_lens, Expr::var("named_i")),
                source_len.clone(),
                out_tok_types,
                out_tok_starts,
                out_tok_lens,
                out_source_words,
                max_out_tokens,
                max_out_source_bytes,
                "passthrough-token-source-span-out-of-bounds",
            );
            passthrough.push(Node::assign(
                "named_i",
                Expr::add(Expr::var("named_i"), Expr::u32(1)),
            ));
            passthrough
        },
        {
            let mut expanded =
                emit_named_replacement_prelude(macro_sizes, in_tok_types, num_tokens.clone());
            expanded.push(Node::if_then_else(
                Expr::eq(
                    Expr::var("named_macro_kind"),
                    Expr::u32(C_MACRO_KIND_OBJECT_LIKE),
                ),
                emit_materialized_object_like_replacement(
                    macro_vals,
                    macro_replacement_params,
                    macro_replacement_starts,
                    macro_replacement_lens,
                    macro_replacement_words,
                    MacroByteLayout::RawU8,
                    out_tok_types,
                    out_tok_starts,
                    out_tok_lens,
                    out_source_words,
                    macro_replacement_source_len.clone(),
                    max_out_tokens,
                    max_out_source_bytes,
                ),
                vec![Node::if_then_else(
                    Expr::eq(Expr::var("named_has_open_paren"), Expr::u32(0)),
                    {
                        let mut passthrough = emit_materialized_output_token(
                            "function_name_passthrough",
                            Expr::var("named_tok"),
                            source_words,
                            MacroByteLayout::RawU8,
                            Expr::load(in_tok_starts, Expr::var("named_i")),
                            Expr::load(in_tok_lens, Expr::var("named_i")),
                            source_len.clone(),
                            out_tok_types,
                            out_tok_starts,
                            out_tok_lens,
                            out_source_words,
                            max_out_tokens,
                            max_out_source_bytes,
                            "function-name-passthrough-source-span-out-of-bounds",
                        );
                        passthrough.push(Node::assign(
                            "named_i",
                            Expr::add(Expr::var("named_i"), Expr::u32(1)),
                        ));
                        passthrough
                    },
                    emit_materialized_function_like_replacement(
                        in_tok_types,
                        in_tok_starts,
                        in_tok_lens,
                        source_words,
                        MacroByteLayout::RawU8,
                        macro_vals,
                        macro_replacement_params,
                        macro_replacement_starts,
                        macro_replacement_lens,
                        macro_replacement_words,
                        MacroByteLayout::RawU8,
                        out_tok_types,
                        out_tok_starts,
                        out_tok_lens,
                        out_source_words,
                        "macro_arg_starts",
                        "macro_arg_ends",
                        num_tokens.clone(),
                        source_len.clone(),
                        macro_replacement_source_len.clone(),
                        max_out_tokens,
                        max_out_source_bytes,
                    ),
                )],
            ));
            expanded
        },
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(in_tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_buffer_count),
            BufferDecl::storage(in_tok_starts, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_buffer_count),
            BufferDecl::storage(in_tok_lens, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_buffer_count),
            BufferDecl::storage(source_words, 3, BufferAccess::ReadOnly, DataType::U8)
                .with_count(0),
            BufferDecl::storage(macro_name_hashes, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_name_starts, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_name_lens, 6, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_name_words, 7, BufferAccess::ReadOnly, DataType::U8)
                .with_count(0),
            BufferDecl::storage(macro_vals, 8, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_sizes, 9, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_kinds, 10, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(
                macro_param_counts,
                11,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(
                macro_replacement_params,
                12,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(
                macro_replacement_starts,
                13,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(
                macro_replacement_lens,
                14,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(
                macro_replacement_words,
                15,
                BufferAccess::ReadOnly,
                DataType::U8,
            )
            .with_count(0),
            BufferDecl::storage(runtime_counts, 16, BufferAccess::ReadOnly, DataType::U32)
                .with_count(3),
            BufferDecl::storage(out_tok_types, 17, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_buffer_count),
            BufferDecl::storage(out_tok_starts, 18, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_buffer_count),
            BufferDecl::storage(out_tok_lens, 19, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_buffer_count),
            BufferDecl::storage(out_source_words, 20, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_source_count),
            BufferDecl::storage(out_tok_counts, 21, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            BufferDecl::storage(
                out_source_counts,
                22,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(1),
            BufferDecl::storage(
                "macro_arg_starts",
                23,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(tok_buffer_count),
            BufferDecl::storage("macro_arg_ends", 24, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_buffer_count),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::opt_named_macro_expansion_materialized",
            vec![Node::if_then(
                Expr::eq(t, Expr::u32(0)),
                vec![
                    Node::let_bind("named_i", Expr::u32(0)),
                    Node::let_bind("named_out_idx", Expr::u32(0)),
                    Node::let_bind("named_source_out_idx", Expr::u32(0)),
                    Node::loop_for(
                        "named_cursor",
                        Expr::u32(0),
                        num_tokens,
                        vec![Node::if_then(
                            Expr::eq(Expr::var("named_cursor"), Expr::var("named_i")),
                            process_current,
                        )],
                    ),
                    Node::store(out_tok_counts, Expr::u32(0), Expr::var("named_out_idx")),
                    Node::store(
                        out_source_counts,
                        Expr::u32(C_MACRO_SOURCE_COUNT_BYTES),
                        Expr::var("named_source_out_idx"),
                    ),
                ],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::opt_named_macro_expansion_materialized_v6")
    .with_non_composable_with_self(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn materialized_program() -> Program {
        opt_named_macro_expansion_materialized(
            "in_tok_types",
            "in_tok_starts",
            "in_tok_lens",
            "source_words",
            "macro_name_hashes",
            "macro_name_starts",
            "macro_name_lens",
            "macro_name_words",
            "macro_vals",
            "macro_sizes",
            "macro_kinds",
            "macro_param_counts",
            "macro_replacement_params",
            "macro_replacement_starts",
            "macro_replacement_lens",
            "macro_replacement_words",
            "runtime_counts",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_source_words",
            "out_tok_counts",
            "out_source_counts",
            Expr::u32(3),
            Expr::u32(7),
            Expr::u32(5),
            4,
            7,
            5,
            8,
            32,
        )
    }

    #[test]
    fn materialized_macro_expansion_declares_raw_u8_input_byte_arenas() {
        let program = materialized_program();
        for name in [
            "source_words",
            "macro_name_words",
            "macro_replacement_words",
        ] {
            let buffer = program
                .buffers()
                .iter()
                .find(|buffer| buffer.name() == name)
                .expect("materialized macro expansion must declare input byte arena");
            assert_eq!(buffer.element(), DataType::U8, "{name}");
            assert_eq!(buffer.count(), 0, "{name}");
        }
    }

    #[test]
    fn materialized_macro_expansion_keeps_output_source_as_u32_byte_words() {
        let program = materialized_program();
        let buffer = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "out_source_words")
            .expect("materialized macro expansion must declare output source arena");
        assert_eq!(buffer.element(), DataType::U32);
        assert_eq!(buffer.count(), 32);
    }
}

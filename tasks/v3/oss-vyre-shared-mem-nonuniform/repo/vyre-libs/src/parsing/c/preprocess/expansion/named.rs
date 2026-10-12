//! Named macro-expansion dispatch builder.

#![allow(missing_docs)] // Internal macro-expansion helpers are documented at the owning module boundary.
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::fnlike::*;
use super::helpers::*;
use super::objlike::*;
use super::*;

pub fn opt_named_macro_expansion(
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
    out_tok_types: &str,
    out_tok_counts: &str,
    num_tokens: Expr,
    source_len: Expr,
    max_out_tokens: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let tok_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    let tok_buffer_count = tok_count.max(1);
    let source_count = match &source_len {
        Expr::LitU32(n) => *n,
        _ => 1,
    }
    .max(1);
    let out_buffer_count = max_out_tokens.max(1);

    let mut process_current = emit_named_macro_scan_prefix(NamedMacroScanSpec {
        in_tok_types,
        in_tok_starts,
        in_tok_lens,
        source_words,
        source_layout: MacroByteLayout::ExpandedU32,
        macro_name_hashes,
        macro_name_starts,
        macro_name_lens,
        macro_name_words,
        macro_name_layout: MacroByteLayout::ExpandedU32,
        macro_vals,
        macro_kinds,
        macro_param_counts,
        source_len: source_len.clone(),
        decode_variadic_param_count: false,
    });

    process_current.push(Node::if_then_else(
        Expr::eq(Expr::var("named_macro_slot"), Expr::u32(EMPTY_MACRO_SLOT)),
        {
            let mut passthrough =
                emit_one_output_token(out_tok_types, Expr::var("named_tok"), max_out_tokens);
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
                emit_object_like_replacement(
                    macro_vals,
                    macro_replacement_params,
                    out_tok_types,
                    max_out_tokens,
                ),
                vec![Node::if_then_else(
                    Expr::eq(Expr::var("named_has_open_paren"), Expr::u32(0)),
                    {
                        let mut passthrough = emit_one_output_token(
                            out_tok_types,
                            Expr::var("named_tok"),
                            max_out_tokens,
                        );
                        passthrough.push(Node::assign(
                            "named_i",
                            Expr::add(Expr::var("named_i"), Expr::u32(1)),
                        ));
                        passthrough
                    },
                    emit_function_like_replacement(
                        in_tok_types,
                        macro_vals,
                        macro_replacement_params,
                        out_tok_types,
                        "macro_arg_starts",
                        "macro_arg_ends",
                        num_tokens.clone(),
                        max_out_tokens,
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
            BufferDecl::storage(source_words, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(source_count),
            BufferDecl::storage(macro_name_hashes, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_name_starts, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_name_lens, 6, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_name_words, 7, BufferAccess::ReadOnly, DataType::U32)
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
            BufferDecl::storage(out_tok_types, 13, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_buffer_count),
            BufferDecl::storage(out_tok_counts, 14, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            BufferDecl::workgroup("macro_arg_starts", tok_buffer_count, DataType::U32),
            BufferDecl::workgroup("macro_arg_ends", tok_buffer_count, DataType::U32),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::opt_named_macro_expansion",
            vec![Node::if_then(
                Expr::eq(t, Expr::u32(0)),
                vec![
                    Node::let_bind("named_i", Expr::u32(0)),
                    Node::let_bind("named_out_idx", Expr::u32(0)),
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
                ],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::opt_named_macro_expansion")
    .with_non_composable_with_self(true)
}

//! Dynamic macro-expansion pipeline builder.

#![allow(missing_docs)] // Internal macro-expansion helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::helpers::*;
use super::*;

pub fn opt_dynamic_macro_expansion(
    in_tok_types: &str,
    macro_keys: &str,
    macro_vals: &str,
    macro_sizes: &str,
    out_tok_types: &str,
    out_tok_counts: &str,
    num_tokens: Expr,
    max_out_tokens: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let mut loop_body = vec![Node::let_bind("tok", Expr::load(in_tok_types, t.clone()))];
    loop_body.extend(emit_macro_lookup(
        "current",
        Expr::var("tok"),
        macro_keys,
        macro_vals,
        "macro_idx",
    ));
    loop_body.extend([
        Node::if_then(
            Expr::eq(Expr::var("tok"), Expr::u32(TOK_PREPROC)),
            vec![Node::assign("macro_idx", Expr::u32(EMPTY_MACRO_SLOT))],
        ),
        // Determine number of tokens to emit
        Node::let_bind("emit_count", Expr::u32(0)),
        Node::if_then_else(
            Expr::eq(Expr::var("macro_idx"), Expr::u32(EMPTY_MACRO_SLOT)),
            vec![Node::assign("emit_count", Expr::u32(1))], // Passthrough original token
            vec![Node::assign(
                "emit_count",
                Expr::load(macro_sizes, Expr::var("macro_idx")),
            )], // Fetch replacement sequence length
        ),
        Node::if_then(
            Expr::and(
                Expr::ne(Expr::var("macro_idx"), Expr::u32(EMPTY_MACRO_SLOT)),
                Expr::gt(
                    Expr::add(Expr::var("macro_idx"), Expr::var("emit_count")),
                    Expr::u32(MACRO_TABLE_SLOTS),
                ),
            ),
            vec![Node::trap(
                Expr::add(Expr::var("macro_idx"), Expr::var("emit_count")),
                "macro-replacement-range-out-of-bounds",
            )],
        ),
        Node::let_bind("warp_base_idx", Expr::u32(0)),
        Node::loop_for("prior", Expr::u32(0), t.clone(), {
            let mut prior_body = vec![Node::let_bind(
                "prior_tok",
                Expr::load(in_tok_types, Expr::var("prior")),
            )];
            prior_body.extend(emit_macro_lookup(
                "prior_lookup",
                Expr::var("prior_tok"),
                macro_keys,
                macro_vals,
                "prior_macro_idx",
            ));
            prior_body.extend([
                Node::if_then(
                    Expr::eq(Expr::var("prior_tok"), Expr::u32(TOK_PREPROC)),
                    vec![Node::assign("prior_macro_idx", Expr::u32(EMPTY_MACRO_SLOT))],
                ),
                Node::let_bind("prior_emit_count", Expr::u32(0)),
                Node::if_then_else(
                    Expr::eq(Expr::var("prior_macro_idx"), Expr::u32(EMPTY_MACRO_SLOT)),
                    vec![Node::assign("prior_emit_count", Expr::u32(1))],
                    vec![
                        Node::assign(
                            "prior_emit_count",
                            Expr::load(macro_sizes, Expr::var("prior_macro_idx")),
                        ),
                        Node::if_then(
                            Expr::gt(
                                Expr::add(
                                    Expr::var("prior_macro_idx"),
                                    Expr::var("prior_emit_count"),
                                ),
                                Expr::u32(MACRO_TABLE_SLOTS),
                            ),
                            vec![Node::trap(
                                Expr::add(
                                    Expr::var("prior_macro_idx"),
                                    Expr::var("prior_emit_count"),
                                ),
                                "macro-prior-replacement-range-out-of-bounds",
                            )],
                        ),
                    ],
                ),
                Node::assign(
                    "warp_base_idx",
                    Expr::add(Expr::var("warp_base_idx"), Expr::var("prior_emit_count")),
                ),
            ]);
            prior_body
        }),
        Node::let_bind(
            "emit_end_idx",
            Expr::add(Expr::var("warp_base_idx"), Expr::var("emit_count")),
        ),
        Node::if_then(
            Expr::gt(Expr::var("emit_end_idx"), Expr::u32(max_out_tokens)),
            vec![Node::trap(
                Expr::var("emit_end_idx"),
                "macro-expansion-output-overflow",
            )],
        ),
        // 3. Dynamic Parallel Token Pasting
        Node::if_then_else(
            Expr::eq(Expr::var("macro_idx"), Expr::u32(EMPTY_MACRO_SLOT)),
            vec![
                // Fast path: Unchanged Token
                Node::store(out_tok_types, Expr::var("warp_base_idx"), Expr::var("tok")),
            ],
            vec![
                // Complex path: Expanding out multiple tokens (e.g. PAGE_SIZE -> (1 << 12))
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::var("emit_count"),
                    vec![
                        Node::let_bind(
                            "replacement_tok",
                            Expr::load(
                                macro_vals,
                                Expr::add(Expr::var("macro_idx"), Expr::var("i")),
                            ),
                        ),
                        Node::store(
                            out_tok_types,
                            Expr::add(Expr::var("warp_base_idx"), Expr::var("i")),
                            Expr::var("replacement_tok"),
                        ),
                    ],
                ),
            ],
        ),
        Node::if_then(
            Expr::eq(Expr::add(t.clone(), Expr::u32(1)), num_tokens.clone()),
            vec![Node::store(
                out_tok_counts,
                Expr::u32(0),
                Expr::var("emit_end_idx"),
            )],
        ),
    ]);

    let tok_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    let tok_buffer_count = tok_count.max(1);
    let out_buffer_count = max_out_tokens.max(1);
    Program::wrapped(
        vec![
            BufferDecl::storage(in_tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_buffer_count),
            BufferDecl::storage(macro_keys, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_vals, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(macro_sizes, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(MACRO_TABLE_SLOTS),
            BufferDecl::storage(out_tok_types, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_buffer_count),
            BufferDecl::storage(out_tok_counts, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::opt_dynamic_macro_expansion",
            vec![child_phase(
                "vyre-libs::parsing::opt_dynamic_macro_expansion",
                vyre_primitives::bitset::select::OP_ID,
                vec![Node::if_then(Expr::lt(t.clone(), num_tokens), loop_body)],
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::opt_dynamic_macro_expansion")
    .with_non_composable_with_self(true)
}

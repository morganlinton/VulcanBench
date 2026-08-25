//! GPU `#define` row parser.
//!
//! Per `TOK_PREPROC` token classified as `TOK_PP_DEFINE`, extract the
//! macro name byte span, optional arg-list byte span, and replacement
//! body byte span. Per-thread, no inter-token state.
//!
//! ## Output columns (one row per token)
//!
//! - `name_start`, `name_len`         -  macro name byte span in `source`.
//! - `args_start`, `args_len`         -  arg-list span (between the `(`
//!                                     immediately after the name and
//!                                     the matching `)`). `args_len = 0`
//!                                     for object-like macros.
//! - `body_start`, `body_len`         -  replacement body span (with
//!                                     trailing horizontal whitespace
//!                                     trimmed).
//! - `is_function_like`               -  `1` if there was a `(`
//!                                     immediately after the name, else 0.
//!
//! Non-DEFINE rows get all-zero output.
//!
//! ## Real-GPU lowering note
//!
//! Same conventions as the rest of the directive-classify family  -
//! `source` is declared as packed U32 so reference-eval and naga-
//! emitted real GPU agree on word-indexed access; byte extraction is
//! inline. Fixed-width whitespace probes keep directive alignment cheap, while
//! macro names and function-like argument lists are scanned with per-row GPU
//! loops bounded by the directive token length. That keeps the compiled program
//! shape independent of translation-unit size without truncating long
//! clang-valid macro identifiers or parameter lists.

use super::gpu_directive_parse_shared::{
    directive_program_from_parse_with_source_layout, push_bounded_byte_scan_until,
    push_c_identifier_span, push_directive_row_bounds, push_hash_and_keyword_start,
    push_keyword_end, push_ws_skip_from_expr, safe_source_byte_expr,
    trailing_ws_flag as is_trailing_ws, DirectiveOutputColumn, DirectiveSourceLayout,
    DirectiveThreadLayout, MAX_DIRECTIVE_WS_PREFIX as MAX_WS_PREFIX,
};
use crate::parsing::c::lex::tokens::TOK_PP_DEFINE;
use vyre::ir::{Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-libs::parsing::c::preprocess::gpu_define_parse";

/// Canonical binding for the input per-token start-offset buffer.
pub const BINDING_TOK_STARTS: u32 = 0;
/// Canonical binding for the input per-token length buffer.
pub const BINDING_TOK_LENS: u32 = 1;
/// Canonical binding for the input directive-kinds buffer.
pub const BINDING_DIRECTIVE_KINDS: u32 = 2;
/// Canonical binding for the input source bytes (packed U32).
pub const BINDING_SOURCE: u32 = 3;
/// Canonical binding for the output `name_start` column.
pub const BINDING_NAME_START_OUT: u32 = 4;
/// Canonical binding for the output `name_len` column.
pub const BINDING_NAME_LEN_OUT: u32 = 5;
/// Canonical binding for the output `args_start` column.
pub const BINDING_ARGS_START_OUT: u32 = 6;
/// Canonical binding for the output `args_len` column.
pub const BINDING_ARGS_LEN_OUT: u32 = 7;
/// Canonical binding for the output `body_start` column.
pub const BINDING_BODY_START_OUT: u32 = 8;
/// Canonical binding for the output `body_len` column.
pub const BINDING_BODY_LEN_OUT: u32 = 9;
/// Canonical binding for the output `is_function_like` column.
pub const BINDING_IS_FUNCTION_LIKE_OUT: u32 = 10;

const OUTPUT_COLUMNS: [DirectiveOutputColumn; 7] = [
    DirectiveOutputColumn {
        name: "name_start_out",
        binding: BINDING_NAME_START_OUT,
    },
    DirectiveOutputColumn {
        name: "name_len_out",
        binding: BINDING_NAME_LEN_OUT,
    },
    DirectiveOutputColumn {
        name: "args_start_out",
        binding: BINDING_ARGS_START_OUT,
    },
    DirectiveOutputColumn {
        name: "args_len_out",
        binding: BINDING_ARGS_LEN_OUT,
    },
    DirectiveOutputColumn {
        name: "body_start_out",
        binding: BINDING_BODY_START_OUT,
    },
    DirectiveOutputColumn {
        name: "body_len_out",
        binding: BINDING_BODY_LEN_OUT,
    },
    DirectiveOutputColumn {
        name: "is_function_like_out",
        binding: BINDING_IS_FUNCTION_LIKE_OUT,
    },
];

/// Length of the `define` keyword (6 bytes), used to step past it.
const DEFINE_KW_LEN: u32 = 6;

/// Build the `#define` row parser `Program`.
///
/// `num_tokens` is kept ONLY to size the host-allocated output buffers
/// (the CUDA backend rejects readback when output buffers don't have a
/// static byte length). The kernel BODY itself uses `Expr::buf_len()` for
/// every per-thread bound  -  so the program AST is independent of the
/// host's input/source size and the dispatcher's pipeline cache hits
#[must_use]
pub fn gpu_define_parse(num_tokens: u32, source_len: u32) -> Program {
    gpu_define_parse_with_source_layout(num_tokens, source_len, DirectiveSourceLayout::PackedU32)
}

/// Build the `#define` row parser over raw `DataType::U8` source bytes.
#[must_use]
pub fn gpu_define_parse_u8(num_tokens: u32, source_len: u32) -> Program {
    gpu_define_parse_with_source_layout(num_tokens, source_len, DirectiveSourceLayout::RawU8)
}

fn gpu_define_parse_with_source_layout(
    num_tokens: u32,
    source_len: u32,
    source_layout: DirectiveSourceLayout,
) -> Program {
    let t = Expr::var("t");
    let safe_load = |addr: Expr| safe_source_byte_expr(source_layout, addr);

    let mut parse: Vec<Node> = Vec::new();
    push_directive_row_bounds(&mut parse);
    push_hash_and_keyword_start(&mut parse, source_layout);
    push_keyword_end(&mut parse, Expr::u32(DEFINE_KW_LEN));
    push_ws_skip_from_expr(
        &mut parse,
        source_layout,
        "np",
        Expr::var("post_kw"),
        "name_skip",
        "name_start_val",
    );
    push_c_identifier_span(
        &mut parse,
        source_layout,
        "name_start_val",
        "name_len_val",
        "name_done",
    );

    // ---- Step 5: function-like check (next byte after name is `(`?) ----
    parse.push(Node::let_bind(
        "after_name_idx",
        Expr::add(Expr::var("name_start_val"), Expr::var("name_len_val")),
    ));
    parse.push(Node::let_bind(
        "after_name_byte",
        safe_load(Expr::var("after_name_idx")),
    ));
    parse.push(Node::let_bind(
        "is_func_val",
        Expr::select(
            Expr::eq(Expr::var("after_name_byte"), Expr::u32(b'(' as u32)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));

    // ---- Step 6: scan args bytes for first `)` (function-like only) ----
    // args_start_val_raw = after_name_idx + 1 (past the `(`). For
    // object-like macros this position is meaningless; we mask the
    // output stores below behind `is_func_val == 1`.
    parse.push(Node::let_bind(
        "args_start_val_raw",
        Expr::add(Expr::var("after_name_idx"), Expr::u32(1)),
    ));
    push_bounded_byte_scan_until(
        &mut parse,
        source_layout,
        "args_i",
        "args_start_val_raw",
        "args_scan_limit",
        "args_byte",
        "args_len_val_raw",
        "args_done",
        Expr::u32(b')' as u32),
        Expr::eq(Expr::var("is_func_val"), Expr::u32(1)),
    );

    // ---- Step 7: body span ----
    // body_pre_start = position right after the closing `)` for
    // function-like macros; right after the name otherwise.
    parse.push(Node::let_bind(
        "body_pre_start",
        Expr::select(
            Expr::eq(Expr::var("is_func_val"), Expr::u32(1)),
            Expr::select(
                Expr::eq(Expr::var("args_done"), Expr::u32(1)),
                Expr::add(
                    Expr::add(
                        Expr::var("args_start_val_raw"),
                        Expr::var("args_len_val_raw"),
                    ),
                    Expr::u32(1),
                ),
                Expr::var("tok_end"),
            ),
            Expr::var("after_name_idx"),
        ),
    ));
    // Skip horizontal WS between `)` (or name) and the start of the body.
    push_ws_skip_from_expr(
        &mut parse,
        source_layout,
        "bp",
        Expr::var("body_pre_start"),
        "body_skip",
        "body_start_val",
    );

    // ---- Step 8: trim trailing whitespace (incl. \n/\r) from the body ----
    // We probe the LAST MAX_WS_PREFIX bytes of the row and count a
    // trailing-WS run. The body length is `tok_end - body_start_val -
    // trailing_ws_count` clamped to >= 0.
    for q in 0..MAX_WS_PREFIX {
        // tb_q = byte at tok_end - 1 - q (last byte first when q=0).
        parse.push(Node::let_bind(
            format!("tb_{q}"),
            Expr::select(
                Expr::lt(
                    Expr::add(Expr::var("body_start_val"), Expr::u32(q + 1)),
                    Expr::add(Expr::var("tok_end"), Expr::u32(1)),
                ),
                safe_load(Expr::sub(Expr::var("tok_end"), Expr::u32(q + 1))),
                Expr::u32(0),
            ),
        ));
    }
    for q in 0..MAX_WS_PREFIX {
        parse.push(Node::let_bind(
            format!("tb_ws_{q}"),
            is_trailing_ws(Expr::var(format!("tb_{q}"))),
        ));
    }
    // trailing_ws_count = first q in [0, MAX_WS_PREFIX) where tb_ws_q
    // == 0 (the run of trailing WS bytes). Same chained-Select shape
    // as `ws_skip_expr` but reading the `tb_ws_*` bindings.
    let trailing_ws_expr = {
        let mut acc = Expr::u32(MAX_WS_PREFIX);
        for q in (0..MAX_WS_PREFIX).rev() {
            let mut prefix_ws = Expr::u32(1);
            for r in 0..q {
                prefix_ws = Expr::bitand(prefix_ws, Expr::var(format!("tb_ws_{r}")));
            }
            let tb_q_not_ws = Expr::select(
                Expr::eq(Expr::var(format!("tb_ws_{q}")), Expr::u32(0)),
                Expr::u32(1),
                Expr::u32(0),
            );
            let cond_u32 = Expr::bitand(tb_q_not_ws, prefix_ws);
            acc = Expr::select(Expr::eq(cond_u32, Expr::u32(1)), Expr::u32(q), acc);
        }
        acc
    };
    parse.push(Node::let_bind("trailing_ws_count", trailing_ws_expr));
    // body_len_val = max(0, (tok_end - trailing_ws_count) - body_start_val).
    parse.push(Node::let_bind(
        "body_end_trimmed",
        Expr::sub(Expr::var("tok_end"), Expr::var("trailing_ws_count")),
    ));
    parse.push(Node::let_bind(
        "body_len_val",
        Expr::select(
            Expr::lt(Expr::var("body_start_val"), Expr::var("body_end_trimmed")),
            Expr::sub(Expr::var("body_end_trimmed"), Expr::var("body_start_val")),
            Expr::u32(0),
        ),
    ));

    // ---- Step 9: commit ----
    // Stores fire only when found_hash == 1. The `is_func` masking
    // for args fields is handled by storing 0 unconditionally for
    // non-function-like rows.
    parse.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("found_hash"), Expr::u32(1)),
            Expr::gt(Expr::var("name_len_val"), Expr::u32(0)),
        ),
        vec![
            Node::store("name_start_out", t.clone(), Expr::var("name_start_val")),
            Node::store("name_len_out", t.clone(), Expr::var("name_len_val")),
            Node::store("body_start_out", t.clone(), Expr::var("body_start_val")),
            Node::store("body_len_out", t.clone(), Expr::var("body_len_val")),
            Node::store("is_function_like_out", t.clone(), Expr::var("is_func_val")),
            Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var("is_func_val"), Expr::u32(1)),
                    Expr::eq(Expr::var("args_done"), Expr::u32(1)),
                ),
                vec![
                    Node::store("args_start_out", t.clone(), Expr::var("args_start_val_raw")),
                    Node::store("args_len_out", t.clone(), Expr::var("args_len_val_raw")),
                ],
            ),
        ],
    ));

    directive_program_from_parse_with_source_layout(
        OP_ID,
        num_tokens,
        source_len,
        source_layout,
        &OUTPUT_COLUMNS,
        DirectiveThreadLayout::InvocationId,
        Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_DEFINE)),
        parse,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::DataType;

    #[test]
    fn op_id_is_canonical_and_stable() {
        assert_eq!(OP_ID, "vyre-libs::parsing::c::preprocess::gpu_define_parse");
    }

    #[test]
    fn build_program_returns_well_formed_program() {
        let p = gpu_define_parse(8, 64);
        assert_eq!(p.buffers().len(), 11);
        assert_eq!(p.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn source_buffer_layouts_preserve_packed_abi_and_raw_u8_variant() {
        let packed = gpu_define_parse(8, 64);
        let raw_u8 = gpu_define_parse_u8(8, 64);
        let packed_source = packed
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: packed define parser source buffer must exist");
        let raw_u8_source = raw_u8
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: raw-U8 define parser source buffer must exist");

        assert_eq!(packed_source.element(), DataType::U32);
        assert_ne!(packed_source.count(), 0);
        assert_eq!(raw_u8_source.element(), DataType::U8);
        assert_eq!(raw_u8_source.count(), 0);
    }
}

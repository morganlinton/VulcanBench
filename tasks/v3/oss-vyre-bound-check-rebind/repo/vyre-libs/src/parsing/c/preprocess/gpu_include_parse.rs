//! GPU `#include` row parser.
//!
//! Phase 17b.7: per `TOK_PREPROC` token classified as
//! `TOK_PP_INCLUDE` or `TOK_PP_INCLUDE_NEXT`, extract the include
//! path's byte span and whether it was the `<…>` (system) form or
//! `"…"` (local) form. Per-thread, fully parallel.
//!
//! ## Output columns (one row per token)
//!
//! - `path_start`, `path_len`         -  byte span between the
//!                                     delimiters (`<`/`>` or `"`/`"`).
//! - `is_system`                      -  `1` for `<…>`, `0` for `"…"`.
//!
//! Non-INCLUDE rows get all-zero output. `path_len == 0` after this
//! kernel means "not a parsed `#include` row"  -  equivalent to the CPU
//! `parse_include_literal` returning `None`/error.
//!
//! ## Real-GPU lowering note
//!
//! Two real-GPU lowering pitfalls (both shared with
//! `gpu_directive_metadata`):
//!
//! 1. `DataType::U8` storage buffers are emitted by vyre-emit-naga as
//!    `array<u32>` (WGSL has no u8 storage). `Expr::load("source",
//!    addr)` therefore returns the u32 word at index `addr`, not the
//!    byte at byte-address `addr`. The kernel does the byte
//!    extraction inline so it produces the correct value on both
//!    backends.
//! 2. Whitespace skipping uses fixed-depth chained Selects because C
//!    directive separators are short in practice. Path extraction is
//!    bounded by the directive row length, so Linux-scale include paths
//!    are not truncated by a compile-time probe cap.
//!
//! ## Wire layout
//!
//! Inputs:
//!   - `tok_starts` (U32), `tok_lens` (U32),
//!     `directive_kinds` (U32)  -  output of 17a.
//!   - `source` (U8).
//!
//! Outputs (all U32, one element per token):
//!   - `path_start_out`, `path_len_out`, `is_system_out`.

use super::gpu_directive_parse_shared::{
    directive_program_from_parse_with_source_layout, push_bounded_byte_scan_until,
    push_directive_row_bounds, push_hash_and_keyword_start, push_keyword_end,
    push_ws_skip_from_expr, safe_source_byte_expr, DirectiveOutputColumn, DirectiveSourceLayout,
    DirectiveThreadLayout,
};
use crate::parsing::c::lex::tokens::{TOK_PP_INCLUDE, TOK_PP_INCLUDE_NEXT};
use vyre::ir::{Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-libs::parsing::c::preprocess::gpu_include_parse_v2";

/// Canonical binding for the input per-token start-offset buffer.
pub const BINDING_TOK_STARTS: u32 = 0;
/// Canonical binding for the input per-token length buffer.
pub const BINDING_TOK_LENS: u32 = 1;
/// Canonical binding for the input directive-kinds buffer.
pub const BINDING_DIRECTIVE_KINDS: u32 = 2;
/// Canonical binding for the input source bytes.
pub const BINDING_SOURCE: u32 = 3;
/// Canonical binding for the output `path_start` column.
pub const BINDING_PATH_START_OUT: u32 = 4;
/// Canonical binding for the output `path_len` column.
pub const BINDING_PATH_LEN_OUT: u32 = 5;
/// Canonical binding for the output `is_system` column.
pub const BINDING_IS_SYSTEM_OUT: u32 = 6;

const OUTPUT_COLUMNS: [DirectiveOutputColumn; 3] = [
    DirectiveOutputColumn {
        name: "path_start_out",
        binding: BINDING_PATH_START_OUT,
    },
    DirectiveOutputColumn {
        name: "path_len_out",
        binding: BINDING_PATH_LEN_OUT,
    },
    DirectiveOutputColumn {
        name: "is_system_out",
        binding: BINDING_IS_SYSTEM_OUT,
    },
];

/// Build the 17b.7 `#include` row parser `Program`.
///
/// Hybrid runtime/static-bound: kernel BODY uses `Expr::buf_len()` for
/// every per-thread bound (so program AST is constant across files),
/// `num_tokens` is kept ONLY for output buffer sizing (CUDA backend
/// requires static byte length for readback), `source_len` is unused.
#[must_use]
pub fn gpu_include_parse(num_tokens: u32, source_len: u32) -> Program {
    gpu_include_parse_with_source_layout(num_tokens, source_len, DirectiveSourceLayout::PackedU32)
}

/// Build the 17b.7 `#include` row parser over raw `DataType::U8` source bytes.
#[must_use]
pub fn gpu_include_parse_u8(num_tokens: u32, source_len: u32) -> Program {
    gpu_include_parse_with_source_layout(num_tokens, source_len, DirectiveSourceLayout::RawU8)
}

fn gpu_include_parse_with_source_layout(
    num_tokens: u32,
    source_len: u32,
    source_layout: DirectiveSourceLayout,
) -> Program {
    let t = Expr::var("t");
    let safe_load = |addr: Expr| safe_source_byte_expr(source_layout, addr);

    let mut parse: Vec<Node> = Vec::new();
    push_directive_row_bounds(&mut parse);
    push_hash_and_keyword_start(&mut parse, source_layout);

    // ---- step 3: skip past keyword. kw_len = 7 (`include`) or 12
    // (`include_next`). Decided by `kind`. ----
    parse.push(Node::let_bind(
        "kw_len_skip",
        Expr::select(
            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_INCLUDE_NEXT)),
            Expr::u32(12),
            Expr::u32(7),
        ),
    ));
    push_keyword_end(&mut parse, Expr::var("kw_len_skip"));

    // ---- step 4: skip WS between keyword and delimiter. ----
    push_ws_skip_from_expr(
        &mut parse,
        source_layout,
        "dp",
        Expr::var("post_kw"),
        "delim_skip",
        "delim_pos",
    );

    // ---- step 5: classify delimiter. ----
    parse.push(Node::let_bind(
        "delim_byte",
        safe_load(Expr::var("delim_pos")),
    ));
    parse.push(Node::let_bind(
        "is_angle",
        Expr::select(
            Expr::eq(Expr::var("delim_byte"), Expr::u32(b'<' as u32)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    parse.push(Node::let_bind(
        "is_quote",
        Expr::select(
            Expr::eq(Expr::var("delim_byte"), Expr::u32(b'"' as u32)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    parse.push(Node::let_bind(
        "valid_delim",
        Expr::select(
            Expr::or(
                Expr::eq(Expr::var("is_angle"), Expr::u32(1)),
                Expr::eq(Expr::var("is_quote"), Expr::u32(1)),
            ),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    parse.push(Node::let_bind(
        "close_byte",
        Expr::select(
            Expr::eq(Expr::var("is_angle"), Expr::u32(1)),
            Expr::u32(b'>' as u32),
            Expr::u32(b'"' as u32),
        ),
    ));
    parse.push(Node::let_bind(
        "path_start_val",
        Expr::add(Expr::var("delim_pos"), Expr::u32(1)),
    ));

    // ---- step 6: scan path bytes to the directive row end for the
    // closing delimiter. This used to be a fixed 48-byte unrolled
    // probe, which silently rejected long Linux/generated include
    // paths. The row-length loop keeps the program shape constant but
    // removes the semantic cap.
    push_bounded_byte_scan_until(
        &mut parse,
        source_layout,
        "path_i",
        "path_start_val",
        "path_scan_limit",
        "path_byte",
        "path_len_val",
        "path_done",
        Expr::var("close_byte"),
        Expr::eq(Expr::u32(1), Expr::u32(1)),
    );

    // ---- step 7: commit if found_hash AND valid_delim ----
    // Both are u32 0/1; bitand stays u32; convert to bool for if_then.
    parse.push(Node::if_then(
        Expr::eq(
            Expr::bitand(
                Expr::bitand(Expr::var("found_hash"), Expr::var("valid_delim")),
                Expr::var("path_done"),
            ),
            Expr::u32(1),
        ),
        vec![
            Node::store("path_start_out", t.clone(), Expr::var("path_start_val")),
            Node::store("path_len_out", t.clone(), Expr::var("path_len_val")),
            Node::store("is_system_out", t.clone(), Expr::var("is_angle")),
        ],
    ));

    directive_program_from_parse_with_source_layout(
        OP_ID,
        num_tokens,
        source_len,
        source_layout,
        &OUTPUT_COLUMNS,
        DirectiveThreadLayout::WorkgroupLinear,
        Expr::or(
            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_INCLUDE)),
            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_INCLUDE_NEXT)),
        ),
        parse,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::DataType;

    #[test]
    fn op_id_is_canonical_and_stable() {
        assert_eq!(
            OP_ID,
            "vyre-libs::parsing::c::preprocess::gpu_include_parse_v2"
        );
    }

    #[test]
    fn build_program_returns_well_formed_program() {
        let p = gpu_include_parse(8, 64);
        assert_eq!(p.buffers().len(), 7);
        assert_eq!(p.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn source_buffer_layouts_preserve_packed_abi_and_raw_u8_variant() {
        let packed = gpu_include_parse(8, 64);
        let raw_u8 = gpu_include_parse_u8(8, 64);
        let packed_source = packed
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: packed include parser source buffer must exist");
        let raw_u8_source = raw_u8
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: raw-U8 include parser source buffer must exist");

        assert_eq!(packed_source.element(), DataType::U32);
        assert_ne!(packed_source.count(), 0);
        assert_eq!(raw_u8_source.element(), DataType::U8);
        assert_eq!(raw_u8_source.count(), 0);
    }
}

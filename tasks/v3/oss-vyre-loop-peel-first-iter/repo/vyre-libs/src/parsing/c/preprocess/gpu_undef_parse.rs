//! GPU `#undef` row parser.
//!
//! Per `TOK_PREPROC` token classified as `TOK_PP_UNDEF`, extract the
//! macro-name byte span. Per-thread, fully parallel.
//!
//! ## Output columns (one row per token)
//!
//! - `name_start`, `name_len`  -  byte span of the macro name within
//!   `source`. Non-UNDEF rows get all-zero output. `name_len == 0`
//!   after this kernel means "not a parsed `#undef` row"  -  equivalent
//!   to the CPU `parse_undef_name` returning `None`/error.
//!
//! ## Wire layout
//!
//! Inputs:
//!   - `tok_starts` (U32), `tok_lens` (U32),
//!     `directive_kinds` (U32)  -  output of `gpu_directive_metadata`.
//!   - `source` (U32 packed bytes; see real-GPU note).
//!
//! Outputs (all U32, one element per token):
//!   - `name_start_out`, `name_len_out`.
//!
//! ## Real-GPU lowering note
//!
//! Same conventions as the rest of the directive-classify family:
//! `source` is declared as packed U32 so reference-eval and
//! naga-emitted real GPU agree on word-indexed access; the byte
//! extraction is in `load_byte_u32`. Macro-name extraction is bounded
//! by the directive row length, not by a compile-time identifier cap.

use super::gpu_directive_parse_shared::{
    directive_program_from_parse_with_source_layout, push_c_identifier_span,
    push_directive_row_bounds, push_hash_and_keyword_start, push_keyword_end,
    push_ws_skip_from_expr, DirectiveOutputColumn, DirectiveSourceLayout, DirectiveThreadLayout,
};
use crate::parsing::c::lex::tokens::TOK_PP_UNDEF;
use vyre::ir::{Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-libs::parsing::c::preprocess::gpu_undef_parse_v2";

/// Canonical binding for the input per-token start-offset buffer.
pub const BINDING_TOK_STARTS: u32 = 0;
/// Canonical binding for the input per-token length buffer.
pub const BINDING_TOK_LENS: u32 = 1;
/// Canonical binding for the input directive-kinds buffer.
pub const BINDING_DIRECTIVE_KINDS: u32 = 2;
/// Canonical binding for the input source bytes (packed U32).
pub const BINDING_SOURCE: u32 = 3;
/// Canonical binding for the output `undef_name_start` column.
/// Renamed from `name_start_out` to avoid colliding with
/// `gpu_define_parse`'s own `name_start_out` when both kernels are
/// fused into a single dispatch (see gpu_extract_directive_payloads).
pub const BINDING_NAME_START_OUT: u32 = 4;
/// Canonical binding for the output `undef_name_len` column.
pub const BINDING_NAME_LEN_OUT: u32 = 5;

const OUTPUT_COLUMNS: [DirectiveOutputColumn; 2] = [
    DirectiveOutputColumn {
        name: "undef_name_start_out",
        binding: BINDING_NAME_START_OUT,
    },
    DirectiveOutputColumn {
        name: "undef_name_len_out",
        binding: BINDING_NAME_LEN_OUT,
    },
];

/// Length of `undef` keyword (5 bytes), used to step past it.
const UNDEF_KW_LEN: u32 = 5;

/// Build the `#undef` row parser `Program`.
///
/// Hybrid runtime/static-bound: kernel BODY uses `Expr::buf_len()` for
/// per-thread bounds, `num_tokens` is kept for output sizing, `source_len`
/// is unused.
#[must_use]
pub fn gpu_undef_parse(num_tokens: u32, source_len: u32) -> Program {
    gpu_undef_parse_with_source_layout(num_tokens, source_len, DirectiveSourceLayout::PackedU32)
}

/// Build the `#undef` row parser over raw `DataType::U8` source bytes.
#[must_use]
pub fn gpu_undef_parse_u8(num_tokens: u32, source_len: u32) -> Program {
    gpu_undef_parse_with_source_layout(num_tokens, source_len, DirectiveSourceLayout::RawU8)
}

fn gpu_undef_parse_with_source_layout(
    num_tokens: u32,
    source_len: u32,
    source_layout: DirectiveSourceLayout,
) -> Program {
    let t = Expr::var("t");

    let mut parse: Vec<Node> = Vec::new();
    push_directive_row_bounds(&mut parse);
    push_hash_and_keyword_start(&mut parse, source_layout);
    push_keyword_end(&mut parse, Expr::u32(UNDEF_KW_LEN));
    push_ws_skip_from_expr(
        &mut parse,
        source_layout,
        "np",
        Expr::var("post_kw"),
        "name_skip",
        "name_start_val",
    );

    // Scan to the directive row end. This removes the old 64-byte
    // macro-name cap while preserving C identifier start/continue
    // semantics.
    push_c_identifier_span(
        &mut parse,
        source_layout,
        "name_start_val",
        "name_len_val",
        "name_done",
    );
    parse.push(Node::let_bind(
        "valid_name",
        Expr::select(
            Expr::ne(Expr::var("name_len_val"), Expr::u32(0)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));

    // Commit if found_hash AND valid_name.
    parse.push(Node::if_then(
        Expr::eq(
            Expr::bitand(Expr::var("found_hash"), Expr::var("valid_name")),
            Expr::u32(1),
        ),
        vec![
            Node::store(
                "undef_name_start_out",
                t.clone(),
                Expr::var("name_start_val"),
            ),
            Node::store("undef_name_len_out", t.clone(), Expr::var("name_len_val")),
        ],
    ));

    directive_program_from_parse_with_source_layout(
        OP_ID,
        num_tokens,
        source_len,
        source_layout,
        &OUTPUT_COLUMNS,
        DirectiveThreadLayout::InvocationId,
        Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_UNDEF)),
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
            "vyre-libs::parsing::c::preprocess::gpu_undef_parse_v2"
        );
    }

    #[test]
    fn build_program_returns_well_formed_program() {
        let p = gpu_undef_parse(8, 64);
        assert_eq!(p.buffers().len(), 6);
        assert_eq!(p.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn source_buffer_layouts_preserve_packed_abi_and_raw_u8_variant() {
        let packed = gpu_undef_parse(8, 64);
        let raw_u8 = gpu_undef_parse_u8(8, 64);
        let packed_source = packed
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: packed undef parser source buffer must exist");
        let raw_u8_source = raw_u8
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: raw-U8 undef parser source buffer must exist");

        assert_eq!(packed_source.element(), DataType::U32);
        assert_ne!(packed_source.count(), 0);
        assert_eq!(raw_u8_source.element(), DataType::U8);
        assert_eq!(raw_u8_source.count(), 0);
    }
}

//! GPU `#ifdef` / `#ifndef` payload evaluator.
//!
//! Per-token, given the directive_kind already classified by
//! `gpu_directive_metadata`, parse the single identifier payload and
//! look it up in the host-supplied `defined_macros` table. For
//! `TOK_PP_IFDEF` emit `1` if defined else `0`. For `TOK_PP_IFNDEF`
//! emit the complement. For every other directive kind emit `0`.
//!
//! ## Wire layout
//!
//! Inputs:
//!   - `tok_starts` (U32)  -  per-token byte offset into `source`.
//!   - `tok_lens` (U32)  -  per-token byte length.
//!   - `directive_kinds` (U32)  -  output of `gpu_directive_metadata`.
//!   - `source` (U32 packed bytes for `gpu_ifdef_value`, raw U8 bytes for
//!     `gpu_ifdef_value_u8`).
//!   - `macro_names_packed` (U32 packed bytes for `gpu_ifdef_value`, raw U8
//!     bytes for `gpu_ifdef_value_u8`)  -  concatenated defined-macro name
//!     bytes. Empty when no macros are defined.
//!   - `macro_offsets` (U32)  -  start offsets of each macro name.
//!     Length `num_macros + 1`; the final entry is the total
//!     `macro_names_packed` length so each name's length is
//!     `offsets[i+1] - offsets[i]`.
//!
//! Outputs:
//!   - `directive_values` (U32)  -  per-token value: `1` / `0` for
//!     ifdef / ifndef; `0` for every other directive kind.
//!
//! ## Real-GPU lowering note
//!
//! Same conventions as the rest of the directive-classify family  -
//! byte extraction is inline. The packed entrypoint preserves the
//! standalone ABI; the raw-U8 entrypoint is used by the preprocessing
//! pipeline to avoid repacking retained source rows. The kernel is
//! **straight-line** (no loops, no outer-scope mutables) to dodge the
//! Q7 carrier-seed family bug in vyre-lower's region-scope phi-merge.
//!
//! The macro-table lookup is the only piece that previously relied
//! on source-specific program construction. It is now a runtime loop
//! over `buf_len("macro_offsets") - 1` macros, and each byte equality
//! check is bounded by the candidate macro-name length. One compiled
//! program handles every macro-table size and identifier length.

use super::gpu_source_bytes::{
    safe_load_source_layout_byte_expr, source_buffer_element, source_byte_len_expr,
    SourceByteLayout,
};
use crate::parsing::c::lex::tokens::{TOK_PP_IFDEF, TOK_PP_IFNDEF};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-libs::parsing::c::preprocess::gpu_ifdef_value";

/// Canonical binding index for the input per-token byte-offset buffer.
pub const BINDING_TOK_STARTS: u32 = 0;
/// Canonical binding index for the input per-token byte-length buffer.
pub const BINDING_TOK_LENS: u32 = 1;
/// Canonical binding index for the input directive-kinds buffer.
pub const BINDING_DIRECTIVE_KINDS: u32 = 2;
/// Canonical binding index for the input source-bytes buffer.
pub const BINDING_SOURCE: u32 = 3;
/// Canonical binding index for the input packed macro-name bytes buffer.
pub const BINDING_MACRO_NAMES_PACKED: u32 = 4;
/// Canonical binding index for the input macro-offsets buffer.
pub const BINDING_MACRO_OFFSETS: u32 = 5;
/// Canonical binding index for the output `directive_values` buffer.
pub const BINDING_DIRECTIVE_VALUES: u32 = 6;

/// Maximum horizontal-WS run before `#`, between `#` and the
/// keyword, between the keyword and the identifier. Cap at 4  -  real
/// rows have 0–1.
const MAX_WS_PREFIX: u32 = 4;

/// Build the ifdef/ifndef-evaluator `Program`.
///
/// `source_len`, `macro_names_len`, and `num_macros` must not specialize the
/// program shape. The source and macro buffers are runtime-sized, the kernel
/// reads their byte capacity through `Expr::buf_len(...)`, and the macro count
/// comes from `buf_len("macro_offsets") - 1`. One compiled program must serve
/// every translation unit and macro table size.
#[must_use]
pub fn gpu_ifdef_value(num_tokens: u32, source_len: u32) -> Program {
    gpu_ifdef_value_with_byte_layouts(
        num_tokens,
        source_len,
        SourceByteLayout::PackedU32,
        SourceByteLayout::PackedU32,
    )
}

/// Build the ifdef/ifndef evaluator over raw `DataType::U8` source and macro
/// name bytes.
///
/// This preserves the same binding order and runtime-sized source shape as the
/// packed ABI while letting pipeline callers pass retained byte strings without
/// host-side U32 repacks.
#[must_use]
pub fn gpu_ifdef_value_u8(num_tokens: u32, source_len: u32) -> Program {
    gpu_ifdef_value_with_byte_layouts(
        num_tokens,
        source_len,
        SourceByteLayout::RawU8,
        SourceByteLayout::RawU8,
    )
}

fn gpu_ifdef_value_with_byte_layouts(
    num_tokens: u32,
    source_len: u32,
    source_layout: SourceByteLayout,
    macro_names_layout: SourceByteLayout,
) -> Program {
    let _ = source_len;
    let t = Expr::var("t");
    let source_byte_len = source_byte_len_expr("source", source_layout);
    let macro_names_byte_len = source_byte_len_expr("macro_names_packed", macro_names_layout);

    let safe_load_source = |addr: Expr| -> Expr {
        safe_load_source_layout_byte_expr("source", source_layout, addr, source_byte_len.clone())
    };
    let safe_load_macro_name = |addr: Expr| -> Expr {
        safe_load_source_layout_byte_expr(
            "macro_names_packed",
            macro_names_layout,
            addr,
            macro_names_byte_len.clone(),
        )
    };
    let is_ws = |b: Expr| -> Expr {
        Expr::select(
            Expr::or(
                Expr::or(
                    Expr::eq(b.clone(), Expr::u32(b' ' as u32)),
                    Expr::eq(b.clone(), Expr::u32(b'\t' as u32)),
                ),
                Expr::or(
                    Expr::eq(b.clone(), Expr::u32(0x0B)),
                    Expr::eq(b, Expr::u32(0x0C)),
                ),
            ),
            Expr::u32(1),
            Expr::u32(0),
        )
    };
    let is_continue = |b: Expr| -> Expr {
        let is_lower = Expr::and(
            Expr::ge(b.clone(), Expr::u32(b'a' as u32)),
            Expr::le(b.clone(), Expr::u32(b'z' as u32)),
        );
        let is_upper = Expr::and(
            Expr::ge(b.clone(), Expr::u32(b'A' as u32)),
            Expr::le(b.clone(), Expr::u32(b'Z' as u32)),
        );
        let is_digit = Expr::and(
            Expr::ge(b.clone(), Expr::u32(b'0' as u32)),
            Expr::le(b.clone(), Expr::u32(b'9' as u32)),
        );
        let is_under = Expr::eq(b, Expr::u32(b'_' as u32));
        Expr::select(
            Expr::or(Expr::or(is_lower, is_upper), Expr::or(is_digit, is_under)),
            Expr::u32(1),
            Expr::u32(0),
        )
    };
    let is_start = |b: Expr| -> Expr {
        let is_lower = Expr::and(
            Expr::ge(b.clone(), Expr::u32(b'a' as u32)),
            Expr::le(b.clone(), Expr::u32(b'z' as u32)),
        );
        let is_upper = Expr::and(
            Expr::ge(b.clone(), Expr::u32(b'A' as u32)),
            Expr::le(b.clone(), Expr::u32(b'Z' as u32)),
        );
        let is_under = Expr::eq(b, Expr::u32(b'_' as u32));
        Expr::select(
            Expr::or(Expr::or(is_lower, is_upper), is_under),
            Expr::u32(1),
            Expr::u32(0),
        )
    };

    // hash_off: scan for `#` within first MAX_WS_PREFIX+1 bytes.
    let hash_off_expr = {
        let mut acc = Expr::u32(0xFFFF_FFFF);
        for p in (0..=MAX_WS_PREFIX).rev() {
            let mut prefix_ws = Expr::u32(1);
            for q in 0..p {
                prefix_ws = Expr::bitand(prefix_ws, Expr::var(format!("hs_ws_{q}")));
            }
            let s_eq_hash = Expr::select(
                Expr::eq(Expr::var(format!("hs_{p}")), Expr::u32(b'#' as u32)),
                Expr::u32(1),
                Expr::u32(0),
            );
            let cond_u32 = Expr::bitand(s_eq_hash, prefix_ws);
            acc = Expr::select(Expr::eq(cond_u32, Expr::u32(1)), Expr::u32(p), acc);
        }
        acc
    };

    let ws_skip_expr = |prefix: &str, n: u32| -> Expr {
        let mut acc = Expr::u32(n);
        for q in (0..n).rev() {
            let mut prefix_ws = Expr::u32(1);
            for r in 0..q {
                prefix_ws = Expr::bitand(prefix_ws, Expr::var(format!("{prefix}_ws_{r}")));
            }
            let xs_q_not_ws = Expr::select(
                Expr::eq(Expr::var(format!("{prefix}_ws_{q}")), Expr::u32(0)),
                Expr::u32(1),
                Expr::u32(0),
            );
            let cond_u32 = Expr::bitand(xs_q_not_ws, prefix_ws);
            acc = Expr::select(Expr::eq(cond_u32, Expr::u32(1)), Expr::u32(q), acc);
        }
        acc
    };

    let mut evaluate: Vec<Node> = Vec::new();
    evaluate.push(Node::let_bind(
        "tok_start",
        Expr::load("tok_starts", t.clone()),
    ));
    evaluate.push(Node::let_bind("tok_len", Expr::load("tok_lens", t.clone())));
    evaluate.push(Node::let_bind(
        "tok_end",
        Expr::add(Expr::var("tok_start"), Expr::var("tok_len")),
    ));

    // Step 1: leading WS + `#`.
    for p in 0..=MAX_WS_PREFIX {
        evaluate.push(Node::let_bind(
            format!("hs_{p}"),
            safe_load_source(Expr::add(Expr::var("tok_start"), Expr::u32(p))),
        ));
    }
    for p in 0..=MAX_WS_PREFIX {
        evaluate.push(Node::let_bind(
            format!("hs_ws_{p}"),
            is_ws(Expr::var(format!("hs_{p}"))),
        ));
    }
    evaluate.push(Node::let_bind("hash_off", hash_off_expr));
    evaluate.push(Node::let_bind(
        "hash_idx",
        Expr::add(Expr::var("tok_start"), Expr::var("hash_off")),
    ));
    evaluate.push(Node::let_bind(
        "found_hash",
        Expr::select(
            Expr::lt(Expr::var("hash_off"), Expr::u32(MAX_WS_PREFIX + 1)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));

    // Step 2: WS between `#` and the keyword.
    for q in 0..MAX_WS_PREFIX {
        evaluate.push(Node::let_bind(
            format!("kp_{q}"),
            safe_load_source(Expr::add(Expr::var("hash_idx"), Expr::u32(q + 1))),
        ));
    }
    for q in 0..MAX_WS_PREFIX {
        evaluate.push(Node::let_bind(
            format!("kp_ws_{q}"),
            is_ws(Expr::var(format!("kp_{q}"))),
        ));
    }
    evaluate.push(Node::let_bind("kw_skip", ws_skip_expr("kp", MAX_WS_PREFIX)));
    evaluate.push(Node::let_bind(
        "kw_start",
        Expr::add(
            Expr::add(Expr::var("hash_idx"), Expr::u32(1)),
            Expr::var("kw_skip"),
        ),
    ));

    // Step 3: keyword length depends on kind (`ifdef`=5, `ifndef`=6).
    evaluate.push(Node::let_bind(
        "kw_len_skip",
        Expr::select(
            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_IFNDEF)),
            Expr::u32(6),
            Expr::u32(5),
        ),
    ));
    evaluate.push(Node::let_bind(
        "post_kw",
        Expr::add(Expr::var("kw_start"), Expr::var("kw_len_skip")),
    ));

    // Step 4: WS between keyword and identifier.
    for q in 0..MAX_WS_PREFIX {
        evaluate.push(Node::let_bind(
            format!("ip_{q}"),
            safe_load_source(Expr::add(Expr::var("post_kw"), Expr::u32(q))),
        ));
    }
    for q in 0..MAX_WS_PREFIX {
        evaluate.push(Node::let_bind(
            format!("ip_ws_{q}"),
            is_ws(Expr::var(format!("ip_{q}"))),
        ));
    }
    evaluate.push(Node::let_bind(
        "ident_skip",
        ws_skip_expr("ip", MAX_WS_PREFIX),
    ));
    evaluate.push(Node::let_bind(
        "ident_start_val",
        Expr::add(Expr::var("post_kw"), Expr::var("ident_skip")),
    ));

    // Step 5: scan identifier bytes to the directive row end. The
    // first byte must be a valid C identifier start; subsequent bytes
    // may be identifier continuations.
    evaluate.push(Node::let_bind(
        "ident_scan_limit",
        Expr::select(
            Expr::lt(Expr::var("ident_start_val"), Expr::var("tok_end")),
            Expr::sub(Expr::var("tok_end"), Expr::var("ident_start_val")),
            Expr::u32(0),
        ),
    ));
    evaluate.push(Node::let_bind("ident_len_val", Expr::u32(0)));
    evaluate.push(Node::let_bind("ident_done", Expr::u32(0)));
    evaluate.push(Node::loop_for(
        "ident_i",
        Expr::u32(0),
        Expr::var("ident_scan_limit"),
        vec![Node::if_then(
            Expr::eq(Expr::var("ident_done"), Expr::u32(0)),
            vec![
                Node::let_bind(
                    "ident_byte",
                    safe_load_source(Expr::add(
                        Expr::var("ident_start_val"),
                        Expr::var("ident_i"),
                    )),
                ),
                Node::let_bind(
                    "ident_byte_ok",
                    Expr::select(
                        Expr::eq(Expr::var("ident_i"), Expr::u32(0)),
                        is_start(Expr::var("ident_byte")),
                        is_continue(Expr::var("ident_byte")),
                    ),
                ),
                Node::if_then_else(
                    Expr::eq(Expr::var("ident_byte_ok"), Expr::u32(1)),
                    vec![Node::assign(
                        "ident_len_val",
                        Expr::add(Expr::var("ident_i"), Expr::u32(1)),
                    )],
                    vec![Node::assign("ident_done", Expr::u32(1))],
                ),
            ],
        )],
    ));

    // Step 6: per-macro byte equality.
    //
    // Outer loop over m in [0, num_macros) is a RUNTIME kernel loop:
    // num_macros is read from `Expr::buf_len("macro_offsets") - 1` so
    // the kernel program structure is independent of how many macros
    // the host supplies. Candidate name comparison is also a runtime
    // loop over `m_len`, guarded by the equal-length check.
    //
    // `macro_names_len` (the byte length of the names buffer) is also runtime.
    // The packed ABI maps words to byte capacity; the raw-U8 variant uses the
    // actual bound byte-for-byte. Both are >= the final macro offset.
    let macro_count_runtime = Expr::sub(Expr::buf_len("macro_offsets"), Expr::u32(1));
    evaluate.push(Node::let_bind("def_found", Expr::u32(0)));
    let compare_macro_body: Vec<Node> = vec![
        Node::let_bind(
            "m_start",
            Expr::cast(DataType::U32, Expr::load("macro_offsets", Expr::var("m"))),
        ),
        Node::let_bind(
            "m_end",
            Expr::cast(
                DataType::U32,
                Expr::load("macro_offsets", Expr::add(Expr::var("m"), Expr::u32(1))),
            ),
        ),
        Node::let_bind("m_len", Expr::sub(Expr::var("m_end"), Expr::var("m_start"))),
        Node::let_bind(
            "all_match",
            Expr::select(
                Expr::and(
                    Expr::ne(Expr::var("ident_len_val"), Expr::u32(0)),
                    Expr::eq(Expr::var("m_len"), Expr::var("ident_len_val")),
                ),
                Expr::u32(1),
                Expr::u32(0),
            ),
        ),
        Node::loop_for(
            "name_k",
            Expr::u32(0),
            Expr::var("m_len"),
            vec![Node::if_then(
                Expr::eq(Expr::var("all_match"), Expr::u32(1)),
                vec![
                    Node::let_bind(
                        "ident_cmp_byte",
                        safe_load_source(Expr::add(
                            Expr::var("ident_start_val"),
                            Expr::var("name_k"),
                        )),
                    ),
                    Node::let_bind(
                        "macro_cmp_byte",
                        safe_load_macro_name(Expr::add(Expr::var("m_start"), Expr::var("name_k"))),
                    ),
                    Node::if_then(
                        Expr::ne(Expr::var("ident_cmp_byte"), Expr::var("macro_cmp_byte")),
                        vec![Node::assign("all_match", Expr::u32(0))],
                    ),
                ],
            )],
        ),
        Node::if_then(
            Expr::eq(Expr::var("all_match"), Expr::u32(1)),
            vec![Node::assign("def_found", Expr::u32(1))],
        ),
    ];
    evaluate.push(Node::loop_for(
        "m",
        Expr::u32(0),
        macro_count_runtime,
        vec![Node::if_then(
            Expr::eq(Expr::var("def_found"), Expr::u32(0)),
            compare_macro_body,
        )],
    ));

    // For #ifndef invert; for #ifdef as-is.
    evaluate.push(Node::let_bind(
        "value_out_val",
        Expr::select(
            Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_IFNDEF)),
            Expr::select(
                Expr::eq(Expr::var("def_found"), Expr::u32(1)),
                Expr::u32(0),
                Expr::u32(1),
            ),
            Expr::var("def_found"),
        ),
    ));

    // Commit only when we actually found `#` in the leading run.
    evaluate.push(Node::if_then(
        Expr::eq(Expr::var("found_hash"), Expr::u32(1)),
        vec![Node::store(
            "directive_values",
            t.clone(),
            Expr::var("value_out_val"),
        )],
    ));

    // Note: this kernel deliberately does NOT pre-zero
    // `directive_values` for non-ifdef/ifndef rows. The host
    // initializes the buffer to zero before dispatch, and the
    // sibling `gpu_if_expression` kernel only writes to if/elif
    // rows. With both kernels touching only their own kind's rows,
    // the two can be safely fused into a single dispatch (the fuser
    // inserts a barrier on the shared `directive_values` write
    // buffer, but pre-zero would clobber the other arm's writes).
    let body: Vec<Node> = vec![
        Node::let_bind("t", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(t.clone(), Expr::u32(num_tokens)),
            vec![
                Node::let_bind("kind", Expr::load("directive_kinds", t.clone())),
                Node::if_then(
                    Expr::or(
                        Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_IFDEF)),
                        Expr::eq(Expr::var("kind"), Expr::u32(TOK_PP_IFNDEF)),
                    ),
                    evaluate,
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(
                "tok_starts",
                BINDING_TOK_STARTS,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(num_tokens.max(1)),
            BufferDecl::storage(
                "tok_lens",
                BINDING_TOK_LENS,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(num_tokens.max(1)),
            BufferDecl::storage(
                "directive_kinds",
                BINDING_DIRECTIVE_KINDS,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(num_tokens.max(1)),
            BufferDecl::storage(
                "source",
                BINDING_SOURCE,
                BufferAccess::ReadOnly,
                source_buffer_element(source_layout),
            )
            .with_count(0),
            // Runtime-sized: count=0 marks the buffer as runtime-bound,
            // so `Expr::buf_len` returns the host-supplied element count
            // and the program's structure stays independent of how many
            // macros the host packs.
            BufferDecl::storage(
                "macro_names_packed",
                BINDING_MACRO_NAMES_PACKED,
                BufferAccess::ReadOnly,
                source_buffer_element(macro_names_layout),
            )
            .with_count(0),
            BufferDecl::storage(
                "macro_offsets",
                BINDING_MACRO_OFFSETS,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(0),
            BufferDecl::storage(
                "directive_values",
                BINDING_DIRECTIVE_VALUES,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(num_tokens.max(1)),
        ],
        [256, 1, 1],
        body,
    )
    .with_entry_op_id(OP_ID)
}

#[cfg(test)]

mod tests {
    use super::*;
    use vyre::ir::DataType;

    #[test]
    fn op_id_is_canonical_and_stable() {
        assert_eq!(OP_ID, "vyre-libs::parsing::c::preprocess::gpu_ifdef_value");
    }

    #[test]
    fn binding_indices_are_canonical_and_stable() {
        assert_eq!(BINDING_TOK_STARTS, 0);
        assert_eq!(BINDING_TOK_LENS, 1);
        assert_eq!(BINDING_DIRECTIVE_KINDS, 2);
        assert_eq!(BINDING_SOURCE, 3);
        assert_eq!(BINDING_MACRO_NAMES_PACKED, 4);
        assert_eq!(BINDING_MACRO_OFFSETS, 5);
        assert_eq!(BINDING_DIRECTIVE_VALUES, 6);
    }

    #[test]
    fn build_program_returns_well_formed_program() {
        let p = gpu_ifdef_value(8, 64);
        assert_eq!(p.buffers().len(), 7);
        assert_eq!(p.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn source_buffer_is_runtime_sized_not_source_length_specialized() {
        let p = gpu_ifdef_value(8, 64);
        let source = p
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: source buffer must exist");
        assert_eq!(
            source.count, 0,
            "source must be runtime-sized so one ifdef evaluator program serves all source lengths"
        );
    }

    #[test]
    fn source_buffer_layouts_preserve_packed_abi_and_raw_u8_variant() {
        let packed = gpu_ifdef_value(8, 64);
        let raw_u8 = gpu_ifdef_value_u8(8, 64);
        for name in ["source", "macro_names_packed"] {
            let packed_buffer = packed
                .buffers()
                .iter()
                .find(|buffer| buffer.name() == name)
                .unwrap_or_else(|| panic!("Fix: packed ifdef evaluator {name} buffer must exist"));
            let raw_u8_buffer = raw_u8
                .buffers()
                .iter()
                .find(|buffer| buffer.name() == name)
                .unwrap_or_else(|| panic!("Fix: raw-U8 ifdef evaluator {name} buffer must exist"));

            assert_eq!(packed_buffer.element(), DataType::U32);
            assert_eq!(packed_buffer.count(), 0);
            assert_eq!(raw_u8_buffer.element(), DataType::U8);
            assert_eq!(raw_u8_buffer.count(), 0);
        }
    }
}

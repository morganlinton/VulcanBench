use crate::parsing::c::parse::gnu_builtins::gpu_builtin_hash_table_words;
use crate::parsing::c::preprocess::gpu_define_parse::gpu_define_parse_u8;
use crate::parsing::c::preprocess::gpu_if_expression::gpu_if_expression_u8;
use crate::parsing::c::preprocess::gpu_ifdef_value::gpu_ifdef_value_u8;
use crate::parsing::c::preprocess::gpu_include_parse::gpu_include_parse_u8;
use crate::parsing::c::preprocess::gpu_undef_parse::gpu_undef_parse_u8;
use vyre::execution_plan::fusion::fuse_programs;

use super::buffers::{bucket_pow2, pack_u32_words_into, unpack_u32_words_exact_into};
use super::tokenization::reject_invalid_if_expression_values;
use super::{ClassifiedTokens, GpuDispatcher};

/// Parsed payload for one directive row.
///
/// Indexed by token position in the source-order token stream. Rows
/// whose `directive_kinds[i] == 0` (not a directive) get
/// [`DirectivePayload::None`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectivePayload {
    /// Not a directive row.
    None,
    /// `#define name [(args)] body`.
    Define {
        /// Macro name bytes.
        name: Vec<u8>,
        /// Macro name byte offset in the filtered source.
        name_start: u32,
        /// Macro name byte length in the filtered source.
        name_len: u32,
        /// Comma-separated args (function-like macros only). Empty for object-like.
        args: Vec<u8>,
        /// Argument-list byte offset in the filtered source.
        args_start: u32,
        /// Argument-list byte length in the filtered source.
        args_len: u32,
        /// Replacement body bytes (trailing whitespace already trimmed).
        body: Vec<u8>,
        /// Replacement body byte offset in the filtered source.
        body_start: u32,
        /// Replacement body byte length in the filtered source.
        body_len: u32,
        /// `true` if the directive used the `name(args)` form.
        is_function_like: bool,
    },
    /// `#undef name`. Currently classified by directive kind only  -
    /// the name extraction reuses the define-parse name span shape
    /// (treats `#undef` as `#define`-shaped for the parse).
    Undef {
        /// Macro name to undefine.
        name: Vec<u8>,
    },
    /// `#include <…>` or `#include "…"` (and `#include_next`).
    Include {
        /// The path bytes between the delimiters.
        path: Vec<u8>,
        /// `true` for `<…>`, `false` for `"…"`.
        is_system: bool,
        /// `true` only for `#include_next`.
        is_next: bool,
    },
    /// `#ifdef name` / `#ifndef name`. The kernel's value column gives
    /// directive truth: definedness for `#ifdef`, complement for `#ifndef`.
    Ifdef {
        /// `1` if the conditional branch is active, else `0`.
        value: u32,
        /// `true` if the directive was `#ifndef` (value semantics inverted).
        negated: bool,
    },
    /// `#if expr` / `#elif expr`. Compatibility extractors may store a
    /// snapshot value here; the production driver re-evaluates reachable rows
    /// against the live GPU macro table before branch selection.
    IfExpr {
        /// The truth value of the expression (1 or 0).
        value: u32,
        /// `true` for `#elif`, `false` for `#if`.
        is_elif: bool,
    },
    /// `#else`. No payload  -  caller flips the conditional frame.
    Else,
    /// `#endif`. No payload  -  caller pops the conditional frame.
    Endif,
    /// `#pragma`, `#line`, `#error`, `#warning`, `#ident`, `#sccs`,
    /// `#null` (empty `#`). Carried by kind only; payload is opaque.
    Other,
}

#[derive(Default)]
pub(super) struct DirectiveExtractionScratch {
    starts_b: Vec<u8>,
    lens_b: Vec<u8>,
    kinds_b: Vec<u8>,
    zero_init: Vec<u8>,
    macro_names: Vec<u8>,
    macro_offsets_b: Vec<u8>,
    macro_values_b: Vec<u8>,
    parse_out: Vec<Vec<u8>>,
    condition_out: Vec<Vec<u8>>,
    name_s: Vec<u32>,
    name_l: Vec<u32>,
    args_s: Vec<u32>,
    args_l: Vec<u32>,
    body_s: Vec<u32>,
    body_l: Vec<u32>,
    is_func: Vec<u32>,
    path_s: Vec<u32>,
    path_l: Vec<u32>,
    is_system: Vec<u32>,
    undef_name_s: Vec<u32>,
    undef_name_l: Vec<u32>,
    ifdef_values: Vec<u32>,
    if_values: Vec<u32>,
}

impl DirectiveExtractionScratch {
    fn prepare_zero_init(&mut self, byte_len: usize) -> Result<(), String> {
        self.zero_init.clear();
        self.zero_init.try_reserve_exact(byte_len).map_err(|error| {
            format!(
                "gpu directive parse zero-init staging could not reserve {byte_len} bytes: {error:?}. Fix: shard preprocessing before directive payload extraction."
            )
        })?;
        self.zero_init.resize(byte_len, 0);
        Ok(())
    }
}

fn directive_word_bytes(word_count: usize, label: &'static str) -> Result<usize, String> {
    word_count.checked_mul(4).ok_or_else(|| {
        format!(
            "gpu directive parse {label} word count {word_count} overflows host byte sizing. Fix: shard preprocessing before directive payload extraction."
        )
    })
}

fn reserve_directive_vec<T>(
    out: &mut Vec<T>,
    additional: usize,
    label: &'static str,
) -> Result<(), String> {
    out.try_reserve_exact(additional).map_err(|error| {
        format!(
            "gpu directive parse could not reserve {additional} {label}: {error:?}. Fix: shard preprocessing before directive payload extraction."
        )
    })
}

/// Extract per-directive payloads for every directive row in
/// `classified`, against the supplied `defined_macros` snapshot.
///
/// Dispatches each per-directive kernel once over the FULL token
/// stream (every kernel is per-thread parallel internally). Then host
/// walks the directive_kinds column to assemble payloads.
///
/// **Note on macro accuracy:** this compatibility API accepts only
/// defined macro names. `#ifdef` / `#ifndef` snapshots are exact for that
/// name set, while `#if` bare identifiers are definedness snapshots rather
/// than full object-like macro integer expansions. The production
/// preprocessor driver does not use these snapshots for branch selection; it
/// re-evaluates reachable conditionals against the live GPU macro table.
///
/// # Errors
/// Returns the dispatcher error verbatim if any stage fails.
pub fn gpu_extract_directive_payloads(
    dispatcher: &dyn GpuDispatcher,
    classified: &ClassifiedTokens,
    defined_macros: &[&[u8]],
) -> Result<Vec<DirectivePayload>, String> {
    let mut scratch = DirectiveExtractionScratch::default();
    gpu_extract_directive_payloads_impl(dispatcher, classified, defined_macros, true, &mut scratch)
}

pub(super) fn gpu_extract_directive_payloads_for_driver_with_scratch(
    dispatcher: &dyn GpuDispatcher,
    classified: &ClassifiedTokens,
    scratch: &mut DirectiveExtractionScratch,
) -> Result<Vec<DirectivePayload>, String> {
    gpu_extract_directive_payloads_impl(dispatcher, classified, &[], false, scratch)
}

fn gpu_extract_directive_payloads_impl(
    dispatcher: &dyn GpuDispatcher,
    classified: &ClassifiedTokens,
    defined_macros: &[&[u8]],
    evaluate_condition_values: bool,
    scratch: &mut DirectiveExtractionScratch,
) -> Result<Vec<DirectivePayload>, String> {
    use crate::parsing::c::lex::tokens::{
        TOK_PP_DEFINE, TOK_PP_ELIF, TOK_PP_ELSE, TOK_PP_ENDIF, TOK_PP_IF, TOK_PP_IFDEF,
        TOK_PP_IFNDEF, TOK_PP_INCLUDE, TOK_PP_INCLUDE_NEXT, TOK_PP_UNDEF,
    };
    let n = classified.tok_types.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    // Fast path: if there are no directive rows at all (no `#define`,
    // `#include`, `#ifdef`, `#if`, etc.), every payload is `None`.
    // Skip the parse-kernel dispatches entirely. For fixtures that
    // arrive at this stage already preprocessor-clean (the common case
    // after the host has expanded includes / inlined defines upstream),
    // this avoids 3 cold native-compiles of the directive parse
    // kernels  -  each of which is ~MB-scale WGSL and ~20s cold-compile
    // on the wgpu Vulkan path. The kernels still dispatch when
    // directives are present.
    if !classified.has_directives() {
        if !evaluate_condition_values {
            return Ok(Vec::new());
        }
        return Ok(vec![DirectivePayload::None; n]);
    }
    // Bucket token-count output shapes only. Directive payload and compatibility
    // condition evaluators consume the retained raw-U8 source bytes directly, so
    // source length no longer specializes shader programs or requires host-side
    // U32 repacking.
    let n_bucket = bucket_pow2(n.max(1), 64);
    let n_pad = n_bucket;
    let source_len = u32::try_from(classified.source.len()).map_err(|_| {
        format!(
            "gpu directive parse source length {} exceeds u32 address space. Fix: shard preprocessing before directive payload extraction.",
            classified.source.len()
        )
    })?;
    pack_u32_words_into(&mut scratch.starts_b, &classified.tok_starts, n_pad)?;
    pack_u32_words_into(&mut scratch.lens_b, &classified.tok_lens, n_pad)?;
    pack_u32_words_into(&mut scratch.kinds_b, &classified.directive_kinds, n_pad)?;

    // ---- Dispatch 1: define + include + undef parsers fused ----
    // All three kernels share the (tok_starts, tok_lens,
    // directive_kinds, source) inputs and have NO overlapping output
    // buffer names (undef_parse's outputs were renamed to
    // `undef_name_*` so they don't collide with define_parse's
    // `name_*`). Fusing reduces 3 separate dispatches to 1, cutting
    // host-side Vec<u8> round-trips substantially. Buffer order in
    // the fused program is set by `fuse_programs` iteration:
    //   shared:     tok_starts, tok_lens, directive_kinds, source
    //   define out: name_start, name_len, args_start, args_len,
    //               body_start, body_len, is_function_like
    //   include out: path_start, path_len, is_system
    //   undef out:  undef_name_start, undef_name_len
    let dp = gpu_define_parse_u8(n_bucket as u32, source_len);
    let ip = gpu_include_parse_u8(n_bucket as u32, source_len);
    let up = gpu_undef_parse_u8(n_bucket as u32, source_len);
    let parse_fused = fuse_programs(&[dp, ip, up])
        .map_err(|e| format!("fuse define+include+undef parse: {e}"))?
        .with_entry_op_id("vyre-libs::parsing::c::preprocess::define_include_undef_parse_fused");
    let zero_init_bytes = directive_word_bytes(n_pad, "zero-init")?;
    scratch.prepare_zero_init(zero_init_bytes)?;
    let parse_inputs = [
        scratch.starts_b.as_slice(),
        scratch.lens_b.as_slice(),
        scratch.kinds_b.as_slice(),
        classified.source.as_ref(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
        scratch.zero_init.as_slice(),
    ];
    dispatcher
        .dispatch_borrowed_into(&parse_fused, &parse_inputs, &mut scratch.parse_out)
        .map_err(|e| format!("gpu_define+include+undef_parse fused: {e}"))?;
    if scratch.parse_out.len() != 12 {
        return Err(format!(
            "gpu_define+include+undef_parse fused: expected exactly 12 outputs, got {}. Fix: backend must return the declared directive parse tables and no extras.",
            scratch.parse_out.len()
        ));
    }
    unpack_u32_words_exact_into(
        &scratch.parse_out[0],
        n_pad,
        "define name_start",
        &mut scratch.name_s,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[1],
        n_pad,
        "define name_len",
        &mut scratch.name_l,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[2],
        n_pad,
        "define args_start",
        &mut scratch.args_s,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[3],
        n_pad,
        "define args_len",
        &mut scratch.args_l,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[4],
        n_pad,
        "define body_start",
        &mut scratch.body_s,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[5],
        n_pad,
        "define body_len",
        &mut scratch.body_l,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[6],
        n_pad,
        "define is_function_like",
        &mut scratch.is_func,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[7],
        n_pad,
        "include path_start",
        &mut scratch.path_s,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[8],
        n_pad,
        "include path_len",
        &mut scratch.path_l,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[9],
        n_pad,
        "include is_system",
        &mut scratch.is_system,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[10],
        n_pad,
        "undef name_start",
        &mut scratch.undef_name_s,
    )?;
    unpack_u32_words_exact_into(
        &scratch.parse_out[11],
        n_pad,
        "undef name_len",
        &mut scratch.undef_name_l,
    )?;

    // ---- Dispatch 2: gpu_ifdef_value ----
    // Both gpu_ifdef_value and gpu_if_expression are kind-gated to
    // disjoint rows of `directive_values`, so they can be fused safely
    // (verified under reference-eval). Real-GPU cold-pipeline evidence
    // showed the merged shader compiles substantially slower, so the
    // release path keeps them as separate dispatches until compile-cache
    // telemetry proves fusion wins end-to-end.
    // gpu_ifdef_value_u8 and gpu_if_expression_u8 now read num_macros and
    // macro-name byte capacity at runtime via Expr::buf_len, so the kernel
    // program shape is independent of how many macros the host supplies.
    // No more bucketing needed for these dimensions  -  one cold compile
    // per process for the ifdef/if-expression pair.
    if evaluate_condition_values {
        // Build defined_macros into raw name bytes plus offsets only when the
        // caller needs snapshot condition values. The production driver skips
        // this branch and re-evaluates reachable conditionals against the live
        // macro table instead.
        let macro_name_bytes =
            defined_macros
                .iter()
                .try_fold(0usize, |total, name| {
                    total.checked_add(name.len()).ok_or_else(|| {
                        "gpu directive parse macro-name byte total overflows usize. Fix: shard preprocessing before directive payload extraction.".to_string()
                    })
                })?;
        scratch.macro_names.clear();
        reserve_directive_vec(
            &mut scratch.macro_names,
            macro_name_bytes,
            "macro-name bytes",
        )?;
        let macro_offset_slots = defined_macros.len().checked_add(1).ok_or_else(|| {
            "gpu directive parse macro-offset slot count overflows usize. Fix: shard preprocessing before directive payload extraction.".to_string()
        })?;
        let mut macro_offsets: Vec<u32> = Vec::new();
        reserve_directive_vec(&mut macro_offsets, macro_offset_slots, "macro-offset slots")?;
        macro_offsets.push(0);
        for name in defined_macros {
            scratch.macro_names.extend_from_slice(name);
            macro_offsets.push(u32::try_from(scratch.macro_names.len()).map_err(|_| {
                format!(
                    "gpu directive parse macro-name byte offset {} exceeds u32 address space. Fix: shard preprocessing before directive payload extraction.",
                    scratch.macro_names.len()
                )
            })?);
        }
        if scratch.macro_names.is_empty() {
            reserve_directive_vec(&mut scratch.macro_names, 1, "macro-name sentinel byte")?;
            scratch.macro_names.push(0);
        }
        pack_u32_words_into(
            &mut scratch.macro_offsets_b,
            &macro_offsets,
            macro_offsets.len(),
        )?;
        let count = defined_macros.len().max(1);
        scratch.macro_values_b.clear();
        let builtin_hashes = gpu_builtin_hash_table_words();
        let macro_value_words = count.checked_add(builtin_hashes.len()).ok_or_else(|| {
            "gpu directive parse macro-value word count overflows usize. Fix: shard preprocessing before directive payload extraction.".to_string()
        })?;
        let macro_value_bytes = directive_word_bytes(macro_value_words, "macro values")?;
        reserve_directive_vec(
            &mut scratch.macro_values_b,
            macro_value_bytes,
            "macro-value bytes",
        )?;
        vyre_primitives::wire::append_u32_slice_le_bytes(
            &builtin_hashes,
            &mut scratch.macro_values_b,
        );
        for idx in 0..count {
            let value = u32::from(idx < defined_macros.len());
            scratch
                .macro_values_b
                .extend_from_slice(&value.to_le_bytes());
        }

        let iv = gpu_ifdef_value_u8(n_bucket as u32, source_len);
        let iv_inputs = [
            scratch.starts_b.as_slice(),
            scratch.lens_b.as_slice(),
            scratch.kinds_b.as_slice(),
            classified.source.as_ref(),
            scratch.macro_names.as_slice(),
            scratch.macro_offsets_b.as_slice(),
            scratch.zero_init.as_slice(),
        ];
        dispatcher
            .dispatch_borrowed_into(&iv, &iv_inputs, &mut scratch.condition_out)
            .map_err(|e| format!("gpu_ifdef_value: {e}"))?;
        if scratch.condition_out.len() != 1 {
            return Err(format!(
                "gpu_ifdef_value: expected exactly 1 output, got {}. Fix: backend must return only the ifdef values table.",
                scratch.condition_out.len()
            ));
        }
        unpack_u32_words_exact_into(
            &scratch.condition_out[0],
            n_pad,
            "ifdef values",
            &mut scratch.ifdef_values,
        )?;

        // ---- Dispatch 3: gpu_if_expression ----
        let ie = gpu_if_expression_u8(n_bucket as u32, source_len);
        let ie_inputs = [
            scratch.starts_b.as_slice(),
            scratch.lens_b.as_slice(),
            scratch.kinds_b.as_slice(),
            classified.source.as_ref(),
            scratch.macro_names.as_slice(),
            scratch.macro_offsets_b.as_slice(),
            scratch.macro_values_b.as_slice(),
            scratch.zero_init.as_slice(),
        ];
        dispatcher
            .dispatch_borrowed_into(&ie, &ie_inputs, &mut scratch.condition_out)
            .map_err(|e| format!("gpu_if_expression: {e}"))?;
        if scratch.condition_out.len() != 1 {
            return Err(format!(
                "gpu_if_expression: expected exactly 1 output, got {}. Fix: backend must return only the #if expression values table.",
                scratch.condition_out.len()
            ));
        }
        unpack_u32_words_exact_into(
            &scratch.condition_out[0],
            n_pad,
            "if expression values",
            &mut scratch.if_values,
        )?;
        reject_invalid_if_expression_values(&scratch.if_values, classified)?;
    } else {
        scratch.ifdef_values.clear();
        scratch.ifdef_values.resize(n, 0);
        scratch.if_values.clear();
        scratch.if_values.resize(n, 0);
    };

    // (define + include + undef fused above into one dispatch.
    // Total: 5 dispatches → 3.)

    // ---- Walk and assemble payloads ----
    let mut out = Vec::new();
    reserve_directive_vec(&mut out, n, "directive payload slots")?;
    for i in 0..n {
        let kind = classified.directive_kinds[i];
        let payload = match kind {
            0 => DirectivePayload::None,
            k if k == TOK_PP_DEFINE => {
                let nb = scratch.name_s[i] as usize;
                let nl = scratch.name_l[i] as usize;
                let ab = scratch.args_s[i] as usize;
                let al = scratch.args_l[i] as usize;
                let bb = scratch.body_s[i] as usize;
                let bl = scratch.body_l[i] as usize;
                let name = payload_span_bytes(&classified.source, nb, nl, i, "define name")?;
                let args = if al == 0 {
                    Vec::new()
                } else {
                    payload_span_bytes(&classified.source, ab, al, i, "define args")?
                };
                let body = if bl == 0 {
                    Vec::new()
                } else {
                    payload_span_bytes(&classified.source, bb, bl, i, "define body")?
                };
                DirectivePayload::Define {
                    name,
                    name_start: scratch.name_s[i],
                    name_len: scratch.name_l[i],
                    args,
                    args_start: scratch.args_s[i],
                    args_len: scratch.args_l[i],
                    body,
                    body_start: scratch.body_s[i],
                    body_len: scratch.body_l[i],
                    is_function_like: scratch.is_func[i] == 1,
                }
            }
            k if k == TOK_PP_UNDEF => {
                // Dedicated `gpu_undef_parse` kernel (5-byte keyword
                // step, single ident scan). Replaces the prior workaround
                // that routed `#undef` through `gpu_define_parse` which
                // bakes in a 6-byte `#define` keyword length.
                let nb = scratch.undef_name_s[i] as usize;
                let nl = scratch.undef_name_l[i] as usize;
                if nl == 0 {
                    DirectivePayload::Undef { name: Vec::new() }
                } else {
                    DirectivePayload::Undef {
                        name: payload_span_bytes(&classified.source, nb, nl, i, "undef name")?,
                    }
                }
            }
            k if k == TOK_PP_INCLUDE || k == TOK_PP_INCLUDE_NEXT => {
                let pb = scratch.path_s[i] as usize;
                let pl = scratch.path_l[i] as usize;
                if pl == 0 {
                    DirectivePayload::Other
                } else {
                    DirectivePayload::Include {
                        path: payload_span_bytes(&classified.source, pb, pl, i, "include path")?,
                        is_system: scratch.is_system[i] == 1,
                        is_next: k == TOK_PP_INCLUDE_NEXT,
                    }
                }
            }
            k if k == TOK_PP_IFDEF => DirectivePayload::Ifdef {
                value: scratch.ifdef_values[i],
                negated: false,
            },
            k if k == TOK_PP_IFNDEF => DirectivePayload::Ifdef {
                value: scratch.ifdef_values[i],
                negated: true,
            },
            k if k == TOK_PP_IF => DirectivePayload::IfExpr {
                value: scratch.if_values[i],
                is_elif: false,
            },
            k if k == TOK_PP_ELIF => DirectivePayload::IfExpr {
                value: scratch.if_values[i],
                is_elif: true,
            },
            k if k == TOK_PP_ELSE => DirectivePayload::Else,
            k if k == TOK_PP_ENDIF => DirectivePayload::Endif,
            _ => DirectivePayload::Other,
        };
        out.push(payload);
    }
    Ok(out)
}

fn payload_span_bytes(
    source: &[u8],
    start: usize,
    len: usize,
    token_index: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    let end = start.checked_add(len).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: {label} span at token {token_index} overflows usize. Fix: repair GPU directive payload span emission."
        )
    })?;
    source
        .get(start..end)
        .map(|bytes| bytes.to_vec())
        .ok_or_else(|| {
            format!(
                "vyre-libs::gpu_pipeline: {label} span {start}..{end} at token {token_index} is outside source length {}. Fix: repair GPU directive payload span emission.",
                source.len()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use vyre::ir::Program;

    struct NoDispatch;

    impl GpuDispatcher for NoDispatch {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
        ) -> Result<Vec<Vec<u8>>, String> {
            Err("directive-free fixture must not dispatch".to_string())
        }
    }

    fn directive_free_classified() -> ClassifiedTokens {
        ClassifiedTokens {
            tok_types: vec![1, 1, 1],
            tok_starts: vec![0, 4, 5],
            tok_lens: vec![3, 1, 1],
            directive_kinds: vec![0, 0, 0],
            directive_count: 0,
            source: Arc::from(b"int x".as_slice()),
        }
    }

    #[test]
    fn driver_payload_extraction_uses_empty_slice_for_directive_free_inputs() {
        let classified = directive_free_classified();
        let mut scratch = DirectiveExtractionScratch::default();
        let payloads = gpu_extract_directive_payloads_for_driver_with_scratch(
            &NoDispatch,
            &classified,
            &mut scratch,
        )
        .expect("Fix: directive-free production extraction must not dispatch");

        assert!(
            payloads.is_empty(),
            "production driver should use empty payload slices for directive-free inputs"
        );
    }

    #[test]
    fn compatibility_payload_extraction_preserves_per_token_none_contract() {
        let classified = directive_free_classified();
        let payloads = gpu_extract_directive_payloads(&NoDispatch, &classified, &[])
            .expect("Fix: compatibility extraction must not dispatch on directive-free inputs");

        assert_eq!(
            payloads,
            vec![
                DirectivePayload::None,
                DirectivePayload::None,
                DirectivePayload::None
            ]
        );
    }

    #[test]
    fn directive_staging_sizing_is_checked_and_fallible() {
        assert_eq!(
            directive_word_bytes(3, "test").expect("Fix: small directive table should fit"),
            12
        );
        assert!(
            directive_word_bytes(usize::MAX, "test").is_err(),
            "Fix: directive word-to-byte sizing must reject usize overflow"
        );

        let mut scratch = DirectiveExtractionScratch::default();
        scratch
            .prepare_zero_init(8)
            .expect("Fix: small directive zero staging should fit");
        assert_eq!(scratch.zero_init, vec![0; 8]);
    }
}

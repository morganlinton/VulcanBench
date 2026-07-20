use std::sync::{Mutex, OnceLock};

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::parsing::c::preprocess::gpu_if_expression::gpu_if_expression_u8;
use crate::parsing::c::preprocess::gpu_if_expression_abi::INVALID_EXPR_VALUE;
use crate::parsing::c::preprocess::gpu_ifdef_value::gpu_ifdef_value_u8;

use super::buffers::{
    bucket_pow2, checked_gpu_u32, pack_u32_words, pack_u32_words_into, read_u32_scalar_exact,
    unpack_u32_words_exact_into,
};
use super::live_conditional_cache::{LiveConditionalCache, LiveConditionalCacheKey};
use super::macro_values;
use super::{GpuDispatcher, MacroDef};

fn live_ifdef_program() -> std::sync::Arc<vyre::ir::Program> {
    static CACHE: OnceLock<std::sync::Arc<vyre::ir::Program>> = OnceLock::new();
    std::sync::Arc::clone(CACHE.get_or_init(|| std::sync::Arc::new(gpu_ifdef_value_u8(1, 0))))
}

fn live_if_expression_program() -> std::sync::Arc<vyre::ir::Program> {
    static CACHE: OnceLock<std::sync::Arc<vyre::ir::Program>> = OnceLock::new();
    std::sync::Arc::clone(CACHE.get_or_init(|| std::sync::Arc::new(gpu_if_expression_u8(1, 0))))
}

fn live_conditional_cache() -> &'static Mutex<LiveConditionalCache> {
    static CACHE: OnceLock<Mutex<LiveConditionalCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(LiveConditionalCache::new()))
}

#[cfg(test)]
mod live_conditional_program_tests {
    use super::*;
    use vyre::ir::{DataType, Program};

    fn assert_byte_buffers_are_raw_u8(program: &Program, label: &str) {
        for name in ["source", "macro_names_packed"] {
            let buffer = program
                .buffers()
                .iter()
                .find(|buffer| buffer.name() == name)
                .unwrap_or_else(|| {
                    panic!("Fix: {label} live conditional {name} buffer must exist")
                });
            assert_eq!(buffer.element(), DataType::U8);
            assert_eq!(buffer.count(), 0);
        }
    }

    #[test]
    fn live_conditional_programs_consume_raw_u8_byte_buffers() {
        assert_byte_buffers_are_raw_u8(&live_ifdef_program(), "ifdef");
        assert_byte_buffers_are_raw_u8(&live_if_expression_program(), "if-expression");
    }
}

#[derive(Default)]
pub(super) struct LiveConditionalScratch {
    row_start_b: Vec<u8>,
    row_len_b: Vec<u8>,
    directive_kind_b: Vec<u8>,
    out_scalar: Vec<u8>,
    dispatch_outputs: Vec<Vec<u8>>,
    batch_row_starts: Vec<u32>,
    batch_row_lens: Vec<u32>,
    batch_directive_kinds: Vec<u32>,
    batch_source: Vec<u8>,
    batch_values: Vec<u32>,
    batch_truths: Vec<bool>,
}

impl LiveConditionalScratch {
    fn prepare_scalar(&mut self, row_len: u32, directive_kind: u32) -> Result<(), String> {
        pack_u32_words_into(&mut self.row_start_b, &[0], 1)?;
        pack_u32_words_into(&mut self.row_len_b, &[row_len], 1)?;
        pack_u32_words_into(&mut self.directive_kind_b, &[directive_kind], 1)?;
        self.out_scalar.clear();
        reserve_live_vec(
            &mut self.out_scalar,
            4,
            "live conditional scalar output bytes",
        )?;
        self.out_scalar.resize(4, 0);
        Ok(())
    }
}

fn insert_live_conditional_cache_value(
    key: LiveConditionalCacheKey,
    value: bool,
    label: &str,
) -> Result<(), String> {
    let mut cache = live_conditional_cache()
        .lock()
        .map_err(|error| format!("{label} conditional cache lock poisoned: {error}"))?;
    cache.insert(key, value);
    Ok(())
}

fn reserve_live_vec<T>(
    out: &mut Vec<T>,
    additional: usize,
    label: &'static str,
) -> Result<(), String> {
    out.try_reserve_exact(additional).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {additional} {label}: {error:?}. Fix: shard preprocessing before live conditional evaluation."
        )
    })
}

fn live_word_bytes(word_count: usize, label: &'static str) -> Result<usize, String> {
    word_count.checked_mul(4).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: live conditional {label} word count {word_count} overflows host byte sizing. Fix: shard preprocessing before live conditional evaluation."
        )
    })
}

pub(super) struct LiveMacroNameBuffers {
    names: Vec<u8>,
    offsets: Vec<u8>,
    values: Vec<u8>,
    names_len: u32,
    count: u32,
    defined_fingerprint: [u8; 16],
    value_fingerprint: [u8; 16],
}

fn live_macro_name_buffers(macros: &[MacroDef]) -> Result<LiveMacroNameBuffers, String> {
    // Pre-size names from the sum of macro-name byte lengths so a
    // 1k-macro defines table grows in one allocation rather than
    // doubling repeatedly during the build.
    let total_name_bytes = macros.iter().try_fold(0usize, |total, mac| {
        total.checked_add(mac.name.len()).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: live macro-name byte total overflows usize. Fix: shard preprocessing before live conditional evaluation.".to_string()
        })
    })?;
    let mut names = Vec::new();
    reserve_live_vec(&mut names, total_name_bytes, "live macro-name bytes")?;
    let offset_slots = macros.len().checked_add(1).ok_or_else(|| {
        "vyre-libs::gpu_pipeline: live macro-offset slot count overflows usize. Fix: shard preprocessing before live conditional evaluation.".to_string()
    })?;
    let mut offsets = Vec::new();
    reserve_live_vec(&mut offsets, offset_slots, "live macro offsets")?;
    let mut seen_names = HashSet::default();
    seen_names.try_reserve(macros.len()).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {} live macro-name dedupe slots: {error:?}. Fix: shard preprocessing before live conditional evaluation.",
            macros.len()
        )
    })?;
    offsets.push(0);
    for mac in macros {
        if mac.name.is_empty() {
            return Err(
                "vyre-libs::gpu_pipeline: empty live macro name reached live conditional packing. Fix: reject malformed #define rows before GPU conditional evaluation."
                    .to_string(),
            );
        }
        if !seen_names.insert(mac.name.as_slice()) {
            let name = String::from_utf8_lossy(&mac.name);
            return Err(format!(
                "vyre-libs::gpu_pipeline: duplicate live macro `{name}` reached live conditional packing. Fix: replace existing definitions before GPU conditional evaluation."
            ));
        }
        names.extend_from_slice(&mac.name);
        offsets.push(checked_gpu_u32(
            "live conditional macro-name table byte length",
            names.len(),
        )?);
    }
    let values = macro_values::macro_integer_values_with_builtin_prefix(macros)?;
    let names_len = checked_gpu_u32("live conditional macro-name table byte length", names.len())?;
    let num_macros = checked_gpu_u32("live conditional macro definition count", macros.len())?;
    if names.is_empty() {
        reserve_live_vec(&mut names, 1, "live macro-name sentinel byte")?;
        names.push(0);
    }
    let offsets = pack_u32_words(&offsets, offsets.len())?;
    let values = pack_u32_words(&values, values.len().max(1))?;
    let defined_fingerprint = live_macro_buffer_fingerprint(&[
        names.as_slice(),
        offsets.as_slice(),
        &names_len.to_le_bytes(),
        &num_macros.to_le_bytes(),
    ]);
    let value_fingerprint = live_macro_buffer_fingerprint(&[
        names.as_slice(),
        offsets.as_slice(),
        values.as_slice(),
        &names_len.to_le_bytes(),
        &num_macros.to_le_bytes(),
    ]);
    Ok(LiveMacroNameBuffers {
        names,
        offsets,
        values,
        names_len,
        count: num_macros,
        defined_fingerprint,
        value_fingerprint,
    })
}

fn live_macro_buffer_fingerprint(parts: &[&[u8]]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}

pub(super) fn cached_live_macro_name_buffers<'a>(
    macros: &[MacroDef],
    cache: &'a mut Option<LiveMacroNameBuffers>,
) -> Result<&'a LiveMacroNameBuffers, String> {
    if cache.is_none() {
        *cache = Some(live_macro_name_buffers(macros)?);
    }
    cache.as_ref().ok_or_else(|| {
        "vyre-libs::gpu_pipeline: live conditional macro buffer cache was not initialized"
            .to_string()
    })
}

pub(super) fn replace_live_macro_indexed(
    macros: &mut Vec<MacroDef>,
    macro_index: &mut HashMap<Vec<u8>, usize>,
    replacement: MacroDef,
) {
    let name = replacement.name.clone();
    remove_live_macro_indexed(macros, macro_index, &name);
    macro_index.insert(name, macros.len());
    macros.push(replacement);
}

pub(super) fn remove_live_macro_indexed(
    macros: &mut Vec<MacroDef>,
    macro_index: &mut HashMap<Vec<u8>, usize>,
    name: &[u8],
) {
    let Some(index) = macro_index.remove(name) else {
        return;
    };
    macros.swap_remove(index);
    if index < macros.len() {
        macro_index.insert(macros[index].name.clone(), index);
    }
}

#[cfg(test)]
mod live_macro_tests {
    use super::*;

    fn object_macro(name: &[u8], body: &[u8]) -> MacroDef {
        MacroDef {
            name: name.to_vec(),
            args: Vec::new(),
            body: body.to_vec(),
            is_function_like: false,
        }
    }

    #[test]
    fn replace_live_macro_keeps_one_authoritative_definition() {
        let mut macros = vec![object_macro(b"FEATURE", b"0"), object_macro(b"OTHER", b"1")];
        let mut index = macros
            .iter()
            .enumerate()
            .map(|(idx, mac)| (mac.name.clone(), idx))
            .collect();
        replace_live_macro_indexed(&mut macros, &mut index, object_macro(b"FEATURE", b"1"));
        assert_eq!(macros.len(), 2);
        assert_eq!(
            macros
                .iter()
                .filter(|mac| mac.name.as_slice() == b"FEATURE")
                .count(),
            1
        );
        assert_eq!(
            macros
                .iter()
                .find(|mac| mac.name.as_slice() == b"FEATURE")
                .expect("Fix: FEATURE must exist")
                .body,
            b"1"
        );
    }

    #[test]
    fn indexed_live_macro_mutation_keeps_authoritative_table_without_scans() {
        let mut macros = Vec::new();
        let mut index = HashMap::default();
        replace_live_macro_indexed(&mut macros, &mut index, object_macro(b"A", b"1"));
        replace_live_macro_indexed(&mut macros, &mut index, object_macro(b"B", b"2"));
        replace_live_macro_indexed(&mut macros, &mut index, object_macro(b"A", b"3"));
        assert_eq!(macros.len(), 2);
        assert_eq!(index.len(), 2);
        let a_index = *index
            .get(b"A".as_slice())
            .expect("Fix: A must remain indexed");
        assert_eq!(macros[a_index].body, b"3");
        remove_live_macro_indexed(&mut macros, &mut index, b"B");
        assert_eq!(macros.len(), 1);
        assert!(!index.contains_key(b"B".as_slice()));
        let a_index = *index
            .get(b"A".as_slice())
            .expect("Fix: A index must be repaired");
        assert_eq!(macros[a_index].name, b"A");
    }
}

/// Re-evaluate an `#ifdef` / `#ifndef` row against the live macro table with
/// the same GPU kernel used by the batched directive extraction pass.
pub(super) fn recompute_ifdef_truth_gpu_with_scratch(
    dispatcher: &dyn GpuDispatcher,
    row_bytes: &[u8],
    directive_kind: u32,
    negated: bool,
    macro_buffers: &LiveMacroNameBuffers,
    scratch: &mut LiveConditionalScratch,
) -> Result<bool, String> {
    let row_len = checked_gpu_u32("live ifdef directive row length", row_bytes.len())?;
    let cache_key = LiveConditionalCacheKey {
        evaluator: 0,
        directive_kind,
        negated,
        row_fingerprint: live_macro_buffer_fingerprint(&[row_bytes]),
        row_len,
        macro_fingerprint: macro_buffers.defined_fingerprint,
        macro_names_len: macro_buffers.names_len,
        num_macros: macro_buffers.count,
    };
    if let Some(value) = live_conditional_cache()
        .lock()
        .map_err(|error| format!("live ifdef conditional cache lock poisoned: {error}"))?
        .lookup(&cache_key)
    {
        return Ok(value);
    }
    // gpu_ifdef_value_u8 reads source and macro table sizes at runtime via
    // Expr::buf_len, so no construction-time bucketing is needed for those
    // dimensions and the row bytes do not need host-side U32 repacking.
    let program = live_ifdef_program();
    scratch.prepare_scalar(row_len, directive_kind)?;
    let inputs: [&[u8]; 7] = [
        &scratch.row_start_b,
        &scratch.row_len_b,
        &scratch.directive_kind_b,
        row_bytes,
        &macro_buffers.names,
        &macro_buffers.offsets,
        &scratch.out_scalar,
    ];
    dispatcher
        .dispatch_borrowed_into(&program, &inputs, &mut scratch.dispatch_outputs)
        .map_err(|e| format!("gpu_ifdef_value live conditional: {e}"))?;
    if scratch.dispatch_outputs.len() != 1 {
        return Err(format!(
            "gpu_ifdef_value live conditional: expected exactly 1 output, got {}. Fix: backend must return only the defined flag.",
            scratch.dispatch_outputs.len()
        ));
    }
    let truth_buf = &scratch.dispatch_outputs[0];
    let value = read_u32_scalar_exact(truth_buf, "gpu_ifdef_value live conditional truth")? == 1;
    insert_live_conditional_cache_value(cache_key, value, "live ifdef")?;
    Ok(value)
}

pub(super) struct IfdefTruthRow<'a> {
    pub(super) row_bytes: &'a [u8],
    pub(super) directive_kind: u32,
}

pub(super) fn recompute_ifdef_truths_gpu_with_scratch<'a>(
    dispatcher: &dyn GpuDispatcher,
    rows: &[IfdefTruthRow<'_>],
    macro_buffers: &LiveMacroNameBuffers,
    scratch: &'a mut LiveConditionalScratch,
) -> Result<&'a [bool], String> {
    if rows.is_empty() {
        scratch.batch_truths.clear();
        return Ok(&scratch.batch_truths);
    }
    let row_count_bucket = bucket_pow2(rows.len(), 64);
    scratch.batch_row_starts.clear();
    scratch.batch_row_lens.clear();
    scratch.batch_directive_kinds.clear();
    scratch.batch_source.clear();
    reserve_live_vec(
        &mut scratch.batch_row_starts,
        row_count_bucket,
        "batched live ifdef row starts",
    )?;
    reserve_live_vec(
        &mut scratch.batch_row_lens,
        row_count_bucket,
        "batched live ifdef row lengths",
    )?;
    reserve_live_vec(
        &mut scratch.batch_directive_kinds,
        row_count_bucket,
        "batched live ifdef directive kinds",
    )?;
    let batch_source_bytes = rows.iter().try_fold(0usize, |total, row| {
        total.checked_add(row.row_bytes.len()).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: batched live ifdef source bytes overflow usize. Fix: shard preprocessing before live conditional evaluation.".to_string()
        })
    })?;
    reserve_live_vec(
        &mut scratch.batch_source,
        batch_source_bytes,
        "batched live ifdef source bytes",
    )?;
    for row in rows {
        scratch.batch_row_starts.push(checked_gpu_u32(
            "batched live ifdef directive row start",
            scratch.batch_source.len(),
        )?);
        scratch.batch_row_lens.push(checked_gpu_u32(
            "batched live ifdef directive row length",
            row.row_bytes.len(),
        )?);
        scratch.batch_directive_kinds.push(row.directive_kind);
        scratch.batch_source.extend_from_slice(row.row_bytes);
    }
    let program = gpu_ifdef_value_u8(row_count_bucket as u32, 0);
    pack_u32_words_into(
        &mut scratch.row_start_b,
        &scratch.batch_row_starts,
        row_count_bucket,
    )?;
    pack_u32_words_into(
        &mut scratch.row_len_b,
        &scratch.batch_row_lens,
        row_count_bucket,
    )?;
    pack_u32_words_into(
        &mut scratch.directive_kind_b,
        &scratch.batch_directive_kinds,
        row_count_bucket,
    )?;
    let out_scalar_bytes = live_word_bytes(row_count_bucket, "batched ifdef output")?;
    scratch.out_scalar.clear();
    reserve_live_vec(
        &mut scratch.out_scalar,
        out_scalar_bytes,
        "batched live ifdef output bytes",
    )?;
    scratch.out_scalar.resize(out_scalar_bytes, 0);
    let inputs: [&[u8]; 7] = [
        &scratch.row_start_b,
        &scratch.row_len_b,
        &scratch.directive_kind_b,
        &scratch.batch_source,
        &macro_buffers.names,
        &macro_buffers.offsets,
        &scratch.out_scalar,
    ];
    dispatcher
        .dispatch_borrowed_into(&program, &inputs, &mut scratch.dispatch_outputs)
        .map_err(|e| format!("gpu_ifdef_value batched live conditional: {e}"))?;
    if scratch.dispatch_outputs.len() != 1 {
        return Err(format!(

            "gpu_ifdef_value batched live conditional: expected exactly 1 output, got {}. Fix: backend must return only the defined flags.",
            scratch.dispatch_outputs.len()
        ));
    }
    unpack_u32_words_exact_into(
        &scratch.dispatch_outputs[0],
        row_count_bucket,
        "gpu_ifdef_value batched live conditional truth",
        &mut scratch.batch_values,
    )?;
    scratch.batch_truths.clear();
    scratch.batch_truths.extend(
        scratch.batch_values[..rows.len()]
            .iter()
            .map(|value| *value == 1),
    );
    Ok(&scratch.batch_truths)
}

/// Re-evaluate an `#if` / `#elif` row against the live macro table with the
/// GPU expression evaluator. Malformed expressions retain the kernel contract:
/// the emitted value is `0`.
pub(super) fn recompute_if_expr_truth_gpu_with_scratch(
    dispatcher: &dyn GpuDispatcher,
    row_bytes: &[u8],
    directive_kind: u32,
    _macros: &[MacroDef],
    macro_buffers: &LiveMacroNameBuffers,
    scratch: &mut LiveConditionalScratch,
) -> Result<bool, String> {
    let row_len = checked_gpu_u32("live if directive row length", row_bytes.len())?;
    let cache_key = LiveConditionalCacheKey {
        evaluator: 1,
        directive_kind,
        negated: false,
        row_fingerprint: live_macro_buffer_fingerprint(&[row_bytes]),
        row_len,
        macro_fingerprint: macro_buffers.value_fingerprint,
        macro_names_len: macro_buffers.names_len,
        num_macros: macro_buffers.count,
    };
    if let Some(value) = live_conditional_cache()
        .lock()
        .map_err(|error| format!("live if-expression conditional cache lock poisoned: {error}"))?
        .lookup(&cache_key)
    {
        return Ok(value);
    }
    // Same runtime-bound raw source treatment as recompute_ifdef_truth_gpu.
    let program = live_if_expression_program();
    scratch.prepare_scalar(row_len, directive_kind)?;
    let inputs: [&[u8]; 8] = [
        &scratch.row_start_b,
        &scratch.row_len_b,
        &scratch.directive_kind_b,
        row_bytes,
        &macro_buffers.names,
        &macro_buffers.offsets,
        &macro_buffers.values,
        &scratch.out_scalar,
    ];
    dispatcher
        .dispatch_borrowed_into(&program, &inputs, &mut scratch.dispatch_outputs)
        .map_err(|e| format!("gpu_if_expression live conditional: {e}"))?;
    if scratch.dispatch_outputs.len() != 1 {
        return Err(format!(
            "gpu_if_expression live conditional: expected exactly 1 output, got {}. Fix: backend must return only the expression value.",
            scratch.dispatch_outputs.len()
        ));
    }
    let value_buf = &scratch.dispatch_outputs[0];
    let raw_value = read_u32_scalar_exact(
        value_buf,
        "gpu_if_expression live conditional expression value",
    )?;
    if raw_value == INVALID_EXPR_VALUE {
        return Err(
            "gpu_if_expression live conditional rejected malformed #if/#elif expression. Fix: repair division/modulo-by-zero or malformed arithmetic before preprocessing."
                .to_string(),
        );
    }
    let value = raw_value == 1;
    insert_live_conditional_cache_value(cache_key, value, "live if-expression")?;
    Ok(value)
}

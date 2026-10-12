use std::cell::RefCell;
use std::mem;

use vyre::ir::{Expr, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_libs::parsing::c::parse::structure::{c11_extract_calls, c11_extract_functions};

use super::buffers::{mark_program_outputs, read_u32_at};
use super::{
    dispatch_borrowed_stage_cached_into, stage_pipeline_cache_key, validate_internal_stage,
};

pub(super) struct StructureRecords {
    pub(super) functions: Vec<u8>,
    pub(super) calls: Vec<u8>,
    pub(super) function_count: u32,
}

#[derive(Default)]
struct StructureRecordScratch {
    function_outputs: Vec<Vec<u8>>,
    call_outputs: Vec<Vec<u8>>,
}

thread_local! {
    static STRUCTURE_RECORD_SCRATCH: RefCell<StructureRecordScratch> =
        RefCell::new(StructureRecordScratch::default());
}

fn compact_record_stream(
    label: &str,
    stage: &str,
    stream_name: &str,
    mut records: Vec<u8>,
    declared_words: u32,
    record_words: u32,
) -> Result<Vec<u8>, String> {
    let declared_bytes = usize::try_from(declared_words)
        .ok()
        .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            format!(
                "{label} {stage} declared {declared_words} {stream_name} words, which overflows host byte indexing. Fix: cap GPU record counts before readback."
            )
        })?;
    let sentinel_bytes = usize::try_from(record_words)
        .ok()
        .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            format!(
                "{label} {stage} uses {record_words}-word {stream_name} records, which overflows host byte indexing. Fix: repair record schema width."
            )
        })?;
    if records.len() < declared_bytes {
        return Err(format!(
            "{label} {stage} declared {declared_words} {stream_name} words ({declared_bytes} bytes) but returned only {} bytes. Fix: backend must return the declared record prefix instead of relying on zero-padding.",
            records.len()
        ));
    }
    if records[declared_bytes..].iter().any(|byte| *byte != 0) {
        return Err(format!(
            "{label} {stage} declared {declared_words} {stream_name} words ({declared_bytes} bytes) but returned nonzero tail data in a {} byte buffer. Fix: record counts must cover every emitted record; do not hide live records past the count prefix.",
            records.len()
        ));
    }
    let compact_bytes = declared_bytes.max(sentinel_bytes);
    if records.len() > compact_bytes {
        records.truncate(compact_bytes);
    } else if records.len() < compact_bytes {
        records.resize(compact_bytes, 0);
    }
    Ok(records)
}

fn compact_sparse_record_stream(
    label: &str,
    stage: &str,
    stream_name: &str,
    mut records: Vec<u8>,
    declared_words: u32,
    record_words: u32,
) -> Result<Vec<u8>, String> {
    if record_words == 0 || declared_words % record_words != 0 {
        return Err(format!(
            "{label} {stage} declared {declared_words} {stream_name} words, not whole {record_words}-word records. Fix: repair sparse record schema accounting."
        ));
    }
    let declared_bytes = usize::try_from(declared_words)
        .ok()
        .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            format!(
                "{label} {stage} declared {declared_words} sparse {stream_name} words, which overflows host byte indexing. Fix: cap GPU record counts before readback."
            )
        })?;
    if records.len() < declared_bytes {
        return Err(format!(
            "{label} {stage} declared {declared_words} sparse {stream_name} words ({declared_bytes} bytes) but returned only {} bytes. Fix: backend must return the full deterministic record span.",
            records.len()
        ));
    }
    if records[declared_bytes..].iter().any(|byte| *byte != 0) {
        return Err(format!(
            "{label} {stage} returned nonzero data after the declared sparse {stream_name} span. Fix: stage counts must cover every emitted record."
        ));
    }
    let record_bytes = usize::try_from(record_words)
        .ok()
        .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            format!(
                "{label} {stage} {stream_name} record width {record_words} overflows host byte indexing. Fix: repair record schema width."
            )
        })?;
    let mut write = 0usize;
    let mut read = 0usize;
    while read < declared_bytes {
        let next = read + record_bytes;
        if records[read..next].iter().any(|byte| *byte != 0) {
            if write != read {
                records.copy_within(read..next, write);
            }
            write += record_bytes;
        }
        read = next;
    }
    if write == 0 {
        records.clear();
        records.resize(record_bytes, 0);
    } else {
        records.truncate(write);
    }
    Ok(records)
}

fn dispatch_structure_stage_into<F>(
    backend: &dyn VyreBackend,
    stage_key: crate::hash::StableHash128,
    build_program: F,
    inputs: &[&[u8]],
    config: &DispatchConfig,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), vyre::BackendError>
where
    F: FnOnce() -> Result<Program, String>,
{
    if backend.id() == "cuda" {
        let program = build_program().map_err(|message| vyre::BackendError::DispatchFailed {
            code: None,
            message,
        })?;
        return backend.dispatch_borrowed_into(&program, inputs, config, outputs);
    }
    dispatch_borrowed_stage_cached_into(backend, stage_key, build_program, inputs, config, outputs)
}

pub(super) fn build_structure_records(
    backend: &dyn VyreBackend,
    token_types: &[u8],
    paren_pairs: &[u8],
    brace_pairs: &[u8],
    n_tokens: u32,
    config: &mut DispatchConfig,
    label: &str,
) -> Result<StructureRecords, String> {
    STRUCTURE_RECORD_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "structure-record dispatch scratch was re-entered on the same thread. Fix: call structure extraction from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        build_structure_records_with_scratch(
            backend,
            token_types,
            paren_pairs,
            brace_pairs,
            n_tokens,
            config,
            label,
            &mut scratch,
        )
    })
}

fn build_structure_records_with_scratch(
    backend: &dyn VyreBackend,
    token_types: &[u8],
    paren_pairs: &[u8],
    brace_pairs: &[u8],
    n_tokens: u32,
    config: &mut DispatchConfig,
    label: &str,
    scratch: &mut StructureRecordScratch,
) -> Result<StructureRecords, String> {
    let nt = n_tokens.max(1);
    config.label = Some(format!("{label} functions"));
    let previous_grid_override = config.grid_override;
    config.grid_override = Some([nt.div_ceil(256), 1, 1]);
    let fn_key = stage_pipeline_cache_key("c11_extract_functions_sparse_records_v3", &[nt as u64]);
    let zero_function_count = 0u32.to_le_bytes();
    let function_inputs = [
        token_types,
        paren_pairs,
        brace_pairs,
        zero_function_count.as_slice(),
    ];
    let function_dispatch = dispatch_structure_stage_into(
        backend,
        fn_key,
        || {
            let fn_prog = c11_extract_functions(
                "tok_types",
                "paren_pairs",
                "brace_pairs",
                Expr::u32(nt),
                "out_functions",
                "out_counts",
            );
            let fn_prog = mark_program_outputs(fn_prog, &["out_functions"]);
            validate_internal_stage(&fn_prog, "c11_extract_functions")?;
            Ok(fn_prog)
        },
        &function_inputs,
        config,
        &mut scratch.function_outputs,
    );
    config.grid_override = previous_grid_override;
    function_dispatch.map_err(|e| format!("{label} c11_extract_functions dispatch failed: {e}"))?;
    if scratch.function_outputs.len() != 2 {
        return Err(format!(
            "{label} c11_extract_functions: expected exactly 2 outputs, got {}. Fix: backend must return function records/counts and no extras.",
            scratch.function_outputs.len()
        ));
    }
    let fn_word_count = read_u32_at(&scratch.function_outputs[1], 0)
        .map_err(|e| format!("{label} function count: {e}"))?;
    if fn_word_count % 3 != 0 {
        return Err(format!(
            "{label} c11_extract_functions emitted {fn_word_count} words, not whole 3-word function records. Fix: repair function record allocation."
        ));
    }
    let max_fn_words = nt.saturating_mul(3);
    if fn_word_count > max_fn_words {
        return Err(format!(
            "{label} c11_extract_functions emitted {fn_word_count} words for {nt} tokens; capacity is {max_fn_words}. Fix: repair function record bounds."
        ));
    }
    let mut functions = Vec::new();
    mem::swap(&mut functions, &mut scratch.function_outputs[0]);
    let function_input_storage = if fn_word_count == max_fn_words {
        compact_sparse_record_stream(
            label,
            "c11_extract_functions",
            "function",
            functions,
            fn_word_count,
            3,
        )?
    } else {
        compact_record_stream(
            label,
            "c11_extract_functions",
            "function",
            functions,
            fn_word_count,
            3,
        )?
    };
    let function_input = function_input_storage.as_slice();
    let function_count = u32::try_from(function_input_storage.len() / (3 * std::mem::size_of::<u32>()))
        .map_err(|_| {
            format!(
                "{label} c11_extract_functions compacted function count exceeds u32. Fix: shard function extraction."
            )
        })?;
    config.label = Some(format!("{label} calls"));
    let previous_grid_override = config.grid_override;
    config.grid_override = Some([nt.div_ceil(256), 1, 1]);
    let call_key = stage_pipeline_cache_key(
        "c11_extract_calls_sparse_records_v3",
        &[nt as u64, function_count as u64],
    );
    let zero_call_count = 0u32.to_le_bytes();
    let call_inputs = [
        token_types,
        paren_pairs,
        function_input,
        zero_call_count.as_slice(),
    ];
    let call_dispatch = dispatch_structure_stage_into(
        backend,
        call_key,
        || {
            let call_prog = c11_extract_calls(
                "tok_types",
                "paren_pairs",
                "functions",
                Expr::u32(nt),
                Expr::u32(function_count),
                "out_calls",
                "out_counts",
            );
            let call_prog = mark_program_outputs(call_prog, &["out_calls"]);
            validate_internal_stage(&call_prog, "c11_extract_calls")?;
            Ok(call_prog)
        },
        &call_inputs,
        config,
        &mut scratch.call_outputs,
    );
    config.grid_override = previous_grid_override;
    call_dispatch.map_err(|e| format!("{label} c11_extract_calls dispatch failed: {e}"))?;
    if scratch.call_outputs.len() != 2 {
        return Err(format!(
            "{label} c11_extract_calls: expected exactly 2 outputs, got {}. Fix: backend must return call records/counts and no extras.",
            scratch.call_outputs.len()
        ));
    }
    let call_word_count =
        read_u32_at(&scratch.call_outputs[1], 0).map_err(|e| format!("{label} call count: {e}"))?;
    if call_word_count % 4 != 0 {
        return Err(format!(
            "{label} c11_extract_calls emitted {call_word_count} words, not whole 4-word call records. Fix: repair call record allocation."
        ));
    }
    let max_call_words = nt.saturating_mul(4);
    if call_word_count > max_call_words {
        return Err(format!(
            "{label} c11_extract_calls emitted {call_word_count} words for {nt} tokens; capacity is {max_call_words}. Fix: repair call record bounds."
        ));
    }
    let mut call_records = Vec::new();
    mem::swap(&mut call_records, &mut scratch.call_outputs[0]);
    let calls = if call_word_count == max_call_words {
        compact_sparse_record_stream(
            label,
            "c11_extract_calls",
            "call",
            call_records,
            call_word_count,
            4,
        )?
    } else {
        compact_record_stream(
            label,
            "c11_extract_calls",
            "call",
            call_records,
            call_word_count,
            4,
        )?
    };

    Ok(StructureRecords {
        functions: function_input_storage,
        calls,
        function_count,
    })
}

#[cfg(test)]
mod tests {
    use super::compact_record_stream;

    use vyre_primitives::wire::pack_u32_slice as pack_words;

    #[test]
    fn compact_record_stream_accepts_fixed_capacity_zero_tail() {
        let mut records = pack_words(&[7, 11, 13, 0, 0, 0, 0, 0, 0]);

        let compact = compact_record_stream(
            "test",
            "c11_extract_functions",
            "function",
            std::mem::take(&mut records),
            3,
            3,
        )
        .expect("Fix: zero tail should compact to declared prefix");

        assert_eq!(compact, pack_words(&[7, 11, 13]));
    }

    #[test]
    fn compact_record_stream_synthesizes_zero_record_sentinel() {
        let compact = compact_record_stream(
            "test",
            "c11_extract_calls",
            "call",
            pack_words(&[0, 0, 0, 0, 0, 0, 0, 0]),
            0,
            4,
        )
        .expect("Fix: zero declared records should retain one zero sentinel record");

        assert_eq!(compact, pack_words(&[0, 0, 0, 0]));
    }

    #[test]
    fn compact_record_stream_rejects_nonzero_tail_records() {
        let error = compact_record_stream(
            "test",
            "c11_extract_functions",
            "function",
            pack_words(&[7, 11, 13, 0, 99, 0]),
            3,
            3,
        )
        .expect_err("nonzero tail must not be hidden behind count prefix");

        assert!(error.contains("nonzero tail data"));
        assert!(error.contains("record counts must cover every emitted record"));
    }

    #[test]
    fn compact_record_stream_rejects_missing_declared_prefix() {
        let error = compact_record_stream(
            "test",
            "c11_extract_calls",
            "call",
            pack_words(&[1, 2, 3]),
            4,
            4,
        )
        .expect_err("declared prefix must be present");

        assert!(error.contains("declared 4 call words"));
        assert!(error.contains("returned only 12 bytes"));
    }

    #[test]
    fn call_extraction_uses_zeroed_sparse_count_input() {
        let source = include_str!("structure_records.rs");
        assert!(
            source.contains("let zero_call_count = 0u32.to_le_bytes();"),
            "Fix: c11_extract_calls must initialize the sparse count buffer before CUDA dispatch."
        );
        assert!(
            !source.contains("mark_program_outputs(call_prog, &[\"out_calls\", \"out_counts\"])"),
            "Fix: c11_extract_calls must not expose the sparse count buffer as an uninitialized write-only output."
        );
    }
}

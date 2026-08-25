use std::cell::RefCell;
use std::mem;
use std::path::Path;

use vyre::ir::Expr;
use vyre::{DispatchConfig, VyreBackend};

use vyre_libs::parsing::c::sema::registry::{
    c_sema_scope, c_sema_scope_packed_haystack, c_sema_scope_symbols_packed_haystack,
};

pub(super) struct SemaScopeResult {
    pub blob: Vec<u8>,
    pub byte_len: u64,
}

#[derive(Default)]
struct SemaScopeScratch {
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static SEMA_SCOPE_SCRATCH: RefCell<SemaScopeScratch> =
        RefCell::new(SemaScopeScratch::default());
}

const GPU_SCOPE_STRIDE_U32: usize = 4;
const OBJECT_SCOPE_STRIDE_U32: usize = 6;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_sema_scope(
    backend: &dyn VyreBackend,
    path: &Path,
    _tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    _source: &[u8],
    tok_types_bytes: &[u8],
    starts: &[u8],
    lens: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    nt: u32,
    packed_haystack: bool,
    readback: bool,
) -> Result<SemaScopeResult, String> {
    SEMA_SCOPE_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "semantic scope dispatch scratch was re-entered on the same thread. Fix: call semantic scope construction from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        build_sema_scope_with_scratch(
            backend,
            path,
            _tok_types,
            tok_starts,
            tok_lens,
            _source,
            tok_types_bytes,
            starts,
            lens,
            haystack,
            haystack_len,
            nt,
            packed_haystack,
            readback,
            &mut scratch,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn build_sema_scope_with_scratch(
    backend: &dyn VyreBackend,
    path: &Path,
    _tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    _source: &[u8],
    tok_types_bytes: &[u8],
    starts: &[u8],
    lens: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    nt: u32,
    packed_haystack: bool,
    readback: bool,
    scratch: &mut SemaScopeScratch,
) -> Result<SemaScopeResult, String> {
    let mut cfg = DispatchConfig::default();
    cfg.label = Some(format!("vyre-frontend-c sema {}", path.display()));
    let token_count = nt.max(1);
    let token_byte_len = usize::try_from(token_count)
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "c_sema_scope token buffer length overflows host indexing for nt={nt}. Fix: shard semantic scope construction before GPU dispatch."
            )
        })?;
    require_exact_input_len("tok_types", tok_types_bytes, token_byte_len)?;
    require_exact_input_len("tok_starts", starts, token_byte_len)?;
    require_exact_input_len("tok_lens", lens, token_byte_len)?;
    let expected_gpu_byte_len = u64::from(token_count)
        .checked_mul((GPU_SCOPE_STRIDE_U32 as u64) * 4)
        .ok_or_else(|| {
            format!(
                "c_sema_scope expected scope tree byte length overflows u64 for nt={nt}. Fix: shard semantic scope construction before GPU dispatch."
            )
        })?;
    let expected_object_byte_len = u64::from(token_count)
        .checked_mul((OBJECT_SCOPE_STRIDE_U32 as u64) * 4)
        .ok_or_else(|| {
            format!(
                "c_sema_scope expected object scope byte length overflows u64 for nt={nt}. Fix: shard semantic scope construction before object emission."
            )
        })?;
    let sema_key = super::stage_pipeline_cache_key(
        "c_sema_scope",
        &[
            haystack_len.max(1) as u64,
            nt.max(1) as u64,
            packed_haystack as u64,
            readback as u64,
        ],
    );
    super::backend_select::dispatch_borrowed_stage_cached_into(
        backend,
        sema_key,
        || {
            let sema_prog = if packed_haystack && !readback {
                c_sema_scope_symbols_packed_haystack(
                    "tok_types",
                    "tok_starts",
                    "tok_lens",
                    "haystack",
                    Expr::u32(haystack_len.max(1)),
                    Expr::u32(nt.max(1)),
                    "out_scope_tree",
                )
            } else if packed_haystack {
                c_sema_scope_packed_haystack(
                    "tok_types",
                    "tok_starts",
                    "tok_lens",
                    "haystack",
                    Expr::u32(haystack_len.max(1)),
                    Expr::u32(nt.max(1)),
                    "out_scope_tree",
                )
            } else {
                c_sema_scope(
                    "tok_types",
                    "tok_starts",
                    "tok_lens",
                    "haystack",
                    Expr::u32(haystack_len.max(1)),
                    Expr::u32(nt.max(1)),
                    "out_scope_tree",
                )
            };
            let sema_prog = super::buffers::mark_program_outputs_readback(
                sema_prog,
                &["out_scope_tree"],
                readback,
            );
            super::validate_internal_stage(&sema_prog, "c_sema_scope")?;
            Ok(sema_prog)
        },
        &[tok_types_bytes, starts, lens, haystack],
        &cfg,
        &mut scratch.outputs,
    )
    .map_err(|e| format!("c_sema_scope dispatch failed: {e}"))?;
    super::buffers::drop_suppressed_readbacks(&mut scratch.outputs);

    let gpu_blob = take_exact_scope_output_from_scratch(
        &mut scratch.outputs,
        expected_gpu_byte_len,
        readback,
    )?;
    let blob = if readback {
        append_scope_source_spans_in_place(gpu_blob, tok_starts, tok_lens, token_count)?
    } else {
        Vec::new()
    };
    Ok(SemaScopeResult {
        blob,
        byte_len: expected_object_byte_len,
    })
}

fn take_exact_scope_output_from_scratch(
    sema_out: &mut Vec<Vec<u8>>,
    expected_byte_len: u64,
    readback: bool,
) -> Result<Vec<u8>, String> {
    let expected_len = usize::try_from(expected_byte_len).map_err(|_| {
        format!(
            "c_sema_scope: expected scope tree byte length {expected_byte_len} exceeds this platform's addressable memory. Fix: split the semantic scope stage into bounded chunks before dispatch."
        )
    })?;
    if !readback {
        if sema_out.is_empty() {
            return Ok(Vec::new());
        }
        return Err(format!(
            "c_sema_scope: expected zero scope tree readbacks for summary-only dispatch, got {}. Fix: suppress out_scope_tree readback when only semantic-scope byte evidence is required.",
            sema_out.len()
        ));
    }
    if sema_out.len() != 1 {
        return Err(format!(
            "c_sema_scope: expected exactly one scope tree output, got {}. Fix: backend must return only out_scope_tree for this stage.",
            sema_out.len()
        ));
    }
    if sema_out[0].len() != expected_len {
        return Err(format!(
            "c_sema_scope: malformed scope tree output: expected {expected_len} bytes, got {}. Fix: backend must materialize exactly nt.max(1) * 16 bytes for out_scope_tree.",
            sema_out[0].len()
        ));
    }
    let mut output = Vec::new();
    mem::swap(&mut output, &mut sema_out[0]);
    Ok(output)
}

fn require_exact_input_len(name: &str, bytes: &[u8], expected_len: usize) -> Result<(), String> {
    if bytes.len() != expected_len {
        return Err(format!(
            "c_sema_scope input `{name}` has {} bytes, expected exactly {expected_len}. Fix: pass one u32 word per resident token; semantic scope construction never pads or truncates token streams.",
            bytes.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
fn take_exact_scope_output(
    mut sema_out: Vec<Vec<u8>>,
    expected_byte_len: u64,
    readback: bool,
) -> Result<Vec<u8>, String> {
    let expected_len = usize::try_from(expected_byte_len).map_err(|_| {
        format!(
            "c_sema_scope: expected scope tree byte length {expected_byte_len} exceeds this platform's addressable memory. Fix: split the semantic scope stage into bounded chunks before dispatch."
        )
    })?;
    if !readback {
        if !sema_out.is_empty() {
            return Err(format!(
                "c_sema_scope: expected zero scope tree readbacks for summary-only dispatch, got {}. Fix: suppress out_scope_tree readback when only semantic-scope byte evidence is required.",
                sema_out.len()
            ));
        }
        return Ok(Vec::new());
    }
    if sema_out.len() != 1 {
        return Err(format!(
            "c_sema_scope: expected exactly one scope tree output, got {}. Fix: backend must return only out_scope_tree for this stage.",
            sema_out.len()
        ));
    }
    let blob = sema_out
        .pop()
        .ok_or_else(|| "c_sema_scope: missing scope tree output".to_string())?;
    if blob.len() != expected_len {
        return Err(format!(
            "c_sema_scope: malformed scope tree output: expected {expected_len} bytes, got {}. Fix: backend must materialize exactly nt.max(1) * 16 bytes for out_scope_tree.",
            blob.len()
        ));
    }
    Ok(blob)
}

fn append_scope_source_spans_in_place(
    mut gpu_blob: Vec<u8>,
    tok_starts: &[u32],
    tok_lens: &[u32],
    token_count: u32,
) -> Result<Vec<u8>, String> {
    let rows = usize::try_from(token_count).map_err(|_| {
        format!(
            "c_sema_scope: token count {token_count} exceeds host address space. Fix: shard semantic scope evidence before object emission."
        )
    })?;
    if tok_starts.len() < rows || tok_lens.len() < rows {
        return Err(format!(
            "c_sema_scope: span streams are shorter than semantic rows: rows={rows}, starts={}, lens={}. Fix: keep GPU lexer span streams aligned with semantic scope rows.",
            tok_starts.len(),
            tok_lens.len()
        ));
    }
    let expected_gpu_len = rows
        .checked_mul(GPU_SCOPE_STRIDE_U32)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "c_sema_scope: GPU scope byte length overflows for {rows} rows. Fix: shard semantic scope evidence before object emission."
            )
        })?;
    if gpu_blob.len() != expected_gpu_len {
        return Err(format!(
            "c_sema_scope: malformed GPU scope payload: expected {expected_gpu_len} bytes, got {}. Fix: backend must emit exactly four u32 semantic words per token before object span attachment.",
            gpu_blob.len()
        ));
    }
    let object_len = rows
        .checked_mul(OBJECT_SCOPE_STRIDE_U32)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "c_sema_scope: object scope byte length overflows for {rows} rows. Fix: shard semantic scope evidence before object emission."
            )
        })?;
    gpu_blob.resize(object_len, 0);
    for row in (0..rows).rev() {
        let src = row
            .checked_mul(GPU_SCOPE_STRIDE_U32)
            .and_then(|word| word.checked_mul(4))
            .ok_or_else(|| {
                format!(
                    "c_sema_scope: row {row} byte offset overflows. Fix: shard semantic scope evidence before object emission."
                )
            })?;
        let dst = row
            .checked_mul(OBJECT_SCOPE_STRIDE_U32)
            .and_then(|word| word.checked_mul(4))
            .ok_or_else(|| {
                format!(
                    "c_sema_scope: object row {row} byte offset overflows. Fix: shard semantic scope evidence before object emission."
                )
            })?;
        gpu_blob.copy_within(src..src + GPU_SCOPE_STRIDE_U32 * 4, dst);
        gpu_blob[dst + GPU_SCOPE_STRIDE_U32 * 4..dst + GPU_SCOPE_STRIDE_U32 * 4 + 4]
            .copy_from_slice(&tok_starts[row].to_le_bytes());
        gpu_blob[dst + GPU_SCOPE_STRIDE_U32 * 4 + 4..dst + GPU_SCOPE_STRIDE_U32 * 4 + 8]
            .copy_from_slice(&tok_lens[row].to_le_bytes());
    }
    Ok(gpu_blob)
}

#[cfg(test)]
mod tests {
    use super::{append_scope_source_spans_in_place, take_exact_scope_output};

    #[test]
    fn exact_scope_output_accepts_single_exact_buffer() {
        let blob = vec![7; 16];
        let out = take_exact_scope_output(vec![blob.clone()], 16, true).unwrap();
        assert_eq!(out, blob);
    }

    #[test]
    fn exact_scope_output_rejects_extra_buffers() {
        let err = take_exact_scope_output(vec![vec![0; 16], vec![1; 16]], 16, true).unwrap_err();
        assert!(err.contains("expected exactly one scope tree output"));
    }

    #[test]
    fn exact_scope_output_rejects_trailing_bytes() {
        let err = take_exact_scope_output(vec![vec![0; 20]], 16, true).unwrap_err();
        assert!(err.contains("malformed scope tree output"));
    }

    #[test]
    fn summary_only_scope_output_accepts_zero_readbacks() {
        let out = take_exact_scope_output(Vec::new(), 16, false).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn scope_object_rows_attach_gpu_lexer_spans() {
        let gpu_rows = [
            1u32.to_le_bytes(),
            u32::MAX.to_le_bytes(),
            2u32.to_le_bytes(),
            0xfeed_u32.to_le_bytes(),
            1u32.to_le_bytes(),
            u32::MAX.to_le_bytes(),
            3u32.to_le_bytes(),
            0xbeef_u32.to_le_bytes(),
        ]
        .concat();
        let out = append_scope_source_spans_in_place(gpu_rows, &[4, 9], &[3, 1], 2).unwrap();
        let words: Vec<u32> = vyre_primitives::wire::decode_u32_le_bytes_all(&out);
        assert_eq!(
            words,
            vec![1, u32::MAX, 2, 0xfeed, 4, 3, 1, u32::MAX, 3, 0xbeef, 9, 1,]
        );
    }

    #[test]
    fn scope_object_rows_expand_in_place_when_capacity_allows() {
        let mut gpu_rows = Vec::with_capacity(12 * 4);
        for word in [1u32, u32::MAX, 2, 0xfeed, 1, u32::MAX, 3, 0xbeef] {
            gpu_rows.extend_from_slice(&word.to_le_bytes());
        }
        let ptr = gpu_rows.as_ptr();
        let out = append_scope_source_spans_in_place(gpu_rows, &[4, 9], &[3, 1], 2).unwrap();

        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.len(), 12 * 4);
        assert_eq!(out.capacity(), 12 * 4);
    }

    #[test]
    fn scope_object_rows_reject_short_span_streams() {
        let gpu_rows = vec![0; 8 * 4];
        let err = append_scope_source_spans_in_place(gpu_rows, &[1], &[1, 2], 2).unwrap_err();
        assert!(
            err.contains("span streams are shorter"),
            "unexpected diagnostic: {err}"
        );
    }
}

//! Diagnostic: run the GPU preprocessor directly on
//! `KERNEL_LIBC_SHAPED_SOURCE` (the fixture that the headline
//! `c11_typed_object_sections` test uses) and assert the output is
//! non-empty.
//!
//! When the headline fails with `tok_types.len() = 0`, the proximate
//! cause is an empty preprocessed source (verified via lex-trace:
//! `source.len=0`). This test isolates the GPU preprocessor stage
//! from the rest of the pipeline to confirm whether the bug is in the
//! preprocessor or downstream.

#[allow(unused_imports)]
use vyre_driver_wgpu as _;

#[path = "gpu_prep_kernel_libc_shape/event_state_contracts.rs"]
mod event_state_contracts;
#[path = "gpu_prep_kernel_libc_shape/include_acceleration_contracts.rs"]
mod include_acceleration_contracts;
#[path = "gpu_prep_kernel_libc_shape/macro_provenance_contracts.rs"]
mod macro_provenance_contracts;
#[path = "gpu_prep_kernel_libc_shape/preprocess_reference.rs"]
mod preprocess_reference;
mod support;

use std::path::PathBuf;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_preprocess_translation_unit, BackendDispatcher, IncludeLoader,
};

struct NullLoader;
impl IncludeLoader for NullLoader {
    fn load(
        &self,
        _path: &[u8],
        _is_system: bool,
        _is_next: bool,
        _from: &std::path::Path,
    ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
        Ok(None)
    }
}

#[test]
fn gpu_preprocess_returns_non_empty_for_kernel_libc_shaped_source() {
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let loader = NullLoader;
    let path = PathBuf::from("kernel_libc_shaped.c");
    let raw = support::KERNEL_LIBC_SHAPED_SOURCE.as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");

    eprintln!(
        "[gpu-prep-trace] raw.len={} out.len={} first_64_bytes={:?}",
        raw.len(),
        res.bytes.len(),
        &res.bytes[..res.bytes.len().min(64)]
    );

    assert_ne!(
        res.bytes.len(),
        0,
        "Fix: GPU preprocessor must emit non-empty output for a fixture that contains \
         no preprocessor directives. Got empty output for a {}-byte source.",
        raw.len()
    );
}

/// Reference-eval each filter pipeline STAGE individually on the
/// failing fixture, tracing what each stage produces. This isolates
/// which kernel emits the wrong output for a 734-byte input.
#[test]
fn reference_eval_filter_pipeline_stages_on_kernel_libc_shaped_source() {
    use vyre_primitives::math::prefix_scan::{prefix_scan, ScanKind};
    use vyre_primitives::parsing::line_splice_classify::line_splice_classify;
    // `byte_compact` and `comment_strip` are crate-private internals covered by
    // vyre-libs and WGPU integration tests. This frontend test isolates the
    // public splice mask and prefix-scan contract for the 734-byte fixture.

    fn cpu(prog: &vyre::ir::Program, inputs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        let value_inputs: Vec<vyre_reference::value::Value> =
            inputs.into_iter().map(|b| b.into()).collect();
        let outs =
            vyre_reference::reference_eval(prog, &value_inputs).expect("reference_eval succeeds");
        outs.into_iter().map(|v| v.to_bytes()).collect()
    }

    let raw = support::KERNEL_LIBC_SHAPED_SOURCE.as_bytes();
    let n = raw.len() as u32;
    let cap = (n as usize).max(1);
    let byte_buf_pad = (cap.div_ceil(4) * 4).max(4);
    let mut padded_input = raw.to_vec();
    padded_input.resize(byte_buf_pad, 0);

    eprintln!("[stage-trace] n={n} byte_buf_pad={byte_buf_pad}");

    // ---- Stage 1: line_splice_classify ----
    let splice_prog = line_splice_classify(n);
    let splice_out = cpu(&splice_prog, vec![padded_input.clone(), vec![0u8; cap * 4]]);
    eprintln!(
        "[stage1 line_splice] reference_eval returned {} buffers",
        splice_out.len()
    );
    // reference_eval returns only OUTPUT buffers (write-side). The
    // kept_mask is the kernel's only output, so it sits at index 0.
    let kept_mask = &splice_out[0];
    let ones_kept = kept_mask.chunks_exact(4).filter(|c| c[0] == 1).count();
    eprintln!(
        "[stage1 line_splice] out_buf.len={} ones_in_kept_mask={} (expect ~{})",
        kept_mask.len(),
        ones_kept,
        n
    );

    // ---- Stage 4: prefix_scan over the kept_mask alone (assume no
    // comments in fixture) ----
    if n <= 1024 {
        let scan_prog = prefix_scan("mask_in", "offsets_out", n, ScanKind::ExclusiveSum);
        let scan_out = cpu(&scan_prog, vec![kept_mask.clone(), vec![0u8; cap * 4]]);
        let offsets = &scan_out[0];
        eprintln!(
            "[stage4 prefix_scan] offsets.len={} last_word_at_n-1={}",
            offsets.len(),
            u32::from_le_bytes([
                offsets[(cap - 1) * 4],
                offsets[(cap - 1) * 4 + 1],
                offsets[(cap - 1) * 4 + 2],
                offsets[(cap - 1) * 4 + 3],
            ])
        );
    } else {
        eprintln!(
            "[stage4 prefix_scan] SKIPPED  -  n={} > 1024 (prefix_scan rejects n > 1024)",
            n
        );
    }

    assert!(
        ones_kept > 0,
        "line_splice_classify must keep at least some bytes for non-empty source"
    );
}

/// Logging dispatcher: runs each dispatch via reference_eval and
/// prints input/output buffer sizes + first-bytes summary. Localizes
/// which sub-dispatch returns the bad data within `gpu_filter_source_bytes`.
#[test]
fn logging_filter_pipeline_for_kernel_libc_shaped_source() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use vyre_libs::parsing::c::preprocess::gpu_pipeline::{gpu_filter_source_bytes, GpuDispatcher};
    struct LoggingDispatcher {
        idx: AtomicUsize,
    }
    impl GpuDispatcher for LoggingDispatcher {
        fn dispatch(
            &self,
            program: &vyre::ir::Program,
            inputs: &[Vec<u8>],
        ) -> Result<Vec<Vec<u8>>, String> {
            let i = self.idx.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "[dispatch#{i}] buffers={} workgroup={:?} input_lens={:?}",
                program.buffers.len(),
                program.workgroup_size(),
                inputs.iter().map(|b| b.len()).collect::<Vec<_>>(),
            );
            for (j, buf) in program.buffers.iter().enumerate() {
                eprintln!(
                    "  buf[{j}] name={:?} access={:?} count={} is_output={}",
                    buf.name(),
                    buf.access,
                    buf.count,
                    buf.is_output
                );
            }
            let value_inputs: Vec<vyre_reference::value::Value> =
                inputs.iter().cloned().map(Into::into).collect();
            let outs = vyre_reference::reference_eval(program, &value_inputs)
                .map_err(|e| format!("reference_eval[{i}]: {e}"))?;
            let out_bytes: Vec<Vec<u8>> = outs.into_iter().map(|v| v.to_bytes()).collect();
            eprintln!(
                "  -> outputs={} output_lens={:?}",
                out_bytes.len(),
                out_bytes.iter().map(|b| b.len()).collect::<Vec<_>>(),
            );
            for (j, out) in out_bytes.iter().enumerate() {
                let words: Vec<u32> = out
                    .chunks_exact(4)
                    .take(8)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                let nonzero = out
                    .chunks_exact(4)
                    .filter(|c| c[0] != 0 || c[1] != 0 || c[2] != 0 || c[3] != 0)
                    .count();
                eprintln!(
                    "    out[{j}] first8_words={:?} nonzero_words={}",
                    words, nonzero
                );
            }
            Ok(out_bytes)
        }

        fn requires_output_inputs(&self) -> bool {
            true
        }
    }
    let dispatcher = LoggingDispatcher {
        idx: AtomicUsize::new(0),
    };
    let raw = support::KERNEL_LIBC_SHAPED_SOURCE.as_bytes();
    let res = gpu_filter_source_bytes(&dispatcher, raw).expect("filter pipeline succeeds");
    eprintln!(
        "[result] raw.len={} compacted.len={}",
        raw.len(),
        res.bytes.len()
    );
}

#[test]
fn gpu_preprocess_size_bisection() {
    // Bisection: at what input size does the GPU preprocessor start
    // returning empty output?
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let loader = NullLoader;
    let path = PathBuf::from("bisect.c");

    // Build inputs of increasing sizes by repeating "int x;\n" (7 bytes).
    let unit = b"int x;\n";
    for size in &[7usize, 252, 256, 257] {
        let mut raw = Vec::new();
        while raw.len() < *size {
            raw.extend_from_slice(unit);
        }
        raw.truncate(*size);
        let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, &raw, &[])
            .expect("gpu preprocess succeeds");
        eprintln!(
            "[gpu-prep-bisect] input.len={} out.len={} out_first20={:?}",
            raw.len(),
            res.bytes.len(),
            std::str::from_utf8(&res.bytes[..res.bytes.len().min(20)]).unwrap_or("<non-utf8>")
        );
    }
}

#[test]
fn gpu_preprocess_returns_non_empty_for_int_main() {
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let loader = NullLoader;
    let path = PathBuf::from("int_main.c");
    let raw = b"int main(void) { return 0; }\n";

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");

    eprintln!(
        "[gpu-prep-trace] raw.len={} out.len={} out_str={:?}",
        raw.len(),
        res.bytes.len(),
        std::str::from_utf8(&res.bytes).unwrap_or("<non-utf8>")
    );

    assert_ne!(
        res.bytes.len(),
        0,
        "GPU preprocessor must emit non-empty output for int main(void) {{ return 0; }}"
    );
}

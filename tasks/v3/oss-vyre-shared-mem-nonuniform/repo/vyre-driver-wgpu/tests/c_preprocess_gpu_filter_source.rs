//! Hardware WGPU parity tests for the C preprocessor source-filter pipeline.

#![allow(deprecated)]
use vyre_libs::parsing::c::preprocess::gpu_comment_strip_mask::reference_gpu_comment_strip_mask;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{gpu_filter_source_bytes, BackendDispatcher};
use vyre_primitives::parsing::line_splice_classify::reference_line_splice_classify;

fn reference_filter_source_bytes(raw: &[u8]) -> Vec<u8> {
    let splice_keep = reference_line_splice_classify(raw);
    let comment_mask = reference_gpu_comment_strip_mask(raw);
    raw.iter()
        .enumerate()
        .filter(|(idx, _)| splice_keep[*idx] == 1 && comment_mask[*idx] != 1)
        .map(|(idx, byte)| if comment_mask[idx] == 2 { b' ' } else { *byte })
        .collect()
}

fn assert_wgpu_filter_matches_reference(source: &[u8]) {
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: WGPU backend must acquire the local GPU for C source-filter parity tests");
    let filtered = gpu_filter_source_bytes(&BackendDispatcher(&backend), source)
        .expect("Fix: gpu_filter_source_bytes must dispatch on the WGPU backend");
    assert_eq!(filtered.bytes, reference_filter_source_bytes(source));
}

#[test]
fn wgpu_filter_source_bytes_matches_reference_for_mixed_c_source() {
    let source = b"#define JOIN(a,b) a/**/b\nint x = 1 + \\\n2; /* block */\n// line\nchar *s = \"// not comment\";\n";
    assert_wgpu_filter_matches_reference(source);
}

#[test]
fn wgpu_filter_source_bytes_matches_reference_for_simple_fast_paths() {
    assert_wgpu_filter_matches_reference(b"int x = 1; // trailing\nint y = 2;\n");
    assert_wgpu_filter_matches_reference(b"int x = /* stripped */ 1; int y = /* also */ 2;\n");
}

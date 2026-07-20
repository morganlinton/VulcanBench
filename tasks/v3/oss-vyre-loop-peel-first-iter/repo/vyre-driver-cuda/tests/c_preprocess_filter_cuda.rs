//! Live CUDA coverage for the C byte-filter pipeline.
//!
//! This drives the real `gpu_filter_source_bytes` orchestrator through raw
//! U8 source buffers, mask generation, prefix scans, and byte compaction.

#![cfg(test)]

mod common;

use common::with_live_backend;
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_libs::parsing::c::preprocess::gpu_comment_strip_mask::reference_gpu_comment_strip_mask;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{gpu_filter_source_bytes, GpuDispatcher};
use vyre_primitives::parsing::line_splice_classify::reference_line_splice_classify;

struct CudaFilterDispatcher<'a>(&'a CudaBackend);

impl GpuDispatcher for CudaFilterDispatcher<'_> {
    fn dispatch(
        &self,
        program: &vyre::ir::Program,
        inputs: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, String> {
        self.0
            .dispatch(program, inputs, &DispatchConfig::default())
            .map_err(|error| format!("CUDA dispatch: {error}"))
    }

    fn dispatch_borrowed(
        &self,
        program: &vyre::ir::Program,
        inputs: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, String> {
        self.0
            .dispatch_borrowed(program, inputs, &DispatchConfig::default())
            .map_err(|error| format!("CUDA borrowed dispatch: {error}"))
    }
}

fn reference_filter_source_bytes(raw: &[u8]) -> Vec<u8> {
    let splice_keep = reference_line_splice_classify(raw);
    let comment_mask = reference_gpu_comment_strip_mask(raw);
    raw.iter()
        .enumerate()
        .filter(|(i, _)| splice_keep[*i] == 1 && comment_mask[*i] != 1)
        .map(|(i, b)| if comment_mask[i] == 2 { b' ' } else { *b })
        .collect()
}

fn generated_c_source(case: u32, min_len: usize) -> Vec<u8> {
    let mut source = Vec::with_capacity(min_len + 256);
    let mut line = 0u32;
    while source.len() < min_len {
        match (case.wrapping_mul(17).wrapping_add(line)) % 8 {
            0 => source.extend_from_slice(format!("int keep_{case}_{line} = {line};\n").as_bytes()),
            1 => source.extend_from_slice(b"#define JOIN(a,b) \\\n  a ## b\n"),
            2 => source.extend_from_slice(b"int x = 1; // strip this line comment\n"),
            3 => source.extend_from_slice(b"int y = /* strip block */ 2;\n"),
            4 => source.extend_from_slice(br#"char *s = "/* not a comment */";"#),
            5 => source.extend_from_slice(b"\nint z = 3; /* multi\nline\ncomment */ int w = 4;\n"),
            6 => source.extend_from_slice(b"int q = a / b; char c = '/';\n"),
            _ => source.extend_from_slice(b"int tail = 1 + \\\r\n2; // crlf splice\n"),
        }
        line += 1;
    }
    source
}

fn generated_line_splice_only_source(case: u32, min_len: usize) -> Vec<u8> {
    let mut source = Vec::with_capacity(min_len + 128);
    let mut line = 0u32;
    while source.len() < min_len {
        if (case + line) % 2 == 0 {
            source.extend_from_slice(
                format!("int splice_{case}_{line} = {line} + \\\n{};\n", line + 1).as_bytes(),
            );
        } else {
            source.extend_from_slice(
                format!("int splice_{case}_{line} = {line} + \\\r\n{};\n", line + 1).as_bytes(),
            );
        }
        line += 1;
    }
    source
}

#[test]
fn cuda_c_preprocess_filter_u8_generated_corpus_matches_reference() {
    with_live_backend("c preprocess filter u8 generated corpus", |backend| {
        let dispatcher = CudaFilterDispatcher(backend);
        let mut checked_input = 0usize;
        let mut checked_output = 0usize;
        for case in 0..32u32 {
            let source = generated_c_source(case, 2049);
            let filtered = gpu_filter_source_bytes(&dispatcher, &source)
                .unwrap_or_else(|error| panic!("Fix: CUDA C filter case {case} failed: {error}"));
            let expected = reference_filter_source_bytes(&source);
            assert_eq!(
                filtered.bytes, expected,
                "Fix: CUDA C filter mismatch on generated case {case}"
            );
            checked_input += source.len();
            checked_output += filtered.bytes.len();
        }
        assert!(
            checked_input > 65_536,
            "Fix: generated CUDA C filter matrix must cross many workgroups."
        );
        assert!(
            checked_output > 32_768,
            "Fix: generated CUDA C filter matrix must compare substantial compacted output."
        );
    });
}

#[test]
fn cuda_c_preprocess_filter_line_splice_only_matrix_matches_reference() {
    with_live_backend("c preprocess filter line-splice-only matrix", |backend| {
        let dispatcher = CudaFilterDispatcher(backend);
        let mut checked_input = 0usize;
        let mut checked_output = 0usize;
        for case in 0..16u32 {
            let source = generated_line_splice_only_source(case, 4097);
            let filtered = gpu_filter_source_bytes(&dispatcher, &source).unwrap_or_else(|error| {
                panic!("Fix: CUDA C line-splice-only filter case {case} failed: {error}")
            });
            let expected = reference_filter_source_bytes(&source);
            assert_eq!(
                filtered.bytes, expected,
                "Fix: CUDA C line-splice-only filter mismatch on case {case}"
            );
            checked_input += source.len();
            checked_output += filtered.bytes.len();
        }
        assert!(
            checked_input > 65_536,
            "Fix: generated CUDA line-splice-only matrix must cross many workgroups."
        );
        assert!(
            checked_output > 32_768,
            "Fix: generated CUDA line-splice-only matrix must compare substantial compacted output."
        );
    });
}

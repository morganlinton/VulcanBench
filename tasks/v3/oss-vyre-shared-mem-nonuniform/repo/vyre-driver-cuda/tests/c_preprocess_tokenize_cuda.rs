//! Live CUDA coverage for the C sparse tokenization pipeline.
//!
//! This drives the real `gpu_tokenize_and_classify` orchestrator through a
//! raw U8 sparse-lexer haystack, prefix scan, sparse-token compaction, and
//! directive metadata classification.

#![cfg(test)]

mod common;

use common::with_live_backend;
use vyre::ir::Program;
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_tokenize_and_classify, ClassifiedTokens, GpuDispatcher,
};
use vyre_reference::value::Value;

struct RefDispatcher;

impl GpuDispatcher for RefDispatcher {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        let values: Vec<Value> = inputs.iter().cloned().map(Value::from).collect();
        let outputs = vyre_reference::reference_eval(program, &values)
            .map_err(|error| format!("reference_eval: {error}"))?;
        Ok(outputs.into_iter().map(|value| value.to_bytes()).collect())
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}

struct CudaTokenizeDispatcher<'a>(&'a CudaBackend);

impl GpuDispatcher for CudaTokenizeDispatcher<'_> {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        self.0
            .dispatch(program, inputs, &DispatchConfig::default())
            .map_err(|error| format!("CUDA dispatch: {error}"))
    }

    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, String> {
        self.0
            .dispatch_borrowed(program, inputs, &DispatchConfig::default())
            .map_err(|error| format!("CUDA borrowed dispatch: {error}"))
    }
}

fn generated_c_source(case: u32, min_len: usize) -> Vec<u8> {
    let mut source = Vec::with_capacity(min_len + 256);
    let mut line = 0u32;
    while source.len() < min_len {
        match (case.wrapping_mul(29).wrapping_add(line)) % 12 {
            0 => {
                source.extend_from_slice(format!("#define VALUE_{case}_{line} {line}\n").as_bytes())
            }
            1 => source.extend_from_slice(b"#if defined(VALUE_0_0) && (3 + 4) > 1\n"),
            2 => source.extend_from_slice(b"#elif 0\n#else\n#endif\n"),
            3 => source.extend_from_slice(b"int value = array[index++] + --other;\n"),
            4 => source.extend_from_slice(b"unsigned long mask = (a << 3) | (b >> 2);\n"),
            5 => source.extend_from_slice(br#"const char *s = "/* token text, not state */";"#),
            6 => source.extend_from_slice(b"\nchar c = '\\n'; char slash = '/';\n"),
            7 => source.extend_from_slice(b"float f = 1.25e+3f; double d = .5;\n"),
            8 => source.extend_from_slice(b"int q = a / b; /* sparse comment */ int r = q % 7;\n"),
            9 => source.extend_from_slice(b"#include <stddef.h>\n#pragma once\n#undef OLD\n"),
            10 => source.extend_from_slice(b"struct node { int x; struct node *next; };\n"),
            _ => source.extend_from_slice(b"if (x && y || z) { x += y; y -= z; }\n"),
        }
        line += 1;
    }
    source
}

fn assert_same_tokens(case: u32, actual: &ClassifiedTokens, expected: &ClassifiedTokens) {
    assert_eq!(
        actual.tok_types, expected.tok_types,
        "Fix: CUDA tokenizer token-kind mismatch on generated case {case}"
    );
    assert_eq!(
        actual.tok_starts, expected.tok_starts,
        "Fix: CUDA tokenizer token-start mismatch on generated case {case}"
    );
    assert_eq!(
        actual.tok_lens, expected.tok_lens,
        "Fix: CUDA tokenizer token-length mismatch on generated case {case}"
    );
    assert_eq!(
        actual.directive_kinds, expected.directive_kinds,
        "Fix: CUDA tokenizer directive-kind mismatch on generated case {case}"
    );
    assert_eq!(
        actual.directive_count, expected.directive_count,
        "Fix: CUDA tokenizer directive-count mismatch on generated case {case}"
    );
}

#[test]
fn cuda_c_preprocess_tokenizer_u8_generated_corpus_matches_reference() {
    with_live_backend("c preprocess tokenizer u8 generated corpus", |backend| {
        let cuda_dispatcher = CudaTokenizeDispatcher(backend);
        let reference_dispatcher = RefDispatcher;
        let mut checked_input = 0usize;
        let mut checked_tokens = 0usize;
        for case in 0..4u32 {
            let source = generated_c_source(case, 2049);
            let expected = gpu_tokenize_and_classify(&reference_dispatcher, &source)
                .unwrap_or_else(|error| {
                    panic!("Fix: reference tokenizer case {case} failed: {error}")
                });
            let actual = gpu_tokenize_and_classify(&cuda_dispatcher, &source)
                .unwrap_or_else(|error| panic!("Fix: CUDA tokenizer case {case} failed: {error}"));
            assert_same_tokens(case, &actual, &expected);
            checked_input += source.len();
            checked_tokens += actual.tok_types.len();
        }
        assert!(
            checked_input > 8_192,
            "Fix: generated CUDA tokenizer matrix must cross multiple workgroups."
        );
        assert!(
            checked_tokens > 1_024,
            "Fix: generated CUDA tokenizer matrix must compare substantial sparse-token output."
        );
    });
}

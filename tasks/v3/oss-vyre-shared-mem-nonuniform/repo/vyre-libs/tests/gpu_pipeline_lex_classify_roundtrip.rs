//! End-to-end test of `gpu_tokenize_and_classify`: lex (existing
//! `c11_lexer`) + directive classify (17a) chained, validated through
//! `vyre_reference::reference_eval`.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre::ir::{DataType, Program};
use vyre_libs::parsing::c::lex::tokens::{
    TOK_PP_DEFINE, TOK_PP_ELIF, TOK_PP_ELSE, TOK_PP_ENDIF, TOK_PP_IF, TOK_PP_IFDEF, TOK_PP_IFNDEF,
    TOK_PP_INCLUDE, TOK_PP_PRAGMA, TOK_PP_UNDEF, TOK_PREPROC,
};
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{gpu_tokenize_and_classify, GpuDispatcher};
use vyre_reference::value::Value;

struct RefDispatcher;

impl GpuDispatcher for RefDispatcher {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        let values: Vec<Value> = inputs.iter().cloned().map(Value::from).collect();
        let outputs = vyre_reference::reference_eval(program, &values)
            .map_err(|e| format!("reference_eval: {e}"))?;
        Ok(outputs.into_iter().map(|v| v.to_bytes().to_vec()).collect())
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}

struct CountingDispatcher {
    haystack_elements: std::cell::RefCell<Vec<DataType>>,
    haystack_input_lens: std::cell::RefCell<Vec<usize>>,
    source_elements: std::cell::RefCell<Vec<DataType>>,
}

impl CountingDispatcher {
    fn new() -> Self {
        Self {
            haystack_elements: std::cell::RefCell::new(Vec::new()),
            haystack_input_lens: std::cell::RefCell::new(Vec::new()),
            source_elements: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl GpuDispatcher for CountingDispatcher {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        self.haystack_elements.borrow_mut().extend(
            program
                .buffers()
                .iter()
                .filter_map(|buffer| (buffer.name() == "haystack").then_some(buffer.element())),
        );
        self.source_elements.borrow_mut().extend(
            program
                .buffers()
                .iter()
                .filter_map(|buffer| (buffer.name() == "source").then_some(buffer.element())),
        );
        self.haystack_input_lens.borrow_mut().extend(
            program
                .buffers()
                .iter()
                .position(|buffer| buffer.name() == "haystack")
                .and_then(|index| inputs.get(index))
                .map(Vec::len),
        );
        RefDispatcher.dispatch(program, inputs)
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}

fn assert_directive_source_dispatches_are_u8(dispatcher: &CountingDispatcher) {
    let elements = dispatcher.source_elements.borrow();
    assert!(
        !elements.is_empty(),
        "tokenization path must dispatch directive metadata over source bytes"
    );
    assert!(
        elements
            .iter()
            .all(|element| matches!(element, DataType::U8)),
        "directive metadata must consume raw U8 source buffers in the pipeline, got {elements:?}"
    );
}

fn assert_sparse_haystack_dispatches_are_u8(dispatcher: &CountingDispatcher) {
    let elements = dispatcher.haystack_elements.borrow();
    assert!(
        !elements.is_empty(),
        "tokenization path must dispatch a sparse lexer haystack program"
    );
    assert!(
        elements
            .iter()
            .all(|element| matches!(element, DataType::U8)),
        "sparse tokenizer must consume raw U8 haystack buffers, got {elements:?}"
    );
}

fn assert_sparse_haystack_inputs_are_unpadded(dispatcher: &CountingDispatcher, raw_len: usize) {
    let input_lens = dispatcher.haystack_input_lens.borrow();
    assert!(
        !input_lens.is_empty(),
        "tokenization path must dispatch a sparse lexer haystack program"
    );
    assert!(
        input_lens.iter().all(|len| *len == raw_len),
        "sparse tokenizer must consume raw haystack length {raw_len}, got {input_lens:?}"
    );
}

fn run(src: &[u8]) -> vyre_libs::parsing::c::preprocess::gpu_pipeline::ClassifiedTokens {
    gpu_tokenize_and_classify(&RefDispatcher, src).expect("gpu_tokenize_and_classify")
}

/// Helper: collect (kind, source-bytes) pairs for every TOK_PREPROC row.
fn directive_rows(
    classified: &vyre_libs::parsing::c::preprocess::gpu_pipeline::ClassifiedTokens,
) -> Vec<(u32, Vec<u8>)> {
    classified
        .directive_kinds
        .iter()
        .enumerate()
        .filter(|&(_, &k)| k != 0)
        .map(|(i, &k)| {
            let s = classified.tok_starts[i] as usize;
            let l = classified.tok_lens[i] as usize;
            (k, classified.source[s..s + l].to_vec())
        })
        .collect()
}

#[test]
fn empty_input_emits_no_tokens() {
    let out = run(b"");
    assert!(out.tok_types.is_empty());
    assert!(out.directive_kinds.is_empty());
}

#[test]
fn plain_code_has_no_directive_rows() {
    let out = run(b"int main(void) { return 0; }");
    assert!(
        out.directive_kinds.iter().all(|&k| k == 0),
        "non-PREPROC tokens must emit kind=0; got {:?}",
        out.directive_kinds
    );
    // At least one TOK_PREPROC NOT present in the stream.
    assert!(out.tok_types.iter().all(|&t| t != TOK_PREPROC));
}

#[test]
fn single_define_classifies_as_pp_define() {
    let out = run(b"#define FOO 1\n");
    let rows = directive_rows(&out);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, TOK_PP_DEFINE);
    assert!(rows[0].1.starts_with(b"#define FOO 1"));
}

#[test]
fn header_guard_emits_three_directives_with_correct_kinds() {
    let out = run(b"#ifndef G\n#define G\nint x;\n#endif\n");
    let rows = directive_rows(&out);
    let kinds: Vec<u32> = rows.iter().map(|(k, _)| *k).collect();
    assert_eq!(kinds, vec![TOK_PP_IFNDEF, TOK_PP_DEFINE, TOK_PP_ENDIF]);
}

#[test]
fn include_pragma_undef_each_classify_correctly() {
    let out = run(b"#include <stdio.h>\n#pragma once\n#undef X\n");
    let kinds: Vec<u32> = directive_rows(&out).into_iter().map(|(k, _)| k).collect();
    assert_eq!(kinds, vec![TOK_PP_INCLUDE, TOK_PP_PRAGMA, TOK_PP_UNDEF]);
}

#[test]
fn if_elif_else_endif_chain() {
    let out = run(b"#if 1\n#elif 0\n#else\n#endif\n");
    let kinds: Vec<u32> = directive_rows(&out).into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        kinds,
        vec![TOK_PP_IF, TOK_PP_ELIF, TOK_PP_ELSE, TOK_PP_ENDIF]
    );
}

#[test]
fn ifdef_ifndef_distinguished() {
    let out = run(b"#ifdef A\n#ifndef B\n");
    let kinds: Vec<u32> = directive_rows(&out).into_iter().map(|(k, _)| k).collect();
    assert_eq!(kinds, vec![TOK_PP_IFDEF, TOK_PP_IFNDEF]);
}

#[test]
fn mixed_code_and_directives_only_directive_rows_have_kinds() {
    let src = b"#define A 1\nint x = A;\n#define B 2\n";
    let dispatcher = CountingDispatcher::new();
    let out = gpu_tokenize_and_classify(&dispatcher, src).expect("gpu_tokenize_and_classify");
    let rows = directive_rows(&out);
    let kinds: Vec<u32> = rows.iter().map(|(k, _)| *k).collect();
    // Two #define rows both classified as TOK_PP_DEFINE; the int line
    // contributes only non-PREPROC tokens (zero in directive_kinds).
    assert_eq!(kinds, vec![TOK_PP_DEFINE, TOK_PP_DEFINE]);
    assert_sparse_haystack_dispatches_are_u8(&dispatcher);
    assert_sparse_haystack_inputs_are_unpadded(&dispatcher, src.len());
    assert_directive_source_dispatches_are_u8(&dispatcher);
}

#[test]
fn dense_real_world_header_block() {
    let src = b"#ifndef FOO_H\n#define FOO_H\n#include <stdio.h>\n#define MAX(a,b) ((a)>(b)?(a):(b))\n#if defined(__GNUC__)\n#endif\n#endif\n";
    let out = run(src);
    let kinds: Vec<u32> = directive_rows(&out).into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        kinds,
        vec![
            TOK_PP_IFNDEF,
            TOK_PP_DEFINE,
            TOK_PP_INCLUDE,
            TOK_PP_DEFINE,
            TOK_PP_IF,
            TOK_PP_ENDIF,
            TOK_PP_ENDIF,
        ]
    );
}

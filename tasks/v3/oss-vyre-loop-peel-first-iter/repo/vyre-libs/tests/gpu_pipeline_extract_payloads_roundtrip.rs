//! End-to-end test of `gpu_extract_directive_payloads`: dispatches
//! all four per-directive parsers (define, include, ifdef, if-expr)
//! over a token stream and assembles per-row payloads.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre::ir::{DataType, Program};
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_extract_directive_payloads, gpu_tokenize_and_classify, DirectivePayload, GpuDispatcher,
};
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
    fused_parse_source_elements: std::cell::RefCell<Vec<DataType>>,
    condition_source_elements: std::cell::RefCell<Vec<DataType>>,
    condition_macro_name_elements: std::cell::RefCell<Vec<DataType>>,
}

impl CountingDispatcher {
    fn new() -> Self {
        Self {
            fused_parse_source_elements: std::cell::RefCell::new(Vec::new()),
            condition_source_elements: std::cell::RefCell::new(Vec::new()),
            condition_macro_name_elements: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl GpuDispatcher for CountingDispatcher {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        if program
            .entry_op_id
            .as_deref()
            .is_some_and(|op_id| op_id.contains("define_include_undef_parse_fused"))
        {
            record_buffer_elements(program, "source", &self.fused_parse_source_elements);
        }
        if program.entry_op_id.as_deref().is_some_and(|op_id| {
            op_id.contains("gpu_ifdef_value") || op_id.contains("gpu_if_expression")
        }) {
            record_buffer_elements(program, "source", &self.condition_source_elements);
            record_buffer_elements(
                program,
                "macro_names_packed",
                &self.condition_macro_name_elements,
            );
        }
        RefDispatcher.dispatch(program, inputs)
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}

fn record_buffer_elements(
    program: &Program,
    name: &'static str,
    out: &std::cell::RefCell<Vec<DataType>>,
) {
    out.borrow_mut().extend(
        program
            .buffers()
            .iter()
            .filter_map(|buffer| (buffer.name() == name).then_some(buffer.element())),
    );
}

fn run(src: &[u8], macros: &[&[u8]]) -> Vec<DirectivePayload> {
    let classified = gpu_tokenize_and_classify(&RefDispatcher, src).expect("tokenize_and_classify");
    gpu_extract_directive_payloads(&RefDispatcher, &classified, macros)
        .expect("extract_directive_payloads")
}

fn assert_fused_payload_parse_source_is_u8(dispatcher: &CountingDispatcher) {
    let elements = dispatcher.fused_parse_source_elements.borrow();
    assert!(
        !elements.is_empty(),
        "directive payload extraction must dispatch the fused define/include/undef parser"
    );
    assert!(
        elements
            .iter()
            .all(|element| matches!(element, DataType::U8)),
        "fused directive payload parser must consume raw U8 source buffers, got {elements:?}"
    );
}

fn assert_condition_evaluator_byte_buffers_are_u8(dispatcher: &CountingDispatcher) {
    let source_elements = dispatcher.condition_source_elements.borrow();
    let macro_name_elements = dispatcher.condition_macro_name_elements.borrow();
    assert!(
        !source_elements.is_empty(),
        "directive payload extraction must dispatch conditional evaluators"
    );
    assert!(
        !macro_name_elements.is_empty(),
        "directive payload extraction must expose conditional macro-name buffers"
    );
    assert!(
        source_elements
            .iter()
            .all(|element| matches!(element, DataType::U8)),
        "conditional directive evaluators must consume raw U8 source buffers, got {source_elements:?}"
    );
    assert!(
        macro_name_elements
            .iter()
            .all(|element| matches!(element, DataType::U8)),
        "conditional directive evaluators must consume raw U8 macro-name buffers, got {macro_name_elements:?}"
    );
}

fn directive_payloads(src: &[u8], macros: &[&[u8]]) -> Vec<DirectivePayload> {
    run(src, macros)
        .into_iter()
        .filter(|p| !matches!(p, DirectivePayload::None))
        .collect()
}

#[test]
fn define_extracts_name_and_body() {
    let p = directive_payloads(b"#define FOO 42\n", &[]);
    assert_eq!(p.len(), 1);
    match &p[0] {
        DirectivePayload::Define {
            name,
            body,
            is_function_like,
            ..
        } => {
            assert_eq!(name, b"FOO");
            assert_eq!(body, b"42");
            assert!(!is_function_like);
        }
        other => panic!("expected Define, got {other:?}"),
    }
}

#[test]
fn function_like_define_carries_args_and_is_func_flag() {
    let p = directive_payloads(b"#define MAX(a,b) ((a)>(b)?(a):(b))\n", &[]);
    match &p[0] {
        DirectivePayload::Define {
            name,
            args,
            body,
            is_function_like,
            ..
        } => {
            assert_eq!(name, b"MAX");
            assert_eq!(args, b"a,b");
            assert_eq!(body, b"((a)>(b)?(a):(b))");
            assert!(is_function_like);
        }
        other => panic!("expected Define, got {other:?}"),
    }
}

#[test]
fn include_system_form() {
    let p = directive_payloads(b"#include <stdio.h>\n", &[]);
    match &p[0] {
        DirectivePayload::Include {
            path,
            is_system,
            is_next,
        } => {
            assert_eq!(path, b"stdio.h");
            assert!(is_system);
            assert!(!is_next);
        }
        other => panic!("expected Include, got {other:?}"),
    }
}

#[test]
fn include_local_form() {
    let p = directive_payloads(b"#include \"foo.h\"\n", &[]);
    match &p[0] {
        DirectivePayload::Include {
            path,
            is_system,
            is_next,
        } => {
            assert_eq!(path, b"foo.h");
            assert!(!is_system);
            assert!(!is_next);
        }
        other => panic!("expected Include, got {other:?}"),
    }
}

#[test]
fn include_next_carries_is_next_flag() {
    let p = directive_payloads(b"#include_next <stdio.h>\n", &[]);
    match &p[0] {
        DirectivePayload::Include { is_next, .. } => assert!(is_next),
        other => panic!("expected Include, got {other:?}"),
    }
}

#[test]
fn ifdef_resolves_to_one_when_macro_defined() {
    let p = directive_payloads(b"#ifdef FOO\n", &[b"FOO"]);
    match &p[0] {
        DirectivePayload::Ifdef { value, negated } => {
            assert_eq!(*value, 1);
            assert!(!negated);
        }
        other => panic!("expected Ifdef, got {other:?}"),
    }
}

#[test]
fn ifndef_carries_negated_flag() {
    let p = directive_payloads(b"#ifndef MISSING\n", &[]);
    match &p[0] {
        DirectivePayload::Ifdef { value, negated } => {
            assert_eq!(*value, 1, "MISSING is undefined → ifndef value=1");
            assert!(negated);
        }
        other => panic!("expected Ifdef, got {other:?}"),
    }
}

#[test]
fn if_expr_evaluates_against_macro_table() {
    let p = directive_payloads(b"#if defined(FOO) && 1\n", &[b"FOO"]);
    match &p[0] {
        DirectivePayload::IfExpr { value, is_elif } => {
            assert_eq!(*value, 1);
            assert!(!is_elif);
        }
        other => panic!("expected IfExpr, got {other:?}"),
    }
}

#[test]
fn elif_carries_is_elif_flag() {
    let p = directive_payloads(b"#if 0\n#elif 1\n", &[]);
    assert_eq!(p.len(), 2);
    match &p[1] {
        DirectivePayload::IfExpr { value, is_elif } => {
            assert_eq!(*value, 1);
            assert!(is_elif);
        }
        other => panic!("expected IfExpr, got {other:?}"),
    }
}

#[test]
fn else_endif_carry_no_payload() {
    let p = directive_payloads(b"#if 1\n#else\n#endif\n", &[]);
    assert!(matches!(p[1], DirectivePayload::Else));
    assert!(matches!(p[2], DirectivePayload::Endif));
}

#[test]
fn pragma_classifies_as_other() {
    let p = directive_payloads(b"#pragma once\n", &[]);
    assert!(matches!(p[0], DirectivePayload::Other));
}

#[test]
fn undef_extracts_macro_name() {
    // gpu_undef_parse extracts the name span; the payload carries
    // those bytes. Empty name reserved for unparseable rows.
    let p = directive_payloads(b"#undef X\n", &[]);
    match &p[0] {
        DirectivePayload::Undef { name } => assert_eq!(name.as_slice(), b"X"),
        other => panic!("expected Undef payload, got {other:?}"),
    }
    let p2 = directive_payloads(b"#undef _LONGER_NAME_42\n", &[]);
    match &p2[0] {
        DirectivePayload::Undef { name } => assert_eq!(name.as_slice(), b"_LONGER_NAME_42"),
        other => panic!("expected Undef payload, got {other:?}"),
    }
}

#[test]
fn fused_define_include_undef_parse_consumes_raw_u8_source() {
    let dispatcher = CountingDispatcher::new();
    let classified = gpu_tokenize_and_classify(
        &dispatcher,
        b"#define FOO 42\n#include <stdio.h>\n#undef FOO\n",
    )
    .expect("tokenize_and_classify");
    let payloads =
        gpu_extract_directive_payloads(&dispatcher, &classified, &[]).expect("payload extraction");
    assert!(payloads
        .iter()
        .any(|payload| matches!(payload, DirectivePayload::Define { .. })));
    assert!(payloads
        .iter()
        .any(|payload| matches!(payload, DirectivePayload::Include { .. })));
    assert!(payloads
        .iter()
        .any(|payload| matches!(payload, DirectivePayload::Undef { .. })));
    assert_fused_payload_parse_source_is_u8(&dispatcher);
}

#[test]
fn condition_evaluators_consume_raw_u8_byte_buffers() {
    let dispatcher = CountingDispatcher::new();
    let classified = gpu_tokenize_and_classify(&dispatcher, b"#ifdef FOO\n#if defined(FOO) && 1\n")
        .expect("tokenize_and_classify");
    let payloads = gpu_extract_directive_payloads(&dispatcher, &classified, &[b"FOO"])
        .expect("payload extraction");
    assert!(payloads
        .iter()
        .any(|payload| matches!(payload, DirectivePayload::Ifdef { value: 1, .. })));
    assert!(payloads
        .iter()
        .any(|payload| matches!(payload, DirectivePayload::IfExpr { value: 1, .. })));
    assert_condition_evaluator_byte_buffers_are_u8(&dispatcher);
}

#[test]
fn dense_real_world_header_block() {
    let src = b"#ifndef GUARD\n#define GUARD\n#include <stdio.h>\n#define MAX(a,b) ((a)>(b)?(a):(b))\n#if defined(__GNUC__)\n#endif\n#endif\n";
    let p = directive_payloads(src, &[b"__GNUC__"]);
    assert_eq!(p.len(), 7);
    assert!(matches!(
        &p[0],
        DirectivePayload::Ifdef {
            negated: true,
            value: 1
        }
    ));
    assert!(matches!(&p[1], DirectivePayload::Define { .. }));
    assert!(matches!(&p[2], DirectivePayload::Include { .. }));
    assert!(matches!(
        &p[3],
        DirectivePayload::Define {
            is_function_like: true,
            ..
        }
    ));
    assert!(matches!(&p[4], DirectivePayload::IfExpr { value: 1, .. }));
    assert!(matches!(&p[5], DirectivePayload::Endif));
    assert!(matches!(&p[6], DirectivePayload::Endif));
}

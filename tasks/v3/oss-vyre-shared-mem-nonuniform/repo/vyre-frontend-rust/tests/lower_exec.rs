//! End-to-end driver smoke test: `compile_unit` with `lower: true` produces a
//! Vyre `Program` that executes on the reference interpreter with the expected
//! result. Proves the full frontend path lex -> parse -> resolve -> typeck ->
//! borrow -> lower -> execute through the public driver API.

#![forbid(unsafe_code)]

use vyre_frontend_rust::pipeline::{RustPipeline, RustPipelineConfig};
use vyre_reference::value::Value;

fn run(src: &str, inputs: &[i32]) -> i32 {
    let config = RustPipelineConfig {
        gpu_lex: false,
        borrow_check: true,
        lower: true,
        lower_lane_count: None,
    };
    let unit = RustPipeline::new(config)
        .compile_unit(src.as_bytes())
        .expect("Fix: a well-formed nano program must compile through the full pipeline");
    let program = unit
        .program
        .expect("Fix: lower:true must produce a Program");
    let values: Vec<Value> = inputs.iter().map(|&x| Value::I32(x)).collect();
    let out = vyre_reference::reference_eval(&program, &values).expect("reference_eval");
    match &out[0] {
        Value::I32(x) => *x,
        Value::U32(x) => *x as i32,
        Value::Bytes(b) => i32::from_le_bytes(b[..4].try_into().expect("4 bytes")),
        other => panic!("unexpected output {other:?}"),
    }
}

fn i32s_to_bytes(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn bytes_to_i32s(bytes: &[u8]) -> Vec<i32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| i32::from_le_bytes(chunk.try_into().expect("i32 chunk")))
        .collect()
}

fn run_batched(src: &str, inputs: &[i32]) -> Vec<i32> {
    let config = RustPipelineConfig {
        gpu_lex: false,
        borrow_check: true,
        lower: true,
        lower_lane_count: Some(inputs.len() as u32),
    };
    let unit = RustPipeline::new(config)
        .compile_unit(src.as_bytes())
        .expect("Fix: a well-formed nano program must compile to batched IR");
    let program = unit
        .program
        .expect("Fix: lower:true must produce a batched Program");
    assert!(
        program
            .buffers()
            .iter()
            .all(|buffer| buffer.count() == inputs.len() as u32),
        "batched pipeline lowering must size every parameter/output buffer to the requested lane count"
    );
    let out = vyre_reference::reference_eval(&program, &[Value::from(i32s_to_bytes(inputs))])
        .expect("reference_eval");
    match &out[0] {
        Value::Bytes(bytes) => bytes_to_i32s(bytes),
        other => panic!("unexpected batched output {other:?}"),
    }
}

#[test]
fn compile_unit_lowers_and_executes_arithmetic() {
    assert_eq!(
        run("fn f(a: i32, b: i32) -> i32 { return a + b * 2; }", &[3, 4]),
        11
    );
}

#[test]
fn compile_unit_lowers_and_executes_branch_and_call() {
    let src = "fn g(a: i32) -> i32 { return a + 1; } \
               fn f(a: i32, b: i32) -> i32 { if a < b { return g(b); } else { return a; } }";
    assert_eq!(run(src, &[2, 5]), 6);
    assert_eq!(run(src, &[9, 5]), 9);
}

#[test]
fn compile_unit_lowers_and_executes_refs() {
    assert_eq!(
        run(
            "fn f(a: i32) -> i32 { let r: &i32 = &a; return *r + 1; }",
            &[6]
        ),
        7
    );
}

#[test]
fn compile_unit_lowers_and_executes_batched_range_loop() {
    let src = "\
fn f(n: i32) -> i32 {
    let mut acc: i32 = 0;
    for i in -8..n {
        if i < 0 {
            acc += i * 2;
        } else {
            acc += i - 1;
        };
    }
    return acc;
}";
    let inputs: Vec<i32> = (0..257).map(|lane| (lane * 7 % 31) - 10).collect();
    let got = run_batched(src, &inputs);
    let expected: Vec<i32> = inputs.iter().copied().map(cpu_range_loop).collect();
    assert_eq!(
        got, expected,
        "public RustPipeline batched lowering must execute scalar source semantics independently per lane"
    );
}

#[test]
fn compile_unit_rejects_zero_lane_batched_lowering() {
    let config = RustPipelineConfig {
        gpu_lex: false,
        borrow_check: true,
        lower: true,
        lower_lane_count: Some(0),
    };
    let error = RustPipeline::new(config)
        .compile_unit(b"fn f(n: i32) -> i32 { return n; }")
        .expect_err("Fix: zero-lane batched lowering must fail loudly");
    assert!(
        error
            .to_string()
            .contains("batched Rust lowering with zero lanes"),
        "got {error}"
    );
}

fn cpu_range_loop(n: i32) -> i32 {
    let mut acc = 0_i32;
    for i in -8..n {
        if i < 0 {
            acc = acc.wrapping_add(i.wrapping_mul(2));
        } else {
            acc = acc.wrapping_add(i.wrapping_sub(1));
        }
    }
    acc
}

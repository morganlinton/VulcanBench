//! Live CUDA coverage for C preprocessor directive payload extraction.
//!
//! This drives the real tokenization and directive-payload pipeline through
//! raw U8 source buffers, including the fused define/include/undef parse path.

#![cfg(test)]

mod common;

use common::with_live_backend;
use vyre::ir::Program;
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_extract_directive_payloads, gpu_tokenize_and_classify, DirectivePayload, GpuDispatcher,
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

struct CudaPayloadDispatcher<'a>(&'a CudaBackend);

impl GpuDispatcher for CudaPayloadDispatcher<'_> {
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

fn payloads(
    dispatcher: &dyn GpuDispatcher,
    source: &[u8],
    macros: &[&[u8]],
) -> Vec<DirectivePayload> {
    let classified = gpu_tokenize_and_classify(dispatcher, source)
        .unwrap_or_else(|error| panic!("Fix: C payload tokenization failed: {error}"));
    gpu_extract_directive_payloads(dispatcher, &classified, macros)
        .unwrap_or_else(|error| panic!("Fix: C payload extraction failed: {error}"))
}

fn meaningful_payload_count(payloads: &[DirectivePayload]) -> usize {
    payloads
        .iter()
        .filter(|payload| !matches!(payload, DirectivePayload::None))
        .count()
}

#[test]
fn cuda_c_preprocess_payloads_match_reference() {
    with_live_backend("c preprocess directive payloads", |backend| {
        let cuda_dispatcher = CudaPayloadDispatcher(backend);
        let reference_dispatcher = RefDispatcher;
        let source = br#"
#define FOO 42
#define MAX(a,b) ((a)>(b)?(a):(b))
#include <stdio.h>
#include_next <linux/compiler.h>
#undef FOO
#ifdef ENABLED
#endif
#ifndef MISSING
#endif
#if defined(ENABLED) && (3 + 4) > 1
#elif 0
#else
#endif
"#;
        let macros: [&[u8]; 1] = [b"ENABLED"];
        let expected = payloads(&reference_dispatcher, source, &macros);
        let actual = payloads(&cuda_dispatcher, source, &macros);
        assert_eq!(
            actual, expected,
            "Fix: CUDA directive payload extraction must match reference output byte-for-byte."
        );
        assert!(
            meaningful_payload_count(&actual) >= 12,
            "Fix: CUDA payload test must cover define, include, undef, ifdef, ifndef, if, elif, else, and endif rows."
        );
    });
}

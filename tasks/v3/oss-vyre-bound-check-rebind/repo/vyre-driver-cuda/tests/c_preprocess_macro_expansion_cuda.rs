//! Live CUDA coverage for GPU-resident C macro expansion.

#![cfg(test)]

mod common;

use std::path::{Path, PathBuf};

use common::with_live_backend;
use vyre::ir::Program;
use vyre::DispatchConfig;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_preprocess_translation_unit, GpuDispatcher, IncludeLoader,
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

struct EmptyLoader;

impl IncludeLoader for EmptyLoader {
    fn load(
        &self,
        _path: &[u8],
        _is_system: bool,
        _is_next: bool,
        _from: &Path,
    ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
        Ok(None)
    }
}

struct CudaPreprocessDispatcher<'a>(&'a vyre_driver_cuda::CudaBackend);

impl GpuDispatcher for CudaPreprocessDispatcher<'_> {
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

#[test]
fn cuda_c_preprocess_macro_expansion_matches_reference() {
    with_live_backend("c preprocess macro expansion", |backend| {
        let source = b"#define OBJ 123\n#define FN(x) x\nint a = OBJ + FN(alpha);\n";
        let loader = EmptyLoader;
        let expected = gpu_preprocess_translation_unit(
            &RefDispatcher,
            &loader,
            Path::new("<macro-ref>"),
            source,
            &[],
        )
        .expect("reference macro expansion must succeed");
        let actual = gpu_preprocess_translation_unit(
            &CudaPreprocessDispatcher(backend),
            &loader,
            Path::new("<macro-cuda>"),
            source,
            &[],
        )
        .expect("CUDA macro expansion must succeed");

        assert_eq!(
            actual.bytes, expected.bytes,
            "Fix: CUDA materialized macro expansion must match reference raw-U8 source/name/replacement byte arenas."
        );
        let out = String::from_utf8_lossy(&actual.bytes);
        assert!(
            out.contains("123") && out.contains("alpha") && !out.contains("OBJ"),
            "Fix: CUDA macro expansion must replace object and function-like macros; got {out:?}"
        );
    });
}

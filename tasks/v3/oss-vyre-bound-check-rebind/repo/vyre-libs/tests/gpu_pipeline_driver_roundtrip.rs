//! End-to-end test of `gpu_preprocess_translation_unit`: the full
//! recursive include driver. Drives the entire 18a→18b→18c→18d chain
//! through the reference dispatcher, with an in-memory `IncludeLoader`
//! so we don't touch the filesystem.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use vyre::ir::{BufferAccess, DataType, Program};
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_preprocess_translation_unit, GpuDispatcher, IncludeEventResidency, IncludeLoader, MacroDef,
};

#[path = "gpu_pipeline_driver_roundtrip/conditionals.rs"]
mod conditionals;
#[path = "gpu_pipeline_driver_roundtrip/includes.rs"]
mod includes;
#[path = "gpu_pipeline_driver_roundtrip/macros.rs"]
mod macros;

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
    dispatches: Cell<usize>,
    macro_byte_arena_elements: std::cell::RefCell<Vec<(String, DataType)>>,
    macro_byte_arena_input_lens: std::cell::RefCell<Vec<(String, usize)>>,
}

impl CountingDispatcher {
    fn new() -> Self {
        Self {
            dispatches: Cell::new(0),
            macro_byte_arena_elements: std::cell::RefCell::new(Vec::new()),
            macro_byte_arena_input_lens: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn dispatches(&self) -> usize {
        self.dispatches.get()
    }

    fn macro_byte_arena_input_lens(&self, name: &str) -> Vec<usize> {
        self.macro_byte_arena_input_lens
            .borrow()
            .iter()
            .filter_map(|(buffer, len)| (buffer == name).then_some(*len))
            .collect()
    }
}

impl GpuDispatcher for CountingDispatcher {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        self.dispatches.set(self.dispatches.get() + 1);
        if program
            .entry_op_id
            .as_deref()
            .is_some_and(|op_id| op_id.contains("opt_named_macro_expansion_materialized"))
        {
            for name in [
                "source_words",
                "macro_name_words",
                "macro_replacement_words",
            ] {
                let buffer = program
                    .buffers()
                    .iter()
                    .find(|buffer| buffer.name() == name)
                    .ok_or_else(|| format!("missing materialized macro byte arena {name}"))?;
                self.macro_byte_arena_elements
                    .borrow_mut()
                    .push((name.to_string(), buffer.element()));
                let input_index = input_index_for_buffer(program, name)
                    .ok_or_else(|| format!("missing materialized macro input slot {name}"))?;
                let len = inputs.get(input_index).map(Vec::len).ok_or_else(|| {
                    format!(
                        "missing materialized macro input {name} at slot {input_index}; got {} inputs",
                        inputs.len()
                    )
                })?;
                self.macro_byte_arena_input_lens
                    .borrow_mut()
                    .push((name.to_string(), len));
            }
        }
        RefDispatcher.dispatch(program, inputs)
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}

fn input_index_for_buffer(program: &Program, name: &str) -> Option<usize> {
    let mut input_index = 0usize;
    for buffer in program.buffers() {
        if buffer.access() == BufferAccess::Workgroup {
            continue;
        }
        if buffer.name() == name {
            return Some(input_index);
        }
        input_index += 1;
    }
    None
}

/// In-memory include loader keyed by exact path bytes.
struct MemLoader {
    files: HashMap<Vec<u8>, Vec<u8>>,
    loads: Cell<usize>,
}

impl MemLoader {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
            loads: Cell::new(0),
        }
    }
    fn add(&mut self, name: &[u8], bytes: &[u8]) -> &mut Self {
        self.files.insert(name.to_vec().into(), bytes.to_vec());
        self
    }
    fn loads(&self) -> usize {
        self.loads.get()
    }
}

impl IncludeLoader for MemLoader {
    fn load(
        &self,
        path: &[u8],
        _is_system: bool,
        _is_next: bool,
        _from: &Path,
    ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
        self.loads.set(self.loads.get() + 1);
        Ok(self.files.get(path).map(|b| {
            (
                PathBuf::from(String::from_utf8_lossy(path).into_owned()),
                b.clone().into(),
            )
        }))
    }
}

fn run(src: &[u8], cli: &[MacroDef], loader: &MemLoader) -> Vec<u8> {
    gpu_preprocess_translation_unit(&RefDispatcher, loader, Path::new("<tu>"), src, cli)
        .expect("preprocess_translation_unit")
        .bytes
}

fn run_err(src: &[u8], cli: &[MacroDef], loader: &MemLoader) -> String {
    match gpu_preprocess_translation_unit(&RefDispatcher, loader, Path::new("<tu>"), src, cli) {
        Ok(_) => panic!("preprocess_translation_unit must reject malformed input"),
        Err(error) => error,
    }
}

#[test]
fn no_directives_passes_through_active_bytes() {
    let loader = MemLoader::new();
    let out = run(b"int x = 1;", &[], &loader);
    // Filtered + tokenized + reassembled.
    let out_str = String::from_utf8_lossy(&out);
    assert!(out_str.contains("int"));
    assert!(out_str.contains("x"));
    assert!(out_str.contains("1"));
}

#[test]
fn large_no_directive_unit_uses_multi_block_sparse_token_scan() {
    let loader = MemLoader::new();
    let mut src = Vec::new();
    for i in 0..48u32 {
        src.extend_from_slice(format!("int large_token_scan_{i} = {i};\n").as_bytes());
    }
    assert!(
        src.len() > 1024,
        "fixture must exceed one prefix-scan block"
    );
    let out = run(&src, &[], &loader);
    let out_str = String::from_utf8_lossy(&out);
    assert!(out_str.contains("large_token_scan_0"));
    assert!(out_str.contains("large_token_scan_47"));
}

#[test]
fn clean_translation_unit_uses_gpu_preprocessor_path() {
    let loader = MemLoader::new();
    let dispatcher = CountingDispatcher::new();
    let source = format!(
        "int already_clean_{} = 42;\nfloat also_clean = 1.0f;\n",
        std::process::id()
    );
    let path = PathBuf::from(format!("<clean-tu-{}>", std::process::id()));
    let out = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, source.as_bytes(), &[])
        .expect("preprocessor-clean source must still use GPU preprocessing stages");
    assert_eq!(out.bytes, source.as_bytes());
    assert!(
        dispatcher.dispatches() > 0,
        "clean translation units must not bypass GPU preprocessing"
    );
}

#[test]
fn line_comment_is_dropped() {
    let loader = MemLoader::new();
    let out = run(b"int x;// comment here\nint y;", &[], &loader);
    let s = String::from_utf8_lossy(&out);
    assert!(!s.contains("comment"));
    assert!(s.contains("int"));
}

#[test]
fn block_comment_is_dropped() {
    let loader = MemLoader::new();
    let out = run(b"int /* drop */ x;", &[], &loader);
    let s = String::from_utf8_lossy(&out);
    assert!(!s.contains("drop"));
}

#[test]
fn line_splice_joins_lines() {
    let loader = MemLoader::new();
    let out = run(b"int x = 1 + \\\n2;", &[], &loader);
    let s = String::from_utf8_lossy(&out);
    // The backslash-newline should be gone; tokens 1 + 2 should
    // appear as part of one expression.
    assert!(!s.contains("\\\n"));
}

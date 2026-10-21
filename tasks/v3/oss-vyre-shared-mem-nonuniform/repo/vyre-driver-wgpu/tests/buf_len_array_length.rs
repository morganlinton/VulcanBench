//! Q3 reproducer: `Expr::buf_len(buffer)` lowers to `naga::ArrayLength`
//! on the wgpu/Vulkan path. ArrayLength must equal the bound storage
//! buffer's element count at dispatch time. The cat_a_gpu_differential
//! pass on 2026-05-02 surfaced a regression where the unbounded
//! `vyre-primitives::hash::fnv1a64` registration (loop bound = buf_len)
//! caused the GPU loop to run zero iterations, returning the unchanged
//! FNV1A64_OFFSET.
//!
//! These tests build the smallest possible Program that exercises
//! `Expr::buf_len` at runtime and assert that the dispatched output
//! reflects the actual bound buffer length. They are written to fail
//! before a Q3 fix lands and pass after, so the workaround in
//! `vyre_primitives::hash::fnv1a` (using `fnv1a64_program_n` instead of
//! `fnv1a64_program`) can be reverted with confidence.
//!
//! Lane: `driver_wgpu` (per `docs/optimization/OWNERSHIP.toml`).

use std::sync::{Arc, OnceLock};

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        WgpuBackend::acquire().expect(
            "Fix: GPU adapter required for buf_len_array_length tests. Run on a host with a working wgpu adapter.",
        )
    })
}

/// Build a Program whose body writes `buf_len(input)` to `out[0]`.
/// `input` is declared without a static count, so the lowering uses
/// `naga::ArrayLength` to read the bound buffer's element count at
/// runtime. `out` is one u32 with explicit count = 1.

fn dispatch_and_read_first_word(program: &Program, input_bytes: Vec<u8>) -> u32 {
    dispatch_and_read_first_word_with_lowering(program, input_bytes, false)
}

/// Like [`dispatch_and_read_first_word`] but routes the program through
/// the same `vyre_foundation::optimizer::pre_lowering::optimize` pass
/// that `cat_a_gpu_differential::lower_for_gpu` uses. The catalog
/// failure cases hit that path; pure direct dispatch does not.
fn dispatch_and_read_first_word_lowered(program: &Program, input_bytes: Vec<u8>) -> u32 {
    dispatch_and_read_first_word_with_lowering(program, input_bytes, true)
}

fn dispatch_and_read_first_word_with_lowering(
    program: &Program,
    input_bytes: Vec<u8>,
    lower: bool,
) -> u32 {
    let lowered;
    let prog = if lower {
        lowered = vyre_foundation::optimizer::pre_lowering::optimize(program.clone());
        &lowered
    } else {
        program
    };
    let inputs = vec![input_bytes, vec![0u8; 4]];
    let outputs = backend()
        .dispatch(prog, &inputs, &DispatchConfig::default())
        .expect("Fix: backend.dispatch must succeed for the buf_len writer program");
    let raw = &outputs[0];
    assert!(
        raw.len() >= 4,
        "Fix: output buffer too small to read a u32 result"
    );
    u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])
}

fn dispatch_and_read_words(program: &Program, input_bytes: Vec<u8>) -> Vec<u32> {
    let inputs = vec![input_bytes, vec![0u8; 16]];
    let outputs = backend()
        .dispatch(program, &inputs, &DispatchConfig::default())
        .expect("Fix: backend.dispatch must succeed for the word writer program");
    outputs[0]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn dispatch_and_read_words_with_inputs(program: &Program, inputs: Vec<Vec<u8>>) -> Vec<u32> {
    let outputs = backend()
        .dispatch(program, &inputs, &DispatchConfig::default())
        .expect("Fix: backend.dispatch must succeed for the word writer program");
    outputs[0]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn u32_bytes(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[path = "buf_len_array_length/basic_len_contracts.rs"]
mod basic_len_contracts;
#[path = "buf_len_array_length/dynamic_pack_contracts.rs"]
mod dynamic_pack_contracts;
#[path = "buf_len_array_length/scatter_contracts.rs"]
mod scatter_contracts;
#[path = "buf_len_array_length/region_loop_contracts.rs"]
mod region_loop_contracts;
#[path = "buf_len_array_length/fnv_loop_contracts.rs"]
mod fnv_loop_contracts;

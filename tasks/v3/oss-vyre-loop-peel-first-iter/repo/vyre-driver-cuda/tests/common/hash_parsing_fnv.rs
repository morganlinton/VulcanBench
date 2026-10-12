use crate::common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::ir::{BufferAccess, DataType, Program};
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_primitives::hash::fnv1a::{
    fnv1a32, fnv1a32_program, fnv1a32_program_u8, fnv1a64, fnv1a64_program_n_u8,
};

fn run_fnv1a32(backend: &CudaBackend, bytes: &[u8]) -> u32 {
    let words: Vec<u32> = bytes.iter().map(|b| *b as u32).collect();
    let n = words.len() as u32;
    let program = fnv1a32_program("input", "out", n);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(&words)];
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    bytes_u32(&outputs[0])[0]
}

fn fnv_inputs_for_program(program: &Program, bytes: &[u8]) -> Vec<Vec<u8>> {
    program
        .buffers()
        .iter()
        .filter_map(|buffer| {
            let backend_allocated = buffer.is_output() || buffer.is_pipeline_live_out();
            let needs_input = matches!(
                buffer.access(),
                BufferAccess::ReadOnly | BufferAccess::ReadWrite | BufferAccess::Uniform
            ) && !backend_allocated
                && buffer.access() != BufferAccess::Workgroup;
            if !needs_input {
                return None;
            }
            if buffer.name() == "input" {
                match buffer.element() {
                    DataType::U8 => Some(bytes.to_vec()),
                    DataType::U32 => {
                        let mut words: Vec<u32> =
                            bytes.iter().map(|byte| u32::from(*byte)).collect();
                        if words.is_empty() {
                            words.push(0);
                        }
                        Some(u32_bytes(&words))
                    }
                    other => panic!("Fix: CUDA FNV input must be U8 or U32, got {other:?}"),
                }
            } else {
                panic!("Fix: unexpected CUDA FNV input buffer `{}`", buffer.name())
            }
        })
        .collect()
}

fn output_index(program: &Program, name: &str) -> usize {
    program
        .buffers()
        .iter()
        .filter(|buffer| {
            buffer.is_output()
                || buffer.is_pipeline_live_out()
                || matches!(
                    buffer.access(),
                    BufferAccess::ReadWrite | BufferAccess::WriteOnly
                )
        })
        .position(|buffer| buffer.name() == name)
        .expect("Fix: CUDA FNV output buffer must be declared")
}

fn run_fnv1a32_program(
    backend: &CudaBackend,
    program: Program,
    bytes: &[u8],
    case_name: &str,
) -> u32 {
    let inputs = fnv_inputs_for_program(&program, bytes);
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let out_index = output_index(&program, "out");
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .unwrap_or_else(|error| panic!("Fix: CUDA {case_name} dispatch failed: {error}"));
    bytes_u32(&outputs[out_index])[0]
}

fn run_fnv1a32_u8(backend: &CudaBackend, bytes: &[u8]) -> u32 {
    run_fnv1a32_program(
        backend,
        fnv1a32_program_u8("input", "out", bytes.len() as u32),
        bytes,
        "packed-u8 FNV-1a32",
    )
}

fn run_fnv1a64_u8(backend: &CudaBackend, bytes: &[u8]) -> u64 {
    let program = fnv1a64_program_n_u8("input", "out", bytes.len() as u32);
    let inputs = fnv_inputs_for_program(&program, bytes);
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let out_index = output_index(&program, "out");
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .unwrap_or_else(|error| panic!("Fix: CUDA packed-u8 FNV-1a64 dispatch failed: {error}"));
    let words = bytes_u32(&outputs[out_index]);
    u64::from(words[0]) | (u64::from(words[1]) << 32)
}

#[test]
fn cuda_fnv1a32_empty_returns_offset_basis() {
    with_live_backend("cuda_fnv1a32_empty_returns_offset_basis", |backend| {
        let bytes: &[u8] = &[];
        let cpu = fnv1a32(bytes);
        let words = vec![0u32; 1];
        let n = 0u32;
        let program = fnv1a32_program("input", "out", n);
        let inputs: Vec<Vec<u8>> = vec![u32_bytes(&words)];
        let mut config = DispatchConfig::default();
        config.grid_override = Some([1, 1, 1]);
        let outputs = backend
            .dispatch(&program, &inputs, &config)
            .expect("dispatch");
        let gpu = bytes_u32(&outputs[0])[0];
        let gpu_u8 = run_fnv1a32_u8(backend, bytes);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu_u8, cpu);
        assert_eq!(gpu, 0x811c_9dc5);
    });
}

#[test]
fn cuda_fnv1a32_single_byte() {
    with_live_backend("cuda_fnv1a32_single_byte", |backend| {
        let bytes = b"a";
        let cpu = fnv1a32(bytes);
        let gpu = run_fnv1a32(backend, bytes);
        let gpu_u8 = run_fnv1a32_u8(backend, bytes);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu_u8, cpu);
    });
}

#[test]
fn cuda_fnv1a32_long_string() {
    with_live_backend("cuda_fnv1a32_long_string", |backend| {
        let bytes = b"the quick brown fox jumps over the lazy dog";
        let cpu = fnv1a32(bytes);
        let gpu = run_fnv1a32(backend, bytes);
        let gpu_u8 = run_fnv1a32_u8(backend, bytes);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu_u8, cpu);
    });
}

#[test]
fn cuda_fnv1a32_distinct_inputs_distinct_hashes() {
    with_live_backend("cuda_fnv1a32_distinct_inputs_distinct_hashes", |backend| {
        let a = run_fnv1a32(backend, b"abc");
        let b = run_fnv1a32(backend, b"abd");
        let a_u8 = run_fnv1a32_u8(backend, b"abc");
        let b_u8 = run_fnv1a32_u8(backend, b"abd");
        assert_ne!(a, b);
        assert_eq!(a, fnv1a32(b"abc"));
        assert_eq!(b, fnv1a32(b"abd"));
        assert_eq!(a_u8, a);
        assert_eq!(b_u8, b);
    });
}

#[test]
fn cuda_fnv1a_packed_u8_generated_matrix_matches_cpu() {
    with_live_backend(
        "cuda_fnv1a_packed_u8_generated_matrix_matches_cpu",
        |backend| {
            for case in 0..96u32 {
                let len = match case % 8 {
                    0 => 0,
                    1 => 1,
                    2 => 31,
                    3 => 32,
                    4 => 255,
                    5 => 256,
                    6 => 1023,
                    _ => 4099,
                };
                let bytes = generated_fnv_bytes(case, len);
                assert_eq!(
                    run_fnv1a32_u8(backend, &bytes),
                    fnv1a32(&bytes),
                    "Fix: CUDA packed-u8 FNV-1a32 mismatch on generated case {case}"
                );
                assert_eq!(
                    run_fnv1a64_u8(backend, &bytes),
                    fnv1a64(&bytes),
                    "Fix: CUDA packed-u8 FNV-1a64 mismatch on generated case {case}"
                );
            }
        },
    );
}

fn generated_fnv_bytes(seed: u32, len: usize) -> Vec<u8> {
    let mut state = u64::from(seed) ^ 0xd131_0ba6_98df_b5ac;
    let mut bytes = Vec::with_capacity(len);
    for index in 0..len {
        state ^= state << 7;
        state ^= state >> 9;
        state = state.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let byte = match (state
            .wrapping_add(index as u64)
            .wrapping_add(u64::from(seed)))
            % 19
        {
            0 => 0,
            1 => 0xFF,
            2 | 3 => b'_',
            4 => b'\n',
            5 => b'\t',
            _ => (state.rotate_left((index % 63) as u32) & 0xFF) as u8,
        };
        bytes.push(byte);
    }
    bytes
}

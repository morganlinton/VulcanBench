//! Hardware WGPU parity tests for C comment-strip masking.

#![allow(deprecated)]
use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_libs::parsing::c::preprocess::gpu_comment_strip_mask::{
    gpu_comment_strip_mask, reference_gpu_comment_strip_mask,
};

fn pack_source_bytes(source: &[u8]) -> Vec<u8> {
    let mut packed = source.to_vec();
    packed.resize((source.len().max(1).div_ceil(4) * 4).max(4), 0);
    packed
}

fn unpack_u32_prefix(bytes: &[u8], count: usize) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .take(count)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn run_gpu_comment_mask(source: &[u8]) -> Vec<u32> {
    let program = gpu_comment_strip_mask(source.len() as u32);
    let inputs = vec![
        pack_source_bytes(source),
        vec![0u8; source.len().max(1) * 4],
    ];
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: WGPU backend must acquire the local GPU for C comment-strip parity tests");
    let outputs = backend
        .dispatch(&program, &inputs, &DispatchConfig::default())
        .expect("Fix: gpu_comment_strip_mask must dispatch on the WGPU backend");
    let bytes = outputs
        .first()
        .expect("Fix: gpu_comment_strip_mask must return comment_mask_out");
    unpack_u32_prefix(bytes, source.len())
}

#[test]
fn wgpu_comment_strip_handles_realistic_c_snippet_on_device() {
    let source = b"// header guard\n#ifndef X\n#define X /* opaque */\nchar *s = \"/* not comment */\";\nint c = '/';\n#endif\n";
    assert_eq!(
        run_gpu_comment_mask(source),
        reference_gpu_comment_strip_mask(source)
    );
}

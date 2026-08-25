//! WGPU parity for the primitive-owned hex decoder.

#![allow(deprecated)]
mod common;
use common::acquire_live_backend as live_backend;
use common::bytes_u32;
use common::u32_bytes;

use vyre::{DispatchConfig, VyreBackend};
use vyre_primitives::decode::hex::{
    hex_decode, hex_decode_reference_packed, hex_decode_table, hex_decoded_capacity,
    HEX_WORKGROUP_SIZE,
};

fn ascii_lanes(input: &[u8]) -> Vec<u32> {
    input.iter().map(|byte| u32::from(*byte)).collect()
}

fn hex_lanes() -> u32 {
    HEX_WORKGROUP_SIZE[0]
}

fn dispatch_hex(input: &[u8]) -> Vec<u32> {
    let backend = live_backend();
    let decoded_words = hex_decoded_capacity(input.len() as u32);
    let program = hex_decode("input", "output", "table", input.len() as u32);
    let mut config = DispatchConfig::default();
    config.grid_override = Some([decoded_words.div_ceil(hex_lanes()).max(1), 1, 1]);
    let outputs = backend
        .dispatch(
            &program,
            &[
                u32_bytes(&ascii_lanes(input)),
                u32_bytes(hex_decode_table().as_ref()),
            ],
            &config,
        )
        .expect("Fix: WGPU hex_decode dispatch must succeed");
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(decoded_words as usize);
    gpu
}

#[test]
fn wgpu_hex_decode_matches_reference_for_mixed_case_and_invalid_nibbles() {
    let input = b"0001020a0FfF7gzzA5";
    assert_eq!(dispatch_hex(input), hex_decode_reference_packed(input));
}

#[test]
fn wgpu_hex_decode_crosses_workgroup_lanes() {
    let mut input = Vec::with_capacity((hex_lanes() as usize + 73) * 2);
    for idx in 0..(hex_lanes() as usize + 73) {
        let byte = ((idx * 37) ^ (idx >> 3)) as u8;
        input.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    assert_eq!(dispatch_hex(&input), hex_decode_reference_packed(&input));
}

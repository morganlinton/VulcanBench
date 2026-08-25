//! WGPU parity for the device-side bitset clear primitive.

mod common;
use common::acquire_live_backend as live_backend;
use common::bytes_u32;
use common::u32_bytes;

use vyre::{DispatchConfig, VyreBackend};
use vyre_primitives::bitset::zero::bitset_zero;

#[test]
fn wgpu_bitset_zero_parity_crosses_workgroup_lanes() {
    let backend = live_backend();
    let mut target = (0..600)
        .map(|idx| 0x5A5A_0000u32 ^ (idx as u32).wrapping_mul(17))
        .collect::<Vec<_>>();
    let program = bitset_zero("target", target.len() as u32);
    let mut config = DispatchConfig::default();
    config.grid_override = Some([3, 1, 1]);

    let outputs = backend
        .dispatch(&program, &[u32_bytes(&target)], &config)
        .expect("Fix: WGPU bitset_zero dispatch must succeed");

    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(target.len());
    target.fill(0);
    assert_eq!(gpu, target);
}

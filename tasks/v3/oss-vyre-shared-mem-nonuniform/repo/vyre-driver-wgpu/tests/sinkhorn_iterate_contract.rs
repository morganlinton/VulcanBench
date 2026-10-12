//! WGPU contract coverage for composed persistent Sinkhorn iteration.

mod common;
use common::u32_bytes;

use vyre_driver::VyreBackend;
use vyre_primitives::math::sinkhorn_iterate::sinkhorn_iterate;

#[test]
fn sinkhorn_iterate_matches_registered_fixture() {
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: WGPU sinkhorn contract requires a live GPU backend.");
    let program = sinkhorn_iterate(
        "k", "kt", "a", "b", "uc", "un", "v", "kv", "ktu", "c", 2, 2, 5,
    );
    let inputs = [
        u32_bytes(&[65_536, 65_536]),
        u32_bytes(&[0, 0]),
        u32_bytes(&[0]),
        u32_bytes(&[65_536, 65_536, 65_536, 65_536]),
        u32_bytes(&[65_536, 65_536, 65_536, 65_536]),
        u32_bytes(&[32_768, 32_768]),
        u32_bytes(&[32_768, 32_768]),
        u32_bytes(&[65_536, 65_536]),
        u32_bytes(&[0, 0]),
        u32_bytes(&[0, 0]),
    ];
    let borrowed = inputs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let outputs = backend
        .dispatch_borrowed(&program, &borrowed, &vyre_driver::DispatchConfig::default())
        .expect("Fix: WGPU must dispatch persistent sinkhorn.");
    assert_eq!(
        outputs,
        vec![
            u32_bytes(&[32_768, 32_768]),
            u32_bytes(&[32_768, 32_768]),
            u32_bytes(&[0]),
            u32_bytes(&[32_768, 32_768]),
            u32_bytes(&[0, 0]),
            u32_bytes(&[0, 0]),
        ]
    );
}

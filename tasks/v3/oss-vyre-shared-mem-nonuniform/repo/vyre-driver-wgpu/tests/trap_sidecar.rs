//! WGPU trap sidecar integration tests.

use vyre_driver::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_words;

#[test]
fn inflate_fixed_huffman_reports_wgpu_trap_tag() {
    let backend = WgpuBackend::acquire().expect("Fix: GPU required for WGPU trap sidecar test");
    let program = vyre_libs::decode::inflate("input", "output", 5);
    let input = pack_words(&[0x03, 0, 0, 0, 0]);
    let len_sidecar = vec![0u8; 4];

    let error = backend
        .dispatch(
            &program,
            &[input, len_sidecar],
            &vyre_driver::DispatchConfig::default(),
        )
        .expect_err("Fix: BTYPE=1 must propagate Node::Trap through WGPU.");
    let message = error.to_string();
    assert!(
        message.contains("wgpu dispatch trapped")
            && message.contains("fixed-Huffman")
            && message.contains("BTYPE=1"),
        "Fix: trap sidecar must report the original unsupported-block tag, got: {message}",
    );
}

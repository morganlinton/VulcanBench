//! Live external CUDA probe contract.
//!
//! Vyre's release path is CUDA-first on this fleet. The external NVIDIA probe
//! and the internal CUDA capability probe must both see a real GPU and produce
//! actionable diagnostics when the host is misconfigured.

use std::process::Command;

use vyre_driver_cuda::CudaDeviceCaps;

#[test]
fn nvidia_smi_and_cuda_probe_both_see_gpu_required_host() {
    let nvidia_smi = Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .expect("Fix: `nvidia-smi -L` must launch on the CUDA-required Vyre host.");

    assert!(
        nvidia_smi.status.success(),
        "Fix: `nvidia-smi -L` failed on the CUDA-required Vyre host.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&nvidia_smi.stdout),
        String::from_utf8_lossy(&nvidia_smi.stderr)
    );

    let listed = String::from_utf8_lossy(&nvidia_smi.stdout);
    let listed_gpus = listed
        .lines()
        .filter(|line| line.trim_start().starts_with("GPU "))
        .count();
    assert!(
        listed_gpus > 0,
        "Fix: `nvidia-smi -L` returned no GPU lines on the CUDA-required Vyre host. Output:\n{listed}"
    );

    let visible = CudaDeviceCaps::visible_device_count()
        .expect("Fix: CUDA driver probe must report visible devices on this GPU host.");
    assert!(
        visible > 0,
        "Fix: CUDA driver reported zero visible devices while `nvidia-smi -L` listed {listed_gpus} GPU(s). Repair CUDA_VISIBLE_DEVICES or container GPU passthrough."
    );

    let probed = CudaDeviceCaps::probe_all()
        .expect("Fix: CUDA capability probe must enumerate every visible device.");
    assert_eq!(
        probed.len(),
        visible,
        "Fix: CUDA probe_all must return exactly one capability record per CUDA-visible GPU."
    );
    for caps in &probed {
        assert!(
            !caps.name.trim().is_empty(),
            "Fix: CUDA device {} returned an empty device name.",
            caps.ordinal
        );
        assert!(
            caps.total_memory > 0,
            "Fix: CUDA device {} ({}) returned zero VRAM.",
            caps.ordinal,
            caps.name
        );
        assert!(
            caps.warp_size > 0,
            "Fix: CUDA device {} ({}) returned invalid warp size {}.",
            caps.ordinal,
            caps.name,
            caps.warp_size
        );
        assert!(
            caps.max_threads_per_block > 0,
            "Fix: CUDA device {} ({}) returned invalid max_threads_per_block {}.",
            caps.ordinal,
            caps.name,
            caps.max_threads_per_block
        );
    }
}

//! Public invariant tests for NVMe to GPU ingest telemetry.

use vyre_runtime::uring::{NativeReadPath, NvmeGpuIngestTelemetry};

fn completed_registered() -> NvmeGpuIngestTelemetry {
    NvmeGpuIngestTelemetry {
        submitted_bytes: 4096,
        completed_bytes: 4096,
        submitted_reads: 4,
        completed_reads: 4,
        registered_mapped_read_submissions: 4,
        gpudirect_nvme_submissions: 0,
        cpu_bounce_bytes: 0,
        failed_completions: 0,
    }
}

fn completed_gpudirect() -> NvmeGpuIngestTelemetry {
    NvmeGpuIngestTelemetry {
        submitted_bytes: 8192,
        completed_bytes: 8192,
        submitted_reads: 2,
        completed_reads: 2,
        registered_mapped_read_submissions: 0,
        gpudirect_nvme_submissions: 2,
        cpu_bounce_bytes: 0,
        failed_completions: 0,
    }
}

#[test]
fn completed_zero_copy_validation_accepts_each_native_path() {
    completed_registered()
        .validate_completed_zero_copy(NativeReadPath::RegisteredMappedRead)
        .expect("Fix: registered mapped read telemetry should validate.");
    completed_gpudirect()
        .validate_completed_zero_copy(NativeReadPath::GpuDirectNvmePassthrough)
        .expect("Fix: GPUDirect NVMe telemetry should validate.");
}

#[test]
fn completed_zero_copy_validation_rejects_cpu_bounce_bytes() {
    let mut telemetry = completed_registered();
    telemetry.cpu_bounce_bytes = 1;
    let err = telemetry
        .validate_completed_zero_copy(NativeReadPath::RegisteredMappedRead)
        .expect_err("CPU bounce bytes must fail zero-copy validation");
    assert!(
        err.to_string().contains("bounce"),
        "zero-copy rejection must mention bounce buffer: {err}"
    );
}

#[test]
fn completed_zero_copy_validation_rejects_failed_or_inflight_reads() {
    let mut failed = completed_registered();
    failed.failed_completions = 1;
    let failed_err = failed
        .validate_completed_zero_copy(NativeReadPath::RegisteredMappedRead)
        .expect_err("failed completions must fail validation");
    assert!(
        failed_err.to_string().contains("failed"),
        "failed-completion error: {failed_err}"
    );

    let mut inflight = completed_registered();
    inflight.completed_reads = 3;
    let inflight_err = inflight
        .validate_completed_zero_copy(NativeReadPath::RegisteredMappedRead)
        .expect_err("inflight reads must fail validation");
    assert!(
        inflight_err.to_string().contains("inflight"),
        "inflight-read error: {inflight_err}"
    );
}

#[test]
fn completed_zero_copy_validation_rejects_byte_and_read_mismatch() {
    let mut bytes = completed_registered();
    bytes.completed_bytes = 2048;
    let bytes_err = bytes
        .validate_completed_zero_copy(NativeReadPath::RegisteredMappedRead)
        .expect_err("byte mismatch must fail validation");
    assert!(
        bytes_err.to_string().contains("byte"),
        "byte-accounting error: {bytes_err}"
    );

    let mut reads = completed_registered();
    reads.completed_reads = 5;
    let reads_err = reads
        .validate_completed_zero_copy(NativeReadPath::RegisteredMappedRead)
        .expect_err("read mismatch must fail validation");
    assert!(
        reads_err.to_string().contains("read"),
        "read-accounting error: {reads_err}"
    );
}

#[test]
fn completed_zero_copy_validation_rejects_path_mixing() {
    let mut registered = completed_registered();
    registered.gpudirect_nvme_submissions = 1;
    let reg_err = registered
        .validate_completed_zero_copy(NativeReadPath::RegisteredMappedRead)
        .expect_err("mixed GPUDirect submissions on registered path");
    assert!(
        reg_err.to_string().contains("GPUDirect") || reg_err.to_string().contains("registered"),
        "path-mixing error (registered): {reg_err}"
    );

    let mut gpudirect = completed_gpudirect();
    gpudirect.registered_mapped_read_submissions = 1;
    let gd_err = gpudirect
        .validate_completed_zero_copy(NativeReadPath::GpuDirectNvmePassthrough)
        .expect_err("mixed registered submissions on GPUDirect path");
    assert!(
        gd_err.to_string().contains("registered") || gd_err.to_string().contains("GPUDirect"),
        "path-mixing error (gpudirect): {gd_err}"
    );
}

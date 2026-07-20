//! Linux ingest loop smoke/e2e: file -> io_uring -> mapped slot -> live GPU.

#![cfg(target_os = "linux")]
#![allow(unsafe_code)]

use core::sync::atomic::AtomicU32;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::time::{Duration, Instant};

use tempfile::tempdir;
use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_runtime::megakernel::MegakernelIoQueue;
use vyre_runtime::uring::{
    AsyncUringStream, GpuMappedBuffer, IoUringState, NativeReadPath, NvmeGpuIngestDriver,
};
use vyre_runtime::PipelineError;

const FILE_BYTES: usize = 4 * 1024 * 1024;
const HASH_WORDS: u32 = 8;

fn write_test_file() -> std::path::PathBuf {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("nvme-gpu-ingest.bin");
    let mut file = File::create(&path).expect("create test file");
    let pattern: Vec<u8> = (0..4096).map(|i| ((i * 17) & 0xFF) as u8).collect();
    let mut remaining = FILE_BYTES;
    while remaining > 0 {
        let chunk = remaining.min(pattern.len());
        file.write_all(&pattern[..chunk])
            .expect("write pattern chunk");
        remaining -= chunk;
    }
    file.flush().expect("flush test file");
    file.seek(SeekFrom::Start(0)).expect("rewind test file");
    // Keep the tempdir alive by leaking it for the process lifetime. This is
    // test-only and avoids threading the directory owner across helper calls.
    let leaked = dir.keep();
    leaked.join("nvme-gpu-ingest.bin")
}

fn make_driver() -> Result<NvmeGpuIngestDriver<'static>, PipelineError> {
    let ring = IoUringState::new(8)?;
    let target = Box::leak(vec![0u8; FILE_BYTES].into_boxed_slice());
    let gpu_buffer = unsafe { GpuMappedBuffer::from_host_visible_slice(target) };
    let tail = Box::leak(Box::new(AtomicU32::new(0)));
    let stream = AsyncUringStream::new(ring, gpu_buffer, tail);
    NvmeGpuIngestDriver::new(stream, 1, MegakernelIoQueue::new(64)?)
}

fn make_gpudirect_driver() -> Result<NvmeGpuIngestDriver<'static>, PipelineError> {
    let ring = IoUringState::new(8)?;
    let target = Box::leak(vec![0u8; FILE_BYTES].into_boxed_slice());
    // SAFETY: The test buffer is leaked for process lifetime and stands in for
    // BAR1-backed memory when the constructor is expected to reject missing
    // GPUDirect configuration before any native NVMe submission.
    let gpu_buffer = unsafe { GpuMappedBuffer::from_host_visible_slice(target) };
    let tail = Box::leak(Box::new(AtomicU32::new(0)));
    let stream = AsyncUringStream::new(ring, gpu_buffer, tail);
    NvmeGpuIngestDriver::new_gpudirect(stream, 1, MegakernelIoQueue::new(64)?)
}

fn copy_hash_program() -> Program {
    let idx = Expr::var("idx");
    Program::wrapped(
        vec![
            BufferDecl::storage("hash_in", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(HASH_WORDS),
            BufferDecl::output("hash_out", 1, DataType::U32).with_count(HASH_WORDS),
        ],
        [HASH_WORDS, 1, 1],
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(idx.clone(), Expr::u32(HASH_WORDS)),
                vec![Node::Store {
                    buffer: "hash_out".into(),
                    index: idx.clone(),
                    value: Expr::load("hash_in", idx),
                }],
            ),
        ],
    )
}

#[test]
fn ingests_file_and_surfaces_hash_through_live_backend() {
    let path = write_test_file();
    let mut driver = match make_driver() {
        Ok(driver) => driver,
        Err(PipelineError::IoUringSyscall { errno, .. })
            if errno == libc::EPERM || errno == libc::ENOSYS =>
        {
            panic!(
                "Fix: io_uring must be available on the local Linux host for I.3; \
                 EPERM/ENOSYS here is a configuration bug."
            );
        }
        Err(err) => panic!("unexpected driver setup failure: {err}"),
    };
    assert_eq!(
        driver.read_path(),
        NativeReadPath::RegisteredMappedRead,
        "plain file ingest is the compatibility mapped-read path; native NVMe must use new_gpudirect + submit_native_nvme_read"
    );

    driver.submit_file(&path, 0).expect("submit ingest");
    let deadline = Instant::now() + Duration::from_secs(15);
    let completed = loop {
        let completions = driver.poll_completions().expect("poll_completions");
        if let Some(done) = completions.into_iter().next() {
            break done;
        }
        assert!(
            Instant::now() < deadline,
            "ingest completion timed out after {:?}",
            Duration::from_secs(15)
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(completed.slot, 0);
    assert_eq!(completed.byte_count as usize, FILE_BYTES);
    assert!(
        driver.megakernel_io_queue().completion(0).is_some(),
        "io_queue slot 0 must be published to the megakernel after CQE completion"
    );

    let cpu_hash = blake3::hash(
        &std::fs::read(&path)
            .expect("Fix: the ingest fixture must remain readable for CPU hashing"),
    );
    let backend = WgpuBackend::acquire()
        .expect("Fix: the local RTX 5090 backend must be available for the ingest e2e test");
    let gpu_bytes = backend
        .dispatch(
            &copy_hash_program(),
            &[
                cpu_hash.as_bytes().to_vec(),
                vec![0_u8; cpu_hash.as_bytes().len()],
            ],
            &DispatchConfig::default(),
        )
        .expect("VYRE hash-copy dispatch")
        .into_iter()
        .next()
        .expect("hash-copy output buffer");
    assert_eq!(
        &gpu_bytes[..cpu_hash.as_bytes().len()],
        cpu_hash.as_bytes(),
        "live GPU output must round-trip the ingested file's BLAKE3 digest"
    );
}

#[test]
fn gpudirect_path_fails_loudly_when_native_nvme_is_not_configured() {
    match make_gpudirect_driver() {
        Ok(driver) => assert_eq!(
            driver.read_path(),
            NativeReadPath::GpuDirectNvmePassthrough,
            "Fix: new_gpudirect must construct only the native NVMe passthrough path."
        ),
        Err(PipelineError::NvmePassthroughDisabled) => {
            assert!(
                !cfg!(feature = "uring-cmd-nvme"),
                "Fix: uring-cmd-nvme builds must not report the feature-disabled error."
            );
        }
        Err(PipelineError::Backend(message)) => {
            assert!(
                message.contains("GPUDirect native read unavailable") && message.contains("Fix:"),
                "Fix: missing GPUDirect/nvidia-fs must be reported as an actionable native-path error, got: {message}"
            );
        }
        Err(PipelineError::IoUringSyscall { errno, .. })
            if errno == libc::EPERM || errno == libc::ENOSYS =>
        {
            panic!(
                "Fix: io_uring must be available for the native GPUDirect ingest probe; \
                 EPERM/ENOSYS is a host configuration bug."
            );
        }
        Err(error) => panic!("unexpected GPUDirect constructor failure: {error}"),
    }
}

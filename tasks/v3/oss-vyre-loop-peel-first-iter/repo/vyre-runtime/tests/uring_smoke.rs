//! Linux-only smoke test: drive `AsyncUringStream` against `/dev/zero`.
//!
//! The test fails loudly when `io_uring_setup` returns an error. Runtime
//! storage paths must not silently skip kernel integration coverage.
//!
//! The `GpuMappedBuffer` constructor's safety contract requires GPU
//! memory, but it is expressed purely over a host-visible pointer +
//! length. For this test we pass a heap buffer and never forward the
//! pointer to a real GPU  -  an explicit test-only relaxation documented
//! at the call site.

#![cfg(target_os = "linux")]
#![allow(unsafe_code)]

use core::sync::atomic::AtomicU32;
use std::fs::File;
use std::os::fd::AsRawFd;

use vyre_runtime::uring::{AsyncUringStream, GpuMappedBuffer, IoUringState, Iovec};
use vyre_runtime::PipelineError;

#[test]
fn reads_from_dev_zero_into_host_buffer() {
    const CHUNK: usize = 4096;

    let ring = match IoUringState::new(8) {
        Ok(r) => r,
        Err(PipelineError::IoUringSyscall { errno, .. })
            if errno == libc::EPERM || errno == libc::ENOSYS =>
        {
            panic!(
                "io_uring unavailable (errno {errno}). Fix: enable io_uring for this host or mark the runtime feature unavailable loudly before running this test."
            );
        }
        Err(e) => panic!("unexpected io_uring setup failure: {e}"),
    };

    let mut target = vec![0xAAu8; CHUNK];
    // SAFETY (test-only): GpuMappedBuffer's contract requires a
    // host-visible GPU allocation; here we pass a heap pointer. We
    // never hand this buffer to a GPU backend  -  the kernel is the
    // only consumer, and the kernel treats `iov_base` as plain
    // host-writable memory.
    let gpu_buffer = unsafe { GpuMappedBuffer::from_host_visible_slice(&mut target) };

    let tail = AtomicU32::new(0);
    // SAFETY (test-only): we keep `tail` alive for the duration of
    // `stream` via the local binding below.
    let mut stream = AsyncUringStream::new(ring, gpu_buffer, &tail);

    let dev_zero = File::open("/dev/zero").expect("open /dev/zero");
    let fd = dev_zero.as_raw_fd();
    let mut iovs = [Iovec {
        iov_base: core::ptr::null_mut(),
        iov_len: 0,
    }];

    // SAFETY: iovs lives until poll completes below (owned by this
    // test frame). fd is live. gpu_buffer was registered above.
    unsafe {
        stream
            .submit_read_to_gpu(fd, 0, CHUNK as u32, 0, &mut iovs)
            .expect("submit /dev/zero read");
    }

    assert_eq!(stream.inflight(), 1, "one read in flight after submit");

    // Poll with a deadline so the test cannot hang in any
    // environment where SQPOLL wake-up semantics differ. 5s is the
    // generous upper bound for a 4KiB /dev/zero read.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while stream.inflight() > 0 {
        let reaped = stream.poll().expect("poll must not error");
        if reaped > 0 {
            break;
        }
        if std::time::Instant::now() >= deadline {
            panic!(
                "io_uring /dev/zero completion did not arrive within 5s. Fix: inspect SQ/CQ wakeups and kernel io_uring restrictions."
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(stream.inflight(), 0, "completion must drain inflight");
    assert_eq!(
        tail.load(core::sync::atomic::Ordering::Acquire),
        1,
        "megakernel tail must advance once on success"
    );
    assert!(
        target.iter().all(|&b| b == 0),
        "dev/zero must zero-fill the target buffer"
    );
}

#[test]
fn empty_iovs_storage_returns_queue_full() {
    let ring = match IoUringState::new(8) {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut target = [0u8; 16];
    let gpu_buffer = unsafe { GpuMappedBuffer::from_host_visible_slice(&mut target) };
    let tail = AtomicU32::new(0);
    let mut stream = AsyncUringStream::new(ring, gpu_buffer, &tail);

    let mut empty: [Iovec; 0] = [];
    let err = unsafe {
        stream
            .submit_read_to_gpu(0, 0, 4, 0, &mut empty)
            .expect_err("empty iovs must be rejected")
    };
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn out_of_bounds_chunk_returns_queue_full() {
    let ring = match IoUringState::new(8) {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut target = [0u8; 16];
    let gpu_buffer = unsafe { GpuMappedBuffer::from_host_visible_slice(&mut target) };
    let tail = AtomicU32::new(0);
    let mut stream = AsyncUringStream::new(ring, gpu_buffer, &tail);

    let mut iovs = [Iovec {
        iov_base: core::ptr::null_mut(),
        iov_len: 0,
    }];
    // chunk_idx=4 * len=8 = 32 bytes > 16-byte buffer
    let err = unsafe {
        stream
            .submit_read_to_gpu(0, 0, 8, 4, &mut iovs)
            .expect_err("out-of-bounds chunk must be rejected")
    };
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

//! `AsyncUringStream` ingesting from a TCP socket.
//!
//! Demonstrates Innov 10: the same pipeline that reads from NVMe
//! reads from any fd the kernel can `readv` into host-visible memory.
//! The GPU is never aware of the source  -  that's the architectural
//! win. When the fd is `AF_XDP` or an RDMA UC queue, the story is
//! identical; TCP is the most portable surface to exercise in CI.

#![cfg(target_os = "linux")]
#![allow(unsafe_code)]

use core::sync::atomic::AtomicU32;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::thread;
use std::time::{Duration, Instant};

use vyre_runtime::uring::{AsyncUringStream, GpuMappedBuffer, IoUringState, Iovec};
use vyre_runtime::PipelineError;

#[test]
fn reads_from_tcp_socket_into_host_buffer() {
    const PAYLOAD: &[u8] = b"vyre-pipeline socket-ingest smoke payload 0123456789";
    const CHUNK: usize = 128;

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

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().unwrap();

    // Spawn the writer: accept + write the payload + close.
    let writer = thread::spawn(move || {
        let (mut srv, _) = listener.accept().unwrap();
        srv.write_all(PAYLOAD).unwrap();
    });

    let client = TcpStream::connect(addr).expect("connect loopback");
    let fd = client.as_raw_fd();

    let mut target = vec![0xAAu8; CHUNK];
    let gpu_buffer = unsafe { GpuMappedBuffer::from_host_visible_slice(&mut target) };
    let tail = AtomicU32::new(0);
    let mut stream = AsyncUringStream::new(ring, gpu_buffer, &tail);

    let mut iovs = [Iovec {
        iov_base: core::ptr::null_mut(),
        iov_len: 0,
    }];

    // SAFETY: iovs + target outlive the completion thanks to the
    // poll loop below. fd is live until end-of-test.
    unsafe {
        stream
            .submit_read_to_gpu(fd, 0, CHUNK as u32, 0, &mut iovs)
            .expect("submit socket read");
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    while stream.inflight() > 0 {
        let reaped = stream.poll().expect("poll");
        if reaped > 0 {
            break;
        }
        if Instant::now() >= deadline {
            writer.join().unwrap();
            panic!(
                "io_uring socket completion did not arrive within 5s. Fix: inspect SQ/CQ wakeups and kernel io_uring restrictions."
            );
        }
        thread::sleep(Duration::from_millis(5));
    }

    writer.join().unwrap();
    assert_eq!(&target[..PAYLOAD.len()], PAYLOAD);
}

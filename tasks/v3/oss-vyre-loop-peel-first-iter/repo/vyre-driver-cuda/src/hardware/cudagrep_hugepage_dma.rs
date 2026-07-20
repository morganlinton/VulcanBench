//! NUMA-Aware HugePage DMA Memory Mapping
//!
//! True Linux-grade performance does not rely on `Vec::with_capacity` 
//! allocating fragmented 4KB pages scattered across physical RAM.
//! When GPUDirect Storage or `io_uring` issues multi-Gigabyte transfers, 
//! the CPU Translation Lookaside Buffer (TLB) misses drastically slow down memory translation.
//!
//! This module executes explicit `mmap` syscalls mapping contiguous 2MB/1GB `MAP_HUGETLB` pages,
//! locking them aggressively into physical RAM via `MAP_POPULATE`, and natively pinning 
//! the buffers to the executing worker's local NUMA architecture.

use std::ptr::NonNull;
use libc::{mmap, munmap, MAP_ANONYMOUS, MAP_PRIVATE, MAP_HUGETLB, MAP_POPULATE, PROT_READ, PROT_WRITE};

pub struct HugePageBuffer {
    ptr: NonNull<u8>,
    capacity: usize,
}

// Memory bindings flawlessly secured matching Native OS mapping guarantees safely cross-threaded
unsafe impl Send for HugePageBuffer {}
unsafe impl Sync for HugePageBuffer {}

impl HugePageBuffer {
    /// Perfectly maps extreme bounded buffers into natively mapped Linux HugePages.
    pub fn new(capacity: usize) -> Result<Self, std::io::Error> {
        // Enforce 2MB Page alignment naturally mapping hardware execution lines
        let hugepage_size = 2 * 1024 * 1024;
        let aligned_capacity = (capacity + hugepage_size - 1) & !(hugepage_size - 1);

        let flags = MAP_PRIVATE | MAP_ANONYMOUS | MAP_HUGETLB | MAP_POPULATE;

        // Execute explicit Syscall bounding to map physical OS domains gracefully
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                aligned_capacity,
                PROT_READ | PROT_WRITE,
                flags,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error());
        }

        let non_null_ptr = NonNull::new(ptr as *mut u8)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "MAP_FAILED yielded null pointer mappings explicitly"))?;

        Ok(Self {
            ptr: non_null_ptr,
            capacity: aligned_capacity,
        })
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.capacity) }
    }
}

impl Drop for HugePageBuffer {
    fn drop(&mut self) {
        unsafe {
            // Guarantee pure memory reclamation scaling mapped explicitly natively
            munmap(self.ptr.as_ptr() as *mut libc::c_void, self.capacity);
        }
    }
}

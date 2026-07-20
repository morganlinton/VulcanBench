#![allow(unsafe_code)]
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct TrackingAllocator;

static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

pub fn get_allocation_stats() -> (u64, u64) {
    let bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let count = ALLOC_COUNT.load(Ordering::Relaxed);
    (bytes, count)
}

#[derive(Debug, Clone, Copy)]
pub struct AllocationSnapshot {
    bytes: u64,
    count: u64,
}

impl AllocationSnapshot {
    pub fn capture() -> Self {
        let (bytes, count) = get_allocation_stats();
        Self { bytes, count }
    }

    pub fn delta_since(self, earlier: Self) -> (u64, u64) {
        (
            self.bytes.saturating_sub(earlier.bytes),
            self.count.saturating_sub(earlier.count),
        )
    }
}

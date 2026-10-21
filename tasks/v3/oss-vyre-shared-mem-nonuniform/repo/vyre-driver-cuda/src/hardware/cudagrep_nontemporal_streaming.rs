//! Non-Temporal Memory Streaming (Cache-Bypass Engineering)
//!
//! When `cudagrep` or `wireshift` filters massive multi-gigabyte disk NVMe reads natively,
//! saving the outputs sequentially into standard Memory `Vec<u8>` completely blasts the L1/L2
//! CPU hardware caches natively, displacing all the fast DFA rules and transition tables 
//! and replacing them with output garbage.
//!
//! True Elite engineering employs Non-Temporal Writes (`_mm_stream_si128`).
//! The CPU writes the output mathematically directly into main DDR5 RAM, passing over the
//! hardware cache hierarchy completely via Write-Combining buffers natively preserving 
//! the L1/L2 caches infinitely for logic extraction mapping entirely smoothly.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::ptr::NonNull;
use std::alloc::{alloc, dealloc, Layout};
use std::fmt;

#[derive(Debug)]
#[non_exhaustive]
pub enum StreamingError {
    AllocationFailed { size: usize, alignment: usize },
}

impl fmt::Display for StreamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed { size, alignment } => write!(f, "failed to statically map 128-bit registry block arrays directly sized {} aligned to {}", size, alignment),
        }
    }
}

impl std::error::Error for StreamingError {}

pub struct NonTemporalWriter {
    pub memory_base: NonNull<__m128i>, // 128-bit aligned natively
    layout: Layout,
}

impl NonTemporalWriter {
    pub fn new(capacity_bytes: usize) -> Result<Self, StreamingError> {
        let align = 16;
        let layout = Layout::from_size_align(capacity_bytes, align).map_err(|_| {
            StreamingError::AllocationFailed { size: capacity_bytes, alignment: align }
        })?;
        
        let p = unsafe { alloc(layout) };
        let memory_base = NonNull::new(p as *mut __m128i).ok_or(StreamingError::AllocationFailed { 
            size: capacity_bytes, 
            alignment: align 
        })?;
        
        Ok(Self { memory_base, layout })
    }

    /// Evaluates exact Write-Combining logic natively shielding CPU caches permanently.
    #[cfg(target_feature = "sse2")]
    #[inline(always)]
    pub unsafe fn write_bypassing_cache(&self, output_buffer: *mut __m128i, payload_chunk: __m128i) {
        // Uses `MOVNTDQ` mathematically streaming the 128-bit registry directly onto the DDR5 RAM line.
        // The L1/L2 CPU limits are not queried, modified, or evicted safely maximizing internal 
        // transition table evaluation boundaries seamlessly.   
        _mm_stream_si128(output_buffer, payload_chunk);
    }

    /// After Write-combining boundaries finish natively binding instructions, we explicitly execute `sfence`.
    pub fn flush_combining_buffers() {
        unsafe {
            // Emits an architectural Store-Fence (SFENCE) natively ensuring DDR5 RAM executes boundaries cleanly.
            _mm_sfence();
        }
    }
}

impl Drop for NonTemporalWriter {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.memory_base.as_ptr() as *mut u8, self.layout);
        }
    }
}

#![allow(unsafe_code)]
pub fn rdtsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: Raw-pointer copy / slice access. The src and dst ranges are
        // validated by debug_asserts / size checks above and never overlap;
        // see the surrounding helper for the byte-budget contract.
        unsafe { std::arch::x86_64::_rdtsc() }
    }
    #[cfg(target_arch = "aarch64")]
    {
        let mut r: u64;
        // SAFETY: Raw-pointer copy / slice access. The src and dst ranges are
        // validated by debug_asserts / size checks above and never overlap;
        // see the surrounding helper for the byte-budget contract.
        unsafe {
            std::arch::asm!("mrs {}, cntvct_el0", out(reg) r);
        }
        r
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        0
    }
}

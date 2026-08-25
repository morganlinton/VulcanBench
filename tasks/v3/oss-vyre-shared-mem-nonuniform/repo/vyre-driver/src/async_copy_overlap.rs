//! D3 substrate: async-copy / kernel-overlap decision policy.
//!
//! When a host→device copy targets a buffer and a downstream kernel
//! does NOT read that buffer, the copy can run on a separate stream
//! concurrently with the kernel. This hides copy latency: a 100 µs
//! H2D transfer overlapped with a 200 µs kernel finishes in 200 µs
//! total instead of 300 µs serial.
//!
//! Pure decision: given the copy's destination slot and the kernel's
//! `ArmBindingSummary`, can the dispatcher fire the copy on a side
//! stream and let the kernel run concurrently on the main stream?
//!
//! Read-after-copy on the same slot is the unsafe case  -  kernel
//! must wait for the copy to land. Otherwise overlap is fine.

use crate::arm_independence::ArmBindingSummary;

/// Verdict for [`can_overlap_copy_with_kernel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyOverlapDecision {
    /// Copy can run on a side stream concurrently with the kernel  -
    /// the kernel does not read the destination slot.
    Overlap,
    /// Kernel reads the slot the copy targets  -  must serialise (copy
    /// completes before kernel starts).
    Serialize,
}

/// Decide whether a host→device copy targeting `copy_dst_slot` can
/// overlap with a kernel described by `kernel_arm`. Pure: no IR walk,
/// no allocation.
#[must_use]
pub fn can_overlap_copy_with_kernel(
    copy_dst_slot: u32,
    kernel_arm: &ArmBindingSummary,
) -> CopyOverlapDecision {
    if kernel_arm.reads.contains(&copy_dst_slot) {
        return CopyOverlapDecision::Serialize;
    }
    if kernel_arm.writes.contains(&copy_dst_slot) {
        // Kernel writes the same slot  -  RAW would race regardless of
        // ordering. The runtime should never plan an H2D copy whose
        // destination is a kernel output, but defensive serialization
        // keeps the verdict sound.
        return CopyOverlapDecision::Serialize;
    }
    CopyOverlapDecision::Overlap
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm(reads: &[u32], writes: &[u32]) -> ArmBindingSummary {
        ArmBindingSummary {
            reads: reads.iter().copied().collect(),
            writes: writes.iter().copied().collect(),
        }
    }

    #[test]
    fn copy_to_unread_slot_overlaps() {
        let kernel = arm(&[0, 1], &[2]);
        assert_eq!(
            can_overlap_copy_with_kernel(7, &kernel),
            CopyOverlapDecision::Overlap
        );
    }

    #[test]
    fn copy_to_kernel_read_slot_serialises() {
        let kernel = arm(&[0, 1], &[2]);
        assert_eq!(
            can_overlap_copy_with_kernel(0, &kernel),
            CopyOverlapDecision::Serialize
        );
    }

    #[test]
    fn copy_to_kernel_write_slot_serialises() {
        // Defensive: copying onto kernel's output buffer is suspect,
        // but if the runtime plans it the substrate must say
        // Serialize so the kernel sees the copied bytes.
        let kernel = arm(&[0], &[5]);
        assert_eq!(
            can_overlap_copy_with_kernel(5, &kernel),
            CopyOverlapDecision::Serialize
        );
    }

    #[test]
    fn copy_with_empty_kernel_overlaps() {
        let kernel = arm(&[], &[]);
        assert_eq!(
            can_overlap_copy_with_kernel(0, &kernel),
            CopyOverlapDecision::Overlap
        );
    }
}

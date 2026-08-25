//! GPUDirect Storage capability probe + passthrough helper.
//!
//! NVMe → VRAM direct DMA on Linux with nvidia-fs installed + kernel
//! 6.0+. Callers probe the capability at startup via
//! [`GpuDirectCapability::probe`]; if present, they use
//! [`encode_nvme_read_sqe`] to pack a 64-byte NVMe Read command that
//! `AsyncUringStream::submit_nvme_passthrough` feeds into
//! `IORING_OP_URING_CMD`. Bytes land in a
//! `GpuMappedBuffer::from_bar1_peer`-backed region with zero host
//! bounce.
//!
//! Gated behind the `uring-cmd-nvme` feature  -  the module is
//! compiled even without the feature so consumers can read
//! [`GpuDirectCapability::probe`] and get a structured
//! `Disabled` result instead of a link error.

#[cfg(all(target_os = "linux", feature = "uring-cmd-nvme"))]
use std::fs;
#[cfg(all(target_os = "linux", feature = "uring-cmd-nvme"))]
use std::io::{ErrorKind, Read as _};

#[cfg(all(target_os = "linux", feature = "uring-cmd-nvme"))]
const MAX_NVIDIA_FS_STATS_BYTES: u64 = 1024 * 1024;

/// Result of probing the host for GPUDirect Storage support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuDirectCapability {
    /// The host exposes nvidia-fs and the kernel reports the
    /// driver as enabled.
    Available {
        /// Raw contents of `/proc/driver/nvidia-fs/stats` at probe
        /// time. Useful for diagnostics; the caller is free to
        /// parse it further.
        stats: String,
    },
    /// Probe ran but nvidia-fs isn't installed or the driver is
    /// disabled. Callers fall back to `IORING_OP_READ_FIXED` into
    /// host-visible GPU memory  -  still zero-copy past the PCIe
    /// root complex, but not bypassed.
    Unavailable {
        /// Why the capability isn't present.
        reason: &'static str,
    },
    /// The `uring-cmd-nvme` feature is compiled out; no GPUDirect
    /// probe ran. Loudly surface this so a caller that explicitly
    /// expected the fast path sees the config mismatch.
    FeatureDisabled,
}

impl GpuDirectCapability {
    /// Probe the host.
    ///
    /// Reads `/proc/driver/nvidia-fs/stats`. Presence of the file +
    /// non-empty contents = `Available`. A file-not-found or
    /// permission error = `Unavailable` with a structured reason.
    /// Non-Linux / feature-disabled hosts return `FeatureDisabled`.
    #[must_use]
    pub fn probe() -> Self {
        #[cfg(not(all(target_os = "linux", feature = "uring-cmd-nvme")))]
        {
            GpuDirectCapability::FeatureDisabled
        }

        #[cfg(all(target_os = "linux", feature = "uring-cmd-nvme"))]
        match read_nvidia_fs_stats() {
            Ok(stats) if !stats.trim().is_empty() => {
                GpuDirectCapability::Available { stats }
            }
            Ok(_) => GpuDirectCapability::Unavailable {
                reason: "nvidia-fs stats file is empty; driver reports no GPUDirect sessions",
            },
            Err(err) if err.kind() == ErrorKind::NotFound => GpuDirectCapability::Unavailable {
                reason: "/proc/driver/nvidia-fs/stats not found; nvidia-fs is not installed",
            },
            Err(err) if err.kind() == ErrorKind::PermissionDenied => GpuDirectCapability::Unavailable {
                reason: "/proc/driver/nvidia-fs/stats refused permission; run with adequate privileges",
            },
            Err(_) => GpuDirectCapability::Unavailable {
                reason: "/proc/driver/nvidia-fs/stats read failed for an unexpected reason",
            },
        }
    }

    /// True when the fast path is available and callers should
    /// construct a `GpuMappedBuffer::from_bar1_peer`-backed region.
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, GpuDirectCapability::Available { .. })
    }
}

#[cfg(all(target_os = "linux", feature = "uring-cmd-nvme"))]
fn read_nvidia_fs_stats() -> std::io::Result<String> {
    let mut file = fs::File::open("/proc/driver/nvidia-fs/stats")?;
    let mut stats = String::new();
    file.by_ref()
        .take(MAX_NVIDIA_FS_STATS_BYTES + 1)
        .read_to_string(&mut stats)?;
    let stats_len = u64::try_from(stats.len()).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("nvidia-fs stats length cannot fit u64: {error}"),
        )
    })?;
    if stats_len > MAX_NVIDIA_FS_STATS_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "nvidia-fs stats exceeded bounded read limit",
        ));
    }
    Ok(stats)
}

/// NVMe `0x02` Read opcode (see NVMe Base Spec 1.4, §5.15).
pub const NVME_CMD_READ: u8 = 0x02;

/// Encode a 64-byte NVMe Read command payload suitable for
/// `AsyncUringStream::submit_nvme_passthrough`. Callers supply
/// the target LBA range + the destination BAR1 peer pointer; the
/// kernel DMAs the blocks directly into VRAM.
///
/// # NVMe passthrough layout
///
/// ```text
/// byte  0..4 : cmd_op (NVME_CMD_READ)
/// byte  4..8 : nsid    (namespace id, commonly 1)
/// byte  8..16: reserved
/// byte 16..24: reserved
/// byte 24..32: reserved (metadata ptr)
/// byte 32..40: dest_ptr (BAR1 peer  -  VRAM)
/// byte 40..48: starting LBA (little-endian u64)
/// byte 48..52: number_of_blocks (zero-based, so `blocks - 1`)
/// byte 52..56: dsmgmt
/// byte 56..60: reserved
/// byte 60..64: reserved
/// ```
///
/// The helper zeroes reserved regions defensively so forging is
/// harder. The caller retains responsibility for validating lba +
/// blocks against the namespace's capacity.
#[must_use]
pub fn encode_nvme_read_sqe(
    namespace_id: u32,
    starting_lba: u64,
    blocks: u32,
    dest_bar1_ptr: u64,
) -> [u8; 64] {
    assert!(
        blocks > 0,
        "NVMe read SQE cannot encode zero blocks; validate read length before submitting GPU-direct ingest"
    );
    let mut buf = [0u8; 64];
    buf[0] = NVME_CMD_READ;
    buf[4..8].copy_from_slice(&namespace_id.to_le_bytes());
    buf[32..40].copy_from_slice(&dest_bar1_ptr.to_le_bytes());
    buf[40..48].copy_from_slice(&starting_lba.to_le_bytes());
    // NVMe encodes "number of logical blocks" as zero-based: 0 = 1 block.
    let zero_based = blocks - 1;
    buf[48..52].copy_from_slice(&zero_based.to_le_bytes());
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_returns_a_structured_variant() {
        // We don't assert which variant  -  depends on host  -  but we
        // do assert the probe never panics and returns one of the
        // defined variants.
        match GpuDirectCapability::probe() {
            GpuDirectCapability::Available { .. } => {}
            GpuDirectCapability::Unavailable { .. } => {}
            GpuDirectCapability::FeatureDisabled => {}
        }
    }

    #[test]
    fn encode_nvme_read_sqe_layout_matches_spec() {
        let sqe = encode_nvme_read_sqe(
            /* nsid = */ 1,
            /* starting_lba = */ 0x1122_3344_5566_7788,
            /* blocks = */ 8,
            /* dest = */ 0xAABB_CCDD_EEFF_0011,
        );
        assert_eq!(sqe[0], NVME_CMD_READ);
        assert_eq!(sqe[4..8], 1u32.to_le_bytes());
        assert_eq!(sqe[32..40], 0xAABB_CCDD_EEFF_0011u64.to_le_bytes());
        assert_eq!(sqe[40..48], 0x1122_3344_5566_7788u64.to_le_bytes());
        // 8 blocks → 7 zero-based.
        assert_eq!(sqe[48..52], 7u32.to_le_bytes());
        // Reserved regions stay zero.
        assert_eq!(&sqe[8..32], &[0u8; 24]);
        assert_eq!(&sqe[52..64], &[0u8; 12]);
    }

    #[test]
    fn encode_nvme_single_block_yields_zero_in_nblocks_field() {
        // NVMe's zero-based encoding: 1 block → 0.
        let sqe = encode_nvme_read_sqe(1, 0, 1, 0);
        assert_eq!(sqe[48..52], 0u32.to_le_bytes());
    }

    #[test]
    fn is_available_reflects_variant() {
        let available = GpuDirectCapability::Available {
            stats: "session_count=1".into(),
        };
        assert!(available.is_available());
        let unavail = GpuDirectCapability::Unavailable {
            reason: "test reason",
        };
        assert!(!unavail.is_available());
        assert!(!GpuDirectCapability::FeatureDisabled.is_available());
    }
}

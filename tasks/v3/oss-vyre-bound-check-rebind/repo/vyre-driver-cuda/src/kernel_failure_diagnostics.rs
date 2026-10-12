//! Actionable CUDA kernel capability diagnostics.

use crate::numeric::CUDA_NUMERIC;

/// Device capabilities relevant to launch eligibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaKernelDeviceEnvelope {
    /// CUDA SM major version.
    pub sm_major: u16,
    /// CUDA SM minor version.
    pub sm_minor: u16,
    /// Maximum threads per block.
    pub max_threads_per_block: u32,
    /// Available shared memory per block.
    pub shared_memory_per_block_bytes: u64,
    /// Whether cooperative grid launch is supported.
    pub supports_cooperative_launch: bool,
    /// Whether tensor-core lowering is supported.
    pub supports_tensor_cores: bool,
}

/// Kernel requirements that must be met before launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaKernelRequirement {
    /// Minimum CUDA SM major version.
    pub min_sm_major: u16,
    /// Minimum CUDA SM minor version when major versions match.
    pub min_sm_minor: u16,
    /// Threads per block requested by the kernel.
    pub requested_threads_per_block: u32,
    /// Shared memory per block requested by the kernel.
    pub requested_shared_memory_bytes: u64,
    /// Whether the kernel requires cooperative launch.
    pub requires_cooperative_launch: bool,
    /// Whether the kernel requires tensor-core instructions.
    pub requires_tensor_cores: bool,
}

/// CUDA launch shape requested by a runtime or generated launcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaKernelLaunchShape {
    /// Grid dimensions in CUDA blocks.
    pub grid: [u32; 3],
    /// Block dimensions in CUDA threads.
    pub block: [u32; 3],
    /// Dynamic shared-memory bytes requested at launch.
    pub dynamic_shared_memory_bytes: u32,
    /// Whether the launch uses the cooperative kernel ABI.
    pub cooperative: bool,
    /// Whether the kernel requires tensor-core instructions.
    pub requires_tensor_cores: bool,
}

/// Release-path launch envelope: device caps, requested shape, derived
/// residency numbers, and capability diagnostics in one stable record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaKernelLaunchEnvelope {
    /// Kernel label supplied by the caller.
    pub kernel: &'static str,
    /// Probed device capability envelope.
    pub device: CudaKernelDeviceEnvelope,
    /// Kernel requirement derived from the requested launch shape.
    pub requirement: CudaKernelRequirement,
    /// Original launch shape.
    pub shape: CudaKernelLaunchShape,
    /// Exact grid block count.
    pub grid_blocks: u64,
    /// Exact CUDA thread count per block.
    pub threads_per_block: u32,
    /// Cooperative resident block limit when the cooperative ABI is used.
    pub cooperative_resident_block_limit: Option<u64>,
    /// Capability diagnostic for the launch.
    pub diagnostic: CudaKernelLaunchDiagnostic,
}

impl CudaKernelLaunchEnvelope {
    /// Return true when the launch is eligible on the device envelope.
    #[must_use]
    pub fn is_launchable(&self) -> bool {
        self.diagnostic.is_launchable()
            && self
                .cooperative_resident_block_limit
                .is_none_or(|limit| self.grid_blocks <= limit)
    }

    /// Stable one-line release diagnostic including shape and residency.
    #[must_use]
    pub fn stable_message(&self) -> String {
        let mut message = self.diagnostic.stable_message();
        push_launch_envelope_suffix(self, &mut message);
        message
    }
}

/// Error while deriving a CUDA launch envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaKernelLaunchEnvelopeError {
    /// Actionable fix message.
    pub fix: String,
}

impl std::fmt::Display for CudaKernelLaunchEnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.fix)
    }
}

impl std::error::Error for CudaKernelLaunchEnvelopeError {}

/// Capability failure reason for one CUDA launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaKernelCapabilityFailure {
    /// Device SM version is below the kernel requirement.
    SmVersion {
        /// Required major version.
        required_major: u16,
        /// Required minor version.
        required_minor: u16,
        /// Actual major version.
        actual_major: u16,
        /// Actual minor version.
        actual_minor: u16,
    },
    /// Requested block size exceeds the device limit.
    ThreadsPerBlock {
        /// Requested threads per block.
        requested: u32,
        /// Device maximum.
        maximum: u32,
    },
    /// Requested shared memory exceeds the device limit.
    SharedMemory {
        /// Requested shared memory bytes.
        requested: u64,
        /// Device maximum.
        maximum: u64,
    },
    /// Cooperative launch is required but unsupported.
    CooperativeLaunch,
    /// Tensor cores are required but unsupported.
    TensorCores,
}

/// Launch diagnostic with all missing CUDA requirements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaKernelLaunchDiagnostic {
    /// Kernel label supplied by the caller.
    pub kernel: &'static str,
    /// Missing or invalid requirements.
    pub failures: Vec<CudaKernelCapabilityFailure>,
}

impl CudaKernelLaunchDiagnostic {
    /// Return true when every requirement is satisfied.
    #[must_use]
    pub fn is_launchable(&self) -> bool {
        self.failures.is_empty()
    }

    /// Stable single-line diagnostic for release logs.
    #[must_use]
    pub fn stable_message(&self) -> String {
        let mut message = String::new();
        write_stable_message(self.kernel, &self.failures, &mut message);
        message
    }

    /// Write the stable single-line diagnostic into caller-owned storage.
    pub fn stable_message_into(&self, out: &mut String) {
        write_stable_message(self.kernel, &self.failures, out);
    }
}

/// Caller-owned scratch for repeated CUDA launch diagnostics.
#[derive(Debug, Default)]
pub struct CudaKernelLaunchDiagnosticScratch {
    failures: Vec<CudaKernelCapabilityFailure>,
    message: String,
}

impl CudaKernelLaunchDiagnosticScratch {
    /// Diagnose and build the stable single-line diagnostic in reusable storage.
    pub fn diagnose_stable_message(
        &mut self,
        kernel: &'static str,
        device: CudaKernelDeviceEnvelope,
        requirement: CudaKernelRequirement,
    ) -> &str {
        record_cuda_kernel_launch_failures(device, requirement, &mut self.failures);
        write_stable_message(kernel, &self.failures, &mut self.message);
        &self.message
    }

    /// Build the stable single-line diagnostic for caller-owned failures.
    pub fn stable_message_for_failures(
        &mut self,
        kernel: &'static str,
        failures: &[CudaKernelCapabilityFailure],
    ) -> &str {
        write_stable_message(kernel, failures, &mut self.message);
        &self.message
    }
}

/// Borrowed launch diagnostic backed by caller-owned scratch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaKernelLaunchDiagnosticRef<'a> {
    /// Kernel label supplied by the caller.
    pub kernel: &'static str,
    /// Missing or invalid requirements.
    pub failures: &'a [CudaKernelCapabilityFailure],
}

impl CudaKernelLaunchDiagnosticRef<'_> {
    /// Return true when every requirement is satisfied.
    #[must_use]
    pub fn is_launchable(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Diagnose whether a CUDA kernel can launch on the selected device.
#[must_use]
pub fn diagnose_cuda_kernel_launch(
    kernel: &'static str,
    device: CudaKernelDeviceEnvelope,
    requirement: CudaKernelRequirement,
) -> CudaKernelLaunchDiagnostic {
    let mut scratch = CudaKernelLaunchDiagnosticScratch::default();
    let kernel = {
        let diagnostic =
            diagnose_cuda_kernel_launch_with_scratch(kernel, device, requirement, &mut scratch);
        diagnostic.kernel
    };
    CudaKernelLaunchDiagnostic {
        kernel,
        failures: scratch.failures,
    }
}

/// Build a release-path CUDA launch envelope from probed device caps and a
/// requested launch shape.
///
/// # Errors
///
/// Returns [`CudaKernelLaunchEnvelopeError`] when grid or block products
/// overflow the release diagnostic fields.
pub fn diagnose_cuda_kernel_launch_shape(
    kernel: &'static str,
    device: CudaKernelDeviceEnvelope,
    shape: CudaKernelLaunchShape,
    cooperative_resident_block_limit: Option<u64>,
) -> Result<CudaKernelLaunchEnvelope, CudaKernelLaunchEnvelopeError> {
    let grid_blocks = checked_dim_product_u64(shape.grid, "grid block count")?;
    let threads_per_block = checked_dim_product_u32(shape.block, "threads per block")?;
    let requirement = CudaKernelRequirement {
        min_sm_major: 0,
        min_sm_minor: 0,
        requested_threads_per_block: threads_per_block,
        requested_shared_memory_bytes: u64::from(shape.dynamic_shared_memory_bytes),
        requires_cooperative_launch: shape.cooperative,
        requires_tensor_cores: shape.requires_tensor_cores,
    };
    let diagnostic = diagnose_cuda_kernel_launch(kernel, device, requirement);
    Ok(CudaKernelLaunchEnvelope {
        kernel,
        device,
        requirement,
        shape,
        grid_blocks,
        threads_per_block,
        cooperative_resident_block_limit,
        diagnostic,
    })
}

/// Diagnose whether a CUDA kernel can launch using caller-owned scratch.
pub fn diagnose_cuda_kernel_launch_with_scratch<'a>(
    kernel: &'static str,
    device: CudaKernelDeviceEnvelope,
    requirement: CudaKernelRequirement,
    scratch: &'a mut CudaKernelLaunchDiagnosticScratch,
) -> CudaKernelLaunchDiagnosticRef<'a> {
    record_cuda_kernel_launch_failures(device, requirement, &mut scratch.failures);

    CudaKernelLaunchDiagnosticRef {
        kernel,
        failures: &scratch.failures,
    }
}

fn record_cuda_kernel_launch_failures(
    device: CudaKernelDeviceEnvelope,
    requirement: CudaKernelRequirement,
    failures: &mut Vec<CudaKernelCapabilityFailure>,
) {
    failures.clear();
    if (device.sm_major, device.sm_minor) < (requirement.min_sm_major, requirement.min_sm_minor) {
        failures.push(CudaKernelCapabilityFailure::SmVersion {
            required_major: requirement.min_sm_major,
            required_minor: requirement.min_sm_minor,
            actual_major: device.sm_major,
            actual_minor: device.sm_minor,
        });
    }
    if requirement.requested_threads_per_block > device.max_threads_per_block {
        failures.push(CudaKernelCapabilityFailure::ThreadsPerBlock {
            requested: requirement.requested_threads_per_block,
            maximum: device.max_threads_per_block,
        });
    }
    if requirement.requested_shared_memory_bytes > device.shared_memory_per_block_bytes {
        failures.push(CudaKernelCapabilityFailure::SharedMemory {
            requested: requirement.requested_shared_memory_bytes,
            maximum: device.shared_memory_per_block_bytes,
        });
    }
    if requirement.requires_cooperative_launch && !device.supports_cooperative_launch {
        failures.push(CudaKernelCapabilityFailure::CooperativeLaunch);
    }
    if requirement.requires_tensor_cores && !device.supports_tensor_cores {
        failures.push(CudaKernelCapabilityFailure::TensorCores);
    }
}

fn write_stable_message(
    kernel: &'static str,
    failures: &[CudaKernelCapabilityFailure],
    out: &mut String,
) {
    use std::fmt::Write as _;

    out.clear();
    let _ = write!(out, "cuda-kernel-capability-v1|kernel={kernel}|status=");
    if failures.is_empty() {
        out.push_str("ok");
        return;
    }
    out.push_str("blocked|fix=");
    for (index, failure) in failures.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        match failure {
            CudaKernelCapabilityFailure::SmVersion {
                required_major,
                required_minor,
                actual_major,
                actual_minor,
            } => {
                let _ = write!(
                    out,
                    "sm_version(required={required_major}.{required_minor},actual={actual_major}.{actual_minor})"
                );
            }
            CudaKernelCapabilityFailure::ThreadsPerBlock { requested, maximum } => {
                let _ = write!(
                    out,
                    "threads_per_block(requested={requested},max={maximum})"
                );
            }
            CudaKernelCapabilityFailure::SharedMemory { requested, maximum } => {
                let _ = write!(out, "shared_memory(requested={requested},max={maximum})");
            }
            CudaKernelCapabilityFailure::CooperativeLaunch => out.push_str("cooperative_launch"),
            CudaKernelCapabilityFailure::TensorCores => out.push_str("tensor_cores"),
        }
    }
}

fn push_launch_envelope_suffix(envelope: &CudaKernelLaunchEnvelope, out: &mut String) {
    use std::fmt::Write as _;

    let _ = write!(
        out,
        "|grid={:?}|block={:?}|grid_blocks={}|threads_per_block={}|dynamic_shared_bytes={}",
        envelope.shape.grid,
        envelope.shape.block,
        envelope.grid_blocks,
        envelope.threads_per_block,
        envelope.shape.dynamic_shared_memory_bytes
    );
    if let Some(limit) = envelope.cooperative_resident_block_limit {
        let _ = write!(out, "|cooperative_resident_block_limit={limit}");
        if envelope.grid_blocks > limit {
            let _ = write!(
                out,
                "|cooperative_residency=blocked(required={},limit={})",
                envelope.grid_blocks, limit
            );
        }
    }
}

fn checked_dim_product_u64(
    dims: [u32; 3],
    label: &'static str,
) -> Result<u64, CudaKernelLaunchEnvelopeError> {
    CUDA_NUMERIC.checked_dim_product_u64(dims).ok_or_else(|| {
        CudaKernelLaunchEnvelopeError {
            fix: format!(
                "CUDA launch envelope {label} overflowed u64 for dimensions {dims:?}. Fix: shard the launch before release diagnostics."
            ),
        }
    })
}

fn checked_dim_product_u32(
    dims: [u32; 3],
    label: &'static str,
) -> Result<u32, CudaKernelLaunchEnvelopeError> {
    CUDA_NUMERIC.checked_dim_product_u32(dims).ok_or_else(|| {
        let product = checked_dim_product_u64(dims, label).map_or_else(
            |_| "overflowed u64".to_string(),
            |value| value.to_string(),
        );
        CudaKernelLaunchEnvelopeError {
            fix: format!(
                "CUDA launch envelope {label} value {product} cannot fit u32. Fix: lower block dimensions before launch."
            ),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_accepts_satisfied_kernel_requirements() {
        let diagnostic = diagnose_cuda_kernel_launch(
            "frontier",
            device(),
            CudaKernelRequirement {
                min_sm_major: 9,
                min_sm_minor: 0,
                requested_threads_per_block: 256,
                requested_shared_memory_bytes: 32_768,
                requires_cooperative_launch: true,
                requires_tensor_cores: true,
            },
        );

        assert!(diagnostic.is_launchable());
        assert_eq!(
            diagnostic.stable_message(),
            "cuda-kernel-capability-v1|kernel=frontier|status=ok"
        );
    }

    #[test]
    fn diagnostic_reports_every_missing_requirement() {
        let diagnostic = diagnose_cuda_kernel_launch(
            "frontier",
            CudaKernelDeviceEnvelope {
                sm_major: 8,
                sm_minor: 6,
                max_threads_per_block: 512,
                shared_memory_per_block_bytes: 16_384,
                supports_cooperative_launch: false,
                supports_tensor_cores: false,
            },
            CudaKernelRequirement {
                min_sm_major: 9,
                min_sm_minor: 0,
                requested_threads_per_block: 1_024,
                requested_shared_memory_bytes: 65_536,
                requires_cooperative_launch: true,
                requires_tensor_cores: true,
            },
        );

        assert!(!diagnostic.is_launchable());
        assert_eq!(diagnostic.failures.len(), 5);
        let message = diagnostic.stable_message();
        assert!(message.contains("sm_version(required=9.0,actual=8.6)"));
        assert!(message.contains("threads_per_block(requested=1024,max=512)"));
        assert!(message.contains("shared_memory(requested=65536,max=16384)"));
        assert!(message.contains("cooperative_launch"));
        assert!(message.contains("tensor_cores"));
    }

    #[test]
    fn launch_envelope_records_shape_residency_and_stable_message() {
        let envelope = diagnose_cuda_kernel_launch_shape(
            "frontier",
            device(),
            CudaKernelLaunchShape {
                grid: [9, 2, 1],
                block: [128, 2, 1],
                dynamic_shared_memory_bytes: 32_768,
                cooperative: true,
                requires_tensor_cores: true,
            },
            Some(16),
        )
        .expect("Fix: valid CUDA launch envelope should derive");

        assert_eq!(envelope.grid_blocks, 18);
        assert_eq!(envelope.threads_per_block, 256);
        assert!(!envelope.is_launchable());
        let message = envelope.stable_message();
        assert!(message.contains("cuda-kernel-capability-v1|kernel=frontier"));
        assert!(message.contains("grid_blocks=18"));
        assert!(message.contains("threads_per_block=256"));
        assert!(message.contains("cooperative_residency=blocked(required=18,limit=16)"));
    }

    #[test]
    fn launch_envelope_rejects_thread_block_product_overflow() {
        let error = diagnose_cuda_kernel_launch_shape(
            "frontier",
            device(),
            CudaKernelLaunchShape {
                grid: [1, 1, 1],
                block: [u32::MAX, u32::MAX, 2],
                dynamic_shared_memory_bytes: 0,
                cooperative: false,
                requires_tensor_cores: false,
            },
            None,
        )
        .expect_err("oversized CUDA block shape must fail before diagnostics");

        assert!(error.fix.contains("threads per block"));
    }

    #[test]
    fn diagnostic_scratch_reuses_failure_and_message_storage() {
        let mut scratch = CudaKernelLaunchDiagnosticScratch::default();
        let failures_ptr = {
            let blocked = diagnose_cuda_kernel_launch_with_scratch(
                "frontier",
                CudaKernelDeviceEnvelope {
                    sm_major: 8,
                    sm_minor: 6,
                    max_threads_per_block: 512,
                    shared_memory_per_block_bytes: 16_384,
                    supports_cooperative_launch: false,
                    supports_tensor_cores: false,
                },
                CudaKernelRequirement {
                    min_sm_major: 9,
                    min_sm_minor: 0,
                    requested_threads_per_block: 1_024,
                    requested_shared_memory_bytes: 65_536,
                    requires_cooperative_launch: true,
                    requires_tensor_cores: true,
                },
                &mut scratch,
            );
            assert!(!blocked.is_launchable());
            assert_eq!(blocked.failures.len(), 5);
            blocked.failures.as_ptr()
        };

        let message = scratch.diagnose_stable_message(
            "frontier",
            CudaKernelDeviceEnvelope {
                sm_major: 8,
                sm_minor: 6,
                max_threads_per_block: 512,
                shared_memory_per_block_bytes: 16_384,
                supports_cooperative_launch: false,
                supports_tensor_cores: false,
            },
            CudaKernelRequirement {
                min_sm_major: 9,
                min_sm_minor: 0,
                requested_threads_per_block: 1_024,
                requested_shared_memory_bytes: 65_536,
                requires_cooperative_launch: true,
                requires_tensor_cores: true,
            },
        );
        assert!(message.contains("status=blocked"));
        let message_ptr = message.as_ptr();

        let launchable_failures_ptr = {
            let launchable = diagnose_cuda_kernel_launch_with_scratch(
                "frontier",
                device(),
                CudaKernelRequirement {
                    min_sm_major: 9,
                    min_sm_minor: 0,
                    requested_threads_per_block: 256,
                    requested_shared_memory_bytes: 32_768,
                    requires_cooperative_launch: true,
                    requires_tensor_cores: true,
                },
                &mut scratch,
            );
            assert!(launchable.is_launchable());
            launchable.failures.as_ptr()
        };
        assert_eq!(launchable_failures_ptr, failures_ptr);

        let message = scratch.diagnose_stable_message(
            "frontier",
            device(),
            CudaKernelRequirement {
                min_sm_major: 9,
                min_sm_minor: 0,
                requested_threads_per_block: 256,
                requested_shared_memory_bytes: 32_768,
                requires_cooperative_launch: true,
                requires_tensor_cores: true,
            },
        );

        assert_eq!(
            message,
            "cuda-kernel-capability-v1|kernel=frontier|status=ok"
        );
        assert_eq!(
            message.as_ptr(),
            message_ptr,
            "Fix: repeated CUDA launch diagnostics must reuse caller-owned message storage instead of allocating one string per failure and joining them."
        );
    }

    fn device() -> CudaKernelDeviceEnvelope {
        CudaKernelDeviceEnvelope {
            sm_major: 12,
            sm_minor: 0,
            max_threads_per_block: 1_024,
            shared_memory_per_block_bytes: 99_840,
            supports_cooperative_launch: true,
            supports_tensor_cores: true,
        }
    }
}

#[cfg(test)]

mod owned_diagnostic_allocation_tests {
    use super::*;

    #[test]
    fn owned_diagnostic_moves_failures_out_of_scratch_without_clone() {
        let diagnostic = diagnose_cuda_kernel_launch(
            "frontier",
            CudaKernelDeviceEnvelope {
                sm_major: 8,
                sm_minor: 9,
                max_threads_per_block: 512,
                shared_memory_per_block_bytes: 32_768,
                supports_cooperative_launch: false,
                supports_tensor_cores: false,
            },
            CudaKernelRequirement {
                min_sm_major: 9,
                min_sm_minor: 0,
                requested_threads_per_block: 1_024,
                requested_shared_memory_bytes: 65_536,
                requires_cooperative_launch: true,
                requires_tensor_cores: true,
            },
        );

        assert_eq!(diagnostic.failures.len(), 5);

        let source = include_str!("kernel_failure_diagnostics.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: CUDA diagnostic production source must be present before tests");
        assert!(
            !production.contains(".to_vec()"),
            "Fix: owned CUDA launch diagnostics must move the scratch failure vector instead of cloning it."
        );
        assert!(
            production.contains("use crate::numeric::CUDA_NUMERIC;")
                && production.contains("CUDA_NUMERIC.checked_dim_product_u64(dims)")
                && production.contains("CUDA_NUMERIC.checked_dim_product_u32(dims)")
                && !production.contains(concat!(
                    "vyre_driver::numeric::",
                    "checked_dim_product"
                )),
            "Fix: CUDA launch-envelope dimension products must route through the shared CUDA numeric policy."
        );
    }
}

//! Backend-neutral device-side convergence planning for iterative analyses.

/// Device-side convergence readback policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConvergenceReadbackPolicy {
    /// Read the changed flag once after the device-side iteration budget completes.
    FinalFlagOnly,
}

/// Execution plan for device-side fixed-point convergence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceConvergencePlan {
    /// Maximum number of device iterations before the final convergence flag is read.
    pub max_device_iterations: u32,
    /// Number of host-visible synchronization points caused by convergence detection.
    pub host_sync_points: u32,
    /// Number of changed-flag bytes read back to the host.
    pub changed_flag_readback_bytes: u32,
    /// Number of per-iteration host polls.
    pub host_iteration_polls: u32,
    /// Readback policy used by the plan.
    pub readback_policy: ConvergenceReadbackPolicy,
}

/// Errors produced while planning device-side convergence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceConvergencePlanError {
    /// Iteration budget was zero.
    EmptyIterationBudget,
    /// Changed flag width is invalid.
    InvalidChangedFlagWidth {
        /// Observed changed-flag byte width.
        bytes: u32,
    },
    /// The requested plan would poll the host every iteration.
    HostPolledConvergence {
        /// Requested number of host-side iteration polls.
        polls: u32,
    },
}

impl std::fmt::Display for DeviceConvergencePlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyIterationBudget => f.write_str(
                "device convergence iteration budget is zero. Fix: use at least one device iteration.",
            ),
            Self::InvalidChangedFlagWidth { bytes } => write!(
                f,
                "device convergence changed-flag width is {bytes} bytes. Fix: use a 4-byte device u32 changed flag."
            ),
            Self::HostPolledConvergence { polls } => write!(
                f,
                "device convergence requested {polls} host iteration polls. Fix: keep convergence detection device-side and read only the final changed flag."
            ),
        }
    }
}

impl std::error::Error for DeviceConvergencePlanError {}

/// Plan convergence detection for an iterative device dataflow kernel.
///
/// # Errors
///
/// Returns [`DeviceConvergencePlanError`] when the iteration budget is empty,
/// the changed flag does not match the device ABI, or the caller asks for
/// host-polled iteration convergence.
pub fn plan_device_convergence(
    max_device_iterations: u32,
    changed_flag_bytes: u32,
    requested_host_iteration_polls: u32,
) -> Result<DeviceConvergencePlan, DeviceConvergencePlanError> {
    if max_device_iterations == 0 {
        return Err(DeviceConvergencePlanError::EmptyIterationBudget);
    }
    if changed_flag_bytes != 4 {
        return Err(DeviceConvergencePlanError::InvalidChangedFlagWidth {
            bytes: changed_flag_bytes,
        });
    }
    if requested_host_iteration_polls != 0 {
        return Err(DeviceConvergencePlanError::HostPolledConvergence {
            polls: requested_host_iteration_polls,
        });
    }

    Ok(DeviceConvergencePlan {
        max_device_iterations,
        host_sync_points: 1,
        changed_flag_readback_bytes: changed_flag_bytes,
        host_iteration_polls: 0,
        readback_policy: ConvergenceReadbackPolicy::FinalFlagOnly,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convergence_plan_reads_final_flag_once() {
        let plan = plan_device_convergence(128, 4, 0).expect("Fix: valid plan should build");

        assert_eq!(plan.max_device_iterations, 128);
        assert_eq!(plan.host_sync_points, 1);
        assert_eq!(plan.changed_flag_readback_bytes, 4);
        assert_eq!(plan.host_iteration_polls, 0);
        assert_eq!(
            plan.readback_policy,
            ConvergenceReadbackPolicy::FinalFlagOnly
        );
    }

    #[test]
    fn convergence_plan_rejects_empty_iteration_budget() {
        let err = plan_device_convergence(0, 4, 0).expect_err("zero iterations cannot converge");

        assert_eq!(err, DeviceConvergencePlanError::EmptyIterationBudget);
        assert!(err.to_string().contains("at least one device iteration"));
    }

    #[test]
    fn convergence_plan_rejects_wrong_changed_flag_width() {
        let err = plan_device_convergence(8, 1, 0).expect_err("changed flag must be a u32");

        assert_eq!(
            err,
            DeviceConvergencePlanError::InvalidChangedFlagWidth { bytes: 1 }
        );
        assert!(err.to_string().contains("4-byte device u32 changed flag"));
    }

    #[test]
    fn convergence_plan_rejects_host_polled_iterations() {
        let err = plan_device_convergence(8, 4, 8)
            .expect_err("host polling every iteration is forbidden");

        assert_eq!(
            err,
            DeviceConvergencePlanError::HostPolledConvergence { polls: 8 }
        );
        assert!(err.to_string().contains("read only the final changed flag"));
    }

    #[test]
    fn generated_convergence_iteration_budgets_preserve_final_only_contract() {
        for max_device_iterations in 1..=4_096 {
            let plan = plan_device_convergence(max_device_iterations, 4, 0)
                .expect("Fix: generated nonzero iteration budgets should plan");
            assert_eq!(plan.max_device_iterations, max_device_iterations);
            assert_eq!(plan.host_sync_points, 1);
            assert_eq!(plan.changed_flag_readback_bytes, 4);
            assert_eq!(plan.host_iteration_polls, 0);
            assert_eq!(
                plan.readback_policy,
                ConvergenceReadbackPolicy::FinalFlagOnly
            );
        }
    }
}

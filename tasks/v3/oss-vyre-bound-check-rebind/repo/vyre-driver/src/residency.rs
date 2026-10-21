//! Backend-neutral resident-resource reuse telemetry.
//!
//! Resident graph reuse is a cross-backend performance invariant, not a CUDA
//! detail. CUDA planners, WGPU resident caches, and higher-level users need
//! to report cold uploads and warm resident reuses with the same vocabulary
//! so upload pressure can be compared without backend-specific adapters.

/// Cold-upload and warm-reuse counters for a retained resident graph.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidentGraphReuseTelemetry {
    /// Resident graph cache misses that required host-to-device upload.
    pub cold_uploads: u64,
    /// Resident graph cache hits that reused an already-live device graph.
    pub warm_reuses: u64,
    /// Graph bytes uploaded by cold misses.
    pub upload_bytes: u64,
    /// Graph upload bytes avoided by warm reuses.
    pub avoided_upload_bytes: u64,
}

impl ResidentGraphReuseTelemetry {
    /// Build telemetry from explicit counters.
    #[must_use]
    pub const fn from_counters(
        cold_uploads: u64,
        warm_reuses: u64,
        upload_bytes: u64,
        avoided_upload_bytes: u64,
    ) -> Self {
        Self {
            cold_uploads,
            warm_reuses,
            upload_bytes,
            avoided_upload_bytes,
        }
    }

    /// Telemetry for one cold graph upload.
    #[must_use]
    pub const fn cold_upload(upload_bytes: u64) -> Self {
        Self {
            cold_uploads: 1,
            warm_reuses: 0,
            upload_bytes,
            avoided_upload_bytes: 0,
        }
    }

    /// Telemetry for one warm resident graph reuse.
    #[must_use]
    pub const fn warm_reuse(avoided_upload_bytes: u64) -> Self {
        Self {
            cold_uploads: 0,
            warm_reuses: 1,
            upload_bytes: 0,
            avoided_upload_bytes,
        }
    }

    /// Return true when no resident-graph reuse event has been recorded.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.cold_uploads == 0
            && self.warm_reuses == 0
            && self.upload_bytes == 0
            && self.avoided_upload_bytes == 0
    }

    /// Merge two telemetry snapshots with checked arithmetic.
    pub fn checked_add(self, rhs: Self) -> Result<Self, ResidentGraphReuseTelemetryError> {
        Ok(Self {
            cold_uploads: crate::accounting::checked_add_u64_value(
                self.cold_uploads,
                rhs.cold_uploads,
                ResidentGraphReuseTelemetryError::CounterOverflow {
                    counter: "cold_uploads",
                },
            )?,
            warm_reuses: crate::accounting::checked_add_u64_value(
                self.warm_reuses,
                rhs.warm_reuses,
                ResidentGraphReuseTelemetryError::CounterOverflow {
                    counter: "warm_reuses",
                },
            )?,
            upload_bytes: crate::accounting::checked_add_u64_value(
                self.upload_bytes,
                rhs.upload_bytes,
                ResidentGraphReuseTelemetryError::ByteCounterOverflow {
                    counter: "upload_bytes",
                },
            )?,
            avoided_upload_bytes: crate::accounting::checked_add_u64_value(
                self.avoided_upload_bytes,
                rhs.avoided_upload_bytes,
                ResidentGraphReuseTelemetryError::ByteCounterOverflow {
                    counter: "avoided_upload_bytes",
                },
            )?,
        })
    }

    /// Return the telemetry delta observed after an earlier monotonic snapshot.
    pub fn checked_delta_since(
        self,
        earlier: Self,
    ) -> Result<Self, ResidentGraphReuseTelemetryError> {
        Ok(Self {
            cold_uploads: crate::accounting::checked_sub_u64_value(
                self.cold_uploads,
                earlier.cold_uploads,
                ResidentGraphReuseTelemetryError::CounterUnderflow {
                    counter: "cold_uploads",
                },
            )?,
            warm_reuses: crate::accounting::checked_sub_u64_value(
                self.warm_reuses,
                earlier.warm_reuses,
                ResidentGraphReuseTelemetryError::CounterUnderflow {
                    counter: "warm_reuses",
                },
            )?,
            upload_bytes: crate::accounting::checked_sub_u64_value(
                self.upload_bytes,
                earlier.upload_bytes,
                ResidentGraphReuseTelemetryError::ByteCounterUnderflow {
                    counter: "upload_bytes",
                },
            )?,
            avoided_upload_bytes: crate::accounting::checked_sub_u64_value(
                self.avoided_upload_bytes,
                earlier.avoided_upload_bytes,
                ResidentGraphReuseTelemetryError::ByteCounterUnderflow {
                    counter: "avoided_upload_bytes",
                },
            )?,
        })
    }

    /// Record one cold graph upload in place.
    pub fn record_cold_upload(
        &mut self,
        upload_bytes: u64,
    ) -> Result<(), ResidentGraphReuseTelemetryError> {
        *self = (*self).checked_add(Self::cold_upload(upload_bytes))?;
        Ok(())
    }

    /// Record one warm resident graph reuse in place.
    pub fn record_warm_reuse(
        &mut self,
        avoided_upload_bytes: u64,
    ) -> Result<(), ResidentGraphReuseTelemetryError> {
        *self = (*self).checked_add(Self::warm_reuse(avoided_upload_bytes))?;
        Ok(())
    }

    /// Record several cold graph uploads in place.
    pub fn record_cold_uploads(
        &mut self,
        cold_uploads: u64,
        upload_bytes: u64,
    ) -> Result<(), ResidentGraphReuseTelemetryError> {
        *self = (*self).checked_add(Self::from_counters(cold_uploads, 0, upload_bytes, 0))?;
        Ok(())
    }

    /// Record several warm resident graph reuses in place.
    pub fn record_warm_reuses(
        &mut self,
        warm_reuses: u64,
        avoided_upload_bytes: u64,
    ) -> Result<(), ResidentGraphReuseTelemetryError> {
        *self =
            (*self).checked_add(Self::from_counters(0, warm_reuses, 0, avoided_upload_bytes))?;
        Ok(())
    }
}

/// Resident graph reuse telemetry arithmetic failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidentGraphReuseTelemetryError {
    /// A count field overflowed `u64`.
    CounterOverflow {
        /// Counter that overflowed.
        counter: &'static str,
    },
    /// A count field moved backward between monotonic snapshots.
    CounterUnderflow {
        /// Counter that moved backward.
        counter: &'static str,
    },
    /// A byte counter field overflowed `u64`.
    ByteCounterOverflow {
        /// Byte counter that overflowed.
        counter: &'static str,
    },
    /// A byte counter field moved backward between monotonic snapshots.
    ByteCounterUnderflow {
        /// Byte counter that moved backward.
        counter: &'static str,
    },
}

impl std::fmt::Display for ResidentGraphReuseTelemetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CounterOverflow { counter } => write!(
                f,
                "resident graph reuse telemetry counter {counter} overflowed u64. Fix: rotate the telemetry window before resident graph reuse accounting saturates."
            ),
            Self::CounterUnderflow { counter } => write!(
                f,
                "resident graph reuse telemetry counter {counter} moved backward between snapshots. Fix: rebuild the resident owner; cache telemetry must be monotonic."
            ),
            Self::ByteCounterOverflow { counter } => write!(
                f,
                "resident graph reuse telemetry byte counter {counter} overflowed u64. Fix: shard the resident graph workload or rotate the telemetry window before byte accounting saturates."
            ),
            Self::ByteCounterUnderflow { counter } => write!(
                f,
                "resident graph reuse telemetry byte counter {counter} moved backward between snapshots. Fix: rebuild the resident owner; cache telemetry must be monotonic."
            ),
        }
    }
}

impl std::error::Error for ResidentGraphReuseTelemetryError {}

#[cfg(test)]
mod tests {
    use super::{ResidentGraphReuseTelemetry, ResidentGraphReuseTelemetryError};

    #[test]
    fn checked_delta_since_returns_monotonic_snapshot_delta() {
        let earlier = ResidentGraphReuseTelemetry::from_counters(1, 2, 64, 128);
        let later = ResidentGraphReuseTelemetry::from_counters(4, 7, 256, 1_024);

        assert_eq!(
            later.checked_delta_since(earlier),
            Ok(ResidentGraphReuseTelemetry::from_counters(3, 5, 192, 896))
        );
    }

    #[test]
    fn checked_delta_since_rejects_counter_regression() {
        let earlier = ResidentGraphReuseTelemetry::from_counters(2, 2, 64, 128);
        let later = ResidentGraphReuseTelemetry::from_counters(1, 2, 64, 128);

        assert_eq!(
            later.checked_delta_since(earlier),
            Err(ResidentGraphReuseTelemetryError::CounterUnderflow {
                counter: "cold_uploads"
            })
        );
    }

    #[test]
    fn checked_delta_since_rejects_byte_counter_regression() {
        let earlier = ResidentGraphReuseTelemetry::from_counters(2, 2, 128, 128);
        let later = ResidentGraphReuseTelemetry::from_counters(2, 2, 64, 128);

        assert_eq!(
            later.checked_delta_since(earlier),
            Err(ResidentGraphReuseTelemetryError::ByteCounterUnderflow {
                counter: "upload_bytes"
            })
        );
    }
}

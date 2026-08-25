//! Backend-neutral transfer accounting policy.
//!
//! Backends repeatedly account bytes, operations, copy counts, and copy slots
//! while staging host/device transfers. This module centralizes the checked
//! arithmetic and leaves each caller to supply only domain wording.

use crate::BackendError;

/// Error wording and split guidance for a transfer-accounting domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransferAccountingPolicy {
    domain: &'static str,
    fix_action: &'static str,
}

impl TransferAccountingPolicy {
    /// Create a transfer-accounting policy.
    #[must_use]
    pub const fn new(domain: &'static str, fix_action: &'static str) -> Self {
        Self { domain, fix_action }
    }

    /// Convert a host-sized byte count to `u64` without truncation.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when `bytes` cannot fit in `u64`.
    pub fn bytes_to_u64(self, bytes: usize, label: &str) -> Result<u64, BackendError> {
        u64::try_from(bytes).map_err(|_| BackendError::InvalidProgram {
            fix: format!(
                "Fix: {} {label} exceeds u64; {}.",
                self.domain, self.fix_action
            ),
        })
    }

    /// Add a byte count to a `u64` accumulator without wraparound.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when conversion or addition would overflow.
    pub fn add_bytes(self, total: &mut u64, bytes: usize, label: &str) -> Result<(), BackendError> {
        let bytes = u64::try_from(bytes).map_err(|_| BackendError::InvalidProgram {
            fix: format!(
                "Fix: {} {label} byte count exceeds u64; {}.",
                self.domain, self.fix_action
            ),
        })?;
        *total = total
            .checked_add(bytes)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {} {label} byte accounting overflowed u64; {}.",
                    self.domain, self.fix_action
                ),
            })?;
        Ok(())
    }

    /// Add a `u64` counter value without wraparound.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when addition would overflow.
    pub fn add_u64_counter(
        self,
        total: &mut u64,
        value: u64,
        label: &str,
        counter: &str,
    ) -> Result<(), BackendError> {
        *total = total
            .checked_add(value)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {} {label} {counter} overflowed u64; {}.",
                    self.domain, self.fix_action
                ),
            })?;
        Ok(())
    }

    /// Add a `usize` counter value without wraparound.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when addition would overflow.
    pub fn add_usize_counter(
        self,
        total: &mut usize,
        value: usize,
        label: &str,
        counter: &str,
    ) -> Result<(), BackendError> {
        *total = total
            .checked_add(value)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {} {label} {counter} overflowed usize; {}.",
                    self.domain, self.fix_action
                ),
            })?;
        Ok(())
    }

    /// Add one transfer operation to a `u64` accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when addition would overflow.
    pub fn add_operation(self, total: &mut u64, label: &str) -> Result<(), BackendError> {
        self.add_u64_counter(total, 1, label, "transfer operation accounting")
    }

    /// Add one copy to a `usize` accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when addition would overflow.
    pub fn add_copy_count(self, total: &mut usize, label: &str) -> Result<(), BackendError> {
        self.add_usize_counter(total, 1, label, "copy counting")
    }

    /// Add copy slots to a `usize` accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when addition would overflow.
    pub fn add_copy_slots(
        self,
        total: &mut usize,
        slots: usize,
        label: &str,
    ) -> Result<(), BackendError> {
        self.add_usize_counter(total, slots, label, "copy-slot accounting")
    }

    /// Multiply two capacity counts without wraparound.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when multiplication would overflow.
    pub fn mul_usize_capacity(
        self,
        lhs: usize,
        rhs: usize,
        label: &str,
    ) -> Result<usize, BackendError> {
        lhs.checked_mul(rhs)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {} {label} capacity overflowed usize for {lhs} x {rhs}; {}.",
                    self.domain, self.fix_action
                ),
            })
    }

    /// Add two capacity counts without wraparound.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when addition would overflow.
    pub fn add_usize_capacity(
        self,
        lhs: usize,
        rhs: usize,
        label: &str,
    ) -> Result<usize, BackendError> {
        lhs.checked_add(rhs)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {} {label} capacity overflowed usize for {lhs} + {rhs}; {}.",
                    self.domain, self.fix_action
                ),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::TransferAccountingPolicy;

    const CUDA_RESIDENT: TransferAccountingPolicy =
        TransferAccountingPolicy::new("CUDA resident", "split the transfer into bounded chunks");

    #[test]
    fn generated_transfer_accounting_matrix_accumulates_exactly() {
        for seed in 0..8192_u64 {
            let mut state = seed ^ 0xA17E_51ED_ACCE_5510;
            let mut bytes = 0_u64;
            let mut expected_bytes = 0_u64;
            let mut operations = 0_u64;
            let mut copy_count = 0_usize;
            let mut copy_slots = 0_usize;

            for _ in 0..16 {
                let byte_count = (next_u64(&mut state) as usize) & 0x3ff;
                let slot_count = (next_u64(&mut state) as usize) & 0x1f;

                CUDA_RESIDENT
                    .add_bytes(&mut bytes, byte_count, "generated upload")
                    .expect("Fix: generated byte accounting must stay in range");
                CUDA_RESIDENT
                    .add_operation(&mut operations, "generated upload")
                    .expect("Fix: generated operation accounting must stay in range");
                CUDA_RESIDENT
                    .add_copy_count(&mut copy_count, "generated upload")
                    .expect("Fix: generated copy accounting must stay in range");
                CUDA_RESIDENT
                    .add_copy_slots(&mut copy_slots, slot_count, "generated upload")
                    .expect("Fix: generated slot accounting must stay in range");
                assert_eq!(
                    CUDA_RESIDENT
                        .mul_usize_capacity(byte_count, 2, "generated upload")
                        .expect("Fix: generated capacity multiplication must stay in range"),
                    byte_count * 2
                );
                assert_eq!(
                    CUDA_RESIDENT
                        .add_usize_capacity(byte_count, slot_count, "generated upload")
                        .expect("Fix: generated capacity addition must stay in range"),
                    byte_count + slot_count
                );

                expected_bytes += byte_count as u64;
            }

            assert_eq!(bytes, expected_bytes);
            assert_eq!(operations, 16);
            assert_eq!(copy_count, 16);
        }
    }

    #[test]
    fn byte_accounting_overflow_is_rejected_without_mutating_total() {
        let mut bytes = u64::MAX;
        let error = CUDA_RESIDENT
            .add_bytes(&mut bytes, 1, "overflow probe")
            .expect_err("Fix: u64 byte accounting overflow must be rejected");

        assert_eq!(bytes, u64::MAX);
        assert!(
            error.to_string().contains("byte accounting overflowed"),
            "Fix: overflow error must identify byte accounting, got {error}"
        );
    }

    #[test]
    fn usize_counter_overflow_is_rejected_without_mutating_total() {
        let mut slots = usize::MAX;
        let error = CUDA_RESIDENT
            .add_copy_slots(&mut slots, 1, "overflow probe")
            .expect_err("Fix: usize copy-slot accounting overflow must be rejected");

        assert_eq!(slots, usize::MAX);
        assert!(
            error
                .to_string()
                .contains("copy-slot accounting overflowed"),
            "Fix: overflow error must identify slot accounting, got {error}"
        );
    }

    #[test]
    fn capacity_overflow_uses_shared_policy_wording() {
        let mul = CUDA_RESIDENT
            .mul_usize_capacity(usize::MAX, 2, "resident batch")
            .expect_err("Fix: capacity multiplication overflow must be rejected");
        assert!(
            mul.to_string().contains("capacity overflowed usize")
                && mul
                    .to_string()
                    .contains("split the transfer into bounded chunks"),
            "Fix: capacity multiplication overflow must carry policy domain and fix: {mul}"
        );

        let add = CUDA_RESIDENT
            .add_usize_capacity(usize::MAX, 1, "resident sequence")
            .expect_err("Fix: capacity addition overflow must be rejected");
        assert!(
            add.to_string().contains("capacity overflowed usize")
                && add
                    .to_string()
                    .contains("split the transfer into bounded chunks"),
            "Fix: capacity addition overflow must carry policy domain and fix: {add}"
        );
    }

    fn next_u64(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }
}

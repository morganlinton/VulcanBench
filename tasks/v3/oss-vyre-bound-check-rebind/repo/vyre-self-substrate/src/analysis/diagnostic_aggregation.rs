//! Compact device-side diagnostic aggregation contracts.

/// Byte layout for one compact diagnostic record emitted by device aggregation.
pub const COMPACT_DIAGNOSTIC_RECORD_BYTES: usize = 24;

/// Compact diagnostic readback plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticAggregationPlan {
    /// Number of input items scanned on device.
    pub input_items: u32,
    /// Number of compact diagnostic records requested.
    pub diagnostic_records: u32,
    /// Maximum diagnostic records allowed by the caller.
    pub max_records: u32,
    /// Bytes read back after device-side compaction.
    pub compact_readback_bytes: usize,
    /// Bytes that would have been read by an unbounded raw per-item payload.
    pub avoided_raw_readback_bytes: usize,
}

/// Diagnostic aggregation planning errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticAggregationError {
    /// The caller requested zero maximum diagnostics.
    ZeroRecordBudget,
    /// The device-reported diagnostic count exceeds the caller budget.
    RecordBudgetExceeded {
        /// Device-reported compact diagnostic records.
        diagnostic_records: u32,
        /// Caller-approved record budget.
        max_records: u32,
    },
    /// Byte arithmetic overflowed.
    ByteCountOverflow,
}

impl std::fmt::Display for DiagnosticAggregationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroRecordBudget => f.write_str(
                "diagnostic aggregation record budget is zero. Fix: choose a bounded nonzero diagnostic readback budget.",
            ),
            Self::RecordBudgetExceeded {
                diagnostic_records,
                max_records,
            } => write!(
                f,
                "device aggregated {diagnostic_records} diagnostics but budget allows {max_records}. Fix: increase max_records or fail with a truncated-diagnostics error before readback."
            ),
            Self::ByteCountOverflow => f.write_str(
                "diagnostic aggregation byte count overflowed. Fix: shard the input before compact diagnostic aggregation.",
            ),
        }
    }
}

impl std::error::Error for DiagnosticAggregationError {}

/// Plan compact diagnostic readback after device-side aggregation.
pub fn plan_compact_diagnostic_readback(
    input_items: u32,
    diagnostic_records: u32,
    max_records: u32,
    raw_record_bytes_per_item: usize,
) -> Result<DiagnosticAggregationPlan, DiagnosticAggregationError> {
    if max_records == 0 {
        return Err(DiagnosticAggregationError::ZeroRecordBudget);
    }
    if diagnostic_records > max_records {
        return Err(DiagnosticAggregationError::RecordBudgetExceeded {
            diagnostic_records,
            max_records,
        });
    }
    let compact_readback_bytes = (diagnostic_records as usize)
        .checked_mul(COMPACT_DIAGNOSTIC_RECORD_BYTES)
        .ok_or(DiagnosticAggregationError::ByteCountOverflow)?;
    let raw_readback_bytes = (input_items as usize)
        .checked_mul(raw_record_bytes_per_item)
        .ok_or(DiagnosticAggregationError::ByteCountOverflow)?;
    let avoided_raw_readback_bytes = raw_readback_bytes.saturating_sub(compact_readback_bytes);

    Ok(DiagnosticAggregationPlan {
        input_items,
        diagnostic_records,
        max_records,
        compact_readback_bytes,
        avoided_raw_readback_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_diagnostic_plan_bounds_readback_to_records() {
        let plan = plan_compact_diagnostic_readback(1_000_000, 3, 16, 16)
            .expect("Fix: bounded compact diagnostics should plan");

        assert_eq!(plan.compact_readback_bytes, 72);
        assert_eq!(plan.avoided_raw_readback_bytes, 15_999_928);
        assert_eq!(plan.max_records, 16);
    }

    #[test]
    fn compact_diagnostic_plan_accepts_zero_actual_records() {
        let plan = plan_compact_diagnostic_readback(4096, 0, 8, 16)
            .expect("Fix: zero diagnostics still has a bounded readback plan");

        assert_eq!(plan.compact_readback_bytes, 0);
        assert_eq!(plan.avoided_raw_readback_bytes, 65_536);
    }

    #[test]
    fn compact_diagnostic_plan_rejects_unbounded_or_over_budget_records() {
        assert_eq!(
            plan_compact_diagnostic_readback(100, 1, 0, 16).expect_err("zero budget must fail"),
            DiagnosticAggregationError::ZeroRecordBudget
        );
        assert_eq!(
            plan_compact_diagnostic_readback(100, 9, 8, 16)
                .expect_err("over budget records must fail"),
            DiagnosticAggregationError::RecordBudgetExceeded {
                diagnostic_records: 9,
                max_records: 8,
            }
        );
    }
}

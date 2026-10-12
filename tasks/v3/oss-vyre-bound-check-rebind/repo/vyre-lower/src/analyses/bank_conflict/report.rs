//! Output type for the bank-conflict analysis.

use serde::{Deserialize, Serialize};

use crate::analyses::AccessKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BankConflictKind {
    /// Threads in the warp access addresses that map to distinct
    /// banks (or the same bank with broadcast semantics on a read).
    /// Full single-cycle throughput.
    NoConflict,
    /// Threads access addresses that map to the same bank but for
    /// reads where hardware broadcast is supported (CUDA: same
    /// 32-bit word). Single cycle.
    BroadcastSafe,
    /// All N threads in a warp hit the same bank with N distinct
    /// addresses. Worst case  -  N-way serialization.
    Conflict { way_count: u32 },
    /// Index pattern not classifiable. Default conservative answer
    /// when phase-1 analysis can't prove safety.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictSeverity {
    /// `NoConflict` or `BroadcastSafe`  -  performance is fine.
    None,
    /// `Conflict { way_count: 2..=4 }`  -  typically a 2-4x slowdown
    /// on the affected accesses.
    Mild,
    /// `Conflict { way_count: 5..=15 }`  -  5-15x slowdown.
    Severe,
    /// `Conflict { way_count: 16+ }`  -  full warp serialization.
    Critical,
    /// Pattern unknown  -  caller should treat as suspect until phase-2
    /// upgrades the analysis.
    Unknown,
}

impl BankConflictKind {
    #[must_use]
    pub fn severity(&self) -> ConflictSeverity {
        match self {
            Self::NoConflict | Self::BroadcastSafe => ConflictSeverity::None,
            Self::Conflict { way_count } => match *way_count {
                0..=1 => ConflictSeverity::None,
                2..=4 => ConflictSeverity::Mild,
                5..=15 => ConflictSeverity::Severe,
                _ => ConflictSeverity::Critical,
            },
            Self::Unknown => ConflictSeverity::Unknown,
        }
    }
}

/// One shared-memory access site identified during analysis.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BankAccessSite {
    /// Index of the op in the kernel body's flat `ops` Vec.
    pub op_index: usize,
    /// Whether this is a load or a store.
    pub kind: AccessKind,
    /// Binding slot the access reads/writes.
    pub binding_slot: u32,
    /// Detected conflict pattern.
    pub conflict: BankConflictKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BankConflictReport {
    pub kernel_id: String,
    pub bank_count: u32,
    pub sites: Vec<BankAccessSite>,
}

impl BankConflictReport {
    /// Number of sites with severity worse than `None`.
    #[must_use]
    pub fn problematic_count(&self) -> usize {
        self.sites
            .iter()
            .filter(|s| !matches!(s.conflict.severity(), ConflictSeverity::None))
            .count()
    }

    /// Number of sites with `Critical` severity (16-way+ conflict).
    #[must_use]
    pub fn critical_count(&self) -> usize {
        self.sites
            .iter()
            .filter(|s| matches!(s.conflict.severity(), ConflictSeverity::Critical))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_conflict_severity_is_none() {
        assert_eq!(
            BankConflictKind::NoConflict.severity(),
            ConflictSeverity::None
        );
    }

    #[test]
    fn broadcast_safe_severity_is_none() {
        assert_eq!(
            BankConflictKind::BroadcastSafe.severity(),
            ConflictSeverity::None
        );
    }

    #[test]
    fn way_2_conflict_is_mild() {
        assert_eq!(
            BankConflictKind::Conflict { way_count: 2 }.severity(),
            ConflictSeverity::Mild
        );
    }

    #[test]
    fn way_4_conflict_is_mild() {
        assert_eq!(
            BankConflictKind::Conflict { way_count: 4 }.severity(),
            ConflictSeverity::Mild
        );
    }

    #[test]
    fn way_8_conflict_is_severe() {
        assert_eq!(
            BankConflictKind::Conflict { way_count: 8 }.severity(),
            ConflictSeverity::Severe
        );
    }

    #[test]
    fn way_15_conflict_is_severe() {
        assert_eq!(
            BankConflictKind::Conflict { way_count: 15 }.severity(),
            ConflictSeverity::Severe
        );
    }

    #[test]
    fn way_16_conflict_is_critical() {
        assert_eq!(
            BankConflictKind::Conflict { way_count: 16 }.severity(),
            ConflictSeverity::Critical
        );
    }

    #[test]
    fn way_32_conflict_is_critical() {
        assert_eq!(
            BankConflictKind::Conflict { way_count: 32 }.severity(),
            ConflictSeverity::Critical
        );
    }

    #[test]
    fn unknown_severity_is_unknown() {
        assert_eq!(
            BankConflictKind::Unknown.severity(),
            ConflictSeverity::Unknown
        );
    }

    #[test]
    fn empty_report_has_zero_problematic() {
        let r = BankConflictReport {
            kernel_id: "empty".into(),
            bank_count: 32,
            sites: vec![],
        };
        assert_eq!(r.problematic_count(), 0);
        assert_eq!(r.critical_count(), 0);
    }

    #[test]
    fn problematic_count_aggregates_correctly() {
        let r = BankConflictReport {
            kernel_id: "k".into(),
            bank_count: 32,
            sites: vec![
                BankAccessSite {
                    op_index: 0,
                    kind: AccessKind::Load,
                    binding_slot: 0,
                    conflict: BankConflictKind::NoConflict,
                },
                BankAccessSite {
                    op_index: 1,
                    kind: AccessKind::Load,
                    binding_slot: 0,
                    conflict: BankConflictKind::Conflict { way_count: 4 },
                },
                BankAccessSite {
                    op_index: 2,
                    kind: AccessKind::Store,
                    binding_slot: 1,
                    conflict: BankConflictKind::Conflict { way_count: 32 },
                },
            ],
        };
        assert_eq!(r.problematic_count(), 2);
        assert_eq!(r.critical_count(), 1);
    }
}

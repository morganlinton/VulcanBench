//! Decode-side budgets for serialized scan database payloads.

use crate::serial::wire::encode::{
    ScanDatabaseHeader, ScanDatabaseSectionKind, MAX_SCAN_DATABASE_SECTIONS,
};

/// Upper bounds applied to a decoded scan database before its payload is
/// trusted or allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanDatabaseDecodeBudget {
    /// Maximum total bytes across all table sections.
    pub max_total_table_bytes: u64,
    /// Maximum number of table sections.
    pub max_section_count: usize,
    /// Maximum automata transition density, in transitions-per-state basis points.
    pub max_transition_density_bps: u64,
    /// Maximum number of duplicate section kinds tolerated.
    pub max_duplicate_section_kinds: usize,
    /// Maximum total verifier-fragment bytes.
    pub max_verifier_fragment_bytes: u64,
}

impl Default for ScanDatabaseDecodeBudget {
    fn default() -> Self {
        Self {
            max_total_table_bytes: 256 * 1024 * 1024,
            max_section_count: MAX_SCAN_DATABASE_SECTIONS,
            max_transition_density_bps: 50_000,
            max_duplicate_section_kinds: 0,
            max_verifier_fragment_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Observed structural shape of a decoded scan database, fed to the budget check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanDatabaseDecodeShape {
    /// Number of automata states across the database.
    pub state_count: u64,
    /// Number of automata transitions across the database.
    pub transition_count: u64,
    /// Total verifier-fragment bytes observed.
    pub verifier_fragment_bytes: u64,
}

/// Per-construct table-size budget keyed by construct family id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanConstructDecodeBudget {
    /// Construct family this budget applies to.
    pub construct_id: &'static str,
    /// Maximum automata states for the construct.
    pub max_states: u64,
    /// Maximum automata transitions for the construct.
    pub max_transitions: u64,
    /// Maximum literal bytes for the construct.
    pub max_literal_bytes: u64,
    /// Maximum capture slots for the construct.
    pub max_capture_slots: u64,
    /// Maximum Unicode table bytes for the construct.
    pub max_unicode_table_bytes: u64,
    /// Maximum verifier-fragment bytes for the construct.
    pub max_verifier_fragment_bytes: u64,
}

/// Observed per-construct table shape, checked against [`ScanConstructDecodeBudget`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanConstructDecodeShape {
    /// Construct family this shape describes.
    pub construct_id: &'static str,
    /// Observed automata states.
    pub states: u64,
    /// Observed automata transitions.
    pub transitions: u64,
    /// Observed literal bytes.
    pub literal_bytes: u64,
    /// Observed capture slots.
    pub capture_slots: u64,
    /// Observed Unicode table bytes.
    pub unicode_table_bytes: u64,
    /// Observed verifier-fragment bytes.
    pub verifier_fragment_bytes: u64,
}

/// Per-construct budget-check result pairing observed shape with its limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanConstructDecodeBudgetEvidence {
    /// Construct family this evidence describes.
    pub construct_id: &'static str,
    /// Observed automata states.
    pub states: u64,
    /// Observed automata transitions.
    pub transitions: u64,
    /// Observed literal bytes.
    pub literal_bytes: u64,
    /// Observed capture slots.
    pub capture_slots: u64,
    /// Observed Unicode table bytes.
    pub unicode_table_bytes: u64,
    /// Observed verifier-fragment bytes.
    pub verifier_fragment_bytes: u64,
    /// Budgeted maximum automata states.
    pub max_states: u64,
    /// Budgeted maximum automata transitions.
    pub max_transitions: u64,
    /// Budgeted maximum literal bytes.
    pub max_literal_bytes: u64,
    /// Budgeted maximum capture slots.
    pub max_capture_slots: u64,
    /// Budgeted maximum Unicode table bytes.
    pub max_unicode_table_bytes: u64,
    /// Budgeted maximum verifier-fragment bytes.
    pub max_verifier_fragment_bytes: u64,
}

impl ScanConstructDecodeBudgetEvidence {
    /// Returns `true` when every observed value is within its budgeted maximum.
    #[must_use]
    pub const fn within_budget(&self) -> bool {
        self.states <= self.max_states
            && self.transitions <= self.max_transitions
            && self.literal_bytes <= self.max_literal_bytes
            && self.capture_slots <= self.max_capture_slots
            && self.unicode_table_bytes <= self.max_unicode_table_bytes
            && self.verifier_fragment_bytes <= self.max_verifier_fragment_bytes
    }
}

/// Database-level budget-check result pairing observed totals with their limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanDatabaseDecodeBudgetEvidence {
    /// Observed total table bytes.
    pub total_table_bytes: u64,
    /// Observed section count.
    pub section_count: usize,
    /// Observed number of duplicate section kinds.
    pub duplicate_section_kinds: usize,
    /// Observed transition density in basis points.
    pub transition_density_bps: u64,
    /// Observed total verifier-fragment bytes.
    pub verifier_fragment_bytes: u64,
    /// Budgeted maximum total table bytes.
    pub max_total_table_bytes: u64,
    /// Budgeted maximum section count.
    pub max_section_count: usize,
    /// Budgeted maximum transition density in basis points.
    pub max_transition_density_bps: u64,
    /// Budgeted maximum number of duplicate section kinds.
    pub max_duplicate_section_kinds: usize,
    /// Budgeted maximum total verifier-fragment bytes.
    pub max_verifier_fragment_bytes: u64,
}

impl ScanDatabaseDecodeBudgetEvidence {
    /// Returns `true` when every observed total is within its budgeted maximum.
    #[must_use]
    pub const fn within_budget(&self) -> bool {
        self.total_table_bytes <= self.max_total_table_bytes
            && self.section_count <= self.max_section_count
            && self.duplicate_section_kinds <= self.max_duplicate_section_kinds
            && self.transition_density_bps <= self.max_transition_density_bps
            && self.verifier_fragment_bytes <= self.max_verifier_fragment_bytes
    }
}

/// Reason a decoded scan database failed a decode-time budget check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanDatabaseDecodeBudgetError {
    /// Summing table-section byte lengths overflowed `u64`.
    TableBytesOverflow,
    /// Total table bytes exceeded the budget.
    TableBytesExceeded {
        /// Observed total table bytes.
        actual: u64,
        /// Budgeted maximum.
        max: u64,
    },
    /// Section count exceeded the budget.
    SectionCountExceeded {
        /// Observed section count.
        actual: usize,
        /// Budgeted maximum.
        max: usize,
    },
    /// Duplicate section kinds exceeded the budget.
    DuplicateSectionsExceeded {
        /// Observed duplicate-kind count.
        actual: usize,
        /// Budgeted maximum.
        max: usize,
    },
    /// Automata transition density exceeded the budget.
    TransitionDensityExceeded {
        /// Observed density in basis points.
        actual_bps: u64,
        /// Budgeted maximum density in basis points.
        max_bps: u64,
    },
    /// Verifier-fragment bytes exceeded the budget.
    VerifierFragmentBytesExceeded {
        /// Observed verifier-fragment bytes.
        actual: u64,
        /// Budgeted maximum.
        max: u64,
    },
    /// A per-construct budget field was exceeded.
    ConstructBudgetExceeded {
        /// Construct family that exceeded its budget.
        construct_id: &'static str,
        /// Budget field that was exceeded.
        field: &'static str,
        /// Observed value.
        actual: u64,
        /// Budgeted maximum.
        max: u64,
    },
}

impl std::fmt::Display for ScanDatabaseDecodeBudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TableBytesOverflow => f.write_str(
                "scan database table byte count overflowed. Fix: reject this cache blob before allocation.",
            ),
            Self::TableBytesExceeded { actual, max } => write!(
                f,
                "scan database table bytes {actual} exceed budget {max}. Fix: reject or rebuild with smaller table sections."
            ),
            Self::SectionCountExceeded { actual, max } => write!(
                f,
                "scan database section count {actual} exceeds budget {max}. Fix: reject this cache blob before allocating section tables."
            ),
            Self::DuplicateSectionsExceeded { actual, max } => write!(
                f,
                "scan database duplicate section kinds {actual} exceed budget {max}. Fix: rebuild with one canonical section per kind or declare an explicit higher duplicate budget."
            ),
            Self::TransitionDensityExceeded {
                actual_bps,
                max_bps,
            } => write!(
                f,
                "scan database transition density {actual_bps} bps exceeds budget {max_bps}. Fix: quarantine dense automata or rebuild with a bounded verifier path."
            ),
            Self::VerifierFragmentBytesExceeded { actual, max } => write!(
                f,
                "scan database verifier fragment bytes {actual} exceed budget {max}. Fix: reject this cache blob or split verifier fragments."
            ),
            Self::ConstructBudgetExceeded {
                construct_id,
                field,
                actual,
                max,
            } => write!(
                f,
                "scan construct `{construct_id}` {field} value {actual} exceeds budget {max}. Fix: reject this cache blob or recompile the construct family with smaller tables."
            ),
        }
    }
}

impl std::error::Error for ScanDatabaseDecodeBudgetError {}

/// Validate decoded scan database header and observed table shape before the
/// payload is trusted or allocated downstream.
///
/// # Errors
///
/// Returns [`ScanDatabaseDecodeBudgetError`] for the first exceeded budget.
pub fn validate_scan_database_decode_budget(
    header: &ScanDatabaseHeader,
    shape: ScanDatabaseDecodeShape,
    budget: ScanDatabaseDecodeBudget,
) -> Result<ScanDatabaseDecodeBudgetEvidence, ScanDatabaseDecodeBudgetError> {
    let mut total_table_bytes = 0u64;
    let mut verifier_section_bytes = 0u64;
    let mut seen = [false; 6];
    let mut duplicate_section_kinds = 0usize;

    for section in &header.table_sections {
        total_table_bytes = total_table_bytes
            .checked_add(section.byte_len)
            .ok_or(ScanDatabaseDecodeBudgetError::TableBytesOverflow)?;
        if section.kind == ScanDatabaseSectionKind::VerifierFragments {
            verifier_section_bytes = verifier_section_bytes
                .checked_add(section.byte_len)
                .ok_or(ScanDatabaseDecodeBudgetError::TableBytesOverflow)?;
        }
        let index = section_kind_index(section.kind);
        if seen[index] {
            duplicate_section_kinds = duplicate_section_kinds.saturating_add(1);
        } else {
            seen[index] = true;
        }
    }

    let verifier_fragment_bytes = verifier_section_bytes.max(shape.verifier_fragment_bytes);
    let transition_density_bps = transition_density_bps(shape)?;
    let evidence = ScanDatabaseDecodeBudgetEvidence {
        total_table_bytes,
        section_count: header.table_sections.len(),
        duplicate_section_kinds,
        transition_density_bps,
        verifier_fragment_bytes,
        max_total_table_bytes: budget.max_total_table_bytes,
        max_section_count: budget.max_section_count,
        max_transition_density_bps: budget.max_transition_density_bps,
        max_duplicate_section_kinds: budget.max_duplicate_section_kinds,
        max_verifier_fragment_bytes: budget.max_verifier_fragment_bytes,
    };

    if evidence.total_table_bytes > evidence.max_total_table_bytes {
        return Err(ScanDatabaseDecodeBudgetError::TableBytesExceeded {
            actual: evidence.total_table_bytes,
            max: evidence.max_total_table_bytes,
        });
    }
    if evidence.section_count > evidence.max_section_count {
        return Err(ScanDatabaseDecodeBudgetError::SectionCountExceeded {
            actual: evidence.section_count,
            max: evidence.max_section_count,
        });
    }
    if evidence.duplicate_section_kinds > evidence.max_duplicate_section_kinds {
        return Err(ScanDatabaseDecodeBudgetError::DuplicateSectionsExceeded {
            actual: evidence.duplicate_section_kinds,
            max: evidence.max_duplicate_section_kinds,
        });
    }
    if evidence.transition_density_bps > evidence.max_transition_density_bps {
        return Err(ScanDatabaseDecodeBudgetError::TransitionDensityExceeded {
            actual_bps: evidence.transition_density_bps,
            max_bps: evidence.max_transition_density_bps,
        });
    }
    if evidence.verifier_fragment_bytes > evidence.max_verifier_fragment_bytes {
        return Err(ScanDatabaseDecodeBudgetError::VerifierFragmentBytesExceeded {
            actual: evidence.verifier_fragment_bytes,
            max: evidence.max_verifier_fragment_bytes,
        });
    }

    Ok(evidence)
}

/// Validate per-construct table shape budgets before construct-specific
/// payloads are trusted.
///
/// # Errors
///
/// Returns [`ScanDatabaseDecodeBudgetError::ConstructBudgetExceeded`] with the
/// exact construct id and budget field that failed.
pub fn validate_scan_construct_decode_budget(
    shape: ScanConstructDecodeShape,
    budget: ScanConstructDecodeBudget,
) -> Result<ScanConstructDecodeBudgetEvidence, ScanDatabaseDecodeBudgetError> {
    let evidence = ScanConstructDecodeBudgetEvidence {
        construct_id: shape.construct_id,
        states: shape.states,
        transitions: shape.transitions,
        literal_bytes: shape.literal_bytes,
        capture_slots: shape.capture_slots,
        unicode_table_bytes: shape.unicode_table_bytes,
        verifier_fragment_bytes: shape.verifier_fragment_bytes,
        max_states: budget.max_states,
        max_transitions: budget.max_transitions,
        max_literal_bytes: budget.max_literal_bytes,
        max_capture_slots: budget.max_capture_slots,
        max_unicode_table_bytes: budget.max_unicode_table_bytes,
        max_verifier_fragment_bytes: budget.max_verifier_fragment_bytes,
    };
    reject_construct_over_budget(
        shape.construct_id,
        "states",
        evidence.states,
        evidence.max_states,
    )?;
    reject_construct_over_budget(
        shape.construct_id,
        "transitions",
        evidence.transitions,
        evidence.max_transitions,
    )?;
    reject_construct_over_budget(
        shape.construct_id,
        "literal_bytes",
        evidence.literal_bytes,
        evidence.max_literal_bytes,
    )?;
    reject_construct_over_budget(
        shape.construct_id,
        "capture_slots",
        evidence.capture_slots,
        evidence.max_capture_slots,
    )?;
    reject_construct_over_budget(
        shape.construct_id,
        "unicode_table_bytes",
        evidence.unicode_table_bytes,
        evidence.max_unicode_table_bytes,
    )?;
    reject_construct_over_budget(
        shape.construct_id,
        "verifier_fragment_bytes",
        evidence.verifier_fragment_bytes,
        evidence.max_verifier_fragment_bytes,
    )?;
    Ok(evidence)
}

fn reject_construct_over_budget(
    construct_id: &'static str,
    field: &'static str,
    actual: u64,
    max: u64,
) -> Result<(), ScanDatabaseDecodeBudgetError> {
    if actual > max {
        Err(ScanDatabaseDecodeBudgetError::ConstructBudgetExceeded {
            construct_id,
            field,
            actual,
            max,
        })
    } else {
        Ok(())
    }
}

fn transition_density_bps(
    shape: ScanDatabaseDecodeShape,
) -> Result<u64, ScanDatabaseDecodeBudgetError> {
    if shape.state_count == 0 {
        return Ok(0);
    }
    shape
        .transition_count
        .checked_mul(10_000)
        .map(|scaled| scaled / shape.state_count)
        .ok_or(ScanDatabaseDecodeBudgetError::TableBytesOverflow)
}

const fn section_kind_index(kind: ScanDatabaseSectionKind) -> usize {
    match kind {
        ScanDatabaseSectionKind::LiteralTable => 0,
        ScanDatabaseSectionKind::AutomataTable => 1,
        ScanDatabaseSectionKind::VerifierFragments => 2,
        ScanDatabaseSectionKind::OutputLayout => 3,
        ScanDatabaseSectionKind::StreamingState => 4,
        ScanDatabaseSectionKind::RelationSeeds => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::wire::encode::{
        ScanDatabaseCompatibilityRecord, ScanDatabaseMode,
        ScanDatabaseReaderCompatibility, ScanDatabaseSectionHeader, UnsupportedScanFeature,
    };

    fn header() -> ScanDatabaseHeader {
        ScanDatabaseHeader {
            pattern_set_digest: [9u8; 32],
            compiler_version: "vyre-scan-budget-test".to_string(),
            mode: ScanDatabaseMode::Block,
            table_sections: vec![
                ScanDatabaseSectionHeader {
                    kind: ScanDatabaseSectionKind::LiteralTable,
                    offset: 0,
                    byte_len: 64,
                    section_digest: 11,
                },
                ScanDatabaseSectionHeader {
                    kind: ScanDatabaseSectionKind::AutomataTable,
                    offset: 64,
                    byte_len: 128,
                    section_digest: 12,
                },
                ScanDatabaseSectionHeader {
                    kind: ScanDatabaseSectionKind::VerifierFragments,
                    offset: 192,
                    byte_len: 32,
                    section_digest: 13,
                },
            ],
            unsupported_features: vec![UnsupportedScanFeature {
                pattern_index: 0,
                feature: "Fix: unsupported feature stays quarantined".to_string(),
            }],
            compatibility: ScanDatabaseCompatibilityRecord {
                construct_tier_digest: 0x51ca,
                dialect_digest: 0xd1a1,
                reader_compatibility: ScanDatabaseReaderCompatibility::RequiresVerifier,
            },
        }
    }

    #[test]
    fn scan_database_decode_budget_accepts_bounded_header() {
        let evidence = validate_scan_database_decode_budget(
            &header(),
            ScanDatabaseDecodeShape {
                state_count: 100,
                transition_count: 250,
                verifier_fragment_bytes: 32,
            },
            ScanDatabaseDecodeBudget {
                max_total_table_bytes: 512,
                max_section_count: 8,
                max_transition_density_bps: 30_000,
                max_duplicate_section_kinds: 0,
                max_verifier_fragment_bytes: 64,
            },
        )
        .unwrap();

        assert!(evidence.within_budget());
        assert_eq!(evidence.total_table_bytes, 224);
        assert_eq!(evidence.transition_density_bps, 25_000);
    }

    #[test]
    fn scan_database_decode_budget_rejects_duplicate_sections() {
        let mut header = header();
        header.table_sections.push(ScanDatabaseSectionHeader {
            kind: ScanDatabaseSectionKind::AutomataTable,
            offset: 224,
            byte_len: 1,
            section_digest: 14,
        });

        let error = validate_scan_database_decode_budget(
            &header,
            ScanDatabaseDecodeShape {
                state_count: 10,
                transition_count: 10,
                verifier_fragment_bytes: 32,
            },
            ScanDatabaseDecodeBudget {
                max_total_table_bytes: 512,
                max_section_count: 8,
                max_transition_density_bps: 20_000,
                max_duplicate_section_kinds: 0,
                max_verifier_fragment_bytes: 64,
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            ScanDatabaseDecodeBudgetError::DuplicateSectionsExceeded { actual: 1, max: 0 }
        );
    }

    #[test]
    fn scan_database_decode_budget_rejects_dense_transitions() {
        let error = validate_scan_database_decode_budget(
            &header(),
            ScanDatabaseDecodeShape {
                state_count: 10,
                transition_count: 100,
                verifier_fragment_bytes: 32,
            },
            ScanDatabaseDecodeBudget {
                max_total_table_bytes: 512,
                max_section_count: 8,
                max_transition_density_bps: 50_000,
                max_duplicate_section_kinds: 0,
                max_verifier_fragment_bytes: 64,
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            ScanDatabaseDecodeBudgetError::TransitionDensityExceeded {
                actual_bps: 100_000,
                max_bps: 50_000
            }
        );
    }

    #[test]
    fn scan_construct_decode_budget_accepts_bounded_construct_shape() {
        let evidence = validate_scan_construct_decode_budget(
            ScanConstructDecodeShape {
                construct_id: "capture_extraction_constructs",
                states: 32,
                transitions: 96,
                literal_bytes: 128,
                capture_slots: 4,
                unicode_table_bytes: 0,
                verifier_fragment_bytes: 512,
            },
            ScanConstructDecodeBudget {
                construct_id: "capture_extraction_constructs",
                max_states: 64,
                max_transitions: 128,
                max_literal_bytes: 256,
                max_capture_slots: 8,
                max_unicode_table_bytes: 0,
                max_verifier_fragment_bytes: 1024,
            },
        )
        .unwrap();

        assert!(evidence.within_budget());
        assert_eq!(evidence.construct_id, "capture_extraction_constructs");
        assert_eq!(evidence.capture_slots, 4);
    }

    #[test]
    fn scan_construct_decode_budget_rejects_exact_construct_field() {
        let error = validate_scan_construct_decode_budget(
            ScanConstructDecodeShape {
                construct_id: "unicode_classes",
                states: 10,
                transitions: 20,
                literal_bytes: 0,
                capture_slots: 0,
                unicode_table_bytes: 4096,
                verifier_fragment_bytes: 0,
            },
            ScanConstructDecodeBudget {
                construct_id: "unicode_classes",
                max_states: 10,
                max_transitions: 20,
                max_literal_bytes: 0,
                max_capture_slots: 0,
                max_unicode_table_bytes: 1024,
                max_verifier_fragment_bytes: 0,
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            ScanDatabaseDecodeBudgetError::ConstructBudgetExceeded {
                construct_id: "unicode_classes",
                field: "unicode_table_bytes",
                actual: 4096,
                max: 1024
            }
        );
        assert!(error.to_string().contains("unicode_classes"));
        assert!(error.to_string().contains("unicode_table_bytes"));
        assert!(error.to_string().contains("Fix:"));
    }
}

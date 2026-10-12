//! Edit-corpus registry contracts for incremental VAST parser evidence.

use std::error::Error;
use std::fmt::{Display, Formatter};

use super::{validate_vast, VastHeader};

/// Schema version for VAST edit-corpus evidence.
pub const VAST_EDIT_CORPUS_SCHEMA_VERSION: u32 = 1;

/// Stable BLAKE3 digest used by VAST edit-corpus evidence.
pub type VastEditDigest = [u8; 32];

/// One source edit in old-source byte coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VastEdit<'a> {
    /// Inclusive byte offset in the old source.
    pub old_start: u32,
    /// Exclusive byte offset in the old source.
    pub old_end: u32,
    /// Replacement bytes inserted at `old_start..old_end`.
    pub replacement: &'a [u8],
}

/// One changed byte range reported by an incremental parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VastChangedRange {
    /// Inclusive byte offset in the old source.
    pub old_start: u32,
    /// Exclusive byte offset in the old source.
    pub old_end: u32,
    /// Inclusive byte offset in the edited source.
    pub new_start: u32,
    /// Exclusive byte offset in the edited source.
    pub new_end: u32,
}

/// One registry case proving incremental VAST update equivalence.
#[derive(Debug, Clone, Copy)]
pub struct VastEditCorpusCase<'a> {
    /// Stable case id used by parser benchmark evidence.
    pub id: &'a str,
    /// Source bytes before applying [`edits`](Self::edits).
    pub before_bytes: &'a [u8],
    /// Sorted, non-overlapping old-source edit script.
    pub edits: &'a [VastEdit<'a>],
    /// Changed ranges reported by the incremental parser.
    pub changed_ranges: &'a [VastChangedRange],
    /// VAST bytes emitted by the incremental update path.
    pub updated_vast: &'a [u8],
    /// VAST bytes emitted by a full parse of the edited source.
    pub full_reparse_vast: &'a [u8],
    /// Parser diagnostics emitted for the edited source.
    pub diagnostics: &'a [u8],
    /// Number of VAST nodes reused from the old tree.
    pub reused_node_count: u32,
}

/// Normalized evidence emitted for one VAST edit-corpus case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VastEditCorpusEvidence {
    /// Schema version for this evidence row.
    pub schema_version: u32,
    /// Old-source byte length.
    pub before_byte_len: u32,
    /// Edited-source byte length.
    pub after_byte_len: u32,
    /// Number of edits in the script.
    pub edit_count: u32,
    /// Number of changed ranges reported.
    pub changed_range_count: u32,
    /// Number of VAST nodes reused from the old tree.
    pub reused_node_count: u32,
    /// Digest of the incremental-update VAST bytes.
    pub vast_digest: VastEditDigest,
    /// Digest of the full-reparse VAST bytes.
    pub full_reparse_vast_digest: VastEditDigest,
    /// Digest of parser diagnostics for the edited source.
    pub diagnostic_digest: VastEditDigest,
    /// Whether incremental update bytes exactly match full-reparse bytes.
    pub update_matches_full_reparse: bool,
}

/// Validation failures for VAST edit-corpus registry rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VastEditCorpusError {
    /// Case id is empty.
    EmptyCaseId,
    /// Edit script is empty.
    EmptyEditScript,
    /// Edit range is malformed.
    InvalidEditRange {
        /// Edit index in the script.
        index: usize,
        /// Inclusive old-source start byte.
        old_start: u32,
        /// Exclusive old-source end byte.
        old_end: u32,
    },
    /// Edit range overlaps or moves backward relative to the previous edit.
    OverlappingEdit {
        /// Edit index in the script.
        index: usize,
        /// Previous consumed old-source byte cursor.
        previous_end: u32,
        /// Current edit start byte.
        old_start: u32,
    },
    /// Edit range exceeds the old-source byte length.
    EditRangeOutOfBounds {
        /// Edit index in the script.
        index: usize,
        /// Exclusive old-source end byte.
        old_end: u32,
        /// Old-source byte length.
        before_len: u32,
    },
    /// Applying edits would exceed `u32` byte coordinates.
    EditedSourceTooLarge,
    /// Reported changed ranges differ from the edit script.
    ChangedRangeMismatch {
        /// Changed ranges derived from the edit script.
        expected: Vec<VastChangedRange>,
        /// Changed ranges supplied by the registry case.
        actual: Vec<VastChangedRange>,
    },
    /// Updated or full-reparse VAST bytes are invalid.
    InvalidVast {
        /// VAST source being validated.
        context: &'static str,
        /// Validation diagnostic.
        reason: String,
    },
    /// Reused-node count exceeds the updated VAST node count.
    ReusedNodeCountTooLarge {
        /// Reported reused-node count.
        reused_node_count: u32,
        /// Updated VAST node count.
        node_count: u32,
    },
    /// Incremental-update VAST bytes differ from full-reparse VAST bytes.
    FullReparseMismatch {
        /// Digest of the incremental-update VAST bytes.
        updated_digest: VastEditDigest,
        /// Digest of the full-reparse VAST bytes.
        full_reparse_digest: VastEditDigest,
    },
}

impl Display for VastEditCorpusError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCaseId => write!(f, "VAST edit corpus case id is empty. Fix: assign a stable parser corpus id."),
            Self::EmptyEditScript => write!(f, "VAST edit corpus case has no edits. Fix: record at least one source edit."),
            Self::InvalidEditRange {
                index,
                old_start,
                old_end,
            } => write!(
                f,
                "VAST edit {index} has invalid range {old_start}..{old_end}. Fix: use half-open old-source byte ranges."
            ),
            Self::OverlappingEdit {
                index,
                previous_end,
                old_start,
            } => write!(
                f,
                "VAST edit {index} starts at {old_start} before previous end {previous_end}. Fix: sort edits and reject overlaps."
            ),
            Self::EditRangeOutOfBounds {
                index,
                old_end,
                before_len,
            } => write!(
                f,
                "VAST edit {index} ends at {old_end} beyond source length {before_len}. Fix: clamp edits to the old source."
            ),
            Self::EditedSourceTooLarge => write!(
                f,
                "VAST edit script produced edited source beyond u32 byte coordinates. Fix: shard the edit corpus."
            ),
            Self::ChangedRangeMismatch { .. } => write!(
                f,
                "VAST edit corpus changed ranges do not match the edit script. Fix: derive changed ranges from the exact applied edits."
            ),
            Self::InvalidVast { context, reason } => write!(
                f,
                "VAST edit corpus {context} bytes are invalid: {reason}. Fix: emit validated VAST bytes before recording evidence."
            ),
            Self::ReusedNodeCountTooLarge {
                reused_node_count,
                node_count,
            } => write!(
                f,
                "VAST edit corpus reused_node_count={reused_node_count} exceeds updated node_count={node_count}. Fix: count only nodes reused in the updated VAST."
            ),
            Self::FullReparseMismatch { .. } => write!(
                f,
                "VAST incremental update bytes differ from full reparse bytes. Fix: repair changed-range invalidation before accepting the corpus row."
            ),
        }
    }
}

impl Error for VastEditCorpusError {}

/// Apply a sorted, non-overlapping edit script to old source bytes.
///
/// # Errors
///
/// Returns [`VastEditCorpusError`] when edit ranges are malformed, overlap, or
/// produce byte coordinates outside the VAST registry contract.
pub fn apply_vast_edit_script(
    before: &[u8],
    edits: &[VastEdit<'_>],
) -> Result<Vec<u8>, VastEditCorpusError> {
    let before_len_u32 =
        u32::try_from(before.len()).map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?;
    let mut edited = Vec::with_capacity(before.len());
    let mut cursor = 0_usize;
    for (index, edit) in edits.iter().enumerate() {
        let old_start = edit.old_start;
        let old_end = edit.old_end;
        if old_end < old_start {
            return Err(VastEditCorpusError::InvalidEditRange {
                index,
                old_start,
                old_end,
            });
        }
        if old_start < cursor as u32 {
            return Err(VastEditCorpusError::OverlappingEdit {
                index,
                previous_end: cursor as u32,
                old_start,
            });
        }
        if old_end > before_len_u32 {
            return Err(VastEditCorpusError::EditRangeOutOfBounds {
                index,
                old_end,
                before_len: before_len_u32,
            });
        }
        let start = old_start as usize;
        let end = old_end as usize;
        edited.extend_from_slice(&before[cursor..start]);
        edited.extend_from_slice(edit.replacement);
        cursor = end;
    }
    edited.extend_from_slice(&before[cursor..]);
    u32::try_from(edited.len()).map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?;
    Ok(edited)
}

/// Derive changed ranges from a sorted, non-overlapping edit script.
///
/// # Errors
///
/// Returns [`VastEditCorpusError`] when applying the script would be invalid.
pub fn changed_ranges_from_vast_edits(
    before: &[u8],
    edits: &[VastEdit<'_>],
) -> Result<Vec<VastChangedRange>, VastEditCorpusError> {
    validate_edit_script(before, edits)?;
    let mut ranges = Vec::with_capacity(edits.len());
    let mut delta: i64 = 0;
    for edit in edits {
        let old_len = i64::from(edit.old_end.saturating_sub(edit.old_start));
        let replacement_len = i64::try_from(edit.replacement.len())
            .map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?;
        let new_start_i64 = i64::from(edit.old_start) + delta;
        let new_end_i64 = new_start_i64 + replacement_len;
        let new_start =
            u32::try_from(new_start_i64).map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?;
        let new_end =
            u32::try_from(new_end_i64).map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?;
        ranges.push(VastChangedRange {
            old_start: edit.old_start,
            old_end: edit.old_end,
            new_start,
            new_end,
        });
        delta += replacement_len - old_len;
    }
    Ok(ranges)
}

/// Build normalized evidence for one edit-corpus case.
///
/// # Errors
///
/// Returns [`VastEditCorpusError`] when edits, changed ranges, VAST bytes, or
/// reuse counts violate the registry contract.
pub fn vast_edit_corpus_evidence(
    case: &VastEditCorpusCase<'_>,
) -> Result<VastEditCorpusEvidence, VastEditCorpusError> {
    if case.id.is_empty() {
        return Err(VastEditCorpusError::EmptyCaseId);
    }
    if case.edits.is_empty() {
        return Err(VastEditCorpusError::EmptyEditScript);
    }
    let after = apply_vast_edit_script(case.before_bytes, case.edits)?;
    let expected_ranges = changed_ranges_from_vast_edits(case.before_bytes, case.edits)?;
    if expected_ranges.as_slice() != case.changed_ranges {
        return Err(VastEditCorpusError::ChangedRangeMismatch {
            expected: expected_ranges,
            actual: case.changed_ranges.to_vec(),
        });
    }

    validate_vast(case.updated_vast).map_err(|error| VastEditCorpusError::InvalidVast {
        context: "updated",
        reason: error.to_string(),
    })?;
    validate_vast(case.full_reparse_vast).map_err(|error| VastEditCorpusError::InvalidVast {
        context: "full reparse",
        reason: error.to_string(),
    })?;
    let updated_header =
        VastHeader::decode(case.updated_vast).map_err(|error| VastEditCorpusError::InvalidVast {
            context: "updated header",
            reason: error.to_string(),
        })?;
    if case.reused_node_count > updated_header.node_count {
        return Err(VastEditCorpusError::ReusedNodeCountTooLarge {
            reused_node_count: case.reused_node_count,
            node_count: updated_header.node_count,
        });
    }

    let vast_digest = vast_edit_digest(case.updated_vast);
    let full_reparse_vast_digest = vast_edit_digest(case.full_reparse_vast);
    if vast_digest != full_reparse_vast_digest || case.updated_vast != case.full_reparse_vast {
        return Err(VastEditCorpusError::FullReparseMismatch {
            updated_digest: vast_digest,
            full_reparse_digest: full_reparse_vast_digest,
        });
    }

    Ok(VastEditCorpusEvidence {
        schema_version: VAST_EDIT_CORPUS_SCHEMA_VERSION,
        before_byte_len: u32::try_from(case.before_bytes.len())
            .map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?,
        after_byte_len: u32::try_from(after.len())
            .map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?,
        edit_count: u32::try_from(case.edits.len())
            .map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?,
        changed_range_count: u32::try_from(case.changed_ranges.len())
            .map_err(|_| VastEditCorpusError::EditedSourceTooLarge)?,
        reused_node_count: case.reused_node_count,
        vast_digest,
        full_reparse_vast_digest,
        diagnostic_digest: vast_edit_digest(case.diagnostics),
        update_matches_full_reparse: true,
    })
}

/// Compute the stable digest used by edit-corpus VAST evidence.
#[must_use]
pub fn vast_edit_digest(bytes: &[u8]) -> VastEditDigest {
    *blake3::hash(bytes).as_bytes()
}

fn validate_edit_script(
    before: &[u8],
    edits: &[VastEdit<'_>],
) -> Result<(), VastEditCorpusError> {
    apply_vast_edit_script(before, edits).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vast::pack_spine_vast;

    #[test]
    fn edit_corpus_evidence_requires_changed_range_full_parse_vast_match() {
        let before = b"int main() { return 0; }\n";
        let edit_start = before
            .iter()
            .position(|byte| *byte == b'0')
            .expect("fixture must contain literal zero") as u32;
        let edits = [VastEdit {
            old_start: edit_start,
            old_end: edit_start + 1,
            replacement: b"1",
        }];
        let changed_ranges = changed_ranges_from_vast_edits(before, &edits)
            .expect("fixture edit ranges must derive");
        let updated_vast = pack_spine_vast(&[1, 2, 3, 4]);
        let full_reparse_vast = updated_vast.clone();
        let case = VastEditCorpusCase {
            id: "c-return-literal-replace",
            before_bytes: before,
            edits: &edits,
            changed_ranges: &changed_ranges,
            updated_vast: &updated_vast,
            full_reparse_vast: &full_reparse_vast,
            diagnostics: b"ok",
            reused_node_count: 3,
        };

        let evidence = vast_edit_corpus_evidence(&case).expect("edit corpus must validate");

        assert_eq!(evidence.schema_version, VAST_EDIT_CORPUS_SCHEMA_VERSION);
        assert_eq!(evidence.before_byte_len, before.len() as u32);
        assert_eq!(evidence.after_byte_len, before.len() as u32);
        assert_eq!(evidence.edit_count, 1);
        assert_eq!(evidence.changed_range_count, 1);
        assert_eq!(evidence.reused_node_count, 3);
        assert_eq!(evidence.vast_digest, evidence.full_reparse_vast_digest);
        assert!(evidence.update_matches_full_reparse);
    }

    #[test]
    fn edit_corpus_rejects_full_reparse_vast_mismatch() {
        let before = b"int value;\n";
        let edits = [VastEdit {
            old_start: 4,
            old_end: 9,
            replacement: b"other",
        }];
        let changed_ranges = changed_ranges_from_vast_edits(before, &edits)
            .expect("fixture edit ranges must derive");
        let updated_vast = pack_spine_vast(&[1, 2, 3]);
        let full_reparse_vast = pack_spine_vast(&[1, 2, 4]);
        let case = VastEditCorpusCase {
            id: "c-ident-rename",
            before_bytes: before,
            edits: &edits,
            changed_ranges: &changed_ranges,
            updated_vast: &updated_vast,
            full_reparse_vast: &full_reparse_vast,
            diagnostics: b"ok",
            reused_node_count: 2,
        };

        let error = vast_edit_corpus_evidence(&case).expect_err("mismatch must reject");

        assert!(matches!(
            error,
            VastEditCorpusError::FullReparseMismatch { .. }
        ));
    }

    #[test]
    fn edit_script_rejects_overlapping_ranges() {
        let before = b"abcdef";
        let edits = [
            VastEdit {
                old_start: 1,
                old_end: 4,
                replacement: b"x",
            },
            VastEdit {
                old_start: 3,
                old_end: 5,
                replacement: b"y",
            },
        ];

        let error = apply_vast_edit_script(before, &edits).expect_err("overlap must reject");

        assert!(matches!(error, VastEditCorpusError::OverlappingEdit { .. }));
    }
}

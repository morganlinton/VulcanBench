//! Generic resident workspace region layout for megakernel adapters.
//!
//! Domain adapters own their region identifiers and capacity policy. Runtime
//! owns the checked contiguous-layout arithmetic because every resident
//! megakernel workspace has the same ABI shape: ordered u32-word regions with
//! fixed record widths and explicit capacities.

/// One contiguous u32-word region inside a resident megakernel workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelWorkspaceRegion<R> {
    /// Domain-owned region id encoded in the workspace manifest.
    pub id: R,
    /// Offset from workspace word zero.
    pub offset_words: u32,
    /// Total words reserved for the region.
    pub words: u32,
    /// Words in one logical record for this region.
    pub record_words: u32,
    /// Logical record capacity for this region.
    pub capacity_records: u32,
}

/// Declarative region specification for bulk resident-workspace layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MegakernelWorkspaceRegionSpec<R> {
    /// Region whose total word count is already known.
    Fixed {
        /// Domain-owned region id encoded in the workspace manifest.
        id: R,
        /// Total words reserved for the region.
        words: u32,
        /// Words in one logical record for this region.
        record_words: u32,
        /// Logical record capacity for this region.
        capacity_records: u32,
    },
    /// Region sized as `record_words * capacity_records`.
    Record {
        /// Domain-owned region id encoded in the workspace manifest.
        id: R,
        /// Words in one logical record for this region.
        record_words: u32,
        /// Logical record capacity for this region.
        capacity_records: u32,
    },
}

impl<R> MegakernelWorkspaceRegionSpec<R> {
    /// Build a fixed-size region specification.
    #[must_use]
    pub const fn fixed(id: R, words: u32, record_words: u32, capacity_records: u32) -> Self {
        Self::Fixed {
            id,
            words,
            record_words,
            capacity_records,
        }
    }

    /// Build a record-backed region specification.
    #[must_use]
    pub const fn record(id: R, record_words: u32, capacity_records: u32) -> Self {
        Self::Record {
            id,
            record_words,
            capacity_records,
        }
    }
}

/// Error returned by bulk resident-workspace layout planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MegakernelWorkspaceLayoutError<R> {
    /// `record_words * capacity_records` overflowed for this record-backed region.
    RecordWordsOverflow {
        /// Region whose record arena overflowed.
        region: R,
    },
    /// Contiguous region offset arithmetic overflowed for this region.
    OffsetOverflow {
        /// Region whose starting offset could not fit the accumulated layout.
        region: R,
    },
}

impl<R: Copy> MegakernelWorkspaceRegion<R> {
    /// Exclusive end offset for this region.
    #[must_use]
    pub const fn end_words(self) -> Option<u32> {
        self.offset_words.checked_add(self.words)
    }
}

/// Return `record_words * capacity_records` for a record-backed region.
#[must_use]
pub const fn workspace_record_words(record_words: u32, capacity_records: u32) -> Option<u32> {
    record_words.checked_mul(capacity_records)
}

/// Build the first region in a resident workspace.
#[must_use]
pub const fn first_workspace_region<R>(
    id: R,
    words: u32,
    record_words: u32,
    capacity_records: u32,
) -> MegakernelWorkspaceRegion<R> {
    MegakernelWorkspaceRegion {
        id,
        offset_words: 0,
        words,
        record_words,
        capacity_records,
    }
}

/// Build the next contiguous region after `previous`.
#[must_use]
pub fn next_workspace_region<R: Copy>(
    previous: MegakernelWorkspaceRegion<R>,
    id: R,
    words: u32,
    record_words: u32,
    capacity_records: u32,
) -> Option<MegakernelWorkspaceRegion<R>> {
    Some(MegakernelWorkspaceRegion {
        id,
        offset_words: previous.end_words()?,
        words,
        record_words,
        capacity_records,
    })
}

/// Build the next contiguous record-backed region after `previous`.
#[must_use]
pub fn next_record_workspace_region<R: Copy>(
    previous: MegakernelWorkspaceRegion<R>,
    id: R,
    record_words: u32,
    capacity_records: u32,
) -> Option<MegakernelWorkspaceRegion<R>> {
    next_workspace_region(
        previous,
        id,
        workspace_record_words(record_words, capacity_records)?,
        record_words,
        capacity_records,
    )
}

/// Build a contiguous resident-workspace layout from declarative specs.
///
/// This is the generic seam domain adapters should use when they own many
/// regions. It centralizes checked record multiplication and checked offset
/// accumulation in `vyre-runtime`, while each adapter keeps its own region ids
/// and capacity policy.
pub fn build_workspace_regions<R: Copy>(
    specs: &[MegakernelWorkspaceRegionSpec<R>],
) -> Result<Vec<MegakernelWorkspaceRegion<R>>, MegakernelWorkspaceLayoutError<R>> {
    let mut regions = Vec::with_capacity(specs.len());
    let mut next_offset_words = 0_u32;

    for spec in specs {
        let (id, words, record_words, capacity_records) = match *spec {
            MegakernelWorkspaceRegionSpec::Fixed {
                id,
                words,
                record_words,
                capacity_records,
            } => (id, words, record_words, capacity_records),
            MegakernelWorkspaceRegionSpec::Record {
                id,
                record_words,
                capacity_records,
            } => {
                let words = workspace_record_words(record_words, capacity_records)
                    .ok_or(MegakernelWorkspaceLayoutError::RecordWordsOverflow { region: id })?;
                (id, words, record_words, capacity_records)
            }
        };
        let end_words = next_offset_words
            .checked_add(words)
            .ok_or(MegakernelWorkspaceLayoutError::OffsetOverflow { region: id })?;
        regions.push(MegakernelWorkspaceRegion {
            id,
            offset_words: next_offset_words,
            words,
            record_words,
            capacity_records,
        });
        next_offset_words = end_words;
    }

    Ok(regions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Region {
        Header,
        Rows,
        Work,
    }

    #[test]
    fn generated_workspace_regions_are_contiguous_for_many_capacities() {
        for rows in [1_u32, 2, 7, 8, 31, 32, 1024] {
            for work in [1_u32, 3, 64, 4096] {
                let header = first_workspace_region(Region::Header, 16, 1, 16);
                let rows = next_record_workspace_region(header, Region::Rows, 5, rows)
                    .expect("Fix: generated row region should fit");
                let work = next_record_workspace_region(rows, Region::Work, 2, work)
                    .expect("Fix: generated work region should fit");

                assert_eq!(header.offset_words, 0);
                assert_eq!(rows.offset_words, header.end_words().unwrap());
                assert_eq!(work.offset_words, rows.end_words().unwrap());
            }
        }
    }

    #[test]
    fn record_region_word_overflow_is_reported_before_offset_overflow() {
        let header = first_workspace_region(Region::Header, 16, 1, 16);

        assert_eq!(
            workspace_record_words(u32::MAX, 2),
            None,
            "record sizing must catch multiplication overflow"
        );
        assert_eq!(
            next_record_workspace_region(header, Region::Rows, u32::MAX, 2),
            None,
            "record-backed layout must reject overflowing record arenas"
        );
    }

    #[test]
    fn next_region_rejects_offset_overflow() {
        let previous = MegakernelWorkspaceRegion {
            id: Region::Header,
            offset_words: u32::MAX,
            words: 1,
            record_words: 1,
            capacity_records: 1,
        };

        assert_eq!(
            next_workspace_region(previous, Region::Rows, 1, 1, 1),
            None,
            "contiguous layout must reject overflowing end offsets"
        );
    }

    #[test]
    fn bulk_workspace_region_builder_matches_chained_layout_for_generated_capacities() {
        for rows in [1_u32, 2, 7, 8, 31, 32, 1024] {
            for work in [1_u32, 3, 64, 4096] {
                let specs = [
                    MegakernelWorkspaceRegionSpec::fixed(Region::Header, 16, 1, 16),
                    MegakernelWorkspaceRegionSpec::record(Region::Rows, 5, rows),
                    MegakernelWorkspaceRegionSpec::record(Region::Work, 2, work),
                ];
                let bulk = build_workspace_regions(&specs)
                    .expect("Fix: generated bulk workspace layout should fit");

                let header = first_workspace_region(Region::Header, 16, 1, 16);
                let rows = next_record_workspace_region(header, Region::Rows, 5, rows)
                    .expect("Fix: generated row region should fit");
                let work = next_record_workspace_region(rows, Region::Work, 2, work)
                    .expect("Fix: generated work region should fit");

                assert_eq!(bulk, vec![header, rows, work]);
            }
        }
    }

    #[test]
    fn bulk_workspace_region_builder_reports_record_and_offset_overflow_separately() {
        let record = [MegakernelWorkspaceRegionSpec::record(
            Region::Rows,
            u32::MAX,
            2,
        )];
        assert_eq!(
            build_workspace_regions(&record),
            Err(MegakernelWorkspaceLayoutError::RecordWordsOverflow {
                region: Region::Rows
            })
        );

        let offset = [
            MegakernelWorkspaceRegionSpec::fixed(Region::Header, u32::MAX, 1, u32::MAX),
            MegakernelWorkspaceRegionSpec::fixed(Region::Rows, 1, 1, 1),
        ];
        assert_eq!(
            build_workspace_regions(&offset),
            Err(MegakernelWorkspaceLayoutError::OffsetOverflow {
                region: Region::Rows
            })
        );
    }
}

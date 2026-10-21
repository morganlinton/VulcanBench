//! Shared state/index frontier model for graph and automata traversal.
//!
//! Sparse graph frontiers and irregular automata frontiers both move compact
//! `(state, index)` work items. This module owns the shared header, compaction
//! rules, duplicate handling, and spill evidence so graph and automata engines
//! do not invent incompatible queue metadata.

/// Schema version for serialized state/index frontier headers.
pub const STATE_INDEX_FRONTIER_SCHEMA_VERSION: u32 = 1;

/// Workload domain using a state/index frontier.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateIndexFrontierDomain {
    /// Graph traversal where `state_id` is a graph node or graph-local state.
    Graph = 1,
    /// Automata traversal where `state_id` is an automata state.
    Automata = 2,
}

impl StateIndexFrontierDomain {
    /// Stable label for reports and serialized evidence.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Graph => "graph",
            Self::Automata => "automata",
        }
    }
}

/// Duplicate handling rule for compacting a state/index frontier.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateIndexDuplicateRule {
    /// Preserve all entries exactly as supplied.
    Preserve = 0,
    /// Drop exact duplicate `(state_id, index)` pairs.
    DropExactStateIndex = 1,
    /// Keep the lowest index for each state id.
    DropStateKeepLowestIndex = 2,
}

impl StateIndexDuplicateRule {
    /// Stable label for reports and serialized evidence.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preserve => "preserve",
            Self::DropExactStateIndex => "drop_exact_state_index",
            Self::DropStateKeepLowestIndex => "drop_state_keep_lowest_index",
        }
    }
}

/// Resident-capacity decision after compaction.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateIndexSpillDecision {
    /// Compacted frontier fits the resident queue capacity.
    Resident = 0,
    /// Compacted frontier exceeds resident capacity and must spill or shard.
    SpillRequired = 1,
}

impl StateIndexSpillDecision {
    /// Stable label for reports and serialized evidence.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resident => "resident",
            Self::SpillRequired => "spill_required",
        }
    }
}

/// One shared frontier item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateIndexFrontierItem {
    /// State id, graph node id, or graph-local state id.
    pub state_id: u32,
    /// Input byte index, graph depth index, or domain-local offset.
    pub index: u32,
}

impl StateIndexFrontierItem {
    /// Construct a shared state/index frontier item.
    #[must_use]
    pub const fn new(state_id: u32, index: u32) -> Self {
        Self { state_id, index }
    }
}

/// Serialized header and evidence for one compacted state/index frontier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateIndexFrontierHeader {
    /// Header schema version.
    pub schema_version: u32,
    /// Workload domain using the frontier.
    pub domain: StateIndexFrontierDomain,
    /// Input entries before compaction.
    pub input_len: u32,
    /// Entries after applying the duplicate rule.
    pub compacted_len: u32,
    /// Resident queue capacity available to this frontier.
    pub capacity: u32,
    /// Number of entries removed by compaction.
    pub duplicate_count: u32,
    /// Duplicate handling rule applied during compaction.
    pub duplicate_rule: StateIndexDuplicateRule,
    /// Resident spill decision after compaction.
    pub spill_decision: StateIndexSpillDecision,
}

impl StateIndexFrontierHeader {
    /// Serialize the header into fixed u32 words.
    #[must_use]
    pub const fn to_words(self) -> [u32; 8] {
        [
            self.schema_version,
            self.domain as u32,
            self.input_len,
            self.compacted_len,
            self.capacity,
            self.duplicate_count,
            self.duplicate_rule as u32,
            self.spill_decision as u32,
        ]
    }

    /// Return true when the header carries the shared graph/automata contract.
    #[must_use]
    pub fn is_complete(self) -> bool {
        self.schema_version == STATE_INDEX_FRONTIER_SCHEMA_VERSION
            && self.input_len >= self.compacted_len
            && self.duplicate_count == self.input_len.saturating_sub(self.compacted_len)
            && match self.spill_decision {
                StateIndexSpillDecision::Resident => self.compacted_len <= self.capacity,
                StateIndexSpillDecision::SpillRequired => self.compacted_len > self.capacity,
            }
    }
}

/// Compact state/index frontier items into caller-owned output storage.
///
/// # Errors
///
/// Returns an actionable error when the frontier length cannot fit the u32
/// header or output storage cannot be reserved.
pub fn try_compact_state_index_frontier_into(
    domain: StateIndexFrontierDomain,
    items: &[StateIndexFrontierItem],
    capacity: u32,
    duplicate_rule: StateIndexDuplicateRule,
    out: &mut Vec<StateIndexFrontierItem>,
) -> Result<StateIndexFrontierHeader, String> {
    let input_len = u32::try_from(items.len()).map_err(|source| {
        format!(
            "state-index frontier length cannot fit u32: {source}. Fix: shard graph or automata frontier chunks before compaction."
        )
    })?;
    if out.capacity() < items.len() {
        out.try_reserve(items.len() - out.capacity()).map_err(|source| {
            format!(
                "state-index frontier output reservation failed: {source}. Fix: reduce resident frontier batch size or spill before compaction."
            )
        })?;
    }
    out.clear();
    out.extend_from_slice(items);
    match duplicate_rule {
        StateIndexDuplicateRule::Preserve => {}
        StateIndexDuplicateRule::DropExactStateIndex => {
            out.sort_unstable();
            out.dedup();
        }
        StateIndexDuplicateRule::DropStateKeepLowestIndex => {
            out.sort_unstable();
            out.dedup_by_key(|item| item.state_id);
        }
    }
    let compacted_len = u32::try_from(out.len()).map_err(|source| {
        format!(
            "state-index compacted frontier length cannot fit u32: {source}. Fix: shard graph or automata frontier chunks after compaction."
        )
    })?;
    let spill_decision = if compacted_len > capacity {
        StateIndexSpillDecision::SpillRequired
    } else {
        StateIndexSpillDecision::Resident
    };
    Ok(StateIndexFrontierHeader {
        schema_version: STATE_INDEX_FRONTIER_SCHEMA_VERSION,
        domain,
        input_len,
        compacted_len,
        capacity,
        duplicate_count: input_len.saturating_sub(compacted_len),
        duplicate_rule,
        spill_decision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_and_automata_headers_share_serialized_shape() {
        let graph = StateIndexFrontierHeader {
            schema_version: STATE_INDEX_FRONTIER_SCHEMA_VERSION,
            domain: StateIndexFrontierDomain::Graph,
            input_len: 4,
            compacted_len: 3,
            capacity: 8,
            duplicate_count: 1,
            duplicate_rule: StateIndexDuplicateRule::DropExactStateIndex,
            spill_decision: StateIndexSpillDecision::Resident,
        };
        let automata = StateIndexFrontierHeader {
            domain: StateIndexFrontierDomain::Automata,
            ..graph
        };

        assert_eq!(graph.to_words().len(), automata.to_words().len());
        assert_eq!(graph.to_words()[0], automata.to_words()[0]);
        assert_eq!(graph.to_words()[2..], automata.to_words()[2..]);
        assert!(graph.is_complete());
        assert!(automata.is_complete());
    }

    #[test]
    fn exact_duplicate_compaction_reports_spill_without_dropping_unique_work() {
        let items = [
            StateIndexFrontierItem::new(3, 30),
            StateIndexFrontierItem::new(1, 10),
            StateIndexFrontierItem::new(3, 30),
            StateIndexFrontierItem::new(2, 20),
        ];
        let mut out = Vec::new();

        let header = try_compact_state_index_frontier_into(
            StateIndexFrontierDomain::Graph,
            &items,
            2,
            StateIndexDuplicateRule::DropExactStateIndex,
            &mut out,
        )
        .expect("Fix: exact duplicate compaction should fit header words");

        assert_eq!(
            out,
            vec![
                StateIndexFrontierItem::new(1, 10),
                StateIndexFrontierItem::new(2, 20),
                StateIndexFrontierItem::new(3, 30),
            ]
        );
        assert_eq!(header.input_len, 4);
        assert_eq!(header.compacted_len, 3);
        assert_eq!(header.duplicate_count, 1);
        assert_eq!(header.spill_decision, StateIndexSpillDecision::SpillRequired);
        assert!(header.is_complete());
    }

    #[test]
    fn duplicate_state_rule_keeps_lowest_index_per_state() {
        let items = [
            StateIndexFrontierItem::new(7, 70),
            StateIndexFrontierItem::new(7, 3),
            StateIndexFrontierItem::new(8, 80),
        ];
        let mut out = Vec::new();

        let header = try_compact_state_index_frontier_into(
            StateIndexFrontierDomain::Automata,
            &items,
            4,
            StateIndexDuplicateRule::DropStateKeepLowestIndex,
            &mut out,
        )
        .expect("Fix: state-level duplicate compaction should fit header words");

        assert_eq!(
            out,
            vec![
                StateIndexFrontierItem::new(7, 3),
                StateIndexFrontierItem::new(8, 80),
            ]
        );
        assert_eq!(header.compacted_len, 2);
        assert_eq!(header.duplicate_count, 1);
        assert_eq!(header.spill_decision, StateIndexSpillDecision::Resident);
        assert!(header.is_complete());
    }
}

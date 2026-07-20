//! Shared dataflow graph-layout coverage validation.

use std::collections::BTreeSet;

/// Dataflow analysis consumer that must use the shared graph-layout module.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DataflowGraphLayoutConsumer {
    /// Dominator analysis.
    Dominators,
    /// Callgraph.
    Callgraph,
    /// IFDS.
    Ifds,
    /// Slicing.
    Slicing,
    /// Range propagation.
    RangePropagation,
}

/// Compatibility check required for shared graph layouts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DataflowGraphLayoutCheck {
    /// Stable layout hash.
    StableHash,
    /// Edge encoding family check.
    EdgeEncodingFamily,
    /// Normalization for duplicate/unsorted edges.
    Normalization,
}

/// One shared graph-layout evidence record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataflowGraphLayoutRecord<'a> {
    /// Consumer.
    pub consumer: DataflowGraphLayoutConsumer,
    /// Compatibility check.
    pub check: DataflowGraphLayoutCheck,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path.
    pub evidence: &'a str,
}

/// Shared graph-layout proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataflowGraphLayoutCoverageProof {
    /// Consumer count.
    pub consumer_count: usize,
    /// Record count.
    pub record_count: usize,
}

/// Shared graph-layout validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataflowGraphLayoutCoverageError {
    /// No records supplied.
    EmptyRecords,
    /// Metadata is empty.
    EmptyMetadata {
        /// Consumer.
        consumer: DataflowGraphLayoutConsumer,
        /// Field.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Consumer.
        consumer: DataflowGraphLayoutConsumer,
        /// Command.
        command: String,
    },
    /// Required consumer is missing.
    MissingConsumer {
        /// Consumer.
        consumer: DataflowGraphLayoutConsumer,
    },
    /// Required compatibility check is missing.
    MissingCheck {
        /// Consumer.
        consumer: DataflowGraphLayoutConsumer,
        /// Check.
        check: DataflowGraphLayoutCheck,
    },
}

impl std::fmt::Display for DataflowGraphLayoutCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "Dataflow graph-layout coverage is empty. Fix: prove dominators, callgraph, IFDS, slicing, and range propagation use shared layout checks."
            ),
            Self::EmptyMetadata { consumer, field } => write!(
                f,
                "Dataflow graph-layout record {consumer:?} has empty {field}. Fix: every record needs command and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { consumer, command } => write!(
                f,
                "Dataflow graph-layout record {consumer:?} uses `{command}` instead of ./cargo_full. Fix: run graph-layout checks through cargo_full."
            ),
            Self::MissingConsumer { consumer } => write!(
                f,
                "Dataflow graph-layout coverage is missing {consumer:?}. Fix: route that consumer through the shared graph-layout contract."
            ),
            Self::MissingCheck { consumer, check } => write!(
                f,
                "Dataflow graph-layout coverage {consumer:?} is missing {check:?}. Fix: add that compatibility check."
            ),
        }
    }
}

impl std::error::Error for DataflowGraphLayoutCoverageError {}

const REQUIRED_CONSUMERS: &[DataflowGraphLayoutConsumer] = &[
    DataflowGraphLayoutConsumer::Dominators,
    DataflowGraphLayoutConsumer::Callgraph,
    DataflowGraphLayoutConsumer::Ifds,
    DataflowGraphLayoutConsumer::Slicing,
    DataflowGraphLayoutConsumer::RangePropagation,
];

const REQUIRED_CHECKS: &[DataflowGraphLayoutCheck] = &[
    DataflowGraphLayoutCheck::StableHash,
    DataflowGraphLayoutCheck::EdgeEncodingFamily,
    DataflowGraphLayoutCheck::Normalization,
];

/// Validate shared Dataflow graph-layout coverage.
pub fn validate_graph_layout_coverage(
    records: &[DataflowGraphLayoutRecord<'_>],
) -> Result<DataflowGraphLayoutCoverageProof, DataflowGraphLayoutCoverageError> {
    if records.is_empty() {
        return Err(DataflowGraphLayoutCoverageError::EmptyRecords);
    }
    let mut seen_consumers = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    for record in records {
        for (field, value) in [("command", record.command), ("evidence", record.evidence)] {
            if value.trim().is_empty() {
                return Err(DataflowGraphLayoutCoverageError::EmptyMetadata {
                    consumer: record.consumer,
                    field,
                });
            }
        }
        if !record.command.trim_start().starts_with("./cargo_full ") {
            return Err(
                DataflowGraphLayoutCoverageError::CommandDoesNotUseCargoFull {
                    consumer: record.consumer,
                    command: record.command.to_owned(),
                },
            );
        }
        seen_consumers.insert(record.consumer);
        pairs.insert((record.consumer, record.check));
    }
    for consumer in REQUIRED_CONSUMERS {
        if !seen_consumers.contains(consumer) {
            return Err(DataflowGraphLayoutCoverageError::MissingConsumer {
                consumer: *consumer,
            });
        }
        for check in REQUIRED_CHECKS {
            if !pairs.contains(&(*consumer, *check)) {
                return Err(DataflowGraphLayoutCoverageError::MissingCheck {
                    consumer: *consumer,
                    check: *check,
                });
            }
        }
    }
    Ok(DataflowGraphLayoutCoverageProof {
        consumer_count: seen_consumers.len(),
        record_count: records.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_layout_coverage_accepts_all_consumers_and_checks() {
        let proof = validate_graph_layout_coverage(&records())
            .expect("Fix: complete graph-layout coverage should pass");

        assert_eq!(proof.consumer_count, 5);
        assert_eq!(proof.record_count, 15);
    }

    #[test]
    fn graph_layout_coverage_rejects_missing_range_propagation() {
        let records: Vec<_> = records()
            .into_iter()
            .filter(|record| record.consumer != DataflowGraphLayoutConsumer::RangePropagation)
            .collect();

        assert_eq!(
            validate_graph_layout_coverage(&records)
                .expect_err("missing range propagation should fail"),
            DataflowGraphLayoutCoverageError::MissingConsumer {
                consumer: DataflowGraphLayoutConsumer::RangePropagation,
            }
        );
    }

    #[test]
    fn graph_layout_coverage_rejects_missing_check_and_raw_cargo() {
        let mut missing_check_records = records();
        missing_check_records.retain(|record| {
            !(record.consumer == DataflowGraphLayoutConsumer::Ifds
                && record.check == DataflowGraphLayoutCheck::Normalization)
        });
        assert_eq!(
            validate_graph_layout_coverage(&missing_check_records)
                .expect_err("missing normalization should fail"),
            DataflowGraphLayoutCoverageError::MissingCheck {
                consumer: DataflowGraphLayoutConsumer::Ifds,
                check: DataflowGraphLayoutCheck::Normalization,
            }
        );

        let mut raw_cargo_records = records();
        raw_cargo_records[0].command = "cargo test";
        assert_eq!(
            validate_graph_layout_coverage(&raw_cargo_records).expect_err("raw cargo should fail"),
            DataflowGraphLayoutCoverageError::CommandDoesNotUseCargoFull {
                consumer: DataflowGraphLayoutConsumer::Dominators,
                command: "cargo test".to_owned(),
            }
        );
    }

    fn records() -> Vec<DataflowGraphLayoutRecord<'static>> {
        let mut records = Vec::new();
        for consumer in REQUIRED_CONSUMERS {
            for check in REQUIRED_CHECKS {
                records.push(DataflowGraphLayoutRecord {
                    consumer: *consumer,
                    check: *check,
                    command: "./cargo_full test -j1 -p dataflow",
                    evidence: "release/dataflow/graph-layout.md",
                });
            }
        }
        records
    }
}

//! Composition metadata helpers for fused programs.
//!
//! Some generated regions are intentionally not composable with another
//! instance of themselves inside the same fused kernel. We encode that
//! contract in the region generator string so both validation and
//! optimization passes can enforce it without needing a new IR field.

use crate::ir::Node;
use rustc_hash::FxHashMap;

/// Generator suffix marking a region as non-composable with itself.
pub const SELF_EXCLUSIVE_REGION_SUFFIX: &str = "#self-exclusive";

/// Append the self-exclusive marker to a generator id.
#[must_use]
pub fn mark_self_exclusive_region(generator: &str) -> String {
    format!("{generator}{SELF_EXCLUSIVE_REGION_SUFFIX}")
}

/// Return the base generator id when this region is self-exclusive.
#[must_use]
pub fn self_exclusive_region_key(generator: &str) -> Option<&str> {
    generator.strip_suffix(SELF_EXCLUSIVE_REGION_SUFFIX)
}

/// Return duplicate self-exclusive generators present in one program.
#[must_use]
pub fn duplicate_self_exclusive_regions(nodes: &[Node]) -> Vec<String> {
    let mut counts = FxHashMap::<&str, usize>::default();
    collect_self_exclusive_regions(nodes, &mut counts);
    let mut duplicates = counts
        .into_iter()
        .filter_map(|(generator, count)| (count > 1).then_some(generator.to_string()))
        .collect::<Vec<_>>();
    duplicates.sort();
    duplicates
}

fn collect_self_exclusive_regions<'a>(nodes: &'a [Node], counts: &mut FxHashMap<&'a str, usize>) {
    for node in nodes {
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                collect_self_exclusive_regions(then, counts);
                collect_self_exclusive_regions(otherwise, counts);
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                collect_self_exclusive_regions(body, counts);
            }
            Node::Region {
                generator, body, ..
            } => {
                if let Some(base) = self_exclusive_region_key(generator.as_str()) {
                    *counts.entry(base).or_insert(0) += 1;
                }
                collect_self_exclusive_regions(body, counts);
            }
            Node::Let { .. }
            | Node::Assign { .. }
            | Node::Store { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. }
            | Node::Return
            | Node::Barrier { .. }
            | Node::IndirectDispatch { .. }
            | Node::AsyncLoad { .. }
            | Node::AsyncStore { .. }
            | Node::AsyncWait { .. }
            | Node::Trap { .. }
            | Node::Resume { .. }
            | Node::Opaque(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Node, Program};
    use std::sync::Arc;

    #[test]
    fn duplicate_self_exclusive_regions_are_reported() {
        let generator = mark_self_exclusive_region("vyre.test.parser");
        let program = Program::wrapped(
            Vec::new(),
            [1, 1, 1],
            vec![
                Node::Region {
                    generator: generator.clone().into(),
                    source_region: None,
                    body: Arc::new(vec![Node::Return]),
                },
                Node::Region {
                    generator: generator.into(),
                    source_region: None,
                    body: Arc::new(vec![Node::Return]),
                },
                Node::Region {
                    generator: "plain.region".into(),
                    source_region: None,
                    body: Arc::new(vec![Node::Return]),
                },
            ],
        );
        assert_eq!(
            duplicate_self_exclusive_regions(program.entry()),
            vec!["vyre.test.parser".to_string()]
        );
    }
}

//! Derived optimizer pass ordering from live pass metadata.
//!
//! This is the S10 artifact: a foundation-owned, machine-checkable pass order
//! derived from the same metadata emitted by `#[vyre_pass]` inventory
//! registrations. The scheduler remains the executor; this module is the
//! auditable certificate surface release tooling and tests can inspect.

use super::scheduler::schedule_pass_metadata_indices;
use super::{registered_pass_registrations, OptimizerError, PassMetadata, PassSchedulingError};

/// One pass in the derived optimizer order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivedPassNode {
    /// Zero-based position in the derived execution order.
    pub position: usize,
    /// Stable pass name.
    pub name: &'static str,
    /// Original static pass metadata.
    pub metadata: PassMetadata,
}

/// One dependency edge in the derived pass-order graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivedPassEdge {
    /// Pass that must appear earlier in the derived order.
    pub before: &'static str,
    /// Pass that depends on `before`.
    pub after: &'static str,
    /// Why the edge exists.
    pub kind: DerivedPassEdgeKind,
}

/// Source of a derived pass-order edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedPassEdgeKind {
    /// Direct `requires = ["..."]` precondition declared by a pass.
    DeclaredRequirement,
    /// Causal edge: one pass invalidates a capability another pass requires.
    CausalInvalidation,
}

/// Derived pass order plus causal back-door evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedPassOrder {
    nodes: Vec<DerivedPassNode>,
    metadata: Vec<PassMetadata>,
    declared_edges: Vec<DerivedPassEdge>,
    causal_edges: Vec<DerivedPassEdge>,
    causal_adjacency: Vec<u32>,
    causal_safe_pair_checks: usize,
}

impl DerivedPassOrder {
    /// Passes in execution order.
    #[must_use]
    pub fn nodes(&self) -> &[DerivedPassNode] {
        &self.nodes
    }

    /// Ordered metadata consumed by validators.
    #[must_use]
    pub fn metadata(&self) -> &[PassMetadata] {
        &self.metadata
    }

    /// Declared requirement edges used to derive topological order.
    #[must_use]
    pub fn declared_edges(&self) -> &[DerivedPassEdge] {
        &self.declared_edges
    }

    /// Causal invalidation edges used by back-door safety checks.
    #[must_use]
    pub fn causal_edges(&self) -> &[DerivedPassEdge] {
        &self.causal_edges
    }

    /// Row-major causal adjacency over [`Self::nodes`].
    #[must_use]
    pub fn causal_adjacency(&self) -> &[u32] {
        &self.causal_adjacency
    }

    /// Number of ordered pass pairs checked by the causal back-door criterion.
    #[must_use]
    pub fn causal_safe_pair_checks(&self) -> usize {
        self.causal_safe_pair_checks
    }

    /// Number of passes in the derived order.
    #[must_use]
    pub fn pass_count(&self) -> usize {
        self.nodes.len()
    }

    /// Total number of derived graph edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.declared_edges.len() + self.causal_edges.len()
    }
}

/// Derive the live registered optimizer pass order.
///
/// # Errors
///
/// Returns [`OptimizerError`] when the registered inventory declares duplicate
/// pass IDs, unknown requirements, or a cyclic order.
pub fn derive_registered_pass_order() -> Result<DerivedPassOrder, OptimizerError> {
    let registrations = registered_pass_registrations()?;
    let metadata = registrations
        .iter()
        .map(|registration| registration.metadata)
        .collect::<Vec<_>>();
    derive_pass_order(&metadata).map_err(OptimizerError::from)
}

/// Derive a pass order from unsorted pass metadata.
///
/// # Errors
///
/// Returns [`PassSchedulingError`] when pass IDs are duplicated, requirements
/// are unknown, or the declared dependency graph is cyclic.
pub fn derive_pass_order(
    metadata: &[PassMetadata],
) -> Result<DerivedPassOrder, PassSchedulingError> {
    let order = schedule_pass_metadata_indices(metadata)?;
    let ordered = order
        .into_iter()
        .map(|index| metadata[index])
        .collect::<Vec<_>>();
    Ok(build_derived_order(ordered))
}

fn build_derived_order(metadata: Vec<PassMetadata>) -> DerivedPassOrder {
    let nodes = metadata
        .iter()
        .enumerate()
        .map(|(position, &pass)| DerivedPassNode {
            position,
            name: pass.name,
            metadata: pass,
        })
        .collect::<Vec<_>>();
    let declared_edges = declared_edges(&metadata);
    let (causal_edges, causal_adjacency) = causal_edges_and_adjacency(&metadata);
    let causal_safe_pair_checks = count_causal_safe_pairs(&causal_adjacency, metadata.len());
    DerivedPassOrder {
        nodes,
        metadata,
        declared_edges,
        causal_edges,
        causal_adjacency,
        causal_safe_pair_checks,
    }
}

fn declared_edges(metadata: &[PassMetadata]) -> Vec<DerivedPassEdge> {
    let mut edges = Vec::new();
    for pass in metadata {
        for &requirement in pass.requires {
            edges.push(DerivedPassEdge {
                before: requirement,
                after: pass.name,
                kind: DerivedPassEdgeKind::DeclaredRequirement,
            });
        }
    }
    edges
}

fn causal_edges_and_adjacency(metadata: &[PassMetadata]) -> (Vec<DerivedPassEdge>, Vec<u32>) {
    let n = metadata.len();
    let mut edges = Vec::new();
    let mut adjacency = vec![0u32; n.saturating_mul(n)];
    for (before_index, before) in metadata.iter().enumerate() {
        for (after_index, after) in metadata.iter().enumerate() {
            if before_index == after_index {
                continue;
            }
            let invalidates_required = before.invalidates.iter().any(|invalidated| {
                after
                    .requires
                    .iter()
                    .any(|required| required == invalidated)
            });
            if invalidates_required {
                adjacency[before_index * n + after_index] = 1;
                edges.push(DerivedPassEdge {
                    before: before.name,
                    after: after.name,
                    kind: DerivedPassEdgeKind::CausalInvalidation,
                });
            }
        }
    }
    (edges, adjacency)
}

fn count_causal_safe_pairs(adjacency: &[u32], n: usize) -> usize {
    let Ok(n_u32) = u32::try_from(n) else {
        return 0;
    };
    let mut checks = 0usize;
    for before in 0..n {
        for after in (before + 1)..n {
            if crate::pass_substrate::adjustment_set_pass_dependency::ordering_is_safe(
                adjacency,
                before as u32,
                after as u32,
                n_u32,
            ) {
                checks = checks.saturating_add(1);
            }
        }
    }
    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(
        name: &'static str,
        requires: &'static [&'static str],
        invalidates: &'static [&'static str],
    ) -> PassMetadata {
        PassMetadata::new(name, requires, invalidates)
    }

    #[test]
    fn derives_topological_order_from_unsorted_metadata() {
        let derived = derive_pass_order(&[
            meta("rewrite", &["facts"], &[]),
            meta("facts", &[], &["facts"]),
        ])
        .expect("Fix: derived pass order must topologically sort declared requirements");
        assert_eq!(
            derived
                .nodes()
                .iter()
                .map(|node| node.name)
                .collect::<Vec<_>>(),
            vec!["facts", "rewrite"]
        );
        assert_eq!(derived.declared_edges().len(), 1);
    }

    #[test]
    fn emits_causal_invalidation_edges_and_adjacency() {
        let derived = derive_pass_order(&[
            meta("shape", &[], &["shape"]),
            meta("consumer", &["shape"], &[]),
        ])
        .expect("Fix: derived order must accept direct fact producer before consumer");
        assert_eq!(derived.causal_edges().len(), 1);
        assert_eq!(derived.causal_edges()[0].before, "shape");
        assert_eq!(derived.causal_edges()[0].after, "consumer");
        assert_eq!(derived.causal_adjacency(), &[0, 1, 0, 0]);
        assert_eq!(derived.causal_safe_pair_checks(), 1);
    }

    #[test]
    fn rejects_unknown_requirement_before_building_artifact() {
        let error = derive_pass_order(&[meta("consumer", &["missing"], &[])])
            .expect_err("Fix: missing requirement must reject derived-order artifact");
        assert_eq!(
            error,
            PassSchedulingError::UnknownRequire {
                pass: "consumer",
                missing: "missing",
            }
        );
    }

    #[test]
    fn live_registered_order_derives() {
        let derived = derive_registered_pass_order()
            .expect("Fix: live registered pass order must derive from inventory metadata");
        assert!(derived.pass_count() > 0);
        assert_eq!(derived.nodes().len(), derived.metadata().len());
    }
}

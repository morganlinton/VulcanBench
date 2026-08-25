//! Include-graph evidence model for GPU-resident preprocessing.

/// Execution residency for an include-graph fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IncludeGraphResidency {
    /// Fact was produced by GPU-resident include discovery.
    GpuResident,
    /// Fact was provided as unavoidable host filesystem metadata.
    HostFilesystemMetadata,
    /// Fact was reused from the translation-unit host byte cache.
    HostMemoryCache,
    /// Fact was produced by an oracle/test-only CPU path.
    CpuOracle,
}

/// One include-graph edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeGraphEdge {
    /// Including source path.
    pub includer: String,
    /// Included source/header path or unresolved include spelling.
    pub includee: String,
    /// Include directive byte offset in the includer.
    pub directive_byte_offset: u64,
    /// Whether the include was resolved to a concrete file.
    pub resolved: bool,
    /// Residency of the edge producer.
    pub residency: IncludeGraphResidency,
}

impl IncludeGraphEdge {
    /// Creates one include-graph edge.
    #[must_use]
    pub fn new(
        includer: impl Into<String>,
        includee: impl Into<String>,
        directive_byte_offset: u64,
        resolved: bool,
        residency: IncludeGraphResidency,
    ) -> Self {
        Self {
            includer: includer.into(),
            includee: includee.into(),
            directive_byte_offset,
            resolved,
            residency,
        }
    }
}

/// Include-graph proof for one preprocessing run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeGraphProof {
    /// Root translation unit.
    pub root: String,
    /// Include edges discovered for the root.
    pub edges: Vec<IncludeGraphEdge>,
}

impl IncludeGraphProof {
    /// Creates an empty include-graph proof.
    #[must_use]
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            edges: Vec::new(),
        }
    }

    /// Adds an include edge.
    pub fn push_edge(&mut self, edge: IncludeGraphEdge) {
        self.edges.push(edge);
    }

    /// Returns whether all production include-discovery edges are GPU-resident.
    ///
    /// Host filesystem metadata is allowed only as metadata input. CPU oracle
    /// edges are not production proof.
    #[must_use]
    pub fn production_edges_are_gpu_resident(&self) -> bool {
        self.edges
            .iter()
            .filter(|edge| {
                edge.residency != IncludeGraphResidency::HostFilesystemMetadata
                    && edge.residency != IncludeGraphResidency::HostMemoryCache
            })
            .all(|edge| edge.residency == IncludeGraphResidency::GpuResident)
    }

    /// Returns unresolved include edges.
    #[must_use]
    pub fn unresolved_edges(&self) -> Vec<&IncludeGraphEdge> {
        self.edges.iter().filter(|edge| !edge.resolved).collect()
    }

    /// Builds frontend include-graph proof from the GPU preprocessor output.
    #[must_use]
    pub fn from_gpu_preprocessed_source(
        root: impl Into<String>,
        source: &vyre_libs::parsing::c::preprocess::gpu_pipeline::PreprocessedSource,
    ) -> Self {
        use vyre_libs::parsing::c::preprocess::gpu_pipeline::IncludeEventResidency;

        let mut proof = Self::new(root);
        for event in &source.include_events {
            let request_residency = match event.request_residency {
                IncludeEventResidency::GpuResidentRequest => IncludeGraphResidency::GpuResident,
                IncludeEventResidency::HostFilesystemMetadata => {
                    IncludeGraphResidency::HostFilesystemMetadata
                }
                IncludeEventResidency::HostMemoryCache => IncludeGraphResidency::CpuOracle,
            };
            let resolution_residency = match event.resolution_residency {
                IncludeEventResidency::GpuResidentRequest => IncludeGraphResidency::GpuResident,
                IncludeEventResidency::HostFilesystemMetadata => {
                    IncludeGraphResidency::HostFilesystemMetadata
                }
                IncludeEventResidency::HostMemoryCache => IncludeGraphResidency::HostMemoryCache,
            };
            proof.push_edge(IncludeGraphEdge::new(
                event.includer.to_string_lossy(),
                event.resolved_path.to_string_lossy(),
                u64::from(event.directive_byte_offset),
                true,
                request_residency,
            ));
            proof.push_edge(IncludeGraphEdge::new(
                event.resolved_path.to_string_lossy(),
                event.resolved_path.to_string_lossy(),
                0,
                true,
                resolution_residency,
            ));
        }
        proof
    }
}

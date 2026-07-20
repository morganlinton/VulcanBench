//! Production CPU-fallback reachability validation.

/// One observed module/import/call edge from a reachability scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReachabilityEdge<'a> {
    /// Source file or module path.
    pub source: &'a str,
    /// Referenced module, symbol, or call target.
    pub target: &'a str,
    /// Whether this edge was observed in production code.
    pub production: bool,
}

/// Approved non-production CPU helper boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApprovedParityBoundary<'a> {
    /// Source-path prefix allowed to use CPU/reference/oracle code.
    pub source_prefix: &'a str,
    /// Human-readable reason.
    pub reason: &'a str,
}

/// CPU-fallback reachability proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuFallbackReachabilityProof {
    /// Number of scanned edges.
    pub scanned_edges: usize,
    /// Number of CPU-like edges accepted because they are parity-only.
    pub approved_parity_edges: usize,
}

/// CPU-fallback reachability validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuFallbackReachabilityError {
    /// No scan edges were provided.
    EmptyScan,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Field name.
        field: &'static str,
    },
    /// Production code can reach a CPU/reference/oracle/fallback target.
    ProductionCpuFallbackReachable {
        /// Source path.
        source: String,
        /// Target path or symbol.
        target: String,
    },
    /// A CPU-like non-production edge lacks an approved parity boundary.
    UnapprovedParityBoundary {
        /// Source path.
        source: String,
        /// Target path or symbol.
        target: String,
    },
}

impl std::fmt::Display for CpuFallbackReachabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyScan => write!(
                f,
                "CPU-fallback reachability scan is empty. Fix: scan production imports/calls before release."
            ),
            Self::EmptyMetadata { field } => write!(
                f,
                "CPU-fallback reachability evidence has empty {field}. Fix: record source, target, and approved parity-boundary metadata."
            ),
            Self::ProductionCpuFallbackReachable { source, target } => write!(
                f,
                "production source `{source}` reaches CPU/reference/oracle target `{target}`. Fix: remove the production edge or move it behind an explicit parity-test boundary."
            ),
            Self::UnapprovedParityBoundary { source, target } => write!(
                f,
                "non-production source `{source}` reaches CPU/reference/oracle target `{target}` without an approved parity boundary. Fix: register the test-only boundary explicitly."
            ),
        }
    }
}

impl std::error::Error for CpuFallbackReachabilityError {}

/// Validate that fallback/reference/oracle helpers are not production-reachable.
pub fn validate_fallback_reachability(
    edges: &[ReachabilityEdge<'_>],
    approved_boundaries: &[ApprovedParityBoundary<'_>],
) -> Result<CpuFallbackReachabilityProof, CpuFallbackReachabilityError> {
    if edges.is_empty() {
        return Err(CpuFallbackReachabilityError::EmptyScan);
    }
    let mut approved_parity_edges = 0_usize;
    for boundary in approved_boundaries {
        if boundary.source_prefix.trim().is_empty() {
            return Err(CpuFallbackReachabilityError::EmptyMetadata {
                field: "source_prefix",
            });
        }
        if boundary.reason.trim().is_empty() {
            return Err(CpuFallbackReachabilityError::EmptyMetadata { field: "reason" });
        }
    }

    for edge in edges {
        if edge.source.trim().is_empty() {
            return Err(CpuFallbackReachabilityError::EmptyMetadata { field: "source" });
        }
        if edge.target.trim().is_empty() {
            return Err(CpuFallbackReachabilityError::EmptyMetadata { field: "target" });
        }
        if !is_cpu_like_target(edge.target) {
            continue;
        }
        if edge.production {
            return Err(
                CpuFallbackReachabilityError::ProductionCpuFallbackReachable {
                    source: edge.source.to_owned(),
                    target: edge.target.to_owned(),
                },
            );
        }
        if approved_boundaries
            .iter()
            .any(|boundary| edge.source.starts_with(boundary.source_prefix))
        {
            approved_parity_edges += 1;
        } else {
            return Err(CpuFallbackReachabilityError::UnapprovedParityBoundary {
                source: edge.source.to_owned(),
                target: edge.target.to_owned(),
            });
        }
    }

    Ok(CpuFallbackReachabilityProof {
        scanned_edges: edges.len(),
        approved_parity_edges,
    })
}

fn is_cpu_like_target(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    lower.contains("cpu")
        || lower.contains("fallback")
        || lower.contains("oracle")
        || lower.contains("reference")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reachability_accepts_cpu_helpers_only_in_approved_parity_tests() {
        let proof = validate_fallback_reachability(
            &[
                edge("vyre-driver-cuda/src/backend.rs", "cuda::dispatch", true),
                edge(
                    "vyre-driver-cuda/tests/csr_frontier_queue_gpu_parity.rs",
                    "reference_csr_queue",
                    false,
                ),
            ],
            &[ApprovedParityBoundary {
                source_prefix: "vyre-driver-cuda/tests/",
                reason: "GPU parity tests compare CUDA against explicit reference oracles",
            }],
        )
        .expect("Fix: approved parity CPU helper should pass");

        assert_eq!(proof.scanned_edges, 2);
        assert_eq!(proof.approved_parity_edges, 1);
    }

    #[test]
    fn reachability_rejects_production_cpu_fallback_edges() {
        assert_eq!(
            validate_fallback_reachability(
                &[edge(
                    "vyre-driver-cuda/src/backend.rs",
                    "cpu_fallback_dispatch",
                    true
                )],
                &[],
            )
            .expect_err("production CPU fallback must fail"),
            CpuFallbackReachabilityError::ProductionCpuFallbackReachable {
                source: "vyre-driver-cuda/src/backend.rs".to_owned(),
                target: "cpu_fallback_dispatch".to_owned(),
            }
        );
    }

    #[test]
    fn reachability_rejects_unapproved_reference_oracles() {
        assert_eq!(
            validate_fallback_reachability(
                &[edge(
                    "vyre-driver-cuda/examples/demo.rs",
                    "reference_oracle",
                    false
                )],
                &[ApprovedParityBoundary {
                    source_prefix: "vyre-driver-cuda/tests/",
                    reason: "test oracle only",
                }],
            )
            .expect_err("unapproved oracle boundary must fail"),
            CpuFallbackReachabilityError::UnapprovedParityBoundary {
                source: "vyre-driver-cuda/examples/demo.rs".to_owned(),
                target: "reference_oracle".to_owned(),
            }
        );
    }

    fn edge<'a>(source: &'a str, target: &'a str, production: bool) -> ReachabilityEdge<'a> {
        ReachabilityEdge {
            source,
            target,
            production,
        }
    }
}

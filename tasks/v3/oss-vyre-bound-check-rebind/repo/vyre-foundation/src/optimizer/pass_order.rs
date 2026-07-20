//! Machine-checkable optimizer pass ordering certificates.
//!
//! The scheduler computes a topological order. This module exposes the same
//! contract as a small validator release tooling can call directly: every pass
//! ID is unique, every declared requirement exists, and every requirement
//! appears before the pass that consumes it.

use super::{derive_registered_pass_order, OptimizerError, PassMetadata, PassSchedulingError};
use rustc_hash::FxHashMap;

/// Summary returned after validating an optimizer pass order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassOrderValidation {
    /// Number of passes validated.
    pub pass_count: usize,
    /// Number of declared dependency edges validated.
    pub dependency_edges: usize,
    /// First pass in the validated order.
    pub first_pass: Option<&'static str>,
    /// Last pass in the validated order.
    pub last_pass: Option<&'static str>,
}

/// Validate the live registered optimizer pass order.
///
/// # Errors
/// Returns [`OptimizerError`] when the registered scheduler inventory is
/// invalid or when the scheduled order violates a declared dependency.
pub fn validate_registered_pass_order() -> Result<PassOrderValidation, OptimizerError> {
    let derived = derive_registered_pass_order()?;
    validate_scheduled_pass_order(derived.metadata()).map_err(OptimizerError::from)
}

/// Validate an already-ordered pass metadata slice.
///
/// # Errors
/// Returns [`PassSchedulingError`] when pass IDs are duplicated, requirements
/// are unknown, or a requirement appears after its dependent pass.
pub fn validate_scheduled_pass_order(
    metadata: &[PassMetadata],
) -> Result<PassOrderValidation, PassSchedulingError> {
    let mut position_by_name =
        FxHashMap::with_capacity_and_hasher(metadata.len(), Default::default());
    for (index, pass) in metadata.iter().enumerate() {
        if position_by_name.insert(pass.name, index).is_some() {
            return Err(PassSchedulingError::DuplicateId { id: pass.name });
        }
    }

    let mut dependency_edges = 0usize;
    for (pass_index, pass) in metadata.iter().enumerate() {
        for &requirement in pass.requires {
            dependency_edges = dependency_edges.saturating_add(1);
            let Some(&requirement_index) = position_by_name.get(requirement) else {
                return Err(PassSchedulingError::UnknownRequire {
                    pass: pass.name,
                    missing: requirement,
                });
            };
            if requirement_index >= pass_index {
                return Err(PassSchedulingError::OrderViolation {
                    pass: pass.name,
                    requirement,
                });
            }
        }
    }

    Ok(PassOrderValidation {
        pass_count: metadata.len(),
        dependency_edges,
        first_pass: metadata.first().map(|pass| pass.name),
        last_pass: metadata.last().map(|pass| pass.name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(name: &'static str, requires: &'static [&'static str]) -> PassMetadata {
        PassMetadata::new(name, requires, &[])
    }

    #[test]
    fn validates_dependency_before_user() {
        let report = validate_scheduled_pass_order(&[
            meta("shape_facts", &[]),
            meta("decode_scan_fuse", &["shape_facts"]),
        ])
        .expect("Fix: dependency appears before consumer");
        assert_eq!(report.pass_count, 2);
        assert_eq!(report.dependency_edges, 1);
        assert_eq!(report.first_pass, Some("shape_facts"));
        assert_eq!(report.last_pass, Some("decode_scan_fuse"));
    }

    #[test]
    fn rejects_dependency_after_user() {
        let error = validate_scheduled_pass_order(&[
            meta("decode_scan_fuse", &["shape_facts"]),
            meta("shape_facts", &[]),
        ])
        .expect_err("consumer must not run before dependency");
        assert_eq!(
            error,
            PassSchedulingError::OrderViolation {
                pass: "decode_scan_fuse",
                requirement: "shape_facts",
            }
        );
    }

    #[test]
    fn rejects_unknown_requirement() {
        let error = validate_scheduled_pass_order(&[meta("decode_scan_fuse", &["shape_facts"])])
            .expect_err("missing dependency must fail release validation");
        assert_eq!(
            error,
            PassSchedulingError::UnknownRequire {
                pass: "decode_scan_fuse",
                missing: "shape_facts",
            }
        );
    }

    #[test]
    fn rejects_duplicate_pass_id() {
        let error =
            validate_scheduled_pass_order(&[meta("shape_facts", &[]), meta("shape_facts", &[])])
                .expect_err("duplicate pass IDs make diagnostics ambiguous");
        assert_eq!(
            error,
            PassSchedulingError::DuplicateId { id: "shape_facts" }
        );
    }

    #[test]
    fn live_registered_order_validates() {
        let report = validate_registered_pass_order()
            .expect("Fix: live optimizer registry must have a valid dependency order");
        assert!(
            report.pass_count > 0,
            "live optimizer registry must not be empty"
        );
    }
}

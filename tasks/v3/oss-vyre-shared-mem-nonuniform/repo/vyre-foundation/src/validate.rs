//! Structural and semantic IR validation.
//!
//! Catches errors at construction time rather than at GPU compile time.
//! A program that passes validation is guaranteed to be well-formed: every
//! buffer reference is declared, every type is consistent, and every
//! recursive depth is bounded. Validation is the mandatory gate between
//! frontend emission and backend lowering.

/// Buffer name and binding validation.
///
/// Ensures that buffer names are unique, binding slots do not collide,
/// and workgroup buffers have positive element counts.
pub mod binding;

/// Default depth limits and limit tracking for recursive validation.
///
/// Defines the maximum call depth, nesting depth, and node count that
/// the validator will accept. These limits prevent pathological programs
/// from causing stack overflow or adapter limit violations.
pub mod depth;

/// Validation error construction helpers.
///
/// Internal utilities for building consistent `ValidationError` values
/// with actionable `Fix:` messages.
pub mod err;
/// Validation options + backend capability hooks.
pub mod options;

/// Adapter limit validation.
///
/// Checks that buffer sizes, workgroup dimensions, and program scale
/// stay within the limits of the target GPU adapter.
pub mod limits;

/// The main validate entry point.
///
/// `validate` is the top-level function that runs every validation pass
/// and returns a vector of errors (empty on success).
pub mod validate;

/// Validation report containing hard failures and warnings.
pub mod report;
/// Detailed validation failure report.
///
/// `ValidationError` describes exactly which invariant was violated,
/// where in the program it occurred, and how to fix it.
pub mod validation_error;

mod atomic_rules;
mod barrier;
mod bytes_rejection;
mod cast;
mod expr_rules;
mod fusion_safety;
/// Linear-type discipline checker (P-1.0-V2.2). Verifies each
/// `BufferDecl::linear_type()` against the actual usage count in the
/// IR. Violators are reported as `ValidationError`s.
pub mod linear_type;
mod nodes;
mod self_composition;
mod shadowing;
/// Shape-refinement predicate checker (P-1.0-V3.2). Evaluates each
/// `BufferDecl::shape_predicate()` against the static `count` and
/// reports a `ValidationError` for every contradiction.
pub mod shape_predicate;
mod typecheck;
mod uniformity;

pub(crate) use binding::Binding;
/// Re-export of default depth limits and limit tracking.
///
/// These constants and types are used by the validator and by tests
/// that need to construct limit configurations.
pub use depth::{
    LimitState, DEFAULT_MAX_CALL_DEPTH, DEFAULT_MAX_EXPR_DEPTH, DEFAULT_MAX_NESTING_DEPTH,
    DEFAULT_MAX_NODE_COUNT,
};
pub(crate) use err::err;
pub use options::{BackendCapabilities, BackendValidationCapabilities, ValidationOptions};
pub use report::{ValidationReport, ValidationWarning};
/// Re-export of the top-level validation function.
///
/// This is the stable entry point called by frontends before handing a
/// `Program` to a backend.
pub use validate::validate;
/// Re-export of the focused `Fma` f32-operand check emit backends run
/// before lowering (see [`validate::fma_f32_violations`]).
pub use validate::fma_f32_violations;
pub use validate::validate_with_options;
/// Re-export of the detailed validation error type.
///
/// Consumers inspect `ValidationError` to produce human-readable
/// diagnostics or to decide whether a program can be retried.
pub use validation_error::ValidationError;

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::ir::{BufferDecl, DataType, Expr, Node, Program};

    #[test]
    fn array_output_buffer_rejected() {
        let program = Program::wrapped(
            vec![BufferDecl::output(
                "out",
                0,
                DataType::Array { element_size: 4 },
            )],
            [1, 1, 1],
            Vec::new(),
        );
        let errors = validate(&program);
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("output buffer `out` uses unsupported element type `array<4B>`")
        }));
    }

    #[test]
    fn tensor_output_buffer_rejected() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::Tensor)],
            [1, 1, 1],
            Vec::new(),
        );
        let errors = validate(&program);
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("output buffer `out` uses unsupported element type `tensor`")
        }));
    }

    #[test]
    fn wrapped_constructor_inserts_root_region_for_raw_entry() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(9)), Node::Return],
        );
        assert!(
            matches!(program.entry(), [Node::Region { generator, .. }] if generator.as_ref() == Program::ROOT_REGION_GENERATOR)
        );
    }

    #[test]
    fn wrapped_constructor_preserves_existing_top_level_regions() {
        let body = vec![Node::Return];
        let region = Node::Region {
            generator: "already.region".into(),
            source_region: None,
            body: std::sync::Arc::new(body),
        };
        let program = Program::wrapped(Vec::new(), [1, 1, 1], vec![region.clone()]);
        assert_eq!(program.entry(), &[region]);
    }

    #[test]
    #[allow(deprecated)]
    fn raw_top_level_statement_is_rejected() {
        let program = Program::new(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(7)), Node::Return],
        );
        let errors = validate(&program);
        assert!(errors.iter().any(|error| {
            error.message.contains("top-level Region") || error.message.contains("Node::Region")
        }));
    }

    /// P-1.0-V2.2 integration: linear_type::check_linear_types is
    /// invoked through the public validate() entry. Buffers declared
    /// `Linear` but unused must surface as a top-level validation
    /// error so backends never see the offending program.
    #[test]
    fn linear_type_violation_surfaces_through_validate() {
        use crate::ir::LinearType;
        let program = Program::wrapped(
            vec![BufferDecl::output("ghost", 0, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear)],
            [1, 1, 1],
            vec![Node::Return],
        );
        let errors = validate(&program);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("`ghost` declared `LinearType::Linear`")),
            "linear-type checker not wired into validate(): got {errors:?}"
        );
    }

    /// Negative twin: an Unrestricted (default) buffer used zero
    /// times must not surface as a linear-type error.
    #[test]
    fn unrestricted_buffer_is_not_flagged_by_linear_type_checker() {
        let program = Program::wrapped(
            vec![BufferDecl::output("ok", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("ok", Expr::u32(0), Expr::u32(0))],
        );
        let errors = validate(&program);
        assert!(
            !errors.iter().any(|e| e.message.contains("LinearType::")),
            "unrestricted buffer flagged: {errors:?}"
        );
    }

    /// P-1.0-V3.2 integration: shape_predicate::check_shape_predicates
    /// is invoked through the public validate() entry. Buffers whose
    /// static count contradicts their declared predicate must surface
    /// as a top-level validation error.
    #[test]
    fn shape_predicate_violation_surfaces_through_validate() {
        use crate::ir::ShapePredicate;
        let program = Program::wrapped(
            vec![BufferDecl::output("misaligned", 0, DataType::U32)
                .with_count(3)
                .with_shape_predicate(ShapePredicate::MultipleOf(64))],
            [1, 1, 1],
            vec![Node::store("misaligned", Expr::u32(0), Expr::u32(0))],
        );
        let errors = validate(&program);
        assert!(
            errors.iter().any(
                |e| e.message.contains("`misaligned`") && e.message.contains("count % 64 == 0")
            ),
            "shape-predicate checker not wired into validate(): got {errors:?}"
        );
    }

    /// Negative twin: a buffer satisfying its predicate must not
    /// surface as a shape-predicate error.
    #[test]
    fn satisfied_shape_predicate_is_not_flagged() {
        use crate::ir::ShapePredicate;
        let program = Program::wrapped(
            vec![BufferDecl::output("aligned", 0, DataType::U32)
                .with_count(128)
                .with_shape_predicate(ShapePredicate::MultipleOf(64))],
            [1, 1, 1],
            vec![Node::store("aligned", Expr::u32(0), Expr::u32(0))],
        );
        let errors = validate(&program);
        assert!(
            !errors.iter().any(|e| e.message.contains("count % ")),
            "satisfied predicate flagged: {errors:?}"
        );
    }
}

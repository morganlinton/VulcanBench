//! Shape-predicate refinement checker (P-1.0-V3.2).
//!
//! Walks each `BufferDecl` and evaluates its
//! [`ShapePredicate`](crate::ir_inner::model::program::ShapePredicate)
//! against the static `count` declared on that buffer. Buffers
//! without a predicate (the default) are always accepted.
//!
//! Wired into `validate::validate` so backends never see a program
//! whose declared shape contradicts the static count.

use crate::ir_inner::model::program::Program;
use crate::validate::{err, ValidationError};

/// Evaluate every buffer's `shape_predicate` against its static `count`.
/// Returns one validation error per violation.
#[must_use]
pub fn check_shape_predicates(program: &Program) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for buffer in program.buffers() {
        let Some(predicate) = buffer.shape_predicate() else {
            continue;
        };
        if !predicate.holds(buffer.count()) {
            errors.push(err(format!(
                "buffer `{}` declared shape predicate `{}` but has count={}. Fix: change the count to satisfy the predicate, or relax the predicate.",
                buffer.name(),
                predicate.describe(),
                buffer.count()
            )));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Node, ShapePredicate};

    fn program_with(buffers: Vec<BufferDecl>) -> Program {
        Program::wrapped(buffers, [1, 1, 1], vec![Node::Return])
    }

    #[test]
    fn no_predicate_never_errors() {
        let prog = program_with(vec![BufferDecl::storage(
            "a",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(7)]);
        assert!(check_shape_predicates(&prog).is_empty());
    }

    #[test]
    fn at_least_violation_errors_with_describe_message() {
        let prog = program_with(vec![BufferDecl::storage(
            "small",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(7)
        .with_shape_predicate(ShapePredicate::AtLeast(64))]);
        let errors = check_shape_predicates(&prog);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("`small`"));
        assert!(errors[0].message.contains("count >= 64"));
        assert!(errors[0].message.contains("count=7"));
    }

    #[test]
    fn multiple_of_satisfied_passes() {
        let prog = program_with(vec![BufferDecl::storage(
            "aligned",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(128)
        .with_shape_predicate(ShapePredicate::MultipleOf(64))]);
        assert!(check_shape_predicates(&prog).is_empty());
    }

    #[test]
    fn exactly_violation_errors() {
        let prog = program_with(vec![BufferDecl::storage(
            "fixed",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(8)
        .with_shape_predicate(ShapePredicate::Exactly(7))]);
        let errors = check_shape_predicates(&prog);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("count == 7"));
    }

    #[test]
    fn and_predicate_evaluates_both_branches() {
        let prog = program_with(vec![BufferDecl::storage(
            "tile",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(96)
        .with_shape_predicate(ShapePredicate::And(
            Box::new(ShapePredicate::AtLeast(64)),
            Box::new(ShapePredicate::MultipleOf(32)),
        ))]);
        assert!(check_shape_predicates(&prog).is_empty());

        let prog2 = program_with(vec![BufferDecl::storage(
            "tile",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(80)
        .with_shape_predicate(ShapePredicate::And(
            Box::new(ShapePredicate::AtLeast(64)),
            Box::new(ShapePredicate::MultipleOf(32)),
        ))]);
        let errors = check_shape_predicates(&prog2);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn multiple_violators_yield_multiple_errors() {
        let prog = program_with(vec![
            BufferDecl::storage("a", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(7)
                .with_shape_predicate(ShapePredicate::Exactly(8)),
            BufferDecl::storage("b", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(3)
                .with_shape_predicate(ShapePredicate::AtLeast(10)),
        ]);
        let errors = check_shape_predicates(&prog);
        assert_eq!(errors.len(), 2);
    }
}

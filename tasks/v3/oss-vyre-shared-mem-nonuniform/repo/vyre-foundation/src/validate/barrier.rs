//! Barrier placement validation.
//!
//! Workgroup barriers in GPU shaders must only appear in uniform control
//! flow: every thread in the workgroup must reach the barrier or none
//! must reach it. This module checks that barrier nodes are not placed
//! inside divergent branches, catching a class of bugs that would
//! otherwise deadlock or produce undefined behavior on the GPU.

use crate::memory_model::MemoryOrdering;
use crate::validate::{err, ValidationError};

/// Ensure a barrier is not placed inside divergent control flow.
///
/// A barrier inside an `If` or `Loop` whose condition is not uniform
/// across the workgroup is illegal in vyre. This function appends a
/// validation error when `divergent` is `true`.
///
/// # Examples
///
/// `check_barrier` is `pub(crate)`; it's exercised indirectly through
/// [`crate::validate::validate::validate`] when a program contains a
/// `Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }` inside a divergent `Node::If`. See the unit tests on
/// [`crate::validate::validate::validate`] for a runnable example.
///
/// # Errors
///
/// Appends a `ValidationError` with code `V010` when `divergent` is
/// `true`.
#[inline]
pub(crate) fn check_barrier(
    divergent: bool,
    ordering: MemoryOrdering,
    errors: &mut Vec<ValidationError>,
) {
    if divergent {
        errors.push(err(
            "V010: barrier may be reached by only part of a workgroup. Fix: move the barrier to uniform control flow."
                .to_string(),
        ));
    }
    if !ordering.is_valid_for_barrier() {
        errors.push(err(format!(
            "V043: barrier uses memory ordering `{ordering:?}`, but barriers must synchronize memory. Fix: use Acquire, Release, AcqRel, or SeqCst; use no barrier at all for Relaxed."
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divergent_barrier_emits_v010() {
        let mut errors = Vec::new();
        check_barrier(true, MemoryOrdering::SeqCst, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("V010"));
    }

    #[test]
    fn uniform_barrier_is_valid() {
        let mut errors = Vec::new();
        check_barrier(false, MemoryOrdering::SeqCst, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn relaxed_barrier_is_rejected() {
        let mut errors = Vec::new();
        check_barrier(false, MemoryOrdering::Relaxed, &mut errors);
        assert!(errors.iter().any(|error| error.message().contains("V043")));
    }
}

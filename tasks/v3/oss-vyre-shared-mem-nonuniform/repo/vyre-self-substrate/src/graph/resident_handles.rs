//! Shared resident-handle utilities for graph dispatch wrappers.

use std::collections::HashSet;

use crate::hardware::scratch::reserve_hash_set as reserve_graph_hash_set;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Free each resident handle at most once while still attempting every unique
/// handle after the first backend failure.
pub(crate) fn free_unique_resident_handles(
    dispatcher: &dyn OptimizerDispatcher,
    handles: &[u64],
    context: &'static str,
) -> Result<(), DispatchError> {
    let mut seen = HashSet::new();
    reserve_graph_hash_set(&mut seen, handles.len(), context)?;
    let mut first_err = None;
    for &handle in handles {
        if !seen.insert(handle) {
            continue;
        }
        if let Err(err) = dispatcher.free_resident(handle) {
            if first_err.is_none() {
                first_err = Some(err);
            }
        }
    }
    match first_err {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use vyre_foundation::ir::Program;

    #[derive(Default)]
    struct RecordingFreeDispatcher {
        freed: RefCell<Vec<u64>>,
        fail_on: Option<u64>,
    }

    impl OptimizerDispatcher for RecordingFreeDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Err(DispatchError::Rejected(
                "Fix: resident handle tests should not dispatch programs.".to_string(),
            ))
        }

        fn free_resident(&self, handle: u64) -> Result<(), DispatchError> {
            self.freed.borrow_mut().push(handle);
            if self.fail_on == Some(handle) {
                return Err(DispatchError::BackendError(format!(
                    "Fix: injected resident free failure for handle {handle}."
                )));
            }
            Ok(())
        }
    }

    #[test]
    fn generated_free_unique_resident_handles_dedupes_and_preserves_order() {
        let dispatcher = RecordingFreeDispatcher::default();

        free_unique_resident_handles(&dispatcher, &[7, 9, 7, 11, 9], "test graph")
            .expect("Fix: deduped resident handle free should succeed");

        assert_eq!(dispatcher.freed.borrow().as_slice(), &[7, 9, 11]);
    }

    #[test]
    fn generated_free_unique_resident_handles_attempts_after_first_failure() {
        let dispatcher = RecordingFreeDispatcher {
            freed: RefCell::new(Vec::new()),
            fail_on: Some(9),
        };

        let error = free_unique_resident_handles(&dispatcher, &[7, 9, 11], "test graph")
            .expect_err("Fix: injected resident handle free failure must surface");

        assert!(
            error.to_string().contains("handle 9"),
            "first resident free error must be returned"
        );
        assert_eq!(dispatcher.freed.borrow().as_slice(), &[7, 9, 11]);
    }
}

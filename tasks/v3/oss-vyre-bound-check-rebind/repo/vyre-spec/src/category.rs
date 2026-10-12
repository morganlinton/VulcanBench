//! Frozen operation-category records used to separate composition from intrinsics.

/// Backend support predicate for a Category C operation.
pub trait BackendAvailability: Send + Sync {
    /// Return true when the backend identified by `op` supports the operation.
    fn available(&self, op: &str) -> bool;
}

impl<F> BackendAvailability for F
where
    F: Fn(&str) -> bool + Send + Sync,
{
    fn available(&self, op: &str) -> bool {
        self(op)
    }
}

/// Function-pointer wrapper for static backend-availability predicates.
#[derive(Clone, Copy)]
pub struct BackendAvailabilityPredicate {
    predicate: fn(&str) -> bool,
}

impl BackendAvailabilityPredicate {
    /// Create a backend-availability predicate from a total function.
    #[must_use]
    pub const fn new(predicate: fn(&str) -> bool) -> Self {
        Self { predicate }
    }

    /// Return true when the named backend supports the operation.
    #[must_use]
    pub fn available(&self, op: &str) -> bool {
        <Self as BackendAvailability>::available(self, op)
    }
}

impl core::fmt::Debug for BackendAvailabilityPredicate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("BackendAvailabilityPredicate(..)")
    }
}

impl BackendAvailability for BackendAvailabilityPredicate {
    fn available(&self, op: &str) -> bool {
        (self.predicate)(op)
    }
}

/// vyre operation category in the frozen data contract.
///
/// Category B  -  runtime opcode dispatch, stack-machine VMs, and eval engines
///  -  is intentionally absent from this enum. vyre has no opcode interpreter
/// and no execution path that is not a lowered IR program on a backend, so
/// Category B is forbidden by the conformance model rather than specified
/// as a valid operation category. Example: `Category::Intrinsic` records that an op is
/// backed by a named hardware capability such as a subgroup intrinsic.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Category {
    /// Compositional operation that must disappear after lowering.
    A {
        /// Operation IDs that define the zero-overhead composition.
        composition_of: Vec<&'static str>,
    },
    /// Hardware intrinsic with declared per-backend availability.
    C {
        /// Hardware unit or backend feature required by the intrinsic.
        hardware: &'static str,
        /// Predicate that returns true when the backend supports this op.
        backend_availability: BackendAvailabilityPredicate,
    },
}

impl Category {
    /// Temporary marker used until every operation receives a real category.
    #[must_use]
    pub fn unclassified() -> Self {
        Self::A {
            composition_of: Vec::new(),
        }
    }

    /// True when the category is the compile-only empty Category A marker.
    #[must_use]
    pub fn is_unclassified(&self) -> bool {
        matches!(self, Self::A { composition_of } if composition_of.is_empty())
    }
}

impl PartialEq for Category {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::A {
                    composition_of: left,
                },
                Self::A {
                    composition_of: right,
                },
            ) => left == right,
            (
                Self::C { hardware: left, .. },
                Self::C {
                    hardware: right, ..
                },
            ) => left == right,
            _ => false,
        }
    }
}

impl Eq for Category {}

//! Descriptor for a Category C hardware intrinsic.
//!
//! `IntrinsicDescriptor` binds a stable name, a required hardware unit, and a
//! CPU reference function. Backends that claim support for the intrinsic must
//! produce output that matches the CPU reference exactly on every witnessed
//! input. Conform proofs carry this descriptor so that any reader can audit
//! the hardware contract that the backend claims to satisfy.

use std::sync::Arc;

/// Flat byte-ABI CPU reference function used by Category C descriptors.
///
/// The function reads raw bytes from `input`, computes the operation's
/// semantics, and appends the result bytes to `output`. This type lives in
/// `vyre-spec` so that conform certificates can embed the function pointer
/// without dragging the rest of the compiler into the data contract.
pub type CpuFn = fn(input: &[u8], output: &mut Vec<u8>);

/// Stable string identity for a backend.
///
/// Concrete driver crates own the spelling of their ids. The spec layer stores
/// ids as opaque strings so adding a backend never requires editing this crate.
#[derive(Debug, Clone)]
pub struct BackendId(Arc<str>);

impl BackendId {
    /// Construct a backend id from any string-like value.
    #[must_use]
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self(name.into())
    }

    /// Return the backend id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BackendId {
    fn from(name: &str) -> Self {
        Self(Arc::from(name))
    }
}

impl From<String> for BackendId {
    fn from(name: String) -> Self {
        Self(Arc::from(name))
    }
}

impl core::fmt::Display for BackendId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq for BackendId {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ref() == other.0.as_ref()
    }
}

impl Eq for BackendId {}

impl core::hash::Hash for BackendId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ref().hash(state);
    }
}

/// Backend identity used when checking Category C intrinsic availability.
///
/// This is intentionally a data wrapper, not an enum. Concrete backend names
/// belong in the driver crates that implement them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Backend {
    id: BackendId,
    name: Option<Arc<str>>,
}

impl Backend {
    /// Construct a backend identity from an opaque backend id.
    #[must_use]
    pub fn new(id: impl Into<BackendId>) -> Self {
        Self {
            id: id.into(),
            name: None,
        }
    }

    /// Construct a backend identity with a friendly display name.
    #[must_use]
    pub fn named(id: impl Into<BackendId>, name: impl Into<Arc<str>>) -> Self {
        Self {
            id: id.into(),
            name: Some(name.into()),
        }
    }

    /// Stable string identifier for this backend.
    #[must_use]
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    /// Friendly backend name, falling back to [`Backend::id`].
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or_else(|| self.id())
    }
}

impl From<BackendId> for Backend {
    fn from(id: BackendId) -> Self {
        Self::new(id)
    }
}

impl From<&str> for Backend {
    fn from(id: &str) -> Self {
        Self::new(BackendId::from(id))
    }
}

impl From<&Backend> for BackendId {
    fn from(backend: &Backend) -> Self {
        backend.id.clone()
    }
}

use crate::op_contract::OperationContract;

/// Descriptor for a Category C hardware intrinsic.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct IntrinsicDescriptor {
    name: &'static str,
    hardware: &'static str,
    cpu_fn: CpuFn,
    contract: Option<OperationContract>,
}

impl IntrinsicDescriptor {
    /// Create an intrinsic descriptor with an explicit CPU reference function.
    #[must_use]
    pub const fn new(name: &'static str, hardware: &'static str, cpu_fn: CpuFn) -> Self {
        Self {
            name,
            hardware,
            cpu_fn,
            contract: None,
        }
    }

    /// Create an intrinsic descriptor with optional execution contract metadata.
    #[must_use]
    pub const fn with_contract(
        name: &'static str,
        hardware: &'static str,
        cpu_fn: CpuFn,
        contract: OperationContract,
    ) -> Self {
        Self {
            name,
            hardware,
            cpu_fn,
            contract: Some(contract),
        }
    }

    /// Stable intrinsic name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Required hardware unit or backend feature.
    #[must_use]
    pub const fn hardware(&self) -> &'static str {
        self.hardware
    }

    /// CPU reference implementation for this intrinsic.
    #[must_use]
    pub const fn cpu_fn(&self) -> CpuFn {
        self.cpu_fn
    }

    /// Optional capability and execution contract annotations.
    #[must_use]
    pub const fn contract(&self) -> Option<&OperationContract> {
        self.contract.as_ref()
    }
}

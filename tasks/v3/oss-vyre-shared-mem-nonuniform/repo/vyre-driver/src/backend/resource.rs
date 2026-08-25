//! Backend-neutral resource handles.

/// A GPU-resident or host-side resource used as an input to a Program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resource {
    /// Host-side byte slice. Replicated to the GPU on each dispatch.
    Borrowed(Vec<u8>),
    /// GPU-resident buffer handle. Zero-copy; no host transfer occurs.
    Resident(u64),
}

impl Default for Resource {
    fn default() -> Self {
        Resource::Borrowed(Vec::new())
    }
}

impl From<Vec<u8>> for Resource {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Borrowed(bytes)
    }
}

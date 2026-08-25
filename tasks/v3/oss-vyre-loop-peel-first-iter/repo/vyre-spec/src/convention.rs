//! Frozen calling-convention versions for generated operation entrypoints.

/// Calling convention version in the frozen data contract.
///
/// Example: `Convention::V2 { lookup_binding: 3 }` records that an operation
/// receives an additional lookup-table buffer at binding 3.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum Convention {
    /// Standard convention: input, output, and params buffers.
    #[default]
    V1,
    /// V1 plus a lookup table buffer.
    V2 {
        /// Binding index for the lookup buffer.
        lookup_binding: u32,
    },
}

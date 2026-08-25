//! Program model  -  a complete, self-contained GPU compute dispatch.
//!
//! A `Program` can be constructed without a GPU, serialized to disk,
//! transmitted over a network, optimized by transformation passes, and lowered
//! to any target backend. It is the unit of work in vyre.
//!
//! Equality is intentionally **structural**, not allocation-based:
//! - `Program::structural_eq` performs an O(N) walk of the visible IR.
//! - [`PartialEq`] delegates to that same structural walk.
//! - Buffer declaration order is treated as a set, because reordering
//!   declarations without changing names/bindings/types does not change
//!   dispatch semantics.
//!
//! This keeps arena-local identities and pointer layouts out of the public API.

mod buffer_decl;
mod builder;
mod canonical;
mod core;
#[allow(clippy::expect_used)]
mod meta;
mod scope;
/// Per-node-kind bitset constants for `ProgramStats`.
pub mod stats;

#[cfg(test)]
mod stats_test {
    include!("stats_test.rs");
}
#[cfg(test)]
mod tests;

pub use self::buffer_decl::{BufferDecl, LinearType, ShapePredicate};
pub use self::core::Program;
pub use self::scope::Scope;
pub use self::stats::ProgramStats;
pub use self::stats::{
    NODE_KIND_ALL_GATHER, NODE_KIND_ALL_REDUCE, NODE_KIND_ASSIGN, NODE_KIND_ASYNC_LOAD,
    NODE_KIND_ASYNC_STORE, NODE_KIND_ASYNC_WAIT, NODE_KIND_BARRIER, NODE_KIND_BLOCK,
    NODE_KIND_BROADCAST, NODE_KIND_EXPRESSION_BEARING_MASK, NODE_KIND_IF,
    NODE_KIND_INDIRECT_DISPATCH, NODE_KIND_LET, NODE_KIND_LOOP, NODE_KIND_OPAQUE,
    NODE_KIND_REDUCE_SCATTER, NODE_KIND_REGION, NODE_KIND_RESUME, NODE_KIND_RETURN,
    NODE_KIND_STORE, NODE_KIND_TRAP,
};

/// Memory tier requested for a declared program region.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MemoryKind {
    /// Large device memory, lowered to storage bindings by GPU backends.
    Global,
    /// Workgroup-local shared memory.
    Shared,
    /// Cached broadcast memory, lowered to uniform bindings by GPU backends.
    Uniform,
    /// Per-invocation function memory.
    Local,
    /// Immutable device memory for the dispatch lifetime.
    Readonly,
    /// Persistent memory (SSD/NVMe), accessed via `AsyncLoad` into Global memory.
    Persistent,
    /// Push constants, root constants, or a uniform-backed fallback.
    Push,
}

/// Non-binding cache behavior hint for a memory region.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum CacheLocality {
    /// One-pass streaming access.
    Streaming,
    /// Reused temporal access.
    Temporal,
    /// Random access with little spatial predictability.
    Random,
}

/// Non-binding memory optimization hints.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct MemoryHints {
    /// Preferred coalescing axis for multidimensional access.
    pub coalesce_axis: Option<u8>,
    /// Preferred byte alignment. `0` means no explicit preference.
    pub preferred_alignment: u32,
    /// Expected cache locality.
    pub cache_locality: CacheLocality,
}

impl Default for MemoryHints {
    fn default() -> Self {
        Self {
            coalesce_axis: None,
            preferred_alignment: 0,
            cache_locality: CacheLocality::Temporal,
        }
    }
}

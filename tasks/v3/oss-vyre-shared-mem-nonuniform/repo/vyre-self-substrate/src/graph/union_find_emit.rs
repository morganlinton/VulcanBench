//! Union-find substrate consumer.
//!
//! The self-substrate consumes the same backend-neutral IR primitive as any
//! other caller. Concrete drivers are responsible for target emission.

mod dispatch;

#[cfg(any(test, feature = "cpu-parity"))]
mod reference;

#[cfg(test)]
mod tests;

pub use dispatch::{
    union_find_alias_program, union_find_alias_via, union_find_alias_via_into,
    union_find_alias_via_with_scratch_into, UnionFindGpuScratch,
};

#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::{canonicalize_parent_to_roots, reference_union_find_alias};

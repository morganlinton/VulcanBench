//! Tier 2.5 reduction primitives  -  `count`/`min`/`max`/`sum` over
//! bitsets and fixed-width u32 ValueSets.
//!
//! Scalar reductions use one grid-stride workgroup and global atomics
//! so the baseline primitive is parallel instead of serial lane-0
//! scaffolding. Higher-level workgroup-tree reductions still compose
//! these where a caller needs per-workgroup partials or f32 support.

/// `reduce_all` - emit `1` when every lane in a u32 ValueSet is non-zero.
pub mod all;
/// `reduce_any` - emit `1` when any lane in a u32 ValueSet is non-zero.
pub mod any;
mod atomic_scalar;
pub mod count;
pub mod count_non_zero;
pub mod gather;
pub mod histogram;
mod indexed_move;
/// Unsigned maximum over a u32 ValueSet.
pub mod max;
/// Unsigned minimum over a u32 ValueSet.
pub mod min;
pub mod multi_block_prefix_scan;
pub mod radix_sort;
pub mod range_counts;
pub mod scatter;
pub mod segment_reduce;
pub mod sum;
pub mod workgroup_any;
pub mod workgroup_tree;

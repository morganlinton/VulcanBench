//! Vector-widened string interning.
//!
//! # Idea (G9)
//!
//! Tier-B TOML label families carry 60 k+ function-name strings. At
//! load time, build a CHD (compress-hash-displace) perfect hash over
//! all label strings. GPU lookup becomes one `subgroupShuffle` +
//! one DRAM load  -  ~4 cycles. Replaces the linear / tree-based
//! resolver in the current label path.
//!
//! Feature-gated behind `intern` (off by default  -  enabled by
//! a downstream analyzer's label resolver when the label corpus grows past the
//! linear threshold).

/// CHD perfect-hash construction + lookup.
pub mod perfect_hash;

pub use perfect_hash::{build_chd, PerfectHash};

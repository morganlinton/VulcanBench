//! Security topology shim.
//!
//! CRITIQUE_VISION_ALIGNMENT_2026-04-23 V5: `match_order` was hoisted
//! out of this module into the domain-neutral
//! [`crate::range_ordering`] because its behaviour  -  byte-range
//! ordering between two tagged streams  -  is not security-specific.
//! A generic query dialect author writing a non-security program still needed this
//! predicate (e.g., "did lexeme A end before lexeme B?"); pulling
//! `vyre-libs::security` transitively for that was a vision drift.
//!
//! This module now only re-exports the helper under a `#[deprecated]`
//! alias so existing callers continue to compile. New code must
//! import from `vyre_libs::range_ordering`.

pub use crate::range_ordering::{MAX_CACHED_POSITIONS, MAX_DEPTH};

use vyre_foundation::ir::{Expr, Node};

/// Deprecated alias  -  see [`crate::range_ordering::match_order`].
#[deprecated(
    since = "0.6.1",
    note = "byte-range ordering is not security-specific; import `vyre_libs::range_ordering::match_order` instead. CRITIQUE_VISION_ALIGNMENT_2026-04-23 V5."
)]
#[must_use]
pub fn match_order(left_id: Expr, right_id: Expr, res_name: &str) -> (Vec<Node>, Expr) {
    crate::range_ordering::match_order(left_id, right_id, res_name)
}

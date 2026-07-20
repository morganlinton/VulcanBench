//! Deprecated compatibility leaf for substring search.

use vyre::ir::Program;

use crate::scan::substring::{substring_search_with_op_id, LEGACY_MATCHING_SUBSTRING_OP_ID};

/// Canonical replacement module path for diagnostics and generated docs.
pub const CANONICAL_SUBSTRING_MODULE: &str = "vyre_libs::scan::substring";

/// Legacy module path retained for backwards-compatible imports.
pub const LEGACY_SUBSTRING_MODULE: &str = "vyre_libs::matching::substring";

/// Build a substring-search Program with the legacy matching op id.
#[must_use]
pub fn substring_search(
    haystack: &str,
    needle: &str,
    matches: &str,
    haystack_len: u32,
    needle_len: u32,
) -> Program {
    substring_search_with_op_id(
        LEGACY_MATCHING_SUBSTRING_OP_ID,
        haystack,
        needle,
        matches,
        haystack_len,
        needle_len,
    )
}

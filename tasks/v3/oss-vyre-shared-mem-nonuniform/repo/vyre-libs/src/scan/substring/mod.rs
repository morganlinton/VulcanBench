//! Substring-search sub-dialect.
mod substring;

pub use substring::{substring_search, SCAN_SUBSTRING_OP_ID};
pub(crate) use substring::{substring_search_with_op_id, LEGACY_MATCHING_SUBSTRING_OP_ID};

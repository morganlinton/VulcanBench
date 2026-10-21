//! Unit tests for engine prefix helpers.
//!
//! Tests extracted from `engine/prefix.rs`.

use crate::engine::prefix::*;
use crate::error::Result;

/// Brace depth prefix over empty input returns a single zero sentinel.
#[test]
pub(super) fn brace_depth_empty() -> Result<()> {
    assert_eq!(brace_depth_prefix(b"")?, vec![0]);
    Ok(())
}

/// Brace depth prefix correctly tracks nested braces.
#[test]
pub(super) fn brace_depth_nested() -> Result<()> {
    assert_eq!(
        brace_depth_prefix(b"a{b{c}d}e")?,
        vec![0, 0, 1, 1, 2, 2, 1, 1, 0]
    );
    Ok(())
}

/// Nested depth prefix counts combined braces and parentheses.
#[test]
pub(super) fn nested_depth_braces_and_parens() -> Result<()> {
    assert_eq!(nested_depth_prefix(b"f({x})")?, vec![0, 0, 1, 2, 2, 1]);
    Ok(())
}

/// Newline prefix sum yields O(1) line-distance queries.
#[test]
pub(super) fn newline_prefix() -> Result<()> {
    let sums = newline_prefix_sum(b"a\nb\nc")?;
    assert_eq!(sums, vec![0, 0, 1, 1, 2, 2]);
    // newline_count_between(0, 4) = sums[4] - sums[0] = 2
    assert_eq!(sums[4] - sums[0], 2);
    // newline_count_between(2, 4) = sums[4] - sums[2] = 1
    assert_eq!(sums[4] - sums[2], 1);
    Ok(())
}

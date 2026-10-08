//! GitHub Flavored Markdown tables.
//!
//! Not yet ported: this target must produce exactly the output of the
//! reference implementation for tables. Until then no table is recognised.

use crate::nodes::Node;
use crate::parser::Parser;

/// Try to open a table (or the next table row) at `line` inside `container`.
/// Returns the new container and two flags as the block parser expects.
pub fn try_opening_block<'a>(
    _parser: &mut Parser<'a, '_, '_>,
    _container: Node<'a>,
    _line: &str,
) -> Option<(Node<'a>, bool, bool)> {
    None
}

/// Whether `line` is a table row (used to continue an open table).
pub fn matches(_line: &str, _spoiler: bool) -> bool {
    false
}

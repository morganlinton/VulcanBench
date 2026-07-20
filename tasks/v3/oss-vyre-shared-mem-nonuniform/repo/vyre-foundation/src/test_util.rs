//! Shared helpers for optimizer and transform unit tests.

use crate::ir::{Node, Program};

/// Return the effective entry body of a `Program`.
///
/// F-IR-29 invariant: `Program::wrapped` produces an entry whose first (and
/// usually only) top-level node is `Node::Region`. Most tests that inspect
/// the optimized IR need to look inside that Region. This helper hides the
/// unwrap so tests stay consistent even when the program has already been
/// through `region_inline` and the wrapper is gone.
pub(crate) fn region_body(program: &Program) -> &[Node] {
    match program.entry() {
        [Node::Region { body, .. }] => body,
        other => other,
    }
}

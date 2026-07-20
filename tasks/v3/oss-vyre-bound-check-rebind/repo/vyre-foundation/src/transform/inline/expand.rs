//! Callee expansion for the inline pass.
//!
//! This module rewrites a callee's IR body into the caller's namespace,
//! renaming variables, substituting input arguments, and hoisting nested
//! expressions into prefix statements.

pub(crate) use callee_expander::CalleeExpander;

/// Per-inline expansion state and variable renaming tables.
pub(super) mod callee_expander;
/// Expression- and node-level expansion implementations.
pub(super) mod impl_calleeexpander;

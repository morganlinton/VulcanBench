//! Induction-variable substitution for the loop passes.
//!
//! The implementation lives in [`crate::transform::subst`] so the optimizer
//! loop passes and reverse-mode autodiff share exactly one complete `var ->
//! expr` rewrite (no duplicated, drift-prone copy). This module is a local
//! alias kept so existing `super::substitution::...` imports stay stable.

pub(super) use crate::transform::subst::{substitute_node, substitute_nodes};

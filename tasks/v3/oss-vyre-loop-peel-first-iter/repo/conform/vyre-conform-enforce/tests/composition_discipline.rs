//! Composition discipline CI gates.
//!
//! These tests enforce the "After Effects" compositional architecture:
//!
//! 1. **No monoliths**  -  every registered op must stay under a complexity
//!    budget. If it exceeds the threshold, the author must split the op
//!    into smaller, reusable compositions.
//!
//! 2. **No reimplementation**  -  if an op's IR contains a subgraph that
//!    structurally matches another registered op, the author must call
//!    that op via `Expr::Call` instead of inlining its logic.
//!
//! Together these gates enforce a composition ratchet: the op catalog
//! grows organically, and every new composition automatically benefits
//! every pipeline that calls it.
//!
//! Implementation lives in two `include!`-d chunks under `__split/`.

include!("__split/composition_discipline_chunk1.rs");
include!("__split/composition_discipline_chunk2.rs");

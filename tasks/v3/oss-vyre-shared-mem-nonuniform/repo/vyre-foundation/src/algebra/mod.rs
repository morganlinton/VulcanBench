//! Algebraic-laws surface  -  registration of provable algebraic
//! identities + composition machinery the optimizer's algebraic
//! catalog (`optimizer/passes/algebraic/`) consumes.
//!
//! Audit cleanup A12 (2026-04-30): grouped from `vyre-foundation/src/`
//! root scatter.

/// Inventory-registered algebraic-law registry (`algebraic_law_registry::laws_for_op`).
pub mod algebraic_law_registry;
/// Region composition + duplicate-self-exclusive expansion helpers.
pub mod composition;

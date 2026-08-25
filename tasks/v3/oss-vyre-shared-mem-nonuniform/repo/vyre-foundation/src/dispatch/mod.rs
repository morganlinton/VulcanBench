//! Dispatch surface  -  dialect lookup, extern op registry, extension hooks.
//!
//! Audit cleanup A12 (2026-04-30): grouped from `vyre-foundation/src/`
//! root scatter.

/// Dialect → backend-impl `dialect_lookup::DialectLookup` table.
pub mod dialect_lookup;
/// Inventory-registered extension hooks (`OpaqueExprResolver`,
/// `OpaqueNodeResolver`, etc).
pub mod extension;
/// External op factory registry.
pub mod extern_registry;

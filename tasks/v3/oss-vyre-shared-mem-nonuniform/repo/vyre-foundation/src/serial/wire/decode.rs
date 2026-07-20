//! Decode `VIR0` bytes into the stable IR wire model.
#![allow(unused_doc_comments)]

/// Deserialize a complete wire-format program.
pub use from_wire::from_wire;
/// Scan database decode budget guard for attacker-controlled cache headers.
pub use scan_database_budget::{
    validate_scan_construct_decode_budget, validate_scan_database_decode_budget,
    ScanConstructDecodeBudget, ScanConstructDecodeBudgetEvidence,
    ScanConstructDecodeShape, ScanDatabaseDecodeBudget, ScanDatabaseDecodeBudgetError,
    ScanDatabaseDecodeBudgetEvidence, ScanDatabaseDecodeShape,
};

/// Reject extension ids that collide with the frozen core tag space.
#[inline]
pub(crate) fn reject_reserved_extension_id(raw: u32, surface: &str) -> Result<u32, String> {
    if (raw & 0x8000_0000) == 0 {
        return Err(format!(
            "InvalidDiscriminant: {surface} opaque id 0x{raw:08x} collides with core IR. Fix: dialect extensions must use ids in 0x8000_0000..=0xffff_ffff."
        ));
    }
    Ok(raw)
}

/// Top-level wire-format program decoder.
pub mod from_wire;
/// Per-variant Reader methods (reads each Node/Expr shape).
pub mod impl_reader;
/// Semantic decode invariants shared by wire readers.
pub(crate) mod invariants;
/// Decode-side budgets for serialized scan database payloads.
pub mod scan_database_budget;

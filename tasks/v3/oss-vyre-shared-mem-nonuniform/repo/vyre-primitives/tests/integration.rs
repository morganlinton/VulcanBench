//! Integration smoke: Tier 2.5 registry is linked and at least one op builds.
//!
//! Requires `hash` + `inventory-registry` (see `Cargo.toml` `[[test]]`).
//! Deeper property/adversarial coverage lives in `vyre-libs` (consumers) and
//! will expand here as the master plan matures.
#![forbid(unsafe_code)]

use vyre_foundation::ir::Program;
use vyre_primitives::hash::fnv1a::fnv1a32_program;

#[test]
fn inventory_registry_exposes_primitives() {
    let mut ids: Vec<_> = vyre_primitives::harness::all_entries()
        .map(|e| e.id)
        .collect();
    ids.sort_unstable();
    assert!(
        !ids.is_empty(),
        "expected vyre-primitives with `inventory-registry` to register at least one op"
    );
    assert!(ids.iter().all(|s| s.starts_with("vyre-primitives::")));
    let p: Program = fnv1a32_program("in", "out", 4);
    p.validate()
        .unwrap_or_else(|e| panic!("expected fnv1a32 Program to validate: {e}"));
}

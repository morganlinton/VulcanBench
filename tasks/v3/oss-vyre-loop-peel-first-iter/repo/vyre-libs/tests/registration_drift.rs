//! Registration drift gate.
//!
//! Vyre has two complementary inventories:
//! - `DialectRegistry`  -  frozen `OpDef` records (signature, category,
//!   contract) used by the validator + optimizer.
//! - `vyre-libs::harness::OpEntry`  -  fixture bundle (build + inputs +
//!   expected_output) iterated by conform harnesses.
//!
//! This gate asserts the direction that matters for correctness: every
//! `OpDef` declared in the DialectRegistry must have a matching
//! `OpEntry` *or* intrinsic harness entry so the conform harness
//! actually exercises it. A declared op with no test is a LAW 5
//! violation (no adversarial coverage).
//!
//! The opposite direction  -  Cat-A composition ops registered only
//! through OpEntry without a separate OpDefRegistration  -  is an
//! architectural property of the Tier-3 harness pattern, not drift.

use std::collections::HashSet;

use vyre_driver::registry::DialectRegistry;

/// Ids declared in the registry whose executable coverage lives in a
/// subsystem-specific test instead of the fixture harness.
/// Every entry must carry a concrete reason.
const EXEMPT_OP_IDS: &[(&str, &str)] = &[
    (
        "core.indirect_dispatch",
        "Runtime-only op  -  exercised end-to-end by runtime dispatch tests, not a fixture harness.",
    ),
    (
        "io.dma_from_nvme",
        "IO op  -  requires NVMe block device; covered by runtime IO tests, not the fixture harness.",
    ),
    (
        "io.write_back_to_nvme",
        "IO op  -  requires NVMe block device; covered by runtime IO tests, not the fixture harness.",
    ),
    (
        "mem.unmap",
        "Memory lifecycle op  -  covered by runtime memory tests, not the fixture harness.",
    ),
    (
        "mem.zerocopy_map",
        "Memory lifecycle op  -  covered by runtime memory tests, not the fixture harness.",
    ),
];

#[test]
fn every_dialect_registered_op_has_a_test_entry() {
    let registry = DialectRegistry::global();
    let exemptions: std::collections::HashMap<&str, &str> = EXEMPT_OP_IDS.iter().copied().collect();

    let mut tested: HashSet<&'static str> = HashSet::new();
    for entry in vyre_libs::harness::all_entries() {
        tested.insert(entry.id);
    }

    let mut drift: Vec<String> = Vec::new();
    for op_def in registry.iter() {
        if tested.contains(op_def.id) {
            continue;
        }
        if exemptions.contains_key(op_def.id) {
            continue;
        }
        drift.push(op_def.id.to_string());
    }

    if !drift.is_empty() {
        drift.sort();
        let mut rendered = String::from(
            "registration drift: op declared in DialectRegistry but no harness OpEntry.\n",
        );
        for id in &drift {
            rendered.push_str(&format!("  - {id}\n"));
        }
        rendered.push_str(
            "Fix: (a) submit an OpEntry under vyre-libs::harness or \
             vyre-intrinsics::harness, or (b) add the op id + a reason \
             to EXEMPT_OP_IDS in this test file.\n",
        );
        panic!("{rendered}");
    }
}

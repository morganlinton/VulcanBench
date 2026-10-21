//! `cargo_full run --bin xtask -- list-ops`  -  walk the central op registries and print
//! the complete catalog grouped by tier.
//!
//! Walks: `vyre-libs`, `vyre-intrinsics`, and `vyre-primitives` (Tier 2.5,
//! requires `inventory-registry` on the primitives crate). IDs are
//! de-duplicated per tier; some Tier-2.5 builders intentionally register
//! under the `vyre-libs::` namespace (compat shims)  -  they appear under the
//! tier that matches their prefix, not the crate that submitted them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Tier classification based on op ID prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::enum_variant_names)]
enum Tier {
    Tier2Intrinsics,   // vyre-intrinsics::*
    Tier2_5Primitives, // vyre-primitives::*
    Tier3Libraries,    // vyre-libs::<domain>::*
    Tier4Composites,   // vyre-libs::composite::*
    Unknown,
}

impl Tier {
    fn as_str(&self) -> &'static str {
        match self {
            Tier::Tier2Intrinsics => "Tier 2  -  Hardware Intrinsics (Naga-required)",
            Tier::Tier2_5Primitives => "Tier 2.5  -  Primitive LEGO Blocks (Generic)",
            Tier::Tier3Libraries => "Tier 3  -  Domain Libraries (Pure Cat-A)",
            Tier::Tier4Composites => "Tier 4  -  Composite Multi-Step Ops",
            Tier::Unknown => "Unknown Tier",
        }
    }
}

pub(crate) fn run(args: &[String]) {
    let out_path = parse_write_flag(args);
    let mut catalog: BTreeMap<Tier, BTreeSet<String>> = BTreeMap::new();

    for entry in vyre_libs::harness::all_entries() {
        catalog
            .entry(classify_id(entry.id))
            .or_default()
            .insert(entry.id.to_string());
    }

    for entry in vyre_intrinsics::harness::all_entries() {
        catalog
            .entry(Tier::Tier2Intrinsics)
            .or_default()
            .insert(entry.id.to_string());
    }

    // Tier 2.5: LEGO primitives (requires `vyre-primitives` with `inventory-registry`).
    for entry in vyre_primitives::harness::all_entries() {
        catalog
            .entry(classify_id(entry.id))
            .or_default()
            .insert(entry.id.to_string());
    }

    let body = build_markdown(&catalog);
    print!("{body}");
    if let Some(path) = out_path {
        if let Some(parent) = Path::new(&path).parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Fix: create_dir_all {parent:?}: {e}");
                std::process::exit(1);
            }
        }
        if let Err(e) = fs::write(&path, &body) {
            eprintln!("Fix: write {}: {e}", path);
            std::process::exit(1);
        }
        eprintln!("Wrote {path} (op inventory snapshot).");
    }
}

fn parse_write_flag(args: &[String]) -> Option<String> {
    for w in args.windows(2) {
        if w[0] == "--write" {
            return Some(w[1].clone());
        }
    }
    None
}

fn build_markdown(catalog: &BTreeMap<Tier, BTreeSet<String>>) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "Vyre Operation Catalog\n\
         ======================\n\
         \n\
         Auto-generated: `cargo_full run --bin xtask -- list-ops` (xtask; walks inventory).\n"
    );
    for (tier, ids) in catalog {
        let mut v: Vec<_> = ids.iter().cloned().collect();
        v.sort();
        let _ = writeln!(s, "\n{} [{} ops]", tier.as_str(), v.len());
        for id in v {
            let _ = writeln!(s, "  - `{id}`");
        }
    }
    s
}

fn classify_id(id: &str) -> Tier {
    if id.starts_with("vyre-intrinsics::") {
        Tier::Tier2Intrinsics
    } else if id.starts_with("vyre-primitives::") {
        Tier::Tier2_5Primitives
    } else if id.starts_with("vyre-libs::composite::") {
        Tier::Tier4Composites
    } else if id.starts_with("vyre-libs::") {
        Tier::Tier3Libraries
    } else {
        Tier::Unknown
    }
}

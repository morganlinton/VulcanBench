//! Runtime category-classification consistency check (F-IR-34).
//!
//! Complements the build-time scanner in `build.rs` by walking the linked
//! `inventory::iter::<OpDefRegistration>` and asserting every entry obeys the
//! A/B/C invariant.
//!
//! Invariant (machine-checked):
//! * `Category::Composite` (A) **must** have `lowerings.primary_text == None`.
//! * `Category::Intrinsic` (B/C) may have `primary_text` present or absent.
//! * `primary_text: Some(...)` **must not** appear on a `Category::Composite` op.

use vyre_driver::registry::{Category, OpDefRegistration};
use vyre_foundation::dialect_lookup::PrimaryTextBuilder;

/// Assert that a single op's declared category matches its lowering table.
///
/// # Panics
///
/// Panics with an actionable `Fix:` message when a Category-A op carries a
/// dedicated target builder arm  -  the exact drift shape that F-IR-34 exists to catch.
pub fn check_opdef(id: &str, category: Category, primary_text: Option<PrimaryTextBuilder>) {
    if category == Category::Composite && primary_text.is_some() {
        panic!(
            "category classification mismatch for op `{id}`: declared Composite (Category A) but lowering table says Some(PrimaryTextBuilder). Fix: Category A ops must be pure IR composition with no dedicated target builder arm."
        );
    }
}

/// Walk every linked `OpDefRegistration` and assert the invariant.
///
/// # Panics
///
/// Panics on the first violating entry.
pub fn check_all_inventory_opdefs() {
    for reg in inventory::iter::<OpDefRegistration>() {
        let def = (reg.op)();
        check_opdef(def.id, def.category, def.lowerings.primary_text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_driver::registry::Category;

    fn dummy_primary_text(
        _: &vyre_foundation::dialect_lookup::LoweringCtx<'_>,
    ) -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn composite_without_primary_text_passes() {
        // Category A, pure IR composition  -  no target builder arm.  This is the
        // canonical correct shape.
        check_opdef("test.cat_a_ok", Category::Composite, None);
    }

    #[test]
    fn intrinsic_without_primary_text_passes() {
        // Category C runtime-only op (e.g. core.indirect_dispatch)  -
        // Intrinsic category but no target-text arm yet.
        check_opdef("test.cat_c_ok", Category::Intrinsic, None);
    }

    #[test]
    fn intrinsic_with_primary_text_passes() {
        // Category B op with a dedicated target builder arm.
        check_opdef(
            "test.cat_b_ok",
            Category::Intrinsic,
            Some(dummy_primary_text),
        );
    }

    #[test]
    #[should_panic(
        expected = "category classification mismatch for op `test.cat_a_bad`: declared Composite (Category A) but lowering table says Some(PrimaryTextBuilder). Fix: Category A ops must be pure IR composition with no dedicated target builder arm."
    )]
    fn composite_with_primary_text_panics() {
        // This is the drift shape: an op claims to be pure composition but
        // secretly requires a backend-specific target builder arm.
        check_opdef(
            "test.cat_a_bad",
            Category::Composite,
            Some(dummy_primary_text),
        );
    }

    #[test]
    fn inventory_walk_does_not_panic() {
        // Exercises every OpDefRegistration linked into the current test
        // binary (vyre-driver core + io ops, plus any dev-dependencies).
        check_all_inventory_opdefs();
    }
}

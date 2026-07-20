//! Inventory-backed OpEntry registry for the intrinsic-differential harness.
//!
//! **Registry Layering**: This file defines the `OpEntry` registry for Tier-2 hardware intrinsics.
//! It operates in parallel with the Cat-A registry (`vyre-harness::OpEntry`) and the Tier-2.5 primitives registry (`vyre-primitives::harness::OpEntry`).
//! For an architectural overview of this three-registry split, see `vyre-harness/README.md`.
//!
//! Every Cat-C intrinsic registers one `OpEntry` via `inventory::submit!`.
//! The test `tests/hardware_conform.rs` iterates the inventory and
//! asserts each op's CPU reference matches the declared
//! `expected_output` bit-for-bit.

use vyre_foundation::ir::Program;

pub type Fixture = Vec<Vec<u8>>;
pub type Fixtures = Vec<Fixture>;
pub type InputsFn = fn() -> Fixtures;
pub type ExpectedFn = fn() -> Fixtures;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareSemantic {
    UnaryU32Map,
    BarrierIdentityU32,
    FmaF32,
    InverseSqrtF32,
    SubgroupAddU32,
    SubgroupBallotU32,
    SubgroupShuffleU32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpShape {
    pub input_buffers: u8,
    pub output_buffers: u8,
    pub lane_bytes: u8,
    pub semantic: HardwareSemantic,
}

impl OpShape {
    #[must_use]
    pub const fn new(
        input_buffers: u8,
        output_buffers: u8,
        lane_bytes: u8,
        semantic: HardwareSemantic,
    ) -> Self {
        Self {
            input_buffers,
            output_buffers,
            lane_bytes,
            semantic,
        }
    }

    #[must_use]
    pub const fn total_buffers(self) -> u8 {
        self.input_buffers + self.output_buffers
    }
}

#[non_exhaustive]
pub struct OpEntry {
    pub id: &'static str,
    pub build: fn() -> Program,
    pub test_inputs: Option<InputsFn>,
    pub expected_output: Option<ExpectedFn>,
    /// Coarse-grained taxonomy tag (T028 / SEPARATION_AUDIT S2 prep).
    /// Examples: `"hardware"`, `"math"`, `"crypto"`, `"graph"`,
    /// `"matching"`. `None` means uncategorised  -  equivalent to the
    /// pre-T028 behaviour. Use `OpEntry::with_category` to set it
    /// without losing the const-fn `new` constructor.
    pub category: Option<&'static str>,
    /// Declarative hardware shape for generated conformance, lowering, and
    /// backend release gates. `None` is retained for external legacy entries.
    pub shape: Option<OpShape>,
}

impl OpEntry {
    /// Construct an `OpEntry` with all required fields set. Exists so
    /// external intrinsic packs can `inventory::submit!(OpEntry::new(...))`
    /// despite the struct being `#[non_exhaustive]` (V7-EXT-003).
    /// `category` initialises to `None`; chain `with_category` if a
    /// category is required at submission time.
    #[must_use]
    pub const fn new(
        id: &'static str,
        build: fn() -> Program,
        test_inputs: Option<InputsFn>,
        expected_output: Option<ExpectedFn>,
    ) -> Self {
        Self {
            id,
            build,
            test_inputs,
            expected_output,
            category: None,
            shape: None,
        }
    }

    /// Set the category and return `self`. `const`-friendly so callers
    /// can write `OpEntry::new(...).with_category("hardware")` inside
    /// `inventory::submit!`.
    #[must_use]
    pub const fn with_category(mut self, category: &'static str) -> Self {
        self.category = Some(category);
        self
    }

    /// Set the declarative hardware shape and return `self`.
    #[must_use]
    pub const fn with_shape(mut self, shape: OpShape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Return the registered coarse-grained taxonomy tag, if any.
    #[must_use]
    pub const fn category(&self) -> Option<&'static str> {
        self.category
    }

    /// Return the registered declarative hardware shape, if any.
    #[must_use]
    pub const fn shape(&self) -> Option<OpShape> {
        self.shape
    }
}

inventory::collect!(OpEntry);

pub fn all_entries() -> impl Iterator<Item = &'static OpEntry> {
    inventory::iter::<OpEntry>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Program;

    fn empty_build() -> Program {
        Program::default()
    }

    #[test]
    fn new_initialises_category_to_none() {
        let entry = OpEntry::new("test::id", empty_build, None, None);
        assert_eq!(entry.category(), None);
        assert_eq!(entry.shape(), None);
    }

    #[test]
    fn with_category_sets_and_returns_self() {
        let entry = OpEntry::new("test::id", empty_build, None, None).with_category("hardware");
        assert_eq!(entry.category(), Some("hardware"));
    }

    #[test]
    fn with_shape_sets_and_returns_self() {
        let shape = OpShape::new(1, 1, 4, HardwareSemantic::UnaryU32Map);
        let entry = OpEntry::new("test::id", empty_build, None, None).with_shape(shape);
        assert_eq!(entry.shape(), Some(shape));
        assert_eq!(shape.total_buffers(), 2);
    }

    #[test]
    fn registered_hardware_entries_carry_hardware_category() {
        // Every inventory::submit! site under hardware/ must declare
        // category = Some("hardware")  -  the T028 contract for vyre-
        // intrinsics. If a future hardware op forgets the field this
        // test fires.
        let mut hardware_count = 0;
        let mut categorised = 0;
        for entry in all_entries() {
            if entry.id.starts_with("vyre-intrinsics::hardware::") {
                hardware_count += 1;
                if entry.category() == Some("hardware") {
                    categorised += 1;
                }
            }
        }
        assert_eq!(
            categorised, hardware_count,
            "every vyre-intrinsics::hardware::* op must declare category=Some(\"hardware\")  -  counted {hardware_count} hardware ops, {categorised} carry the category"
        );
        assert!(
            hardware_count > 0,
            "expected at least one registered vyre-intrinsics::hardware:: op"
        );
    }

    #[test]
    fn registered_hardware_entries_carry_shapes() {
        let mut hardware_count = 0;
        let mut shaped = 0;
        for entry in all_entries() {
            if entry.id.starts_with("vyre-intrinsics::hardware::") {
                hardware_count += 1;
                if entry.shape().is_some() {
                    shaped += 1;
                }
            }
        }
        assert_eq!(
            shaped, hardware_count,
            "every vyre-intrinsics::hardware::* op must declare OpShape metadata"
        );
        assert!(
            hardware_count > 0,
            "expected at least one registered vyre-intrinsics::hardware:: op"
        );
    }
}

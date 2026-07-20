#![forbid(unsafe_code)]
//! Universal Cat-A op harness registry + Region builder.
//!
//! **Registry Layering**: This file defines the `OpEntry` registry for Tier-3 Cat-A compositions.
//! It operates in parallel with the Tier-2.5 primitives registry (`vyre-primitives::harness::OpEntry`) and the Tier-2 hardware intrinsics registry (`vyre-intrinsics::harness::OpEntry`).
//! For an architectural overview of this three-registry split, see `vyre-harness/README.md`.
//!
//! Every Cat-A composition that participates in automated harness
//! checks registers one `OpEntry` through `inventory::submit!`. The
//! conform integration test at `tests/universal_harness.rs` discovers
//! every entry and validates: program validity, wire round-trip, CSE
//! stability, and (when available) CPU-oracle parity.
//!
//! The crate also re-exports the Region builder used by every Cat-A
//! library to wrap its produced `Vec<Node>` so optimizer passes treat
//! the library call as an opaque unit by default. See
//! [`region`](self::region) for `wrap`, `wrap_anonymous`, `wrap_child`,
//! `tag_program`.

pub mod fp_contract;
pub mod region;

pub use region::{reparent_program_children, tag_program, wrap, wrap_anonymous, wrap_child};

/// Re-exported so the [`vyre_op!`] macro can call into `inventory`
/// without callers needing to add it as a direct dependency.
#[doc(hidden)]
pub use inventory;

use vyre::ir::Program;

/// Canonical operation tier used by harness, catalog, and matrix gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpTier {
    /// Foundation-level IR rewrite or built-in IR operation.
    FoundationIr,
    /// Tier-2 hardware intrinsic.
    Intrinsic,
    /// Tier-2.5 reusable primitive.
    Primitive,
    /// Tier-3 library composition.
    Libs,
    /// Runtime or driver-owned operation.
    Runtime,
    /// External consumer registered into the shared harness.
    External,
    /// Identifier does not match any accepted registry namespace.
    Unknown,
}

impl OpTier {
    /// Return the stable `OP_MATRIX.toml` tier spelling.
    #[must_use]
    pub const fn matrix_value(self) -> &'static str {
        match self {
            Self::FoundationIr => "foundation_ir",
            Self::Intrinsic => "intrinsic",
            Self::Primitive => "primitive",
            Self::Libs => "libs",
            Self::Runtime => "runtime",
            Self::External => "external",
            Self::Unknown => "unknown",
        }
    }
}

/// Classify an operation id by the canonical namespace contract.
#[must_use]
pub fn classify_op_id(id: &str) -> OpTier {
    if id.starts_with("vyre-intrinsics::hardware::") {
        OpTier::Intrinsic
    } else if id.starts_with("vyre-primitives::") {
        OpTier::Primitive
    } else if id.starts_with("vyre-libs::") {
        OpTier::Libs
    } else if id.starts_with("core.") || id.starts_with("io.") || id.starts_with("mem.") {
        OpTier::Runtime
    } else if is_external_crate_namespace(id) {
        OpTier::External
    } else {
        OpTier::Unknown
    }
}

fn is_external_crate_namespace(id: &str) -> bool {
    let Some((crate_name, _)) = id.split_once("::") else {
        return false;
    };
    !crate_name.is_empty() && !crate_name.starts_with("vyre-")
}

/// Deterministic fixture input cases.
pub type InputsFn = fn() -> Vec<Vec<Vec<u8>>>;
/// Deterministic expected-output fixtures.
pub type ExpectedFn = fn() -> Vec<Vec<Vec<u8>>>;

/// Shared migration-compatible fixture descriptor for registered Cat-A programs.
///
/// At migration time, new entries may still rely on the
/// [`OpEntry::expected_output`] field while legacy entries that only
/// set `expected_output` and omit an oracle are skipped from oracle
/// comparison. Once all entries migrate, `expected_output` is
/// deprecated but kept for back-compat until that migration completes.
//
// The struct is intentionally NOT `#[non_exhaustive]` so the dozens of
// in-tree vyre-libs registrations (graph/, parsing/, security/, …) can
// continue to use struct-literal syntax. External consumers should still
// prefer `OpEntry::new(...)` to keep their code resilient to future
// fields, but every cross-crate field addition will be accompanied by
// either a bump or a defaulted helper.
pub struct OpEntry {
    /// Stable operation identifier.
    pub id: &'static str,

    /// Construct the [`Program`] under test.
    pub build: fn() -> Program,

    /// Deterministic fixture input bytes in declaration order.
    ///
    /// The harness passes this into both `vyre_reference::reference_eval` and the
    /// legacy `expected_output` oracle when they are both provided.
    pub test_inputs: Option<InputsFn>,

    /// Legacy fixture oracle output bytes.
    ///
    /// Kept during migration so existing registrations in
    /// `src/{math,nn,crypto,matching}` remain buildable without edits.
    pub expected_output: Option<ExpectedFn>,

    /// Coarse-grained taxonomy tag (T028 / SEPARATION_AUDIT S2 prep).
    /// Examples: `"math"`, `"nn"`, `"crypto"`, `"scan"`, `"parsing"`,
    /// `"graph"`, `"security"`, `"dataflow"`, `"compiler"`. `None`
    /// means uncategorised  -  equivalent to the pre-T028 behaviour.
    pub category: Option<&'static str>,
}

impl OpEntry {
    /// Construct an `OpEntry` with all required fields set. Exists so
    /// community Cat-A crates can `inventory::submit!(OpEntry::new(...))`
    /// despite the struct being `#[non_exhaustive]` (V7-EXT-004).
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
        }
    }

    /// Set the category and return `self`. `const`-friendly so callers
    /// can write `OpEntry::new(...).with_category("math")` inside
    /// `inventory::submit!`.
    #[must_use]
    pub const fn with_category(mut self, category: &'static str) -> Self {
        self.category = Some(category);
        self
    }

    /// Return the registered coarse-grained taxonomy tag, if any.
    #[must_use]
    pub const fn category(&self) -> Option<&'static str> {
        self.category
    }

    /// Allowed output drift in ULPs for f32-producing backends.
    ///
    /// `0` means byte-identity is required. Non-zero tolerances are used only
    /// for ops whose contract already permits backend-defined transcendental
    /// drift OR whose lowered IR contains f32 mul+add chains that the WGSL
    /// implementation is allowed to fuse into a single FMA (one rounding
    /// instead of two). Catalog wrappers like
    /// `vyre-libs::catalog::math::<name>::consumer_a` are mapped to the
    /// underlying primitive id so they inherit the primitive's tolerance
    /// without a hand-maintained per-wrapper row (Q5 in
    /// `docs/optimization/ROADMAP.md`).
    #[must_use]
    pub fn tolerance(&self) -> u32 {
        Self::tolerance_for_id(self.id)
    }

    /// Resolve the ULP tolerance for an op id, normalising catalog wrapper
    /// ids through their underlying primitive path first.
    ///
    /// Public for cross-crate consumers (e.g. the conformance harness)
    /// that want to consult the tolerance contract without holding a
    /// concrete `OpEntry`.
    #[must_use]
    pub fn tolerance_for_id(id: &str) -> u32 {
        if let Some(path) = catalog_primitive_path(id) {
            return primitive_tolerance_for_path(path);
        }
        explicit_tolerance_for_id(id)
    }
}

fn explicit_tolerance_for_id(id: &str) -> u32 {
    match id {
        "vyre-libs::nn::softmax" => 1,
        "vyre-libs::nn::attention" => 4,
        "vyre-libs::nn::gqa_attention" => 4,
        "vyre-libs::nn::layer_norm" => 1,
        "vyre-libs::nn::silu" => 1,
        // Observed 2-ULP drift on a 5090 lane (CPU 0x415dd0f4 vs
        // GPU 0x415dd0f6) under FMA fusion in cat_a_gpu_differential.
        "vyre-libs::nn::logit_softcap" => 2,
        "vyre-libs::nn::rms_norm" => 2,
        "vyre-libs::nn::rms_norm_linear" => 2,
        "vyre-libs::math::fft::fft_convolve_circular_complex" => 4,
        "vyre-libs::optim::newton_schulz_5step" => 64,
        // `decay*ema + (1-decay)*theta`  -  straight mul+add chain,
        // one lane drifts 1 ULP from CPU's serial mul+add+add to
        // GPU's fused mul-add (WGSL-spec-allowed).
        "vyre-libs::optim::ema_apply" => 1,
        // MuonEq-R computes momentum and Nesterov update through
        // adjacent f32 mul+add chains. Random-input WGPU parity observed
        // 6-ULP drift on an RTX backend under allowed FMA contraction.
        "vyre-libs::optim::muoneq_r" => 8,
        // Newton-Schulz Cat-A primitive: the polynomial has nested
        // mul+add steps that fuse to FMA on GPU. Random-input parity
        // observed 24-ULP drift across catalog wrappers.
        "vyre-primitives::math::newton_schulz_poly5_f32" => 32,
        _ => 0,
    }
}

fn primitive_tolerance_for_path(path: &str) -> u32 {
    match path {
        "math::newton_schulz_poly5_f32" => 32,
        _ => 0,
    }
}

fn catalog_primitive_path(id: &str) -> Option<&str> {
    let rest = id.strip_prefix("vyre-libs::catalog::")?;
    rest.strip_suffix("::consumer_a")
        .or_else(|| rest.strip_suffix("::consumer_b"))
}

inventory::collect!(OpEntry);

/// Return all registered operation entries.
pub fn all_entries() -> impl Iterator<Item = &'static OpEntry> {
    inventory::iter::<OpEntry>()
}

/// Fixpoint contract for dataflow ops whose GPU body performs one
/// iteration per dispatch.
///
/// Submitting a `FixpointRegistration` alongside an `OpEntry` tells the
/// conform harness to call `backend.dispatch` in a loop until the
/// `converged_flag_buffer` reads zero before comparing against the CPU
/// reference. Without this registration such ops would always diverge
/// in a single-dispatch byte-identity test even though their lowering
/// is correct.
#[derive(Clone, Debug)]
pub struct FixpointContract {
    /// Name of the RW buffer whose bytes-interpreted-as-`u32` must
    /// equal zero for the fixpoint loop to terminate. Semantics: the
    /// GPU body writes `1` whenever any lane updated shared state;
    /// the driver clears it between iterations.
    pub converged_flag_buffer: &'static str,
    /// Hard cap on driver iterations before the loop bails out. Every
    /// fixpoint op MUST reach its answer in a known-bounded number of
    /// steps so the harness cannot hang.
    pub max_iterations: u32,
}

/// Link-time registration binding a fixpoint contract to an op id.
pub struct FixpointRegistration {
    /// Stable op id (`OpEntry::id`) this contract applies to.
    pub op_id: &'static str,
    /// Fixpoint contract parameters.
    pub contract: FixpointContract,
}

inventory::collect!(FixpointRegistration);

/// Look up the fixpoint contract registered for `op_id`, if any.
#[must_use]
pub fn fixpoint_contract(op_id: &str) -> Option<&'static FixpointContract> {
    inventory::iter::<FixpointRegistration>()
        .find(|registration| registration.op_id == op_id)
        .map(|registration| &registration.contract)
}

/// Convergence contract for ops whose GPU body performs one
/// iteration per dispatch and needs an external driver loop to
/// reach fixpoint before byte-identity comparison.
///
/// Submitting a `ConvergenceContract` alongside an `OpEntry` tells
/// the conform harness to dispatch the backend in a loop (transfer
/// step + `bitset_fixpoint` convergence check) until the changed
/// flag clears or the iteration budget is exhausted.
#[derive(Clone, Debug)]
pub struct ConvergenceContract {
    /// Stable op id (`OpEntry::id`) this contract applies to.
    pub op_id: &'static str,
    /// Hard cap on driver iterations before the loop bails out.
    pub max_iterations: u32,
}

inventory::collect!(ConvergenceContract);

/// Look up the convergence contract registered for `op_id`, if any.
#[must_use]
pub fn convergence_contract(op_id: &str) -> Option<&'static ConvergenceContract> {
    inventory::iter::<ConvergenceContract>().find(|contract| contract.op_id == op_id)
}

// Tolerance metadata and capability requirements are encoded in
// [`OpEntry::tolerance`] and checked by the conform lenses directly.
// There is no global exemption registry; every registered op must
// provide runnable `test_inputs` / `expected_output` fixtures or fail
// loudly with a diagnostic.

/// Declarative op registration shorthand (ROADMAP S8 generator half).
///
/// One source declares a Cat-A op; the macro expands to the matching
/// `inventory::submit!{ OpEntry { .. } }` so writers can't accidentally
/// drift from the canonical `OpEntry` shape.
///
/// ## Forms
///
/// ```ignore
/// // Minimal: just id + builder.
/// vyre_harness::vyre_op! {
///     id: "vyre-libs::math::matmul",
///     build: || matmul("a", "b", "out", 2, 2, 2),
/// }
///
/// // Full: explicit fixtures.
/// vyre_harness::vyre_op! {
///     id: "vyre-libs::math::matmul",
///     build: || matmul("a", "b", "out", 2, 2, 2),
///     test_inputs: || vec![vec![input_bytes(), output_zeros()]],
///     expected_output: || vec![vec![expected_bytes()]],
/// }
/// ```
///
/// `test_inputs` / `expected_output` default to `None` when omitted.
/// Every form expands to exactly one `inventory::submit!` of an
/// `OpEntry`, keeping the registration shape locked to the struct
/// definition above.
///
/// Future `OpEntry` field additions extend this macro with defaulted
/// arms so existing call sites stay green.
#[macro_export]
macro_rules! vyre_op {
    (
        id: $id:expr,
        build: $build:expr $(,)?
    ) => {
        $crate::vyre_op! {
            id: $id,
            build: $build,
            test_inputs: ::core::option::Option::None,
            expected_output: ::core::option::Option::None,
        }
    };
    (
        id: $id:expr,
        build: $build:expr,
        test_inputs: $inputs:expr,
        expected_output: $output:expr $(,)?
    ) => {
        $crate::inventory::submit! {
            $crate::OpEntry {
                id: $id,
                build: $build,
                test_inputs: $inputs,
                expected_output: $output,
                category: None,
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{catalog_primitive_path, OpEntry};

    #[test]
    fn catalog_primitive_path_strips_consumer_a() {
        // Q5: catalog wrapper consumer_a/b ids canonicalize to their
        // wrapped primitive id so tolerance() does not depend on a
        // hand-maintained per-wrapper row.
        assert_eq!(
            catalog_primitive_path("vyre-libs::catalog::math::newton_schulz_poly5_f32::consumer_a"),
            Some("math::newton_schulz_poly5_f32"),
        );
    }

    #[test]
    fn catalog_primitive_path_strips_consumer_b() {
        assert_eq!(
            catalog_primitive_path("vyre-libs::catalog::math::newton_schulz_poly5_f32::consumer_b"),
            Some("math::newton_schulz_poly5_f32"),
        );
    }

    #[test]
    fn catalog_primitive_path_rejects_non_catalog_ids() {
        assert_eq!(catalog_primitive_path("vyre-libs::nn::softmax"), None);
        assert_eq!(
            catalog_primitive_path("vyre-primitives::hash::fnv1a64"),
            None
        );
    }

    #[test]
    fn tolerance_for_id_inherits_from_primitive_through_catalog_wrapper() {
        // The proving test for Q5: catalog consumer_a and consumer_b
        // return the same ULP tolerance as their underlying primitive,
        // without anyone hand-adding a wrapper row.
        let primitive = OpEntry::tolerance_for_id("vyre-primitives::math::newton_schulz_poly5_f32");
        let consumer_a = OpEntry::tolerance_for_id(
            "vyre-libs::catalog::math::newton_schulz_poly5_f32::consumer_a",
        );
        let consumer_b = OpEntry::tolerance_for_id(
            "vyre-libs::catalog::math::newton_schulz_poly5_f32::consumer_b",
        );
        assert_eq!(consumer_a, primitive);
        assert_eq!(consumer_b, primitive);
        assert!(
            primitive > 0,
            "primitive needs a non-zero tolerance for this test to be meaningful"
        );
    }

    #[test]
    fn tolerance_defaults_to_zero_byte_identity() {
        // Adversarial: an unknown op id (not catalog-shaped, no
        // explicit tolerance row) must default to byte identity.
        assert_eq!(
            OpEntry::tolerance_for_id("vyre-libs::foo::bar::baz_unknown_op_id"),
            0
        );
        // Adversarial: a catalog-shaped id whose primitive is unknown
        // must also default to byte identity, not silently leak a
        // tolerance from another op.
        assert_eq!(
            OpEntry::tolerance_for_id(
                "vyre-libs::catalog::imaginary::path_that_does_not_exist::consumer_a"
            ),
            0
        );
    }

    #[test]
    fn muoneq_r_random_stress_tolerance_covers_fma_contraction() {
        assert_eq!(OpEntry::tolerance_for_id("vyre-libs::optim::muoneq_r"), 8);
    }

    #[test]
    fn newton_schulz_random_stress_tolerance_covers_nested_fma_drift() {
        assert_eq!(
            OpEntry::tolerance_for_id("vyre-primitives::math::newton_schulz_poly5_f32"),
            32
        );
        assert_eq!(
            OpEntry::tolerance_for_id("vyre-libs::optim::newton_schulz_5step"),
            64
        );
    }

    // ---------------- vyre_op! macro (S8 generator) ----------------

    fn _trivial_program() -> vyre_foundation::ir::Program {
        use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
        )
    }

    // Minimal-form invocation; expansion succeeds at compile time.
    crate::vyre_op! {
        id: "vyre-harness::test::trivial_minimal",
        build: _trivial_program,
    }

    // Full-form invocation with explicit fixtures.
    crate::vyre_op! {
        id: "vyre-harness::test::trivial_full",
        build: _trivial_program,
        test_inputs: ::core::option::Option::Some(|| vec![vec![vec![0u8; 4]]]),
        expected_output: ::core::option::Option::Some(|| vec![vec![vec![7u8, 0, 0, 0]]]),
    }

    #[test]
    fn vyre_op_macro_minimal_form_registers_entry() {
        let entry = crate::all_entries()
            .find(|e| e.id == "vyre-harness::test::trivial_minimal")
            .expect("Fix: vyre_op! minimal form must register an OpEntry");
        assert!(entry.test_inputs.is_none());
        assert!(entry.expected_output.is_none());
    }

    #[test]
    fn vyre_op_macro_full_form_registers_entry_with_fixtures() {
        let entry = crate::all_entries()
            .find(|e| e.id == "vyre-harness::test::trivial_full")
            .expect("Fix: vyre_op! full form must register an OpEntry");
        assert!(entry.test_inputs.is_some());
        assert!(entry.expected_output.is_some());
    }

    #[test]
    fn vyre_op_macro_build_fn_produces_program() {
        let entry = crate::all_entries()
            .find(|e| e.id == "vyre-harness::test::trivial_minimal")
            .expect("Fix: entry must exist");
        let program = (entry.build)();
        assert_ne!(program.entry().len(), 0);
    }
}

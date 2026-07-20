//! Frozen catalog of engine invariants required for vyre conformance.
//!
//! Each invariant owns a test-family function returning concrete
//! [`TestDescriptor`] entries. Descriptor names use the
//! `conform/tests/<file>.rs::<test_fn>` pattern so community TOML rule packs
//! can extend the same shape without depending on Rust module paths.

use crate::{
    engine_invariant::InvariantId, invariant::Invariant, invariant_category::InvariantCategory,
    test_descriptor::TestDescriptor,
};

/// Empty test-family sentinel for external custom invariant catalogs.
///
/// The built-in invariant catalog never uses this function; every built-in
/// invariant declares concrete happy and adversarial descriptors.
#[must_use]
pub fn empty_test_family() -> &'static [TestDescriptor] {
    &[]
}

const I1_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::deterministic_backend_passes",
        "Happy path: a stable backend produces byte-identical output across repeated runs.",
        InvariantId::I1,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::flaky_backend_run_3_at_wg_256",
        "Adversarial path: a backend that changes output on a later run is rejected.",
        InvariantId::I1,
    ),
];

const I2_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::xor_then_popcount_matches_sequential",
        "Happy path: lowering a composed XOR and popcount chain matches sequential execution.",
        InvariantId::I2,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::sub_then_abs_then_clamp_matches_sequential",
        "Adversarial path: mixed signed arithmetic composition preserves sequential semantics.",
        InvariantId::I2,
    ),
];

const I3_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::reference_backend_certifies_clean",
        "Happy path: the reference backend certifies cleanly against the conformance runner.",
        InvariantId::I3,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::broken_cpu_fn_produces_actionable_violation",
        "Adversarial path: backend/reference disagreement produces an actionable violation.",
        InvariantId::I3,
    ),
];

const I4_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::structural_wire_round_trip_preserves_program",
        "Happy path: valid generated programs survive structural wire-format round trips.",
        InvariantId::I4,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::handcrafted_structural_violation_is_detected",
        "Adversarial path: malformed structural wire input is detected instead of accepted.",
        InvariantId::I4,
    ),
];

const I5_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::reference_round2_1_validation_gate_rejects_invalid_programs",
        "Happy path: validation gates invalid programs before execution or lowering.",
        InvariantId::I5,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::reference_round2_7_huge_workgroup_allocation_is_rejected",
        "Adversarial path: validated programs reject unbounded workgroup allocation requests.",
        InvariantId::I5,
    ),
];

const I6_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::validation_generation_emits_accept_and_reject_cases",
        "Happy path: generated validation suites include both accepting and rejecting cases.",
        InvariantId::I6,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::reference_round2_8_call_arity_rejected_before_allocation",
        "Adversarial path: call-arity validation fires independently before allocation.",
        InvariantId::I6,
    ),
];

const I7_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::xor_then_xor_proof_preserves_commutative_and_associative",
        "Happy path: composition proof preserves commutative and associative laws.",
        InvariantId::I7,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::xor_then_add_does_not_claim_commutativity_without_theorem",
        "Adversarial path: law inference refuses unproven composition claims.",
        InvariantId::I7,
    ),
];

const I8_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::reference_run_bit_matches_cpu_fn_for_registered_specs",
        "Happy path: reference execution matches each registered CPU reference function.",
        InvariantId::I8,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::dual_references_agree_on_all_registered_ops",
        "Adversarial path: dual reference oracles fuzz registered operations for disagreement.",
        InvariantId::I8,
    ),
];

const I9_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::falsifiability_matrix_covers_every_algebraic_law_variant",
        "Happy path: the falsifiability matrix covers every declared algebraic-law variant.",
        InvariantId::I9,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::every_matrix_entry_catches_violators",
        "Adversarial path: every matrix entry catches a concrete law violator.",
        InvariantId::I9,
    ),
];

const I10_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::budget_tracker_rejects_oversized_input",
        "Happy path: budget tracking rejects inputs beyond declared resource limits.",
        InvariantId::I10,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::public_entry_points_survive_fail_on_every_allocation",
        "Adversarial path: public entry points survive allocator failure injection.",
        InvariantId::I10,
    ),
];

const I11_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::correct_backend_passes_all_oob_tests",
        "Happy path: a conforming backend handles out-of-bounds cases without panics.",
        InvariantId::I11,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::unop_negate_i32_min_wraps_without_panic",
        "Adversarial path: signed minimum negation wraps according to spec instead of panicking.",
        InvariantId::I11,
    ),
];

const I12_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::enforces_oob_load_store_and_atomic_contract",
        "Happy path: OOB load, store, and atomic semantics are defined by the interpreter.",
        InvariantId::I12,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::bad_atomic_backend_fails_with_actionable_message",
        "Adversarial path: undefined atomic OOB behavior is caught with an actionable error.",
        InvariantId::I12,
    ),
];

const I13_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::published_specs_have_not_drifted",
        "Happy path: published specs remain stable across compatible versions.",
        InvariantId::I13,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::golden_replay_all",
        "Adversarial path: committed golden cases replay to detect userspace semantic drift.",
        InvariantId::I13,
    ),
];

const I14_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::every_public_enum_in_vyre_is_non_exhaustive",
        "Happy path: public enums keep non-exhaustive compatibility discipline.",
        InvariantId::I14,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::unknown_data_type_byte_returns_error_not_panic",
        "Adversarial path: unknown data-type tags produce structured errors instead of panics.",
        InvariantId::I14,
    ),
];

const I15_TESTS: &[TestDescriptor] = &[
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::certificate_public_getters_are_available",
        "Happy path: certificate data remains inspectable without mutable escape hatches.",
        InvariantId::I15,
    ),
    descriptor(
        "conform/vyre-conform-enforce/tests/invariants.rs::certificate_strength_controls_witnessed_law_detection",
        "Adversarial path: certificate strength changes expose previously missed law violations.",
        InvariantId::I15,
    ),
];

const fn descriptor(
    name: &'static str,
    purpose: &'static str,
    invariant: InvariantId,
) -> TestDescriptor {
    TestDescriptor {
        name,
        purpose,
        invariant,
    }
}

fn i1_test_family() -> &'static [TestDescriptor] {
    I1_TESTS
}

fn i2_test_family() -> &'static [TestDescriptor] {
    I2_TESTS
}

fn i3_test_family() -> &'static [TestDescriptor] {
    I3_TESTS
}

fn i4_test_family() -> &'static [TestDescriptor] {
    I4_TESTS
}

fn i5_test_family() -> &'static [TestDescriptor] {
    I5_TESTS
}

fn i6_test_family() -> &'static [TestDescriptor] {
    I6_TESTS
}

fn i7_test_family() -> &'static [TestDescriptor] {
    I7_TESTS
}

fn i8_test_family() -> &'static [TestDescriptor] {
    I8_TESTS
}

fn i9_test_family() -> &'static [TestDescriptor] {
    I9_TESTS
}

fn i10_test_family() -> &'static [TestDescriptor] {
    I10_TESTS
}

fn i11_test_family() -> &'static [TestDescriptor] {
    I11_TESTS
}

fn i12_test_family() -> &'static [TestDescriptor] {
    I12_TESTS
}

fn i13_test_family() -> &'static [TestDescriptor] {
    I13_TESTS
}

fn i14_test_family() -> &'static [TestDescriptor] {
    I14_TESTS
}

fn i15_test_family() -> &'static [TestDescriptor] {
    I15_TESTS
}

static INVARIANTS: &[Invariant] = &[
    Invariant {
        id: InvariantId::I1,
        name: "Determinism",
        description: "Same ir::Program + same inputs -> byte-identical outputs, every run, every backend, every device. No exceptions.",
        category: InvariantCategory::Execution,
        test_family: i1_test_family,
    },
    Invariant {
        id: InvariantId::I2,
        name: "Composition commutativity with lowering",
        description: "lower(compose(a, b)) is semantically equivalent to lower(a) followed by lower(b). Lowering must not reorder or alter composition semantics.",
        category: InvariantCategory::Execution,
        test_family: i2_test_family,
    },
    Invariant {
        id: InvariantId::I3,
        name: "Backend equivalence",
        description: "For every Program P and every pair of conformant backends (B1, B2), B1.run(P) == B2.run(P). Bit-exact. The reference interpreter is one of the backends.",
        category: InvariantCategory::Execution,
        test_family: i3_test_family,
    },
    Invariant {
        id: InvariantId::I4,
        name: "IR wire-format round-trip",
        description: "from_wire(to_wire(P)) == P for every valid P. The wire format is a lossless binary serialization of ir::Program; it must not alter semantics through the round-trip. vyre has no opcode VM and no interpreter on either side of the codec.",
        category: InvariantCategory::Execution,
        test_family: i4_test_family,
    },
    Invariant {
        id: InvariantId::I5,
        name: "Validation soundness",
        description: "If validate(P) returns empty, lower(P) must not panic, allocate unboundedly, or produce UB. A validated program is a lowerable program.",
        category: InvariantCategory::Execution,
        test_family: i5_test_family,
    },
    Invariant {
        id: InvariantId::I6,
        name: "Validation completeness (partial)",
        description: "For every declared V-rule, there exists a test input that triggers exactly that rule and no others. Rules must be independently triggerable so the validator's coverage is provably non-overlapping.",
        category: InvariantCategory::Execution,
        test_family: i6_test_family,
    },
    Invariant {
        id: InvariantId::I7,
        name: "Law monotonicity under composition",
        description: "If op A declares law L and op B is defined as compose(A, ...) where the composition preserves L per composition.rs theorems, then B automatically has law L. The inference engine must never lose a law that the theorems prove.",
        category: InvariantCategory::Algebra,
        test_family: i7_test_family,
    },
    Invariant {
        id: InvariantId::I8,
        name: "Reference agreement",
        description: "The reference interpreter result equals the CPU reference_fn result for every op, every input. Zero tolerance. Disagreement is a Phase 2 blocker because one of them is wrong.",
        category: InvariantCategory::Algebra,
        test_family: i8_test_family,
    },
    Invariant {
        id: InvariantId::I9,
        name: "Law falsifiability",
        description: "For every declared law on every op, removing that law from the declaration must cause at least one test to fail. No decorative laws; every declaration earns its keep.",
        category: InvariantCategory::Algebra,
        test_family: i9_test_family,
    },
    Invariant {
        id: InvariantId::I10,
        name: "Bounded allocation",
        description: "No operation allocates more than program.buffers.total_size + program.workgroup_mem + O(nodes). Every allocation has a pre-computable bound.",
        category: InvariantCategory::Resource,
        test_family: i10_test_family,
    },
    Invariant {
        id: InvariantId::I11,
        name: "No panic",
        description: "No lowered program can panic the runtime regardless of input data. Div by zero returns 0, OOB load returns 0, OOB store is a no-op. Panicking on user input is a backend bug.",
        category: InvariantCategory::Resource,
        test_family: i11_test_family,
    },
    Invariant {
        id: InvariantId::I12,
        name: "No undefined behaviour",
        description: "No lowered shader can produce undefined behaviour on any conformant backend. All observable semantics are defined by the spec; none are left to backend discretion.",
        category: InvariantCategory::Resource,
        test_family: i12_test_family,
    },
    Invariant {
        id: InvariantId::I13,
        name: "Userspace stability",
        description: "A Program valid under vyre v1.x is valid and produces identical results under every v1.y where y >= x. The Linux userspace compatibility rule.",
        category: InvariantCategory::Stability,
        test_family: i13_test_family,
    },
    Invariant {
        id: InvariantId::I14,
        name: "Non-exhaustive discipline",
        description: "Adding a new DataType, BinOp, or Expr variant never breaks existing Programs. Wire-facing public enums stay backward-compatible so downstream handles unknown variants gracefully.",
        category: InvariantCategory::Stability,
        test_family: i14_test_family,
    },
    Invariant {
        id: InvariantId::I15,
        name: "Certificate stability",
        description: "A conformance certificate issued at v1.x remains valid at v1.y if the backend has not changed. A certificate is durable; version bumps do not retroactively invalidate past proofs.",
        category: InvariantCategory::Stability,
        test_family: i15_test_family,
    },
];

/// The full invariant catalog. Order matches I1..I15.
#[must_use]
pub fn invariants() -> &'static [Invariant] {
    INVARIANTS
}

#![allow(missing_docs)]

extern crate self as vyre;

mod support;

pub use support::{dialect, ir, ops, optimizer};

use vyre_macros::{define_op, vyre_pass, AlgebraicLaws};

#[vyre_pass(
    name = "release_surface_pass",
    requires = ["domtree"],
    invalidates = ["alias"],
    phase = "megakernel",
    boundary_class = "backend_aware",
    requires_caps = ["cuda", "resident_dispatch"],
    preserves_abi = false,
    cost_model_family = "dataflow"
)]
pub struct ReleaseSurfacePass;

impl ReleaseSurfacePass {
    fn analyze_impl(program: &ir::Program) -> optimizer::PassAnalysis {
        if program.id == 0 {
            optimizer::PassAnalysis::SKIP
        } else {
            optimizer::PassAnalysis::RUN
        }
    }

    fn transform(program: ir::Program) -> optimizer::PassResult {
        optimizer::pass_result(program, true)
    }
}

#[derive(AlgebraicLaws)]
#[vyre(laws = [Commutative, "Identity { element: 0 }"])]
pub struct XorLikeOp;

define_op! {
    id = "release.surface.xor_like",
    dialect = "release.surface",
    category = A,
    inputs = ["u32", "u32"],
    outputs = ["u32"],
    laws = [Commutative, Identity { element: 0 }],
    program = crate::ir::Program { id: 7 },
}

#[vyre_pass(name = "release_surface_defaults", requires = [], invalidates = [])]
pub struct ReleaseSurfaceDefaultsPass;

impl ReleaseSurfaceDefaultsPass {
    fn analyze_impl(_program: &ir::Program) -> optimizer::PassAnalysis {
        optimizer::PassAnalysis::RUN
    }

    fn transform(program: ir::Program) -> optimizer::PassResult {
        optimizer::unchanged(program)
    }
}

#[vyre_pass(
    name = "release_surface_analyze_always",
    requires = [],
    invalidates = [],
    analyze = "always"
)]
pub struct ReleaseSurfaceAnalyzeAlwaysPass;

impl ReleaseSurfaceAnalyzeAlwaysPass {
    fn transform(program: ir::Program) -> optimizer::PassResult {
        optimizer::unchanged(program)
    }
}

macro_rules! define_metadata_matrix_pass {
    (
        $type_name:ident,
        $pass_name:literal,
        $phase:literal,
        $boundary_class:literal,
        $cost_model_family:literal
    ) => {
        #[vyre_pass(
                                                            name = $pass_name,
                                                            requires = [],
                                                            invalidates = [],
                                                            phase = $phase,
                                                            boundary_class = $boundary_class,
                                                            cost_model_family = $cost_model_family
                                                        )]
        pub struct $type_name;

        impl $type_name {
            fn analyze_impl(_program: &ir::Program) -> optimizer::PassAnalysis {
                optimizer::PassAnalysis::RUN
            }

            fn transform(program: ir::Program) -> optimizer::PassResult {
                optimizer::unchanged(program)
            }
        }
    };
}

define_metadata_matrix_pass!(
    MatrixCanonicalizationPass,
    "matrix_canonicalization",
    "canonicalization",
    "abi_preserving",
    "scalar"
);
define_metadata_matrix_pass!(
    MatrixScalarAlgebraPass,
    "matrix_scalar_algebra",
    "scalar_algebra",
    "abi_changing",
    "loop"
);
define_metadata_matrix_pass!(
    MatrixLoopPass,
    "matrix_loop",
    "loop",
    "runtime_aware",
    "memory"
);
define_metadata_matrix_pass!(
    MatrixMemoryPass,
    "matrix_memory",
    "memory",
    "domain_specific",
    "fusion"
);
define_metadata_matrix_pass!(
    MatrixFusionPass,
    "matrix_fusion",
    "fusion_cse",
    "abi_preserving",
    "sync"
);
define_metadata_matrix_pass!(
    MatrixSyncPass,
    "matrix_sync",
    "sync",
    "abi_changing",
    "megakernel"
);
define_metadata_matrix_pass!(
    MatrixSpecializationPass,
    "matrix_specialization",
    "specialization",
    "runtime_aware",
    "scalar"
);
define_metadata_matrix_pass!(
    MatrixCleanupPass,
    "matrix_cleanup",
    "cleanup",
    "domain_specific",
    "loop"
);
define_metadata_matrix_pass!(
    MatrixDataflowPass,
    "matrix_dataflow",
    "dataflow",
    "backend_aware",
    "dataflow"
);

#[test]
fn vyre_pass_emits_metadata_trait_impl_and_inventory_registration() {
    let pass = ReleaseSurfacePass;
    let metadata = optimizer::ProgramPass::metadata(&pass);

    assert_eq!(metadata.name, "release_surface_pass");
    assert_eq!(metadata.requires, &["domtree"]);
    assert_eq!(metadata.invalidates, &["alias"]);
    assert_eq!(metadata.phase, optimizer::PassPhase::Megakernel);
    assert_eq!(
        metadata.boundary_class,
        optimizer::PassBoundaryClass::BackendAware
    );
    assert_eq!(metadata.requires_caps, &["cuda", "resident_dispatch"]);
    assert!(!metadata.preserves_abi);
    assert_eq!(
        metadata.cost_model_family,
        optimizer::CostModelFamily::Dataflow
    );

    assert_eq!(
        optimizer::ProgramPass::analyze(&pass, &ir::Program { id: 0 }),
        optimizer::PassAnalysis::SKIP
    );
    assert_eq!(
        optimizer::ProgramPass::analyze(&pass, &ir::Program { id: 1 }),
        optimizer::PassAnalysis::RUN
    );
    assert_eq!(
        optimizer::ProgramPass::fingerprint(&pass, &ir::Program { id: 5 }),
        optimizer::fingerprint_program(&ir::Program { id: 5 })
    );

    let registered = inventory::iter::<optimizer::ProgramPassRegistration>
        .into_iter()
        .any(|registration| {
            registration.metadata.name == "release_surface_pass"
                && (registration.factory)().metadata().requires_caps
                    == &["cuda", "resident_dispatch"]
        });
    assert!(registered);
}

#[test]
fn vyre_pass_default_metadata_is_a_stable_contract() {
    let pass = ReleaseSurfaceDefaultsPass;
    let metadata = optimizer::ProgramPass::metadata(&pass);

    assert_eq!(metadata.name, "release_surface_defaults");
    assert_eq!(metadata.requires, &[] as &[&str]);
    assert_eq!(metadata.invalidates, &[] as &[&str]);
    assert_eq!(metadata.phase, optimizer::PassPhase::Unclassified);
    assert_eq!(
        metadata.boundary_class,
        optimizer::PassBoundaryClass::Unknown
    );
    assert_eq!(metadata.requires_caps, &[] as &[&str]);
    assert!(metadata.preserves_abi);
    assert_eq!(
        metadata.cost_model_family,
        optimizer::CostModelFamily::Unknown
    );

    let program = ir::Program { id: 11 };
    assert_eq!(
        optimizer::ProgramPass::analyze(&pass, &program),
        optimizer::PassAnalysis::RUN
    );
    assert_eq!(
        optimizer::ProgramPass::transform(&pass, program.clone()),
        optimizer::unchanged(program)
    );
}

#[test]
fn vyre_pass_analyze_always_does_not_require_inherent_analyze_impl() {
    let pass = ReleaseSurfaceAnalyzeAlwaysPass;
    let metadata = optimizer::ProgramPass::metadata(&pass);

    assert_eq!(metadata.name, "release_surface_analyze_always");
    assert_eq!(
        optimizer::ProgramPass::analyze(&pass, &ir::Program { id: 0 }),
        optimizer::PassAnalysis::RUN
    );
    assert_eq!(
        optimizer::ProgramPass::analyze(&pass, &ir::Program { id: u64::MAX }),
        optimizer::PassAnalysis::RUN
    );
}

#[test]
fn vyre_pass_metadata_matrix_covers_every_non_default_phase_mapping() {
    let cases = [
        (
            optimizer::ProgramPass::metadata(&MatrixCanonicalizationPass),
            optimizer::PassPhase::Canonicalization,
            optimizer::PassBoundaryClass::AbiPreserving,
            optimizer::CostModelFamily::Scalar,
        ),
        (
            optimizer::ProgramPass::metadata(&MatrixScalarAlgebraPass),
            optimizer::PassPhase::ScalarAlgebra,
            optimizer::PassBoundaryClass::AbiChanging,
            optimizer::CostModelFamily::Loop,
        ),
        (
            optimizer::ProgramPass::metadata(&MatrixLoopPass),
            optimizer::PassPhase::Loop,
            optimizer::PassBoundaryClass::RuntimeAware,
            optimizer::CostModelFamily::Memory,
        ),
        (
            optimizer::ProgramPass::metadata(&MatrixMemoryPass),
            optimizer::PassPhase::Memory,
            optimizer::PassBoundaryClass::DomainSpecific,
            optimizer::CostModelFamily::Fusion,
        ),
        (
            optimizer::ProgramPass::metadata(&MatrixFusionPass),
            optimizer::PassPhase::FusionCse,
            optimizer::PassBoundaryClass::AbiPreserving,
            optimizer::CostModelFamily::Sync,
        ),
        (
            optimizer::ProgramPass::metadata(&MatrixSyncPass),
            optimizer::PassPhase::Sync,
            optimizer::PassBoundaryClass::AbiChanging,
            optimizer::CostModelFamily::Megakernel,
        ),
        (
            optimizer::ProgramPass::metadata(&MatrixSpecializationPass),
            optimizer::PassPhase::Specialization,
            optimizer::PassBoundaryClass::RuntimeAware,
            optimizer::CostModelFamily::Scalar,
        ),
        (
            optimizer::ProgramPass::metadata(&MatrixCleanupPass),
            optimizer::PassPhase::Cleanup,
            optimizer::PassBoundaryClass::DomainSpecific,
            optimizer::CostModelFamily::Loop,
        ),
        (
            optimizer::ProgramPass::metadata(&MatrixDataflowPass),
            optimizer::PassPhase::Dataflow,
            optimizer::PassBoundaryClass::BackendAware,
            optimizer::CostModelFamily::Dataflow,
        ),
    ];

    for (metadata, expected_phase, expected_boundary, expected_cost) in cases {
        assert_eq!(metadata.phase, expected_phase);
        assert_eq!(metadata.boundary_class, expected_boundary);
        assert_eq!(metadata.cost_model_family, expected_cost);
        assert_eq!(metadata.requires, &[] as &[&str]);
        assert_eq!(metadata.invalidates, &[] as &[&str]);
        assert!(metadata.preserves_abi);
    }
}

#[test]
fn algebraic_laws_and_define_op_emit_runtime_discoverable_contracts() {
    assert_eq!(
        <XorLikeOp as ops::AlgebraicLawProvider>::laws(),
        &[
            ops::AlgebraicLaw::Commutative,
            ops::AlgebraicLaw::Identity { element: 0 }
        ]
    );

    let op = inventory::iter::<dialect::OpDefRegistration>
        .into_iter()
        .map(|registration| (registration.factory)())
        .find(|op| op.id == "release.surface.xor_like")
        .expect("define_op! must submit release-surface op registration");

    assert_eq!(op.dialect, "release.surface");
    assert_eq!(op.category, dialect::Category::A);
    assert_eq!(op.signature.inputs.len(), 2);
    assert_eq!(op.signature.outputs.len(), 1);
    assert_eq!(op.signature.inputs[0].ty, "u32");
    assert_eq!(op.signature.outputs[0].ty, "u32");
    assert_eq!(
        op.laws,
        &[
            ops::AlgebraicLaw::Commutative,
            ops::AlgebraicLaw::Identity { element: 0 }
        ]
    );
    assert_eq!(op.compose.expect("compose function must exist")().id, 7);
}

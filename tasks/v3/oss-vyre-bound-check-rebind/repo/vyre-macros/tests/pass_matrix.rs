#![allow(missing_docs)]

extern crate self as vyre;

mod support;

pub use support::{ir, optimizer};

use vyre_macros::vyre_pass;

macro_rules! define_phase_pass {
    ($ty:ident, $name:literal, $phase:literal) => {
        #[vyre_pass(name = $name, requires = [], invalidates = [], phase = $phase, analyze = "always")]
        pub struct $ty;

        impl $ty {
            fn transform(program: crate::ir::Program) -> crate::optimizer::PassResult {
                crate::optimizer::unchanged(program)
            }
        }
    };
}

macro_rules! define_boundary_pass {
    ($ty:ident, $name:literal, $boundary:literal) => {
        #[vyre_pass(name = $name, requires = [], invalidates = [], boundary_class = $boundary, analyze = "always")]
        pub struct $ty;

        impl $ty {
            fn transform(program: crate::ir::Program) -> crate::optimizer::PassResult {
                crate::optimizer::unchanged(program)
            }
        }
    };
}

macro_rules! define_cost_pass {
    ($ty:ident, $name:literal, $cost:literal) => {
        #[vyre_pass(name = $name, requires = [], invalidates = [], cost_model_family = $cost, analyze = "always")]
        pub struct $ty;

        impl $ty {
            fn transform(program: crate::ir::Program) -> crate::optimizer::PassResult {
                crate::optimizer::unchanged(program)
            }
        }
    };
}

define_phase_pass!(PhaseUnclassified, "phase.unclassified", "unclassified");
define_phase_pass!(
    PhaseCanonicalization,
    "phase.canonicalization",
    "canonicalization"
);
define_phase_pass!(PhaseScalarAlgebra, "phase.scalar_algebra", "scalar_algebra");
define_phase_pass!(PhaseLoop, "phase.loop", "loop");
define_phase_pass!(PhaseMemory, "phase.memory", "memory");
define_phase_pass!(PhaseFusionCse, "phase.fusion_cse", "fusion_cse");
define_phase_pass!(PhaseSync, "phase.sync", "sync");
define_phase_pass!(
    PhaseSpecialization,
    "phase.specialization",
    "specialization"
);
define_phase_pass!(PhaseCleanup, "phase.cleanup", "cleanup");
define_phase_pass!(PhaseDataflow, "phase.dataflow", "dataflow");
define_phase_pass!(PhaseMegakernel, "phase.megakernel", "megakernel");

define_boundary_pass!(BoundaryUnknown, "boundary.unknown", "unknown");
define_boundary_pass!(
    BoundaryAbiPreserving,
    "boundary.abi_preserving",
    "abi_preserving"
);
define_boundary_pass!(BoundaryAbiChanging, "boundary.abi_changing", "abi_changing");
define_boundary_pass!(
    BoundaryBackendAware,
    "boundary.backend_aware",
    "backend_aware"
);
define_boundary_pass!(
    BoundaryRuntimeAware,
    "boundary.runtime_aware",
    "runtime_aware"
);
define_boundary_pass!(
    BoundaryDomainSpecific,
    "boundary.domain_specific",
    "domain_specific"
);

define_cost_pass!(CostUnknown, "cost.unknown", "unknown");
define_cost_pass!(CostScalar, "cost.scalar", "scalar");
define_cost_pass!(CostLoop, "cost.loop", "loop");
define_cost_pass!(CostMemory, "cost.memory", "memory");
define_cost_pass!(CostFusion, "cost.fusion", "fusion");
define_cost_pass!(CostSync, "cost.sync", "sync");
define_cost_pass!(CostDataflow, "cost.dataflow", "dataflow");
define_cost_pass!(CostMegakernel, "cost.megakernel", "megakernel");

#[test]
fn vyre_pass_phase_matrix_emits_expected_metadata() {
    use optimizer::{PassPhase, ProgramPass};
    let cases: &[(&dyn ProgramPass, PassPhase)] = &[
        (&PhaseUnclassified, PassPhase::Unclassified),
        (&PhaseCanonicalization, PassPhase::Canonicalization),
        (&PhaseScalarAlgebra, PassPhase::ScalarAlgebra),
        (&PhaseLoop, PassPhase::Loop),
        (&PhaseMemory, PassPhase::Memory),
        (&PhaseFusionCse, PassPhase::FusionCse),
        (&PhaseSync, PassPhase::Sync),
        (&PhaseSpecialization, PassPhase::Specialization),
        (&PhaseCleanup, PassPhase::Cleanup),
        (&PhaseDataflow, PassPhase::Dataflow),
        (&PhaseMegakernel, PassPhase::Megakernel),
    ];
    for (pass, phase) in cases {
        let metadata = pass.metadata();
        assert_eq!(metadata.phase, *phase, "{}", metadata.name);
        assert_eq!(
            pass.analyze(&ir::Program { id: 0 }),
            optimizer::PassAnalysis::RUN
        );
    }
}

#[test]
fn vyre_pass_boundary_matrix_emits_expected_metadata() {
    use optimizer::{PassBoundaryClass, ProgramPass};
    let cases: &[(&dyn ProgramPass, PassBoundaryClass)] = &[
        (&BoundaryUnknown, PassBoundaryClass::Unknown),
        (&BoundaryAbiPreserving, PassBoundaryClass::AbiPreserving),
        (&BoundaryAbiChanging, PassBoundaryClass::AbiChanging),
        (&BoundaryBackendAware, PassBoundaryClass::BackendAware),
        (&BoundaryRuntimeAware, PassBoundaryClass::RuntimeAware),
        (&BoundaryDomainSpecific, PassBoundaryClass::DomainSpecific),
    ];
    for (pass, boundary_class) in cases {
        let metadata = pass.metadata();
        assert_eq!(
            metadata.boundary_class, *boundary_class,
            "{}",
            metadata.name
        );
    }
}

#[test]
fn vyre_pass_cost_model_matrix_emits_expected_metadata() {
    use optimizer::{CostModelFamily, ProgramPass};
    let cases: &[(&dyn ProgramPass, CostModelFamily)] = &[
        (&CostUnknown, CostModelFamily::Unknown),
        (&CostScalar, CostModelFamily::Scalar),
        (&CostLoop, CostModelFamily::Loop),
        (&CostMemory, CostModelFamily::Memory),
        (&CostFusion, CostModelFamily::Fusion),
        (&CostSync, CostModelFamily::Sync),
        (&CostDataflow, CostModelFamily::Dataflow),
        (&CostMegakernel, CostModelFamily::Megakernel),
    ];
    for (pass, cost_model_family) in cases {
        let metadata = pass.metadata();
        assert_eq!(
            metadata.cost_model_family, *cost_model_family,
            "{}",
            metadata.name
        );
    }
}

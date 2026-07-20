#![allow(missing_docs)]

extern crate self as vyre;

mod support;

pub use support::{dialect, ir, ops, optimizer};

use vyre_macros::{define_op, vyre_pass, AlgebraicLaws};

#[vyre_pass(
    name = "generated.matrix.selective",
    requires = ["domtree", "alias", "cfg"],
    invalidates = ["cost", "schedule"],
    phase = "dataflow",
    boundary_class = "backend_aware",
    requires_caps = ["cuda", "resident_dispatch", "graph_capture"],
    preserves_abi = false,
    cost_model_family = "megakernel"
)]
pub struct GeneratedSelectivePass;

impl GeneratedSelectivePass {
    fn analyze_impl(program: &ir::Program) -> optimizer::PassAnalysis {
        if program.id.count_ones() % 2 == 0 {
            optimizer::PassAnalysis::RUN
        } else {
            optimizer::PassAnalysis::SKIP
        }
    }

    fn transform(program: ir::Program) -> optimizer::PassResult {
        let changed = program.id.count_ones() % 2 == 0;
        optimizer::pass_result(program, changed)
    }
}

#[vyre_pass(
    name = "generated.matrix.always",
    requires = [],
    invalidates = [],
    phase = "cleanup",
    boundary_class = "abi_preserving",
    cost_model_family = "memory",
    analyze = "always"
)]
pub struct GeneratedAlwaysPass;

impl GeneratedAlwaysPass {
    fn transform(program: ir::Program) -> optimizer::PassResult {
        optimizer::unchanged(program)
    }
}

#[vyre_pass(
    name = "generated.matrix.dense_metadata",
    requires = [
        "parse",
        "dominance",
        "alias",
        "range",
        "layout",
        "residency",
        "occupancy",
        "fusion"
    ],
    invalidates = [
        "value_numbering",
        "schedule",
        "buffer_liveness",
        "resident_cache",
        "launch_shape"
    ],
    phase = "megakernel",
    boundary_class = "runtime_aware",
    requires_caps = [
        "cuda",
        "resident_buffers",
        "cuda_graph",
        "cooperative_launch",
        "async_copy",
        "timestamp_query"
    ],
    preserves_abi = false,
    cost_model_family = "dataflow",
    analyze = "always"
)]
pub struct GeneratedDenseMetadataPass;

impl GeneratedDenseMetadataPass {
    fn transform(program: ir::Program) -> optimizer::PassResult {
        optimizer::unchanged(program)
    }
}

#[derive(AlgebraicLaws)]
#[vyre(laws = [Commutative, Associative, "Identity { element: 0 }"])]
pub struct GeneratedLawProvider;

define_op! {
    id = "generated.matrix.xor_reduce",
    dialect = "generated.matrix",
    category = B,
    inputs = ["u32", "u32", "u32"],
    outputs = ["u32"],
    laws = [Commutative, Associative, Identity { element: 0 }],
    program = crate::ir::Program { id: 0xC0DE },
}

struct PassCase {
    pass: &'static dyn optimizer::ProgramPass,
    name: &'static str,
    requires: &'static [&'static str],
    invalidates: &'static [&'static str],
    phase: optimizer::PassPhase,
    boundary_class: optimizer::PassBoundaryClass,
    requires_caps: &'static [&'static str],
    preserves_abi: bool,
    cost_model_family: optimizer::CostModelFamily,
}

static SELECTIVE: GeneratedSelectivePass = GeneratedSelectivePass;
static ALWAYS: GeneratedAlwaysPass = GeneratedAlwaysPass;
static DENSE: GeneratedDenseMetadataPass = GeneratedDenseMetadataPass;

#[test]
fn generated_vyre_pass_matrix_checks_thousands_of_metadata_and_behavior_cases() {
    let cases = [
        PassCase {
            pass: &SELECTIVE,
            name: "generated.matrix.selective",
            requires: &["domtree", "alias", "cfg"],
            invalidates: &["cost", "schedule"],
            phase: optimizer::PassPhase::Dataflow,
            boundary_class: optimizer::PassBoundaryClass::BackendAware,
            requires_caps: &["cuda", "resident_dispatch", "graph_capture"],
            preserves_abi: false,
            cost_model_family: optimizer::CostModelFamily::Megakernel,
        },
        PassCase {
            pass: &ALWAYS,
            name: "generated.matrix.always",
            requires: &[],
            invalidates: &[],
            phase: optimizer::PassPhase::Cleanup,
            boundary_class: optimizer::PassBoundaryClass::AbiPreserving,
            requires_caps: &[],
            preserves_abi: true,
            cost_model_family: optimizer::CostModelFamily::Memory,
        },
        PassCase {
            pass: &DENSE,
            name: "generated.matrix.dense_metadata",
            requires: &[
                "parse",
                "dominance",
                "alias",
                "range",
                "layout",
                "residency",
                "occupancy",
                "fusion",
            ],
            invalidates: &[
                "value_numbering",
                "schedule",
                "buffer_liveness",
                "resident_cache",
                "launch_shape",
            ],
            phase: optimizer::PassPhase::Megakernel,
            boundary_class: optimizer::PassBoundaryClass::RuntimeAware,
            requires_caps: &[
                "cuda",
                "resident_buffers",
                "cuda_graph",
                "cooperative_launch",
                "async_copy",
                "timestamp_query",
            ],
            preserves_abi: false,
            cost_model_family: optimizer::CostModelFamily::Dataflow,
        },
    ];

    let mut assertions = 0usize;
    for seed in 0u64..4096 {
        let id = seed
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .rotate_left((seed % 63) as u32);
        let program = ir::Program { id };

        for case in &cases {
            let metadata = case.pass.metadata();
            assert_eq!(metadata.name, case.name);
            assert_eq!(metadata.requires, case.requires);
            assert_eq!(metadata.invalidates, case.invalidates);
            assert_eq!(metadata.phase, case.phase);
            assert_eq!(metadata.boundary_class, case.boundary_class);
            assert_eq!(metadata.requires_caps, case.requires_caps);
            assert_eq!(metadata.preserves_abi, case.preserves_abi);
            assert_eq!(metadata.cost_model_family, case.cost_model_family);
            assert_eq!(
                case.pass.fingerprint(&program),
                optimizer::fingerprint_program(&program)
            );

            let analysis = case.pass.analyze(&program);
            let result = case.pass.transform(program.clone());
            assert_eq!(result.program, program);
            if case.name == "generated.matrix.selective" {
                assert_eq!(analysis.should_run, program.id.count_ones() % 2 == 0);
                assert_eq!(result.changed, analysis.should_run);
            } else {
                assert!(analysis.should_run);
                assert!(!result.changed);
            }
            assertions += 16;
        }
    }
    assert_eq!(assertions, 4096 * cases.len() * 16);
}

#[test]
fn generated_define_op_and_law_provider_matrix_remains_runtime_discoverable() {
    assert_eq!(
        <GeneratedLawProvider as ops::AlgebraicLawProvider>::laws(),
        &[
            ops::AlgebraicLaw::Commutative,
            ops::AlgebraicLaw::Associative,
            ops::AlgebraicLaw::Identity { element: 0 }
        ]
    );

    let op = inventory::iter::<dialect::OpDefRegistration>
        .into_iter()
        .map(|registration| (registration.factory)())
        .find(|op| op.id == "generated.matrix.xor_reduce")
        .expect("define_op! must publish generated matrix op registration");

    assert_eq!(op.dialect, "generated.matrix");
    assert_eq!(op.category, dialect::Category::B);
    assert_eq!(op.signature.inputs.len(), 3);
    assert_eq!(op.signature.outputs.len(), 1);
    assert!(op.signature.inputs.iter().all(|param| param.ty == "u32"));
    assert_eq!(
        op.laws,
        &[
            ops::AlgebraicLaw::Commutative,
            ops::AlgebraicLaw::Associative,
            ops::AlgebraicLaw::Identity { element: 0 }
        ]
    );
    assert_eq!(
        op.compose.expect("compose function must exist")().id,
        0xC0DE
    );
}

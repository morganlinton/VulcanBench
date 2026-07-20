#![allow(missing_docs)]

extern crate self as vyre;

mod support;

pub use support::{dialect, ir, ir_inner, ops, optimizer};

use vyre_macros::{define_op, skip_builder, vyre_ast_registry, vyre_pass, AlgebraicLaws};

#[vyre_pass(
    name = "macro_compile_backed_pass",
    requires = ["domtree", "alias"],
    invalidates = ["cfg"],
    phase = "dataflow",
    boundary_class = "abi_preserving",
    requires_caps = ["cuda"],
    preserves_abi = false,
    cost_model_family = "megakernel"
)]
pub struct CompileBackedPass;

impl CompileBackedPass {
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

#[vyre_pass(name = "macro_analyze_always", requires = [], invalidates = [], analyze = "always")]
pub struct AnalyzeAlwaysPass;

impl AnalyzeAlwaysPass {
    fn transform(program: ir::Program) -> optimizer::PassResult {
        optimizer::unchanged(program)
    }
}

#[vyre_pass(name = "macro_defaulted_pass", requires = [], invalidates = [])]
pub struct DefaultedPass;

impl DefaultedPass {
    fn analyze_impl(_program: &ir::Program) -> optimizer::PassAnalysis {
        optimizer::PassAnalysis::RUN
    }

    fn transform(program: ir::Program) -> optimizer::PassResult {
        optimizer::unchanged(program)
    }
}

#[derive(AlgebraicLaws)]
#[vyre(laws = [Commutative, Associative, "Identity { element: 0 }"])]
pub struct XorLawProvider;

#[derive(AlgebraicLaws)]
#[vyre(laws = [Associative])]
pub enum AssociativeEnumLawProvider {
    Variant,
}

define_op! {
    id = "primitive.bitwise.xor",
    dialect = "primitive.bitwise",
    category = A,
    inputs = ["u32", "u32"],
    outputs = ["u32"],
    laws = [Commutative, Associative],
    program = ir::Program { id: 42 },
}

vyre_ast_registry! {
    TestExpr {
        Unit,
        Unary(u32),
        Pair { left: u32, right: u32 },
    }
}

vyre_ast_registry! {}

#[skip_builder]
pub struct SkipBuilderTarget {
    value: u32,
}

#[test]
fn vyre_pass_expands_to_metadata_analysis_transform_and_inventory_entry() {
    let pass = CompileBackedPass;
    let metadata = optimizer::ProgramPass::metadata(&pass);

    assert_eq!(metadata.name, "macro_compile_backed_pass");
    assert_eq!(metadata.requires, &["domtree", "alias"]);
    assert_eq!(metadata.invalidates, &["cfg"]);
    assert_eq!(metadata.phase, optimizer::PassPhase::Dataflow);
    assert_eq!(
        metadata.boundary_class,
        optimizer::PassBoundaryClass::AbiPreserving
    );
    assert_eq!(metadata.requires_caps, &["cuda"]);
    assert!(!metadata.preserves_abi);
    assert_eq!(
        metadata.cost_model_family,
        optimizer::CostModelFamily::Megakernel
    );

    assert!(!optimizer::ProgramPass::analyze(&pass, &ir::Program { id: 0 }).should_run);
    assert!(optimizer::ProgramPass::analyze(&pass, &ir::Program { id: 7 }).should_run);
    assert!(optimizer::ProgramPass::transform(&pass, ir::Program { id: 7 }).changed);
    assert_eq!(
        optimizer::ProgramPass::fingerprint(&pass, &ir::Program { id: 7 }),
        optimizer::fingerprint_program(&ir::Program { id: 7 })
    );

    let registered = inventory::iter::<optimizer::ProgramPassRegistration>
        .into_iter()
        .any(|registration| registration.metadata.name == "macro_compile_backed_pass");
    assert!(registered);

    let factory_metadata = inventory::iter::<optimizer::ProgramPassRegistration>
        .into_iter()
        .find(|registration| registration.metadata.name == "macro_compile_backed_pass")
        .map(|registration| (registration.factory)().metadata())
        .expect("registered pass factory should instantiate macro_compile_backed_pass");
    assert_eq!(factory_metadata.name, "macro_compile_backed_pass");
}

#[test]
fn vyre_pass_analyze_always_skips_missing_analyze_impl_requirement() {
    let pass = AnalyzeAlwaysPass;
    assert!(optimizer::ProgramPass::analyze(&pass, &ir::Program { id: 0 }).should_run);
}

#[test]
fn vyre_pass_defaults_are_abi_preserving_unknown_metadata() {
    let pass = DefaultedPass;
    let metadata = optimizer::ProgramPass::metadata(&pass);

    assert_eq!(metadata.name, "macro_defaulted_pass");
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
}

#[test]
fn algebraic_laws_derive_pins_declared_laws() {
    assert_eq!(
        <XorLawProvider as ops::AlgebraicLawProvider>::laws(),
        &[
            ops::AlgebraicLaw::Commutative,
            ops::AlgebraicLaw::Associative,
            ops::AlgebraicLaw::Identity { element: 0 },
        ]
    );
    assert_eq!(
        <AssociativeEnumLawProvider as ops::AlgebraicLawProvider>::laws(),
        &[ops::AlgebraicLaw::Associative]
    );
}

#[test]
fn define_op_registers_dialect_signature_laws_and_program_factory() {
    let op = inventory::iter::<dialect::OpDefRegistration>
        .into_iter()
        .map(|registration| (registration.factory)())
        .find(|op| op.id == "primitive.bitwise.xor")
        .expect("define_op! should submit an op registration");

    assert_eq!(op.dialect, "primitive.bitwise");
    assert_eq!(op.category, dialect::Category::A);
    assert_eq!(op.signature.inputs.len(), 2);
    assert_eq!(op.signature.outputs.len(), 1);
    assert_eq!(
        op.laws,
        &[
            ops::AlgebraicLaw::Commutative,
            ops::AlgebraicLaw::Associative
        ]
    );
    assert_eq!((op.compose.expect("compose factory should exist"))().id, 42);
}

#[test]
fn ast_registry_generates_enum_equality_op_ids_and_decoder_scaffold() {
    assert_eq!(testexpr_op_id(&TestExpr::Unit), "vyre.testexpr.unit");
    assert_eq!(testexpr_op_id(&TestExpr::Unary(5)), "vyre.testexpr.unary");
    assert_eq!(
        testexpr_op_id(&TestExpr::Pair { left: 1, right: 2 }),
        "vyre.testexpr.pair"
    );

    assert_eq!(TestExpr::Unary(5), TestExpr::Unary(5));
    assert_ne!(TestExpr::Unary(5), TestExpr::Unary(6));
    assert_eq!(
        TestExpr::Pair { left: 1, right: 2 },
        TestExpr::Pair { left: 1, right: 2 }
    );
    assert_ne!(
        TestExpr::Pair { left: 1, right: 2 },
        TestExpr::Pair { left: 2, right: 1 }
    );

    match generate_testexpr_gpu_vm_decoder() {
        ir_inner::model::node::Node::If { .. } => {}
        other => panic!("expected generated decoder cascade, got {other:?}"),
    }
}

#[test]
fn ast_registry_accepts_empty_manifest_as_noop() {
    let pass = DefaultedPass;
    assert_eq!(
        optimizer::ProgramPass::metadata(&pass).name,
        "macro_defaulted_pass"
    );
}

#[test]
fn skip_builder_attribute_preserves_the_input_item() {
    let target = SkipBuilderTarget { value: 9 };
    assert_eq!(target.value, 9);
}

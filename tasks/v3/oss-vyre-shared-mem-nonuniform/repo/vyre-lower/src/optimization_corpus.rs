//! Generated optimization corpus for release-scale rewrite coverage.
//!
//! The release bar is thousands of optimization cases, not a small
//! hand-maintained smoke set. This module generates deterministic
//! descriptor-level cases across rewrite families, operand shapes,
//! constants, memory classes, dispatch shapes, and op variants.

use serde::{Deserialize, Serialize};
use vyre_foundation::ir::{BinOp, DataType, UnOp};

use crate::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

/// Minimum generated cases required by the release optimization gate.
pub const RELEASE_MIN_OPTIMIZATION_CASES: usize = 4_096;

/// One generated release optimization case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationCorpusCase {
    /// Stable generated case id.
    pub id: String,
    /// Optimization family this case exercises.
    pub family: String,
    /// Descriptor that is verified and optimized by the release corpus.
    pub descriptor: KernelDescriptor,
}

/// Machine-readable summary for the generated corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationCorpusManifest {
    /// Artifact schema version.
    pub schema_version: u32,
    /// Minimum release case count required by the gate.
    pub required_min_cases: usize,
    /// Number of generated cases in this manifest.
    pub generated_cases: usize,
    /// Number of cases accepted by descriptor verification.
    pub verified_cases: usize,
    /// Number of cases changed by the canonical optimization pipeline.
    pub optimized_cases: usize,
    /// Number of cases that exercise Dataflow-aware rewrites.
    pub dataflow_analysis_cases: usize,
    /// Number of Dataflow-aware cases that actually fired.
    pub dataflow_analysis_optimized_cases: usize,
    /// Number of cases that exhausted rewrite convergence fuel.
    pub non_converged_cases: usize,
    /// Total operation count before optimization.
    pub total_ops_before: usize,
    /// Total operation count after optimization.
    pub total_ops_after: usize,
    /// Per-family generated case counts.
    pub families: Vec<OptimizationFamilyCount>,
    /// Release blockers found while validating the corpus.
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationFamilyCount {
    /// Optimization family id.
    pub family: String,
    /// Number of generated cases in the family.
    pub cases: usize,
}

/// Generate the deterministic release corpus.
#[must_use]
pub fn generate_release_corpus() -> Vec<OptimizationCorpusCase> {
    let mut cases = Vec::new();
    for seed in 0..256 {
        push_algebraic_cases(seed, &mut cases);
        push_boolean_cases(seed, &mut cases);
        push_memory_cases(seed, &mut cases);
        push_control_cases(seed, &mut cases);
        push_vector_layout_cases(seed, &mut cases);
        push_analysis_fixture_cases(seed, &mut cases);
    }
    cases
}

/// Summarize a generated corpus for release evidence artifacts.
#[must_use]
pub fn manifest_for(cases: &[OptimizationCorpusCase]) -> OptimizationCorpusManifest {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for case in cases {
        *counts.entry(case.family.clone()).or_default() += 1;
    }
    let validation = validate_release_corpus(cases);
    OptimizationCorpusManifest {
        schema_version: 1,
        required_min_cases: RELEASE_MIN_OPTIMIZATION_CASES,
        generated_cases: cases.len(),
        verified_cases: validation.verified_cases,
        optimized_cases: validation.optimized_cases,
        dataflow_analysis_cases: validation.dataflow_analysis_cases,
        dataflow_analysis_optimized_cases: validation.dataflow_analysis_optimized_cases,
        non_converged_cases: validation.non_converged_cases,
        total_ops_before: validation.total_ops_before,
        total_ops_after: validation.total_ops_after,
        families: counts
            .into_iter()
            .map(|(family, cases)| OptimizationFamilyCount { family, cases })
            .collect(),
        blockers: validation.blockers,
    }
}

/// Validation summary for the generated corpus. This is intentionally
/// descriptor-level: every generated case must verify before
/// optimization, run through the canonical rewrite stack, and verify
/// after optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationCorpusValidation {
    /// Number of generated descriptors accepted by verification.
    pub verified_cases: usize,
    /// Number of generated descriptors changed by optimization.
    pub optimized_cases: usize,
    /// Number of generated descriptors using Dataflow facts.
    pub dataflow_analysis_cases: usize,
    /// Number of Dataflow descriptors changed by external-dataflow optimization.
    pub dataflow_analysis_optimized_cases: usize,
    /// Number of descriptors that did not converge.
    pub non_converged_cases: usize,
    /// Total operation count before optimization.
    pub total_ops_before: usize,
    /// Total operation count after optimization.
    pub total_ops_after: usize,
    /// Validation blockers.
    pub blockers: Vec<String>,
}

/// Validate all generated release corpus descriptors through the real
/// `verify_then_optimize` production entry point.
#[must_use]
pub fn validate_release_corpus(cases: &[OptimizationCorpusCase]) -> OptimizationCorpusValidation {
    let mut verified_cases = 0usize;
    let mut optimized_cases = 0usize;
    let mut dataflow_analysis_cases = 0usize;
    let mut dataflow_analysis_optimized_cases = 0usize;
    let mut non_converged_cases = 0usize;
    let mut total_ops_before = 0usize;
    let mut total_ops_after = 0usize;
    let mut blockers = Vec::new();

    for case in cases {
        match crate::verify_then_optimize(&case.descriptor) {
            Ok((_optimized, stats)) => {
                verified_cases += 1;
                total_ops_before = total_ops_before.saturating_add(stats.ops_before);
                total_ops_after = total_ops_after.saturating_add(stats.ops_after);
                if !stats.is_no_op() {
                    optimized_cases += 1;
                }
                if !stats.converged {
                    non_converged_cases += 1;
                    blockers.push(format!(
                        "case `{}` did not converge within the canonical rewrite fuel",
                        case.id
                    ));
                }
                if is_weir_dataflow_family(&case.family) {
                    dataflow_analysis_cases += 1;
                    let alias_facts = release_alias_facts();
                    let reaching_defs = release_reaching_defs(&case.family);
                    let (optimized, dataflow_stats) = if case.family == "weir-dataflow-loop-fission"
                    {
                        (
                            crate::rewrites::loop_fission_with_dataflow_facts(
                                &case.descriptor,
                                &alias_facts,
                                &reaching_defs,
                            ),
                            crate::rewrites::OptimizationStats {
                                ops_before: case.descriptor.body.ops.len(),
                                ops_after: case.descriptor.body.ops.len(),
                                bindings_before: case.descriptor.bindings.slots.len(),
                                bindings_after: case.descriptor.bindings.slots.len(),
                                literals_before: case.descriptor.body.literals.len(),
                                literals_after: case.descriptor.body.literals.len(),
                                iterations: 1,
                                converged: true,
                            },
                        )
                    } else {
                        crate::rewrites::run_all_with_dataflow_stats(
                            &case.descriptor,
                            &alias_facts,
                            &reaching_defs,
                        )
                    };
                    if !dataflow_case_fired(&case.family, &case.descriptor, &optimized) {
                        blockers.push(format!(
                            "case `{}` did not fire under Dataflow-aware optimization",
                            case.id
                        ));
                    } else {
                        dataflow_analysis_optimized_cases += 1;
                    }
                    if !dataflow_stats.converged {
                        non_converged_cases += 1;
                        blockers.push(format!(
                            "case `{}` did not converge under Dataflow-aware rewrite fuel",
                            case.id
                        ));
                    }
                }
            }
            Err(error) => blockers.push(format!(
                "case `{}` failed verify_then_optimize: {:?}",
                case.id, error
            )),
        }
    }

    OptimizationCorpusValidation {
        verified_cases,
        optimized_cases,
        dataflow_analysis_cases,
        dataflow_analysis_optimized_cases,
        non_converged_cases,
        total_ops_before,
        total_ops_after,
        blockers,
    }
}

fn store_count(body: &KernelBody) -> usize {
    let local = body
        .ops
        .iter()
        .filter(|op| {
            matches!(
                op.kind,
                KernelOpKind::StoreGlobal | KernelOpKind::StoreShared
            )
        })
        .count();
    local + body.child_bodies.iter().map(store_count).sum::<usize>()
}

fn loop_count(body: &KernelBody) -> usize {
    let local = body
        .ops
        .iter()
        .filter(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. }))
        .count();
    local + body.child_bodies.iter().map(loop_count).sum::<usize>()
}

fn top_level_load_count(body: &KernelBody) -> usize {
    body.ops
        .iter()
        .filter(|op| matches!(op.kind, KernelOpKind::LoadGlobal | KernelOpKind::LoadShared))
        .count()
}

fn dataflow_case_fired(family: &str, before: &KernelDescriptor, after: &KernelDescriptor) -> bool {
    match family {
        "weir-dataflow-dse" => store_count(&after.body) < store_count(&before.body),
        "weir-dataflow-loop-fusion" => loop_count(&after.body) < loop_count(&before.body),
        "weir-dataflow-loop-fission" => loop_count(&after.body) > loop_count(&before.body),
        "weir-dataflow-licm" => {
            top_level_load_count(&after.body) > top_level_load_count(&before.body)
        }
        _ => after != before,
    }
}

fn is_weir_dataflow_family(family: &str) -> bool {
    matches!(
        family,
        "weir-dataflow-dse"
            | "weir-dataflow-loop-fusion"
            | "weir-dataflow-loop-fission"
            | "weir-dataflow-licm"
    )
}

fn release_alias_facts() -> crate::analyses::alias_facts::AliasFactSet {
    let mut facts = crate::analyses::alias_facts::AliasFactSet::default();
    facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
        left_binding: 0,
        left_index: 10,
        right_binding: 0,
        right_index: 11,
    });
    facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
        left_binding: 0,
        left_index: 11,
        right_binding: 0,
        right_index: 12,
    });
    facts
}

fn release_reaching_defs(family: &str) -> crate::analyses::reaching_def_facts::ReachingDefFactSet {
    let mut facts = crate::analyses::reaching_def_facts::ReachingDefFactSet::default();
    if family == "weir-dataflow-dse" {
        facts.set_reaching_defs(20, vec![10]);
    } else {
        facts.set_reaching_defs(20, vec![11]);
    }
    facts.set_reaching_defs(40, vec![12]);
    facts
}

fn push_algebraic_cases(seed: u32, cases: &mut Vec<OptimizationCorpusCase>) {
    for (name, op, rhs) in [
        ("add_zero", BinOp::Add, LiteralValue::U32(0)),
        ("sub_zero", BinOp::Sub, LiteralValue::U32(0)),
        ("mul_one", BinOp::Mul, LiteralValue::U32(1)),
        ("mul_pow2", BinOp::Mul, LiteralValue::U32(1 << (seed % 16))),
        ("div_one", BinOp::Div, LiteralValue::U32(1)),
        (
            "mod_pow2",
            BinOp::Mod,
            LiteralValue::U32(1 << ((seed % 15) + 1)),
        ),
        ("bitand_all", BinOp::BitAnd, LiteralValue::U32(u32::MAX)),
        ("bitor_zero", BinOp::BitOr, LiteralValue::U32(0)),
        ("xor_zero", BinOp::BitXor, LiteralValue::U32(0)),
    ] {
        cases.push(binary_case(
            "algebraic",
            name,
            seed,
            op,
            LiteralValue::U32(seed),
            rhs,
        ));
    }
    cases.push(egraph_constant_chain_case(
        "add_const_chain",
        seed,
        BinOp::Add,
    ));
    cases.push(egraph_constant_chain_case(
        "mul_const_chain",
        seed,
        BinOp::Mul,
    ));
    cases.push(egraph_constant_chain_case(
        "bitand_const_chain",
        seed,
        BinOp::BitAnd,
    ));
    cases.push(egraph_constant_chain_case(
        "bitor_const_chain",
        seed,
        BinOp::BitOr,
    ));
    cases.push(egraph_constant_chain_case(
        "bitxor_const_chain",
        seed,
        BinOp::BitXor,
    ));
}

fn push_boolean_cases(seed: u32, cases: &mut Vec<OptimizationCorpusCase>) {
    for (name, op, lhs, rhs) in [
        (
            "and_true",
            BinOp::And,
            LiteralValue::Bool(seed & 1 == 0),
            LiteralValue::Bool(true),
        ),
        (
            "or_false",
            BinOp::Or,
            LiteralValue::Bool(seed & 1 == 1),
            LiteralValue::Bool(false),
        ),
        (
            "eq_self",
            BinOp::Eq,
            LiteralValue::U32(seed),
            LiteralValue::U32(seed),
        ),
        (
            "ne_self",
            BinOp::Ne,
            LiteralValue::U32(seed),
            LiteralValue::U32(seed),
        ),
        (
            "lt_const",
            BinOp::Lt,
            LiteralValue::U32(seed),
            LiteralValue::U32(seed + 1),
        ),
        (
            "ge_const",
            BinOp::Ge,
            LiteralValue::U32(seed + 1),
            LiteralValue::U32(seed),
        ),
    ] {
        cases.push(binary_case("predicate", name, seed, op, lhs, rhs));
    }
    cases.push(egraph_boolean_chain_case(
        "bool_and_const_chain",
        seed,
        BinOp::And,
    ));
    cases.push(egraph_boolean_chain_case(
        "bool_or_const_chain",
        seed,
        BinOp::Or,
    ));
    cases.push(unary_case(
        "predicate",
        "not_not",
        seed,
        UnOp::LogicalNot,
        LiteralValue::Bool(seed & 1 == 0),
    ));
}

fn push_memory_cases(seed: u32, cases: &mut Vec<OptimizationCorpusCase>) {
    for memory_class in [
        MemoryClass::Global,
        MemoryClass::Shared,
        MemoryClass::Constant,
        MemoryClass::Scratch,
    ] {
        let visibilities: &[BindingVisibility] = if matches!(memory_class, MemoryClass::Constant) {
            &[BindingVisibility::ReadOnly]
        } else {
            &[
                BindingVisibility::ReadOnly,
                BindingVisibility::WriteOnly,
                BindingVisibility::ReadWrite,
            ]
        };
        for visibility in visibilities {
            let mut desc = literal_descriptor("memory", "binding_layout", seed);
            desc.bindings.slots.push(BindingSlot {
                slot: slot_for_memory_class(seed % 16, memory_class),
                element_type: DataType::U32,
                element_count: Some(256 + seed),
                memory_class,
                visibility: *visibility,
                name: format!("buf_{memory_class:?}_{visibility:?}_{seed}"),
            });
            cases.push(OptimizationCorpusCase {
                id: format!("memory.binding_layout.{memory_class:?}.{visibility:?}.{seed}"),
                family: "memory-layout".to_string(),
                descriptor: desc,
            });
        }
    }
    cases.push(dataflow_dse_case(seed));
}

fn dataflow_dse_case(seed: u32) -> OptimizationCorpusCase {
    let family = "weir-dataflow-dse";
    let mut desc = literal_descriptor(family, "equivalent_dynamic_index", seed);
    desc.bindings.slots = vec![buffer_slot(
        0,
        MemoryClass::Global,
        BindingVisibility::ReadWrite,
        "dse_buf",
    )];
    desc.body.literals = vec![
        LiteralValue::U32(seed % 64),
        LiteralValue::U32(seed.wrapping_mul(3)),
        LiteralValue::U32(seed.wrapping_mul(5).wrapping_add(1)),
    ];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(10),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![1],
            result: Some(11),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![2],
            result: Some(12),
        },
        KernelOp {
            kind: KernelOpKind::Copy,
            operands: vec![10],
            result: Some(20),
        },
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 10, 11],
            result: None,
        },
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 20, 12],
            result: None,
        },
    ];
    OptimizationCorpusCase {
        id: format!("{family}.equivalent_dynamic_index.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn push_control_cases(seed: u32, cases: &mut Vec<OptimizationCorpusCase>) {
    let mut desc = literal_descriptor("control", "branch_collapse", seed);
    desc.body.literals = vec![LiteralValue::Bool(seed & 1 == 0)];
    desc.body.child_bodies.push(KernelBody {
        ops: vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        }],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(seed)],
    });
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::StructuredIfThen,
        operands: vec![0, 0],
        result: None,
    });
    cases.push(OptimizationCorpusCase {
        id: format!("control.branch_collapse.{seed}"),
        family: "control-flow".to_string(),
        descriptor: desc,
    });
    cases.push(dataflow_loop_fusion_case(seed));
    cases.push(dataflow_loop_fission_case(seed));
    cases.push(dataflow_licm_case(seed));
}

fn dataflow_loop_fusion_case(seed: u32) -> OptimizationCorpusCase {
    let family = "weir-dataflow-loop-fusion";
    let mut desc = loop_descriptor_with_parent_values(family, "equivalent_alias_indices", seed);
    desc.body.ops.push(structured_loop(0));
    desc.body.ops.push(structured_loop(1));
    desc.body.child_bodies = vec![
        KernelBody {
            ops: vec![store_global(10, 30)],
            child_bodies: vec![],
            literals: vec![],
        },
        KernelBody {
            ops: vec![store_global(20, 31)],
            child_bodies: vec![],
            literals: vec![],
        },
    ];
    OptimizationCorpusCase {
        id: format!("{family}.equivalent_alias_indices.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn dataflow_loop_fission_case(seed: u32) -> OptimizationCorpusCase {
    let family = "weir-dataflow-loop-fission";
    let mut desc = loop_descriptor_with_parent_values(family, "equivalent_alias_indices", seed);
    desc.body.ops.push(structured_loop(0));
    desc.body.child_bodies = vec![KernelBody {
        ops: vec![store_global(10, 30), store_global(20, 31)],
        child_bodies: vec![],
        literals: vec![],
    }];
    OptimizationCorpusCase {
        id: format!("{family}.equivalent_alias_indices.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn dataflow_licm_case(seed: u32) -> OptimizationCorpusCase {
    let family = "weir-dataflow-licm";
    let mut desc = loop_descriptor_with_parent_values(family, "equivalent_alias_indices", seed);
    let index_literal = desc.body.literals.len() as u32;
    desc.body
        .literals
        .push(LiteralValue::U32(seed.wrapping_add(13)));
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::Literal,
        operands: vec![index_literal],
        result: Some(12),
    });
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::Copy,
        operands: vec![12],
        result: Some(40),
    });
    desc.body.ops.push(structured_loop(0));
    desc.body.child_bodies = vec![KernelBody {
        ops: vec![
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 20],
                result: Some(50),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![0, 40, 50],
                result: None,
            },
        ],
        child_bodies: vec![],
        literals: vec![],
    }];
    OptimizationCorpusCase {
        id: format!("{family}.equivalent_alias_indices.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn loop_descriptor_with_parent_values(family: &str, name: &str, seed: u32) -> KernelDescriptor {
    let mut desc = literal_descriptor(family, name, seed);
    desc.bindings.slots = vec![buffer_slot(
        0,
        MemoryClass::Global,
        BindingVisibility::ReadWrite,
        "loop_buf",
    )];
    desc.body.literals = vec![
        LiteralValue::U32(0),
        LiteralValue::U32(64),
        LiteralValue::U32(seed % 64),
        LiteralValue::U32((seed % 64).wrapping_add(1)),
        LiteralValue::U32(seed.wrapping_add(7)),
        LiteralValue::U32(seed.wrapping_add(9)),
    ];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![1],
            result: Some(1),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![2],
            result: Some(10),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![3],
            result: Some(11),
        },
        KernelOp {
            kind: KernelOpKind::Copy,
            operands: vec![11],
            result: Some(20),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![4],
            result: Some(30),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![5],
            result: Some(31),
        },
    ];
    desc
}

fn structured_loop(child: u32) -> KernelOp {
    KernelOp {
        kind: KernelOpKind::StructuredForLoop {
            loop_var: "i".into(),
        },
        operands: vec![0, 1, child],
        result: None,
    }
}

fn store_global(index: u32, value: u32) -> KernelOp {
    KernelOp {
        kind: KernelOpKind::StoreGlobal,
        operands: vec![0, index, value],
        result: None,
    }
}

fn push_vector_layout_cases(seed: u32, cases: &mut Vec<OptimizationCorpusCase>) {
    for workgroup in [32, 64, 128, 256] {
        let mut desc = binary_case(
            "vector-layout",
            "dispatch_shape",
            seed.saturating_mul(workgroup),
            BinOp::Add,
            LiteralValue::U32(seed),
            LiteralValue::U32(workgroup),
        )
        .descriptor;
        desc.dispatch = Dispatch::new(workgroup, 1, 1);
        cases.push(OptimizationCorpusCase {
            id: format!("vector_layout.dispatch_shape.{workgroup}.{seed}"),
            family: "vector-layout".to_string(),
            descriptor: desc,
        });
    }
}

fn push_analysis_fixture_cases(seed: u32, cases: &mut Vec<OptimizationCorpusCase>) {
    cases.push(coalesce_fixture_case(seed, "unit_stride"));
    cases.push(coalesce_fixture_case(seed, "strided"));
    cases.push(coalesce_fixture_case(seed, "broadcast"));
    cases.push(shared_mem_promote_fixture_case(seed));
    cases.push(bank_conflict_fixture_case(seed));
    cases.push(vec_pack_fixture_case(seed));
}

fn coalesce_fixture_case(seed: u32, name: &str) -> OptimizationCorpusCase {
    let family = "A13-coalesce-fixture";
    let mut desc = literal_descriptor(family, name, seed);
    desc.bindings.slots = vec![
        buffer_slot(
            0,
            MemoryClass::Global,
            BindingVisibility::ReadOnly,
            "coalesce_in",
        ),
        buffer_slot(
            1,
            MemoryClass::Global,
            BindingVisibility::WriteOnly,
            "coalesce_out",
        ),
    ];
    desc.body.literals = vec![LiteralValue::U32(if name == "strided" {
        4
    } else {
        (seed % 17) + 1
    })];
    desc.body.ops = vec![KernelOp {
        kind: KernelOpKind::LocalInvocationId,
        operands: vec![0],
        result: Some(0),
    }];
    let index = match name {
        "strided" => {
            desc.body.ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(1),
            });
            desc.body.ops.push(KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Mul),
                operands: vec![0, 1],
                result: Some(2),
            });
            2
        }
        "broadcast" => {
            desc.body.ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(2),
            });
            2
        }
        _ => 0,
    };
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::LoadGlobal,
        operands: vec![0, index],
        result: Some(3),
    });
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::StoreGlobal,
        operands: vec![1, 0, 3],
        result: None,
    });
    OptimizationCorpusCase {
        id: format!("{family}.{name}.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn shared_mem_promote_fixture_case(seed: u32) -> OptimizationCorpusCase {
    let family = "A14-shared-mem-promote-fixture";
    let mut desc = literal_descriptor(family, "repeated_global_tile_load", seed);
    desc.bindings.slots = vec![
        buffer_slot(
            0,
            MemoryClass::Global,
            BindingVisibility::ReadOnly,
            "tile_in",
        ),
        buffer_slot(
            1,
            MemoryClass::Global,
            BindingVisibility::WriteOnly,
            "tile_out",
        ),
    ];
    desc.body.literals = vec![LiteralValue::U32((seed % 8) + 1)];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::LocalInvocationId,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::LoadGlobal,
            operands: vec![0, 0],
            result: Some(1),
        },
        KernelOp {
            kind: KernelOpKind::LoadGlobal,
            operands: vec![0, 0],
            result: Some(2),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Add),
            operands: vec![1, 2],
            result: Some(3),
        },
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![1, 0, 3],
            result: None,
        },
    ];
    OptimizationCorpusCase {
        id: format!("{family}.repeated_global_tile_load.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn bank_conflict_fixture_case(seed: u32) -> OptimizationCorpusCase {
    let family = "A15-bank-conflict-fixture";
    let mut desc = literal_descriptor(family, "shared_stride_32", seed);
    let shared_tile_slot = crate::lower::WORKGROUP_SLOT_BASE;
    desc.bindings.slots = vec![
        buffer_slot(
            0,
            MemoryClass::Shared,
            BindingVisibility::ReadWrite,
            "shared_tile",
        ),
        buffer_slot(
            1,
            MemoryClass::Global,
            BindingVisibility::WriteOnly,
            "bank_out",
        ),
    ];
    desc.body.literals = vec![LiteralValue::U32(32)];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::LocalInvocationId,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(1),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![0, 1],
            result: Some(2),
        },
        KernelOp {
            kind: KernelOpKind::LoadShared,
            operands: vec![shared_tile_slot, 2],
            result: Some(3),
        },
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![1, 0, 3],
            result: None,
        },
    ];
    OptimizationCorpusCase {
        id: format!("{family}.shared_stride_32.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn vec_pack_fixture_case(seed: u32) -> OptimizationCorpusCase {
    let family = "A16-vec-pack-fixture";
    let mut desc = literal_descriptor(family, "contiguous_u32x4", seed);
    desc.bindings.slots = vec![
        buffer_slot(
            0,
            MemoryClass::Global,
            BindingVisibility::ReadOnly,
            "vec_in",
        ),
        buffer_slot(
            1,
            MemoryClass::Global,
            BindingVisibility::WriteOnly,
            "vec_out",
        ),
    ];
    desc.body.literals = vec![
        LiteralValue::U32(4),
        LiteralValue::U32(0),
        LiteralValue::U32(1),
        LiteralValue::U32(2),
        LiteralValue::U32(3),
    ];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::LocalInvocationId,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(1),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![0, 1],
            result: Some(2),
        },
    ];
    for lane in 0..4 {
        let literal_result = 3 + lane * 3;
        let index_result = literal_result + 1;
        let load_result = literal_result + 2;
        desc.body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![lane + 1],
            result: Some(literal_result),
        });
        desc.body.ops.push(KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Add),
            operands: vec![2, literal_result],
            result: Some(index_result),
        });
        desc.body.ops.push(KernelOp {
            kind: KernelOpKind::LoadGlobal,
            operands: vec![0, index_result],
            result: Some(load_result),
        });
    }
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::BinOpKind(BinOp::Add),
        operands: vec![5, 8],
        result: Some(15),
    });
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::BinOpKind(BinOp::Add),
        operands: vec![11, 14],
        result: Some(16),
    });
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::BinOpKind(BinOp::Add),
        operands: vec![15, 16],
        result: Some(17),
    });
    desc.body.ops.push(KernelOp {
        kind: KernelOpKind::StoreGlobal,
        operands: vec![1, 0, 17],
        result: None,
    });
    OptimizationCorpusCase {
        id: format!("{family}.contiguous_u32x4.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn buffer_slot(
    slot: u32,
    memory_class: MemoryClass,
    visibility: BindingVisibility,
    name: &str,
) -> BindingSlot {
    BindingSlot {
        slot: slot_for_memory_class(slot, memory_class),
        element_type: DataType::U32,
        element_count: Some(4096),
        memory_class,
        visibility,
        name: name.to_string(),
    }
}

fn slot_for_memory_class(slot: u32, memory_class: MemoryClass) -> u32 {
    if matches!(memory_class, MemoryClass::Shared | MemoryClass::Scratch) {
        crate::lower::WORKGROUP_SLOT_BASE + slot
    } else {
        slot
    }
}

fn binary_case(
    family: &str,
    name: &str,
    seed: u32,
    op: BinOp,
    lhs: LiteralValue,
    rhs: LiteralValue,
) -> OptimizationCorpusCase {
    let mut desc = literal_descriptor(family, name, seed);
    desc.body.literals = vec![lhs, rhs];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![1],
            result: Some(1),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![0, 1],
            result: Some(2),
        },
    ];
    OptimizationCorpusCase {
        id: format!("{family}.{name}.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn unary_case(
    family: &str,
    name: &str,
    seed: u32,
    op: UnOp,
    input: LiteralValue,
) -> OptimizationCorpusCase {
    let mut desc = literal_descriptor(family, name, seed);
    desc.body.literals = vec![input];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::UnOpKind(op),
            operands: vec![0],
            result: Some(1),
        },
    ];
    OptimizationCorpusCase {
        id: format!("{family}.{name}.{seed}"),
        family: family.to_string(),
        descriptor: desc,
    }
}

fn egraph_constant_chain_case(name: &str, seed: u32, op: BinOp) -> OptimizationCorpusCase {
    let mut desc = literal_descriptor("egraph", name, seed);
    desc.bindings.slots = vec![BindingSlot {
        slot: 0,
        element_type: DataType::U32,
        element_count: Some(1024),
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::WriteOnly,
        name: "out".to_string(),
    }];
    let left_const = (seed % 7) + 2;
    let right_const = (seed % 11) + 3;
    desc.body.literals = vec![
        LiteralValue::U32(left_const),
        LiteralValue::U32(right_const),
        LiteralValue::U32(seed % 1024),
    ];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::LocalInvocationId,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(1),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![1],
            result: Some(2),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![0, 1],
            result: Some(3),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![3, 2],
            result: Some(4),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![2],
            result: Some(5),
        },
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 5, 4],
            result: None,
        },
    ];
    OptimizationCorpusCase {
        id: format!("egraph.{name}.{seed}"),
        family: "egraph".to_string(),
        descriptor: desc,
    }
}

fn egraph_boolean_chain_case(name: &str, seed: u32, op: BinOp) -> OptimizationCorpusCase {
    let mut desc = literal_descriptor("egraph", name, seed);
    desc.body.literals = vec![
        LiteralValue::Bool(seed & 1 == 0),
        LiteralValue::Bool(seed & 2 == 0),
        LiteralValue::Bool(seed & 4 == 0),
    ];
    desc.body.ops = vec![
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![1],
            result: Some(1),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![0, 1],
            result: Some(2),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![2],
            result: Some(3),
        },
        KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![2, 3],
            result: Some(4),
        },
    ];
    OptimizationCorpusCase {
        id: format!("egraph.{name}.{seed}"),
        family: "egraph".to_string(),
        descriptor: desc,
    }
}

fn literal_descriptor(family: &str, name: &str, seed: u32) -> KernelDescriptor {
    KernelDescriptor {
        id: format!("{family}.{name}.{seed}"),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(seed)],
        },
    }
}

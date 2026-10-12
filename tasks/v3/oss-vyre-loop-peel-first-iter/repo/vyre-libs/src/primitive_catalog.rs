//! Registered Tier-3 consumers for Tier-2.5 primitives.
//!
//! These wrappers keep the primitive substrate executable from the same
//! inventory surface as the rest of `vyre-libs`. Each wrapper builds the real
//! primitive Program and tags it under an owning library op id, so conformance,
//! composition audits, and downstream catalogs see runnable IR rather than an
//! orphan primitive.

use std::sync::Arc;

use vyre::ir::{BufferAccess, DataType, MemoryKind, Node, Program};
use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum PrimitivePromotionBoundary {
    ProductionConsumers(&'static [&'static str]),
    ProofGates(&'static [&'static str]),
    NonShippedFeature(&'static str),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResearchPrimitivePromotion {
    pub(crate) primitive_id: &'static str,
    pub(crate) boundary: PrimitivePromotionBoundary,
}

pub(crate) const RESEARCH_PRIMITIVE_PROMOTIONS: &[ResearchPrimitivePromotion] = &[
    ResearchPrimitivePromotion {
        primitive_id: "vyre-primitives::math::hensel_lift_step",
        boundary: PrimitivePromotionBoundary::ProofGates(&[
            "inventory_expected_output",
            "padic_cpu_quadratic_root_converges",
        ]),
    },
];

pub(crate) fn validate_research_primitive_promotions(
    promotions: &[ResearchPrimitivePromotion],
) -> Result<(), String> {
    for promotion in promotions {
        let primitive_entry = vyre_primitives::harness::all_entries()
            .find(|entry| entry.id == promotion.primitive_id)
            .ok_or_else(|| {
                format!(
                    "Fix: research primitive `{}` is promotion-gated but missing primitive inventory registration.",
                    promotion.primitive_id
                )
            })?;
        match promotion.boundary {
            PrimitivePromotionBoundary::ProductionConsumers(consumers) => {
                if consumers.is_empty() {
                    return Err(format!(
                        "Fix: research primitive `{}` declares a production-consumer gate with no consumers.",
                        promotion.primitive_id
                    ));
                }
                for consumer in consumers {
                    if crate::harness::all_entries().all(|entry| entry.id != *consumer) {
                        return Err(format!(
                            "Fix: research primitive `{}` production consumer `{consumer}` is not registered in vyre-libs harness.",
                            promotion.primitive_id
                        ));
                    }
                }
            }
            PrimitivePromotionBoundary::ProofGates(proofs) => {
                if proofs.is_empty() {
                    return Err(format!(
                        "Fix: research primitive `{}` declares a proof gate with no proof evidence.",
                        promotion.primitive_id
                    ));
                }
                if primitive_entry.expected_output.is_none() {
                    return Err(format!(
                        "Fix: research primitive `{}` proof gate requires an inventory expected_output witness.",
                        promotion.primitive_id
                    ));
                }
            }
            PrimitivePromotionBoundary::NonShippedFeature(feature) => {
                if feature.is_empty() {
                    return Err(format!(
                        "Fix: research primitive `{}` declares an empty non-shipped feature boundary.",
                        promotion.primitive_id
                    ));
                }
            }
        }
    }
    Ok(())
}

fn primitive_program(wrapper_id: &str, primitive_id: &str) -> Program {
    let Some(entry) =
        vyre_primitives::harness::all_entries().find(|entry| entry.id == primitive_id)
    else {
        return Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
    };
    let primitive = (entry.build)();
    let primitive_child = Node::Region {
        generator: Ident::from(primitive_id),
        source_region: Some(GeneratorRef {
            name: wrapper_id.to_string(),
        }),
        body: Arc::new(primitive.entry().to_vec()),
    };
    Program::wrapped(
        primitive.buffers().to_vec(),
        primitive.workgroup_size(),
        vec![crate::region::wrap_anonymous(
            wrapper_id,
            vec![primitive_child],
        )],
    )
}

fn backend_owned_output(access: BufferAccess, is_output: bool, is_pipeline_live_out: bool) -> bool {
    matches!(access, BufferAccess::WriteOnly)
        || is_output
        || (is_pipeline_live_out && matches!(access, BufferAccess::ReadWrite))
}

fn catalog_inputs_for(entry: &vyre_primitives::harness::OpEntry) -> Vec<Vec<Vec<u8>>> {
    let program = (entry.build)();
    let logical_input_count = program
        .buffers()
        .iter()
        .filter(|buffer| {
            !matches!(buffer.access(), BufferAccess::Workgroup)
                && !backend_owned_output(
                    buffer.access(),
                    buffer.is_output(),
                    buffer.is_pipeline_live_out(),
                )
        })
        .count();
    let legacy_input_count = program
        .buffers()
        .iter()
        .filter(|buffer| !matches!(buffer.access(), BufferAccess::Workgroup))
        .count();
    let raw_cases = entry
        .test_inputs
        .map(|build| build())
        .unwrap_or_else(|| synthetic_catalog_inputs(&program, entry.id));
    raw_cases
        .into_iter()
        .map(|case| {
            if case.len() == logical_input_count {
                return case;
            }
            if case.len() != legacy_input_count {
                return Vec::new();
            }
            program
                .buffers()
                .iter()
                .filter(|buffer| !matches!(buffer.access(), BufferAccess::Workgroup))
                .zip(case)
                .filter_map(|(buffer, value)| {
                    if backend_owned_output(
                        buffer.access(),
                        buffer.is_output(),
                        buffer.is_pipeline_live_out(),
                    ) {
                        None
                    } else {
                        Some(value)
                    }
                })
                .collect()
        })
        .collect()
}

fn synthetic_catalog_inputs(program: &Program, primitive_id: &str) -> Vec<Vec<Vec<u8>>> {
    let mut case = Vec::new();
    for buffer in program.buffers() {
        if matches!(buffer.kind(), MemoryKind::Shared)
            || backend_owned_output(
                buffer.access(),
                buffer.is_output(),
                buffer.is_pipeline_live_out(),
            )
        {
            continue;
        }
        let Some(byte_len) = buffer.static_byte_len().ok().flatten() else {
            continue;
        };
        case.push(synthetic_catalog_bytes(&buffer.element(), byte_len));
    }
    if case.is_empty() {
        return Vec::new();
    }
    vec![case]
}

fn synthetic_catalog_bytes(element: &DataType, byte_len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; byte_len];
    match element {
        DataType::F32 => {
            for chunk in bytes.chunks_exact_mut(4) {
                chunk.copy_from_slice(&1.0f32.to_le_bytes());
            }
        }
        DataType::F64 => {
            for chunk in bytes.chunks_exact_mut(8) {
                chunk.copy_from_slice(&1.0f64.to_le_bytes());
            }
        }
        DataType::F16 | DataType::BF16 => {
            for chunk in bytes.chunks_exact_mut(2) {
                chunk.copy_from_slice(&0x3c00u16.to_le_bytes());
            }
        }
        _ => {}
    }
    bytes
}

macro_rules! catalog_pair {
    ($base:literal, $primitive:literal) => {
        const _: () = {
            fn inputs() -> Vec<Vec<Vec<u8>>> {
                let Some(entry) =
                    vyre_primitives::harness::all_entries().find(|entry| entry.id == $primitive)
                else {
                    return Vec::new();
                };
                catalog_inputs_for(entry)
            }
            fn expected() -> Vec<Vec<Vec<u8>>> {
                let Some(entry) =
                    vyre_primitives::harness::all_entries().find(|entry| entry.id == $primitive)
                else {
                    return Vec::new();
                };
                let Some(expected_output) = entry.expected_output else {
                    return Vec::new();
                };
                expected_output()
            }
            inventory::submit! {
                crate::harness::OpEntry {
                    id: concat!($base, "::consumer_a"),
                    build: || primitive_program(concat!($base, "::consumer_a"), $primitive),
                    test_inputs: Some(inputs),
                    expected_output: Some(expected),
                    category: None,
                }
            }
            inventory::submit! {
                crate::harness::OpEntry {
                    id: concat!($base, "::consumer_b"),
                    build: || primitive_program(concat!($base, "::consumer_b"), $primitive),
                    test_inputs: Some(inputs),
                    expected_output: Some(expected),
                    category: None,
                }
            }
        };
    };
}

catalog_pair!(
    "vyre-libs::catalog::predicate::return_value_of",
    "vyre-primitives::predicate::return_value_of"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::popcount",
    "vyre-primitives::bitset::popcount"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::size_argument_of",
    "vyre-primitives::predicate::size_argument_of"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::node_kind_eq",
    "vyre-primitives::predicate::node_kind_eq"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::in_function",
    "vyre-primitives::predicate::in_function"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::not",
    "vyre-primitives::bitset::not"
);
catalog_pair!(
    "vyre-libs::catalog::graph::tensor_flow_forward",
    "vyre-primitives::graph::tensor_flow_forward"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::literal_of",
    "vyre-primitives::predicate::literal_of"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::segment_reduce_sum",
    "vyre-primitives::reduce::segment_reduce_sum"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::min",
    "vyre-primitives::reduce::min"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::count",
    "vyre-primitives::reduce::count"
);
catalog_pair!(
    "vyre-libs::catalog::graph::scc_decompose",
    "vyre-primitives::graph::scc_decompose"
);
catalog_pair!(
    "vyre-libs::catalog::math::scallop_join_wide",
    "vyre-primitives::math::scallop_join_wide"
);
catalog_pair!(
    "vyre-libs::catalog::decode::inflate_stored",
    "vyre-primitives::decode::inflate_stored"
);
catalog_pair!(
    "vyre-libs::catalog::vfs::resolve",
    "vyre-primitives::vfs::resolve"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::call_to",
    "vyre-primitives::predicate::call_to"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::arg_of",
    "vyre-primitives::predicate::arg_of"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::contains",
    "vyre-primitives::bitset::contains"
);
catalog_pair!(
    "vyre-libs::catalog::nn::quest_score_pages",
    "vyre-primitives::nn::quest_score_pages"
);
catalog_pair!(
    "vyre-libs::catalog::nn::quest_zero_fill",
    "vyre-primitives::nn::quest_zero_fill"
);
catalog_pair!(
    "vyre-libs::catalog::text::byte_histogram_256",
    "vyre-primitives::text::byte_histogram_256"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::four_russians_apply_byte_lut",
    "vyre-primitives::bitset::four_russians_apply_byte_lut"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::four_russians_dense_matvec_byte_lut",
    "vyre-primitives::bitset::four_russians_dense_matvec_byte_lut"
);
catalog_pair!(
    "vyre-libs::catalog::math::sheaf_laplacian_eigenvalue",
    "vyre-primitives::math::sheaf_laplacian_eigenvalue"
);
catalog_pair!(
    "vyre-libs::catalog::hash::fnv1a64",
    "vyre-primitives::hash::fnv1a64"
);
catalog_pair!(
    "vyre-libs::catalog::hash::fnv1a32",
    "vyre-primitives::hash::fnv1a32"
);
catalog_pair!(
    "vyre-libs::catalog::decode::base64_decode",
    "vyre-primitives::decode::base64_decode"
);
catalog_pair!(
    "vyre-libs::catalog::math::amg_v_cycle",
    "vyre-primitives::math::amg_v_cycle"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::in_package",
    "vyre-primitives::predicate::in_package"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::scatter",
    "vyre-primitives::reduce::scatter"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::xor",
    "vyre-primitives::bitset::xor"
);
catalog_pair!(
    "vyre-libs::catalog::graph::persistent_bfs",
    "vyre-primitives::graph::persistent_bfs"
);
catalog_pair!(
    "vyre-libs::catalog::graph::dominator_tree",
    "vyre-primitives::graph::dominator_tree"
);
catalog_pair!(
    "vyre-libs::catalog::hash::blake3_round",
    "vyre-primitives::hash::blake3_round"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::max",
    "vyre-primitives::reduce::max"
);
catalog_pair!(
    "vyre-libs::catalog::math::scallop_join",
    "vyre-primitives::math::scallop_join"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::edge",
    "vyre-primitives::predicate::edge"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::histogram",
    "vyre-primitives::reduce::histogram"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::any",
    "vyre-primitives::bitset::any"
);
catalog_pair!(
    "vyre-libs::catalog::math::sinkhorn_iterate",
    "vyre-primitives::math::sinkhorn_iterate"
);
catalog_pair!(
    "vyre-libs::catalog::fixpoint::bitset_fixpoint",
    "vyre-primitives::fixpoint::bitset_fixpoint"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::or",
    "vyre-primitives::bitset::or"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::sum",
    "vyre-primitives::reduce::sum"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::gather",
    "vyre-primitives::reduce::gather"
);
catalog_pair!(
    "vyre-libs::catalog::graph::persistent_bfs_step",
    "vyre-primitives::graph::persistent_bfs_step"
);
catalog_pair!(
    "vyre-libs::catalog::math::tensor_train_decompose",
    "vyre-primitives::math::tensor_train_decompose"
);
catalog_pair!(
    "vyre-libs::catalog::predicate::in_file",
    "vyre-primitives::predicate::in_file"
);
catalog_pair!(
    "vyre-libs::catalog::math::bellman_shortest_path",
    "vyre-primitives::math::bellman_shortest_path"
);
catalog_pair!(
    "vyre-libs::catalog::matching::bracket_match",
    "vyre-primitives::matching::bracket_match"
);
catalog_pair!(
    "vyre-libs::catalog::math::newton_schulz_poly5_f32",
    "vyre-primitives::math::newton_schulz_poly5_f32"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::workgroup_sum_u32",
    "vyre-primitives::reduce::workgroup_sum_u32"
);
catalog_pair!(
    "vyre-libs::catalog::parsing::ast_cse_structural_hash",
    "vyre-primitives::parsing::ast_cse_structural_hash"
);
catalog_pair!(
    "vyre-libs::catalog::decode::rle_segment_lengths",
    "vyre-primitives::decode::rle_segment_lengths"
);
catalog_pair!(
    "vyre-libs::catalog::graph::vast_walk_postorder",
    "vyre-primitives::graph::vast_walk_postorder"
);
catalog_pair!(
    "vyre-libs::catalog::graph::vast_walk_preorder",
    "vyre-primitives::graph::vast_walk_preorder"
);
catalog_pair!(
    "vyre-libs::catalog::parsing::ssa_dominance_scan",
    "vyre-primitives::parsing::ssa_dominance_scan"
);
catalog_pair!(
    "vyre-libs::catalog::text::encoding_classify",
    "vyre-primitives::text::encoding_classify"
);

catalog_pair!(
    "vyre-libs::catalog::bitset::and",
    "vyre-primitives::bitset::and"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::and_into",
    "vyre-primitives::bitset::and_into"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::and_not",
    "vyre-primitives::bitset::and_not"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::and_not_into",
    "vyre-primitives::bitset::and_not_into"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::clear_bit",
    "vyre-primitives::bitset::clear_bit"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::copy",
    "vyre-primitives::bitset::copy"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::equal",
    "vyre-primitives::bitset::equal"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::or_into",
    "vyre-primitives::bitset::or_into"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::select1_query",
    "vyre-primitives::bitset::select1_query"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::set_bit",
    "vyre-primitives::bitset::set_bit"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::stochastic_and_mul",
    "vyre-primitives::bitset::stochastic_and_mul"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::subset_of",
    "vyre-primitives::bitset::subset_of"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::test_bit",
    "vyre-primitives::bitset::test_bit"
);
catalog_pair!(
    "vyre-libs::catalog::bitset::xor_into",
    "vyre-primitives::bitset::xor_into"
);
catalog_pair!(
    "vyre-libs::catalog::fixpoint::persistent_fixpoint",
    "vyre-primitives::fixpoint::persistent_fixpoint"
);
catalog_pair!(
    "vyre-libs::catalog::geom::clifford2_geometric_product",
    "vyre-primitives::geom::clifford2_geometric_product"
);
catalog_pair!(
    "vyre-libs::catalog::graph::csr_backward_traverse",
    "vyre-primitives::graph::csr_backward_traverse"
);
catalog_pair!(
    "vyre-libs::catalog::graph::csr_forward_or_changed",
    "vyre-primitives::graph::csr_forward_or_changed"
);
catalog_pair!(
    "vyre-libs::catalog::graph::csr_forward_traverse",
    "vyre-primitives::graph::csr_forward_traverse"
);
catalog_pair!(
    "vyre-libs::catalog::graph::csr_frontier_degree_sum",
    "vyre-primitives::graph::csr_frontier_degree_sum"
);
catalog_pair!(
    "vyre-libs::catalog::graph::dominator_frontier",
    "vyre-primitives::graph::dominator_frontier"
);
catalog_pair!(
    "vyre-libs::catalog::graph::functor_apply",
    "vyre-primitives::graph::functor_apply"
);
catalog_pair!(
    "vyre-libs::catalog::graph::monoidal_compose",
    "vyre-primitives::graph::monoidal_compose"
);
catalog_pair!(
    "vyre-libs::catalog::graph::motif",
    "vyre-primitives::graph::motif"
);
catalog_pair!(
    "vyre-libs::catalog::graph::path_reconstruct",
    "vyre-primitives::graph::path_reconstruct"
);
catalog_pair!(
    "vyre-libs::catalog::graph::sheaf_diffusion_step",
    "vyre-primitives::graph::sheaf_diffusion_step"
);
catalog_pair!(
    "vyre-libs::catalog::hash::blake3_g",
    "vyre-primitives::hash::blake3_g"
);
catalog_pair!(
    "vyre-libs::catalog::hash::crc32",
    "vyre-primitives::hash::crc32"
);
catalog_pair!(
    "vyre-libs::catalog::hash::ntt_butterfly_stage",
    "vyre-primitives::hash::ntt_butterfly_stage"
);
catalog_pair!(
    "vyre-libs::catalog::hash::sparse_fft_bin_hash",
    "vyre-primitives::hash::sparse_fft_bin_hash"
);
catalog_pair!(
    "vyre-libs::catalog::label::resolve_family",
    "vyre-primitives::label::resolve_family"
);
catalog_pair!(
    "vyre-libs::catalog::math::amg_jacobi_step",
    "vyre-primitives::math::amg_jacobi_step"
);
catalog_pair!(
    "vyre-libs::catalog::math::amg_v_cycle::v_cycle_phase",
    "vyre-primitives::math::amg_v_cycle::v_cycle_phase"
);
catalog_pair!(
    "vyre-libs::catalog::math::bhattacharyya_coefficient",
    "vyre-primitives::math::bhattacharyya_coefficient"
);
catalog_pair!(
    "vyre-libs::catalog::math::bigint_add_carry",
    "vyre-primitives::math::bigint_add_carry"
);
catalog_pair!(
    "vyre-libs::catalog::math::conformal_threshold",
    "vyre-primitives::math::conformal_threshold"
);
catalog_pair!(
    "vyre-libs::catalog::math::conv1d",
    "vyre-primitives::math::conv1d"
);
catalog_pair!(
    "vyre-libs::catalog::math::dot_partial",
    "vyre-primitives::math::dot_partial"
);
catalog_pair!(
    "vyre-libs::catalog::math::gaussian_rdp_step",
    "vyre-primitives::math::gaussian_rdp_step"
);
catalog_pair!(
    "vyre-libs::catalog::math::hensel_lift_step",
    "vyre-primitives::math::hensel_lift_step"
);
catalog_pair!(
    "vyre-libs::catalog::math::iht_threshold",
    "vyre-primitives::math::iht_threshold"
);
catalog_pair!(
    "vyre-libs::catalog::math::mori_zwanzig_project_step",
    "vyre-primitives::math::mori_zwanzig_project_step"
);
catalog_pair!(
    "vyre-libs::catalog::math::mp_edge_clip",
    "vyre-primitives::math::mp_edge_clip"
);
catalog_pair!(
    "vyre-libs::catalog::math::qsvt_block_encode",
    "vyre-primitives::math::qsvt_block_encode"
);
catalog_pair!(
    "vyre-libs::catalog::math::semiring_gemm",
    "vyre-primitives::math::semiring_gemm"
);
catalog_pair!(
    "vyre-libs::catalog::math::sheaf_laplacian_eigenvalue::power_iteration_phase",
    "vyre-primitives::math::sheaf_laplacian_eigenvalue::power_iteration_phase"
);
catalog_pair!(
    "vyre-libs::catalog::math::sinkhorn_scale",
    "vyre-primitives::math::sinkhorn_scale"
);
catalog_pair!(
    "vyre-libs::catalog::math::softmax_step",
    "vyre-primitives::math::softmax_step"
);
catalog_pair!(
    "vyre-libs::catalog::math::tensor_network_pair_contract",
    "vyre-primitives::math::tensor_network_pair_contract"
);
catalog_pair!(
    "vyre-libs::catalog::nn::attention_max_pass",
    "vyre-primitives::nn::attention_max_pass"
);
catalog_pair!(
    "vyre-libs::catalog::nn::attention_sum_pass",
    "vyre-primitives::nn::attention_sum_pass"
);
catalog_pair!(
    "vyre-libs::catalog::nn::attention_write_pass",
    "vyre-primitives::nn::attention_write_pass"
);
catalog_pair!(
    "vyre-libs::catalog::nn::quest_select_top_k",
    "vyre-primitives::nn::quest_select_top_k"
);
catalog_pair!(
    "vyre-libs::catalog::parsing::core_delimiter_match",
    "vyre-primitives::parsing::core_delimiter_match"
);
catalog_pair!(
    "vyre-libs::catalog::parsing::line_splice_classify",
    "vyre-primitives::parsing::line_splice_classify"
);
catalog_pair!(
    "vyre-libs::catalog::parsing::planar_rewrite_schedule",
    "vyre-primitives::parsing::planar_rewrite_schedule"
);
catalog_pair!(
    "vyre-libs::catalog::parsing::whitespace_classify_word",
    "vyre-primitives::parsing::whitespace_classify_word"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::all",
    "vyre-primitives::reduce::all"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::any",
    "vyre-primitives::reduce::any"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::count_non_zero",
    "vyre-primitives::reduce::count_non_zero"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::radix_sort",
    "vyre-primitives::reduce::radix_sort"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::range_counts_u32",
    "vyre-primitives::reduce::range_counts_u32"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::workgroup_any_u32",
    "vyre-primitives::reduce::workgroup_any_u32"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::workgroup_max_f32",
    "vyre-primitives::reduce::workgroup_max_f32"
);
catalog_pair!(
    "vyre-libs::catalog::reduce::workgroup_sum_f32",
    "vyre-primitives::reduce::workgroup_sum_f32"
);
catalog_pair!(
    "vyre-libs::catalog::text::char_class",
    "vyre-primitives::text::char_class"
);
catalog_pair!(
    "vyre-libs::catalog::text::line_index",
    "vyre-primitives::text::line_index"
);
catalog_pair!(
    "vyre-libs::catalog::text::utf8_shape_counts",
    "vyre-primitives::text::utf8_shape_counts"
);
catalog_pair!(
    "vyre-libs::catalog::text::utf8_validate",
    "vyre-primitives::text::utf8_validate"
);
catalog_pair!(
    "vyre-libs::catalog::visual::packed_rgba_map",
    "vyre-primitives::visual::packed_rgba_map"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_primitive_promotion_gate_accepts_padic_hensel_proof_evidence() {
        validate_research_primitive_promotions(RESEARCH_PRIMITIVE_PROMOTIONS)
            .expect("Fix: p-adic Hensel lift must carry proof-gate evidence.");
    }

    #[test]
    fn research_primitive_promotion_gate_rejects_empty_proof_evidence() {
        let promotions = [ResearchPrimitivePromotion {
            primitive_id: "vyre-primitives::math::hensel_lift_step",
            boundary: PrimitivePromotionBoundary::ProofGates(&[]),
        }];

        let error = validate_research_primitive_promotions(&promotions)
            .expect_err("Fix: empty proof gates must reject.");
        assert!(error.contains("no proof evidence"));
    }

    #[test]
    fn research_primitive_promotion_gate_accepts_registered_production_consumers() {
        let promotions = [ResearchPrimitivePromotion {
            primitive_id: "vyre-primitives::math::hensel_lift_step",
            boundary: PrimitivePromotionBoundary::ProductionConsumers(&[
                "vyre-libs::catalog::math::hensel_lift_step::consumer_a",
                "vyre-libs::catalog::math::hensel_lift_step::consumer_b",
            ]),
        }];

        validate_research_primitive_promotions(&promotions)
            .expect("Fix: registered catalog consumers must satisfy explicit consumer gates.");
    }

    #[test]
    fn research_primitive_promotion_gate_rejects_missing_production_consumers() {
        let promotions = [ResearchPrimitivePromotion {
            primitive_id: "vyre-primitives::math::hensel_lift_step",
            boundary: PrimitivePromotionBoundary::ProductionConsumers(&[
                "vyre-libs::catalog::math::hensel_lift_step::missing_consumer",
            ]),
        }];

        let error = validate_research_primitive_promotions(&promotions)
            .expect_err("Fix: missing production consumers must reject.");
        assert!(error.contains("missing_consumer"));
    }
}

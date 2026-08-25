//! `cargo_full run --bin xtask -- whats-similar --op-id <id>`  -  pre-write similarity query.
//!
//! Surfaces the "should I reimplement?" question at write-time, before
//! a near-duplicate op lands in the registry. Walks every registered
//! op (Tier 2, 2.5, and 3), fingerprints them, and reports the top-N
//! nearest matches by bigram-cosine structural similarity.
//!
//! The fingerprint is the same one `lego-audit` check 1 uses  -  bigram
//! cosine over the IR-shape fingerprint. Two ops with score ≥ 0.80 are
//! candidates for merging or for extracting a shared Tier-2.5 primitive.
//!
//! ## Usage
//!
//! ```text
//! # Score a registered op against everything else.
//! cargo_full run --bin xtask -- whats-similar --op-id vyre-libs::math::matmul_strassen_2x2
//!
//! # Top 10 instead of the default 5.
//! cargo_full run --bin xtask -- whats-similar --op-id vyre-libs::math::matmul --top 10
//!
//! # Lower the floor (defaults to 0.20  -  anything weaker is noise).
//! cargo_full run --bin xtask -- whats-similar --op-id ... --min 0.05
//!
//! # Scan the whole registered-op surface for near duplicates.
//! cargo_full run --bin xtask -- whats-similar --all --top 50
//! ```
//!
//! Pre-write workflow: register your candidate op (even a skeletal
//! `OpEntry { build: || trivial_program(), .. }` works), run
//! whats-similar against its id, decide whether to reuse, merge, or
//! ship as new. The fingerprint sees the IR shape, not the function
//! name, so renaming will not hide a duplicate.
//!
//! ## Why not file-based?
//!
//! A `.rs` file with un-registered ops cannot produce a Program without
//! the inventory plumbing, so the fingerprint cannot be computed
//! directly from source. `--op-id` requires the candidate to be a
//! registered (even draft) entry. This is the right gate: if you
//! cannot register the op, you do not yet know what shape it builds.

use std::path::PathBuf;
use std::process;

use crate::dedup_report::{
    duplicate_family_report, duplicate_report_generator_command, duplicate_report_json_path,
    duplicate_severity, registered_op_duplicate_family_id, registered_op_duplicate_subject,
    registered_op_owner_lane, structural_similarity, write_duplicate_report_json,
    DuplicateEvidence, DuplicateFamilyFinding, DuplicateFamilyReport, DuplicateSubject,
};
use crate::lego_audit::{collect_ops, OpInfo, Tier};

const DEFAULT_TOP_N: usize = 5;
const DEFAULT_MIN_SCORE: f64 = 0.20;
const DEFAULT_ALL_MIN_SCORE: f64 = 0.80;

pub(crate) fn run(args: &[String]) {
    let cli = match parse_args(args) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Fix: {err}");
            print_usage();
            process::exit(1);
        }
    };

    let ops = collect_ops();
    match &cli.mode {
        Mode::Target(op_id) => run_target_query(
            &ops,
            op_id,
            cli.top_n,
            cli.min_score,
            cli.duplicate_report_json.as_ref(),
        ),
        Mode::All => run_all_pairs_query(
            &ops,
            cli.top_n,
            cli.min_score,
            cli.duplicate_report_json.as_ref(),
        ),
    }
}

fn run_target_query(
    ops: &[OpInfo],
    op_id: &str,
    top_n: usize,
    min_score: f64,
    duplicate_report_json: Option<&PathBuf>,
) {
    let target = match ops.iter().find(|o| o.id == op_id) {
        Some(op) => op,
        None => {
            eprintln!(
                "Fix: op id `{op_id}` not found in any registry. Register the candidate via `inventory::submit! {{ OpEntry {{ id: \"...\", build: || ..., .. }} }}` before running whats-similar."
            );
            process::exit(1);
        }
    };

    let mut scored: Vec<(f64, bool, bool, &OpInfo)> = ops
        .iter()
        .filter(|o| o.id != target.id)
        .filter(|o| o.fingerprint.len() >= 10)
        .map(|o| {
            (
                structural_similarity(&target.fingerprint, &o.fingerprint),
                same_buffer_contract(target, o),
                same_centralized_family(target, o),
                o,
            )
        })
        .filter(|(s, _, _, _)| *s >= min_score)
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_n);
    if let Some(path) = duplicate_report_json {
        let generator_command = duplicate_report_generator_command(
            &format!("whats-similar --op-id {}", target.id),
            path,
        );
        let report = target_duplicate_report(target, &scored, &generator_command);
        if let Err(error) = write_duplicate_report_json(path, &report) {
            eprintln!(
                "Fix: whats-similar could not write duplicate family report `{}`: {error}",
                path.display()
            );
            process::exit(1);
        }
    }

    println!(
        "whats-similar: target `{}` (tier={}, own_nodes={}, composed_nodes={}, fingerprint={} bytes)",
        target.id,
        tier_label(target.tier),
        target.own_nodes,
        target.composed_nodes,
        target.fingerprint.len()
    );
    println!();

    if scored.is_empty() {
        println!(
            "  ✓ no neighbors at score ≥ {:.2}. The op shape is novel (or your fingerprint is too short).",
            min_score
        );
        return;
    }

    println!(
        "  Top {} matches by bigram-cosine structural similarity:",
        scored.len()
    );
    for (i, (score, same_contract, same_family, op)) in scored.iter().enumerate() {
        let verdict = pair_verdict(*score, *same_contract, *same_family);
        println!(
            "    {:>2}. {:>5.1}%  {}  ({})",
            i + 1,
            score * 100.0,
            op.id,
            verdict
        );
        println!(
            "         tier={} own={} composed={} children={}",
            tier_label(op.tier),
            op.own_nodes,
            op.composed_nodes,
            op.children.len()
        );
        if !same_contract {
            println!(
                "         contract=DIFFERENT target_buffers={} match_buffers={}",
                target.buffer_signature.len(),
                op.buffer_signature.len()
            );
        }
        if *same_family {
            println!(
                "         implementation=CENTRALIZED family={}",
                implementation_family(target).unwrap_or("unknown")
            );
        }
    }
    println!();
    println!(
        "  Bar: ≥ 0.95 = duplicate, ≥ 0.80 = very similar, ≥ 0.50 = same family, < 0.20 = unrelated."
    );
}

fn run_all_pairs_query(
    ops: &[OpInfo],
    top_n: usize,
    min_score: f64,
    duplicate_report_json: Option<&PathBuf>,
) {
    let eligible: Vec<&OpInfo> = ops.iter().filter(|op| op.fingerprint.len() >= 10).collect();
    let mut pairs: Vec<(f64, &OpInfo, &OpInfo)> = Vec::new();
    let mut contract_variants = 0usize;
    let mut centralized_family_variants = 0usize;
    let mut distinct_family_variants = 0usize;
    for left_index in 0..eligible.len() {
        for right in eligible.iter().skip(left_index + 1) {
            let left = eligible[left_index];
            let right = *right;
            let score = structural_similarity(&left.fingerprint, &right.fingerprint);
            if score >= min_score {
                if same_centralized_family(left, right) {
                    centralized_family_variants += 1;
                    continue;
                }
                if known_distinct_implementation_family(left, right) {
                    distinct_family_variants += 1;
                    continue;
                }
                if !same_buffer_contract(left, right) {
                    contract_variants += 1;
                    continue;
                }
                pairs.push((score, left, right));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    pairs.truncate(top_n);
    if let Some(path) = duplicate_report_json {
        let generator_command = duplicate_report_generator_command("whats-similar --all", path);
        let report = all_pairs_duplicate_report(&pairs, &generator_command);
        if let Err(error) = write_duplicate_report_json(path, &report) {
            eprintln!(
                "Fix: whats-similar could not write duplicate family report `{}`: {error}",
                path.display()
            );
            process::exit(1);
        }
    }

    println!(
        "whats-similar: scanned {} registered ops for all-pairs duplicate candidates (min={:.2}, top={})",
        eligible.len(),
        min_score,
        top_n
    );
    if contract_variants > 0 {
        println!(
            "  skipped {contract_variants} same-body pairs with different buffer contracts; these are wrapper/variant candidates, not raw duplicate ops."
        );
    }
    if centralized_family_variants > 0 {
        println!(
            "  skipped {centralized_family_variants} same-family pairs already routed through a centralized builder."
        );
    }
    if distinct_family_variants > 0 {
        println!(
            "  skipped {distinct_family_variants} known-distinct implementation-family pairs with shared scaffolding but different semantics."
        );
    }
    if pairs.is_empty() {
        println!("  no registered-op pairs crossed the duplicate/similarity floor.");
        return;
    }
    for (index, (score, left, right)) in pairs.iter().enumerate() {
        let verdict = match *score {
            s if s >= 0.95 => "DUPLICATE",
            s if s >= 0.80 => "VERY SIMILAR",
            s if s >= 0.50 => "SIMILAR",
            _ => "RELATED",
        };
        println!("  {:>2}. {:>5.1}%  {}", index + 1, score * 100.0, verdict);
        println!(
            "      A: {} tier={} own={} composed={}",
            left.id,
            tier_label(left.tier),
            left.own_nodes,
            left.composed_nodes
        );
        println!(
            "      B: {} tier={} own={} composed={}",
            right.id,
            tier_label(right.tier),
            right.own_nodes,
            right.composed_nodes
        );
    }
}

fn target_duplicate_report(
    target: &OpInfo,
    scored: &[(f64, bool, bool, &OpInfo)],
    generator_command: &str,
) -> DuplicateFamilyReport {
    let families = scored
        .iter()
        .map(|(score, same_contract, same_family, op)| {
            registered_op_duplicate_family(
                *score,
                target,
                *op,
                *same_contract,
                *same_family,
            )
        })
        .collect();
    duplicate_family_report(
        generator_command,
        "registered-op-ir-shape",
        families,
    )
}

fn all_pairs_duplicate_report(
    pairs: &[(f64, &OpInfo, &OpInfo)],
    generator_command: &str,
) -> DuplicateFamilyReport {
    let families = pairs
        .iter()
        .map(|(score, left, right)| {
            registered_op_duplicate_family(*score, *left, *right, true, false)
        })
        .collect();
    duplicate_family_report(
        generator_command,
        "registered-op-ir-shape",
        families,
    )
}

fn registered_op_duplicate_family(
    score: f64,
    left: &OpInfo,
    right: &OpInfo,
    same_contract: bool,
    same_family: bool,
) -> DuplicateFamilyFinding {
    DuplicateFamilyFinding {
        family_id: registered_op_duplicate_family_id(&left.id, &right.id),
        detector: "whats-similar".to_string(),
        severity: duplicate_severity(score),
        score,
        left: registered_op_subject(left),
        right: registered_op_subject(right),
        import_owner: registered_op_import_owner(left, right, same_family),
        import_target: registered_op_import_target(left, right, same_contract, same_family),
        evidence: DuplicateEvidence {
            similarity_metric: "ir-shape-bigram-cosine",
            left_metric: format!(
                "tier={}:own_nodes={}:composed_nodes={}:fingerprint_bytes={}",
                tier_label(left.tier),
                left.own_nodes,
                left.composed_nodes,
                left.fingerprint.len()
            ),
            right_metric: format!(
                "tier={}:own_nodes={}:composed_nodes={}:fingerprint_bytes={}",
                tier_label(right.tier),
                right.own_nodes,
                right.composed_nodes,
                right.fingerprint.len()
            ),
            dedup_action: if same_family {
                "keep_shared_builder_family_and_remove_duplicate_registration"
            } else if same_contract {
                "extract_shared_primitive_or_reuse_existing_op"
            } else {
                "share_helper_without_merging_distinct_contracts"
            },
        },
    }
}

fn registered_op_subject(op: &OpInfo) -> DuplicateSubject {
    registered_op_duplicate_subject(
        &op.id,
        &op.fingerprint,
        op.own_nodes + op.composed_nodes,
    )
}

fn registered_op_import_owner(left: &OpInfo, right: &OpInfo, same_family: bool) -> String {
    if same_family {
        return implementation_family(left)
            .or_else(|| implementation_family(right))
            .map(ToString::to_string)
            .unwrap_or_else(|| registered_op_owner_lane(&left.id).to_string());
    }
    if left.tier <= right.tier {
        registered_op_owner_lane(&left.id).to_string()
    } else {
        registered_op_owner_lane(&right.id).to_string()
    }
}

fn registered_op_import_target(
    left: &OpInfo,
    right: &OpInfo,
    same_contract: bool,
    same_family: bool,
) -> String {
    if same_family {
        return implementation_family(left)
            .or_else(|| implementation_family(right))
            .map(ToString::to_string)
            .unwrap_or_else(|| "shared_registered_op_family".to_string());
    }
    if !same_contract {
        return "shared_helper_for_contract_variants".to_string();
    }
    if left.tier <= right.tier {
        left.id.clone()
    } else {
        right.id.clone()
    }
}

fn same_buffer_contract(left: &OpInfo, right: &OpInfo) -> bool {
    left.buffer_signature == right.buffer_signature
}

fn same_centralized_family(left: &OpInfo, right: &OpInfo) -> bool {
    let Some(left_family) = implementation_family(left) else {
        return false;
    };
    implementation_family(right) == Some(left_family)
}

fn known_distinct_implementation_family(left: &OpInfo, right: &OpInfo) -> bool {
    known_distinct_implementation_family_id(&left.id, &right.id)
}

fn known_distinct_implementation_family_id(left_id: &str, right_id: &str) -> bool {
    let Some(left_family) = implementation_family_id(left_id) else {
        return false;
    };
    let Some(right_family) = implementation_family_id(right_id) else {
        return false;
    };
    matches!(
        (left_family, right_family),
        (
            "vyre-intrinsics::hardware::barrier_identity_u32_program",
            "vyre-intrinsics::hardware::unary_u32_program"
        ) | (
            "vyre-intrinsics::hardware::unary_u32_program",
            "vyre-intrinsics::hardware::barrier_identity_u32_program"
        )
    )
}

fn implementation_family(op: &OpInfo) -> Option<&'static str> {
    implementation_family_id(&op.id)
}

fn implementation_family_id(op_id: &str) -> Option<&'static str> {
    match op_id {
        "vyre-primitives::bitset::and"
        | "vyre-primitives::bitset::and_not"
        | "vyre-primitives::bitset::or"
        | "vyre-primitives::bitset::stochastic_and_mul"
        | "vyre-primitives::bitset::xor" => Some("vyre-primitives::bitset::binary_word"),
        "vyre-primitives::bitset::and_into"
        | "vyre-primitives::bitset::and_not_into"
        | "vyre-primitives::bitset::copy"
        | "vyre-primitives::bitset::or_into"
        | "vyre-primitives::bitset::xor_into" => {
            Some("vyre-primitives::bitset::target_operand_word")
        }
        "vyre-primitives::bitset::equal" | "vyre-primitives::bitset::subset_of" => {
            Some("vyre-primitives::bitset::relation")
        }
        "vyre-primitives::bitset::set_bit" | "vyre-primitives::bitset::clear_bit" => {
            Some("vyre-primitives::bitset::bit_update")
        }
        "vyre-primitives::predicate::literal_of"
        | "vyre-primitives::predicate::node_kind_eq"
        | "vyre-primitives::label::resolve_family" => Some("vyre-primitives::nodeset_filter"),
        "vyre-primitives::graph::vast_walk_preorder"
        | "vyre-primitives::graph::vast_walk_postorder" => {
            Some("vyre-primitives::graph::vast_tree_walk_order")
        }
        "vyre-primitives::graph::csr_forward_traverse"
        | "vyre-primitives::graph::csr_backward_traverse"
        | "vyre-primitives::graph::csr_frontier_degree_sum"
        | "vyre-primitives::graph::tensor_flow_forward"
        | "vyre-primitives::predicate::call_to"
        | "vyre-primitives::predicate::edge"
        | "vyre-primitives::predicate::return_value_of"
        | "vyre-primitives::predicate::arg_of"
        | "vyre-primitives::predicate::size_argument_of" => {
            Some("vyre-primitives::graph::csr_frontier_step")
        }
        "vyre-intrinsics::hardware::workgroup_barrier"
        | "vyre-intrinsics::hardware::storage_barrier" => {
            Some("vyre-intrinsics::hardware::barrier_identity_u32_program")
        }
        "vyre-intrinsics::hardware::bit_reverse_u32"
        | "vyre-intrinsics::hardware::popcount_u32" => {
            Some("vyre-intrinsics::hardware::unary_u32_program")
        }
        "vyre-primitives::graph::monoidal_compose"
        | "vyre-primitives::math::tensor_network_pair_contract"
        | "vyre-primitives::math::semiring_gemm" => {
            Some("vyre-primitives::fixed_u32_matmul::u32_matmul_program")
        }
        "vyre-primitives::math::sinkhorn_scale" | "vyre-primitives::math::gaussian_rdp_step" => {
            Some("vyre-primitives::math::u32_binary_map")
        }
        "vyre-primitives::math::iht_threshold" | "vyre-primitives::math::mp_edge_clip" => {
            Some("vyre-primitives::math::u32_vector_scalar_map")
        }
        "vyre-primitives::reduce::sum"
        | "vyre-primitives::reduce::min"
        | "vyre-primitives::reduce::max"
        | "vyre-primitives::reduce::any"
        | "vyre-primitives::reduce::all" => Some("vyre-primitives::reduce::atomic_grid_stride_u32"),
        "vyre-primitives::reduce::gather" | "vyre-primitives::reduce::scatter" => {
            Some("vyre-primitives::reduce::indexed_move")
        }
        "vyre-primitives::reduce::workgroup_sum_f32"
        | "vyre-primitives::reduce::workgroup_sum_u32"
        | "vyre-primitives::reduce::workgroup_max_f32" => {
            Some("vyre-primitives::reduce::workgroup_tree")
        }
        "vyre-libs::math::atomic::atomic_add_u32"
        | "vyre-libs::math::atomic::atomic_and_u32"
        | "vyre-libs::math::atomic::atomic_exchange_u32"
        | "vyre-libs::math::atomic::atomic_max_u32"
        | "vyre-libs::math::atomic::atomic_min_u32"
        | "vyre-libs::math::atomic::atomic_or_u32"
        | "vyre-libs::math::atomic::atomic_xor_u32" => {
            Some("vyre-libs::math::atomic::build_atomic_serial")
        }
        "vyre-libs::logical::nand"
        | "vyre-libs::logical::nor"
        | "vyre-libs::math::algebra::join"
        | "vyre-libs::math::algebra::meet"
        | "vyre-libs::math::algebra::minplus_mul"
        | "vyre-libs::math::avg_floor" => {
            Some("vyre-libs::math::elementwise::u32_elementwise_binary")
        }
        "vyre-libs::math::lzcnt_u32"
        | "vyre-libs::math::tzcnt_u32"
        | "vyre-libs::math::wrapping_neg" => {
            Some("vyre-libs::math::elementwise::u32_elementwise_unary")
        }
        "vyre-libs::nn::gelu" | "vyre-libs::nn::leaky_relu_sq" => {
            Some("vyre-libs::nn::activation::f32_unary_activation_program")
        }
        "vyre-libs::nn::rms_norm" | "vyre-libs::nn::softmax" => {
            Some("vyre-libs::builder::strided_writeback_child")
        }
        "vyre-libs::parsing::c11_gnu_inline_asm_pass"
        | "vyre-libs::parsing::opt_stack_layout_generation" => {
            Some("vyre-libs::compiler::atomic_collect_u32")
        }
        "vyre-libs::parsing::c_sema_scope.scope"
        | "vyre-libs::parsing::c_sema_scope.scope.brace"
        | "vyre-libs::parsing::c_sema_scope.scope.function_parameters"
        | "vyre-libs::parsing::c_sema_scope.decl"
        | "vyre-libs::parsing::c_sema_scope.identifier_intern" => {
            Some("vyre-libs::parsing::c_sema_scope_phase")
        }
        _ => None,
    }
}

fn pair_verdict(score: f64, same_contract: bool, same_family: bool) -> &'static str {
    if same_family {
        return match score {
            s if s >= 0.95 => {
                "CENTRALIZED FAMILY  -  same emitted kernel is already routed through a shared builder"
            }
            s if s >= 0.80 => {
                "CENTRALIZED FAMILY  -  similar emitted kernel already shares implementation plumbing"
            }
            _ => "loosely related centralized family",
        };
    }
    if !same_contract {
        return match score {
            s if s >= 0.95 => {
                "CONTRACT VARIANT  -  same body shape but different buffer contract; share helpers, do not merge ops"
            }
            s if s >= 0.80 => {
                "CONTRACT-SHAPE FAMILY  -  similar body under different buffer contract"
            }
            _ => "loosely related contract variant",
        };
    }
    match score {
        s if s >= 0.95 => "DUPLICATE  -  almost certainly the same shape; reuse instead",
        s if s >= 0.80 => "VERY SIMILAR  -  extract shared body to vyre-primitives or reuse",
        s if s >= 0.50 => "SIMILAR  -  same family; consider whether divergence is justified",
        _ => "loosely related",
    }
}

fn tier_label(t: Tier) -> &'static str {
    match t {
        Tier::T2 => "T2",
        Tier::T2_5 => "T2.5",
        Tier::T3 => "T3",
        Tier::Other => "?",
    }
}

#[derive(Debug)]
struct Cli {
    mode: Mode,
    top_n: usize,
    min_score: f64,
    duplicate_report_json: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
enum Mode {
    Target(String),
    All,
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut op_id: Option<String> = None;
    let mut all = false;
    let mut top_n = DEFAULT_TOP_N;
    let mut min_score = None;
    let mut duplicate_report_json = None;
    let mut iter = args.iter().skip(2);
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--all" => {
                all = true;
            }
            "--op-id" => {
                op_id = Some(
                    iter.next()
                        .cloned()
                        .ok_or_else(|| "--op-id needs a value".to_string())?,
                );
            }
            "--top" => {
                let v = iter
                    .next()
                    .ok_or_else(|| "--top needs a value".to_string())?;
                top_n = v
                    .parse::<usize>()
                    .map_err(|e| format!("--top must be a positive integer ({e})"))?;
                if top_n == 0 {
                    return Err("--top must be > 0".to_string());
                }
            }
            "--min" => {
                let v = iter
                    .next()
                    .ok_or_else(|| "--min needs a value".to_string())?;
                let parsed_min_score = v
                    .parse::<f64>()
                    .map_err(|e| format!("--min must be a float in [0,1] ({e})"))?;
                if !(0.0..=1.0).contains(&parsed_min_score) {
                    return Err("--min must be in [0,1]".to_string());
                }
                min_score = Some(parsed_min_score);
            }
            "--duplicate-report-json" => {
                duplicate_report_json = Some(duplicate_report_json_path(
                    "--duplicate-report-json",
                    iter.next().map(String::as_str),
                    "--duplicate-report-json needs a value",
                )?);
            }
            "--file" => {
                return Err(
                    "Fix: whats-similar compares registered OpEntry programs; register the candidate and pass its id with --op-id <id>"
                        .to_string(),
                );
            }
            other => return Err(format!("unknown arg `{other}`")),
        }
    }
    if all && op_id.is_some() {
        return Err("--all and --op-id are mutually exclusive".to_string());
    }
    let mode = if all {
        Mode::All
    } else {
        Mode::Target(op_id.ok_or_else(|| "--op-id is required unless --all is set".to_string())?)
    };
    let min_score = min_score.unwrap_or(match &mode {
        Mode::All => DEFAULT_ALL_MIN_SCORE,
        Mode::Target(_) => DEFAULT_MIN_SCORE,
    });
    Ok(Cli {
        mode,
        top_n,
        min_score,
        duplicate_report_json,
    })
}

fn print_usage() {
    eprintln!(
        "Usage: cargo_full run --bin xtask -- whats-similar --op-id <id> [--top N] [--min FLOAT]\n\
         Usage: cargo_full run --bin xtask -- whats-similar --all [--top N] [--min FLOAT]\n\
         Add --duplicate-report-json PATH to write the shared duplicate-family report schema.\n\
         \n\
         Pre-write similarity query: report the top-N ops most structurally\n\
         similar to <id> by IR-shape bigram cosine. Use BEFORE shipping a new\n\
         op to detect reinvention. Use --all to find duplicate candidates\n\
         across the entire registered-op surface.\n\
         \n\
         Defaults: --top 5, --min 0.20 for --op-id, --min 0.80 for --all."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_requires_op_id() {
        let args = vec!["xtask".to_string(), "whats-similar".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_with_op_id_and_defaults() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--op-id".to_string(),
            "vyre-libs::math::matmul".to_string(),
        ];
        let cli = parse_args(&args).unwrap();
        assert_eq!(
            cli.mode,
            Mode::Target("vyre-libs::math::matmul".to_string())
        );
        assert_eq!(cli.top_n, DEFAULT_TOP_N);
        assert!((cli.min_score - DEFAULT_MIN_SCORE).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_top_and_min_overrides() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--op-id".to_string(),
            "x".to_string(),
            "--top".to_string(),
            "10".to_string(),
            "--min".to_string(),
            "0.05".to_string(),
        ];
        let cli = parse_args(&args).unwrap();
        assert_eq!(cli.mode, Mode::Target("x".to_string()));
        assert_eq!(cli.top_n, 10);
        assert!((cli.min_score - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_duplicate_report_json_path() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--all".to_string(),
            "--duplicate-report-json".to_string(),
            "release/evidence/dedup/registered-op-duplicates.json".to_string(),
        ];
        let cli = parse_args(&args).unwrap();
        assert_eq!(cli.mode, Mode::All);
        assert_eq!(
            cli.duplicate_report_json,
            Some(PathBuf::from(
                "release/evidence/dedup/registered-op-duplicates.json"
            ))
        );
    }

    #[test]
    fn parse_all_sets_duplicate_floor() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--all".to_string(),
        ];
        let cli = parse_args(&args).unwrap();
        assert_eq!(cli.mode, Mode::All);
        assert_eq!(cli.top_n, DEFAULT_TOP_N);
        assert!((cli.min_score - DEFAULT_ALL_MIN_SCORE).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_rejects_all_with_op_id() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--all".to_string(),
            "--op-id".to_string(),
            "x".to_string(),
        ];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_rejects_top_zero() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--op-id".to_string(),
            "x".to_string(),
            "--top".to_string(),
            "0".to_string(),
        ];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_rejects_min_out_of_range() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--op-id".to_string(),
            "x".to_string(),
            "--min".to_string(),
            "1.5".to_string(),
        ];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_rejects_unknown_arg() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--op-id".to_string(),
            "x".to_string(),
            "--bogus".to_string(),
        ];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_file_arg_returns_helpful_error() {
        let args = vec![
            "xtask".to_string(),
            "whats-similar".to_string(),
            "--file".to_string(),
            "x.rs".to_string(),
        ];
        let err = parse_args(&args).unwrap_err();
        assert!(err.contains("register the candidate"));
    }

    #[test]
    fn implementation_family_tracks_shared_builders() {
        assert_eq!(
            implementation_family_id("vyre-primitives::bitset::and"),
            implementation_family_id("vyre-primitives::bitset::stochastic_and_mul")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::predicate::size_argument_of"),
            implementation_family_id("vyre-primitives::graph::csr_backward_traverse")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::graph::csr_forward_traverse"),
            implementation_family_id("vyre-primitives::graph::csr_backward_traverse")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::graph::csr_forward_traverse"),
            implementation_family_id("vyre-primitives::graph::csr_frontier_degree_sum")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::graph::csr_forward_traverse"),
            implementation_family_id("vyre-primitives::graph::tensor_flow_forward")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::graph::vast_walk_preorder"),
            implementation_family_id("vyre-primitives::graph::vast_walk_postorder")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::math::semiring_gemm"),
            implementation_family_id("vyre-primitives::math::tensor_network_pair_contract")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::math::semiring_gemm"),
            implementation_family_id("vyre-primitives::graph::monoidal_compose")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::math::sinkhorn_scale"),
            implementation_family_id("vyre-primitives::math::gaussian_rdp_step")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::math::iht_threshold"),
            implementation_family_id("vyre-primitives::math::mp_edge_clip")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::predicate::node_kind_eq"),
            implementation_family_id("vyre-primitives::label::resolve_family")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::bitset::and_not"),
            implementation_family_id("vyre-primitives::bitset::or")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::bitset::and_into"),
            implementation_family_id("vyre-primitives::bitset::xor_into")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::bitset::copy"),
            implementation_family_id("vyre-primitives::bitset::and_into")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::bitset::set_bit"),
            implementation_family_id("vyre-primitives::bitset::clear_bit")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::reduce::workgroup_sum_f32"),
            implementation_family_id("vyre-primitives::reduce::workgroup_max_f32")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::reduce::sum"),
            implementation_family_id("vyre-primitives::reduce::any")
        );
        assert_eq!(
            implementation_family_id("vyre-primitives::reduce::gather"),
            implementation_family_id("vyre-primitives::reduce::scatter")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::math::atomic::atomic_or_u32"),
            implementation_family_id("vyre-libs::math::atomic::atomic_xor_u32")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::logical::nand"),
            implementation_family_id("vyre-libs::logical::nor")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::logical::nand"),
            implementation_family_id("vyre-libs::math::algebra::meet")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::math::algebra::join"),
            implementation_family_id("vyre-libs::math::avg_floor")
        );
        assert_eq!(
            implementation_family_id("vyre-intrinsics::hardware::bit_reverse_u32"),
            implementation_family_id("vyre-intrinsics::hardware::popcount_u32")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::math::lzcnt_u32"),
            implementation_family_id("vyre-libs::math::tzcnt_u32")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::math::wrapping_neg"),
            implementation_family_id("vyre-libs::math::lzcnt_u32")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::nn::gelu"),
            implementation_family_id("vyre-libs::nn::leaky_relu_sq")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::nn::rms_norm"),
            implementation_family_id("vyre-libs::nn::softmax")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::parsing::c11_gnu_inline_asm_pass"),
            implementation_family_id("vyre-libs::parsing::opt_stack_layout_generation")
        );
        assert_eq!(
            implementation_family_id("vyre-libs::parsing::c_sema_scope.scope"),
            implementation_family_id("vyre-libs::parsing::c_sema_scope.identifier_intern")
        );
        assert!(known_distinct_implementation_family_id(
            "vyre-intrinsics::hardware::workgroup_barrier",
            "vyre-intrinsics::hardware::bit_reverse_u32"
        ));
        assert!(!known_distinct_implementation_family_id(
            "vyre-intrinsics::hardware::workgroup_barrier",
            "vyre-intrinsics::hardware::storage_barrier"
        ));
    }

    #[test]
    fn unrelated_ops_do_not_gain_family_suppression() {
        assert_ne!(
            implementation_family_id("vyre-libs::math::atomic::atomic_or_u32"),
            implementation_family_id("vyre-primitives::bitset::and")
        );
        assert!(implementation_family_id("unknown::op").is_none());
    }
}
